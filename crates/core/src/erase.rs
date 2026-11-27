//! Erasing originals to the system Trash, and undoing that from the copies (ADR 0003).
//! Both read the shoot folder's log (`log::path_in`), never the current settings.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::apply::{copy_verified, hash_file};
use crate::i18n::IoReason;
use crate::log::{self, Event, LineProblem, LogLine};

/// What happened to each original.
#[derive(Debug, Default)]
pub struct Outcome {
    /// Originals erased (or restored), absolute.
    pub done: Vec<PathBuf>,
    /// Originals left as they are (or log files with lines skipped), and why.
    pub kept: Vec<(PathBuf, Keep)>,
}

/// Which of the two files a check was about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    Copy,
    Original,
}

/// Why an original was left alone; `i18n::Lang::keep_reason` words it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Keep {
    Unreadable {
        which: Which,
        path: PathBuf,
        error: IoReason,
    },
    /// No longer the size and SHA-256 the log recorded.
    Mismatch {
        which: Which,
        path: PathBuf,
    },
    CannotWriteLog {
        log: PathBuf,
        error: IoReason,
    },
    /// Undo restored it but couldn't log that.
    RestoredNotLogged {
        log: PathBuf,
        error: IoReason,
    },
    /// The copy is on the same device as the original: erasing would leave one exemplar.
    SameDevice {
        copy: PathBuf,
    },
    /// The system Trash refused it.
    Trash(String),
    /// Copying it back failed.
    CopyBack(IoReason),
    /// Moving it back failed.
    MoveBack(IoReason),
    /// Its original name is used by another file now: the moved file stays.
    NameTaken {
        path: PathBuf,
    },
    /// The moved file's size changed since it was moved (edited?): it stays.
    Changed {
        path: PathBuf,
    },
    /// The moved file is no longer where it was moved (renamed or deleted since).
    Gone {
        path: PathBuf,
    },
    /// A log line that couldn't be used, skipped.
    LogLine {
        line: usize,
        problem: LineProblem,
    },
}

impl Outcome {
    /// Start with one `kept` entry per log line that couldn't be used.
    fn with_problems(log_path: &Path, problems: Vec<(usize, LineProblem)>) -> Self {
        Outcome {
            done: Vec::new(),
            kept: problems
                .into_iter()
                .map(|(line, problem)| (log_path.to_path_buf(), Keep::LogLine { line, problem }))
                .collect(),
        }
    }
}

/// Move to the Trash every original of `shoot` whose copy was verified, after checking
/// again that the copy and the original both still match the logged hash.
pub fn erase(shoot: &Path) -> io::Result<Outcome> {
    let log_path = log::path_in(shoot);
    let (lines, problems) = log::read(&log_path)?;
    let mut outcome = Outcome::with_problems(&log_path, problems);
    for line in last_events(&lines) {
        // `erased` with the original still there: an earlier Trash move failed.
        let erasable = matches!(line.event, Event::Copied | Event::Restored | Event::Erased);
        if !erasable || !line.source.exists() {
            continue;
        }
        if let Err(why) = check(&shoot.join(&line.dest), &line, Which::Copy)
            .and_then(|()| check(&line.source, &line, Which::Original))
        {
            outcome.kept.push((line.source.clone(), why));
            continue;
        }
        let copy = shoot.join(&line.dest);
        if same_device(&copy, &line.source) {
            outcome
                .kept
                .push((line.source.clone(), Keep::SameDevice { copy }));
            continue;
        }
        // Logged first: undo must know about every original that may be in the Trash.
        if line.event != Event::Erased
            && let Err(e) = log::append(&log_path, &line.follow_up(Event::Erased))
        {
            let why = Keep::CannotWriteLog {
                log: log_path.clone(),
                error: IoReason::from(&e),
            };
            outcome.kept.push((line.source.clone(), why));
            continue;
        }
        match trash::delete(&line.source) {
            Ok(()) => outcome.done.push(line.source.clone()),
            Err(e) => outcome
                .kept
                .push((line.source.clone(), Keep::Trash(e.to_string()))),
        }
    }
    Ok(outcome)
}

/// Copy every erased original of `shoot` back to its name and folder from its copy,
/// once the copy matches the logged hash. Originals already back are left alone.
pub fn undo(shoot: &Path) -> io::Result<Outcome> {
    let log_path = log::path_in(shoot);
    let (lines, problems) = log::read(&log_path)?;
    let mut outcome = Outcome::with_problems(&log_path, problems);
    for line in last_events(&lines) {
        if line.event != Event::Erased || line.source.exists() {
            continue;
        }
        let copy = shoot.join(&line.dest);
        let restored = check(&copy, &line, Which::Copy).and_then(|()| {
            copy_verified(&copy, &line.source)
                .map(|_| ())
                .map_err(|e| Keep::CopyBack(IoReason::from(&e)))
        });
        match restored {
            Ok(()) => {
                if let Err(e) = log::append(&log_path, &line.follow_up(Event::Restored)) {
                    let why = Keep::RestoredNotLogged {
                        log: log_path.clone(),
                        error: IoReason::from(&e),
                    };
                    outcome.kept.push((line.source.clone(), why));
                }
                outcome.done.push(line.source.clone());
            }
            Err(why) => outcome.kept.push((line.source.clone(), why)),
        }
    }
    Ok(outcome)
}

/// Move back the files that the move `run` of `shoot` moved (ADR 0006): other moves and
/// erased originals are left alone.
pub fn undo_move(shoot: &Path, run: &str) -> io::Result<Outcome> {
    let log_path = log::path_in(shoot);
    let (lines, problems) = log::read(&log_path)?;
    let mut outcome = Outcome::with_problems(&log_path, problems);
    // That run's lines only: a later run may have logged the same original since.
    let of_run: Vec<LogLine> = lines
        .into_iter()
        .filter(|l| l.run.as_deref() == Some(run))
        .collect();
    for line in last_events(&of_run) {
        if line.event == Event::Moved {
            move_back(shoot, &log_path, &line, &mut outcome);
        }
    }
    Ok(outcome)
}

/// Undo the move of one file (ADR 0006): rename it back to its original name.
fn move_back(shoot: &Path, log_path: &Path, line: &LogLine, outcome: &mut Outcome) {
    let moved = shoot.join(&line.dest);
    let size = fs::symlink_metadata(&moved).map(|m| m.len()).ok();
    if size.is_none() {
        // Back at its name already, or never moved (the log is written first): nothing
        // to undo. Else it was renamed or deleted since.
        if fs::symlink_metadata(&line.source).is_err() {
            let why = Keep::Gone { path: moved };
            outcome.kept.push((line.source.clone(), why));
        }
        return;
    }
    if fs::symlink_metadata(&line.source).is_ok() {
        let why = Keep::NameTaken {
            path: line.source.clone(),
        };
        outcome.kept.push((line.source.clone(), why));
        return;
    }
    if size != line.size {
        outcome
            .kept
            .push((line.source.clone(), Keep::Changed { path: moved }));
        return;
    }
    if let Err(e) = crate::apply::move_file(&moved, &line.source) {
        let why = Keep::MoveBack(IoReason::from(&e));
        outcome.kept.push((line.source.clone(), why));
        return;
    }
    if let Err(e) = log::append(log_path, &line.follow_up(Event::MovedBack)) {
        let why = Keep::RestoredNotLogged {
            log: log_path.to_path_buf(),
            error: IoReason::from(&e),
        };
        outcome.kept.push((line.source.clone(), why));
    }
    outcome.done.push(line.source.clone());
}

/// The last event of each original, in log order of their first appearance.
fn last_events(lines: &[LogLine]) -> Vec<LogLine> {
    let mut order = Vec::new();
    let mut last: BTreeMap<PathBuf, &LogLine> = BTreeMap::new();
    for l in lines {
        if last.insert(l.source.clone(), l).is_none() {
            order.push(l.source.clone());
        }
    }
    order.iter().map(|s| last[s].clone()).collect()
}

/// `path` still has the size and SHA-256 the log recorded.
fn check(path: &Path, line: &LogLine, which: Which) -> Result<(), Keep> {
    let (size, sha256) = hash_file(path).map_err(|e| Keep::Unreadable {
        which,
        path: path.to_path_buf(),
        error: IoReason::from(&e),
    })?;
    if Some(size) == line.size && line.sha256.as_deref() == Some(sha256.as_str()) {
        Ok(())
    } else {
        Err(Keep::Mismatch {
            which,
            path: path.to_path_buf(),
        })
    }
}

/// Whether two existing files are on the same device (disk, card, partition).
fn same_device(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let (a_path, b_path) = (a, b);
        let (Ok(a), Ok(b)) = (std::fs::metadata(a), std::fs::metadata(b)) else {
            return false;
        };
        if a.dev() == b.dev() {
            return true;
        }
        // Linux: btrfs subvolumes and bind mounts show their own device numbers for one
        // disk; the disk itself is the source of the mount holding each file, found by
        // mount point in /proc/self/mountinfo.
        #[cfg(target_os = "linux")]
        if let Ok(info) = std::fs::read_to_string("/proc/self/mountinfo") {
            let disk = |p: &Path| {
                let real = std::fs::canonicalize(p).ok()?;
                // Only real disks: "tmpfs", "overlay"… name no disk and prove nothing.
                mount_source_for(&info, &real).filter(|s| s.starts_with("/dev/"))
            };
            if let (Some(x), Some(y)) = (disk(a_path), disk(b_path)) {
                return x == y;
            }
        }
        false
    }
    #[cfg(windows)]
    {
        // The volume: the drive letter or share the real path starts with.
        let volume = |p: &Path| {
            std::fs::canonicalize(p).ok().and_then(|p| {
                p.components()
                    .next()
                    .map(|c| c.as_os_str().to_ascii_lowercase())
            })
        };
        matches!((volume(a), volume(b)), (Some(x), Some(y)) if x == y)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (a, b);
        false
    }
}

/// The mount source (e.g. `/dev/nvme0n1p3`) of the device `major:minor` in a
/// `/proc/self/mountinfo` text.
pub fn mount_source(mountinfo: &str, major: u64, minor: u64) -> Option<String> {
    let wanted = format!("{major}:{minor}");
    mountinfo.lines().find_map(|line| {
        let (before, after) = line.split_once(" - ")?;
        if before.split(' ').nth(2)? != wanted {
            return None;
        }
        after.split(' ').nth(1).map(str::to_owned)
    })
}

/// The mount source (e.g. `/dev/nvme0n1p3`) of the mount holding `path` (absolute, real),
/// found by mount point in a `/proc/self/mountinfo` text: the longest mount point that
/// contains `path`.
pub fn mount_source_for(mountinfo: &str, path: &Path) -> Option<String> {
    mount_field_for(mountinfo, path, 1)
}

/// mountinfo writes spaces, tabs, newlines and backslashes as `\040`, `\011`, `\012`,
/// `\134`.
fn unescape_mount(field: &str) -> PathBuf {
    let field = field
        .replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\012", "\n")
        .replace("\\134", "\\");
    PathBuf::from(field)
}

/// The file system type (e.g. `vfat`, `exfat`, `ext4`) of the mount holding `path`
/// (absolute, real), found like `mount_source_for`.
pub fn mount_fstype_for(mountinfo: &str, path: &Path) -> Option<String> {
    mount_field_for(mountinfo, path, 0)
}

/// Field `n` after the ` - ` separator (0: type, 1: source) of the mount holding `path`.
fn mount_field_for(mountinfo: &str, path: &Path, n: usize) -> Option<String> {
    mountinfo
        .lines()
        .filter_map(|line| {
            let (before, after) = line.split_once(" - ")?;
            let point = unescape_mount(before.split(' ').nth(4)?);
            let field = after.split(' ').nth(n)?.to_owned();
            path.starts_with(&point).then_some((point, field))
        })
        .max_by_key(|(point, _)| point.components().count())
        .map(|(_, field)| field)
}
