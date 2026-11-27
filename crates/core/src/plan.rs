//! Planning: where each source file goes.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;

use crate::layout::{Layout, Vars};
use crate::log::{self, Event, LogLine};
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
    /// The metadata library crashed on this file: dated by its file time (to report).
    pub date_unreadable: bool,
    /// Things to tell the user about this file before copying.
    pub notes: Vec<Note>,
}

/// Something worth telling about a file before copying it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
    /// Its capture date is before 2000-01-02 (a reset camera clock starts at 2000-01-01,
    /// or earlier) or after tomorrow: the camera's clock may be wrong.
    SuspiciousDate(NaiveDate),
    /// The file is empty (0 bytes): often a sign of a damaged card.
    Empty,
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub entries: Vec<Entry>,
    /// Empty folders for edited files, relative to the destination folder.
    pub edited: BTreeSet<PathBuf>,
    /// Files whose verified copy is already in the shoot (same name, size and SHA-256 in
    /// its log, copy still there): not copied again.
    pub already: Vec<Already>,
}

/// A source file already copied into the shoot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Already {
    pub name: String,
    pub source: PathBuf,
    /// The existing copy, relative to the destination folder.
    pub copy: PathBuf,
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
    /// The path inside the destination is longer than `MAX_PATH_IN_DEST` characters.
    PathTooLong {
        dest: PathBuf,
        len: usize,
    },
    /// The file is over 4 GiB and the destination is FAT32.
    TooBigForFat {
        name: String,
        size: u64,
    },
    /// Every file is already copied into the shoot: their number.
    AllCopied(usize),
    /// Another copy is writing into this shoot folder right now.
    Busy(PathBuf),
    /// The destination can't be written to (tried in its nearest existing folder).
    NotWritable {
        path: PathBuf,
        error: crate::i18n::IoReason,
    },
    /// The destination's disk can't hold the copies: bytes `needed` (margin included)
    /// and `free`.
    NoRoom {
        needed: u64,
        free: u64,
    },
    /// A folder or file of the destination can't be read, so numbers or free names
    /// can't be checked.
    CannotRead {
        path: PathBuf,
        error: crate::i18n::IoReason,
    },
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
    let mut already = Vec::new();
    let mut copied_cache: BTreeMap<PathBuf, Vec<LogLine>> = BTreeMap::new();
    for g in groups {
        let date = match req.date {
            DateChoice::Given(d) => Some(d),
            DateChoice::Files | DateChoice::EachDay => Some(g.date().date()),
            DateChoice::NoDate => None,
        };
        let shoot = shoot_folder(req, date);
        if !copied_cache.contains_key(&shoot) {
            let copies = copies_in(&req.dest.join(&shoot))?;
            copied_cache.insert(shoot.clone(), copies);
        }
        let copied = &copied_cache[&shoot];
        let mut new_files = Vec::new();
        let mut copied_here: Option<PathBuf> = None;
        for f in &g.files {
            match find_copy(f, copied, &req.dest.join(&shoot)) {
                Some(copy) => {
                    copied_here.get_or_insert_with(|| copy.clone());
                    already.push(Already {
                        name: f.name.clone(),
                        source: f.path.clone(),
                        copy: shoot.join(copy),
                    });
                }
                None => new_files.push(f),
            }
        }
        if new_files.is_empty() {
            continue;
        }
        // Part of the group is already copied (e.g. its JPEG): the rest takes that
        // copy's number, so files sharing a base name keep sharing one.
        let existing = copied_here.and_then(|copy| number_of(req, date, &copy));
        let n = match existing {
            Some(n) => n,
            None => {
                if !last_number.contains_key(&shoot) {
                    let highest = highest_number(req, &shoot, date)?;
                    last_number.insert(shoot.clone(), highest);
                }
                let n = last_number.get_mut(&shoot).expect("inserted above");
                *n += 1;
                *n
            }
        };
        let kind = match g.kind {
            Kind::Photo => &req.layout.photo,
            Kind::Video => &req.layout.video,
        };
        for f in new_files {
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
                date_unreadable: f.date_unreadable,
                notes: notes_on(f),
            });
        }
    }
    if entries.is_empty() && !already.is_empty() {
        return Err(PlanError::AllCopied(already.len()));
    }
    check_destinations(&entries)?;
    for dir in &edited {
        check_path(dir)?;
    }
    for e in &entries {
        let path = req.dest.join(&e.dest);
        match fs::symlink_metadata(&path) {
            Ok(_) => return Err(PlanError::Exists(e.dest.clone())),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(cannot_read(&path, &err)),
        }
    }
    check_room(req, &entries)?;
    Ok(Plan {
        entries,
        edited,
        already,
    })
}

/// What to tell about `f` before copying it: a capture date before 2000-01-02 (a camera
/// clock never set restarts at 2000-01-01, or earlier) or after tomorrow, an empty file.
fn notes_on(f: &crate::scan::SourceFile) -> Vec<Note> {
    let mut notes = Vec::new();
    let day = f.date.date();
    let earliest = NaiveDate::from_ymd_opt(2000, 1, 2).expect("a valid date");
    let tomorrow = chrono::Local::now().date_naive().succ_opt().unwrap_or(day);
    if day < earliest || day > tomorrow {
        notes.push(Note::SuspiciousDate(day));
    }
    if fs::metadata(&f.path).is_ok_and(|m| m.len() == 0) {
        notes.push(Note::Empty);
    }
    notes
}

/// The verified copies a shoot folder's log records whose file is still there (none
/// without a log; a log that can't be read is an error, not "nothing copied").
fn copies_in(shoot: &Path) -> Result<Vec<LogLine>, PlanError> {
    let path = shoot.join(log::LOG_NAME);
    let lines = match log::read(&path) {
        Ok((lines, _)) => lines,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(cannot_read(&path, &err)),
    };
    Ok(lines
        .into_iter()
        .filter(|l| matches!(l.event, Event::Copied | Event::Erased | Event::Restored))
        .filter(|l| shoot.join(&l.dest).is_file())
        .collect())
}

/// The number in an existing copy's file name (`p/o/jpg/txo_0007.jpg` → 7).
fn number_of(req: &Request, date: Option<NaiveDate>, copy: &Path) -> Option<usize> {
    let vars = Vars {
        date,
        shoot: &req.shoot,
        prefix: &req.prefix,
        ..Vars::default()
    };
    let kinds = [req.layout.photo.as_str(), req.layout.video.as_str()];
    let name = copy.file_name()?.to_str()?;
    req.layout
        .original
        .file_name()
        .number_in(name, &vars, kinds)
}

/// The copy (relative to the shoot) of `f` among `copied`: same file name and size,
/// then the same SHA-256 for the original and for the copy as it is now (a copy edited
/// since doesn't count). Hashes are read only for files that pass the first two.
fn find_copy(f: &crate::scan::SourceFile, copied: &[LogLine], shoot: &Path) -> Option<PathBuf> {
    let size = fs::metadata(&f.path).ok()?.len();
    let candidates: Vec<&LogLine> = copied
        .iter()
        .filter(|l| l.size == Some(size))
        .filter(|l| l.source.file_name().and_then(|n| n.to_str()) == Some(f.name.as_str()))
        .collect();
    if candidates.is_empty() {
        return None;
    }
    let sha256 = cached_hash(&f.path)?;
    candidates
        .into_iter()
        .filter(|l| l.sha256.as_deref() == Some(sha256.as_str()))
        .find(|l| cached_hash(&shoot.join(&l.dest)).as_deref() == Some(sha256.as_str()))
        .map(|l| l.dest.clone())
}

/// SHA-256 of a file, kept for the process by path, size and modification time: a copy
/// plans up to three times (preview, then twice under the locks).
fn cached_hash(path: &Path) -> Option<String> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    type Key = (PathBuf, u64, Option<std::time::SystemTime>);
    static CACHE: OnceLock<Mutex<HashMap<Key, String>>> = OnceLock::new();
    let meta = fs::metadata(path).ok()?;
    let key = (path.to_path_buf(), meta.len(), meta.modified().ok());
    let cache = CACHE.get_or_init(Default::default);
    if let Some(hash) = cache.lock().ok()?.get(&key) {
        return Some(hash.clone());
    }
    let (_, hash) = crate::apply::hash_file(path).ok()?;
    cache.lock().ok()?.insert(key, hash.clone());
    Some(hash)
}

/// The largest file FAT32 can hold (4 GiB less one byte).
pub const FAT32_MAX: u64 = 4 * 1024 * 1024 * 1024 - 1;

/// Whether a file of `size` bytes can't be stored on a file system of type `fs_type`
/// (as mountinfo or statfs name it).
pub fn too_big_for(fs_type: &str, size: u64) -> bool {
    matches!(fs_type, "vfat" | "msdos" | "fat" | "fat32") && size > FAT32_MAX
}

/// The longest path inside the destination: room to move a shoot onto a PC later
/// (Windows stops at 260 characters for a whole path).
pub const MAX_PATH_IN_DEST: usize = 200;

/// The type of the file system holding `folder` (`vfat`, `exfat`…), where it can be told:
/// Linux only for now.
fn fs_type(folder: &Path) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let info = fs::read_to_string("/proc/self/mountinfo").ok()?;
        let real = fs::canonicalize(folder).ok()?;
        crate::erase::mount_fstype_for(&info, &real)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = folder;
        None
    }
}

/// Room kept on top of the files: the log, the folders, the file system's own blocks.
pub const ROOM_MARGIN: u64 = 64 * 1024 * 1024;

/// Refuse a plan whose copies can't fit on the destination's disk.
fn check_room(req: &Request, entries: &[Entry]) -> Result<(), PlanError> {
    let mut needed = ROOM_MARGIN;
    let mut sizes = Vec::with_capacity(entries.len());
    for e in entries {
        let size = fs::metadata(&e.source)
            .map_err(|err| cannot_read(&e.source, &err))?
            .len();
        needed = needed.saturating_add(size);
        sizes.push(size);
    }
    // The destination may not exist yet: ask about its nearest existing folder (absolute,
    // so a relative path ends at the current folder, not at "").
    let dest = std::path::absolute(&req.dest).map_err(|err| cannot_read(&req.dest, &err))?;
    let mut at = dest.as_path();
    while !at.exists() {
        match at.parent() {
            Some(parent) => at = parent,
            None => break,
        }
    }
    let free = fs4::available_space(at).map_err(|err| cannot_read(at, &err))?;
    if needed > free {
        return Err(PlanError::NoRoom { needed, free });
    }
    if let Some(kind) = fs_type(at)
        && let Some((e, size)) = entries
            .iter()
            .zip(&sizes)
            .find(|(_, s)| too_big_for(&kind, **s))
    {
        return Err(PlanError::TooBigForFat {
            name: e.name.clone(),
            size: *size,
        });
    }
    check_writable(at)?;
    // Shoot folders that already exist (earlier copies) may be read-only on their own.
    let mut probed = vec![at.to_path_buf()];
    for e in entries {
        let folder = dest.join(&e.shoot);
        if folder.is_dir() && !probed.contains(&folder) {
            check_writable(&folder)?;
            probed.push(folder);
        }
    }
    Ok(())
}

/// Write and remove a small file in `folder`, where the shoot's folders will be made:
/// a read-only disk or a missing permission is caught before any copy.
fn check_writable(folder: &Path) -> Result<(), PlanError> {
    let probe = folder.join(format!(".exportpro-write-test-{}", std::process::id()));
    let written = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe);
    match written {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            Ok(())
        }
        Err(err) => Err(PlanError::NotWritable {
            path: folder.to_path_buf(),
            error: crate::i18n::IoReason::from(&err),
        }),
    }
}

/// Bytes `.part` adds to a file name while it is being copied.
const PART_SUFFIX_LEN: usize = ".part".len();

/// Every segment of `path` valid everywhere, and the whole path no longer than
/// `MAX_PATH_IN_DEST`, counted as Windows counts (UTF-16 units).
fn check_path(path: &Path) -> Result<(), PlanError> {
    for seg in path.iter() {
        check(&seg.to_string_lossy())?;
    }
    let len = path.to_string_lossy().encode_utf16().count();
    if len > MAX_PATH_IN_DEST {
        return Err(PlanError::PathTooLong {
            dest: path.to_path_buf(),
            len,
        });
    }
    Ok(())
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
        check_path(&e.dest)?;
        // The copy is written as `<name>.part` first: that name must fit too.
        let name = e.dest.file_name().unwrap_or_default().to_string_lossy();
        if name.len() + PART_SUFFIX_LEN > 255 {
            return Err(PlanError::InvalidName {
                name: name.into_owned(),
                problem: NameProblem::TooLong,
            });
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
fn highest_number(
    req: &Request,
    folder: &Path,
    date: Option<NaiveDate>,
) -> Result<usize, PlanError> {
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
    })?;
    Ok(highest)
}

/// Call `f` with the name of every file under `dir`; none if `dir` doesn't exist, an
/// error if a folder can't be read.
fn walk(dir: &Path, f: &mut impl FnMut(&str)) -> Result<(), PlanError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(cannot_read(dir, &e)),
    };
    for entry in entries {
        let entry = entry.map_err(|e| cannot_read(dir, &e))?;
        let kind = entry
            .file_type()
            .map_err(|e| cannot_read(&entry.path(), &e))?;
        if kind.is_dir() {
            walk(&entry.path(), f)?;
        } else if let Some(name) = entry.file_name().to_str() {
            f(name);
        }
    }
    Ok(())
}

fn cannot_read(path: &Path, e: &io::Error) -> PlanError {
    PlanError::CannotRead {
        path: path.to_path_buf(),
        error: crate::i18n::IoReason::from(e),
    }
}
