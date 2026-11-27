//! SSH over russh: key or agent authentication, jump hosts, command execution and
//! port forwarding (direct-tcpip channels), all in-process. Connections are pooled and reused.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use russh::client::{self, Handle, Msg};
use russh::keys::{HashAlg, PrivateKeyWithHashAlg, PublicKey, PublicKeyOrCertificate};
use russh::{Channel, ChannelMsg};
use tokio::sync::Mutex;

use crate::error::HostKeyProblem;
use crate::sshconf::Endpoint;

const TCP_TIMEOUT: Duration = Duration::from_secs(5);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(20);

/// Checks server keys against `~/.ssh/known_hosts` (read only) and DevBench's own `known_hosts`.
pub struct Client {
    host: String,
    port: u16,
    known_hosts: Vec<PathBuf>,
    problem: Arc<StdMutex<Option<HostKeyProblem>>>,
}

impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(&mut self, server_key: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        let PublicKeyOrCertificate::PublicKey { key, .. } = server_key else {
            log::warn!("{}: host certificates are not supported", self.host);
            return Ok(false);
        };
        let mut changed = false;
        for file in &self.known_hosts {
            match russh::keys::check_known_hosts_path(&self.host, self.port, key, file) {
                Ok(true) => return Ok(true),
                Ok(false) => {}
                Err(russh::keys::Error::KeyChanged { .. }) => changed = true,
                Err(e) => log::warn!("reading {}: {e}", file.display()),
            }
        }
        *self.problem.lock().unwrap() = Some(HostKeyProblem {
            host: self.host.clone(),
            port: self.port,
            fingerprint: key.fingerprint(HashAlg::Sha256).to_string(),
            key: key.to_openssh().unwrap_or_default(),
            changed,
        });
        Ok(false)
    }
}

/// An authenticated connection. Holds its jump connection so the tunnel stays up.
pub struct Conn {
    pub handle: Handle<Client>,
    _via: Option<Arc<Conn>>,
}

pub struct ExecOutput {
    pub stdout: String,
    pub stderr: String,
    /// None when the connection ended without an exit status (e.g. the machine powered off).
    pub code: Option<u32>,
}

type Slot = Arc<Mutex<Option<Arc<Conn>>>>;

pub struct Pool {
    slots: StdMutex<HashMap<String, Slot>>,
    app_known_hosts: PathBuf,
}

impl Pool {
    pub fn new(app_known_hosts: PathBuf) -> Self {
        Self { slots: StdMutex::new(HashMap::new()), app_known_hosts }
    }

    /// Forget all pooled connections (after a config change). Tunnels in use keep theirs.
    pub fn clear(&self) {
        self.slots.lock().unwrap().clear();
    }

    /// Drops the pooled connection for this endpoint so the next use reconnects.
    pub fn evict(&self, ep: &Endpoint) {
        self.slots.lock().unwrap().remove(&ep.cache_key());
    }

    pub fn trust(&self, host: &str, port: u16, key: &str) -> Result<()> {
        let key = PublicKey::from_openssh(key.trim()).context("parsing the host key")?;
        russh::keys::known_hosts::learn_known_hosts_path(host, port, &key, &self.app_known_hosts)
            .with_context(|| format!("writing {}", self.app_known_hosts.display()))?;
        Ok(())
    }

    /// A live connection to `ep`, reusing a pooled one when it is still open.
    pub async fn get(&self, ep: &Endpoint) -> Result<Arc<Conn>> {
        let slot = {
            let mut slots = self.slots.lock().unwrap();
            slots.entry(ep.cache_key()).or_default().clone()
        };
        let mut guard = slot.lock().await;
        if let Some(c) = guard.as_ref() {
            if !c.handle.is_closed() {
                return Ok(c.clone());
            }
        }
        let conn = Arc::new(Box::pin(self.connect(ep)).await?);
        *guard = Some(conn.clone());
        Ok(conn)
    }

    async fn connect(&self, ep: &Endpoint) -> Result<Conn> {
        let config = Arc::new(client::Config {
            inactivity_timeout: None,
            keepalive_interval: Some(Duration::from_secs(15)),
            keepalive_max: 3,
            nodelay: true,
            ..Default::default()
        });
        let mut known_hosts = Vec::new();
        if let Some(home) = dirs::home_dir() {
            known_hosts.push(home.join(".ssh").join("known_hosts"));
        }
        known_hosts.push(self.app_known_hosts.clone());
        let problem = Arc::new(StdMutex::new(None));
        let handler = Client { host: ep.host.clone(), port: ep.port, known_hosts, problem: problem.clone() };

        let (handshake, via) = match &ep.jump {
            None => {
                let tcp = tokio::time::timeout(TCP_TIMEOUT, tokio::net::TcpStream::connect((ep.host.as_str(), ep.port)))
                    .await
                    .map_err(|_| anyhow!("{}:{} did not answer (timeout)", ep.host, ep.port))?
                    .with_context(|| format!("connecting to {}:{}", ep.host, ep.port))?;
                tcp.set_nodelay(true).ok();
                (tokio::time::timeout(HANDSHAKE_TIMEOUT, client::connect_stream(config, tcp, handler)).await, None)
            }
            Some(jump) => {
                let parent = self.get(jump).await.with_context(|| format!("jump host {}", jump.host))?;
                let ch = parent
                    .handle
                    .channel_open_direct_tcpip(ep.host.clone(), ep.port as u32, "127.0.0.1", 0)
                    .await
                    .with_context(|| format!("{} cannot reach {}:{}", jump.host, ep.host, ep.port))?;
                let stream = ch.into_stream();
                (tokio::time::timeout(HANDSHAKE_TIMEOUT, client::connect_stream(config, stream, handler)).await, Some(parent))
            }
        };
        let mut handle = match handshake {
            Err(_) => bail!("SSH handshake with {}:{} timed out", ep.host, ep.port),
            Ok(Err(e)) => {
                if let Some(p) = problem.lock().unwrap().take() {
                    return Err(p.into());
                }
                return Err(anyhow!(e).context(format!("SSH to {}:{}", ep.host, ep.port)));
            }
            Ok(Ok(h)) => h,
        };
        authenticate(&mut handle, ep).await?;
        Ok(Conn { handle, _via: via })
    }

    /// Runs a command; reconnects once if the pooled connection turned out to be dead.
    pub async fn exec(&self, ep: &Endpoint, command: &str, timeout: Duration) -> Result<ExecOutput> {
        let conn = self.get(ep).await?;
        let mut ch = match conn.handle.channel_open_session().await {
            Ok(ch) => ch,
            Err(_) => {
                self.evict(ep);
                self.get(ep).await?.handle.channel_open_session().await.context("opening an SSH session channel")?
            }
        };
        match tokio::time::timeout(timeout, run(&mut ch, command)).await {
            Ok(r) => r,
            Err(_) => {
                // Close the channel: dropping it leaves the session open on the server, where it
                // counts against sshd's MaxSessions (10) until every new command is refused.
                let _ = tokio::time::timeout(Duration::from_secs(2), ch.close()).await;
                Err(anyhow!("command timed out after {}s", timeout.as_secs()))
            }
        }
    }

    /// Opens a forwarded TCP connection from `ep` to `host:port` (like `ssh -L`). The returned
    /// connection must be kept alive as long as the channel is used.
    pub async fn forward(&self, ep: &Endpoint, host: &str, port: u16) -> Result<(Arc<Conn>, Channel<Msg>)> {
        let conn = self.get(ep).await?;
        match conn.handle.channel_open_direct_tcpip(host, port as u32, "127.0.0.1", 0).await {
            Ok(ch) => Ok((conn, ch)),
            Err(_) if conn.handle.is_closed() => {
                self.evict(ep);
                let conn = self.get(ep).await?;
                let ch = conn.handle.channel_open_direct_tcpip(host, port as u32, "127.0.0.1", 0).await?;
                Ok((conn, ch))
            }
            Err(e) => Err(anyhow!(e).context(format!("{} cannot open {host}:{port}", ep.host))),
        }
    }
}

/// A Linux shell script that the server itself stops after `secs` (killed 3 s later if it does
/// not stop), so a command that hangs there (apt-check, virsh on a busy libvirt) cannot pile up
/// when DevBench gives up waiting and asks again.
pub fn bounded(script: &str, secs: u64) -> String {
    let q = sh_quote(script);
    format!("if command -v timeout >/dev/null 2>&1; then exec timeout -k 3 {secs} sh -c {q}; else exec sh -c {q}; fi")
}

async fn run(ch: &mut Channel<Msg>, command: &str) -> Result<ExecOutput> {
    ch.exec(true, command).await?;
    let (mut out, mut err, mut code) = (Vec::new(), Vec::new(), None);
    while let Some(msg) = ch.wait().await {
        match msg {
            ChannelMsg::Data { data } => out.extend_from_slice(&data),
            ChannelMsg::ExtendedData { data, .. } => err.extend_from_slice(&data),
            ChannelMsg::ExitStatus { exit_status } => code = Some(exit_status),
            ChannelMsg::Close => break,
            _ => {}
        }
    }
    Ok(ExecOutput {
        stdout: String::from_utf8_lossy(&out).into_owned(),
        stderr: String::from_utf8_lossy(&err).into_owned(),
        code,
    })
}

async fn authenticate(handle: &mut Handle<Client>, ep: &Endpoint) -> Result<()> {
    let mut notes: Vec<String> = Vec::new();

    for path in &ep.keys {
        if !path.exists() {
            continue;
        }
        let key = match russh::keys::load_secret_key(path, None) {
            Ok(k) => k,
            Err(e) => {
                notes.push(format!("{}: {e} (passphrase-protected keys work through ssh-agent)", path.display()));
                continue;
            }
        };
        let hash = if key.algorithm().is_rsa() { handle_rsa_hash(handle).await } else { None };
        let res = handle
            .authenticate_publickey(ep.user.clone(), PrivateKeyWithHashAlg::new(Arc::new(key), hash))
            .await?;
        if res.success() {
            return Ok(());
        }
        notes.push(format!("{}: rejected by the server", path.display()));
    }

    match agent_auth(handle, &ep.user).await {
        Ok(true) => return Ok(()),
        Ok(false) => notes.push("ssh-agent: no accepted key".into()),
        Err(e) => notes.push(format!("ssh-agent: {e}")),
    }

    if notes.is_empty() {
        notes.push("no key file found".into());
    }
    bail!("SSH login as '{}' on {} failed: {}", ep.user, ep.host, notes.join("; "))
}

async fn handle_rsa_hash(handle: &Handle<Client>) -> Option<HashAlg> {
    handle.best_supported_rsa_hash().await.ok().flatten().flatten()
}

async fn agent_auth(handle: &mut Handle<Client>, user: &str) -> Result<bool> {
    #[cfg(windows)]
    let mut agent = russh::keys::agent::client::AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent")
        .await
        .map_err(|e| anyhow!("not running ({e})"))?
        .dynamic();
    #[cfg(unix)]
    let mut agent = russh::keys::agent::client::AgentClient::connect_env().await.map_err(|e| anyhow!("{e}"))?.dynamic();

    let ids = agent.request_identities().await?;
    for id in ids {
        let key = id.public_key().into_owned();
        let hash = if key.algorithm().is_rsa() { handle_rsa_hash(handle).await } else { None };
        match handle.authenticate_publickey_with(user, key, hash, &mut agent).await {
            Ok(r) if r.success() => return Ok(true),
            Ok(_) => {}
            Err(e) => log::debug!("agent key refused: {e:?}"),
        }
    }
    Ok(false)
}

/// A command line that runs a PowerShell script on Windows, whatever OpenSSH's default shell
/// there is (cmd or PowerShell): the script travels base64-encoded, so nothing needs quoting.
pub fn powershell(script: &str) -> String {
    use base64::Engine;
    let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    format!("powershell -NoProfile -NonInteractive -EncodedCommand {}", base64::engine::general_purpose::STANDARD.encode(utf16))
}

/// Quotes a value for a POSIX shell.
pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}
