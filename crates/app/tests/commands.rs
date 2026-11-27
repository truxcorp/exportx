//! Behaviours 13-15 at the app's command layer: what the screens get back.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use exportpro_app::commands::{self, DateRequest, PlanRequest, PlanResult, PlannedFile};
use exportpro_core::Lang;

struct Dirs {
    _tmp: tempfile::TempDir,
    src: PathBuf,
    dest: PathBuf,
    settings: PathBuf,
}

fn dirs() -> Dirs {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("card");
    let dest = tmp.path().join("photos");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dest).unwrap();
    let settings = tmp.path().join("config/settings.toml");
    Dirs {
        _tmp: tmp,
        src,
        dest,
        settings,
    }
}

/// A file whose content is its name, modified at `secs` after 2024-03-05 00:00 UTC.
fn mk(dir: &Path, name: &str, day_offset: u64, secs: u64) {
    let path = dir.join(name);
    fs::write(&path, name).unwrap();
    let t =
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_709_596_800 + day_offset * 86_400 + secs);
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(t))
        .unwrap();
}

fn request(d: &Dirs, date: DateRequest) -> PlanRequest {
    PlanRequest {
        source: d.src.clone(),
        dest: d.dest.clone(),
        shoot: "s".into(),
        prefix: "txo".into(),
        date,
        preset: "classique".into(),
    }
}

/// Set the environment once, before any test goes on: TZ=UTC (the file times above are
/// UTC; the app reads them in local time) and a Trash of the tests' own. Every test calls
/// it first, and `Once` makes the others wait until it is done, so no thread reads the
/// environment while it is written.
fn utc() {
    static ENV: std::sync::Once = std::sync::Once::new();
    ENV.call_once(|| {
        let data = std::env::temp_dir().join(format!("exportpro-app-tests-{}", std::process::id()));
        // SAFETY: inside call_once, before any test thread proceeds to read the environment.
        unsafe {
            std::env::set_var("TZ", "UTC");
            std::env::set_var("XDG_DATA_HOME", data);
        }
    });
}

/// The preview's entries, as the app shows them before Copy.
fn previewed(req: &PlanRequest) -> Vec<PlannedFile> {
    match commands::plan(req, Lang::En) {
        PlanResult::Plan { entries } => entries,
        other => panic!("{other:?}"),
    }
}

#[test]
fn plan_returns_the_preview() {
    utc();
    let d = dirs();
    mk(&d.src, "a.jpg", 0, 3600);

    let result = commands::plan(&request(&d, DateRequest::Files), Lang::En);

    assert_eq!(
        result,
        PlanResult::Plan {
            entries: vec![PlannedFile {
                name: "a.jpg".into(),
                dest: "2024/03/05/s/p/o/jpg/txo_0001.jpg".into()
            }]
        }
    );
}

#[test]
fn plan_returns_the_days_to_choose_from() {
    utc();
    let d = dirs();
    mk(&d.src, "a.jpg", 0, 3600);
    mk(&d.src, "b.jpg", 2, 3600);

    let result = commands::plan(&request(&d, DateRequest::Files), Lang::En);

    let days = vec![
        ("2024-03-05".parse().unwrap(), 1),
        ("2024-03-07".parse().unwrap(), 1),
    ];
    assert_eq!(result, PlanResult::Days { days });
}

#[test]
fn each_day_and_no_date_choices_plan_accordingly() {
    utc();
    let d = dirs();
    mk(&d.src, "a.jpg", 0, 3600);
    mk(&d.src, "b.jpg", 2, 3600);

    let each = commands::plan(&request(&d, DateRequest::Each), Lang::En);
    let none = commands::plan(&request(&d, DateRequest::None), Lang::En);

    let dests = |r: PlanResult| match r {
        PlanResult::Plan { entries } => entries.into_iter().map(|e| e.dest).collect::<Vec<_>>(),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        dests(each),
        [
            "2024/03/05/s/p/o/jpg/txo_0001.jpg",
            "2024/03/07/s/p/o/jpg/txo_0001.jpg"
        ]
    );
    assert_eq!(
        dests(none),
        ["s/p/o/jpg/txo_0001.jpg", "s/p/o/jpg/txo_0002.jpg"]
    );
}

#[test]
fn a_refused_plan_comes_back_as_a_message_in_the_apps_language() {
    utc();
    let d = dirs();
    mk(&d.src, "a.jpg", 0, 3600);
    let mut req = request(&d, DateRequest::Files);
    req.shoot = "CON".into();

    let result = commands::plan(&req, Lang::Fr);

    assert_eq!(
        result,
        PlanResult::Error {
            message: "nom invalide \"CON\" : il est réservé sous Windows".into()
        }
    );
}

#[test]
fn the_lisible_preset_is_used() {
    utc();
    let d = dirs();
    mk(&d.src, "a.jpg", 0, 3600);
    let mut req = request(&d, DateRequest::Files);
    req.preset = "lisible".into();

    let result = commands::plan(&req, Lang::En);

    assert_eq!(
        result,
        PlanResult::Plan {
            entries: vec![PlannedFile {
                name: "a.jpg".into(),
                dest: "2024/03/05/s/photo/originaux/jpg/txo_0001.jpg".into()
            }]
        }
    );
}

#[test]
fn apply_copies_with_progress_and_remembers_the_choices() {
    utc();
    let d = dirs();
    mk(&d.src, "a.jpg", 0, 3600);
    mk(&d.src, "b.jpg", 0, 3700);
    let mut req = request(&d, DateRequest::Files);
    req.prefix = "md".into();
    req.preset = "lisible".into();
    let mut seen = Vec::new();

    let expected = previewed(&req);
    let outcome = commands::apply(&req, &expected, Lang::En, &d.settings, |done, total| {
        seen.push((done, total))
    })
    .unwrap();

    assert_eq!(outcome.copied, 2);
    assert!(outcome.failed.is_empty());
    let shoot = d.dest.join("2024/03/05/s");
    assert_eq!(outcome.shoots, std::slice::from_ref(&shoot));
    assert!(shoot.join("photo/originaux/jpg/md_0002.jpg").is_file());
    assert_eq!(seen, [(1, 2), (2, 2)]);
    let saved = commands::settings(&d.settings);
    assert_eq!(saved.dest.as_deref(), Some(d.dest.as_path()));
    assert_eq!(saved.prefix.as_deref(), Some("md"));
    assert_eq!(saved.preset.as_deref(), Some("lisible"));
}

#[test]
fn erase_then_undo_from_the_result_screen() {
    utc();
    let d = dirs();
    mk(&d.src, "a.jpg", 0, 3600);
    let req = request(&d, DateRequest::Files);
    let outcome =
        commands::apply(&req, &previewed(&req), Lang::En, &d.settings, |_, _| {}).unwrap();

    let erased = commands::erase(&outcome.shoots);
    assert_eq!((erased.done, erased.kept.len()), (1, 0));
    assert!(!d.src.join("a.jpg").exists());

    let undone = commands::undo(&outcome.shoots);
    assert_eq!((undone.done, undone.kept.len()), (1, 0));
    assert_eq!(fs::read_to_string(d.src.join("a.jpg")).unwrap(), "a.jpg");
}

#[test]
fn the_language_follows_the_system_unless_switched() {
    utc();
    let d = dirs();
    let none = commands::settings(&d.settings);
    assert_eq!(commands::lang(&none, Some("fr-FR")), Lang::Fr);
    assert_eq!(commands::lang(&none, Some("en-GB")), Lang::En);
    assert_eq!(commands::lang(&none, None), Lang::En);

    commands::set_lang(&d.settings, Lang::En).unwrap();

    let switched = commands::settings(&d.settings);
    assert_eq!(commands::lang(&switched, Some("fr-FR")), Lang::En);
}

// Review of PR #13: reproduction.

#[test]
fn copy_refuses_when_the_card_changed_since_the_preview() {
    utc();
    let d = dirs();
    mk(&d.src, "a.jpg", 0, 3600);
    let req = request(&d, DateRequest::Files);
    let expected = previewed(&req);
    mk(&d.src, "b.jpg", 1, 3600); // written to the card after the preview

    let result = commands::apply(&req, &expected, Lang::En, &d.settings, |_, _| {});

    let message = result.unwrap_err();
    assert!(message.contains("changed since the preview"), "{message}");
    assert!(!d.dest.join("2024").exists(), "something was copied");
}
