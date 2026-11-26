//! Turns the app's machine settings plus `~/.ssh/config` into concrete SSH endpoints.

use std::path::PathBuf;

use anyhow::{Result, bail};
use serde::Serialize;
use ssh2_config::{ParseRule, SshConfig};

use crate::config::{Machine, Vm};

/// Where and how to connect, including the hop(s) in front of it.
#[derive(Debug, Clone)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
    pub user: String,
    /// Private key files to try, in order. An SSH agent is tried after them.
    pub keys: Vec<PathBuf>,
    pub jump: Option<Box<Endpoint>>,
}

impl Endpoint {
    /// Identifies a connection for reuse: same target through the same hops.
    pub fn cache_key(&self) -> String {
        let mut k = format!("{}@{}:{}", self.user, self.host, self.port);
        if let Some(j) = &self.jump {
            k.push_str(" via ");
            k.push_str(&j.cache_key());
        }
        k
    }
}

fn load_ssh_config() -> Option<SshConfig> {
    let path = dirs::home_dir()?.join(".ssh").join("config");
    if !path.exists() {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    let mut reader = std::io::BufReader::new(file);
    SshConfig::default()
        .parse(&mut reader, ParseRule::ALLOW_UNKNOWN_FIELDS | ParseRule::ALLOW_UNSUPPORTED_FIELDS)
        .ok()
}

pub fn expand_home(p: &str) -> PathBuf {
    let p = p.trim();
    if let Some(rest) = p.strip_prefix("~/").or_else(|| p.strip_prefix("~\\")) {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(p)
}

fn default_keys() -> Vec<PathBuf> {
    let Some(ssh) = dirs::home_dir().map(|h| h.join(".ssh")) else { return Vec::new() };
    ["id_ed25519", "id_ecdsa", "id_rsa"].iter().map(|n| ssh.join(n)).collect()
}

fn local_user() -> String {
    std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_else(|_| "root".into())
}

/// Splits `[user@]host[:port]` (also `[user@][v6addr]:port`).
fn parse_spec(spec: &str) -> (Option<String>, String, Option<u16>) {
    let spec = spec.trim().trim_start_matches("ssh://");
    let (user, rest) = match spec.rsplit_once('@') {
        Some((u, r)) => (Some(u.to_string()), r),
        None => (None, spec),
    };
    if let Some(inner) = rest.strip_prefix('[') {
        if let Some((addr, after)) = inner.split_once(']') {
            let port = after.strip_prefix(':').and_then(|p| p.parse().ok());
            return (user, addr.to_string(), port);
        }
    }
    match rest.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') => match p.parse() {
            Ok(port) => (user, h.to_string(), Some(port)),
            Err(_) => (user, rest.to_string(), None),
        },
        _ => (user, rest.to_string(), None),
    }
}

struct Overrides<'a> {
    port: u16,
    user: &'a str,
    key: &'a str,
    jump: &'a str,
}

fn resolve(cfg: &Option<SshConfig>, alias: &str, o: Overrides, depth: usize) -> Result<Endpoint> {
    if depth > 6 {
        bail!("jump host chain too long (loop in ProxyJump?)");
    }
    let params = cfg.as_ref().map(|c| c.query(alias));
    let host = params.as_ref().and_then(|p| p.host_name.clone()).unwrap_or_else(|| alias.to_string());
    let port = if o.port != 0 { o.port } else { params.as_ref().and_then(|p| p.port).unwrap_or(22) };
    let user = if !o.user.trim().is_empty() {
        o.user.trim().to_string()
    } else {
        params.as_ref().and_then(|p| p.user.clone()).unwrap_or_else(local_user)
    };
    let mut keys: Vec<PathBuf> = if !o.key.trim().is_empty() {
        vec![expand_home(o.key)]
    } else {
        params
            .as_ref()
            .and_then(|p| p.identity_file.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|p| expand_home(&p.to_string_lossy()))
            .collect()
    };
    for k in default_keys() {
        if !keys.contains(&k) {
            keys.push(k);
        }
    }

    // ProxyJump: the app's field wins over ~/.ssh/config; "none" switches it off.
    let chain: Vec<String> = if !o.jump.trim().is_empty() {
        o.jump.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    } else {
        params.as_ref().and_then(|p| p.proxy_jump.clone()).unwrap_or_default()
    };
    let chain: Vec<String> = chain.into_iter().filter(|s| !s.eq_ignore_ascii_case("none")).collect();

    let mut jump: Option<Box<Endpoint>> = None;
    for (i, spec) in chain.iter().enumerate() {
        let (u, h, p) = parse_spec(spec);
        let mut hop = resolve(
            cfg,
            &h,
            Overrides {
                port: p.unwrap_or(0),
                user: u.as_deref().unwrap_or(""),
                key: "",
                // The first hop may have its own ProxyJump; later hops go through the previous one.
                jump: if i == 0 { "" } else { "none" },
            },
            depth + 1,
        )?;
        if i > 0 {
            hop.jump = jump.take();
        }
        jump = Some(Box::new(hop));
    }

    Ok(Endpoint { host, port, user, keys, jump })
}

pub fn machine_endpoint(m: &Machine) -> Result<Endpoint> {
    resolve(
        &load_ssh_config(),
        m.ssh_host.trim(),
        Overrides { port: m.ssh_port, user: &m.ssh_user, key: &m.ssh_key, jump: &m.jump_host },
        0,
    )
}

/// The guest as seen from its host machine; the machine is the jump host.
pub fn guest_endpoint(machine: &Endpoint, vm: &Vm, guest_address: &str) -> Endpoint {
    let mut keys = Vec::new();
    if !vm.guest_ssh_key.trim().is_empty() {
        keys.push(expand_home(&vm.guest_ssh_key));
    }
    for k in &machine.keys {
        if !keys.contains(k) {
            keys.push(k.clone());
        }
    }
    Endpoint {
        host: guest_address.to_string(),
        port: vm.guest_ssh_port,
        user: vm.guest_ssh_user.trim().to_string(),
        keys,
        jump: Some(Box::new(machine.clone())),
    }
}

#[derive(Debug, Serialize)]
pub struct SshHostEntry {
    pub alias: String,
    pub host_name: String,
    pub user: String,
    pub port: u16,
    pub proxy_jump: String,
}

/// Concrete `Host` aliases from `~/.ssh/config` (no wildcards), for the import list.
pub fn list_hosts() -> Vec<SshHostEntry> {
    let Some(cfg) = load_ssh_config() else { return Vec::new() };
    let mut out: Vec<SshHostEntry> = Vec::new();
    for h in cfg.get_hosts() {
        for clause in &h.pattern {
            let a = &clause.pattern;
            if clause.negated || a.contains(['*', '?', '!']) || out.iter().any(|e| &e.alias == a) {
                continue;
            }
            let p = cfg.query(a);
            out.push(SshHostEntry {
                alias: a.clone(),
                host_name: p.host_name.clone().unwrap_or_else(|| a.clone()),
                user: p.user.clone().unwrap_or_default(),
                port: p.port.unwrap_or(22),
                proxy_jump: p.proxy_jump.clone().unwrap_or_default().join(","),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::parse_spec;

    #[test]
    fn specs() {
        assert_eq!(parse_spec("host"), (None, "host".into(), None));
        assert_eq!(parse_spec("u@host:2222"), (Some("u".into()), "host".into(), Some(2222)));
        assert_eq!(parse_spec("[fe80::1]:22"), (None, "fe80::1".into(), Some(22)));
        assert_eq!(parse_spec("fe80::1"), (None, "fe80::1".into(), None));
    }
}
