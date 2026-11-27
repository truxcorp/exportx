//! Erasing originals to the system Trash, and undoing that from the copies (ADR 0003).
//! Both read the shoot folder's `exportpro.log`, never the current settings.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use crate::apply::{copy_verified, hash_file};
use crate::log::{self, Event, LOG_NAME, LineProblem, LogLine};

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
        error: String,
    },
    /// No longer the size and SHA-256 the log recorded.
    Mismatch {
        which: Which,
        path: PathBuf,
    },
    CannotWriteLog {
        log: PathBuf,
        error: String,
    },
    /// Undo restored it but couldn't log that.
    RestoredNotLogged {
        log: PathBuf,
        error: String,
    },
    /// The system Trash refused it.
    Trash(String),
    /// Copying it back failed.
    CopyBack(String),
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
    let log_path = shoot.join(LOG_NAME);
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
        // Logged first: undo must know about every original that may be in the Trash.
        if line.event != Event::Erased
            && let Err(e) = log::append(&log_path, &line.follow_up(Event::Erased))
        {
            let why = Keep::CannotWriteLog {
                log: log_path.clone(),
                error: e.to_string(),
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
    let log_path = shoot.join(LOG_NAME);
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
                .map_err(|e| Keep::CopyBack(e.to_string()))
        });
        match restored {
            Ok(()) => {
                if let Err(e) = log::append(&log_path, &line.follow_up(Event::Restored)) {
                    let why = Keep::RestoredNotLogged {
                        log: log_path.clone(),
                        error: e.to_string(),
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
        error: e.to_string(),
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
