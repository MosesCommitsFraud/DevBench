//! Reads a machine's state (uptime, CPU, RAM, disk) and its VMs' states in one SSH command.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::config::{Machine, Vm};
use crate::ssh::sh_quote;

#[derive(Debug, Serialize, Default)]
pub struct MachineStatus {
    pub online: bool,
    pub hostname: String,
    pub kernel: String,
    pub uptime_secs: u64,
    pub load: [f64; 3],
    pub cpus: u32,
    pub cpu_percent: f64,
    pub mem_total: u64,
    pub mem_available: u64,
    pub disk_total: u64,
    pub disk_used: u64,
    /// Distribution name, e.g. "Ubuntu 24.04.1 LTS".
    pub os: String,
    pub cpu_model: String,
    /// Interface of the default route and its IPv4 address.
    pub iface: String,
    pub ip: String,
    /// Bytes per second on the default interface since the last poll.
    pub net_rx_rate: f64,
    pub net_tx_rate: f64,
    pub vms: Vec<VmStatus>,
    /// Packages being installed right now, by DevBench's update service or by the system itself.
    pub updating: Option<Updating>,
    /// Raw (total, idle) CPU ticks; CPU usage is the difference between two polls.
    #[serde(skip)]
    pub cpu_ticks: Option<(u64, u64)>,
    /// Raw (rx, tx) byte counters of the default interface.
    #[serde(skip)]
    pub net_bytes: Option<(u64, u64)>,
}

impl MachineStatus {
    /// Sets `cpu_percent` from the previous poll's ticks, or the average since boot on the first poll.
    pub fn apply_cpu(&mut self, previous: Option<(u64, u64)>) {
        let Some((t2, i2)) = self.cpu_ticks else { return };
        let (t1, i1) = previous.filter(|p| p.0 < t2).unwrap_or((0, 0));
        let dt = t2 - t1;
        if dt > 0 {
            self.cpu_percent = 100.0 * (dt - i2.saturating_sub(i1).min(dt)) as f64 / dt as f64;
        }
    }
}

#[derive(Debug, Serialize, Default, PartialEq)]
pub struct Updating {
    /// DevBench's update service (devbench-update) runs; false = apt or dpkg run otherwise,
    /// e.g. unattended-upgrades.
    pub devbench: bool,
    /// The program at work, e.g. "dpkg" or "unattended-upgr".
    pub process: String,
    /// The latest line of the update log, e.g. "Setting up linux-image-6.8.0-49-generic ...".
    pub line: String,
}

#[derive(Debug, Serialize, Default)]
pub struct VmStatus {
    pub id: String,
    /// libvirt's state text, e.g. "running", "shut off", "paused"; empty if unknown.
    pub state: String,
    pub running: bool,
    pub error: Option<String>,
    pub vcpus: u32,
    /// Memory assigned to the VM, in bytes.
    pub memory: u64,
    /// Share of the VM's vCPUs in use since the last poll, 0..100; None on the first poll.
    pub cpu_percent: Option<f64>,
    /// IPv4 address libvirt knows for the guest (DHCP lease or ARP), if any.
    pub ip: String,
    /// libvirt starts the VM when the host boots.
    pub autostart: bool,
    /// Total CPU time of the VM in nanoseconds (for the usage between polls).
    #[serde(skip)]
    pub cpu_time: Option<u64>,
}

impl MachineStatus {
    /// Sets the network rates from the previous poll's counters and the time in between.
    pub fn apply_net(&mut self, previous: Option<((u64, u64), std::time::Instant)>, now: std::time::Instant) {
        let (Some((rx, tx)), Some(((prx, ptx), t))) = (self.net_bytes, previous) else { return };
        let secs = now.duration_since(t).as_secs_f64();
        if secs > 0.2 && rx >= prx && tx >= ptx {
            self.net_rx_rate = (rx - prx) as f64 / secs;
            self.net_tx_rate = (tx - ptx) as f64 / secs;
        }
    }

    /// Sets each VM's CPU usage from its previous CPU time sample.
    pub fn apply_vm_cpu(&mut self, previous: &std::collections::HashMap<String, (u64, std::time::Instant)>, now: std::time::Instant) {
        for v in &mut self.vms {
            let (Some(t2), Some((t1, at))) = (v.cpu_time, previous.get(&v.id)) else { continue };
            let wall = now.duration_since(*at).as_nanos() as f64 * v.vcpus.max(1) as f64;
            if v.running && wall > 0.0 && t2 >= *t1 {
                v.cpu_percent = Some((100.0 * (t2 - t1) as f64 / wall).min(100.0));
            }
        }
    }
}

pub fn script(m: &Machine) -> String {
    let mut s = String::from(
        "export LC_ALL=C\n\
         echo @uptime; cat /proc/uptime\n\
         echo @load; cat /proc/loadavg\n\
         echo @cpu; head -n1 /proc/stat; sleep 0.5; head -n1 /proc/stat\n\
         echo @nproc; nproc\n\
         echo @mem; grep -E '^(MemTotal|MemAvailable):' /proc/meminfo\n\
         echo @disk; df -P -B1 / | tail -n1\n\
         echo @host; hostname; uname -r\n\
         echo @os; (. /etc/os-release 2>/dev/null; echo \"$PRETTY_NAME\")\n\
         echo @cpumodel; grep -m1 'model name' /proc/cpuinfo | cut -d: -f2-\n\
         echo @net; i=$(ip -o route show default 2>/dev/null | awk '{print $5; exit}'); echo \"$i\"; \
         ip -o -4 addr show dev \"$i\" 2>/dev/null | awk '{print $4; exit}'; grep \"^ *$i:\" /proc/net/dev\n",
    );
    s.push_str(UPDATING);
    for vm in &m.vms {
        let (uri, name) = (sh_quote(&vm.libvirt_uri), sh_quote(&vm.libvirt_name));
        s.push_str(&format!(
            "echo @vm {id}; st=$(virsh -c {uri} domstate {name} 2>&1); echo \"$st\"\n\
             echo @vmstats {id}; virsh -c {uri} domstats {name} --cpu-total --balloon --vcpu 2>/dev/null | grep -E 'cpu.time=|balloon.maximum=|vcpu.current='\n\
             echo @vmauto {id}; virsh -c {uri} dominfo {name} 2>/dev/null | sed -n 's/^Autostart: *//p'\n\
             echo @vmip {id}; [ \"$st\" = running ] && {{ virsh -c {uri} domifaddr {name} --source lease 2>/dev/null; virsh -c {uri} domifaddr {name} --source arp 2>/dev/null; }}\n",
            id = vm.id
        ));
    }
    // The last VM's `[ ... ] &&` must not decide the exit status.
    s.push_str("true\n");
    s
}

/// Whether packages are being installed: DevBench's update service, or apt/dpkg on their own.
/// unattended-upgrade is matched by its command line: its process name is cut to
/// "unattended-upgr", the same as the helper unattended-upgrade-shutdown that always runs.
const UPDATING: &str = r#"echo @upd
echo "unit|$(systemctl is-active devbench-update 2>/dev/null)"
p="$(pgrep -x 'apt-get|apt|dpkg' 2>/dev/null | head -n1 | xargs -r ps -o comm= -p 2>/dev/null)"
[ -z "$p" ] && pgrep -f '/usr/bin/unattended-upgrade( |$)' >/dev/null 2>&1 && p=unattended-upgrade
echo "proc|$p"
[ -r /var/log/devbench-update.log ] && echo "line|$(tail -n 30 /var/log/devbench-update.log | grep -v '^@@' | grep -v '^ *$' | tail -n1 | cut -c1-200)"
"#;

fn sections(out: &str) -> Vec<(&str, Vec<&str>)> {
    let mut v: Vec<(&str, Vec<&str>)> = Vec::new();
    for line in out.lines() {
        if let Some(name) = line.strip_prefix('@') {
            v.push((name.trim(), Vec::new()));
        } else if let Some(last) = v.last_mut() {
            last.1.push(line);
        }
    }
    v
}

fn cpu_ticks(line: &str) -> Option<(u64, u64)> {
    let nums: Vec<u64> = line.split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
    if nums.len() < 4 {
        return None;
    }
    let total: u64 = nums.iter().take(8).sum();
    let idle = nums[3] + nums.get(4).copied().unwrap_or(0); // idle + iowait
    Some((total, idle))
}

fn kib_field(lines: &[&str], name: &str) -> u64 {
    lines
        .iter()
        .find(|l| l.starts_with(name))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|x| x.parse::<u64>().ok())
        .unwrap_or(0)
        * 1024
}

pub fn parse(out: &str) -> MachineStatus {
    let mut st = MachineStatus { online: true, ..Default::default() };
    for (name, lines) in sections(out) {
        let first = lines.first().copied().unwrap_or("");
        match name {
            "uptime" => {
                st.uptime_secs = first.split_whitespace().next().and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0) as u64
            }
            "load" => {
                for (i, x) in first.split_whitespace().take(3).enumerate() {
                    st.load[i] = x.parse().unwrap_or(0.0);
                }
            }
            "cpu" => st.cpu_ticks = cpu_ticks(first),
            "nproc" => st.cpus = first.trim().parse().unwrap_or(0),
            "mem" => {
                st.mem_total = kib_field(&lines, "MemTotal:");
                st.mem_available = kib_field(&lines, "MemAvailable:");
            }
            "disk" => {
                let f: Vec<&str> = first.split_whitespace().collect();
                if f.len() >= 4 {
                    st.disk_total = f[1].parse().unwrap_or(0);
                    st.disk_used = f[2].parse().unwrap_or(0);
                }
            }
            "host" => {
                st.hostname = first.trim().to_string();
                st.kernel = lines.get(1).map(|s| s.trim().to_string()).unwrap_or_default();
            }
            "os" => st.os = first.trim().to_string(),
            "upd" => st.updating = updating(&lines),
            "cpumodel" => st.cpu_model = tidy_cpu_model(first),
            "net" => {
                st.iface = first.trim().to_string();
                for l in lines.iter().skip(1) {
                    if let Some((_, counters)) = l.split_once(':').filter(|(n, _)| n.trim() == st.iface && !st.iface.is_empty()) {
                        let f: Vec<u64> = counters.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                        if f.len() >= 9 {
                            st.net_bytes = Some((f[0], f[8]));
                        }
                    } else if l.contains('/') && st.ip.is_empty() {
                        st.ip = l.trim().split('/').next().unwrap_or("").to_string();
                    }
                }
            }
            _ => {
                if let Some(id) = name.strip_prefix("vm ") {
                    let text = lines.join(" ").trim().to_string();
                    let known = ["running", "idle", "paused", "in shutdown", "shut off", "crashed", "pmsuspended"];
                    let (state, error) = if known.contains(&text.as_str()) { (text, None) } else { (String::new(), Some(text)) };
                    st.vms.push(VmStatus { id: id.to_string(), running: state == "running", state, error, ..Default::default() });
                } else if let Some(id) = name.strip_prefix("vmstats ") {
                    let Some(v) = st.vms.iter_mut().find(|v| v.id == id) else { continue };
                    for l in &lines {
                        let Some((k, val)) = l.trim().split_once('=') else { continue };
                        let n = val.trim().parse::<u64>().unwrap_or(0);
                        match k {
                            "cpu.time" => v.cpu_time = Some(n),
                            "balloon.maximum" => v.memory = n * 1024,
                            "vcpu.current" => v.vcpus = n as u32,
                            _ => {}
                        }
                    }
                } else if let Some(id) = name.strip_prefix("vmauto ") {
                    let Some(v) = st.vms.iter_mut().find(|v| v.id == id) else { continue };
                    v.autostart = first.trim() == "enable";
                } else if let Some(id) = name.strip_prefix("vmip ") {
                    let Some(v) = st.vms.iter_mut().find(|v| v.id == id) else { continue };
                    v.ip = lines
                        .iter()
                        .filter_map(|l| {
                            let f: Vec<&str> = l.split_whitespace().collect();
                            (f.len() >= 4 && f[2] == "ipv4").then(|| f[3].split('/').next().unwrap_or("").to_string())
                        })
                        .find(|a| !a.is_empty() && !a.starts_with("127."))
                        .unwrap_or_default();
                }
            }
        }
    }
    st
}

fn updating(lines: &[&str]) -> Option<Updating> {
    let field = |k: &str| lines.iter().find_map(|l| l.strip_prefix(k)).map(|v| v.trim().to_string()).unwrap_or_default();
    let unit = field("unit|");
    let devbench = unit == "active" || unit == "activating";
    let process = field("proc|");
    if !devbench && process.is_empty() {
        return None;
    }
    Some(Updating { devbench, process, line: if devbench { field("line|") } else { String::new() } })
}

/// "AMD Ryzen 9 7950X 16-Core Processor" -> "AMD Ryzen 9 7950X".
fn tidy_cpu_model(raw: &str) -> String {
    let mut s = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    for noise in ["(R)", "(TM)", "(tm)", " CPU", " Processor"] {
        s = s.replace(noise, "");
    }
    if let Some(i) = s.find(" @ ") {
        s.truncate(i);
    }
    s.split(' ').filter(|w| !w.ends_with("-Core")).collect::<Vec<_>>().join(" ").trim().to_string()
}

pub const STATUS_TIMEOUT: Duration = Duration::from_secs(15);

/// The guest's IPv4 address as libvirt knows it (DHCP lease, then guest agent, then ARP).
pub async fn guest_address(pool: &crate::ssh::Pool, ep: &crate::sshconf::Endpoint, vm: &Vm) -> Result<String> {
    if !vm.guest_address.trim().is_empty() {
        return Ok(vm.guest_address.trim().to_string());
    }
    let mut cmd = String::from("export LC_ALL=C\n");
    for source in ["lease", "agent", "arp"] {
        cmd.push_str(&format!(
            "virsh -c {} domifaddr {} --source {source} 2>/dev/null\n",
            sh_quote(&vm.libvirt_uri),
            sh_quote(&vm.libvirt_name)
        ));
    }
    let out = pool.exec(ep, &cmd, Duration::from_secs(15)).await.context("asking libvirt for the guest address")?;
    for line in out.stdout.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() >= 4 && f[2] == "ipv4" {
            let addr = f[3].split('/').next().unwrap_or("");
            if !addr.is_empty() && !addr.starts_with("127.") {
                return Ok(addr.to_string());
            }
        }
    }
    bail!("libvirt does not know an IPv4 address for '{}' (is it running? set the guest address in the settings)", vm.libvirt_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_status() {
        let out = "@uptime\n3551.08 27000.1\n@load\n0.17 0.68 1.33 1/700 1234\n@cpu\n\
                   cpu  150 0 150 900 0 0 0 0 0 0\n@nproc\n16\n\
                   @mem\nMemTotal:       30000000 kB\nMemAvailable:   20000000 kB\n\
                   @disk\n/dev/nvme0n1p2 1000 400 600 40% /\n@host\ndevhost\n7.0.0\n\
                   @os\nUbuntu 24.04.1 LTS\n@cpumodel\n AMD Ryzen 9 7950X 16-Core Processor\n\
                   @net\nenp5s0\n192.0.2.10/24\n  enp5s0: 1000 10 0 0 0 0 0 0 2000 20 0 0 0 0 0 0\n\
                   @vm abc\nrunning\n@vmstats abc\n  cpu.time=5000\n  balloon.maximum=8388608\n  vcpu.current=4\n\
                   @vmip abc\n vnet0 52:54:00:aa:bb:cc ipv4 192.168.122.5/24\n\
                   @vm def\nshut off\n@vm ghi\nerror: failed to get domain 'x'\n";
        let mut st = parse(out);
        st.apply_cpu(Some((1000, 800)));
        assert_eq!(st.uptime_secs, 3551);
        assert_eq!(st.cpus, 16);
        assert!((st.cpu_percent - 50.0).abs() < 0.01);
        assert_eq!(st.mem_total, 30000000 * 1024);
        assert_eq!(st.disk_used, 400);
        assert_eq!(st.hostname, "devhost");
        assert!(st.vms[0].running);
        assert_eq!(st.vms[1].state, "shut off");
        assert!(st.vms[2].error.is_some());
        assert_eq!(st.os, "Ubuntu 24.04.1 LTS");
        assert_eq!(st.cpu_model, "AMD Ryzen 9 7950X");
        assert_eq!(st.ip, "192.0.2.10");
        assert_eq!(st.net_bytes, Some((1000, 2000)));
        assert_eq!(st.vms[0].vcpus, 4);
        assert_eq!(st.vms[0].memory, 8 * 1024 * 1024 * 1024);
        assert_eq!(st.vms[0].ip, "192.168.122.5");
        assert!(!st.vms[0].autostart);
        assert!(st.updating.is_none());
    }

    #[test]
    fn script_is_valid_shell() {
        let mut m = Machine::default();
        m.vms.push(Vm { id: "w".into(), libvirt_name: "win 11".into(), ..Default::default() });
        let s = script(&m);
        assert!(s.contains("echo @upd") && s.contains("echo @vmauto w"));
        // Syntax check where a bash is at hand (Linux CI, Git Bash).
        if let Ok(out) = std::process::Command::new("bash").args(["-n", "-c", &s]).output() {
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        }
    }

    #[test]
    fn parses_autostart_and_updates() {
        let out = "@vm abc\nrunning\n@vmauto abc\nenable\n@vm def\nshut off\n@vmauto def\ndisable\n\
                   @upd\nunit|active\nproc|dpkg\nline|Setting up libvirt-daemon (10.0.0) ...\n";
        let st = parse(out);
        assert!(st.vms[0].autostart && !st.vms[1].autostart);
        assert_eq!(
            st.updating,
            Some(Updating { devbench: true, process: "dpkg".into(), line: "Setting up libvirt-daemon (10.0.0) ...".into() })
        );
        let idle = parse("@upd\nunit|inactive\nproc|\nline|0 upgraded, 0 newly installed\n");
        assert!(idle.updating.is_none());
        let other = parse("@upd\nunit|inactive\nproc|unattended-upgr\n").updating.unwrap();
        assert!(!other.devbench && other.line.is_empty());
    }
}
