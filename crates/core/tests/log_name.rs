//! The shoot log is `exportx.log`; a shoot copied by ExportPro keeps its `exportpro.log`.

use std::fs;
use std::path::Path;

use exportpro_core::{DateChoice, Layout, Request};

fn request(dest: &Path) -> Request {
    Request {
        dest: dest.to_path_buf(),
        shoot: "s".into(),
        prefix: "txo".into(),
        date: DateChoice::NoDate,
        layout: Layout::default(),
    }
}

fn copy(src: &Path, req: &Request) -> exportpro_core::Outcome {
    let plan = exportpro_core::plan(&exportpro_core::scan(src).unwrap(), req).unwrap();
    exportpro_core::apply(&plan, req)
}

#[test]
fn a_new_shoot_gets_exportx_log() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("card");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.jpg"), "a").unwrap();
    let req = request(&dir.path().join("photos"));

    copy(&src, &req);

    let shoot = req.dest.join("s");
    assert!(shoot.join("exportx.log").is_file());
    assert!(!shoot.join("exportpro.log").exists());
}

#[test]
fn a_shoot_with_an_exportpro_log_keeps_using_it_for_copies_erase_and_undo() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("card");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.jpg"), "a").unwrap();
    // The copies on another device than the card: erase refuses the same one.
    let dest = tempfile::tempdir_in("/dev/shm").unwrap();
    let req = request(&dest.path().join("photos"));
    copy(&src, &req);
    let shoot = req.dest.join("s");
    // As ExportPro left it.
    fs::rename(shoot.join("exportx.log"), shoot.join("exportpro.log")).unwrap();
    fs::write(src.join("b.jpg"), "b").unwrap();

    let plan = exportpro_core::plan(&exportpro_core::scan(&src).unwrap(), &req).unwrap();
    exportpro_core::apply(&plan, &req);
    let erased = exportpro_core::erase::erase(&shoot).unwrap();
    let undone = exportpro_core::erase::undo(&shoot).unwrap();

    assert_eq!(plan.already.len(), 1, "a.jpg is known from the old log");
    assert!(!shoot.join("exportx.log").exists(), "no second log");
    let lines = fs::read_to_string(shoot.join("exportpro.log")).unwrap();
    assert!(lines.contains("b.jpg"));
    assert_eq!(erased.done.len(), 2, "{erased:?}");
    assert_eq!(undone.done.len(), 2, "{undone:?}");
}

#[test]
fn an_exportpro_log_appearing_in_an_exportx_shoot_hides_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("card");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.jpg"), "a").unwrap();
    let req = request(&dir.path().join("photos"));
    copy(&src, &req);
    let shoot = req.dest.join("s");
    // ExportPro (still installed beside ExportX) started its own log there since.
    fs::write(shoot.join("exportpro.log"), "").unwrap();
    fs::write(src.join("b.jpg"), "b").unwrap();

    let plan = exportpro_core::plan(&exportpro_core::scan(&src).unwrap(), &req).unwrap();

    assert_eq!(
        plan.already.len(),
        1,
        "a.jpg is still known from exportx.log"
    );
}

#[test]
fn a_copy_waits_for_an_exportpro_copy_into_the_same_shoot() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("card");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.jpg"), "a").unwrap();
    let req = request(&dir.path().join("photos"));
    let shoot = req.dest.join("s");
    fs::create_dir_all(&shoot).unwrap();
    let held = fs::File::create(shoot.join(".exportpro.lock")).unwrap();
    held.lock().unwrap();

    let groups = exportpro_core::scan(&src).unwrap();
    let locked = exportpro_core::apply::plan_locked(&groups, &req);

    assert!(
        matches!(locked, Err(exportpro_core::PlanError::Busy(_))),
        "{locked:?}"
    );
    assert!(
        shoot.join(".exportpro.lock").exists(),
        "ExportPro's lock stays"
    );
}
