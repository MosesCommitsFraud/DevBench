//! Files on this PC and on machines or guests (over SFTP): list, create folders, rename,
//! delete, and copy or move between any two places with progress.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result, anyhow, bail};
use russh_sftp::client::SftpSession;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Where files live: this PC, or a machine (optionally a guest VM on it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Location {
    Local,
    Remote { machine: String, vm: Option<String> },
}

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    /// Seconds since 1970, 0 if unknown.
    pub modified: u64,
}

#[derive(Debug, Serialize)]
pub struct Listing {
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<Entry>,
    /// Places to jump to: drives and the home folder locally, the home folder and "/" remotely.
    pub places: Vec<String>,
}

/// One side of a file operation.
pub enum Fs {
    Local,
    Remote(Arc<SftpSession>),
}

type Reader = Box<dyn AsyncRead + Unpin + Send>;
type Writer = Box<dyn AsyncWrite + Unpin + Send>;

fn sftp_err(e: russh_sftp::client::error::Error) -> anyhow::Error {
    anyhow!("{e}")
}

pub fn remote_join(dir: &str, name: &str) -> String {
    if dir.ends_with('/') { format!("{dir}{name}") } else { format!("{dir}/{name}") }
}

fn remote_parent(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    match trimmed.rfind('/') {
        Some(0) => Some("/".into()),
        Some(i) => Some(trimmed[..i].to_string()),
        None => None,
    }
}

fn remote_name(path: &str) -> String {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or(path).to_string()
}

fn local_places() -> Vec<String> {
    let mut v = Vec::new();
    if let Some(h) = dirs::home_dir() {
        v.push(h.display().to_string());
    }
    #[cfg(windows)]
    for c in b'A'..=b'Z' {
        let d = format!("{}:\\", c as char);
        if Path::new(&d).exists() {
            v.push(d);
        }
    }
    #[cfg(not(windows))]
    v.push("/".into());
    v
}

impl Fs {
    /// The starting folder: the user's home.
    pub async fn home(&self) -> Result<String> {
        match self {
            Fs::Local => Ok(dirs::home_dir().context("no home folder")?.display().to_string()),
            Fs::Remote(s) => s.canonicalize(".").await.map_err(sftp_err),
        }
    }

    pub async fn list(&self, path: &str) -> Result<Listing> {
        match self {
            Fs::Local => {
                let p = PathBuf::from(path);
                let mut entries = Vec::new();
                let mut rd = tokio::fs::read_dir(&p).await.with_context(|| format!("opening {path}"))?;
                while let Some(e) = rd.next_entry().await? {
                    let Ok(md) = e.metadata().await else { continue };
                    entries.push(Entry {
                        name: e.file_name().to_string_lossy().into_owned(),
                        path: e.path().display().to_string(),
                        is_dir: md.is_dir(),
                        size: if md.is_dir() { 0 } else { md.len() },
                        modified: md
                            .modified()
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs())
                            .unwrap_or(0),
                    });
                }
                sort(&mut entries);
                Ok(Listing {
                    path: p.display().to_string(),
                    parent: p.parent().map(|x| x.display().to_string()),
                    entries,
                    places: local_places(),
                })
            }
            Fs::Remote(s) => {
                let path = s.canonicalize(path).await.map_err(sftp_err).with_context(|| format!("opening {path}"))?;
                let mut entries = Vec::new();
                for e in s.read_dir(&path).await.map_err(sftp_err).with_context(|| format!("listing {path}"))? {
                    let name = e.file_name();
                    if name == "." || name == ".." {
                        continue;
                    }
                    let md = e.metadata();
                    let full = remote_join(&path, &name);
                    // Follow symlinks so linked folders open like folders.
                    let is_dir = if md.is_symlink() {
                        s.metadata(&full).await.map(|m| m.is_dir()).unwrap_or(false)
                    } else {
                        md.is_dir()
                    };
                    entries.push(Entry {
                        name,
                        path: full,
                        is_dir,
                        size: if is_dir { 0 } else { md.len() },
                        modified: md.mtime.unwrap_or(0) as u64,
                    });
                }
                sort(&mut entries);
                let home = s.canonicalize(".").await.unwrap_or_else(|_| "/".into());
                let mut places = vec![home];
                if places[0] != "/" {
                    places.push("/".into());
                }
                Ok(Listing { parent: remote_parent(&path), path, entries, places })
            }
        }
    }

    pub fn join(&self, dir: &str, name: &str) -> String {
        match self {
            Fs::Local => Path::new(dir).join(name).display().to_string(),
            Fs::Remote(_) => remote_join(dir, name),
        }
    }

    pub fn name(&self, path: &str) -> String {
        match self {
            Fs::Local => Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            Fs::Remote(_) => remote_name(path),
        }
    }

    async fn is_dir(&self, path: &str) -> Result<bool> {
        match self {
            Fs::Local => Ok(tokio::fs::metadata(path).await?.is_dir()),
            Fs::Remote(s) => Ok(s.metadata(path).await.map_err(sftp_err)?.is_dir()),
        }
    }

    async fn size(&self, path: &str) -> Result<u64> {
        match self {
            Fs::Local => Ok(tokio::fs::metadata(path).await?.len()),
            Fs::Remote(s) => Ok(s.metadata(path).await.map_err(sftp_err)?.len()),
        }
    }

    async fn exists(&self, path: &str) -> bool {
        match self {
            Fs::Local => tokio::fs::try_exists(path).await.unwrap_or(false),
            Fs::Remote(s) => s.try_exists(path).await.unwrap_or(false),
        }
    }

    pub async fn mkdir(&self, path: &str) -> Result<()> {
        match self {
            Fs::Local => Ok(tokio::fs::create_dir(path).await.with_context(|| format!("creating {path}"))?),
            Fs::Remote(s) => s.create_dir(path).await.map_err(sftp_err).with_context(|| format!("creating {path}")),
        }
    }

    pub async fn rename(&self, from: &str, to: &str) -> Result<()> {
        match self {
            Fs::Local => Ok(tokio::fs::rename(from, to).await.with_context(|| format!("renaming {from}"))?),
            Fs::Remote(s) => s.rename(from, to).await.map_err(sftp_err).with_context(|| format!("renaming {from}")),
        }
    }

    /// Deletes a file, or a folder with everything in it.
    pub async fn remove(&self, path: &str) -> Result<()> {
        match self {
            Fs::Local => {
                if tokio::fs::symlink_metadata(path).await?.is_dir() {
                    tokio::fs::remove_dir_all(path).await
                } else {
                    tokio::fs::remove_file(path).await
                }
                .with_context(|| format!("deleting {path}"))
            }
            Fs::Remote(s) => {
                let md = s.symlink_metadata(path).await.map_err(sftp_err)?;
                if md.is_dir() {
                    let children: Vec<String> = s
                        .read_dir(path)
                        .await
                        .map_err(sftp_err)?
                        .map(|e| e.file_name())
                        .filter(|n| n != "." && n != "..")
                        .collect();
                    for c in children {
                        Box::pin(self.remove(&remote_join(path, &c))).await?;
                    }
                    s.remove_dir(path).await.map_err(sftp_err).with_context(|| format!("deleting {path}"))
                } else {
                    s.remove_file(path).await.map_err(sftp_err).with_context(|| format!("deleting {path}"))
                }
            }
        }
    }

    async fn reader(&self, path: &str) -> Result<Reader> {
        Ok(match self {
            Fs::Local => Box::new(tokio::fs::File::open(path).await.with_context(|| format!("opening {path}"))?),
            Fs::Remote(s) => Box::new(s.open(path).await.map_err(sftp_err).with_context(|| format!("opening {path}"))?),
        })
    }

    async fn writer(&self, path: &str) -> Result<Writer> {
        Ok(match self {
            Fs::Local => Box::new(tokio::fs::File::create(path).await.with_context(|| format!("creating {path}"))?),
            Fs::Remote(s) => Box::new(s.create(path).await.map_err(sftp_err).with_context(|| format!("creating {path}"))?),
        })
    }

    pub async fn read_all(&self, path: &str, limit: u64) -> Result<Vec<u8>> {
        let size = self.size(path).await?;
        if size > limit {
            bail!("{path} is {size} bytes, more than the limit of {limit}");
        }
        let mut r = self.reader(path).await?;
        let mut buf = Vec::with_capacity(size as usize);
        r.read_to_end(&mut buf).await?;
        Ok(buf)
    }

    pub async fn write_all(&self, path: &str, data: &[u8]) -> Result<()> {
        let mut w = self.writer(path).await?;
        w.write_all(data).await?;
        w.shutdown().await?;
        Ok(())
    }
}

fn sort(entries: &mut [Entry]) {
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
}

#[derive(Debug, Clone, Serialize)]
pub struct Progress {
    pub id: String,
    /// The file being copied right now.
    pub current: String,
    pub files_done: u64,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub state: &'static str,
    pub error: Option<String>,
}

pub struct Transfer<'a> {
    pub from: &'a Fs,
    pub to: &'a Fs,
    pub cancel: Arc<AtomicBool>,
    pub progress: Progress,
    pub report: &'a (dyn Fn(&Progress) + Send + Sync),
    last_report: std::time::Instant,
}

impl<'a> Transfer<'a> {
    pub fn new(id: String, from: &'a Fs, to: &'a Fs, cancel: Arc<AtomicBool>, report: &'a (dyn Fn(&Progress) + Send + Sync)) -> Self {
        Self {
            from,
            to,
            cancel,
            progress: Progress {
                id,
                current: String::new(),
                files_done: 0,
                bytes_done: 0,
                bytes_total: 0,
                state: "running",
                error: None,
            },
            report,
            last_report: std::time::Instant::now(),
        }
    }

    async fn total(&self, path: &str) -> Result<u64> {
        if self.from.is_dir(path).await? {
            let mut sum = 0;
            for e in self.from.list(path).await?.entries {
                sum += Box::pin(self.total(&e.path)).await?;
            }
            Ok(sum)
        } else {
            self.from.size(path).await
        }
    }

    /// Copies `sources` into the folder `dest_dir`. Existing files with the same name are replaced.
    pub async fn copy(&mut self, sources: &[String], dest_dir: &str) -> Result<()> {
        for s in sources {
            self.progress.bytes_total += self.total(s).await?;
        }
        (self.report)(&self.progress);
        for s in sources {
            let target = self.to.join(dest_dir, &self.from.name(s));
            if s == &target && std::ptr::eq(self.from, self.to) {
                bail!("{s} is already in that folder");
            }
            self.copy_one(s, &target).await?;
        }
        Ok(())
    }

    async fn copy_one(&mut self, src: &str, dst: &str) -> Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            bail!("cancelled");
        }
        if self.from.is_dir(src).await? {
            if !self.to.exists(dst).await {
                self.to.mkdir(dst).await?;
            }
            for e in self.from.list(src).await?.entries {
                let child = self.to.join(dst, &e.name);
                Box::pin(self.copy_one(&e.path, &child)).await?;
            }
            return Ok(());
        }
        self.progress.current = self.from.name(src);
        let mut r = self.from.reader(src).await?;
        let mut w = self.to.writer(dst).await?;
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                drop(w);
                let _ = self.to.remove(dst).await;
                bail!("cancelled");
            }
            let n = r.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            w.write_all(&buf[..n]).await?;
            self.progress.bytes_done += n as u64;
            if self.last_report.elapsed().as_millis() > 150 {
                self.last_report = std::time::Instant::now();
                (self.report)(&self.progress);
            }
        }
        w.shutdown().await?;
        self.progress.files_done += 1;
        (self.report)(&self.progress);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_paths() {
        assert_eq!(remote_join("/home/u", "a"), "/home/u/a");
        assert_eq!(remote_join("/", "a"), "/a");
        assert_eq!(remote_parent("/home/u"), Some("/home".into()));
        assert_eq!(remote_parent("/home"), Some("/".into()));
        assert_eq!(remote_parent("/"), None);
        assert_eq!(remote_parent("/C:/Users"), Some("/C:".into()));
        assert_eq!(remote_name("/home/u/file.txt"), "file.txt");
    }
}
