//! `exportpro`: the core's features in a terminal.
//!
//!   exportpro plan|apply --source <dir> --dest <dir> --shoot <name> [--prefix <p>]
//!                  [--date YYYY-MM-DD | --split | --no-date]
//!                  [--preset classique|lisible] [--shoot-path <template>] [--file-path <template>]
//!
//!   exportpro erase|undo <shoot folder>...
//!
//! Exit status: 0 done, 1 error, 2 the files span several days: they are listed, choose
//! one with --date or --split.

use std::path::PathBuf;
use std::process::ExitCode;

use chrono::NaiveDate;
use exportpro_core::{DateChoice, Layout, LayoutError, Plan, PlanError, Request, Template};

const DEFAULT_PREFIX: &str = "txo";
const USAGE: &str = "usage: exportpro plan|apply --source <dir> --dest <dir> --shoot <name> [--prefix <p>] \
[--date YYYY-MM-DD | --split | --no-date] [--preset classique|lisible] \
[--shoot-path <template>] [--file-path <template>]\n       \
exportpro erase|undo <shoot folder>...";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some(cmd @ ("plan" | "apply")) => match run(cmd, &args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(Failure::Error(msg)) => {
                eprintln!("exportpro: {msg}");
                ExitCode::FAILURE
            }
            Err(Failure::ChooseDay(days)) => {
                for (day, n) in days {
                    println!("{day}: {n} file{}", if n == 1 { "" } else { "s" });
                }
                eprintln!(
                    "exportpro: the files span several days: choose one with --date, or --split to give each day its folder"
                );
                ExitCode::from(2)
            }
        },
        Some(cmd @ ("erase" | "undo")) => run_log_command(cmd, &args[1..]),
        None | Some("-h" | "--help") => {
            println!("exportpro {}", env!("CARGO_PKG_VERSION"));
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("exportpro: unknown command {other:?}\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Default)]
struct PlanArgs {
    source: Option<PathBuf>,
    dest: Option<PathBuf>,
    shoot: Option<String>,
    prefix: Option<String>,
    date: Option<NaiveDate>,
    split: bool,
    no_date: bool,
    preset: Option<String>,
    shoot_path: Option<String>,
    file_path: Option<String>,
}

enum Failure {
    Error(String),
    ChooseDay(Vec<(NaiveDate, usize)>),
}

impl From<String> for Failure {
    fn from(msg: String) -> Self {
        Failure::Error(msg)
    }
}

impl From<&str> for Failure {
    fn from(msg: &str) -> Self {
        Failure::Error(msg.to_owned())
    }
}

fn parse_plan(args: &[String]) -> Result<PlanArgs, String> {
    let mut a = PlanArgs::default();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{flag} needs a value"))
        };
        match flag.as_str() {
            "--source" => a.source = Some(value()?.into()),
            "--dest" => a.dest = Some(value()?.into()),
            "--shoot" => a.shoot = Some(value()?),
            "--prefix" => a.prefix = Some(value()?),
            "--date" => {
                let v = value()?;
                let d = NaiveDate::parse_from_str(&v, "%Y-%m-%d")
                    .map_err(|_| format!("--date: {v} is not a date (YYYY-MM-DD)"))?;
                a.date = Some(d);
            }
            "--split" => a.split = true,
            "--no-date" => a.no_date = true,
            "--preset" => a.preset = Some(value()?),
            "--shoot-path" => a.shoot_path = Some(value()?),
            "--file-path" => a.file_path = Some(value()?),
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(a)
}

/// The preset (default classique) with the templates given on the command line.
fn layout(
    preset: &Option<String>,
    shoot_path: &Option<String>,
    file_path: &Option<String>,
) -> Result<Layout, Failure> {
    let mut layout = match preset {
        None => Layout::default(),
        Some(name) => Layout::preset(name)
            .ok_or_else(|| format!("unknown preset {name:?} (classique, lisible)"))?,
    };
    let bad = |e: LayoutError| Failure::Error(format!("invalid tree: {e}"));
    if let Some(t) = shoot_path {
        layout.shoot = Template::parse(t).map_err(|e| bad(e.into()))?;
    }
    if let Some(t) = file_path {
        layout.original = Template::parse(t).map_err(|e| bad(e.into()))?;
    }
    layout.validate().map_err(bad)?;
    Ok(layout)
}

fn run(cmd: &str, args: &[String]) -> Result<(), Failure> {
    let (plan, req) = make_plan(args)?;
    if cmd == "plan" {
        for e in &plan.entries {
            println!("{} -> {}", e.name, e.dest.display());
        }
        return Ok(());
    }
    let outcome = exportpro_core::apply(&plan, &req);
    for e in &outcome.copied {
        println!("{} -> {}", e.name, e.dest.display());
    }
    for (e, err) in &outcome.failed {
        eprintln!(
            "exportpro: FAILED {} -> {}: {err}",
            e.name,
            e.dest.display()
        );
    }
    for log in &outcome.logs {
        println!("log: {}", log.display());
    }
    if outcome.failed.is_empty() {
        println!(
            "{} file(s) copied; the originals are untouched",
            outcome.copied.len()
        );
        Ok(())
    } else {
        Err(Failure::Error(format!(
            "{} file(s) copied, {} failed; the originals are untouched",
            outcome.copied.len(),
            outcome.failed.len()
        )))
    }
}

/// `exportpro erase|undo <shoot folder>...`
fn run_log_command(cmd: &str, folders: &[String]) -> ExitCode {
    if folders.is_empty() {
        eprintln!("exportpro: {cmd} needs the shoot folder(s) holding exportpro.log\n{USAGE}");
        return ExitCode::FAILURE;
    }
    let mut ok = true;
    for folder in folders {
        let folder = PathBuf::from(folder);
        let result = if cmd == "erase" {
            exportpro_core::erase::erase(&folder)
        } else {
            exportpro_core::erase::undo(&folder)
        };
        match result {
            Ok(outcome) => {
                let verb = if cmd == "erase" {
                    "moved to the Trash"
                } else {
                    "restored"
                };
                for path in &outcome.done {
                    println!("{verb}: {}", path.display());
                }
                for (path, why) in &outcome.kept {
                    eprintln!("exportpro: kept {}: {why}", path.display());
                }
                println!(
                    "{} file(s) {verb}, {} kept",
                    outcome.done.len(),
                    outcome.kept.len()
                );
                ok &= outcome.kept.is_empty();
            }
            Err(e) => {
                eprintln!("exportpro: {}: {e}", folder.join("exportpro.log").display());
                ok = false;
            }
        }
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn make_plan(args: &[String]) -> Result<(Plan, Request), Failure> {
    let a = parse_plan(args)?;
    let source = a.source.ok_or("--source is required")?;
    let dest = a.dest.ok_or("--dest is required")?;
    let req = Request {
        dest,
        shoot: a.shoot.ok_or("--shoot is required")?,
        prefix: a.prefix.unwrap_or_else(|| DEFAULT_PREFIX.to_owned()),
        date: match (a.date, a.split, a.no_date) {
            (Some(d), false, false) => DateChoice::Given(d),
            (None, true, false) => DateChoice::EachDay,
            (None, false, true) => DateChoice::NoDate,
            (None, false, false) => DateChoice::Files,
            _ => return Err("--date, --split and --no-date exclude each other".into()),
        },
        layout: layout(&a.preset, &a.shoot_path, &a.file_path)?,
    };
    let groups = exportpro_core::scan(&source)
        .map_err(|e| format!("cannot read {}: {e}", source.display()))?;
    let plan = exportpro_core::plan(&groups, &req).map_err(|e| match e {
        PlanError::NoFiles => Failure::Error(format!("no files to sort in {}", source.display())),
        PlanError::SeveralDays(days) => Failure::ChooseDay(days),
        PlanError::Exists(dest) => Failure::Error(format!(
            "{} already exists: nothing was planned",
            dest.display()
        )),
        PlanError::InvalidName { name, problem } => {
            Failure::Error(format!("invalid name {name:?}: {problem}"))
        }
        PlanError::SameDestination {
            first,
            second,
            dest,
            only_case,
        } => Failure::Error(format!(
            "{first} and {second} would both go to {}{}",
            dest.display(),
            if only_case {
                " (names differing only by case are one file on macOS and Windows)"
            } else {
                ""
            }
        )),
    })?;
    Ok((plan, req))
}
