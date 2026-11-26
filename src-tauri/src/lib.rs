//! DevBench: a control center for home servers and their virtual machines.

mod activity;
mod config;
mod core;
mod error;
mod ssh;
mod sshconf;
mod status;
mod vnc;
mod wol;

use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::config::Config;
use crate::core::{Core, PowerAction, VmAction};
use crate::error::CmdResult;

struct AppState {
    core: Core,
    bridge: Arc<vnc::Bridge>,
    /// A config file that could not be read at start; shown in the UI instead of failing silently.
    startup_error: Option<String>,
}

#[tauri::command]
fn get_config(state: State<'_, AppState>) -> Config {
    state.core.config()
}

#[tauri::command]
fn startup_error(state: State<'_, AppState>) -> Option<String> {
    state.startup_error.clone()
}

#[tauri::command]
fn save_config(state: State<'_, AppState>, config: Config) -> CmdResult<Config> {
    Ok(state.core.save_config(config)?)
}

#[tauri::command]
fn reload_config(state: State<'_, AppState>) -> CmdResult<Config> {
    Ok(state.core.reload_config()?)
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
    Ok(state.core.status(&machine_id).await?)
}

#[tauri::command]
async fn wake(state: State<'_, AppState>, machine_id: String) -> CmdResult<()> {
    Ok(state.core.wake(&machine_id).await?)
}

#[tauri::command]
async fn power(state: State<'_, AppState>, machine_id: String, action: String) -> CmdResult<()> {
    Ok(state.core.power(&machine_id, PowerAction::parse(&action)?).await?)
}

#[tauri::command]
async fn vm_action(state: State<'_, AppState>, machine_id: String, vm_id: String, action: String) -> CmdResult<()> {
    Ok(state.core.vm_action(&machine_id, &vm_id, VmAction::parse(&action)?).await?)
}

#[tauri::command]
async fn list_domains(state: State<'_, AppState>, machine_id: String, uri: String) -> CmdResult<Vec<String>> {
    Ok(state.core.list_domains(&machine_id, &uri).await?)
}

#[tauri::command]
fn trust_host_key(state: State<'_, AppState>, host: String, port: u16, key: String) -> CmdResult<()> {
    Ok(state.core.trust_host_key(&host, port, &key)?)
}

#[tauri::command]
fn activity(limit: usize) -> Vec<activity::Entry> {
    activity::recent(limit.min(1000))
}

#[derive(Serialize)]
struct VncTarget {
    url: String,
    title: String,
}

#[tauri::command]
async fn vnc_open(state: State<'_, AppState>, machine_id: String, vm_id: String) -> CmdResult<VncTarget> {
    let (conn, channel, title) = state.core.vnc_channel(&machine_id, &vm_id).await?;
    let url = state.bridge.register(conn, channel).await?;
    Ok(VncTarget { url, title })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (core, load_error) = Core::load();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // The updater is configured only in release builds (tauri.release.conf.json); a plain
            // `tauri build` from source has no update channel and no signing key.
            #[cfg(desktop)]
            if app.config().plugins.0.contains_key("updater") {
                app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
                app.handle().plugin(tauri_plugin_process::init())?;
            }
            Ok(())
        })
        .manage(AppState { core, bridge: Default::default(), startup_error: load_error })
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
            activity,
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
