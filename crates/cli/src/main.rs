//! `exportpro`: the core's features in a terminal.
//!
//!   exportpro plan|apply --source <dir> --dest <dir> --shoot <name> [--prefix <p>]
//!                  [--date YYYY-MM-DD | --split | --no-date]
//!                  [--preset classique|lisible] [--shoot-path <template>] [--file-path <template>]
//!
//!   exportpro erase|undo <shoot folder>...
//!
//! Every command takes `--lang fr|en`; the default follows LC_ALL, LC_MESSAGES, LANG.
//!
//! Exit status: 0 done, 1 error, 2 the files span several days: they are listed, choose
//! one with --date or --split.

use std::path::PathBuf;
use std::process::ExitCode;

use chrono::NaiveDate;
use exportpro_core::{DateChoice, Lang, Layout, Plan, Request, Settings, Template};

const DEFAULT_PREFIX: &str = "txo";
const USAGE: &str = "usage: exportpro plan|apply --source <dir> --dest <dir> --shoot <name> [--prefix <p>] \
[--date YYYY-MM-DD | --split | --no-date] [--preset classique|lisible] \
[--shoot-path <template>] [--file-path <template>] [--lang fr|en]\n       \
exportpro erase|undo <shoot folder>... [--lang fr|en]";

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // `--lang fr|en` anywhere switches the language; else the environment's.
    let lang = match args.iter().position(|a| a == "--lang") {
        Some(i) if i + 1 < args.len() => {
            let tag = args.remove(i + 1);
            args.remove(i);
            Lang::parse(&tag)
        }
        _ => Lang::from_env(),
    };
    match args.first().map(String::as_str) {
        Some(cmd @ ("plan" | "apply")) => match run(cmd, &args[1..], lang) {
            Ok(()) => ExitCode::SUCCESS,
            Err(Failure::Error(msg)) => {
                eprintln!("exportpro: {msg}");
                ExitCode::FAILURE
            }
            Err(Failure::ChooseDay(days)) => {
                for (day, n) in days {
                    println!("{}", lang.day_count(day, n));
                }
                eprintln!("exportpro: {}", lang.several_days());
                ExitCode::from(2)
            }
        },
        Some(cmd @ ("erase" | "undo")) => run_log_command(cmd, &args[1..], lang),
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
    lang: Lang,
) -> Result<Layout, Failure> {
    let mut layout = match preset {
        None => Layout::default(),
        Some(name) => Layout::preset(name).ok_or_else(|| match lang {
            Lang::En => format!("unknown preset {name:?} (classique, lisible)"),
            Lang::Fr => format!("préréglage inconnu {name:?} (classique, lisible)"),
        })?,
    };
    let bad = |e: exportpro_core::LayoutError| Failure::Error(lang.layout_error(&e));
    if let Some(t) = shoot_path {
        layout.shoot = Template::parse(t).map_err(|e| bad(e.into()))?;
    }
    if let Some(t) = file_path {
        layout.original = Template::parse(t).map_err(|e| bad(e.into()))?;
    }
    layout.validate().map_err(bad)?;
    Ok(layout)
}

fn run(cmd: &str, args: &[String], lang: Lang) -> Result<(), Failure> {
    let (plan, req) = make_plan(args, lang)?;
    if cmd == "plan" {
        for e in &plan.entries {
            println!("{} -> {}", e.name, e.dest.display());
        }
        return Ok(());
    }
    let outcome = exportpro_core::apply(&plan, &req);
    if !outcome.copied.is_empty() {
        let saved = Settings {
            dest: Some(req.dest.clone()),
            prefix: Some(req.prefix.clone()),
            ..Settings::load()
        };
        // A preset is remembered; custom templates on the command line are not.
        let saved = Settings {
            preset: preset_used(args).or(saved.preset),
            ..saved
        };
        if let Err(e) = saved.save() {
            eprintln!("exportpro: settings: {e}");
        }
    }
    for e in &outcome.copied {
        println!("{} -> {}", e.name, e.dest.display());
    }
    for (e, err) in &outcome.failed {
        eprintln!("exportpro: {}", lang.failed_copy(&e.name, &e.dest, err));
    }
    for log in &outcome.logs {
        println!("log: {}", log.display());
    }
    if outcome.failed.is_empty() {
        println!("{}", lang.copied(outcome.copied.len()));
        Ok(())
    } else {
        Err(Failure::Error(lang.copied_with_failures(
            outcome.copied.len(),
            outcome.failed.len(),
        )))
    }
}

/// `exportpro erase|undo <shoot folder>...`
fn run_log_command(cmd: &str, folders: &[String], lang: Lang) -> ExitCode {
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
                    lang.erased()
                } else {
                    lang.restored()
                };
                for path in &outcome.done {
                    println!("{}", lang.done_line(verb, path));
                }
                for (path, why) in &outcome.kept {
                    eprintln!("exportpro: {}", lang.kept(path, why));
                }
                println!(
                    "{}",
                    lang.summary(outcome.done.len(), verb, outcome.kept.len())
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

/// The preset of a run, when it used no custom template.
fn preset_used(args: &[String]) -> Option<String> {
    let a = parse_plan(args).ok()?;
    let saved = Settings::load();
    if a.shoot_path.is_some() || a.file_path.is_some() {
        return None;
    }
    Some(
        a.preset
            .or(saved.preset)
            .unwrap_or_else(|| "classique".into()),
    )
}

fn make_plan(args: &[String], lang: Lang) -> Result<(Plan, Request), Failure> {
    let mut a = parse_plan(args)?;
    // Options left out come from the last copy.
    let saved = Settings::load();
    a.dest = a.dest.or(saved.dest);
    a.prefix = a.prefix.or(saved.prefix);
    // Custom templates start from classique, whatever was remembered: the same command
    // line always builds the same tree.
    if a.shoot_path.is_none() && a.file_path.is_none() {
        a.preset = a.preset.or(saved.preset);
    }
    let source = a.source.ok_or("--source is required")?;
    let dest = a.dest.ok_or("--dest is required")?;
    // Absolute, so the remembered destination means the same folder from anywhere.
    let dest =
        std::path::absolute(&dest).map_err(|e| Failure::Error(lang.cannot_read(&dest, &e)))?;
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
        layout: layout(&a.preset, &a.shoot_path, &a.file_path, lang)?,
    };
    let groups =
        exportpro_core::scan(&source).map_err(|e| Failure::Error(lang.cannot_read(&source, &e)))?;
    let plan = exportpro_core::plan(&groups, &req).map_err(|e| match e {
        exportpro_core::PlanError::SeveralDays(days) => Failure::ChooseDay(days),
        e => Failure::Error(lang.plan_error(&e, &source)),
    })?;
    Ok((plan, req))
}
