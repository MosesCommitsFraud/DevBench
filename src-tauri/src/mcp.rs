//! `devbench mcp`: an MCP server on stdio, so Claude and other AI agents can use DevBench
//! directly: machine status, Wake-on-LAN, VM control, shell commands, files, and a VM's
//! screen with mouse and keyboard. Nothing is allowed until the user switches it on per
//! machine and VM in Settings, and every call is written to the activity log.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::service::{NotificationContext, RoleServer};
use rmcp::{ErrorData, ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::activity;
use crate::agents::{self, Agent};
use crate::config::{Machine, Vm};
use crate::core::{Core, PowerAction, VmAction};
use crate::files::Location;
use crate::vncclient::Screen;

const MAX_OUTPUT: usize = 100 * 1024;
const MAX_FILE: u64 = 2 * 1024 * 1024;

#[derive(Clone)]
pub struct DevBenchMcp {
    core: Arc<Core>,
    screens: Arc<Mutex<HashMap<String, Arc<ScreenState>>>>,
    agent: Arc<std::sync::Mutex<Agent>>,
    tool_router: ToolRouter<Self>,
}

struct ScreenState {
    screen: Screen,
    /// Factor from screenshot pixels to screen pixels (1.0 until a scaled screenshot was taken).
    scale: std::sync::Mutex<f64>,
}

#[derive(Deserialize, JsonSchema)]
struct MachineArg {
    /// Machine id or name, as list_machines shows it.
    machine: String,
}

#[derive(Deserialize, JsonSchema)]
struct PowerArg {
    machine: String,
    /// "poweroff" or "reboot".
    action: String,
}

#[derive(Deserialize, JsonSchema)]
struct VmActionArg {
    machine: String,
    /// VM id or name.
    vm: String,
    /// "start", "shutdown" (asks the guest to shut down), "reboot" or "destroy" (force off).
    action: String,
}

#[derive(Deserialize, JsonSchema)]
struct ExecArg {
    machine: String,
    /// Run inside this VM's guest instead of on the machine. Windows guests run the command
    /// with their default SSH shell (usually cmd.exe; prefix with `powershell -Command` for PowerShell).
    vm: Option<String>,
    command: String,
    /// Seconds before the command is abandoned (default 60, at most 600).
    timeout_secs: Option<u64>,
}

#[derive(Deserialize, JsonSchema)]
struct PathArg {
    machine: String,
    vm: Option<String>,
    /// Path on the machine or guest. Windows guests use SFTP paths like /C:/Users/name/file.txt.
    path: String,
}

#[derive(Deserialize, JsonSchema)]
struct WriteArg {
    machine: String,
    vm: Option<String>,
    path: String,
    /// The new file content (replaces the file).
    content: String,
    /// Set when `content` is base64 for binary data.
    base64: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct ScreenArg {
    machine: String,
    vm: String,
    /// Longest side of the returned image in pixels (default 1280).
    max_size: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
struct PointArg {
    machine: String,
    vm: String,
    /// X in the pixels of the last vm_screenshot image.
    x: f64,
    /// Y in the pixels of the last vm_screenshot image.
    y: f64,
    /// "left" (default), "right" or "middle".
    button: Option<String>,
    /// Click twice.
    double: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
struct ScrollArg {
    machine: String,
    vm: String,
    x: f64,
    y: f64,
    /// Wheel steps; positive scrolls down, negative up.
    dy: i32,
}

#[derive(Deserialize, JsonSchema)]
struct TypeArg {
    machine: String,
    vm: String,
    /// Text to type as keystrokes. "\n" presses Enter.
    text: String,
}

#[derive(Deserialize, JsonSchema)]
struct KeyArg {
    machine: String,
    vm: String,
    /// One key or a combination joined with "+", e.g. "enter", "ctrl+c", "win+r", "ctrl+alt+delete", "f5".
    keys: String,
}

fn text(s: impl Into<String>) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(s)])
}

fn failure(e: anyhow::Error) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(format!("{e:#}"))])
}

fn truncate(s: &str) -> String {
    if s.len() <= MAX_OUTPUT {
        return s.to_string();
    }
    let mut end = MAX_OUTPUT;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[... {} more bytes cut]", &s[..end], s.len() - end)
}

impl DevBenchMcp {
    pub fn new(core: Arc<Core>, agent: Agent) -> Self {
        Self {
            core,
            screens: Default::default(),
            agent: Arc::new(std::sync::Mutex::new(agent)),
            tool_router: Self::tool_router(),
        }
    }

    /// The machine, if agents may see it.
    fn machine(&self, key: &str) -> Result<Machine> {
        self.core.refresh_config()?;
        let cfg = self.core.config();
        let m = cfg.machine(key)?;
        if !m.agent_access {
            bail!("agents are not allowed to use {} (switch it on in DevBench → Settings → AI agents)", m.name);
        }
        Ok(m.clone())
    }

    fn vm(&self, machine: &str, vm: &str) -> Result<(Machine, Vm)> {
        let m = self.machine(machine)?;
        let v = m.vm(vm)?.clone();
        if !v.agent_access {
            bail!("agents are not allowed to use the VM {} (DevBench → Settings → AI agents)", v.name);
        }
        Ok((m, v))
    }

    /// Checks the permission for commands and files on a machine or in a guest.
    fn exec_target(&self, machine: &str, vm: Option<&str>) -> Result<(Machine, Option<Vm>)> {
        match vm {
            None => {
                let m = self.machine(machine)?;
                if !m.agent_exec {
                    bail!("agents may not run commands or edit files on {} (DevBench → Settings → AI agents)", m.name);
                }
                Ok((m, None))
            }
            Some(v) => {
                let (m, vm) = self.vm(machine, v)?;
                if !vm.agent_exec {
                    bail!("agents may not run commands or edit files in {} (DevBench → Settings → AI agents)", vm.name);
                }
                Ok((m, Some(vm)))
            }
        }
    }

    /// Writes the activity log and the agent's heartbeat file.
    fn log(&self, tool: &str, machine: &str, vm: &str, summary: &str, result: &Result<CallToolResult>) {
        let (ok, error) = match result {
            Ok(r) if r.is_error != Some(true) => (true, String::new()),
            Ok(r) => (false, r.content.iter().filter_map(|c| c.as_text().map(|t| t.text.clone())).collect::<Vec<_>>().join(" ")),
            Err(e) => (false, format!("{e:#}")),
        };
        let _ = activity::append(&activity::Entry {
            time: activity::now(),
            tool: tool.to_string(),
            machine: machine.to_string(),
            vm: vm.to_string(),
            summary: summary.chars().take(300).collect(),
            ok,
            error: error.chars().take(500).collect(),
        });
        let mut a = self.agent.lock().unwrap();
        a.last_tool = tool.to_string();
        a.last_target = [machine, vm].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" / ");
        a.calls += 1;
        agents::write(&mut a);
    }

    fn finish(&self, tool: &str, machine: &str, vm: &str, summary: &str, result: Result<CallToolResult>) -> Result<CallToolResult, ErrorData> {
        self.log(tool, machine, vm, summary, &result);
        Ok(result.unwrap_or_else(failure))
    }

    async fn screen(&self, machine: &Machine, vm: &Vm) -> Result<Arc<ScreenState>> {
        let key = format!("{}/{}", machine.id, vm.id);
        let mut screens = self.screens.lock().await;
        if let Some(s) = screens.get(&key) {
            if s.screen.is_alive() {
                return Ok(s.clone());
            }
        }
        let (conn, ch, _) = self.core.vnc_channel(&machine.id, &vm.id).await?;
        let screen = Screen::connect(conn, ch, vm.vnc_password.clone()).await?;
        let s = Arc::new(ScreenState { screen, scale: std::sync::Mutex::new(1.0) });
        screens.insert(key, s.clone());
        Ok(s)
    }

    async fn point(&self, a: &PointArg) -> Result<(Arc<ScreenState>, f64, f64)> {
        let (m, v) = self.vm(&a.machine, &a.vm)?;
        let s = self.screen(&m, &v).await?;
        let k = *s.scale.lock().unwrap();
        Ok((s, a.x * k, a.y * k))
    }
}

#[tool_router]
impl DevBenchMcp {
    /// Lists the machines (and their VMs) that agents may use, with what they are allowed to do.
    #[tool]
    async fn list_machines(&self) -> Result<CallToolResult, ErrorData> {
        let r = (|| -> Result<CallToolResult> {
            self.core.refresh_config()?;
            let cfg = self.core.config();
            let list: Vec<serde_json::Value> = cfg
                .machines
                .iter()
                .filter(|m| m.agent_access)
                .map(|m| {
                    serde_json::json!({
                        "id": m.id, "name": m.name, "ssh_host": m.ssh_host,
                        "can_wake": !m.mac.is_empty(), "can_exec": m.agent_exec, "can_power": m.agent_power,
                        "vms": m.vms.iter().filter(|v| v.agent_access).map(|v| serde_json::json!({
                            "id": v.id, "name": v.name, "libvirt_name": v.libvirt_name,
                            "has_screen": v.vnc_mode != crate::config::VncMode::None, "can_exec": v.agent_exec,
                        })).collect::<Vec<_>>(),
                    })
                })
                .collect();
            if list.is_empty() {
                return Ok(text(
                    "No machine is open to agents. The user can allow it in DevBench → Settings → AI agents.",
                ));
            }
            Ok(text(serde_json::to_string_pretty(&list)?))
        })();
        self.finish("list_machines", "", "", "listed machines", r)
    }

    /// Status of a machine: online, uptime, CPU, memory, disk, and the state of each VM.
    #[tool]
    async fn machine_status(&self, Parameters(a): Parameters<MachineArg>) -> Result<CallToolResult, ErrorData> {
        let r = async {
            let m = self.machine(&a.machine)?;
            match self.core.status(&m.id).await {
                Ok(st) => {
                    let vms: Vec<_> = m
                        .vms
                        .iter()
                        .filter(|v| v.agent_access)
                        .map(|v| {
                            let s = st.vms.iter().find(|x| x.id == v.id);
                            serde_json::json!({ "vm": v.name, "state": s.map(|x| x.state.clone()).unwrap_or_default() })
                        })
                        .collect();
                    Ok(text(serde_json::to_string_pretty(&serde_json::json!({
                        "online": true, "hostname": st.hostname, "uptime_secs": st.uptime_secs,
                        "cpu_percent": (st.cpu_percent * 10.0).round() / 10.0, "load": st.load, "threads": st.cpus,
                        "mem_used_bytes": st.mem_total - st.mem_available, "mem_total_bytes": st.mem_total,
                        "disk_used_bytes": st.disk_used, "disk_total_bytes": st.disk_total, "vms": vms,
                    }))?))
                }
                Err(e) => Ok(text(format!("{{\"online\": false, \"error\": {:?}}}", format!("{e:#}")))),
            }
        }
        .await;
        self.finish("machine_status", &a.machine, "", "read status", r)
    }

    /// Sends a Wake-on-LAN packet to a machine that is off. It takes a minute or two to boot.
    #[tool]
    async fn wake_machine(&self, Parameters(a): Parameters<MachineArg>) -> Result<CallToolResult, ErrorData> {
        let r = async {
            let m = self.machine(&a.machine)?;
            self.core.wake(&m.id).await?;
            Ok(text(format!("Wake-on-LAN packet sent to {}. Check machine_status in a minute.", m.name)))
        }
        .await;
        self.finish("wake_machine", &a.machine, "", "Wake-on-LAN", r)
    }

    /// Powers a machine off or reboots it. All its VMs stop with it.
    #[tool]
    async fn power_machine(&self, Parameters(a): Parameters<PowerArg>) -> Result<CallToolResult, ErrorData> {
        let r = async {
            let m = self.machine(&a.machine)?;
            if !m.agent_power {
                bail!("agents may not power off or reboot {} (DevBench → Settings → AI agents)", m.name);
            }
            self.core.power(&m.id, PowerAction::parse(&a.action)?).await?;
            Ok(text(format!("{} {}", m.name, if a.action == "reboot" { "is rebooting" } else { "is shutting down" })))
        }
        .await;
        self.finish("power_machine", &a.machine, "", &a.action, r)
    }

    /// Starts, shuts down, reboots or force-stops ("destroy") a VM.
    #[tool]
    async fn vm_action(&self, Parameters(a): Parameters<VmActionArg>) -> Result<CallToolResult, ErrorData> {
        let r = async {
            let (m, v) = self.vm(&a.machine, &a.vm)?;
            let action = VmAction::parse(&a.action)?;
            if action == VmAction::Destroy && !m.agent_power {
                bail!("agents may not force VMs off on {} (DevBench → Settings → AI agents)", m.name);
            }
            self.core.vm_action(&m.id, &v.id, action).await?;
            Ok(text(format!("{}: {} done", v.name, a.action)))
        }
        .await;
        self.finish("vm_action", &a.machine, &a.vm, &a.action, r)
    }

    /// Runs a shell command over SSH on a machine, or inside a VM's guest, and returns its output and exit code.
    #[tool]
    async fn exec(&self, Parameters(a): Parameters<ExecArg>) -> Result<CallToolResult, ErrorData> {
        let r = async {
            let (m, v) = self.exec_target(&a.machine, a.vm.as_deref())?;
            let timeout = Duration::from_secs(a.timeout_secs.unwrap_or(60).clamp(1, 600));
            let out = self.core.exec(&m.id, v.as_ref().map(|v| v.id.as_str()), &a.command, timeout).await?;
            let mut s = format!("exit code: {}\n", out.code.map(|c| c.to_string()).unwrap_or_else(|| "none".into()));
            if !out.stdout.is_empty() {
                s.push_str(&format!("--- stdout ---\n{}\n", truncate(&out.stdout)));
            }
            if !out.stderr.is_empty() {
                s.push_str(&format!("--- stderr ---\n{}\n", truncate(&out.stderr)));
            }
            Ok(if out.code == Some(0) { text(s) } else { CallToolResult::error(vec![ContentBlock::text(s)]) })
        }
        .await;
        self.finish("exec", &a.machine, a.vm.as_deref().unwrap_or(""), &a.command, r)
    }

    /// Lists a folder on a machine or in a guest (over SFTP).
    #[tool]
    async fn list_dir(&self, Parameters(a): Parameters<PathArg>) -> Result<CallToolResult, ErrorData> {
        let r = async {
            let (m, v) = self.exec_target(&a.machine, a.vm.as_deref())?;
            let fs = self.core.fs(&Location::Remote { machine: m.id, vm: v.map(|v| v.id) }).await?;
            let path = if a.path.trim().is_empty() { fs.home().await? } else { a.path.clone() };
            let l = fs.list(&path).await?;
            let mut s = format!("{}\n", l.path);
            for e in l.entries {
                s.push_str(&if e.is_dir { format!("{}/\n", e.name) } else { format!("{}  ({} bytes)\n", e.name, e.size) });
            }
            Ok(text(truncate(&s)))
        }
        .await;
        self.finish("list_dir", &a.machine, a.vm.as_deref().unwrap_or(""), &a.path, r)
    }

    /// Reads a file (up to 2 MB) from a machine or a guest. Binary files come back as base64.
    #[tool]
    async fn read_file(&self, Parameters(a): Parameters<PathArg>) -> Result<CallToolResult, ErrorData> {
        let r = async {
            let (m, v) = self.exec_target(&a.machine, a.vm.as_deref())?;
            let fs = self.core.fs(&Location::Remote { machine: m.id, vm: v.map(|v| v.id) }).await?;
            let data = fs.read_all(&a.path, MAX_FILE).await?;
            Ok(match String::from_utf8(data) {
                Ok(s) => text(s),
                Err(e) => {
                    use base64::Engine;
                    text(format!("base64:{}", base64::engine::general_purpose::STANDARD.encode(e.into_bytes())))
                }
            })
        }
        .await;
        self.finish("read_file", &a.machine, a.vm.as_deref().unwrap_or(""), &a.path, r)
    }

    /// Writes (creates or replaces) a file on a machine or in a guest.
    #[tool]
    async fn write_file(&self, Parameters(a): Parameters<WriteArg>) -> Result<CallToolResult, ErrorData> {
        let r = async {
            let (m, v) = self.exec_target(&a.machine, a.vm.as_deref())?;
            let fs = self.core.fs(&Location::Remote { machine: m.id, vm: v.map(|v| v.id) }).await?;
            let data = if a.base64.unwrap_or(false) {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD.decode(a.content.trim()).map_err(|e| anyhow!("base64: {e}"))?
            } else {
                a.content.clone().into_bytes()
            };
            fs.write_all(&a.path, &data).await?;
            Ok(text(format!("wrote {} bytes to {}", data.len(), a.path)))
        }
        .await;
        self.finish("write_file", &a.machine, a.vm.as_deref().unwrap_or(""), &a.path, r)
    }

    /// Takes a screenshot of a VM's screen (PNG). Mouse coordinates for vm_click, vm_move and vm_scroll use this image's pixels.
    #[tool]
    async fn vm_screenshot(&self, Parameters(a): Parameters<ScreenArg>) -> Result<CallToolResult, ErrorData> {
        let r = async {
            let (m, v) = self.vm(&a.machine, &a.vm)?;
            let s = self.screen(&m, &v).await?;
            let shot = s.screen.screenshot(a.max_size.unwrap_or(1280) as usize).await?;
            *s.scale.lock().unwrap() = shot.screen_width as f64 / shot.width as f64;
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&shot.png);
            Ok(CallToolResult::success(vec![
                ContentBlock::image(b64, "image/png"),
                ContentBlock::text(format!(
                    "Image {}x{} (screen {}x{}). Use image pixel coordinates for clicks.",
                    shot.width, shot.height, shot.screen_width, shot.screen_height
                )),
            ]))
        }
        .await;
        self.finish("vm_screenshot", &a.machine, &a.vm, "screenshot", r)
    }

    /// Clicks in a VM at a point of the last screenshot.
    #[tool]
    async fn vm_click(&self, Parameters(a): Parameters<PointArg>) -> Result<CallToolResult, ErrorData> {
        let r = async {
            let (s, x, y) = self.point(&a).await?;
            let button = match a.button.as_deref() {
                Some("right") => 3,
                Some("middle") => 2,
                _ => 1,
            };
            s.screen.click(x, y, button, if a.double.unwrap_or(false) { 2 } else { 1 }).await?;
            Ok(text("clicked"))
        }
        .await;
        let summary = format!("{} click at {:.0},{:.0}", a.button.as_deref().unwrap_or("left"), a.x, a.y);
        self.finish("vm_click", &a.machine, &a.vm, &summary, r)
    }

    /// Moves the mouse in a VM to a point of the last screenshot (for hover menus).
    #[tool]
    async fn vm_move(&self, Parameters(a): Parameters<PointArg>) -> Result<CallToolResult, ErrorData> {
        let r = async {
            let (s, x, y) = self.point(&a).await?;
            s.screen.move_to(x, y).await?;
            Ok(text("moved"))
        }
        .await;
        self.finish("vm_move", &a.machine, &a.vm, &format!("move to {:.0},{:.0}", a.x, a.y), r)
    }

    /// Scrolls the mouse wheel in a VM at a point of the last screenshot.
    #[tool]
    async fn vm_scroll(&self, Parameters(a): Parameters<ScrollArg>) -> Result<CallToolResult, ErrorData> {
        let r = async {
            let (m, v) = self.vm(&a.machine, &a.vm)?;
            let s = self.screen(&m, &v).await?;
            let k = *s.scale.lock().unwrap();
            s.screen.scroll(a.x * k, a.y * k, a.dy).await?;
            Ok(text("scrolled"))
        }
        .await;
        self.finish("vm_scroll", &a.machine, &a.vm, &format!("scroll {}", a.dy), r)
    }

    /// Types text in a VM as keystrokes, into whatever has the keyboard focus.
    #[tool]
    async fn vm_type(&self, Parameters(a): Parameters<TypeArg>) -> Result<CallToolResult, ErrorData> {
        let r = async {
            let (m, v) = self.vm(&a.machine, &a.vm)?;
            let s = self.screen(&m, &v).await?;
            s.screen.type_text(&a.text).await?;
            Ok(text(format!("typed {} characters", a.text.chars().count())))
        }
        .await;
        let summary = format!("typed {} characters", a.text.chars().count());
        self.finish("vm_type", &a.machine, &a.vm, &summary, r)
    }

    /// Presses a key or key combination in a VM, e.g. "enter", "ctrl+s", "win+r", "alt+f4", "ctrl+alt+delete".
    #[tool]
    async fn vm_key(&self, Parameters(a): Parameters<KeyArg>) -> Result<CallToolResult, ErrorData> {
        let r = async {
            let (m, v) = self.vm(&a.machine, &a.vm)?;
            let s = self.screen(&m, &v).await?;
            s.screen.press(&a.keys).await?;
            Ok(text(format!("pressed {}", a.keys)))
        }
        .await;
        self.finish("vm_key", &a.machine, &a.vm, &a.keys, r)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for DevBenchMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("devbench", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "DevBench controls the user's home servers and their virtual machines. Start with list_machines. \
                 Use exec for shell work on a machine or in a guest, read_file/write_file/list_dir for files, and \
                 vm_screenshot plus vm_click/vm_type/vm_key to work in a VM's desktop: take a screenshot, act, then \
                 take another to check. Only machines and VMs the user opened to agents are visible.",
            )
    }

    async fn on_initialized(&self, context: NotificationContext<RoleServer>) {
        if let Some(info) = context.peer.peer_info() {
            let mut a = self.agent.lock().unwrap();
            a.client = info.client_info.name.clone();
            agents::write(&mut a);
        }
    }
}

/// Runs the MCP server on stdin/stdout until the client disconnects.
pub fn run() -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(async {
        let (core, err) = Core::load();
        if let Some(e) = err {
            eprintln!("devbench: config file: {e}");
        }
        let pid = std::process::id();
        let mut agent = Agent {
            pid,
            client: "agent".into(),
            started: activity::now(),
            seen: 0,
            last_tool: String::new(),
            last_target: String::new(),
            calls: 0,
        };
        agents::write(&mut agent);
        let server = DevBenchMcp::new(Arc::new(core), agent);
        let heartbeat = server.agent.clone();
        let beat = tokio::spawn(async move {
            loop {
                tokio::time::sleep(agents::HEARTBEAT).await;
                let mut a = heartbeat.lock().unwrap();
                agents::write(&mut a);
            }
        });
        let result = async {
            let service = server.serve(rmcp::transport::stdio()).await.map_err(|e| anyhow!("MCP: {e}"))?;
            service.waiting().await.map_err(|e| anyhow!("MCP: {e}"))?;
            anyhow::Ok(())
        }
        .await;
        beat.abort();
        agents::remove(pid);
        result
    })
}
