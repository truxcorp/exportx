//! Copying a plan: each file copied, fsynced, verified and logged (ADR 0003).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::log::{Event, LayoutRecord, LogLine, append};
use crate::plan::{Entry, Plan, Request};

/// What happened to each file.
#[derive(Debug, Default)]
pub struct Outcome {
    pub copied: Vec<Entry>,
    /// The entry and why its copy failed.
    pub failed: Vec<(Entry, String)>,
    /// Every `exportpro.log` written, absolute.
    pub logs: Vec<PathBuf>,
}

/// Copy every entry of `plan` into `req.dest`, logging each one in its shoot folder's
/// `exportpro.log`, then create the empty folders for edited files. Originals are never
/// touched; a failed copy is removed, reported and logged, and the others go on.
pub fn apply(plan: &Plan, req: &Request) -> Outcome {
    apply_with_progress(plan, req, |_, _| {})
}

/// `apply`, calling `progress(done, total)` after each file.
pub fn apply_with_progress(
    plan: &Plan,
    req: &Request,
    mut progress: impl FnMut(usize, usize),
) -> Outcome {
    let layout = LayoutRecord::from(&req.layout);
    let mut outcome = Outcome::default();
    let total = plan.entries.len();
    for (i, e) in plan.entries.iter().enumerate() {
        let log = req.dest.join(&e.shoot).join(crate::log::LOG_NAME);
        let dest = req.dest.join(&e.dest);
        let (line, copy_error) = match copy_verified(&e.source, &dest) {
            Ok((size, sha256)) => {
                strip_extended_attributes(&dest);
                let line = LogLine::new(Event::Copied, e, &layout).with_hash(size, sha256);
                (line, None)
            }
            Err(err) => {
                let line = LogLine::new(Event::Failed, e, &layout).with_error(err.to_string());
                (line, Some(err.to_string()))
            }
        };
        match (append(&log, &line), copy_error) {
            // A copy the log doesn't know about can't be erased or undone: a failure.
            (Err(err), _) => outcome
                .failed
                .push((e.clone(), format!("cannot write {}: {err}", log.display()))),
            (Ok(()), Some(err)) => outcome.failed.push((e.clone(), err)),
            (Ok(()), None) => outcome.copied.push(e.clone()),
        }
        if log.exists() && !outcome.logs.contains(&log) {
            outcome.logs.push(log);
        }
        progress(i + 1, total);
    }
    for dir in &plan.edited {
        if let Err(err) = fs::create_dir_all(req.dest.join(dir)) {
            let e = Entry {
                name: dir.display().to_string(),
                source: PathBuf::new(),
                dest: dir.clone(),
                shoot: PathBuf::new(),
                file: PathBuf::new(),
            };
            outcome.failed.push((e, err.to_string()));
        }
    }
    outcome
}

/// Copy `src` to a new file `dst` (never overwriting) with the source's file times, fsync
/// it, then read it back and compare size and SHA-256 with the source. Returns both. On
/// any error the partial copy is removed. The read-back can come from the OS cache: it
/// checks the copy path, not the medium.
pub fn copy_verified(src: &Path, dst: &Path) -> io::Result<(u64, String)> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut input = File::open(src)?;
    let mut output = OpenOptions::new().write(true).create_new(true).open(dst)?;
    let result = (|| {
        let (size, sha256) = hash_while(&mut input, |chunk| output.write_all(chunk))?;
        let meta = input.metadata()?;
        let mut times = fs::FileTimes::new().set_modified(meta.modified()?);
        if let Ok(accessed) = meta.accessed() {
            times = times.set_accessed(accessed);
        }
        output.set_times(times)?;
        output.sync_all()?;
        let (copy_size, copy_sha256) = hash_file(dst)?;
        if copy_size != size || copy_sha256 != sha256 {
            return Err(io::Error::other("the copy differs from the original"));
        }
        sync_dir(dst.parent().unwrap_or(Path::new(".")));
        Ok((size, sha256))
    })();
    if result.is_err() {
        drop(output);
        let _ = fs::remove_file(dst);
    }
    result
}

/// Size and SHA-256 (lower-case hex) of a file.
pub fn hash_file(path: &Path) -> io::Result<(u64, String)> {
    hash_while(&mut File::open(path)?, |_| Ok(()))
}

fn hash_while(
    input: &mut impl Read,
    mut each: impl FnMut(&[u8]) -> io::Result<()>,
) -> io::Result<(u64, String)> {
    let mut hasher = Sha256::new();
    let mut buf = vec![0; 1 << 20];
    let mut size = 0;
    loop {
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        each(&buf[..n])?;
        size += n as u64;
    }
    let hex = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok((size, hex))
}

/// Make the new directory entry durable (Unix; Windows can't open a directory).
pub(crate) fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(d) = File::open(dir) {
        let _ = d.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// macOS: remove extended attributes from the copy (v0.1.2's "xattr 164" fix: some
/// attributes made copied files unusable).
fn strip_extended_attributes(path: &Path) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("/usr/bin/xattr")
        .arg("-c")
        .arg(path)
        .status();
    #[cfg(not(target_os = "macos"))]
    let _ = path;
}
