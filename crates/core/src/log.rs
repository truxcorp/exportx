//! `exportx.log` (`exportpro.log` in shoots ExportPro copied): one JSON line per file, in
//! each shoot folder. It is the contract erase
//! and undo rely on (ADR 0003), so it records the layout used and never depends on the
//! current settings.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::layout::Layout;
use crate::plan::Entry;

pub const LOG_NAME: &str = "exportx.log";
/// The log's name before the app was renamed ExportX.
pub const OLD_LOG_NAME: &str = "exportpro.log";

/// The log of `shoot`: its `exportpro.log` if ExportPro started one there (kept, so a
/// shoot never has two logs), else `exportx.log`.
pub fn path_in(shoot: &Path) -> PathBuf {
    let old = shoot.join(OLD_LOG_NAME);
    if fs::symlink_metadata(&old).is_ok() {
        old
    } else {
        shoot.join(LOG_NAME)
    }
}
/// Bumped when a line's meaning changes.
pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Event {
    /// Copied and verified: `size` and `sha256` describe both files.
    Copied,
    /// The copy failed: `error` says why.
    Failed,
    /// The original was moved to the Trash after its copy was verified again.
    Erased,
    /// The original was copied back from its verified copy.
    Restored,
    /// Moved into the tree instead of copied (same disk, ADR 0006); logged before the
    /// move. `size` is the file's.
    Moved,
    /// Moved back to its original name by undo.
    #[serde(rename = "moved-back")]
    MovedBack,
}

/// The templates and folder names a copy was made with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutRecord {
    pub shoot: String,
    pub original: String,
    pub photo: String,
    pub video: String,
}

impl From<&Layout> for LayoutRecord {
    fn from(l: &Layout) -> Self {
        LayoutRecord {
            shoot: l.shoot.to_string(),
            original: l.original.to_string(),
            photo: l.photo.clone(),
            video: l.video.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogLine {
    pub v: u32,
    pub event: Event,
    /// Local time, RFC 3339.
    pub time: String,
    /// The original, absolute.
    pub source: PathBuf,
    /// The copy, relative to the shoot folder (the log's folder).
    pub dest: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<String>,
    pub layout: LayoutRecord,
    /// The copy or move this line belongs to: undoing a move takes back that one only.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub run: Option<String>,
}

impl LogLine {
    pub fn new(event: Event, e: &Entry, layout: &LayoutRecord) -> Self {
        LogLine {
            v: FORMAT_VERSION,
            event,
            time: chrono::Local::now().to_rfc3339(),
            source: e.source.clone(),
            dest: e.file.clone(),
            size: None,
            sha256: None,
            error: None,
            layout: layout.clone(),
            run: None,
        }
    }

    pub fn in_run(mut self, run: &str) -> Self {
        self.run = Some(run.to_owned());
        self
    }

    pub fn with_hash(mut self, size: u64, sha256: String) -> Self {
        self.size = Some(size);
        self.sha256 = Some(sha256);
        self
    }

    pub fn with_size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    pub fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }

    /// The same file, a new event, now.
    pub fn follow_up(&self, event: Event) -> Self {
        LogLine {
            event,
            time: chrono::Local::now().to_rfc3339(),
            error: None,
            ..self.clone()
        }
    }
}

/// Why a log line can't be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineProblem {
    /// Not a line this version can parse (e.g. torn by a crash): the parser's message.
    Unreadable(String),
    /// Written by a newer ExportPro, in this format version.
    Newer(u64),
}

/// A log line's number (from 1) and why it can't be used.
pub type LineIssue = (usize, LineProblem);

/// The lines of the log at `path` that this version can use, in order, and the number
/// (from 1) and problem of each line it can't.
pub fn read(path: &Path) -> io::Result<(Vec<LogLine>, Vec<LineIssue>)> {
    let mut lines = Vec::new();
    let mut problems = Vec::new();
    for (i, text) in fs::read_to_string(path)?.lines().enumerate() {
        if text.trim().is_empty() {
            continue;
        }
        let parsed = serde_json::from_str::<serde_json::Value>(text).and_then(|value| {
            let v = value.get("v").and_then(serde_json::Value::as_u64);
            if v.is_some_and(|v| v > u64::from(FORMAT_VERSION)) {
                return Ok(Err(LineProblem::Newer(v.unwrap_or_default())));
            }
            serde_json::from_value::<LogLine>(value).map(Ok)
        });
        match parsed {
            Ok(Ok(line)) => lines.push(line),
            Ok(Err(problem)) => problems.push((i + 1, problem)),
            Err(e) => problems.push((i + 1, LineProblem::Unreadable(e.to_string()))),
        }
    }
    Ok((lines, problems))
}

/// Append one line to the log at `path`, creating it (and its folder) if needed.
pub fn append(path: &Path, line: &LogLine) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    let mut text = serde_json::to_string(line).map_err(io::Error::other)?;
    text.push('\n');
    f.write_all(text.as_bytes())?;
    f.sync_data()?;
    if let Some(parent) = path.parent() {
        crate::apply::sync_dir(parent);
    }
    Ok(())
}
