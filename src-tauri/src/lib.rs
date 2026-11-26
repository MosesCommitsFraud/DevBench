//! DevBench: a control center for home servers and their virtual machines.

mod activity;
mod agents;
mod config;
mod core;
mod error;
mod files;
mod mcp;
mod ssh;
mod sshconf;
mod status;
mod vnc;
mod vncclient;
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
async fn fs_list(state: State<'_, AppState>, loc: files::Location, path: Option<String>) -> CmdResult<files::Listing> {
    let fs = state.core.fs(&loc).await?;
    let path = match path.filter(|p| !p.trim().is_empty()) {
        Some(p) => p,
        None => fs.home().await?,
    };
    match fs.list(&path).await {
        Ok(l) => Ok(l),
        Err(e) => {
            state.core.forget_fs(&loc).await;
            Err(e.into())
        }
    }
}

#[tauri::command]
async fn fs_mkdir(state: State<'_, AppState>, loc: files::Location, dir: String, name: String) -> CmdResult<()> {
    let fs = state.core.fs(&loc).await?;
    Ok(fs.mkdir(&fs.join(&dir, &name)).await?)
}

#[tauri::command]
async fn fs_rename(state: State<'_, AppState>, loc: files::Location, dir: String, path: String, name: String) -> CmdResult<()> {
    let fs = state.core.fs(&loc).await?;
    Ok(fs.rename(&path, &fs.join(&dir, &name)).await?)
}

#[tauri::command]
async fn fs_delete(state: State<'_, AppState>, loc: files::Location, paths: Vec<String>) -> CmdResult<()> {
    let fs = state.core.fs(&loc).await?;
    for p in paths {
        fs.remove(&p).await?;
    }
    Ok(())
}

/// Starts a copy or move in the background; progress arrives as `transfer` events.
#[tauri::command]
fn fs_copy(
    app: tauri::AppHandle,
    id: String,
    from: files::Location,
    sources: Vec<String>,
    to: files::Location,
    dest: String,
    move_files: bool,
) {
    use tauri::{Emitter, Manager};
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let emit = |p: &files::Progress| {
            let _ = app.emit("transfer", p);
        };
        let result = state.core.copy(id.clone(), &from, &sources, &to, &dest, move_files, &emit).await;
        let (st, error) = match result {
            Ok(()) => ("done", None),
            Err(e) if format!("{e:#}").contains("cancelled") => ("cancelled", None),
            Err(e) => ("failed", Some(format!("{e:#}"))),
        };
        let _ = app.emit(
            "transfer",
            files::Progress { id, current: String::new(), files_done: 0, bytes_done: 0, bytes_total: 0, state: st, error },
        );
    });
}

#[tauri::command]
fn fs_cancel(state: State<'_, AppState>, id: String) {
    state.core.cancel_transfer(&id);
}

#[tauri::command]
fn running_agents() -> Vec<agents::Agent> {
    agents::running()
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

/// Opens a VM's screen, or the machine's own desktop when `vm_id` is missing.
#[tauri::command]
async fn vnc_open(state: State<'_, AppState>, machine_id: String, vm_id: Option<String>) -> CmdResult<VncTarget> {
    let (conn, channel, title) = match vm_id {
        Some(vm) => state.core.vnc_channel(&machine_id, &vm).await?,
        None => state.core.machine_vnc_channel(&machine_id).await?,
    };
    let url = state.bridge.register(conn, channel, vnc::Kind::Raw).await?;
    Ok(VncTarget { url, title })
}

/// Opens a shell with a terminal on the machine or in a guest. The WebSocket carries the
/// terminal's bytes as binary messages; a text message `{"cols":80,"rows":24}` resizes it.
#[tauri::command]
async fn term_open(state: State<'_, AppState>, machine_id: String, vm_id: Option<String>, cols: u32, rows: u32) -> CmdResult<VncTarget> {
    let (conn, channel, title) = state.core.shell_channel(&machine_id, vm_id.as_deref(), cols, rows).await?;
    let url = state.bridge.register(conn, channel, vnc::Kind::Terminal).await?;
    Ok(VncTarget { url, title })
}

/// Folders on this PC worth a shortcut in the file browser: home, desktop, downloads, documents.
#[tauri::command]
fn local_places() -> Vec<(String, String)> {
    let mut v = Vec::new();
    for (name, dir) in [
        ("home", dirs::home_dir()),
        ("desktop", dirs::desktop_dir()),
        ("downloads", dirs::download_dir()),
        ("documents", dirs::document_dir()),
    ] {
        if let Some(d) = dir.filter(|d| d.is_dir()) {
            v.push((name.to_string(), d.display().to_string()));
        }
    }
    v
}

/// `devbench mcp`: serve AI agents over MCP on stdin/stdout instead of opening the window.
pub fn run_mcp() -> anyhow::Result<()> {
    mcp::run()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (core, load_error) = Core::load();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
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
            term_open,
            local_places,
            activity,
            running_agents,
            fs_list,
            fs_mkdir,
            fs_rename,
            fs_delete,
            fs_copy,
            fs_cancel,
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
