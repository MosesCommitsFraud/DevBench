//! DevBench: a control center for home servers and their virtual machines.

mod activity;
mod agents;
mod config;
mod core;
mod error;
mod files;
mod forward;
mod guest;
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
    core: Arc<Core>,
    /// Ports of machines and guests forwarded to this PC.
    forwards: forward::Forwards,
    bridge: Arc<vnc::Bridge>,
    /// A config file that could not be read at start; shown in the UI instead of failing silently.
    startup_error: Option<String>,
    /// Open VNC connections for the dashboard's screen previews, per "machine/vm".
    previews: tokio::sync::Mutex<std::collections::HashMap<String, Arc<vncclient::Screen>>>,
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
async fn fs_drives(state: State<'_, AppState>, loc: files::Location) -> CmdResult<Vec<files::Drive>> {
    Ok(state.core.drives(&loc).await?)
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

/// A small picture of a VM's screen (or of the machine's desktop when `vm_id` is missing) as a
/// PNG data URL, for the dashboard. The VNC connection stays open for the next call.
#[tauri::command]
async fn screen_preview(state: State<'_, AppState>, machine_id: String, vm_id: Option<String>) -> CmdResult<String> {
    use base64::Engine;
    let key = format!("{machine_id}/{}", vm_id.as_deref().unwrap_or(""));
    let cached = state.previews.lock().await.get(&key).filter(|s| s.is_alive()).cloned();
    let screen = match cached {
        Some(s) => s,
        None => {
            let (conn, ch, _) = match &vm_id {
                Some(vm) => state.core.vnc_channel(&machine_id, vm).await?,
                None => state.core.machine_vnc_channel(&machine_id).await?,
            };
            let password = match &vm_id {
                Some(vm) => state.core.config().machine(&machine_id)?.vm(vm)?.vnc_password.clone(),
                None => String::new(),
            };
            let s = Arc::new(vncclient::Screen::connect(conn, ch, password).await?);
            state.previews.lock().await.insert(key.clone(), s.clone());
            s
        }
    };
    match screen.screenshot(640).await {
        Ok(shot) => Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(shot.png))),
        Err(e) => {
            state.previews.lock().await.remove(&key);
            Err(e.into())
        }
    }
}

/// Closes the preview connections, e.g. when the window is hidden or a full screen view opens.
#[tauri::command]
async fn screen_preview_stop(state: State<'_, AppState>) -> CmdResult<()> {
    state.previews.lock().await.clear();
    Ok(())
}

/// Opens a shell with a terminal on the machine or in a guest. The WebSocket carries the
/// terminal's bytes as binary messages; a text message `{"cols":80,"rows":24}` resizes it.
#[tauri::command]
async fn term_open(state: State<'_, AppState>, machine_id: String, vm_id: Option<String>, cols: u32, rows: u32) -> CmdResult<VncTarget> {
    let (conn, channel, title) = state.core.shell_channel(&machine_id, vm_id.as_deref(), cols, rows).await?;
    let url = state.bridge.register(conn, channel, vnc::Kind::Terminal).await?;
    Ok(VncTarget { url, title })
}

#[tauri::command]
fn port_forwards(state: State<'_, AppState>) -> Vec<forward::ForwardInfo> {
    state.forwards.list()
}

/// Forwards a port of a machine (or of a guest, with `vm_id`) to 127.0.0.1 on this PC.
#[tauri::command]
async fn port_forward_open(
    state: State<'_, AppState>,
    machine_id: String,
    vm_id: Option<String>,
    host: Option<String>,
    port: u16,
    local_port: Option<u16>,
) -> CmdResult<forward::ForwardInfo> {
    state.core.config().machine(&machine_id)?;
    Ok(state
        .forwards
        .open(state.core.clone(), &machine_id, vm_id.as_deref(), host.as_deref(), port, local_port)
        .await?)
}

#[tauri::command]
fn port_forward_close(state: State<'_, AppState>, id: String) {
    state.forwards.close(&id);
}

/// What a running guest reports about itself: system, memory, disk, GPU.
#[tauri::command]
async fn guest_stats(state: State<'_, AppState>, machine_id: String, vm_id: String) -> CmdResult<guest::GuestStats> {
    Ok(state.core.guest_stats(&machine_id, &vm_id).await?)
}

/// Ports something listens on, as suggestions to forward.
#[tauri::command]
async fn listening_ports(state: State<'_, AppState>, machine_id: String, vm_id: Option<String>) -> CmdResult<Vec<forward::ListenPort>> {
    Ok(state.core.listening_ports(&machine_id, vm_id.as_deref()).await?)
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
            // Updates come from the GitHub releases (latest.json, signed), see docs/releasing.md.
            #[cfg(desktop)]
            {
                app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
                app.handle().plugin(tauri_plugin_process::init())?;
            }
            Ok(())
        })
        .manage(AppState { core: Arc::new(core), forwards: Default::default(), bridge: Default::default(), startup_error: load_error, previews: Default::default() })
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
            screen_preview,
            screen_preview_stop,
            local_places,
            activity,
            running_agents,
            fs_list,
            fs_drives,
            port_forwards,
            port_forward_open,
            port_forward_close,
            listening_ports,
            guest_stats,
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

        // Terminal through the WebSocket bridge: resize, run a command, read its output.
        {
            use futures_util::{SinkExt, StreamExt};
            use tokio_tungstenite::tungstenite::Message;
            let conn = pool.get(&ep).await.unwrap();
            let ch = conn.handle.channel_open_session().await.unwrap();
            ch.request_pty(false, "xterm-256color", 80, 24, 0, 0, &[]).await.unwrap();
            ch.request_shell(false).await.unwrap();
            let bridge = std::sync::Arc::new(vnc::Bridge::default());
            let url = bridge.register(conn, ch, vnc::Kind::Terminal).await.unwrap();
            let (mut ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
            ws.send(Message::Text(r#"{"cols":120,"rows":40}"#.into())).await.unwrap();
            ws.send(Message::Binary(b"echo devbench-$((40+2)); stty size; exit\n".to_vec().into())).await.unwrap();
            let mut out = Vec::new();
            let read = async {
                while let Some(Ok(msg)) = ws.next().await {
                    if let Message::Binary(b) = msg {
                        out.extend_from_slice(&b);
                    }
                }
            };
            tokio::time::timeout(std::time::Duration::from_secs(10), read).await.ok();
            let text = String::from_utf8_lossy(&out);
            println!("terminal output tail: {:?}", &text[text.len().saturating_sub(200)..]);
            assert!(text.contains("devbench-42"));
            assert!(text.contains("40 120"));
        }

        // The machine's own desktop: DEVBENCH_TEST_DESKTOP_PORT=5901, optionally
        // DEVBENCH_TEST_SHOT=<file.png> to save what it shows.
        if let Some(port) = std::env::var("DEVBENCH_TEST_DESKTOP_PORT").ok().and_then(|p| p.parse::<u16>().ok()) {
            let (conn, ch) = pool.forward(&ep, "127.0.0.1", port).await.map_err(error::AppError::from).unwrap();
            let screen = vncclient::Screen::connect(conn, ch, String::new()).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let shot = screen.screenshot(1600).await.unwrap();
            println!("desktop: {}x{}", shot.screen_width, shot.screen_height);
            if let Ok(path) = std::env::var("DEVBENCH_TEST_SHOT") {
                std::fs::write(path, &shot.png).unwrap();
            }
        }

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

/// Drives, ports, guest stats and a real port forward, against the machines in the user's own
/// config. Run with:
/// `DEVBENCH_TEST_MACHINE=<machine id> DEVBENCH_TEST_VM_ID=<vm id> DEVBENCH_TEST_FORWARD_PORT=<port listening on the machine> cargo test live_dev -- --ignored --nocapture`
#[cfg(test)]
mod live_dev_tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn live_dev_tools() {
        let machine = std::env::var("DEVBENCH_TEST_MACHINE").expect("DEVBENCH_TEST_MACHINE");
        let vm = std::env::var("DEVBENCH_TEST_VM_ID").ok();
        let (core, err) = Core::load();
        assert!(err.is_none(), "{err:?}");
        let core = Arc::new(core);
        let loc = files::Location::Remote { machine: machine.clone(), vm: None };
        println!("machine drives: {:#?}", core.drives(&loc).await.unwrap());
        println!("machine ports: {:#?}", core.listening_ports(&machine, None).await.unwrap());

        if let Some(vm) = &vm {
            let t = std::time::Instant::now();
            println!("guest stats: {:#?} in {:?}", core.guest_stats(&machine, vm).await.unwrap(), t.elapsed());
            let t = std::time::Instant::now();
            core.guest_stats(&machine, vm).await.unwrap();
            println!("guest stats again (OS known): {:?}", t.elapsed());
            let gloc = files::Location::Remote { machine: machine.clone(), vm: Some(vm.clone()) };
            println!("guest drives: {:#?}", core.drives(&gloc).await.unwrap());
            println!("guest ports: {:#?}", core.listening_ports(&machine, Some(vm)).await.unwrap());
        }

        if let Some(port) = std::env::var("DEVBENCH_TEST_FORWARD_PORT").ok().and_then(|p| p.parse::<u16>().ok()) {
            let fw = forward::Forwards::default();
            let info = fw.open(core.clone(), &machine, None, None, port, None).await.unwrap();
            println!("forward: {info:?}");
            let mut sock = tokio::net::TcpStream::connect(("127.0.0.1", info.local_port)).await.unwrap();
            let mut buf = [0u8; 12];
            tokio::time::timeout(std::time::Duration::from_secs(10), sock.read_exact(&mut buf)).await.unwrap().unwrap();
            println!("first bytes through the forward: {:?}", String::from_utf8_lossy(&buf));
            assert_eq!(fw.list()[0].connections, 1);
            assert!(fw.list()[0].error.is_none());
            drop(sock);
            fw.close(&info.id);
            assert!(fw.list().is_empty());
        }
    }
}
