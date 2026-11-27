//! Checks that catch a bad copy before it happens.
mod common;
use common::*;

use std::fs;
use std::os::unix::fs::PermissionsExt;

#[test]
fn a_destination_that_cannot_be_written_is_refused_when_planning() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    fs::set_permissions(&d.dest, fs::Permissions::from_mode(0o555)).unwrap();

    let out = exportpro_with(
        &[("LANG", "fr_FR.UTF-8")],
        &plan_args(&d, &["--shoot", "s"]),
    );

    fs::set_permissions(&d.dest, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    let err = stderr(&out);
    assert!(err.contains("impossible d'écrire dans"), "{err}");
    assert!(err.contains("permission refusée"), "{err}");
    assert_eq!(moves(&out), Vec::<String>::new());
}

#[test]
fn a_second_copy_into_a_shoot_being_copied_is_refused() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    let shoot = d.dest.join("2024/03/05/s");
    fs::create_dir_all(&shoot).unwrap();
    // Another copy holds the shoot's lock.
    let lock = fs::File::create(shoot.join(".exportx.lock")).unwrap();
    lock.try_lock().unwrap();

    let out = exportpro_with(
        &[("LANG", "fr_FR.UTF-8")],
        &command_args("apply", &d, &["--shoot", "s"]),
    );

    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    assert!(
        stderr(&out).contains("Une autre copie est en cours"),
        "{}",
        stderr(&out)
    );
    assert!(!shoot.join("p").exists(), "nothing copied");
    drop(lock);
}

/// Every file under `dir`, recursively.
fn files_under(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                found.extend(files_under(&p));
            } else {
                found.push(p);
            }
        }
    }
    found
}

#[test]
fn a_copy_killed_midway_leaves_no_file_under_its_final_name() {
    let d = Dirs::new();
    fs::create_dir_all(&d.src).unwrap();
    let big = fs::File::create(d.src.join("clip.mov")).unwrap();
    big.set_len(2_000_000_000).unwrap(); // 2 GB, sparse: takes a moment to copy
    // 2 GB need room: copy into the temp folder, not /dev/shm.
    let dest = d.home.join("photos");
    fs::create_dir_all(&dest).unwrap();
    let final_name = dest.join("2024/03/05/s/v/o/mov/txo_0001.mov");
    let (src, to) = (d.src.to_str().unwrap(), dest.to_str().unwrap());
    let args = [
        "apply",
        "--source",
        src,
        "--dest",
        to,
        "--shoot",
        "s",
        "--date",
        "2024-03-05",
    ];
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_exportpro"))
        .args(args)
        .env("XDG_CONFIG_HOME", d.home.join(".config"))
        .spawn()
        .unwrap();

    // Kill it as soon as the copy has started writing.
    let started = std::time::Instant::now();
    while files_under(&dest.join("2024/03/05/s/v")).is_empty()
        && started.elapsed() < std::time::Duration::from_secs(20)
    {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    child.kill().unwrap();
    child.wait().unwrap();

    assert!(
        !final_name.exists(),
        "a half-written file has the final name"
    );
    let leftovers = files_under(&dest.join("2024/03/05/s/v"));
    assert!(
        leftovers
            .iter()
            .all(|p| p.to_string_lossy().ends_with(".part")),
        "{leftovers:?}"
    );
}

#[test]
fn a_part_file_left_by_a_crash_does_not_block_the_next_copy() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    mk(
        &d.dest.join("2024/03/05/s/p/o/jpg"),
        "2024-03-05 09:00",
        &["txo_0001.jpg.part"],
    );

    let out = apply(&d, &["--shoot", "s"]);

    assert_ok(&out);
    let copy = d.dest.join("2024/03/05/s/p/o/jpg/txo_0001.jpg");
    assert_eq!(fs::read_to_string(copy).unwrap(), "a.jpg");
    assert!(
        !d.dest
            .join("2024/03/05/s/p/o/jpg/txo_0001.jpg.part")
            .exists()
    );
}

// Files already copied into the shoot are skipped.

#[test]
fn copying_the_same_card_again_copies_only_the_new_files() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg", "b.jpg"]);
    assert_ok(&apply(&d, &["--shoot", "s"]));
    mk(&d.src, "2024-03-05 10:00", &["c.jpg"]);

    let out = exportpro_with(
        &[("LANG", "fr_FR.UTF-8")],
        &command_args("apply", &d, &["--shoot", "s"]),
    );

    assert_ok(&out);
    let jpg = d.dest.join("2024/03/05/s/p/o/jpg");
    assert_eq!(
        fs::read_to_string(jpg.join("txo_0003.jpg")).unwrap(),
        "c.jpg"
    );
    assert!(!jpg.join("txo_0004.jpg").exists(), "a or b copied again");
    let err = stderr(&out);
    assert!(err.contains("déjà copié : a.jpg"), "{err}");
    assert!(err.contains("déjà copié : b.jpg"), "{err}");
}

#[test]
fn copying_a_card_already_fully_copied_is_refused() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg", "b.jpg"]);
    assert_ok(&apply(&d, &["--shoot", "s"]));

    let out = apply(&d, &["--shoot", "s"]);

    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    assert!(
        stderr(&out).contains("all 2 files are already copied"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn a_new_file_with_an_old_name_is_copied() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["IMG_0001.JPG"]);
    assert_ok(&apply(&d, &["--shoot", "s"]));
    // The camera's counter restarted: same name, another photo.
    fs::write(d.src.join("IMG_0001.JPG"), "another photo").unwrap();
    set_mtime(&d.src.join("IMG_0001.JPG"), epoch("2024-03-05 11:00"));

    let out = apply(&d, &["--shoot", "s"]);

    assert_ok(&out);
    let copy = d.dest.join("2024/03/05/s/p/o/jpg/txo_0002.jpg");
    assert_eq!(fs::read_to_string(copy).unwrap(), "another photo");
}

#[test]
fn erase_refuses_when_the_copies_are_on_the_originals_device() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    // A destination next to the source: the same device, as a copy onto the card itself.
    let dest = d.home.join("same-disk");
    fs::create_dir_all(&dest).unwrap();
    let (src, to) = (d.src.to_str().unwrap(), dest.to_str().unwrap());
    assert_ok(&exportpro(&[
        "apply", "--source", src, "--dest", to, "--shoot", "s",
    ]));
    let shoot = dest.join("2024/03/05/s");
    let data = d.home.join(".local/share");

    let out = exportpro_with(
        &[
            ("LANG", "fr_FR.UTF-8"),
            ("HOME", d.home.to_str().unwrap()),
            ("XDG_DATA_HOME", data.to_str().unwrap()),
        ],
        &["erase", shoot.to_str().unwrap()],
    );

    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    assert!(
        stderr(&out).contains("même disque que l'original"),
        "{}",
        stderr(&out)
    );
    assert_eq!(fs::read_to_string(d.src.join("a.jpg")).unwrap(), "a.jpg");
}

// Review of PR #22 (round 1).

#[test]
fn a_raw_whose_jpeg_was_copied_before_gets_the_jpegs_number() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["A.CR2", "A.JPG"]);
    fs::set_permissions(d.src.join("A.CR2"), fs::Permissions::from_mode(0o000)).unwrap();
    let first = apply(&d, &["--shoot", "s"]);
    fs::set_permissions(d.src.join("A.CR2"), fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        first.status.code(),
        Some(1),
        "the CR2 copy fails the first time"
    );

    let out = apply(&d, &["--shoot", "s"]);

    assert_ok(&out);
    let cr2 = d.dest.join("2024/03/05/s/p/o/cr2/txo_0001.cr2");
    assert_eq!(fs::read_to_string(cr2).unwrap(), "A.CR2");
}

#[test]
fn a_shoot_log_that_cannot_be_read_refuses_the_plan() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    assert_ok(&apply(&d, &["--shoot", "s"]));
    let log = d.dest.join("2024/03/05/s/exportx.log");
    fs::set_permissions(&log, fs::Permissions::from_mode(0o000)).unwrap();

    let out = exportpro_with(
        &[("LANG", "fr_FR.UTF-8")],
        &plan_args(&d, &["--shoot", "s"]),
    );

    fs::set_permissions(&log, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    assert!(stderr(&out).contains("exportx.log"), "{}", stderr(&out));
}

#[test]
fn an_existing_shoot_folder_that_cannot_be_written_is_refused_when_planning() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    let shoot = d.dest.join("2024/03/05/s");
    fs::create_dir_all(&shoot).unwrap();
    fs::set_permissions(&shoot, fs::Permissions::from_mode(0o555)).unwrap();

    let out = plan(&d, &["--shoot", "s"]);

    fs::set_permissions(&shoot, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    assert!(stderr(&out).contains("cannot write to"), "{}", stderr(&out));
}

#[test]
fn no_lock_file_is_left_after_a_copy() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);

    assert_ok(&apply(&d, &["--shoot", "s"]));

    assert!(!d.dest.join("2024/03/05/s/.exportx.lock").exists());
}

#[test]
fn a_copy_changed_since_is_not_counted_as_already_copied() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    assert_ok(&apply(&d, &["--shoot", "s"]));
    // Rotated in a viewer: the copy no longer matches the original.
    fs::write(d.dest.join("2024/03/05/s/p/o/jpg/txo_0001.jpg"), "rotated").unwrap();

    let out = apply(&d, &["--shoot", "s"]);

    assert_ok(&out);
    let again = d.dest.join("2024/03/05/s/p/o/jpg/txo_0002.jpg");
    assert_eq!(fs::read_to_string(again).unwrap(), "a.jpg");
}
