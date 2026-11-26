//! Everything DevBench can do, independent of who asks: the GUI's commands and the MCP server
//! for AI agents both call into `Core`.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use russh::Channel;
use russh::client::Msg;

use crate::config::{self, Config, Machine, Vm, VncMode};
use crate::ssh::{self, Conn, ExecOutput, sh_quote};
use crate::sshconf::{self, Endpoint};
use crate::status::{self, MachineStatus};
use crate::wol;

pub struct Core {
    config: RwLock<Config>,
    pool: ssh::Pool,
    /// Last CPU tick sample per machine, so usage covers the whole interval between polls.
    cpu: std::sync::Mutex<std::collections::HashMap<String, (u64, u64)>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    Poweroff,
    Reboot,
}

impl PowerAction {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "poweroff" | "shutdown" => Ok(Self::Poweroff),
            "reboot" => Ok(Self::Reboot),
            _ => bail!("unknown power action '{s}' (poweroff or reboot)"),
        }
    }
    fn verb(self) -> &'static str {
        match self {
            Self::Poweroff => "poweroff",
            Self::Reboot => "reboot",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmAction {
    Start,
    Shutdown,
    Reboot,
    Destroy,
}

impl VmAction {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "start" => Ok(Self::Start),
            "shutdown" => Ok(Self::Shutdown),
            "reboot" => Ok(Self::Reboot),
            "destroy" | "force-off" => Ok(Self::Destroy),
            _ => bail!("unknown VM action '{s}' (start, shutdown, reboot or destroy)"),
        }
    }
    fn verb(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Shutdown => "shutdown",
            Self::Reboot => "reboot",
            Self::Destroy => "destroy",
        }
    }
}

impl Core {
    /// Loads the config file; a broken file yields an empty config plus the error to show.
    pub fn load() -> (Self, Option<String>) {
        let (config, err) = match config::load() {
            Ok(c) => (c, None),
            Err(e) => (Config::default(), Some(format!("{e:#}"))),
        };
        let known_hosts = config::config_dir().map(|d| d.join("known_hosts")).unwrap_or_else(|_| "known_hosts".into());
        (Self { config: RwLock::new(config), pool: ssh::Pool::new(known_hosts), cpu: Default::default() }, err)
    }

    pub fn config(&self) -> Config {
        self.config.read().unwrap().clone()
    }

    pub fn save_config(&self, mut config: Config) -> Result<Config> {
        config.normalize()?;
        config::save(&config)?;
        *self.config.write().unwrap() = config.clone();
        self.pool.clear();
        Ok(config)
    }

    pub fn reload_config(&self) -> Result<Config> {
        let config = config::load()?;
        *self.config.write().unwrap() = config.clone();
        self.pool.clear();
        Ok(config)
    }

    pub fn trust_host_key(&self, host: &str, port: u16, key: &str) -> Result<()> {
        self.pool.trust(host, port, key)
    }

    fn machine(&self, key: &str) -> Result<(Machine, Endpoint)> {
        let m = self.config.read().unwrap().machine(key)?.clone();
        let ep = sshconf::machine_endpoint(&m)?;
        Ok((m, ep))
    }

    fn machine_vm(&self, machine: &str, vm: &str) -> Result<(Machine, Vm, Endpoint)> {
        let (m, ep) = self.machine(machine)?;
        let v = m.vm(vm)?.clone();
        Ok((m, v, ep))
    }

    pub async fn status(&self, machine: &str) -> Result<MachineStatus> {
        let (m, ep) = self.machine(machine)?;
        let out = self.pool.exec(&ep, &status::script(&m), status::STATUS_TIMEOUT).await?;
        let mut st = status::parse(&out.stdout);
        let mut cpu = self.cpu.lock().unwrap();
        st.apply_cpu(cpu.get(&m.id).copied());
        if let Some(ticks) = st.cpu_ticks {
            cpu.insert(m.id.clone(), ticks);
        }
        Ok(st)
    }

    pub async fn wake(&self, machine: &str) -> Result<()> {
        let (m, _) = self.machine(machine)?;
        if m.mac.trim().is_empty() {
            bail!("{} has no MAC address configured", m.name);
        }
        wol::wake(&m.mac, &m.wol_broadcast, m.wol_port).await
    }

    pub async fn power(&self, machine: &str, action: PowerAction) -> Result<()> {
        let (_, ep) = self.machine(machine)?;
        let verb = action.verb();
        let out = self.pool.exec(&ep, &format!("sudo -n systemctl {verb}"), Duration::from_secs(20)).await?;
        self.pool.evict(&ep);
        match out.code {
            Some(0) | None => Ok(()),
            Some(_) if out.stderr.contains("password") => bail!(
                "sudo asks for a password. Allow 'systemctl {verb}' without one (see docs/sudoers-devbench in the DevBench repository)."
            ),
            Some(c) => bail!("systemctl {verb} failed ({c}): {}", out.stderr.trim()),
        }
    }

    pub async fn vm_action(&self, machine: &str, vm: &str, action: VmAction) -> Result<()> {
        let (_, vm, ep) = self.machine_vm(machine, vm)?;
        let verb = action.verb();
        let cmd = format!("LC_ALL=C virsh -c {} {verb} {}", sh_quote(&vm.libvirt_uri), sh_quote(&vm.libvirt_name));
        let out = self.pool.exec(&ep, &cmd, Duration::from_secs(60)).await?;
        if out.code != Some(0) {
            let msg = if out.stderr.trim().is_empty() { out.stdout.trim() } else { out.stderr.trim() };
            bail!("virsh {verb}: {msg}");
        }
        Ok(())
    }

    pub async fn list_domains(&self, machine: &str, uri: &str) -> Result<Vec<String>> {
        let (_, ep) = self.machine(machine)?;
        let uri = if uri.trim().is_empty() { "qemu:///system" } else { uri };
        let cmd = format!("LC_ALL=C virsh -c {} list --all --name", sh_quote(uri));
        let out = self.pool.exec(&ep, &cmd, Duration::from_secs(20)).await?;
        if out.code != Some(0) {
            bail!("virsh list: {}", out.stderr.trim());
        }
        Ok(out.stdout.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect())
    }

    /// The SSH endpoint of a guest (through its machine).
    async fn guest(&self, m: &Machine, vm: &Vm, ep: &Endpoint) -> Result<Endpoint> {
        if vm.guest_ssh_user.trim().is_empty() {
            bail!("{} / {}: set the guest SSH user in the settings", m.name, vm.name);
        }
        let addr = status::guest_address(&self.pool, ep, vm).await?;
        Ok(sshconf::guest_endpoint(ep, vm, &addr))
    }

    /// The endpoint commands and files go to: the machine, or a guest when `vm` is given.
    async fn target(&self, machine: &str, vm: Option<&str>) -> Result<Endpoint> {
        match vm {
            None => Ok(self.machine(machine)?.1),
            Some(v) => {
                let (m, vm, ep) = self.machine_vm(machine, v)?;
                self.guest(&m, &vm, &ep).await
            }
        }
    }

    pub async fn exec(&self, machine: &str, vm: Option<&str>, command: &str, timeout: Duration) -> Result<ExecOutput> {
        let ep = self.target(machine, vm).await?;
        self.pool.exec(&ep, command, timeout).await
    }

    async fn sftp(&self, machine: &str, vm: Option<&str>) -> Result<russh_sftp::client::SftpSession> {
        let ep = self.target(machine, vm).await?;
        let conn = self.pool.get(&ep).await?;
        let ch = conn.handle.channel_open_session().await.context("opening an SSH channel")?;
        ch.request_subsystem(true, "sftp").await.context("starting SFTP")?;
        russh_sftp::client::SftpSession::new(ch.into_stream()).await.map_err(|e| anyhow!("SFTP: {e}"))
    }

    pub async fn read_file(&self, machine: &str, vm: Option<&str>, path: &str) -> Result<Vec<u8>> {
        let sftp = self.sftp(machine, vm).await?;
        sftp.read(path).await.map_err(|e| anyhow!("reading {path}: {e}"))
    }

    pub async fn write_file(&self, machine: &str, vm: Option<&str>, path: &str, data: &[u8]) -> Result<()> {
        let sftp = self.sftp(machine, vm).await?;
        sftp.write(path, data).await.map_err(|e| anyhow!("writing {path}: {e}"))
    }

    /// A forwarded TCP connection to the VM's VNC server, plus a display title.
    /// Keep the returned `Conn` alive while the channel is in use.
    pub async fn vnc_channel(&self, machine: &str, vm: &str) -> Result<(Arc<Conn>, Channel<Msg>, String)> {
        let (m, vm, ep) = self.machine_vm(machine, vm)?;
        let title = format!("{} · {}", vm.name, m.name);
        let (conn, ch) = match vm.vnc_mode {
            VncMode::None => bail!("VNC is switched off for {}", vm.name),
            VncMode::Host => self.pool.forward(&ep, &vm.vnc_host, vm.vnc_port).await?,
            VncMode::Guest => {
                let guest = self.guest(&m, &vm, &ep).await?;
                self.pool
                    .forward(&guest, &vm.vnc_host, vm.vnc_port)
                    .await
                    .with_context(|| format!("VNC server in {} ({}:{})", vm.name, vm.vnc_host, vm.vnc_port))?
            }
        };
        Ok((conn, ch, title))
    }
}
