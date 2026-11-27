//! Scanning a source folder: top-level files, grouped by base name.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Local, NaiveDateTime};

use crate::dates::capture_date;

/// Lower-case extensions treated as video (the bash version's VIDEO_EXTS).
pub const VIDEO_EXTS: &[&str] = &[
    "mov", "mp4", "m4v", "avi", "mts", "m2ts", "mpg", "mpeg", "3gp", "mkv", "wmv", "mxf", "crm",
    "braw", "r3d", "lrv",
];

/// Photo or video: picks the `{kind}` folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Photo,
    Video,
}

/// One file of the source folder.
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// File name as found (`IMG_0001.CR2`).
    pub name: String,
    pub path: PathBuf,
    /// Lower-case extension (`cr2`).
    pub ext: String,
    /// Capture date (stored in the file, else the file time), local time.
    pub date: NaiveDateTime,
}

/// Files sharing a base name (`IMG_1.CR2` + `IMG_1.JPG`): they get one number.
#[derive(Debug, Clone)]
pub struct Group {
    /// Lower-case base name.
    pub key: String,
    pub kind: Kind,
    /// Sorted by name.
    pub files: Vec<SourceFile>,
}

impl Group {
    /// The group's date: its earliest file's.
    pub fn date(&self) -> NaiveDateTime {
        self.files
            .iter()
            .map(|f| f.date)
            .min()
            .expect("a group has files")
    }
}

/// Top-level files of `dir` (paths made absolute), hidden files and files without a base name or extension
/// skipped, grouped by lower-case base name and sorted by date, then base name.
pub fn scan(dir: &Path) -> io::Result<Vec<Group>> {
    // Absolute, so the log names the originals wherever erase and undo run from.
    let dir = std::path::absolute(dir)?;
    let mut groups: Vec<Group> = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some((base, ext)) = split_name(&name) else {
            continue;
        };
        let file = SourceFile {
            path: entry.path(),
            ext: ext.to_lowercase(),
            date: match capture_date(&entry.path()) {
                Some(d) => d,
                None => file_date(&entry.path())?,
            },
            name: name.clone(),
        };
        let key = base.to_lowercase();
        match groups.iter_mut().find(|g| g.key == key) {
            Some(g) => g.files.push(file),
            None => groups.push(Group {
                key,
                kind: Kind::Photo,
                files: vec![file],
            }),
        }
    }
    for g in &mut groups {
        g.files.sort_by(|a, b| a.name.cmp(&b.name));
        if g.files.iter().any(|f| VIDEO_EXTS.contains(&f.ext.as_str())) {
            g.kind = Kind::Video;
        }
    }
    groups.sort_by(|a, b| a.date().cmp(&b.date()).then_with(|| a.key.cmp(&b.key)));
    Ok(groups)
}

/// `name.ext` → (`name`, `ext`); `None` for hidden files or a missing base or extension.
fn split_name(name: &str) -> Option<(&str, &str)> {
    if name.starts_with('.') {
        return None;
    }
    let (base, ext) = name.rsplit_once('.')?;
    (!base.is_empty() && !ext.is_empty()).then_some((base, ext))
}

/// The earliest of the file's creation and modification times, local time.
fn file_date(path: &Path) -> io::Result<NaiveDateTime> {
    let meta = fs::metadata(path)?;
    let modified = meta.modified()?;
    let earliest = match meta.created() {
        Ok(created) if created < modified => created,
        _ => modified,
    };
    Ok(local(earliest))
}

fn local(t: SystemTime) -> NaiveDateTime {
    DateTime::<Local>::from(t).naive_local()
}
