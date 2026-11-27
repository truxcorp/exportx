//! The capture-date reader must never take the scan down with it.

use std::path::Path;

use exportpro_core::dates::capture_date_with;

#[test]
fn a_panic_in_the_metadata_library_gives_no_date_instead_of_a_crash() {
    let date = capture_date_with(Path::new("broken.cr2"), |_| panic!("library bug"));

    assert_eq!(date, None);
}

use exportpro_core::dates::{CaptureDate, read_capture_date_with};

#[test]
fn a_crash_of_the_library_is_told_apart_from_a_missing_date() {
    let crashed = read_capture_date_with(Path::new("odd.cr2"), |_| panic!("library bug"));
    let missing = read_capture_date_with(Path::new("plain.jpg"), |_| {
        Err(nom_exif::Error::UnsupportedFormat)
    });

    assert_eq!(crashed, CaptureDate::Unreadable);
    assert_eq!(missing, CaptureDate::Missing);
}

/// Runs in a child process (see below): a guarded crash, nothing else.
#[test]
fn child_guarded_crash() {
    if std::env::var_os("EXPORTPRO_CHILD").is_some() {
        read_capture_date_with(Path::new("odd.cr2"), |_| panic!("library bug"));
    }
}

#[test]
fn a_crash_of_the_library_prints_no_panic_trace() {
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "child_guarded_crash",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("EXPORTPRO_CHILD", "1")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert!(!stderr.contains("library bug"), "{stderr}");
}

#[test]
fn a_file_the_library_crashes_on_is_flagged_and_dated_by_its_file_time() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("odd.cr2"), "x").unwrap();

    let groups = exportpro_core::scan::scan_with(dir.path(), |_| panic!("library bug")).unwrap();

    let file = &groups[0].files[0];
    assert!(file.date_unreadable);
}

#[test]
fn the_plan_marks_the_files_the_library_crashed_on() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("odd.cr2"), "x").unwrap();
    let groups = exportpro_core::scan::scan_with(dir.path(), |_| panic!("library bug")).unwrap();
    let req = exportpro_core::Request {
        dest: dir.path().join("dest"),
        shoot: "s".into(),
        prefix: "txo".into(),
        date: exportpro_core::DateChoice::NoDate,
        layout: exportpro_core::Layout::default(),
    };

    let plan = exportpro_core::plan(&groups, &req).unwrap();

    assert!(plan.entries[0].date_unreadable);
}

#[test]
fn a_restore_that_cannot_be_logged_is_explained_in_french() {
    use exportpro_core::erase::Keep;
    use exportpro_core::i18n::IoReason;

    let reason = Keep::RestoredNotLogged {
        log: "/photos/s/exportpro.log".into(),
        error: IoReason {
            kind: std::io::ErrorKind::PermissionDenied,
            text: "Permission denied (os error 13)".into(),
        },
    };

    let text = exportpro_core::Lang::Fr.keep_reason(&reason);

    assert!(text.contains("permission refusée"), "{text}");
}
