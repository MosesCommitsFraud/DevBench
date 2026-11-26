//! Everything DevBench can do, independent of who asks: the GUI's commands and the MCP server
//! for AI agents both call into `Core`.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use russh::Channel;
use russh::client::Msg;

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use russh_sftp::client::SftpSession;

use crate::config::{self, Config, Machine, Vm, VncMode};
use crate::files::{Fs, Location, Progress, Transfer};
use crate::ssh::{self, Conn, ExecOutput, sh_quote};
use crate::sshconf::{self, Endpoint};
use crate::status::{self, MachineStatus};
use crate::wol;

pub struct Core {
    config: RwLock<Config>,
    pool: ssh::Pool,
    /// Last CPU tick sample per machine, so usage covers the whole interval between polls.
    cpu: std::sync::Mutex<HashMap<String, (u64, u64)>>,
    /// Guest addresses from libvirt, reused for a minute.
    guest_addrs: std::sync::Mutex<HashMap<String, (String, Instant)>>,
    /// Open SFTP sessions per endpoint, with the connection they run on.
    sftp: tokio::sync::Mutex<HashMap<String, (Arc<Conn>, Arc<SftpSession>)>>,
    /// Cancel flags of running file transfers.
    transfers: std::sync::Mutex<HashMap<String, Arc<AtomicBool>>>,
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
        (Self { config: RwLock::new(config), pool: ssh::Pool::new(known_hosts), cpu: Default::default(), guest_addrs: Default::default(), sftp: Default::default(), transfers: Default::default() }, err)
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
        let key = format!("{}/{}", m.id, vm.id);
        let cached = self
            .guest_addrs
            .lock()
            .unwrap()
            .get(&key)
            .filter(|(_, t)| t.elapsed().as_secs() < 60)
            .map(|(a, _)| a.clone());
        let addr = match cached {
            Some(a) => a,
            None => {
                let a = status::guest_address(&self.pool, ep, vm).await?;
                self.guest_addrs.lock().unwrap().insert(key, (a.clone(), Instant::now()));
                a
            }
        };
        Ok(sshconf::guest_endpoint(ep, vm, &addr))
    }

    /// The endpoint commands and files go to: the machine, or a guest when `vm` is given.
    pub async fn target(&self, machine: &str, vm: Option<&str>) -> Result<Endpoint> {
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

    /// Files on this PC or, over SFTP, on a machine or guest.
    pub async fn fs(&self, loc: &Location) -> Result<Fs> {
        let Location::Remote { machine, vm } = loc else { return Ok(Fs::Local) };
        let ep = self.target(machine, vm.as_deref()).await?;
        let key = ep.cache_key();
        let mut cache = self.sftp.lock().await;
        if let Some((conn, s)) = cache.get(&key) {
            if !conn.handle.is_closed() {
                return Ok(Fs::Remote(s.clone()));
            }
        }
        let conn = self.pool.get(&ep).await?;
        let ch = conn.handle.channel_open_session().await.context("opening an SSH channel")?;
        ch.request_subsystem(true, "sftp").await.context("starting SFTP")?;
        let s = Arc::new(SftpSession::new(ch.into_stream()).await.map_err(|e| anyhow!("SFTP: {e}"))?);
        cache.insert(key, (conn, s.clone()));
        Ok(Fs::Remote(s))
    }

    /// Drops a cached SFTP session after an error, so the next call opens a fresh one.
    pub async fn forget_fs(&self, loc: &Location) {
        if let Location::Remote { machine, vm } = loc {
            if let Ok(ep) = self.target(machine, vm.as_deref()).await {
                self.sftp.lock().await.remove(&ep.cache_key());
            }
        }
    }

    /// Copies (or moves) `sources` from one place into the folder `dest` at another.
    #[allow(clippy::too_many_arguments)]
    pub async fn copy(
        &self,
        id: String,
        from: &Location,
        sources: &[String],
        to: &Location,
        dest: &str,
        move_files: bool,
        report: &(dyn Fn(&Progress) + Send + Sync),
    ) -> Result<()> {
        let src = self.fs(from).await?;
        if move_files && from == to {
            for s in sources {
                src.rename(s, &src.join(dest, &src.name(s))).await?;
            }
            return Ok(());
        }
        let dst = if from == to { None } else { Some(self.fs(to).await?) };
        let cancel = Arc::new(AtomicBool::new(false));
        self.transfers.lock().unwrap().insert(id.clone(), cancel.clone());
        let result = {
            let mut t = Transfer::new(id.clone(), &src, dst.as_ref().unwrap_or(&src), cancel, report);
            t.copy(sources, dest).await
        };
        self.transfers.lock().unwrap().remove(&id);
        result?;
        if move_files {
            for s in sources {
                src.remove(s).await?;
            }
        }
        Ok(())
    }

    pub fn cancel_transfer(&self, id: &str) {
        if let Some(c) = self.transfers.lock().unwrap().get(id) {
            c.store(true, std::sync::atomic::Ordering::Relaxed);
        }
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
