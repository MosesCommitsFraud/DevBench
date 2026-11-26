//! Running agents: every `devbench mcp` process keeps a small file in `<config dir>/agents/`
//! with a heartbeat, so the GUI can show which agents are connected right now.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// An entry older than this is a process that ended without cleaning up.
const STALE: Duration = Duration::from_secs(45);
pub const HEARTBEAT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub pid: u32,
    /// The MCP client's name, e.g. "claude-code".
    pub client: String,
    pub started: String,
    /// Unix seconds of the last heartbeat or tool call.
    pub seen: u64,
    pub last_tool: String,
    pub last_target: String,
    pub calls: u64,
}

fn dir() -> Option<PathBuf> {
    crate::config::config_dir().ok().map(|d| d.join("agents"))
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn write(agent: &mut Agent) {
    agent.seen = now();
    let Some(dir) = dir() else { return };
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(text) = serde_json::to_string(agent) {
        let _ = std::fs::write(dir.join(format!("{}.json", agent.pid)), text);
    }
}

pub fn remove(pid: u32) {
    if let Some(dir) = dir() {
        let _ = std::fs::remove_file(dir.join(format!("{pid}.json")));
    }
}

/// Agents with a recent heartbeat; stale files are deleted.
pub fn running() -> Vec<Agent> {
    let Some(dir) = dir() else { return Vec::new() };
    let Ok(rd) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let path = e.path();
        let agent: Option<Agent> = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok());
        match agent {
            Some(a) if now().saturating_sub(a.seen) <= STALE.as_secs() => out.push(a),
            _ => {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    out.sort_by_key(|a| a.pid);
    out
}
