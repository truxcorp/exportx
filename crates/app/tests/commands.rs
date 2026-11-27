//! Behaviours 13-15 at the app's command layer: what the screens get back.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use exportpro_app::commands::{
    self, DateRequest, PlanRequest, PlanResult, PlannedFile, ProgressLine,
};
use exportpro_core::apply::Mode;
use exportpro_core::{Lang, Progress};

struct Dirs {
    _tmp: tempfile::TempDir,
    _dest_tmp: tempfile::TempDir,
    src: PathBuf,
    dest: PathBuf,
    settings: PathBuf,
}

fn dirs() -> Dirs {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("card");
    // Another device than the card, as in real use (erase refuses copies on the same one).
    let dest_tmp =
        tempfile::tempdir_in("/dev/shm").unwrap_or_else(|_| tempfile::tempdir().unwrap());
    let dest = dest_tmp.path().join("photos");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dest).unwrap();
    let settings = tmp.path().join("config/settings.toml");
    Dirs {
        _tmp: tmp,
        _dest_tmp: dest_tmp,
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

/// A card and a destination on the same disk: the files can be moved (ADR 0006).
fn dirs_on_one_disk() -> Dirs {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("card");
    let dest = tmp.path().join("photos");
    fs::create_dir_all(&src).unwrap();
    let settings = tmp.path().join("config/settings.toml");
    Dirs {
        _dest_tmp: tempfile::tempdir().unwrap(),
        _tmp: tmp,
        src,
        dest,
        settings,
    }
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
        PlanResult::Plan { entries, .. } => entries,
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
            entries: vec![PlannedFile::new(
                "a.jpg",
                "2024/03/05/s/p/o/jpg/txo_0001.jpg"
            )],
            movable: false,
            copy_size: "5 B".into(),
            copy_fits: true,
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
        PlanResult::Plan { entries, .. } => entries.into_iter().map(|e| e.dest).collect::<Vec<_>>(),
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
            entries: vec![PlannedFile::new(
                "a.jpg",
                "2024/03/05/s/photo/originaux/jpg/txo_0001.jpg"
            )],
            movable: false,
            copy_size: "5 B".into(),
            copy_fits: true,
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
    let outcome = commands::apply(&req, &expected, Lang::En, &d.settings, |line| {
        seen.push(line)
    })
    .unwrap();

    assert_eq!(outcome.copied, 2);
    assert!(outcome.failed.is_empty());
    let shoot = d.dest.join("2024/03/05/s");
    assert_eq!(outcome.shoots, std::slice::from_ref(&shoot));
    assert!(shoot.join("photo/originaux/jpg/md_0002.jpg").is_file());
    let end = Progress {
        files_done: 2,
        files_total: 2,
        bytes_done: 10,
        bytes_total: 10,
    };
    assert!(
        seen.windows(2)
            .all(|w| w[0].progress.bytes_done <= w[1].progress.bytes_done),
        "{seen:?}"
    );
    assert_eq!(
        seen.last(),
        Some(&ProgressLine {
            progress: end,
            text: "2 / 2 files · 10 B / 10 B".into()
        })
    );
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
    let outcome = commands::apply(&req, &previewed(&req), Lang::En, &d.settings, |_| {}).unwrap();

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

    let result = commands::apply(&req, &expected, Lang::En, &d.settings, |_| {});

    let message = result.unwrap_err();
    assert!(message.contains("changed since the preview"), "{message}");
    assert!(!d.dest.join("2024").exists(), "something was copied");
}

// A pasted path is checked as soon as it is confirmed.

#[test]
fn an_existing_folder_is_accepted_and_made_absolute() {
    let d = dirs();

    let checked = commands::check_folder(d.src.to_str().unwrap(), None, Lang::En);

    assert_eq!(checked, Ok(d.src.clone()));
}

#[test]
fn a_tilde_is_the_home_folder() {
    utc();
    let home = std::env::var("HOME").unwrap();
    let checked = commands::check_folder("~", None, Lang::En);

    assert_eq!(checked, Ok(PathBuf::from(home)));
}

#[test]
fn a_missing_folder_or_a_file_is_refused_in_the_apps_language() {
    let d = dirs();
    let missing = d.src.join("nope");
    fs::write(d.src.join("a.jpg"), "a").unwrap();

    let gone = commands::check_folder(missing.to_str().unwrap(), None, Lang::Fr);
    let file = commands::check_folder(d.src.join("a.jpg").to_str().unwrap(), None, Lang::Fr);

    assert_eq!(
        gone,
        Err(format!("Ce dossier n'existe pas : {}", missing.display()))
    );
    assert_eq!(
        file,
        Err(format!(
            "{} n'est pas un dossier",
            d.src.join("a.jpg").display()
        ))
    );
}

#[test]
fn the_destination_cannot_be_the_source_or_inside_it() {
    let d = dirs();
    let inside = d.src.join("tri");
    fs::create_dir_all(&inside).unwrap();

    let same = commands::check_folder(d.src.to_str().unwrap(), Some(&d.src), Lang::En);
    let within = commands::check_folder(inside.to_str().unwrap(), Some(&d.src), Lang::En);
    let elsewhere = commands::check_folder(d.dest.to_str().unwrap(), Some(&d.src), Lang::En);

    assert_eq!(
        same,
        Err("The destination can't be the source folder or inside it".into())
    );
    assert_eq!(
        within,
        Err("The destination can't be the source folder or inside it".into())
    );
    assert_eq!(elsewhere, Ok(d.dest.clone()));
}

// Review of PR #20.

#[test]
fn notes_in_another_language_do_not_count_as_a_changed_preview() {
    utc();
    let d = dirs();
    mk(&d.src, "a.jpg", 0, 3600);
    let req = request(&d, DateRequest::Files);
    let mut expected = previewed(&req);
    expected[0].note = Some("noté dans l'autre langue".into());

    let outcome = commands::apply(&req, &expected, Lang::En, &d.settings, |_| {});

    assert_eq!(outcome.map(|o| o.copied), Ok(1));
}

#[test]
fn a_chosen_folder_is_taken_as_is_spaces_included() {
    let d = dirs();
    let spaced = d.src.join("DCIM ");
    fs::create_dir_all(&spaced).unwrap();

    let checked = commands::check_folder(spaced.to_str().unwrap(), None, Lang::En);

    assert_eq!(checked, Ok(spaced));
}

#[test]
fn a_folder_that_cannot_be_reached_says_so_instead_of_missing() {
    use std::os::unix::fs::PermissionsExt;
    let d = dirs();
    let locked = d.src.join("locked");
    let inner = locked.join("photos");
    fs::create_dir_all(&inner).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

    let checked = commands::check_folder(inner.to_str().unwrap(), None, Lang::Fr);

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    let message = checked.unwrap_err();
    assert!(message.contains("permission refusée"), "{message}");
}

#[test]
fn the_preview_lists_files_already_copied_and_the_copy_accepts_it() {
    utc();
    let d = dirs();
    mk(&d.src, "a.jpg", 0, 3600);
    let req = request(&d, DateRequest::Files);
    commands::apply(&req, &previewed(&req), Lang::En, &d.settings, |_| {}).unwrap();
    mk(&d.src, "b.jpg", 0, 3700);

    let preview = previewed(&req);
    let outcome = commands::apply(&req, &preview, Lang::En, &d.settings, |_| {}).unwrap();

    let already: Vec<_> = preview
        .iter()
        .filter(|f| f.already)
        .map(|f| f.name.as_str())
        .collect();
    assert_eq!(already, ["a.jpg"]);
    assert_eq!(outcome.copied, 1);
}

#[test]
fn plan_offers_moving_on_the_same_disk_with_what_a_copy_would_add() {
    utc();
    let d = dirs_on_one_disk();
    mk(&d.src, "a.jpg", 0, 3600);
    mk(&d.src, "b.jpg", 0, 3700);

    let result = commands::plan(&request(&d, DateRequest::Files), Lang::Fr);

    match result {
        PlanResult::Plan {
            entries,
            movable,
            copy_size,
            copy_fits,
        } => {
            assert_eq!(entries.len(), 2);
            assert!(movable);
            assert_eq!(copy_size, "10 o");
            assert!(copy_fits);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn moving_from_the_preview_then_undo_puts_the_files_back() {
    utc();
    let d = dirs_on_one_disk();
    mk(&d.src, "a.jpg", 0, 3600);
    mk(&d.src, "b.jpg", 0, 3700);
    let req = request(&d, DateRequest::Files);
    let expected = previewed(&req);

    let outcome =
        commands::apply_mode(&req, &expected, Mode::Move, Lang::En, &d.settings, |_| {}).unwrap();

    assert_eq!((outcome.copied, outcome.moved), (0, 2), "{outcome:?}");
    assert!(outcome.failed.is_empty());
    assert!(!d.src.join("a.jpg").exists());
    let shoot = d.dest.join("2024/03/05/s");
    assert_eq!(outcome.shoots, std::slice::from_ref(&shoot));
    assert!(shoot.join("p/o/jpg/txo_0001.jpg").is_file());

    let undone = commands::undo_move(&outcome.shoots, &outcome.run, Lang::En);

    assert_eq!((undone.done, undone.kept.len()), (2, 0));
    assert_eq!(fs::read_to_string(d.src.join("a.jpg")).unwrap(), "a.jpg");
}
