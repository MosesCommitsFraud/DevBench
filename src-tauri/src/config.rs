//! The user's configuration: machines and their VMs, stored as TOML in the OS config folder.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub machines: Vec<Machine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Machine {
    pub id: String,
    pub name: String,
    /// Hostname, IP address or a `Host` alias from `~/.ssh/config`.
    pub ssh_host: String,
    /// 0 = from `~/.ssh/config`, otherwise 22.
    pub ssh_port: u16,
    /// Empty = from `~/.ssh/config`, otherwise the local user name.
    pub ssh_user: String,
    /// Empty = `IdentityFile` from `~/.ssh/config`, otherwise the default key files.
    pub ssh_key: String,
    /// Like OpenSSH's ProxyJump: `[user@]host[:port]` or an alias. Empty = from `~/.ssh/config`.
    pub jump_host: String,
    /// MAC address for Wake-on-LAN. Empty hides the Wake button.
    pub mac: String,
    pub wol_broadcast: String,
    pub wol_port: u16,
    /// AI agents (MCP) may see this machine: status, Wake-on-LAN, its VM list.
    pub agent_access: bool,
    /// Agents may run shell commands and read/write files on the machine.
    pub agent_exec: bool,
    /// Agents may power off or reboot the machine and force VMs off.
    pub agent_power: bool,
    pub vms: Vec<Vm>,
}

impl Default for Machine {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            ssh_host: String::new(),
            ssh_port: 0,
            ssh_user: String::new(),
            ssh_key: String::new(),
            jump_host: String::new(),
            mac: String::new(),
            wol_broadcast: "255.255.255.255".into(),
            wol_port: 9,
            agent_access: false,
            agent_exec: false,
            agent_power: false,
            vms: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum VncMode {
    /// SSH through the machine into the guest, then to `vnc_host:vnc_port` inside the guest.
    #[default]
    Guest,
    /// Forward from the machine itself to `vnc_host:vnc_port` (e.g. QEMU's own VNC display).
    Host,
    /// No VNC.
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Vm {
    pub id: String,
    pub name: String,
    pub libvirt_name: String,
    pub libvirt_uri: String,
    /// Empty = ask libvirt (`virsh domifaddr`).
    pub guest_address: String,
    pub guest_ssh_user: String,
    pub guest_ssh_port: u16,
    pub guest_ssh_key: String,
    pub vnc_mode: VncMode,
    pub vnc_host: String,
    pub vnc_port: u16,
    /// Password of the VNC server, only needed if it has one. Stored in plain text: prefer
    /// a VNC server that listens on localhost only, reached through SSH.
    pub vnc_password: String,
    /// Agents may start/stop the VM, take screenshots and use mouse and keyboard.
    pub agent_access: bool,
    /// Agents may run commands and read/write files inside the guest.
    pub agent_exec: bool,
}

impl Default for Vm {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            libvirt_name: String::new(),
            libvirt_uri: "qemu:///system".into(),
            guest_address: String::new(),
            guest_ssh_user: String::new(),
            guest_ssh_port: 22,
            guest_ssh_key: String::new(),
            vnc_mode: VncMode::Guest,
            vnc_host: "127.0.0.1".into(),
            vnc_port: 5900,
            vnc_password: String::new(),
            agent_access: false,
            agent_exec: false,
        }
    }
}

impl Machine {
    /// Finds a VM by id, else by display name or libvirt name (case-insensitive).
    pub fn vm(&self, key: &str) -> Result<&Vm> {
        let k = key.trim();
        self.vms
            .iter()
            .find(|v| v.id == k)
            .or_else(|| self.vms.iter().find(|v| v.name.eq_ignore_ascii_case(k) || v.libvirt_name.eq_ignore_ascii_case(k)))
            .with_context(|| format!("{} has no VM '{key}'", self.name))
    }
}

impl Config {
    /// Finds a machine by id, or else by name (case-insensitive).
    pub fn machine(&self, key: &str) -> Result<&Machine> {
        self.machines
            .iter()
            .find(|m| m.id == key)
            .or_else(|| self.machines.iter().find(|m| m.name.eq_ignore_ascii_case(key.trim())))
            .with_context(|| format!("unknown machine '{key}'"))
    }

    /// Checks the values the app relies on and fills in missing ids.
    pub fn normalize(&mut self) -> Result<()> {
        let mut ids = std::collections::HashSet::new();
        for (i, m) in self.machines.iter_mut().enumerate() {
            m.name = m.name.trim().to_string();
            m.ssh_host = m.ssh_host.trim().to_string();
            if m.name.is_empty() {
                m.name = if m.ssh_host.is_empty() { format!("Machine {}", i + 1) } else { m.ssh_host.clone() };
            }
            if m.ssh_host.is_empty() {
                bail!("{}: SSH host is empty", m.name);
            }
            if m.id.trim().is_empty() || !ids.insert(m.id.clone()) {
                m.id = new_id();
                ids.insert(m.id.clone());
            }
            if !m.mac.trim().is_empty() {
                crate::wol::parse_mac(&m.mac).with_context(|| format!("{}: MAC address", m.name))?;
            }
            if m.wol_broadcast.trim().is_empty() {
                m.wol_broadcast = "255.255.255.255".into();
            }
            if m.wol_port == 0 {
                m.wol_port = 9;
            }
            let mut vm_ids = std::collections::HashSet::new();
            for v in m.vms.iter_mut() {
                v.libvirt_name = v.libvirt_name.trim().to_string();
                if v.libvirt_name.is_empty() {
                    bail!("{}: a VM has no libvirt name", m.name);
                }
                if v.name.trim().is_empty() {
                    v.name = v.libvirt_name.clone();
                }
                if v.libvirt_uri.trim().is_empty() {
                    v.libvirt_uri = "qemu:///system".into();
                }
                if v.id.trim().is_empty() || !vm_ids.insert(v.id.clone()) {
                    v.id = new_id();
                    vm_ids.insert(v.id.clone());
                }
                if v.guest_ssh_port == 0 {
                    v.guest_ssh_port = 22;
                }
                if v.vnc_host.trim().is_empty() {
                    v.vnc_host = "127.0.0.1".into();
                }
                if v.vnc_port == 0 {
                    v.vnc_port = 5900;
                }
                if v.vnc_mode == VncMode::Guest && v.guest_ssh_user.trim().is_empty() {
                    bail!("{} / {}: VNC through the guest needs a guest SSH user", m.name, v.name);
                }
            }
        }
        Ok(())
    }
}

pub fn new_id() -> String {
    let mut b = [0u8; 6];
    getrandom::fill(&mut b).expect("OS random source");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The folder holding `config.toml` and DevBench's own `known_hosts`.
pub fn config_dir() -> Result<PathBuf> {
    Ok(dirs::config_dir().context("no config folder on this system")?.join("DevBench"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

pub fn load() -> Result<Config> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(Config::default());
    }
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let mut cfg: Config = toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    cfg.normalize()?;
    Ok(cfg)
}

pub fn save(cfg: &Config) -> Result<()> {
    let path = config_path()?;
    std::fs::create_dir_all(path.parent().expect("config path has a parent"))?;
    let body = toml::to_string_pretty(cfg)?;
    let text = format!(
        "# DevBench configuration. Edited by the app's settings screen; you can also edit it by hand.\n\
         # See config.example.toml in the DevBench repository for every field.\n\n{body}"
    );
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}
