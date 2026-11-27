//! Helpers for the CLI's end-to-end tests: real folders, real files, the real binary.
#![allow(dead_code)] // each test file uses a different subset

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime};

use tempfile::TempDir;

/// A source and a destination folder, deleted at the end of the test.
pub struct Dirs {
    _tmp: TempDir,
    pub src: PathBuf,
    pub dest: PathBuf,
    /// A home folder for the test (the Trash lives in it).
    pub home: PathBuf,
}

impl Dirs {
    pub fn new() -> Self {
        Self::with_source_name("src")
    }

    pub fn with_source_name(name: &str) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join(name);
        let dest = tmp.path().join("dest");
        let home = tmp.path().join("home");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dest).unwrap();
        fs::create_dir_all(&home).unwrap();
        Dirs {
            _tmp: tmp,
            src,
            dest,
            home,
        }
    }
}

/// Create files in `dir` whose content is their own name and whose modification time is
/// `stamp` ("YYYY-MM-DD hh:mm", UTC), like the bash tests' `mk`.
pub fn mk(dir: &Path, stamp: &str, names: &[&str]) {
    fs::create_dir_all(dir).unwrap();
    let t = epoch(stamp);
    for name in names {
        let path = dir.join(name);
        fs::write(&path, name).unwrap();
        set_mtime(&path, t);
    }
}

/// Set a file's modification time to `t` seconds since the epoch.
pub fn set_mtime(path: &Path, t: i64) {
    let when = SystemTime::UNIX_EPOCH + Duration::from_secs(t as u64);
    let file = fs::File::options().write(true).open(path).unwrap();
    file.set_times(fs::FileTimes::new().set_modified(when))
        .unwrap();
}

/// Seconds since the epoch of "YYYY-MM-DD hh:mm" in UTC.
pub fn epoch(stamp: &str) -> i64 {
    let n = |r: std::ops::Range<usize>| stamp[r].parse::<i64>().unwrap();
    let (y, mo, d, h, mi) = (n(0..4), n(5..7), n(8..10), n(11..13), n(14..16));
    // Days from civil date (Howard Hinnant's algorithm).
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    days * 86_400 + h * 3_600 + mi * 60
}

/// Run `exportpro <args>` in UTC with English messages.
pub fn exportpro(args: &[&str]) -> Output {
    exportpro_in_tz("UTC", args)
}

/// Run `exportpro <args>` with English messages in the time zone `tz` (e.g. "Europe/Paris").
pub fn exportpro_in_tz(tz: &str, args: &[&str]) -> Output {
    exportpro_with(&[("TZ", tz)], args)
}

/// Run `exportpro <args>` in UTC with English messages and the extra environment `env`.
pub fn exportpro_with(env: &[(&str, &str)], args: &[&str]) -> Output {
    exportpro_in(Path::new("."), env, args)
}

/// Like `exportpro_with`, from the working folder `cwd`.
pub fn exportpro_in(cwd: &Path, env: &[(&str, &str)], args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_exportpro"));
    cmd.current_dir(cwd);
    cmd.args(args).env("TZ", "UTC").env("LANG", "en_US.UTF-8");
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.output().unwrap()
}

/// Copy `tests/fixtures/<name>` into `dir`, with modification time `stamp` (UTC).
pub fn fixture(dir: &Path, name: &str, stamp: &str) {
    let from = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let to = dir.join(name);
    fs::copy(from, &to).unwrap();
    set_mtime(&to, epoch(stamp));
}

/// `plan_args(dirs, extra)`: the arguments of `exportpro plan --source <src> --dest <dest> <extra>`.
pub fn plan_args<'a>(dirs: &'a Dirs, extra: &[&'a str]) -> Vec<&'a str> {
    command_args("plan", dirs, extra)
}

/// `exportpro apply --source <src> --dest <dest> <extra>`: plan, then copy.
pub fn apply(dirs: &Dirs, extra: &[&str]) -> Output {
    exportpro(&command_args("apply", dirs, extra))
}

/// Run `exportpro <args>` with the test's home folder (and so its own Trash).
pub fn exportpro_at_home(dirs: &Dirs, args: &[&str]) -> Output {
    let home = dirs.home.to_str().unwrap();
    let data = dirs.home.join(".local/share");
    exportpro_with(
        &[("HOME", home), ("XDG_DATA_HOME", data.to_str().unwrap())],
        args,
    )
}

/// The test home's freedesktop Trash `files` folder.
pub fn trash_files(dirs: &Dirs) -> PathBuf {
    dirs.home.join(".local/share/Trash/files")
}

/// The arguments of `exportpro <command> --source <src> --dest <dest> <extra>`.
pub fn command_args<'a>(command: &'a str, dirs: &'a Dirs, extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec![
        command,
        "--source",
        dirs.src.to_str().unwrap(),
        "--dest",
        dirs.dest.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    args
}

/// `exportpro plan --source <src> --dest <dest> <extra>`
pub fn plan(dirs: &Dirs, extra: &[&str]) -> Output {
    exportpro(&plan_args(dirs, extra))
}

pub fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

pub fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// The `original -> destination` lines of a plan's output, in order.
pub fn moves(out: &Output) -> Vec<String> {
    stdout(out)
        .lines()
        .filter(|l| l.contains(" -> "))
        .map(str::to_owned)
        .collect()
}

/// Assert the command succeeded, showing its output if not.
pub fn assert_ok(out: &Output) {
    assert!(
        out.status.success(),
        "exit {:?}\nstdout:\n{}\nstderr:\n{}",
        out.status.code(),
        stdout(out),
        stderr(out)
    );
}
