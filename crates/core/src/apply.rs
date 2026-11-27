//! Copying a plan: each file copied, fsynced, verified and logged (ADR 0003).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::i18n::IoReason;
use crate::log::{Event, LayoutRecord, LogLine, append};
use crate::plan::{Entry, Plan, Request};

/// What happened to each file.
#[derive(Debug, Default)]
pub struct Outcome {
    pub copied: Vec<Entry>,
    /// The entry and why its copy failed.
    pub failed: Vec<(Entry, CopyFailure)>,
    /// Every `exportpro.log` written, absolute.
    pub logs: Vec<PathBuf>,
}

/// Why one file of a copy failed; `i18n::Lang::copy_failure` words it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyFailure {
    /// Copying or verifying the file failed (the partial copy is removed).
    Copy(IoReason),
    /// The copy is there but the log couldn't record it, so erase and undo can't use it.
    Log { log: PathBuf, error: IoReason },
    /// An empty folder for edited files couldn't be created.
    Folder(IoReason),
    /// Not tried: the destination filled up on an earlier file.
    NotCopiedDiskFull,
}

/// Copy every entry of `plan` into `req.dest`, logging each one in its shoot folder's
/// `exportpro.log`, then create the empty folders for edited files. Originals are never
/// touched; a failed copy is removed, reported and logged, and the others go on.
pub fn apply(plan: &Plan, req: &Request) -> Outcome {
    apply_with_progress(plan, req, |_, _| {})
}

/// `apply_with_progress` with the copy function given: the seam tests use to fill the
/// disk.
pub fn apply_using(
    plan: &Plan,
    req: &Request,
    mut copy: impl FnMut(&Path, &Path) -> io::Result<(u64, String)>,
    mut progress: impl FnMut(usize, usize),
) -> Outcome {
    let layout = LayoutRecord::from(&req.layout);
    let mut outcome = Outcome::default();
    let total = plan.entries.len();
    // Once the destination is full, trying the other files only wastes time.
    let mut full = false;
    for (i, e) in plan.entries.iter().enumerate() {
        if full {
            outcome
                .failed
                .push((e.clone(), CopyFailure::NotCopiedDiskFull));
            progress(i + 1, total);
            continue;
        }
        let log = req.dest.join(&e.shoot).join(crate::log::LOG_NAME);
        let dest = req.dest.join(&e.dest);
        let (line, copy_error) = match copy(&e.source, &dest) {
            Ok((size, sha256)) => {
                strip_extended_attributes(&dest);
                let line = LogLine::new(Event::Copied, e, &layout).with_hash(size, sha256);
                (line, None)
            }
            Err(err) => {
                full = is_full(&err);
                let line = LogLine::new(Event::Failed, e, &layout).with_error(err.to_string());
                (line, Some(CopyFailure::Copy(IoReason::from(&err))))
            }
        };
        match (append(&log, &line), copy_error) {
            // The copy's own error matters most (the log often fails for the same reason).
            (_, Some(err)) => outcome.failed.push((e.clone(), err)),
            // A copy the log doesn't know about can't be erased or undone: a failure.
            (Err(err), None) => {
                full = is_full(&err);
                outcome.failed.push((
                    e.clone(),
                    CopyFailure::Log {
                        log: log.clone(),
                        error: IoReason::from(&err),
                    },
                ));
            }
            (Ok(()), None) => outcome.copied.push(e.clone()),
        }
        if log.exists() && !outcome.logs.contains(&log) {
            outcome.logs.push(log);
        }
        progress(i + 1, total);
    }
    for dir in &plan.edited {
        let failure = if full {
            Some(CopyFailure::NotCopiedDiskFull)
        } else {
            fs::create_dir_all(req.dest.join(dir))
                .err()
                .map(|err| CopyFailure::Folder(IoReason::from(&err)))
        };
        if let Some(failure) = failure {
            let e = Entry {
                name: dir.display().to_string(),
                source: PathBuf::new(),
                dest: dir.clone(),
                shoot: PathBuf::new(),
                file: PathBuf::new(),
                date_unreadable: false,
            };
            outcome.failed.push((e, failure));
        }
    }
    outcome
}

/// The destination has no room left: disk full or the user's quota reached.
fn is_full(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded
    )
}

/// `apply`, calling `progress(done, total)` after each file.
pub fn apply_with_progress(
    plan: &Plan,
    req: &Request,
    progress: impl FnMut(usize, usize),
) -> Outcome {
    apply_using(plan, req, copy_verified, progress)
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
