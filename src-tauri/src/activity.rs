//! A log of what AI agents did through DevBench, one JSON object per line in
//! `<config dir>/activity.log`. The MCP server writes it, the GUI shows it.

use std::io::Write;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    pub time: String,
    pub tool: String,
    pub machine: String,
    pub vm: String,
    pub summary: String,
    pub ok: bool,
    pub error: String,
}

/// Keeps the log from growing without bound: past this size the older half is dropped.
const MAX_BYTES: u64 = 2 * 1024 * 1024;

fn path() -> Result<std::path::PathBuf> {
    Ok(crate::config::config_dir()?.join("activity.log"))
}

pub fn append(entry: &Entry) -> Result<()> {
    let path = path()?;
    std::fs::create_dir_all(path.parent().expect("has a parent"))?;
    if std::fs::metadata(&path).map(|m| m.len() > MAX_BYTES).unwrap_or(false) {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        std::fs::write(&path, lines[lines.len() / 2..].join("\n") + "\n")?;
    }
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(f, "{}", serde_json::to_string(entry)?)?;
    Ok(())
}

/// The most recent entries, newest first.
pub fn recent(limit: usize) -> Vec<Entry> {
    let Ok(path) = path() else { return Vec::new() };
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.lines().rev().filter_map(|l| serde_json::from_str(l).ok()).take(limit).collect()
}

/// Current time as RFC 3339 UTC, without pulling in a date crate.
pub fn now() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (days, rem) = (secs / 86400, secs % 86400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

#[cfg(test)]
mod tests {
    #[test]
    fn time_format() {
        let s = super::now();
        assert_eq!(s.len(), 20);
        assert!(s.starts_with("20"));
    }
}
