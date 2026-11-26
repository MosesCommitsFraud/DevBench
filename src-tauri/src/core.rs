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
use crate::files::{self, Drive, Fs, Location, Progress, Transfer};
use crate::ssh::{self, Conn, ExecOutput, sh_quote};
use crate::sshconf::{self, Endpoint};
use crate::status::{self, MachineStatus};
use crate::wol;

/// (rx, tx) byte counters and when they were read.
type NetSample = ((u64, u64), Instant);
/// CPU time per VM id and when it was read.
type VmCpuSamples = HashMap<String, (u64, Instant)>;

pub struct Core {
    config: RwLock<Config>,
    pool: ssh::Pool,
    /// Last CPU tick sample per machine, so usage covers the whole interval between polls.
    cpu: std::sync::Mutex<HashMap<String, (u64, u64)>>,
    /// Last network counter sample per machine, for the transfer rate.
    net: std::sync::Mutex<HashMap<String, NetSample>>,
    /// Last CPU time sample per VM (key: machine id / VM id).
    vm_cpu: std::sync::Mutex<HashMap<String, VmCpuSamples>>,
    /// Guest addresses from libvirt, reused for a minute.
    guest_addrs: std::sync::Mutex<HashMap<String, (String, Instant)>>,
    /// Open SFTP sessions per endpoint, with the connection they run on.
    sftp: tokio::sync::Mutex<HashMap<String, (Arc<Conn>, Arc<SftpSession>)>>,
    /// Cancel flags of running file transfers.
    transfers: std::sync::Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// When the config file was last read, to pick up changes made by another process.
    config_stamp: std::sync::Mutex<Option<std::time::SystemTime>>,
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

fn config_mtime() -> Option<std::time::SystemTime> {
    config::config_path().ok().and_then(|p| std::fs::metadata(p).ok()).and_then(|m| m.modified().ok())
}

impl Core {
    /// Loads the config file; a broken file yields an empty config plus the error to show.
    pub fn load() -> (Self, Option<String>) {
        let (config, err) = match config::load() {
            Ok(c) => (c, None),
            Err(e) => (Config::default(), Some(format!("{e:#}"))),
        };
        let known_hosts = config::config_dir().map(|d| d.join("known_hosts")).unwrap_or_else(|_| "known_hosts".into());
        (Self { config: RwLock::new(config), pool: ssh::Pool::new(known_hosts), cpu: Default::default(), net: Default::default(), vm_cpu: Default::default(), guest_addrs: Default::default(), sftp: Default::default(), transfers: Default::default(), config_stamp: std::sync::Mutex::new(config_mtime()) }, err)
    }

    pub fn config(&self) -> Config {
        self.config.read().unwrap().clone()
    }

    /// Re-reads the config file if another process (the GUI or an editor) changed it.
    pub fn refresh_config(&self) -> Result<()> {
        let stamp = config_mtime();
        let mut last = self.config_stamp.lock().unwrap();
        if stamp != *last {
            *last = stamp;
            drop(last);
            self.reload_config()?;
        }
        Ok(())
    }

    pub fn save_config(&self, mut config: Config) -> Result<Config> {
        config.normalize()?;
        config::save(&config)?;
        *self.config_stamp.lock().unwrap() = config_mtime();
        *self.config.write().unwrap() = config.clone();
        self.pool.clear();
        Ok(config)
    }

    pub fn reload_config(&self) -> Result<Config> {
        let config = config::load()?;
        *self.config_stamp.lock().unwrap() = config_mtime();
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
        let now = Instant::now();
        let mut st = status::parse(&out.stdout);
        let mut cpu = self.cpu.lock().unwrap();
        st.apply_cpu(cpu.get(&m.id).copied());
        if let Some(ticks) = st.cpu_ticks {
            cpu.insert(m.id.clone(), ticks);
        }
        let mut net = self.net.lock().unwrap();
        st.apply_net(net.get(&m.id).copied(), now);
        if let Some(b) = st.net_bytes {
            net.insert(m.id.clone(), (b, now));
        }
        let mut vm_cpu = self.vm_cpu.lock().unwrap();
        let samples = vm_cpu.entry(m.id.clone()).or_default();
        st.apply_vm_cpu(samples, now);
        samples.clear();
        for v in &st.vms {
            if let Some(t) = v.cpu_time.filter(|_| v.running) {
                samples.insert(v.id.clone(), (t, now));
            }
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
            Some(_) if ["password", "authentication is required", "terminal is required"].iter().any(|m| out.stderr.contains(m)) => bail!(
                "sudo asks for a password. Allow 'systemctl {verb}' without one (see docs/sudoers-devbench in the DevBench repository)."
            ),
            Some(c) => bail!("systemctl {verb} failed ({c}): {}", out.stderr.trim()),
        }
    }

    pub async fn vm_action(&self, machine: &str, vm: &str, action: VmAction) -> Result<()> {
        let (m, vm, ep) = self.machine_vm(machine, vm)?;
        let verb = action.verb();
        // Shutting down from inside the guest is much faster than libvirt's ACPI signal,
        // which a guest (Windows in particular) answers at its own pace.
        if matches!(action, VmAction::Shutdown | VmAction::Reboot) && !vm.guest_ssh_user.trim().is_empty() {
            let reboot = action == VmAction::Reboot;
            match tokio::time::timeout(Duration::from_secs(12), self.guest_power(&m, &vm, &ep, reboot)).await {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(e)) => log::info!("{}: shutdown inside the guest failed, using ACPI: {e:#}", vm.name),
                Err(_) => log::info!("{}: guest did not answer in time, using ACPI", vm.name),
            }
        }
        let cmd = format!("LC_ALL=C virsh -c {} {verb} {}", sh_quote(&vm.libvirt_uri), sh_quote(&vm.libvirt_name));
        let out = self.pool.exec(&ep, &cmd, Duration::from_secs(60)).await?;
        if out.code != Some(0) {
            let msg = if out.stderr.trim().is_empty() { out.stdout.trim() } else { out.stderr.trim() };
            bail!("virsh {verb}: {msg}");
        }
        Ok(())
    }

    /// Shuts the guest down (or reboots it) with its own command over SSH.
    async fn guest_power(&self, m: &Machine, vm: &Vm, ep: &Endpoint, reboot: bool) -> Result<()> {
        let guest = self.guest(m, vm, ep).await?;
        let loc = Location::Remote { machine: m.id.clone(), vm: Some(vm.id.clone()) };
        // Windows' SFTP server shows drive paths like /C:/Users/name.
        let home = self.fs(&loc).await?.home().await.unwrap_or_default();
        let windows = home.len() >= 3 && home.as_bytes()[0] == b'/' && home.as_bytes()[2] == b':';
        let cmd = match (windows, reboot) {
            (true, false) => "shutdown /s /f /t 0",
            (true, true) => "shutdown /r /f /t 0",
            (false, false) => "sudo -n systemctl poweroff",
            (false, true) => "sudo -n systemctl reboot",
        };
        let result = self.pool.exec(&guest, cmd, Duration::from_secs(10)).await;
        // The connections die with the guest; open fresh ones next time.
        self.pool.evict(&guest);
        self.sftp.lock().await.remove(&guest.cache_key());
        self.guest_addrs.lock().unwrap().remove(&format!("{}/{}", m.id, vm.id));
        match result {
            Ok(out) if out.code.unwrap_or(0) == 0 => Ok(()),
            Ok(out) => bail!("{cmd}: {}", if out.stderr.trim().is_empty() { out.stdout.trim() } else { out.stderr.trim() }),
            Err(e) => Err(e),
        }
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

    /// The disks of this PC or of a machine or guest, with label and free space. A Linux system
    /// answers `findmnt`; when that fails the guest is asked as a Windows system.
    pub async fn drives(&self, loc: &Location) -> Result<Vec<Drive>> {
        let Location::Remote { machine, vm } = loc else {
            return Ok(tokio::task::spawn_blocking(files::local_drives).await?);
        };
        let ep = self.target(machine, vm.as_deref()).await?;
        let out = self.pool.exec(&ep, files::LINUX_DRIVES, Duration::from_secs(15)).await?;
        if out.code == Some(0) {
            return Ok(files::parse_linux_drives(&out.stdout));
        }
        let out = self.pool.exec(&ep, files::WINDOWS_DRIVES, Duration::from_secs(30)).await?;
        Ok(if out.code == Some(0) { files::parse_windows_drives(&out.stdout) } else { Vec::new() })
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

    /// An interactive shell with a terminal on the machine, or in a guest when `vm` is given.
    pub async fn shell_channel(&self, machine: &str, vm: Option<&str>, cols: u32, rows: u32) -> Result<(Arc<Conn>, Channel<Msg>, String)> {
        let (m, _) = self.machine(machine)?;
        let title = match vm {
            Some(v) => format!("{} · {}", m.vm(v)?.name, m.name),
            None => m.name.clone(),
        };
        let ep = self.target(machine, vm).await?;
        let mut last = None;
        // A pooled connection may have died since its last use: retry once on a fresh one.
        for _ in 0..2 {
            let conn = self.pool.get(&ep).await?;
            match conn.handle.channel_open_session().await {
                Ok(ch) => {
                    ch.request_pty(false, "xterm-256color", cols.max(20), rows.max(5), 0, 0, &[]).await?;
                    ch.request_shell(false).await?;
                    return Ok((conn, ch, title));
                }
                Err(e) => {
                    self.pool.evict(&ep);
                    last = Some(e);
                }
            }
        }
        Err(anyhow!(last.expect("set in the loop")).context("opening a shell"))
    }

    /// A forwarded TCP connection to the desktop of the machine itself (a VNC server on it).
    pub async fn machine_vnc_channel(&self, machine: &str) -> Result<(Arc<Conn>, Channel<Msg>, String)> {
        let (m, ep) = self.machine(machine)?;
        if m.desktop_vnc_port == 0 {
            bail!("{} has no desktop configured (set its VNC port in the settings)", m.name);
        }
        let host = if m.desktop_vnc_host.trim().is_empty() { "127.0.0.1" } else { m.desktop_vnc_host.trim() };
        let (conn, ch) = self
            .pool
            .forward(&ep, host, m.desktop_vnc_port)
            .await
            .with_context(|| format!("VNC server on {} ({host}:{})", m.name, m.desktop_vnc_port))?;
        Ok((conn, ch, m.name.clone()))
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
