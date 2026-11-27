//! `exportx`: the core's features in a terminal.
//!
//!   exportx plan|apply --source <dir> --dest <dir> --shoot <name> [--prefix <p>]
//!                  [--date YYYY-MM-DD | --split | --no-date]
//!                  [--preset classique|lisible] [--shoot-path <template>] [--file-path <template>]
//!                  [--move]   (same disk only: move instead of copy, undone by `undo`)
//!
//!   exportx erase|undo <shoot folder>...
//!   exportx undo --run <id> <shoot folder>...   (puts back what `apply --move` moved)
//!
//!   exportx update-check   (is a newer version out? from the app's update endpoint)
//!   exportx --version
//!
//! Every command takes `--lang fr|en`; the default follows LC_ALL, LC_MESSAGES, LANG.
//!
//! Exit status: 0 done, 1 error, 2 the files span several days: they are listed, choose
//! one with --date or --split.

use std::path::PathBuf;
use std::process::ExitCode;

use chrono::NaiveDate;
use exportpro_core::apply::{Mode, ShootLocks};
use exportpro_core::{DateChoice, Lang, Layout, Plan, Request, Settings, Template};

const DEFAULT_PREFIX: &str = "txo";
const USAGE: &str = "usage: exportx plan|apply --source <dir> --dest <dir> --shoot <name> [--prefix <p>] \
[--date YYYY-MM-DD | --split | --no-date] [--preset classique|lisible] \
[--shoot-path <template>] [--file-path <template>] [--move] [--lang fr|en]\n       \
exportx erase|undo <shoot folder>... [--lang fr|en]\n       \
exportx undo --run <id> <shoot folder>...   (puts back what `apply --move` moved)\n       \
exportx update-check   (is a newer version out?)\n       \
exportx --version";

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
            eprintln!("exportx: {msg}");
            return ExitCode::FAILURE;
        }
    };
    match args.first().map(String::as_str) {
        Some(cmd @ ("plan" | "apply")) => match run(cmd, &args[1..], lang) {
            Ok(()) => ExitCode::SUCCESS,
            Err(Failure::Error(msg)) => {
                eprintln!("exportx: {msg}");
                ExitCode::FAILURE
            }
            Err(Failure::ChooseDay(days)) => {
                for (day, n) in days {
                    println!("{}", lang.day_count(day, n));
                }
                eprintln!("exportx: {}", lang.several_days());
                ExitCode::from(2)
            }
        },
        Some(cmd @ ("erase" | "undo")) => run_log_command(cmd, &args[1..], lang),
        Some("-V" | "--version") => {
            println!("exportx {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("update-check") => update_check(lang),
        None | Some("-h" | "--help") => {
            println!("exportx {}", env!("CARGO_PKG_VERSION"));
            println!("{USAGE}");
            println!();
            println!("{}", lang.formats_help());
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("exportx: {}\n{USAGE}", lang.unknown_command(other));
            ExitCode::FAILURE
        }
    }
}

/// Whether a newer release is out (the app's update check, ADR 0007).
fn update_check(lang: Lang) -> ExitCode {
    let current = env!("CARGO_PKG_VERSION");
    match exportpro_core::update::check(&exportpro_core::update::endpoint(), current) {
        Ok(Some(release)) => {
            let page = exportpro_core::update::release_page(&release.version);
            println!("{}", lang.update_available(&release.version, &page));
            ExitCode::SUCCESS
        }
        Ok(None) => {
            println!("{}", lang.up_to_date(current));
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("exportx: {}", lang.update_check_failed(&why));
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
    /// `--move`: move the files instead of copying them (same disk only, ADR 0006).
    moving: bool,
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
            "--move" => a.moving = true,
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
    let (plan, req, mode, _locks) = make_plan(args, lang, cmd == "apply")?;
    for a in &plan.already {
        eprintln!("exportx: {}", lang.already_copied(&a.name, &a.copy));
    }
    for e in plan.entries.iter().filter(|e| e.date_unreadable) {
        eprintln!("exportx: {}", lang.date_unreadable(&e.name));
    }
    for e in &plan.entries {
        for n in &e.notes {
            eprintln!("exportx: {}", lang.note(&e.name, n));
        }
    }
    if cmd == "plan" {
        for e in &plan.entries {
            println!("{} -> {}", e.name, e.dest.display());
        }
        return Ok(());
    }
    let outcome = apply_showing_progress(&plan, &req, mode, lang);
    if !outcome.copied.is_empty() || !outcome.moved.is_empty() {
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
            eprintln!("exportx: settings: {e}");
        }
    }
    for e in outcome.copied.iter().chain(&outcome.moved) {
        println!("{} -> {}", e.name, e.dest.display());
    }
    for (e, err) in &outcome.failed {
        eprintln!(
            "exportx: {}",
            lang.failed_copy(&e.name, &e.dest, &lang.copy_failure(err))
        );
    }
    for log in &outcome.logs {
        println!("log: {}", log.display());
    }
    if !outcome.moved.is_empty() {
        let shoots: Vec<PathBuf> = outcome
            .logs
            .iter()
            .filter_map(|log| log.parent().map(PathBuf::from))
            .collect();
        println!("{}", lang.moved(outcome.moved.len(), &outcome.run, &shoots));
    }
    if outcome.failed.is_empty() {
        if !outcome.copied.is_empty() || outcome.moved.is_empty() {
            println!("{}", lang.copied(outcome.copied.len()));
        }
        Ok(())
    } else {
        let (copied, moved, failed) = (
            outcome.copied.len(),
            outcome.moved.len(),
            outcome.failed.len(),
        );
        Err(Failure::Error(if mode == Mode::Move {
            lang.moved_with_failures(moved, copied, failed)
        } else {
            lang.copied_with_failures(copied, failed)
        }))
    }
}

/// `exportx erase|undo <shoot folder>...`
fn run_log_command(cmd: &str, args: &[String], lang: Lang) -> ExitCode {
    // `undo --run <id>`: move back the files of that move (`apply --move` prints it).
    let mut folders = args.to_vec();
    let mut run = None;
    if cmd == "undo"
        && let Some(i) = folders.iter().position(|a| a == "--run")
    {
        if i + 1 >= folders.len() {
            eprintln!("exportx: {}", lang.needs_value("--run"));
            return ExitCode::FAILURE;
        }
        run = Some(folders.remove(i + 1));
        folders.remove(i);
    }
    if folders.is_empty() {
        eprintln!("exportx: {}\n{USAGE}", lang.needs_shoot_folders(cmd));
        return ExitCode::FAILURE;
    }
    let mut ok = true;
    for folder in &folders {
        let folder = PathBuf::from(folder);
        let result = match (cmd, &run) {
            ("erase", _) => exportpro_core::erase::erase(&folder),
            (_, Some(run)) => exportpro_core::erase::undo_move(&folder, run),
            (_, None) => exportpro_core::erase::undo(&folder),
        };
        match result {
            Ok(outcome) => {
                let verb = match (cmd, &run) {
                    ("erase", _) => lang.erased(),
                    (_, Some(_)) => lang.moved_back(),
                    (_, None) => lang.restored(),
                };
                for path in &outcome.done {
                    println!("{}", lang.done_line(verb, path));
                }
                for (path, why) in &outcome.kept {
                    eprintln!("exportx: {}", lang.kept(path, &lang.keep_reason(why)));
                }
                println!(
                    "{}",
                    lang.summary(outcome.done.len(), verb, outcome.kept.len())
                );
                ok &= outcome.kept.is_empty();
            }
            Err(e) => {
                eprintln!(
                    "exportx: {}: {e}",
                    exportpro_core::log::path_in(&folder).display()
                );
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
) -> Result<(Plan, Request, Mode, Option<ShootLocks>), Failure> {
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
    let mode = if a.moving { Mode::Move } else { Mode::Copy };
    if locked {
        let (plan, locks) =
            exportpro_core::apply::plan_locked_for(&groups, &req, mode).map_err(refused)?;
        Ok((plan, req, mode, Some(locks)))
    } else {
        let plan = exportpro_core::plan::plan_for(&groups, &req, mode).map_err(refused)?;
        Ok((plan, req, mode, None))
    }
}

/// Copy the plan; on a terminal, keep the app's progress line updated on stderr.
fn apply_showing_progress(
    plan: &Plan,
    req: &Request,
    mode: Mode,
    lang: Lang,
) -> exportpro_core::Outcome {
    use std::io::{IsTerminal, Write};
    use std::time::Instant;

    let mut err = std::io::stderr();
    if !err.is_terminal() {
        return exportpro_core::apply::apply_with(plan, req, mode, |_| {});
    }
    let mut meter = exportpro_core::progress::Meter::new(Instant::now());
    let mut shown = false;
    let outcome = exportpro_core::apply::apply_with(plan, req, mode, |p| {
        if let Some(report) = meter.update(Instant::now(), p) {
            // Back to the line's start, then clear what's left of the previous line.
            let _ = write!(err, "\r{}\x1b[K", lang.progress(&report));
            let _ = err.flush();
            shown = true;
        }
    });
    if shown {
        let _ = writeln!(err);
    }
    outcome
}
