//! Behaviours 11 and 12: erase the originals to the Trash, only once their copy is
//! verified, and undo that from the copies.
mod common;
use common::*;

use std::fs;
use std::path::{Path, PathBuf};

fn content(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Copy a.jpg, b.CR2 + b.JPG into dest; returns the shoot folder.
fn copied(d: &Dirs) -> PathBuf {
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    mk(&d.src, "2024-03-05 09:01", &["b.CR2", "b.JPG"]);
    let out = apply(d, &["--shoot", "s"]);
    assert_ok(&out);
    d.dest.join("2024/03/05/s")
}

fn erase(d: &Dirs, shoot: &Path) -> std::process::Output {
    exportpro_at_home(d, &["erase", shoot.to_str().unwrap()])
}

fn undo(d: &Dirs, shoot: &Path) -> std::process::Output {
    exportpro_at_home(d, &["undo", shoot.to_str().unwrap()])
}

#[test]
fn erase_moves_verified_originals_to_the_trash() {
    let d = Dirs::new();
    let shoot = copied(&d);

    let out = erase(&d, &shoot);

    assert_ok(&out);
    for name in ["a.jpg", "b.CR2", "b.JPG"] {
        assert!(!d.src.join(name).exists(), "{name} still in the source");
        assert_eq!(
            content(&trash_files(&d).join(name)),
            name,
            "{name} not in the Trash"
        );
    }
    assert_eq!(content(&shoot.join("p/o/jpg/txo_0001.jpg")), "a.jpg");
    assert_eq!(content(&shoot.join("p/o/cr2/txo_0002.cr2")), "b.CR2");
}

#[test]
fn an_original_whose_copy_no_longer_matches_is_kept() {
    let d = Dirs::new();
    let shoot = copied(&d);
    fs::write(shoot.join("p/o/cr2/txo_0002.cr2"), "damaged").unwrap();

    let out = erase(&d, &shoot);

    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    assert!(stderr(&out).contains("b.CR2"), "stderr: {}", stderr(&out));
    assert_eq!(content(&d.src.join("b.CR2")), "b.CR2");
    assert!(!d.src.join("a.jpg").exists());
    assert!(!d.src.join("b.JPG").exists());
}

#[test]
fn an_original_changed_since_its_copy_is_kept() {
    let d = Dirs::new();
    let shoot = copied(&d);
    fs::write(d.src.join("a.jpg"), "edited after the copy").unwrap();

    let out = erase(&d, &shoot);

    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    assert!(stderr(&out).contains("a.jpg"), "stderr: {}", stderr(&out));
    assert_eq!(content(&d.src.join("a.jpg")), "edited after the copy");
}

#[test]
fn an_original_whose_copy_failed_is_never_erased() {
    use std::os::unix::fs::PermissionsExt;
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg", "b.jpg"]);
    fs::set_permissions(d.src.join("b.jpg"), fs::Permissions::from_mode(0o000)).unwrap();
    let out = apply(&d, &["--shoot", "s"]);
    fs::set_permissions(d.src.join("b.jpg"), fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(out.status.code(), Some(1));
    let shoot = d.dest.join("2024/03/05/s");

    let out = erase(&d, &shoot);

    assert_ok(&out);
    assert!(!d.src.join("a.jpg").exists());
    assert_eq!(content(&d.src.join("b.jpg")), "b.jpg");
}

#[test]
fn undo_restores_the_originals_from_the_copies() {
    let d = Dirs::new();
    let shoot = copied(&d);
    assert_ok(&erase(&d, &shoot));

    let out = undo(&d, &shoot);

    assert_ok(&out);
    for name in ["a.jpg", "b.CR2", "b.JPG"] {
        assert_eq!(content(&d.src.join(name)), name, "{name} not restored");
    }
}

#[test]
fn undo_recreates_a_source_folder_that_is_gone() {
    let d = Dirs::new();
    let shoot = copied(&d);
    assert_ok(&erase(&d, &shoot));
    fs::remove_dir(&d.src).unwrap(); // the card was formatted

    let out = undo(&d, &shoot);

    assert_ok(&out);
    assert_eq!(content(&d.src.join("b.JPG")), "b.JPG");
}

#[test]
fn undo_refuses_a_copy_that_no_longer_matches() {
    let d = Dirs::new();
    let shoot = copied(&d);
    assert_ok(&erase(&d, &shoot));
    fs::write(shoot.join("p/o/jpg/txo_0001.jpg"), "damaged").unwrap();

    let out = undo(&d, &shoot);

    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    assert!(stderr(&out).contains("a.jpg"), "stderr: {}", stderr(&out));
    assert!(!d.src.join("a.jpg").exists());
    assert_eq!(content(&d.src.join("b.CR2")), "b.CR2");
}

#[test]
fn undo_leaves_an_original_that_is_back_already() {
    let d = Dirs::new();
    let shoot = copied(&d);
    assert_ok(&erase(&d, &shoot));
    fs::write(d.src.join("a.jpg"), "put back by hand").unwrap();

    let out = undo(&d, &shoot);

    assert_ok(&out);
    assert_eq!(content(&d.src.join("a.jpg")), "put back by hand");
    assert_eq!(content(&d.src.join("b.CR2")), "b.CR2");
}

// Review of PR #12: reproductions.

fn mtime(path: &Path) -> std::time::SystemTime {
    fs::metadata(path).unwrap().modified().unwrap()
}

#[test]
fn a_relative_source_is_logged_absolute_and_erase_works_from_elsewhere() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    let parent = d.src.parent().unwrap();
    let home = d.home.to_str().unwrap();
    let out = exportpro_in(
        parent,
        &[],
        &[
            "apply",
            "--source",
            "src",
            "--dest",
            d.dest.to_str().unwrap(),
            "--shoot",
            "s",
        ],
    );
    assert_ok(&out);
    let shoot = d.dest.join("2024/03/05/s");

    let data = d.home.join(".local/share");
    let out = exportpro_in(
        &d.home,
        &[("HOME", home), ("XDG_DATA_HOME", data.to_str().unwrap())],
        &["erase", shoot.to_str().unwrap()],
    );

    assert_ok(&out);
    assert!(!d.src.join("a.jpg").exists(), "stdout: {}", stdout(&out));
}

#[test]
fn an_erase_that_cannot_be_logged_trashes_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let d = Dirs::new();
    let shoot = copied(&d);
    let log = shoot.join("exportpro.log");
    fs::set_permissions(&log, fs::Permissions::from_mode(0o444)).unwrap();

    let out = erase(&d, &shoot);

    fs::set_permissions(&log, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    for name in ["a.jpg", "b.CR2", "b.JPG"] {
        assert_eq!(content(&d.src.join(name)), name, "{name} left the source");
    }
}

#[test]
fn a_torn_log_line_does_not_block_the_rest() {
    let d = Dirs::new();
    let shoot = copied(&d);
    let log = shoot.join("exportpro.log");
    let mut text = fs::read_to_string(&log).unwrap();
    text.push_str("{\"v\":1,\"event\":\"cop");
    fs::write(&log, text).unwrap();

    let out = erase(&d, &shoot);

    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    assert!(stderr(&out).contains("line 4"), "stderr: {}", stderr(&out));
    for name in ["a.jpg", "b.CR2", "b.JPG"] {
        assert!(!d.src.join(name).exists(), "{name} still in the source");
    }
}

#[test]
fn a_log_line_from_a_newer_version_is_skipped() {
    let d = Dirs::new();
    let shoot = copied(&d);
    let log = shoot.join("exportpro.log");
    let text = fs::read_to_string(&log)
        .unwrap()
        .replacen("\"v\":1", "\"v\":2", 1);
    fs::write(&log, text).unwrap();

    let out = erase(&d, &shoot);

    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    assert!(stderr(&out).contains("line 1"), "stderr: {}", stderr(&out));
    assert_eq!(content(&d.src.join("a.jpg")), "a.jpg");
    assert!(!d.src.join("b.CR2").exists());
}

#[test]
fn copies_and_restored_originals_keep_the_file_time() {
    let d = Dirs::new();
    let shoot = copied(&d);
    let before = mtime(&d.src.join("a.jpg"));
    assert_eq!(mtime(&shoot.join("p/o/jpg/txo_0001.jpg")), before);
    assert_ok(&erase(&d, &shoot));

    assert_ok(&undo(&d, &shoot));

    assert_eq!(mtime(&d.src.join("a.jpg")), before);
}
