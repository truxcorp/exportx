//! A destination that fills up during the copy: stop at once, don't try every file.

use std::fs;
use std::io;

use exportpro_core::apply::{CopyFailure, apply_using, copy_verified};
use exportpro_core::{DateChoice, Layout, Request};

#[test]
fn the_copy_stops_at_the_first_disk_full_and_lists_the_rest_as_not_copied() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("card");
    fs::create_dir_all(&src).unwrap();
    for name in ["a.jpg", "b.jpg", "c.jpg", "d.jpg"] {
        fs::write(src.join(name), name).unwrap();
    }
    let req = Request {
        dest: dir.path().join("photos"),
        shoot: "s".into(),
        prefix: "txo".into(),
        date: DateChoice::NoDate,
        layout: Layout::default(),
    };
    let plan = exportpro_core::plan(&exportpro_core::scan(&src).unwrap(), &req).unwrap();
    let mut tried = Vec::new();

    // The disk is "full" from the second file on.
    let outcome = apply_using(
        &plan,
        &req,
        |from, to| {
            tried.push(from.file_name().unwrap().to_owned());
            if tried.len() == 1 {
                copy_verified(from, to)
            } else {
                Err(io::Error::from(io::ErrorKind::StorageFull))
            }
        },
        |_, _| {},
    );

    assert_eq!(
        tried.len(),
        2,
        "no copy tried after the disk filled: {tried:?}"
    );
    assert_eq!(outcome.copied.len(), 1);
    // The files only: the folders for edited files are listed too since the #21 review.
    let failures: Vec<&CopyFailure> = outcome
        .failed
        .iter()
        .filter(|(e, _)| e.name.ends_with(".jpg"))
        .map(|(_, f)| f)
        .collect();
    assert!(matches!(failures[0], CopyFailure::Copy(r) if r.kind == io::ErrorKind::StorageFull));
    assert_eq!(
        failures[1..],
        [
            &CopyFailure::NotCopiedDiskFull,
            &CopyFailure::NotCopiedDiskFull
        ]
    );
}

// Review of PR #21.

fn card_and_plan(dir: &std::path::Path) -> (exportpro_core::Plan, Request) {
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

#[test]
fn edited_folders_not_made_because_the_disk_filled_are_listed() {
    let dir = tempfile::tempdir().unwrap();
    let (plan, req) = card_and_plan(dir.path());

    let outcome = apply_using(
        &plan,
        &req,
        |_, _| Err(io::Error::from(io::ErrorKind::StorageFull)),
        |_, _| {},
    );

    let folders: Vec<_> = outcome
        .failed
        .iter()
        .filter(|(e, _)| e.name == "s/p/s/1/jpg")
        .map(|(_, f)| f)
        .collect();
    assert_eq!(folders, [&CopyFailure::NotCopiedDiskFull]);
}

#[test]
fn a_full_disk_met_while_logging_also_stops_the_copy() {
    let dir = tempfile::tempdir().unwrap();
    let (plan, req) = card_and_plan(dir.path());
    let shoot = req.dest.join("s");
    fs::create_dir_all(&shoot).unwrap();
    // Every write to /dev/full fails with "no space left on device".
    std::os::unix::fs::symlink("/dev/full", shoot.join("exportpro.log")).unwrap();
    let mut tried = 0;

    let outcome = apply_using(
        &plan,
        &req,
        |from, to| {
            tried += 1;
            copy_verified(from, to)
        },
        |_, _| {},
    );

    assert_eq!(tried, 1, "no copy tried after the log met a full disk");
    let not_copied = outcome
        .failed
        .iter()
        .filter(|(_, f)| *f == CopyFailure::NotCopiedDiskFull)
        .count();
    assert!(not_copied >= 2, "{:?}", outcome.failed);
}

#[test]
fn an_exceeded_quota_counts_as_a_full_disk() {
    let dir = tempfile::tempdir().unwrap();
    let (plan, req) = card_and_plan(dir.path());
    let mut tried = 0;

    apply_using(
        &plan,
        &req,
        |_, _| {
            tried += 1;
            Err(io::Error::from(io::ErrorKind::QuotaExceeded))
        },
        |_, _| {},
    );

    assert_eq!(tried, 1);
}

#[test]
fn a_relative_destination_not_made_yet_is_measured_from_the_current_folder() {
    let dir = tempfile::tempdir().unwrap();
    let (_, mut req) = card_and_plan(dir.path());
    req.dest = format!("not-made-yet-{}", std::process::id()).into();
    let groups = exportpro_core::scan(&dir.path().join("card")).unwrap();

    let plan = exportpro_core::plan(&groups, &req);

    assert!(plan.is_ok(), "{:?}", plan.err());
}

#[test]
fn sizes_just_under_a_unit_round_into_the_next() {
    assert_eq!(exportpro_core::Lang::En.size(999_960), "1.0 MB");
    assert_eq!(exportpro_core::Lang::Fr.size(999_960_000), "1,0 Go");
    assert_eq!(exportpro_core::Lang::En.size(999), "999 B");
}
