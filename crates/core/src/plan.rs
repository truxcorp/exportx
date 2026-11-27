//! Planning: where each source file goes.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;

use crate::layout::{Layout, Vars};
use crate::names::{NameProblem, check_name};
use crate::scan::{Group, Kind, VIDEO_EXTS};

/// What the user asked for.
#[derive(Debug, Clone)]
pub struct Request {
    /// The destination folder the tree starts in.
    pub dest: PathBuf,
    pub shoot: String,
    pub prefix: String,
    pub date: DateChoice,
    pub layout: Layout,
}

/// Which date names the folders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateChoice {
    /// The files' own date, when they all share one day.
    Files,
    /// A date entered (or one day chosen) by the user: every file goes into it.
    Given(NaiveDate),
    /// Each group goes into its own day's folder.
    EachDay,
    /// No date folders: `<shoot>/…`.
    NoDate,
}

/// One file to copy: `source` → `dest` (relative to the destination folder).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub source: PathBuf,
    /// `shoot` joined with `file`.
    pub dest: PathBuf,
    /// The shoot folder, relative to the destination folder: the log goes there.
    pub shoot: PathBuf,
    /// The file, relative to the shoot folder.
    pub file: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub entries: Vec<Entry>,
    /// Empty folders for edited files, relative to the destination folder.
    pub edited: BTreeSet<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    NoFiles,
    /// The files span several days and `DateChoice::Files` can't pick one: each day
    /// with its number of files, for the user to choose.
    SeveralDays(Vec<(NaiveDate, usize)>),
    /// A shoot name, prefix or path segment that some OS can't use.
    InvalidName {
        name: String,
        problem: NameProblem,
    },
    /// Two files would get the same destination, or ones differing only by case
    /// (one file on macOS and Windows).
    SameDestination {
        first: String,
        second: String,
        dest: PathBuf,
        /// The two destinations differ only by case (else they are identical).
        only_case: bool,
    },
    /// Something already exists at this destination (relative to `Request::dest`).
    Exists(PathBuf),
}

/// Each day the groups fall on (a group's day is its earliest file's), with its file count.
pub fn days(groups: &[Group]) -> Vec<(NaiveDate, usize)> {
    let mut days = BTreeMap::new();
    for g in groups {
        *days.entry(g.date().date()).or_insert(0) += g.files.len();
    }
    days.into_iter().collect()
}

/// Number the groups in order and place each file in the tree.
pub fn plan(groups: &[Group], req: &Request) -> Result<Plan, PlanError> {
    for name in [&req.shoot, &req.prefix] {
        check(name)?;
    }
    if groups.is_empty() {
        return Err(PlanError::NoFiles);
    }
    if req.date == DateChoice::Files {
        let days = days(groups);
        if days.len() > 1 {
            return Err(PlanError::SeveralDays(days));
        }
    }
    // Numbering runs per shoot folder: days sharing one folder share one numbering.
    let mut last_number: BTreeMap<PathBuf, usize> = BTreeMap::new();
    let mut entries = Vec::new();
    let mut edited = BTreeSet::new();
    for g in groups {
        let date = match req.date {
            DateChoice::Given(d) => Some(d),
            DateChoice::Files | DateChoice::EachDay => Some(g.date().date()),
            DateChoice::NoDate => None,
        };
        let shoot = shoot_folder(req, date);
        let n = last_number
            .entry(shoot.clone())
            .or_insert_with_key(|folder| highest_number(req, folder, date));
        *n += 1;
        let n = *n;
        let kind = match g.kind {
            Kind::Photo => &req.layout.photo,
            Kind::Video => &req.layout.video,
        };
        for f in &g.files {
            let vars = Vars {
                date,
                shoot: &req.shoot,
                prefix: &req.prefix,
                kind,
                ext: &f.ext,
                n,
            };
            let file: PathBuf = req.layout.original.expand(&vars).into_iter().collect();
            let edited_templates = match g.kind {
                Kind::Photo => &req.layout.edited_photo,
                Kind::Video if VIDEO_EXTS.contains(&f.ext.as_str()) => &req.layout.edited_video,
                Kind::Video => &Vec::new(),
            };
            for t in edited_templates {
                edited.insert(shoot.join(t.expand(&vars).into_iter().collect::<PathBuf>()));
            }
            entries.push(Entry {
                name: f.name.clone(),
                source: f.path.clone(),
                dest: shoot.join(&file),
                shoot: shoot.clone(),
                file,
            });
        }
    }
    check_destinations(&entries)?;
    if let Some(e) = entries
        .iter()
        .find(|e| fs::symlink_metadata(req.dest.join(&e.dest)).is_ok())
    {
        return Err(PlanError::Exists(e.dest.clone()));
    }
    Ok(Plan { entries, edited })
}

fn check(name: &str) -> Result<(), PlanError> {
    check_name(name).map_err(|problem| PlanError::InvalidName {
        name: name.to_owned(),
        problem,
    })
}

/// Every path segment is valid everywhere, and no two destinations match up to case.
fn check_destinations(entries: &[Entry]) -> Result<(), PlanError> {
    let mut seen: BTreeMap<String, &Entry> = BTreeMap::new();
    for e in entries {
        for seg in e.dest.iter() {
            check(&seg.to_string_lossy())?;
        }
        let key = e.dest.to_string_lossy().to_lowercase();
        if let Some(first) = seen.insert(key, e) {
            return Err(PlanError::SameDestination {
                first: first.name.clone(),
                second: e.name.clone(),
                dest: e.dest.clone(),
                only_case: first.dest != e.dest,
            });
        }
    }
    Ok(())
}

/// The shoot folder of `date`, relative to the destination.
fn shoot_folder(req: &Request, date: Option<NaiveDate>) -> PathBuf {
    let vars = Vars {
        date,
        shoot: &req.shoot,
        prefix: &req.prefix,
        ..Vars::default()
    };
    req.layout.shoot.expand(&vars).into_iter().collect()
}

/// The highest number already used in `folder`, the shoot folder of `date` (0 if none).
fn highest_number(req: &Request, folder: &Path, date: Option<NaiveDate>) -> usize {
    let vars = Vars {
        date,
        shoot: &req.shoot,
        prefix: &req.prefix,
        ..Vars::default()
    };
    let file_name = req.layout.original.file_name();
    let kinds = [req.layout.photo.as_str(), req.layout.video.as_str()];
    let mut highest = 0;
    walk(&req.dest.join(folder), &mut |name| {
        if let Some(n) = file_name.number_in(name, &vars, kinds) {
            highest = highest.max(n);
        }
    });
    highest
}

/// Call `f` with the name of every file under `dir` (none if it doesn't exist).
fn walk(dir: &Path, f: &mut impl FnMut(&str)) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            walk(&entry.path(), f);
        } else if let Some(name) = entry.file_name().to_str() {
            f(name);
        }
    }
}
