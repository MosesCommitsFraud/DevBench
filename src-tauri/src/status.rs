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
    pub vms: Vec<VmStatus>,
    /// Raw (total, idle) CPU ticks; CPU usage is the difference between two polls.
    #[serde(skip)]
    pub cpu_ticks: Option<(u64, u64)>,
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

#[derive(Debug, Serialize)]
pub struct VmStatus {
    pub id: String,
    /// libvirt's state text, e.g. "running", "shut off", "paused"; empty if unknown.
    pub state: String,
    pub running: bool,
    pub error: Option<String>,
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
         echo @host; hostname; uname -r\n",
    );
    for vm in &m.vms {
        s.push_str(&format!(
            "echo @vm {}; virsh -c {} domstate {} 2>&1\n",
            vm.id,
            sh_quote(&vm.libvirt_uri),
            sh_quote(&vm.libvirt_name)
        ));
    }
    s
}

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
            _ => {
                if let Some(id) = name.strip_prefix("vm ") {
                    let text = lines.join(" ").trim().to_string();
                    let known = ["running", "idle", "paused", "in shutdown", "shut off", "crashed", "pmsuspended"];
                    let (state, error) = if known.contains(&text.as_str()) { (text, None) } else { (String::new(), Some(text)) };
                    st.vms.push(VmStatus { id: id.to_string(), running: state == "running", state, error });
                }
            }
        }
    }
    st
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
                   cpu  100 0 100 800 0 0 0 0 0 0\ncpu  150 0 150 900 0 0 0 0 0 0\n@nproc\n16\n\
                   @mem\nMemTotal:       30000000 kB\nMemAvailable:   20000000 kB\n\
                   @disk\n/dev/nvme0n1p2 1000 400 600 40% /\n@host\ndevhost\n7.0.0\n\
                   @vm abc\nrunning\n\n@vm def\nshut off\n@vm ghi\nerror: failed to get domain 'x'\n";
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
    }
}
