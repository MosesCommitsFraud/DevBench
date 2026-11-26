//! What a guest reports about itself over its own SSH: system, uptime, memory, system disk and
//! (with an NVIDIA card, e.g. one passed through) the GPU. libvirt only sees a VM from outside.
//! Linux guests answer a shell script; Windows guests a PowerShell script.

use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct GuestStats {
    pub windows: bool,
    pub os: String,
    pub uptime_secs: u64,
    pub mem_total: u64,
    pub mem_available: u64,
    /// The system disk: "/" or "C:".
    pub disk_name: String,
    pub disk_total: u64,
    pub disk_free: u64,
    pub gpu: Option<Gpu>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Gpu {
    pub name: String,
    /// 0..100
    pub util: f64,
    pub mem_used: u64,
    pub mem_total: u64,
    /// Degrees Celsius, None if not reported.
    pub temp: Option<f64>,
}

const GPU_QUERY: &str = "nvidia-smi --query-gpu=name,utilization.gpu,memory.used,memory.total,temperature.gpu --format=csv,noheader,nounits";

pub fn linux_script() -> String {
    format!(
        "export LC_ALL=C\n\
         . /etc/os-release 2>/dev/null; echo \"os|${{PRETTY_NAME:-Linux}}\"\n\
         echo \"up|$(cut -d. -f1 /proc/uptime)\"\n\
         awk '/^MemTotal:/{{t=$2}} /^MemAvailable:/{{a=$2}} END{{printf \"mem|%.0f|%.0f\\n\", t*1024, a*1024}}' /proc/meminfo\n\
         df -PB1 / | awk 'NR==2{{print \"disk|/|\"$2\"|\"$4}}'\n\
         command -v nvidia-smi >/dev/null 2>&1 && echo \"gpu|$({GPU_QUERY} 2>/dev/null | head -n1)\"\n\
         exit 0\n"
    )
}

pub fn windows_script() -> String {
    format!(
        "$ErrorActionPreference='SilentlyContinue'\n\
         $os=Get-CimInstance Win32_OperatingSystem\n\
         $d=Get-CimInstance Win32_LogicalDisk -Filter \"DeviceID='$($env:SystemDrive)'\"\n\
         \"windows|1\"\n\
         \"os|$($os.Caption)\"\n\
         \"up|$([int64]((Get-Date)-$os.LastBootUpTime).TotalSeconds)\"\n\
         \"mem|$([uint64]$os.TotalVisibleMemorySize*1024)|$([uint64]$os.FreePhysicalMemory*1024)\"\n\
         \"disk|$($env:SystemDrive)|$($d.Size)|$($d.FreeSpace)\"\n\
         $g = & {GPU_QUERY} 2>$null\n\
         if ($g) {{ \"gpu|$($g | Select-Object -First 1)\" }}\n"
    )
}

pub fn parse(out: &str) -> GuestStats {
    let mut s = GuestStats::default();
    for line in out.lines() {
        let f: Vec<&str> = line.trim().split('|').collect();
        let num = |i: usize| f.get(i).and_then(|x| x.trim().parse::<f64>().ok()).unwrap_or(0.0) as u64;
        match f[0] {
            "windows" => s.windows = true,
            "os" => s.os = f.get(1).unwrap_or(&"").trim().to_string(),
            "up" => s.uptime_secs = num(1),
            "mem" => (s.mem_total, s.mem_available) = (num(1), num(2)),
            "disk" => {
                s.disk_name = f.get(1).unwrap_or(&"").trim().to_string();
                (s.disk_total, s.disk_free) = (num(2), num(3));
            }
            "gpu" => s.gpu = f.get(1).and_then(|g| parse_gpu(g)),
            _ => {}
        }
    }
    s
}

/// One line of `nvidia-smi --format=csv,noheader,nounits`: name, util %, MiB used, MiB total, °C.
fn parse_gpu(line: &str) -> Option<Gpu> {
    let f: Vec<&str> = line.split(',').map(str::trim).collect();
    if f.len() < 4 || f[0].is_empty() {
        return None;
    }
    let n = |i: usize| f.get(i).and_then(|x| x.parse::<f64>().ok());
    Some(Gpu {
        name: f[0].to_string(),
        util: n(1).unwrap_or(0.0),
        mem_used: (n(2).unwrap_or(0.0) * 1048576.0) as u64,
        mem_total: (n(3).unwrap_or(0.0) * 1048576.0) as u64,
        temp: n(4),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_output() {
        let out = "windows|1\r\nos|Microsoft Windows 11 Pro\r\nup|3600\r\nmem|17179869184|8589934592\r\n\
                   disk|C:|255690010624|120000000000\r\ngpu|NVIDIA GeForce RTX 3060 Ti, 12, 364, 8192, 34.97\r\n";
        let s = parse(out);
        assert!(s.windows);
        assert_eq!(s.os, "Microsoft Windows 11 Pro");
        assert_eq!(s.uptime_secs, 3600);
        assert_eq!(s.mem_available, 8589934592);
        assert_eq!(s.disk_name, "C:");
        let g = s.gpu.unwrap();
        assert_eq!(g.name, "NVIDIA GeForce RTX 3060 Ti");
        assert_eq!(g.util, 12.0);
        assert_eq!(g.mem_total, 8192 * 1048576);
        assert_eq!(g.temp, Some(34.97));
    }

    #[test]
    fn linux_output_without_gpu() {
        let s = parse("os|Ubuntu 24.04 LTS\nup|99\nmem|1000|400\ndisk|/|5000|2000\n");
        assert!(!s.windows);
        assert_eq!((s.disk_total, s.disk_free), (5000, 2000));
        assert!(s.gpu.is_none());
        assert!(parse("gpu|\n").gpu.is_none());
    }
}
