//! DevBench: a control center for home servers and their virtual machines.

mod config;
mod error;
mod ssh;
mod sshconf;
mod status;
mod vnc;
mod wol;

use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::{Context, anyhow};
use serde::Serialize;
use tauri::State;

use config::{Config, VncMode};
use error::CmdResult;
use ssh::sh_quote;

struct AppState {
    config: RwLock<Config>,
    pool: ssh::Pool,
    bridge: Arc<vnc::Bridge>,
    /// A config file that could not be read at start; shown in the UI instead of failing silently.
    startup_error: Option<String>,
}

impl AppState {
    fn snapshot(&self) -> Config {
        self.config.read().unwrap().clone()
    }
}

#[tauri::command]
fn get_config(state: State<'_, AppState>) -> Config {
    state.snapshot()
}

#[tauri::command]
fn startup_error(state: State<'_, AppState>) -> Option<String> {
    state.startup_error.clone()
}

#[tauri::command]
fn save_config(state: State<'_, AppState>, mut config: Config) -> CmdResult<Config> {
    config.normalize()?;
    config::save(&config)?;
    *state.config.write().unwrap() = config.clone();
    state.pool.clear();
    Ok(config)
}

#[tauri::command]
fn reload_config(state: State<'_, AppState>) -> CmdResult<Config> {
    let config = config::load()?;
    *state.config.write().unwrap() = config.clone();
    state.pool.clear();
    Ok(config)
}

#[tauri::command]
fn config_path() -> CmdResult<String> {
    Ok(config::config_path()?.display().to_string())
}

#[tauri::command]
fn ssh_config_hosts() -> Vec<sshconf::SshHostEntry> {
    sshconf::list_hosts()
}

#[tauri::command]
async fn machine_status(state: State<'_, AppState>, machine_id: String) -> CmdResult<status::MachineStatus> {
    let cfg = state.snapshot();
    let m = cfg.machine(&machine_id)?;
    let ep = sshconf::machine_endpoint(m)?;
    let out = state.pool.exec(&ep, &status::script(m), status::STATUS_TIMEOUT).await?;
    Ok(status::parse(&out.stdout))
}

#[tauri::command]
async fn wake(state: State<'_, AppState>, machine_id: String) -> CmdResult<()> {
    let cfg = state.snapshot();
    let m = cfg.machine(&machine_id)?;
    if m.mac.trim().is_empty() {
        return Err(anyhow!("{} has no MAC address configured", m.name).into());
    }
    wol::wake(&m.mac, &m.wol_broadcast, m.wol_port).await?;
    Ok(())
}

#[tauri::command]
async fn power(state: State<'_, AppState>, machine_id: String, action: String) -> CmdResult<()> {
    let verb = match action.as_str() {
        "poweroff" | "reboot" => action.as_str(),
        _ => return Err(anyhow!("unknown power action '{action}'").into()),
    };
    let cfg = state.snapshot();
    let m = cfg.machine(&machine_id)?;
    let ep = sshconf::machine_endpoint(m)?;
    let out = state.pool.exec(&ep, &format!("sudo -n systemctl {verb}"), Duration::from_secs(20)).await?;
    state.pool.evict(&ep);
    match out.code {
        Some(0) | None => Ok(()),
        Some(_) if out.stderr.contains("password") => Err(anyhow!(
            "sudo asks for a password. Allow 'systemctl {verb}' without one (see docs/sudoers-devbench in the DevBench repository)."
        )
        .into()),
        Some(c) => Err(anyhow!("systemctl {verb} failed ({c}): {}", out.stderr.trim()).into()),
    }
}

#[tauri::command]
async fn vm_action(state: State<'_, AppState>, machine_id: String, vm_id: String, action: String) -> CmdResult<()> {
    let verb = match action.as_str() {
        "start" | "shutdown" | "reboot" | "destroy" => action.as_str(),
        _ => return Err(anyhow!("unknown VM action '{action}'").into()),
    };
    let cfg = state.snapshot();
    let m = cfg.machine(&machine_id)?;
    let vm = m.vms.iter().find(|v| v.id == vm_id).context("unknown VM")?;
    let ep = sshconf::machine_endpoint(m)?;
    let cmd = format!("LC_ALL=C virsh -c {} {verb} {}", sh_quote(&vm.libvirt_uri), sh_quote(&vm.libvirt_name));
    let out = state.pool.exec(&ep, &cmd, Duration::from_secs(60)).await?;
    if out.code != Some(0) {
        let msg = if out.stderr.trim().is_empty() { out.stdout.trim() } else { out.stderr.trim() };
        return Err(anyhow!("virsh {verb}: {msg}").into());
    }
    Ok(())
}

#[tauri::command]
async fn list_domains(state: State<'_, AppState>, machine_id: String, uri: String) -> CmdResult<Vec<String>> {
    let cfg = state.snapshot();
    let m = cfg.machine(&machine_id)?;
    let ep = sshconf::machine_endpoint(m)?;
    let uri = if uri.trim().is_empty() { "qemu:///system".to_string() } else { uri };
    let out = state
        .pool
        .exec(&ep, &format!("LC_ALL=C virsh -c {} list --all --name", sh_quote(&uri)), Duration::from_secs(20))
        .await?;
    if out.code != Some(0) {
        return Err(anyhow!("virsh list: {}", out.stderr.trim()).into());
    }
    Ok(out.stdout.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect())
}

#[tauri::command]
fn trust_host_key(state: State<'_, AppState>, host: String, port: u16, key: String) -> CmdResult<()> {
    state.pool.trust(&host, port, &key)?;
    Ok(())
}

#[derive(Serialize)]
struct VncTarget {
    url: String,
    title: String,
}

#[tauri::command]
async fn vnc_open(state: State<'_, AppState>, machine_id: String, vm_id: String) -> CmdResult<VncTarget> {
    let cfg = state.snapshot();
    let m = cfg.machine(&machine_id)?;
    let vm = m.vms.iter().find(|v| v.id == vm_id).context("unknown VM")?;
    let ep = sshconf::machine_endpoint(m)?;
    let (conn, channel) = match vm.vnc_mode {
        VncMode::None => return Err(anyhow!("VNC is switched off for {}", vm.name).into()),
        VncMode::Host => state.pool.forward(&ep, &vm.vnc_host, vm.vnc_port).await?,
        VncMode::Guest => {
            if vm.guest_ssh_user.trim().is_empty() {
                return Err(anyhow!("{}: set the guest SSH user in the settings", vm.name).into());
            }
            let addr = status::guest_address(&state.pool, &ep, vm).await?;
            let guest = sshconf::guest_endpoint(&ep, vm, &addr);
            state
                .pool
                .forward(&guest, &vm.vnc_host, vm.vnc_port)
                .await
                .with_context(|| format!("VNC server in {} ({}:{})", vm.name, vm.vnc_host, vm.vnc_port))?
        }
    };
    let url = state.bridge.register(conn, channel).await?;
    Ok(VncTarget { url, title: format!("{} · {}", vm.name, m.name) })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (config, load_error) = match config::load() {
        Ok(c) => (c, None),
        Err(e) => (Config::default(), Some(format!("{e:#}"))),
    };
    let known_hosts = config::config_dir().map(|d| d.join("known_hosts")).unwrap_or_else(|_| "known_hosts".into());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            config: RwLock::new(config),
            pool: ssh::Pool::new(known_hosts),
            bridge: Default::default(),
            startup_error: load_error,
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            startup_error,
            save_config,
            reload_config,
            config_path,
            ssh_config_hosts,
            machine_status,
            wake,
            power,
            vm_action,
            list_domains,
            trust_host_key,
            vnc_open,
        ])
        .run(tauri::generate_context!())
        .expect("error while running DevBench");
}

/// Live checks against a real machine, driven by environment variables (nothing about any
/// setup is stored here). Run with:
/// `DEVBENCH_TEST_HOST=<ssh alias> DEVBENCH_TEST_VM=<libvirt name> DEVBENCH_TEST_GUEST_USER=<user> cargo test -- --ignored --nocapture`
#[cfg(test)]
mod live_tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn status_and_vnc_tunnel() {
        let host = std::env::var("DEVBENCH_TEST_HOST").expect("DEVBENCH_TEST_HOST");
        let vm_name = std::env::var("DEVBENCH_TEST_VM").ok();
        let mut m = config::Machine { id: "m".into(), name: host.clone(), ssh_host: host, ..Default::default() };
        if let Some(name) = &vm_name {
            m.vms.push(config::Vm {
                id: "v".into(),
                name: name.clone(),
                libvirt_name: name.clone(),
                guest_ssh_user: std::env::var("DEVBENCH_TEST_GUEST_USER").unwrap_or_default(),
                ..Default::default()
            });
        }
        let pool = ssh::Pool::new(std::env::temp_dir().join("devbench-test-known_hosts"));
        let ep = sshconf::machine_endpoint(&m).unwrap();
        println!("endpoint: {}", ep.cache_key());
        let t = std::time::Instant::now();
        let out = pool.exec(&ep, &status::script(&m), status::STATUS_TIMEOUT).await.map_err(error::AppError::from);
        let out = match out {
            Ok(o) => o,
            Err(e) => panic!("{}", serde_json::to_string(&e).unwrap()),
        };
        let st = status::parse(&out.stdout);
        println!("status in {:?}: {}", t.elapsed(), serde_json::to_string_pretty(&st).unwrap());
        let t = std::time::Instant::now();
        pool.exec(&ep, "true", status::STATUS_TIMEOUT).await.unwrap();
        println!("second command on the pooled connection: {:?}", t.elapsed());

        let Some(vm) = m.vms.first() else { return };
        if vm.guest_ssh_user.is_empty() || !st.vms[0].running {
            return;
        }
        let addr = status::guest_address(&pool, &ep, vm).await.unwrap();
        println!("guest address: {addr}");
        let guest = sshconf::guest_endpoint(&ep, vm, &addr);
        let (_conn, ch) = pool.forward(&guest, &vm.vnc_host, vm.vnc_port).await.map_err(error::AppError::from).unwrap();
        use tokio::io::AsyncReadExt;
        let mut s = ch.into_stream();
        let mut hello = [0u8; 12];
        s.read_exact(&mut hello).await.unwrap();
        println!("VNC server greeting: {:?}", String::from_utf8_lossy(&hello));
        assert!(hello.starts_with(b"RFB "));
    }
}
