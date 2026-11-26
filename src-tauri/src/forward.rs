//! Port forwarding like `ssh -L`: a listener on 127.0.0.1 of this PC whose connections go
//! through SSH to a port on a machine or in a guest. A listener costs nothing until something
//! connects; the SSH channel opens per connection, so a forward survives the machine sleeping.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use crate::core::Core;

#[derive(Debug, Clone, Serialize)]
pub struct ForwardInfo {
    /// "machine/vm/port", so the same port of the same system is forwarded only once.
    pub id: String,
    pub machine: String,
    pub vm: Option<String>,
    /// Where the connection goes, seen from the machine or guest ("localhost" for its own ports).
    pub remote_host: String,
    pub remote_port: u16,
    /// The port on this PC (127.0.0.1).
    pub local_port: u16,
    /// Connections open right now.
    pub connections: u32,
    /// The last failure to reach the port; cleared by the next connection that works.
    pub error: Option<String>,
}

struct Entry {
    info: Arc<Mutex<ForwardInfo>>,
    task: JoinHandle<()>,
}

#[derive(Default)]
pub struct Forwards {
    map: Mutex<HashMap<String, Entry>>,
}

impl Drop for Forwards {
    fn drop(&mut self) {
        for e in self.map.lock().unwrap().values() {
            e.task.abort();
        }
    }
}

impl Forwards {
    pub fn list(&self) -> Vec<ForwardInfo> {
        let mut v: Vec<ForwardInfo> = self.map.lock().unwrap().values().map(|e| e.info.lock().unwrap().clone()).collect();
        v.sort_by(|a, b| a.machine.cmp(&b.machine).then(a.vm.cmp(&b.vm)).then(a.remote_port.cmp(&b.remote_port)));
        v
    }

    /// Starts forwarding `remote_port` of a machine (or a guest when `vm` is given) to this PC.
    /// Without `local_port` it takes the same port number when that is free, else any free one.
    pub async fn open(
        &self,
        core: Arc<Core>,
        machine: &str,
        vm: Option<&str>,
        remote_host: Option<&str>,
        remote_port: u16,
        local_port: Option<u16>,
    ) -> Result<ForwardInfo> {
        if remote_port == 0 {
            bail!("port 0 cannot be forwarded");
        }
        let id = format!("{machine}/{}/{remote_port}", vm.unwrap_or(""));
        if let Some(e) = self.map.lock().unwrap().get(&id) {
            return Ok(e.info.lock().unwrap().clone());
        }
        let listener = match local_port.filter(|p| *p != 0) {
            Some(p) => TcpListener::bind(("127.0.0.1", p)).await.with_context(|| format!("port {p} on this PC is taken"))?,
            None => match TcpListener::bind(("127.0.0.1", remote_port)).await {
                Ok(l) => l,
                Err(_) => TcpListener::bind(("127.0.0.1", 0)).await?,
            },
        };
        let info = Arc::new(Mutex::new(ForwardInfo {
            id: id.clone(),
            machine: machine.to_string(),
            vm: vm.map(String::from),
            remote_host: remote_host.filter(|h| !h.trim().is_empty()).unwrap_or("localhost").to_string(),
            remote_port,
            local_port: listener.local_addr()?.port(),
            connections: 0,
            error: None,
        }));
        let task = tokio::spawn(accept_loop(listener, core, info.clone()));
        let snapshot = info.lock().unwrap().clone();
        let mut map = self.map.lock().unwrap();
        if let Some(old) = map.insert(id, Entry { info, task }) {
            // Opened twice at the same moment: keep the newer one.
            old.task.abort();
        }
        Ok(snapshot)
    }

    pub fn close(&self, id: &str) {
        if let Some(e) = self.map.lock().unwrap().remove(id) {
            e.task.abort();
        }
    }
}

async fn accept_loop(listener: TcpListener, core: Arc<Core>, info: Arc<Mutex<ForwardInfo>>) {
    loop {
        let mut sock = match listener.accept().await {
            Ok((s, _)) => s,
            Err(e) => {
                log::warn!("port forward accept: {e}");
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                continue;
            }
        };
        sock.set_nodelay(true).ok();
        let core = core.clone();
        let info = info.clone();
        tokio::spawn(async move {
            let (machine, vm, host, port) = {
                let i = info.lock().unwrap();
                (i.machine.clone(), i.vm.clone(), i.remote_host.clone(), i.remote_port)
            };
            match core.port_channel(&machine, vm.as_deref(), &host, port).await {
                Ok((conn, ch)) => {
                    {
                        let mut i = info.lock().unwrap();
                        i.connections += 1;
                        i.error = None;
                    }
                    let mut stream = ch.into_stream();
                    let _ = tokio::io::copy_bidirectional(&mut sock, &mut stream).await;
                    drop(conn);
                    info.lock().unwrap().connections -= 1;
                }
                Err(e) => {
                    log::info!("port forward {machine}/{port}: {e:#}");
                    info.lock().unwrap().error = Some(format!("{e:#}"));
                }
            }
        });
    }
}

/// A port something listens on, as `ss` or Windows reports it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ListenPort {
    pub port: u16,
    /// The address it listens on; "localhost" when that is a wildcard or loopback address.
    pub host: String,
    /// The program, when the system tells (Linux names only the user's own programs).
    pub process: String,
}

pub const LINUX_LISTENING: &str = "ss -Hltnp";
/// PowerShell script for Windows systems.
pub const WINDOWS_LISTENING: &str = "Get-NetTCPConnection -State Listen | ForEach-Object { '{0}|{1}|{2}' -f $_.LocalAddress,$_.LocalPort,(Get-Process -Id $_.OwningProcess -ErrorAction SilentlyContinue).ProcessName }";

/// Ports that are part of the system itself, not something to open in a browser.
fn boring(port: u16, host: &str) -> bool {
    matches!(port, 22 | 53 | 111 | 135 | 139 | 445 | 631 | 5040 | 5355 | 5357 | 7680)
        || port >= 49152
        || host.starts_with("127.0.0.53")
        || host.starts_with("127.0.0.54")
}

fn normalize_host(addr: &str) -> String {
    let a = addr.trim_matches(|c| c == '[' || c == ']');
    let a = a.split('%').next().unwrap_or(a);
    if matches!(a, "" | "*" | "0.0.0.0" | "::" | "::1" | "localhost") || a.starts_with("127.") {
        "localhost".into()
    } else {
        a.to_string()
    }
}

fn push(v: &mut Vec<ListenPort>, port: u16, host: String, process: String) {
    if boring(port, &host) {
        return;
    }
    let host = normalize_host(&host);
    match v.iter_mut().find(|p| p.port == port) {
        // The same port on several addresses: the wildcard/loopback one reaches all of them.
        Some(p) => {
            if host == "localhost" {
                p.host = host;
            }
            if p.process.is_empty() {
                p.process = process;
            }
        }
        None => v.push(ListenPort { port, host, process }),
    }
}

/// Parses `ss -Hltnp`: `LISTEN 0 511 0.0.0.0:3000 0.0.0.0:* users:(("node",pid=1,fd=20))`.
pub fn parse_ss(out: &str) -> Vec<ListenPort> {
    let mut v = Vec::new();
    for line in out.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 5 {
            continue;
        }
        let local = f[3];
        let Some(colon) = local.rfind(':') else { continue };
        let Ok(port) = local[colon + 1..].parse::<u16>() else { continue };
        let process = line
            .find("((\"")
            .map(|i| &line[i + 3..])
            .and_then(|r| r.find('"').map(|j| r[..j].to_string()))
            .unwrap_or_default();
        push(&mut v, port, local[..colon].to_string(), process);
    }
    v.sort_by_key(|p| p.port);
    v
}

/// Parses the PowerShell line format `address|port|process`.
pub fn parse_windows_listening(out: &str) -> Vec<ListenPort> {
    let mut v = Vec::new();
    for line in out.lines() {
        let mut f = line.trim().splitn(3, '|');
        let (Some(host), Some(port)) = (f.next(), f.next()) else { continue };
        let Ok(port) = port.trim().parse::<u16>() else { continue };
        push(&mut v, port, host.trim().to_string(), f.next().unwrap_or("").trim().to_string());
    }
    v.sort_by_key(|p| p.port);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ss() {
        let out = "LISTEN 0 4096 0.0.0.0:22 0.0.0.0:*\n\
                   LISTEN 0 5 127.0.0.1:5901 0.0.0.0:* users:((\"Xtigervnc\",pid=1,fd=9))\n\
                   LISTEN 0 5 [::1]:5901 [::]:* users:((\"Xtigervnc\",pid=1,fd=10))\n\
                   LISTEN 0 32 192.168.122.1:53 0.0.0.0:*\n\
                   LISTEN 0 511 *:3000 *:* users:((\"node\",pid=2,fd=20))\n\
                   LISTEN 0 128 192.168.122.1:8080 0.0.0.0:*\n";
        let p = parse_ss(out);
        assert_eq!(
            p,
            vec![
                ListenPort { port: 3000, host: "localhost".into(), process: "node".into() },
                ListenPort { port: 5901, host: "localhost".into(), process: "Xtigervnc".into() },
                ListenPort { port: 8080, host: "192.168.122.1".into(), process: "".into() },
            ]
        );
    }

    #[test]
    fn windows() {
        let out = "0.0.0.0|135|svchost\r\n::|5173|node\r\n127.0.0.1|5173|node\r\n0.0.0.0|49664|lsass\r\n0.0.0.0|5900|tvnserver\r\n";
        let p = parse_windows_listening(out);
        assert_eq!(p.iter().map(|x| x.port).collect::<Vec<_>>(), vec![5173, 5900]);
        assert_eq!(p[0].process, "node");
    }
}
