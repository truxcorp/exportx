//! Copying a plan: each file copied, fsynced, verified and logged (ADR 0003).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::i18n::IoReason;
use crate::log::{Event, LayoutRecord, LogLine, append};
use crate::plan::{Entry, Plan, Request};
use crate::progress::Progress;

/// What happened to each file.
#[derive(Debug, Default)]
pub struct Outcome {
    pub copied: Vec<Entry>,
    /// Moved instead of copied (ADR 0006).
    pub moved: Vec<Entry>,
    /// The entry and why its copy failed.
    pub failed: Vec<(Entry, CopyFailure)>,
    /// Every `exportpro.log` written, absolute.
    pub logs: Vec<PathBuf>,
    /// This copy or move's id in the logs: `erase::undo_move` takes it.
    pub run: String,
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
    /// Not tried: the source folder disappeared (card removed) on an earlier file.
    NotCopiedSourceGone,
    /// Not tried: the destination folder disappeared (disk unplugged) on an earlier file.
    NotCopiedDestGone,
    /// Not tried: the card or the disk stopped answering (I/O error, device gone) on an
    /// earlier file.
    NotCopiedDeviceGone,
}

/// Copy the files, or move them when the destination is on the same disk (ADR 0006).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Copy,
    Move,
}

/// Copy every entry of `plan` into `req.dest`, logging each one in its shoot folder's
/// `exportpro.log`, then create the empty folders for edited files. Originals are never
/// touched; a failed copy is removed, reported and logged, and the others go on.
pub fn apply(plan: &Plan, req: &Request) -> Outcome {
    apply_with_progress(plan, req, |_| {})
}

/// The locks of the shoot folders a copy writes into, released when dropped.
#[derive(Debug)]
pub struct ShootLocks {
    held: Vec<(File, PathBuf)>,
    /// Folders made to hold the locks, outermost first: removed (if still empty) when the
    /// copy is refused after all.
    created: Vec<PathBuf>,
}

impl ShootLocks {
    /// The shoot folders (relative to the destination) these locks cover.
    fn covers(&self, folder: &Path) -> bool {
        self.held
            .iter()
            .any(|(_, lock)| lock.parent() == Some(folder))
    }
}

impl ShootLocks {
    /// Undo `create_dir_all`s: remove the folders made for these locks if still empty.
    fn remove_created(mut self) {
        let created = std::mem::take(&mut self.created);
        drop(self);
        for dir in created.iter().rev() {
            let _ = fs::remove_dir(dir);
        }
    }
}

impl Drop for ShootLocks {
    /// The lock files go when the copy is done, only if still ours: another copy may have
    /// replaced the file meanwhile.
    fn drop(&mut self) {
        for (file, path) in self.held.drain(..) {
            if is_same_file(&file, &path) {
                let _ = fs::remove_file(&path);
            }
        }
    }
}

/// Whether `path` still names the open `file` (Unix: same device and inode). Elsewhere,
/// where an open file can't be deleted or replaced, it always does.
fn is_same_file(file: &File, path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (file.metadata(), fs::metadata(path)) {
            (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
            _ => false,
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (file, path);
        true
    }
}

/// Lock every shoot folder of `plan` (`.exportpro.lock`, made if needed) for the length
/// of a copy, so two copies never number files in the same shoot at once. Refused with
/// the folder already locked by another copy.
pub fn lock_shoots(plan: &Plan, req: &Request) -> Result<ShootLocks, crate::plan::PlanError> {
    lock_shoots_with(plan, req, |f| f.try_lock())
}

/// `lock_shoots` with the lock attempt given: the seam tests use for file systems
/// without locks. Where locks aren't supported (some network or FUSE mounts) the copy
/// goes on without one rather than being refused.
pub fn lock_shoots_with(
    plan: &Plan,
    req: &Request,
    try_lock: impl Fn(&File) -> Result<(), fs::TryLockError>,
) -> Result<ShootLocks, crate::plan::PlanError> {
    let mut locks = ShootLocks {
        held: Vec::new(),
        created: Vec::new(),
    };
    add_locks(&mut locks, plan, req, &try_lock)?;
    Ok(locks)
}

fn add_locks(
    locks: &mut ShootLocks,
    plan: &Plan,
    req: &Request,
    try_lock: &impl Fn(&File) -> Result<(), fs::TryLockError>,
) -> Result<(), crate::plan::PlanError> {
    use crate::plan::PlanError;
    for e in &plan.entries {
        let folder = req.dest.join(&e.shoot);
        if locks.covers(&folder) {
            continue;
        }
        let cannot = |err: &io::Error| PlanError::NotWritable {
            path: folder.clone(),
            error: IoReason::from(err),
        };
        let mut missing = Vec::new();
        let mut up = Some(folder.as_path());
        while let Some(dir) = up.filter(|d| !d.exists()) {
            missing.push(dir.to_path_buf());
            up = dir.parent();
        }
        fs::create_dir_all(&folder).map_err(|err| cannot(&err))?;
        locks.created.extend(missing.into_iter().rev());
        let path = folder.join(".exportpro.lock");
        // Locking the file we opened proves nothing if another copy deleted and replaced
        // it meanwhile: check the path still names it, else try again with the new one.
        let mut attempts = 0;
        loop {
            attempts += 1;
            let file = OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open(&path)
                .map_err(|err| cannot(&err))?;
            match try_lock(&file) {
                Err(fs::TryLockError::WouldBlock) => return Err(PlanError::Busy(folder)),
                Ok(()) if !is_same_file(&file, &path) && attempts < 5 => continue,
                Ok(()) if !is_same_file(&file, &path) => return Err(PlanError::Busy(folder)),
                // Locked, or no locks on this file system: either way, ours to clean up.
                Ok(()) | Err(fs::TryLockError::Error(_)) => {
                    locks.held.push((file, path));
                    break;
                }
            }
        }
    }
    Ok(())
}

/// Plan, lock the plan's shoot folders, then plan again under the locks, so the numbers
/// and free names are read while no other copy can change them.
pub fn plan_locked(
    groups: &[crate::scan::Group],
    req: &Request,
) -> Result<(Plan, ShootLocks), crate::plan::PlanError> {
    plan_locked_for(groups, req, Mode::Copy)
}

/// `plan_locked`, to copy or to move the files.
pub fn plan_locked_for(
    groups: &[crate::scan::Group],
    req: &Request,
    mode: Mode,
) -> Result<(Plan, ShootLocks), crate::plan::PlanError> {
    lock_and_plan(groups, req, mode, || {})
}

/// `plan_locked` with `between` run after the first plan, before locking: the seam tests
/// use to play another copy finishing in that gap.
pub fn plan_locked_with(
    groups: &[crate::scan::Group],
    req: &Request,
    between: impl FnOnce(),
) -> Result<(Plan, ShootLocks), crate::plan::PlanError> {
    lock_and_plan(groups, req, Mode::Copy, between)
}

fn lock_and_plan(
    groups: &[crate::scan::Group],
    req: &Request,
    mode: Mode,
    between: impl FnOnce(),
) -> Result<(Plan, ShootLocks), crate::plan::PlanError> {
    let first = crate::plan::plan_for(groups, req, mode)?;
    between();
    let mut locks = ShootLocks {
        held: Vec::new(),
        created: Vec::new(),
    };
    let locked = add_locks(&mut locks, &first, req, &|f: &File| f.try_lock())
        .and_then(|()| crate::plan::plan_for(groups, req, mode))
        // The second plan may reach a shoot folder the first didn't (it appeared meanwhile).
        .and_then(|plan| {
            add_locks(&mut locks, &plan, req, &|f: &File| f.try_lock()).map(|()| plan)
        });
    match locked {
        Ok(plan) => Ok((plan, locks)),
        Err(e) => {
            locks.remove_created();
            Err(e)
        }
    }
}

/// Give a verified `.part` its final name; `remove` deletes the `.part` once linked.
/// Once the final name exists the copy is done: failing to remove the `.part` leaves a
/// stray file (removed by a later copy to that name), never a copy reported as failed.
pub fn finish_copy(
    part: &Path,
    dst: &Path,
    remove: impl FnOnce(&Path) -> io::Result<()>,
) -> io::Result<()> {
    match fs::hard_link(part, dst) {
        Ok(()) => {
            let _ = remove(part);
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(e),
        Err(_) => {
            if fs::symlink_metadata(dst).is_ok() {
                return Err(io::Error::from(io::ErrorKind::AlreadyExists));
            }
            fs::rename(part, dst)
        }
    }
}

/// `apply_with_progress` with the copy function given: the seam tests use to fill the
/// disk. `progress(done, total)` counts files.
pub fn apply_using(
    plan: &Plan,
    req: &Request,
    mut copy: impl FnMut(&Path, &Path) -> io::Result<(u64, String)>,
    mut progress: impl FnMut(usize, usize),
) -> Outcome {
    apply_measured(
        plan,
        req,
        Mode::Copy,
        |from, to, _| copy(from, to),
        |p| progress(p.files_done, p.files_total),
    )
}

/// Copies `from` to `to`, telling `written(bytes so far)` as it goes.
type Written<'a> = &'a mut dyn FnMut(u64);

/// `apply_using`, reporting files and bytes.
fn apply_measured(
    plan: &Plan,
    req: &Request,
    mode: Mode,
    mut copy: impl FnMut(&Path, &Path, Written) -> io::Result<(u64, String)>,
    mut report: impl FnMut(Progress),
) -> Outcome {
    let layout = LayoutRecord::from(&req.layout);
    let mut outcome = Outcome {
        run: new_run(),
        ..Outcome::default()
    };
    let run = outcome.run.clone();
    // Sizes as the copy starts: one metadata read per file.
    let sizes: Vec<u64> = plan
        .entries
        .iter()
        .map(|e| fs::metadata(&e.source).map_or(0, |m| m.len()))
        .collect();
    let mut done = Progress {
        files_total: plan.entries.len(),
        bytes_total: sizes.iter().sum(),
        ..Progress::default()
    };
    let file_done = |done: &mut Progress, report: &mut dyn FnMut(Progress), i: usize| {
        done.files_done = i + 1;
        done.bytes_done += sizes[i];
        report(*done);
    };
    // The destination the user chose is made now (if missing), so it can be watched: from
    // here on, a copy never makes it again.
    let _ = fs::create_dir_all(&req.dest);
    let places = Places::new(plan, req);
    // Once the destination is full or gone, or the card is gone, trying the other files
    // only wastes time: they get this reason instead.
    let mut stop: Option<CopyFailure> = None;
    for (i, e) in plan.entries.iter().enumerate() {
        // Checked before each copy: an unplugged disk can leave its empty mount-point
        // folder, which a copy would quietly fill on the computer's own disk.
        if stop.is_none() && places.dest_gone() {
            stop = Some(CopyFailure::NotCopiedDestGone);
        }
        if let Some(reason) = &stop {
            outcome.failed.push((e.clone(), reason.clone()));
            file_done(&mut done, &mut report, i);
            continue;
        }
        let log = req.dest.join(&e.shoot).join(crate::log::LOG_NAME);
        let dest = req.dest.join(&e.dest);
        // A symbolic link is copied: moving it would move the link, not the photo.
        let is_link = fs::symlink_metadata(&e.source).is_ok_and(|m| m.is_symlink());
        if mode == Mode::Move && !is_link {
            let line = LogLine::new(Event::Moved, e, &layout)
                .with_size(sizes[i])
                .in_run(&run);
            // Logged first: undo must know about every file that may have moved.
            match append(&log, &line).map(|()| move_file(&e.source, &dest)) {
                Err(err) => {
                    stop = places.stop_reason(&err);
                    let error = IoReason::from(&err);
                    let log = log.clone();
                    outcome
                        .failed
                        .push((e.clone(), CopyFailure::Log { log, error }));
                }
                Ok(Err(err)) => {
                    let failed = line.follow_up(Event::Failed).with_error(err.to_string());
                    let _ = append(&log, &failed);
                    stop = places.stop_reason(&err);
                    let failure = CopyFailure::Copy(IoReason::from(&err));
                    outcome.failed.push((e.clone(), failure));
                }
                Ok(Ok(())) => outcome.moved.push(e.clone()),
            }
            if log.exists() && !outcome.logs.contains(&log) {
                outcome.logs.push(log);
            }
            file_done(&mut done, &mut report, i);
            continue;
        }
        let (before, size) = (done, sizes[i]);
        let mut written = |bytes: u64| {
            report(Progress {
                bytes_done: before.bytes_done + bytes.min(size),
                ..before
            })
        };
        let (line, copy_error) = match copy(&e.source, &dest, &mut written) {
            Ok((size, sha256)) => {
                strip_extended_attributes(&dest);
                let line = LogLine::new(Event::Copied, e, &layout)
                    .with_hash(size, sha256)
                    .in_run(&run);
                (line, None)
            }
            Err(err) => {
                stop = places.stop_reason(&err);
                let line = LogLine::new(Event::Failed, e, &layout)
                    .with_error(err.to_string())
                    .in_run(&run);
                (line, Some(CopyFailure::Copy(IoReason::from(&err))))
            }
        };
        // No log line into a destination that's gone: writing it would make its folders
        // again, wherever the path now leads.
        let logged = if places.dest_gone() {
            Ok(())
        } else {
            append(&log, &line)
        };
        match (logged, copy_error) {
            // The copy's own error matters most (the log often fails for the same reason).
            (_, Some(err)) => outcome.failed.push((e.clone(), err)),
            // A copy the log doesn't know about can't be erased or undone: a failure.
            (Err(err), None) => {
                stop = places.stop_reason(&err);
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
        file_done(&mut done, &mut report, i);
    }
    // Edited folders live on the destination: only a full or missing one keeps them away.
    let destination_down = if places.dest_gone() {
        Some(CopyFailure::NotCopiedDestGone)
    } else {
        stop.clone().filter(|s| {
            matches!(
                s,
                CopyFailure::NotCopiedDiskFull | CopyFailure::NotCopiedDeviceGone
            )
        })
    };
    for dir in &plan.edited {
        let failure = if let Some(reason) = &destination_down {
            Some(reason.clone())
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
                notes: Vec::new(),
            };
            outcome.failed.push((e, failure));
        }
    }
    outcome
}

/// A new run id: the time to the microsecond, the process and a counter, so two runs never
/// share one.
fn new_run() -> String {
    static COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let time = chrono::Local::now().format("%Y%m%dT%H%M%S%.6f");
    format!("{time}-{}-{n}", std::process::id())
}

/// The source and destination folders of a copy, as they were when it started.
struct Places {
    source: Option<PathBuf>,
    source_device: Option<u64>,
    dest: PathBuf,
    dest_device: Option<u64>,
}

impl Places {
    fn new(plan: &Plan, req: &Request) -> Self {
        let source = plan
            .entries
            .first()
            .and_then(|e| e.source.parent())
            .map(Path::to_path_buf);
        Places {
            source_device: source.as_deref().and_then(device_of),
            source,
            dest_device: device_of(&req.dest),
            dest: req.dest.clone(),
        }
    }

    /// The destination folder is gone, or now on another device (its disk unplugged,
    /// leaving the mount-point folder on the computer's disk).
    fn dest_gone(&self) -> bool {
        !self.dest.exists() || device_of(&self.dest) != self.dest_device
    }

    /// The card is gone: its folder vanished, changed device, or is now empty (an
    /// unmounted card leaves its mount-point folder behind).
    fn source_gone(&self) -> bool {
        let Some(source) = &self.source else {
            return false;
        };
        !source.exists()
            || device_of(source) != self.source_device
            || fs::read_dir(source).is_ok_and(|mut d| d.next().is_none())
    }

    /// Why no other file should be tried after `err`: the destination full or gone (told
    /// first: nothing must be written where it was), the card removed, or a device that
    /// stopped answering while still mounted.
    fn stop_reason(&self, err: &io::Error) -> Option<CopyFailure> {
        if is_full(err) {
            Some(CopyFailure::NotCopiedDiskFull)
        } else if self.dest_gone() {
            Some(CopyFailure::NotCopiedDestGone)
        } else if self.source_gone() {
            Some(CopyFailure::NotCopiedSourceGone)
        } else if is_device_gone(err) {
            Some(CopyFailure::NotCopiedDeviceGone)
        } else {
            None
        }
    }
}

/// A device pulled out while still mounted answers I/O errors (EIO), "no such device"
/// (ENODEV) or "no such device or address" (ENXIO), its folders still in place.
fn is_device_gone(err: &io::Error) -> bool {
    #[cfg(unix)]
    {
        matches!(err.raw_os_error(), Some(5 | 6 | 19))
    }
    #[cfg(not(unix))]
    {
        let _ = err;
        false
    }
}

/// The device a folder is on (Unix); `None` elsewhere or if it can't be read.
fn device_of(path: &Path) -> Option<u64> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        fs::metadata(path).ok().map(|m| m.dev())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

/// The destination has no room left: disk full or the user's quota reached.
fn is_full(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded
    )
}

/// `apply`, reporting its progress after each file.
pub fn apply_with_progress(plan: &Plan, req: &Request, progress: impl FnMut(Progress)) -> Outcome {
    apply_with(plan, req, Mode::Copy, progress)
}

/// `apply_with_progress`, copying or moving the files.
pub fn apply_with(
    plan: &Plan,
    req: &Request,
    mode: Mode,
    progress: impl FnMut(Progress),
) -> Outcome {
    apply_measured(plan, req, mode, copy_verified_with, progress)
}

/// Rename `src` to the new name `dst`, making its folders, never replacing a file there:
/// a hard link refuses an existing name atomically, then the old name goes; file systems
/// without hard links (FAT, exFAT) get a check, then a rename.
pub fn move_file(src: &Path, dst: &Path) -> io::Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::symlink_metadata(dst).is_ok() {
        return Err(io::Error::from(io::ErrorKind::AlreadyExists));
    }
    match fs::hard_link(src, dst) {
        Ok(()) => {
            if let Err(e) = fs::remove_file(src) {
                // Never two names for one file: the move didn't happen.
                let _ = fs::remove_file(dst);
                return Err(e);
            }
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists || is_cross_device(&e) => {
            return Err(e);
        }
        Err(_) => {
            if fs::symlink_metadata(dst).is_ok() {
                return Err(io::Error::from(io::ErrorKind::AlreadyExists));
            }
            fs::rename(src, dst)?;
        }
    }
    sync_dir(dst.parent().unwrap_or(Path::new(".")));
    sync_dir(src.parent().unwrap_or(Path::new(".")));
    Ok(())
}

/// The two paths are on different file systems (EXDEV): no rename between them.
pub fn is_cross_device(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::CrossesDevices
}

/// Copy `src` to a new file `dst` (never overwriting) with the source's file times, fsync
/// it, then read it back and compare size and SHA-256 with the source. Returns both. On
/// any error the partial copy is removed. The read-back can come from the OS cache: it
/// checks the copy path, not the medium.
pub fn copy_verified(src: &Path, dst: &Path) -> io::Result<(u64, String)> {
    copy_verified_with(src, dst, &mut |_| {})
}

/// `copy_verified`, telling `written(bytes so far)` after each chunk written.
pub fn copy_verified_with(src: &Path, dst: &Path, written: Written) -> io::Result<(u64, String)> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::symlink_metadata(dst).is_ok() {
        return Err(io::Error::from(io::ErrorKind::AlreadyExists));
    }
    // Written under a temporary name, renamed once verified: a copy cut short (crash,
    // power loss, killed) never sits under a name that looks finished.
    let part = part_name(dst);
    let mut input = File::open(src)?;
    // A stale .part (left by a crash) is removed, never written through: it may be a
    // second name of a finished copy (crash between linking and unlinking).
    match fs::remove_file(&part) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&part)?;
    let result = (|| {
        let mut so_far = 0;
        let mut unsynced = 0;
        let (size, sha256) = hash_while(&mut input, |chunk| {
            output.write_all(chunk)?;
            so_far += chunk.len() as u64;
            unsynced += chunk.len() as u64;
            // Flushed as it goes: the progress then follows the medium, not the OS cache,
            // and the final sync of a large file on a slow card or stick stays short.
            if unsynced >= SYNC_EVERY {
                output.sync_data()?;
                unsynced = 0;
            }
            written(so_far);
            Ok(())
        })?;
        let meta = input.metadata()?;
        let mut times = fs::FileTimes::new().set_modified(meta.modified()?);
        if let Ok(accessed) = meta.accessed() {
            times = times.set_accessed(accessed);
        }
        output.set_times(times)?;
        output.sync_all()?;
        let (copy_size, copy_sha256) = hash_file(&part)?;
        if copy_size != size || copy_sha256 != sha256 {
            return Err(io::Error::other("the copy differs from the original"));
        }
        finish(&part, dst)?;
        sync_dir(dst.parent().unwrap_or(Path::new(".")));
        Ok((size, sha256))
    })();
    if result.is_err() {
        drop(output);
        let _ = fs::remove_file(&part);
    }
    result
}

/// A copy is flushed to its medium every this many bytes.
const SYNC_EVERY: u64 = 64 * 1024 * 1024;

/// `IMG_1.CR2` → `IMG_1.CR2.part`, next to it.
fn part_name(dst: &Path) -> PathBuf {
    let mut name = dst.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dst.with_file_name(name)
}

/// Give the verified `.part` its final name, never replacing a file already there: a hard
/// link refuses an existing name atomically; file systems without hard links (FAT, exFAT)
/// get a check, then a rename.
fn finish(part: &Path, dst: &Path) -> io::Result<()> {
    finish_copy(part, dst, |p| fs::remove_file(p))
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
