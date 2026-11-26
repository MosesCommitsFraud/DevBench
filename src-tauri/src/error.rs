//! Errors as the frontend sees them: a plain message, or a host key that needs the user's decision.

use serde::Serialize;

/// A server's host key that is not trusted yet or has changed. Travels inside `anyhow::Error`
/// so the command layer can turn it into a structured error.
#[derive(Debug, Clone)]
pub struct HostKeyProblem {
    pub host: String,
    pub port: u16,
    pub fingerprint: String,
    /// The key in OpenSSH format (`ssh-ed25519 AAAA...`).
    pub key: String,
    pub changed: bool,
}

impl std::fmt::Display for HostKeyProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.changed {
            write!(
                f,
                "The host key of {}:{} has CHANGED ({}). This can mean an attack, or that the server was reinstalled. \
                 Remove the old entry from known_hosts if the change is expected.",
                self.host, self.port, self.fingerprint
            )
        } else {
            write!(f, "Unknown host key for {}:{} ({})", self.host, self.port, self.fingerprint)
        }
    }
}

impl std::error::Error for HostKeyProblem {}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppError {
    Error {
        message: String,
    },
    HostKeyUnknown {
        message: String,
        host: String,
        port: u16,
        fingerprint: String,
        key: String,
    },
    HostKeyChanged {
        message: String,
        host: String,
        port: u16,
        fingerprint: String,
    },
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        if let Some(p) = e.chain().find_map(|c| c.downcast_ref::<HostKeyProblem>()) {
            let message = p.to_string();
            return if p.changed {
                AppError::HostKeyChanged { message, host: p.host.clone(), port: p.port, fingerprint: p.fingerprint.clone() }
            } else {
                AppError::HostKeyUnknown {
                    message,
                    host: p.host.clone(),
                    port: p.port,
                    fingerprint: p.fingerprint.clone(),
                    key: p.key.clone(),
                }
            };
        }
        // "context: cause: cause" in one line.
        AppError::Error { message: format!("{e:#}") }
    }
}

pub type CmdResult<T> = Result<T, AppError>;
