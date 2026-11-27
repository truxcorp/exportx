//! Moving instead of copying when the destination is on the source's disk (ADR 0006).

use std::fs;
use std::path::Path;

use exportpro_core::apply::Mode;
use exportpro_core::log::{Event, read};
use exportpro_core::{DateChoice, Layout, Plan, Request};

fn request(dest: &Path) -> Request {
    Request {
        dest: dest.to_path_buf(),
        shoot: "s".into(),
        prefix: "txo".into(),
        date: DateChoice::NoDate,
        layout: Layout::default(),
    }
}

fn card(dir: &Path) -> std::path::PathBuf {
    let src = dir.join("card");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.jpg"), "photo a").unwrap();
    fs::write(src.join("b.mov"), "a video").unwrap();
    src
}

fn plan(src: &Path, req: &Request) -> Plan {
    exportpro_core::plan(&exportpro_core::scan(src).unwrap(), req).unwrap()
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn the_plan_offers_moving_only_when_the_destination_is_on_the_source_disk() {
    let dir = tempfile::tempdir().unwrap();
    let src = card(dir.path());
    let other = tempfile::tempdir_in("/dev/shm").unwrap();

    let (here, there) = (
        request(&dir.path().join("photos")),
        request(&other.path().join("photos")),
    );
    let same = plan(&src, &here);
    let elsewhere = plan(&src, &there);

    assert!(exportpro_core::plan::movable(&same, &here));
    assert!(!exportpro_core::plan::movable(&elsewhere, &there));
    assert_eq!(same.bytes, 14);
    // The test rename leaves nothing behind.
    assert_eq!(names(dir.path()), ["card"]);
    assert_eq!(names(&src), ["a.jpg", "b.mov"]);
    assert!(names(other.path()).is_empty());
}

fn plan_to_move(src: &Path, req: &Request) -> Plan {
    let groups = exportpro_core::scan(src).unwrap();
    exportpro_core::plan::plan_for(&groups, req, Mode::Move).unwrap()
}

#[test]
fn moving_renames_each_file_into_the_tree_and_logs_it() {
    let dir = tempfile::tempdir().unwrap();
    let src = card(dir.path());
    let req = request(&dir.path().join("photos"));
    let plan = plan_to_move(&src, &req);

    let outcome = exportpro_core::apply::apply_with(&plan, &req, Mode::Move, |_| {});

    assert_eq!(outcome.moved.len(), 2, "{outcome:?}");
    assert!(outcome.copied.is_empty() && outcome.failed.is_empty());
    let shoot = req.dest.join("s");
    for e in &plan.entries {
        assert!(!e.source.exists(), "{} still there", e.source.display());
    }
    let a = &plan.entries[0];
    assert_eq!(
        fs::read_to_string(req.dest.join(&a.dest)).unwrap(),
        "photo a"
    );
    // The source folder stays, empty.
    assert!(names(&src).is_empty());
    let (lines, problems) = read(&shoot.join("exportpro.log")).unwrap();
    assert!(problems.is_empty());
    assert_eq!(lines.len(), 2);
    assert!(
        lines
            .iter()
            .all(|l| l.event == Event::Moved && l.size == Some(7))
    );
    assert!(lines.iter().all(|l| l.sha256.is_none()));
}

#[test]
fn moving_needs_no_free_space_but_a_copy_does() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("card");
    fs::create_dir_all(&src).unwrap();
    // 8 TB, sparse: far more than the disk's free space, almost nothing on disk.
    let f = fs::File::create(src.join("big.mov")).unwrap();
    f.set_len(8_000_000_000_000).unwrap();
    let req = request(&dir.path().join("photos"));
    let groups = exportpro_core::scan(&src).unwrap();

    let copy = exportpro_core::plan::plan_for(&groups, &req, Mode::Copy);
    let moving = exportpro_core::plan::plan_for(&groups, &req, Mode::Move);

    assert!(
        matches!(copy, Err(exportpro_core::PlanError::NoRoom { .. })),
        "{copy:?}"
    );
    assert!(moving.is_ok(), "{moving:?}");
}

#[test]
fn moving_to_another_disk_is_refused_with_a_reason() {
    let dir = tempfile::tempdir().unwrap();
    let src = card(dir.path());
    let other = tempfile::tempdir_in("/dev/shm").unwrap();
    let req = request(&other.path().join("photos"));
    let groups = exportpro_core::scan(&src).unwrap();

    let refused = exportpro_core::plan::plan_for(&groups, &req, Mode::Move).unwrap_err();

    assert_eq!(refused, exportpro_core::PlanError::NotMovable);
    assert_eq!(
        exportpro_core::Lang::En.plan_error(&refused, &src),
        "the destination is on another disk than the source: the files can't be moved, only copied"
    );
}

#[test]
fn a_symbolic_link_is_copied_not_moved() {
    let dir = tempfile::tempdir().unwrap();
    let src = card(dir.path());
    let elsewhere = dir.path().join("elsewhere.jpg");
    fs::write(&elsewhere, "linked photo").unwrap();
    std::os::unix::fs::symlink(&elsewhere, src.join("c.jpg")).unwrap();
    let req = request(&dir.path().join("photos"));
    let plan = plan_to_move(&src, &req);

    let outcome = exportpro_core::apply::apply_with(&plan, &req, Mode::Move, |_| {});

    assert_eq!(
        (outcome.moved.len(), outcome.copied.len()),
        (2, 1),
        "{outcome:?}"
    );
    let c = &outcome.copied[0];
    assert_eq!(c.name, "c.jpg");
    assert!(
        fs::symlink_metadata(src.join("c.jpg"))
            .unwrap()
            .is_symlink()
    );
    let copy = req.dest.join(&c.dest);
    assert!(!fs::symlink_metadata(&copy).unwrap().is_symlink());
    assert_eq!(fs::read_to_string(&copy).unwrap(), "linked photo");
    assert_eq!(fs::read_to_string(&elsewhere).unwrap(), "linked photo");
}

#[test]
fn a_move_never_replaces_a_file_that_appeared_at_its_destination() {
    let dir = tempfile::tempdir().unwrap();
    let src = card(dir.path());
    let req = request(&dir.path().join("photos"));
    let plan = plan_to_move(&src, &req);
    let taken = req.dest.join(&plan.entries[0].dest);
    fs::create_dir_all(taken.parent().unwrap()).unwrap();
    fs::write(&taken, "someone else's").unwrap();

    let outcome = exportpro_core::apply::apply_with(&plan, &req, Mode::Move, |_| {});

    assert_eq!(
        (outcome.moved.len(), outcome.failed.len()),
        (1, 1),
        "{outcome:?}"
    );
    assert_eq!(fs::read_to_string(&taken).unwrap(), "someone else's");
    assert_eq!(fs::read_to_string(src.join("a.jpg")).unwrap(), "photo a");
    // Logged as failed after the `moved` line: undo leaves it alone.
    let (lines, _) = read(&req.dest.join("s/exportpro.log")).unwrap();
    let events: Vec<Event> = lines.iter().map(|l| l.event).collect();
    assert_eq!(events, [Event::Moved, Event::Failed, Event::Moved]);
}

#[test]
fn undo_moves_the_files_back_to_their_names() {
    let dir = tempfile::tempdir().unwrap();
    let src = card(dir.path());
    let req = request(&dir.path().join("photos"));
    let plan = plan_to_move(&src, &req);
    let moved = exportpro_core::apply::apply_with(&plan, &req, Mode::Move, |_| {});
    let shoot = req.dest.join("s");

    let undone = exportpro_core::erase::undo_move(&shoot, &moved.run).unwrap();
    let again = exportpro_core::erase::undo_move(&shoot, &moved.run).unwrap();

    assert_eq!((undone.done.len(), undone.kept.len()), (2, 0), "{undone:?}");
    assert_eq!(names(&src), ["a.jpg", "b.mov"]);
    assert_eq!(fs::read_to_string(src.join("a.jpg")).unwrap(), "photo a");
    for e in &plan.entries {
        assert!(!req.dest.join(&e.dest).exists());
    }
    let (lines, _) = read(&shoot.join("exportpro.log")).unwrap();
    assert_eq!(lines.last().unwrap().event, Event::MovedBack);
    assert_eq!((again.done.len(), again.kept.len()), (0, 0), "{again:?}");
}

#[test]
fn undo_leaves_a_moved_file_whose_name_is_taken_or_whose_size_changed() {
    let dir = tempfile::tempdir().unwrap();
    let src = card(dir.path());
    let req = request(&dir.path().join("photos"));
    let plan = plan_to_move(&src, &req);
    let moved = exportpro_core::apply::apply_with(&plan, &req, Mode::Move, |_| {});
    let shoot = req.dest.join("s");
    fs::write(src.join("a.jpg"), "a new a.jpg").unwrap();
    let b = req.dest.join(&plan.entries[1].dest);
    fs::write(&b, "edited, longer video").unwrap();

    let undone = exportpro_core::erase::undo_move(&shoot, &moved.run).unwrap();

    assert!(undone.done.is_empty(), "{undone:?}");
    let reasons: Vec<String> = undone
        .kept
        .iter()
        .map(|(_, why)| exportpro_core::Lang::En.keep_reason(why))
        .collect();
    assert_eq!(
        reasons,
        [
            format!(
                "{} is taken: the moved file stays where it is",
                src.join("a.jpg").display()
            ),
            format!("{} changed since it was moved", b.display()),
        ]
    );
    assert_eq!(
        fs::read_to_string(src.join("a.jpg")).unwrap(),
        "a new a.jpg"
    );
    assert_eq!(fs::read_to_string(&b).unwrap(), "edited, longer video");
}

#[test]
fn undo_takes_back_that_move_only_and_undo_erase_leaves_moves_alone() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("card");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.jpg"), "first move").unwrap();
    let req = request(&dir.path().join("photos"));
    let first =
        exportpro_core::apply::apply_with(&plan_to_move(&src, &req), &req, Mode::Move, |_| {});
    fs::write(src.join("c.jpg"), "second move").unwrap();
    let second =
        exportpro_core::apply::apply_with(&plan_to_move(&src, &req), &req, Mode::Move, |_| {});
    let shoot = req.dest.join("s");
    assert_ne!(first.run, second.run);

    let erase_undo = exportpro_core::erase::undo(&shoot).unwrap();
    let undone = exportpro_core::erase::undo_move(&shoot, &second.run).unwrap();

    assert_eq!(
        (erase_undo.done.len(), erase_undo.kept.len()),
        (0, 0),
        "{erase_undo:?}"
    );
    assert_eq!(undone.done, [src.join("c.jpg")]);
    assert_eq!(names(&src), ["c.jpg"]);
    assert!(
        req.dest.join(&first.moved[0].dest).is_file(),
        "the first move stays"
    );
}

#[test]
fn a_move_still_needs_room_for_the_links_it_copies() {
    let dir = tempfile::tempdir().unwrap();
    let src = card(dir.path());
    // A link to an 8 TB (sparse) video: moving copies it, and it can't fit.
    let huge = dir.path().join("library.mov");
    fs::File::create(&huge)
        .unwrap()
        .set_len(8_000_000_000_000)
        .unwrap();
    std::os::unix::fs::symlink(&huge, src.join("c.mov")).unwrap();
    let req = request(&dir.path().join("photos"));
    let groups = exportpro_core::scan(&src).unwrap();

    let moving = exportpro_core::plan::plan_for(&groups, &req, Mode::Move);

    assert!(
        matches!(moving, Err(exportpro_core::PlanError::NoRoom { .. })),
        "{moving:?}"
    );
}

#[test]
fn undo_tells_about_a_moved_file_that_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    let src = card(dir.path());
    let req = request(&dir.path().join("photos"));
    let plan = plan_to_move(&src, &req);
    let moved = exportpro_core::apply::apply_with(&plan, &req, Mode::Move, |_| {});
    let a = req.dest.join(&plan.entries[0].dest);
    fs::rename(&a, a.with_file_name("renamed.jpg")).unwrap();

    let undone = exportpro_core::erase::undo_move(&req.dest.join("s"), &moved.run).unwrap();

    assert_eq!(undone.done, [src.join("b.mov")]);
    let reasons: Vec<String> = undone
        .kept
        .iter()
        .map(|(_, why)| exportpro_core::Lang::En.keep_reason(why))
        .collect();
    assert_eq!(reasons, [format!("{} is no longer there", a.display())]);
}

#[test]
fn undo_finds_a_move_even_after_a_later_move_of_the_same_name() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("card");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.jpg"), "first a").unwrap();
    let req = request(&dir.path().join("photos"));
    let first =
        exportpro_core::apply::apply_with(&plan_to_move(&src, &req), &req, Mode::Move, |_| {});
    // A new a.jpg, moved into the same shoot by a second run.
    fs::write(src.join("a.jpg"), "second a, longer").unwrap();
    let second =
        exportpro_core::apply::apply_with(&plan_to_move(&src, &req), &req, Mode::Move, |_| {});
    let shoot = req.dest.join("s");
    exportpro_core::erase::undo_move(&shoot, &second.run).unwrap();
    fs::rename(src.join("a.jpg"), dir.path().join("kept aside.jpg")).unwrap();

    let undone = exportpro_core::erase::undo_move(&shoot, &first.run).unwrap();

    assert_eq!(undone.done, [src.join("a.jpg")], "{undone:?}");
    assert_eq!(fs::read_to_string(src.join("a.jpg")).unwrap(), "first a");
}

#[test]
fn a_copy_plan_writes_nothing_in_the_source_folder() {
    let dir = tempfile::tempdir().unwrap();
    let src = card(dir.path());
    let req = request(&dir.path().join("photos"));
    let before = fs::metadata(&src).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));

    plan(&src, &req);

    assert_eq!(fs::metadata(&src).unwrap().modified().unwrap(), before);
}
