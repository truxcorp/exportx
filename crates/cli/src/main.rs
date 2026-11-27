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

/// Options that take a value: what follows them is never read as an option.
const VALUE_OPTIONS: &[&str] = &[
    "--source",
    "--dest",
    "--shoot",
    "--prefix",
    "--date",
    "--preset",
    "--shoot-path",
    "--file-path",
];

/// Take every `--lang fr|en` out of `args` where an option name can stand (for plan and
/// apply, not as another option's value); the last one wins. The environment's language
/// when there is none.
fn take_lang(args: &mut Vec<String>) -> Result<Lang, String> {
    let plan_like = matches!(args.first().map(String::as_str), Some("plan" | "apply"));
    let mut lang = None;
    let mut i = 0;
    while i < args.len() {
        if plan_like && VALUE_OPTIONS.contains(&args[i].as_str()) {
            i += 2;
        } else if args[i] == "--lang" {
            if i + 1 >= args.len() {
                return Err(lang.unwrap_or_else(Lang::from_env).needs_value("--lang"));
            }
            let tag = args.remove(i + 1);
            args.remove(i);
            lang = Some(Lang::parse(&tag));
        } else {
            i += 1;
        }
    }
    Ok(lang.unwrap_or_else(Lang::from_env))
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let lang = match take_lang(&mut args) {
        Ok(lang) => lang,
        Err(msg) => {
            eprintln!("exportpro: {msg}");
            return ExitCode::FAILURE;
        }
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
            eprintln!("exportpro: {}\n{USAGE}", lang.unknown_command(other));
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

fn parse_plan(args: &[String], lang: Lang) -> Result<PlanArgs, String> {
    let mut a = PlanArgs::default();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| lang.needs_value(flag));
        match flag.as_str() {
            "--source" => a.source = Some(value()?.into()),
            "--dest" => a.dest = Some(value()?.into()),
            "--shoot" => a.shoot = Some(value()?),
            "--prefix" => a.prefix = Some(value()?),
            "--date" => {
                let v = value()?;
                let d = NaiveDate::parse_from_str(&v, "%Y-%m-%d")
                    .map_err(|_| lang.not_a_date("--date", &v))?;
                a.date = Some(d);
            }
            "--split" => a.split = true,
            "--no-date" => a.no_date = true,
            "--preset" => a.preset = Some(value()?),
            "--shoot-path" => a.shoot_path = Some(value()?),
            "--file-path" => a.file_path = Some(value()?),
            other => return Err(lang.unknown_option(other)),
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
    // A copy plans under the shoot locks; a preview locks nothing.
    let (plan, req, _locks) = make_plan(args, lang, cmd == "apply")?;
    for a in &plan.already {
        eprintln!("exportpro: {}", lang.already_copied(&a.name, &a.copy));
    }
    for e in plan.entries.iter().filter(|e| e.date_unreadable) {
        eprintln!("exportpro: {}", lang.date_unreadable(&e.name));
    }
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
        eprintln!(
            "exportpro: {}",
            lang.failed_copy(&e.name, &e.dest, &lang.copy_failure(err))
        );
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
        eprintln!("exportpro: {}\n{USAGE}", lang.needs_shoot_folders(cmd));
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
                    eprintln!("exportpro: {}", lang.kept(path, &lang.keep_reason(why)));
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
    let a = parse_plan(args, Lang::En).ok()?;
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

/// The plan of a `plan` or `apply` command line; with `locked`, made under the shoot
/// locks, which are returned with it.
fn make_plan(
    args: &[String],
    lang: Lang,
    locked: bool,
) -> Result<(Plan, Request, Option<exportpro_core::apply::ShootLocks>), Failure> {
    let mut a = parse_plan(args, lang)?;
    // Options left out come from the last copy.
    let saved = Settings::load();
    a.dest = a.dest.or(saved.dest);
    a.prefix = a.prefix.or(saved.prefix);
    // Custom templates start from classique, whatever was remembered: the same command
    // line always builds the same tree.
    if a.shoot_path.is_none() && a.file_path.is_none() {
        a.preset = a.preset.or(saved.preset);
    }
    let source = a.source.ok_or_else(|| lang.required("--source"))?;
    let dest = a.dest.ok_or_else(|| lang.required("--dest"))?;
    // Absolute, so the remembered destination means the same folder from anywhere.
    let dest = std::path::absolute(&dest)
        .map_err(|e| Failure::Error(lang.cannot_read(&dest, &(&e).into())))?;
    let req = Request {
        dest,
        shoot: a.shoot.ok_or_else(|| lang.required("--shoot"))?,
        prefix: a.prefix.unwrap_or_else(|| DEFAULT_PREFIX.to_owned()),
        date: match (a.date, a.split, a.no_date) {
            (Some(d), false, false) => DateChoice::Given(d),
            (None, true, false) => DateChoice::EachDay,
            (None, false, true) => DateChoice::NoDate,
            (None, false, false) => DateChoice::Files,
            _ => return Err(lang.dates_exclusive().into()),
        },
        layout: layout(&a.preset, &a.shoot_path, &a.file_path, lang)?,
    };
    let groups = exportpro_core::scan(&source)
        .map_err(|e| Failure::Error(lang.cannot_read(&source, &(&e).into())))?;
    let refused = |e| match e {
        exportpro_core::PlanError::SeveralDays(days) => Failure::ChooseDay(days),
        e => Failure::Error(lang.plan_error(&e, &source)),
    };
    if locked {
        let (plan, locks) = exportpro_core::apply::plan_locked(&groups, &req).map_err(refused)?;
        Ok((plan, req, Some(locks)))
    } else {
        let plan = exportpro_core::plan(&groups, &req).map_err(refused)?;
        Ok((plan, req, None))
    }
}
