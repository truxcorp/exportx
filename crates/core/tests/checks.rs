//! Items 8 and 9 of the robustness audit, at the core's level.

use std::fs;
use std::io;
use std::path::Path;

use exportpro_core::apply::{CopyFailure, apply_using, copy_verified};
use exportpro_core::erase::mount_fstype_for;
use exportpro_core::plan::{FAT32_MAX, too_big_for};
use exportpro_core::{DateChoice, Layout, Request};

fn setup(dir: &Path) -> (exportpro_core::Plan, Request) {
    let src = dir.join("card");
    fs::create_dir_all(&src).unwrap();
    for name in ["a.jpg", "b.jpg", "c.jpg"] {
        fs::write(src.join(name), name).unwrap();
    }
    let req = Request {
        dest: dir.join("photos"),
        shoot: "s".into(),
        prefix: "txo".into(),
        date: DateChoice::NoDate,
        layout: Layout::default(),
    };
    let plan = exportpro_core::plan(&exportpro_core::scan(&src).unwrap(), &req).unwrap();
    (plan, req)
}

fn files_only(outcome: &exportpro_core::Outcome) -> Vec<&CopyFailure> {
    outcome
        .failed
        .iter()
        .filter(|(e, _)| e.name.ends_with(".jpg"))
        .map(|(_, f)| f)
        .collect()
}

#[test]
fn a_card_removed_midway_stops_the_copy() {
    let dir = tempfile::tempdir().unwrap();
    let (plan, req) = setup(dir.path());
    let card = dir.path().join("card");
    let mut tried = 0;

    let outcome = apply_using(
        &plan,
        &req,
        |from, to| {
            tried += 1;
            if tried == 1 {
                return copy_verified(from, to);
            }
            fs::remove_dir_all(&card).unwrap(); // the card is pulled out
            Err(io::Error::from(io::ErrorKind::NotFound))
        },
        |_, _| {},
    );

    assert_eq!(tried, 2, "no copy tried once the card is gone");
    assert_eq!(
        files_only(&outcome)[1..],
        [&CopyFailure::NotCopiedSourceGone]
    );
}

#[test]
fn a_destination_unplugged_midway_stops_the_copy() {
    let dir = tempfile::tempdir().unwrap();
    let (plan, req) = setup(dir.path());
    let dest = req.dest.clone();
    let mut tried = 0;

    let outcome = apply_using(
        &plan,
        &req,
        |from, to| {
            tried += 1;
            if tried == 1 {
                return copy_verified(from, to);
            }
            fs::remove_dir_all(&dest).unwrap(); // the disk is unplugged
            Err(io::Error::from(io::ErrorKind::NotFound))
        },
        |_, _| {},
    );

    assert_eq!(tried, 2, "no copy tried once the destination is gone");
    assert_eq!(files_only(&outcome)[1..], [&CopyFailure::NotCopiedDestGone]);
}

#[test]
fn files_over_4_gib_do_not_fit_on_fat32_only() {
    assert!(too_big_for("vfat", FAT32_MAX + 1));
    assert!(too_big_for("msdos", 5_000_000_000));
    assert!(!too_big_for("vfat", FAT32_MAX));
    assert!(!too_big_for("exfat", 50_000_000_000));
    assert!(!too_big_for("ext4", 50_000_000_000));
}

#[test]
fn the_file_system_of_a_folder_is_found_by_mount_point() {
    let mountinfo = "\
29 1 0:26 /root / rw shared:1 - btrfs /dev/nvme0n1p3 rw
31 29 8:17 / /media/stick rw shared:3 - vfat /dev/sdb1 rw
";
    let fs_of = |p: &str| mount_fstype_for(mountinfo, Path::new(p));
    assert_eq!(fs_of("/media/stick/photos").as_deref(), Some("vfat"));
    assert_eq!(fs_of("/home/me").as_deref(), Some("btrfs"));
}

// Review of PR #23 (round 1).

#[test]
fn an_unplugged_destination_is_never_rebuilt() {
    let dir = tempfile::tempdir().unwrap();
    let (plan, req) = setup(dir.path());
    let dest = req.dest.clone();
    let mut tried = 0;

    apply_using(
        &plan,
        &req,
        |from, to| {
            tried += 1;
            if tried == 1 {
                return copy_verified(from, to);
            }
            fs::remove_dir_all(&dest).unwrap();
            Err(io::Error::from(io::ErrorKind::NotFound))
        },
        |_, _| {},
    );

    assert!(
        !dest.exists(),
        "the destination folder was made again (log or folders)"
    );
}

#[test]
fn a_removed_card_still_gets_its_edited_folders_made() {
    let dir = tempfile::tempdir().unwrap();
    let (plan, req) = setup(dir.path());
    let card = dir.path().join("card");
    let mut tried = 0;

    let outcome = apply_using(
        &plan,
        &req,
        |from, to| {
            tried += 1;
            if tried == 1 {
                return copy_verified(from, to);
            }
            fs::remove_dir_all(&card).unwrap();
            Err(io::Error::from(io::ErrorKind::NotFound))
        },
        |_, _| {},
    );

    assert!(req.dest.join("s/p/s/1/jpg").is_dir());
    assert!(
        !outcome.failed.iter().any(|(e, _)| e.name == "s/p/s/1/jpg"),
        "{:?}",
        outcome.failed
    );
}

#[test]
fn a_card_emptied_at_its_mount_point_counts_as_removed() {
    let dir = tempfile::tempdir().unwrap();
    let (plan, req) = setup(dir.path());
    let card = dir.path().join("card");
    let mut tried = 0;

    let outcome = apply_using(
        &plan,
        &req,
        |from, to| {
            tried += 1;
            if tried == 1 {
                return copy_verified(from, to);
            }
            // Unmounted: the mount-point folder stays, empty.
            for e in fs::read_dir(&card).unwrap() {
                fs::remove_file(e.unwrap().path()).unwrap();
            }
            Err(io::Error::from(io::ErrorKind::NotFound))
        },
        |_, _| {},
    );

    assert_eq!(tried, 2, "no copy tried once the card is gone");
    assert_eq!(
        files_only(&outcome)[1..],
        [&CopyFailure::NotCopiedSourceGone]
    );
}

#[test]
fn edited_folders_get_the_same_length_checks_as_files() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("card");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.jpg"), "a").unwrap();
    let layout = Layout {
        edited_photo: vec![
            exportpro_core::Template::parse(
                "{kind}/retouches-finales-pour-le-client/{prefix}/{prefix}/jpg",
            )
            .unwrap(),
        ],
        ..Layout::default()
    };
    let req = Request {
        dest: dir.path().join("photos"),
        shoot: "s".into(),
        prefix: "p".repeat(80),
        date: DateChoice::NoDate,
        layout,
    };

    let plan = exportpro_core::plan(&exportpro_core::scan(&src).unwrap(), &req);

    assert!(
        matches!(plan, Err(exportpro_core::PlanError::PathTooLong { .. })),
        "{:?}",
        plan.map(|p| p.edited)
    );
}

// Review of PR #23 (round 2).

#[test]
fn card_and_destination_gone_together_rebuild_nothing_and_blame_the_destination() {
    let dir = tempfile::tempdir().unwrap();
    let (plan, req) = setup(dir.path());
    let (card, dest) = (dir.path().join("card"), req.dest.clone());
    let mut tried = 0;

    let outcome = apply_using(
        &plan,
        &req,
        |from, to| {
            tried += 1;
            if tried == 1 {
                return copy_verified(from, to);
            }
            // One hub unplugged: both go.
            fs::remove_dir_all(&card).unwrap();
            fs::remove_dir_all(&dest).unwrap();
            Err(io::Error::from(io::ErrorKind::NotFound))
        },
        |_, _| {},
    );

    assert!(
        !dest.exists(),
        "a log or folder was made where the destination was"
    );
    let folder = outcome
        .failed
        .iter()
        .find(|(e, _)| e.name == "s/p/s/1/jpg")
        .map(|(_, f)| f);
    assert_eq!(folder, Some(&CopyFailure::NotCopiedDestGone));
}

#[test]
fn a_device_that_stops_answering_stops_the_copy() {
    let dir = tempfile::tempdir().unwrap();
    let (plan, req) = setup(dir.path());
    let mut tried = 0;

    let outcome = apply_using(
        &plan,
        &req,
        |from, to| {
            tried += 1;
            if tried == 1 {
                return copy_verified(from, to);
            }
            Err(io::Error::from_raw_os_error(5)) // EIO: pulled out while still mounted
        },
        |_, _| {},
    );

    assert_eq!(tried, 2, "no copy tried once the device stopped answering");
    assert_eq!(
        files_only(&outcome)[1..],
        [&CopyFailure::NotCopiedDeviceGone]
    );
}
