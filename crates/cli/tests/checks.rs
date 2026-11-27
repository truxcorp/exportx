//! Items 6-10 of the robustness audit: told or refused before the copy.
mod common;
use common::*;

use std::fs;

fn french_plan(d: &Dirs, extra: &[&str]) -> std::process::Output {
    exportpro_with(&[("LANG", "fr_FR.UTF-8")], &plan_args(d, extra))
}

#[test]
fn a_capture_date_from_a_camera_clock_never_set_is_flagged() {
    let d = Dirs::new();
    mk(&d.src, "2000-01-01 00:03", &["reset.jpg"]);

    let out = french_plan(&d, &["--shoot", "s"]);

    assert_ok(&out);
    assert!(
        stderr(&out).contains("reset.jpg : date de prise de vue suspecte (2000-01-01)"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn a_capture_date_in_the_future_is_flagged() {
    let d = Dirs::new();
    // A camera clock set in the future shows in EXIF (file times are capped by the
    // file's creation, which is now).
    fixture(&d.src, "future.jpg", "2024-03-05 09:00");

    let out = plan(&d, &["--shoot", "s", "--date", "2024-03-05"]);

    assert_ok(&out);
    assert!(
        stderr(&out).contains("future.jpg: suspicious capture date (2099-06-01)"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn an_ordinary_date_is_not_flagged() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);

    let out = plan(&d, &["--shoot", "s"]);

    assert_ok(&out);
    assert_eq!(stderr(&out), "");
}

#[test]
fn an_empty_media_file_is_flagged() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    fs::write(d.src.join("broken.jpg"), "").unwrap();
    set_mtime(&d.src.join("broken.jpg"), epoch("2024-03-05 09:01"));

    let out = french_plan(&d, &["--shoot", "s"]);

    assert_ok(&out);
    assert!(
        stderr(&out).contains("broken.jpg : fichier vide (0 octet), la carte est peut-être abîmée"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn a_name_longer_than_255_bytes_is_refused() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    let shoot = "é".repeat(130); // 260 bytes in UTF-8

    let out = plan(&d, &["--shoot", &shoot]);

    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    assert!(
        stderr(&out).contains("longer than 255 bytes"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn a_path_too_long_to_move_to_a_pc_is_refused() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    let shoot = "s".repeat(190); // 2024/03/05/<190>/p/o/jpg/txo_0001.jpg > 200

    let out = exportpro_with(
        &[("LANG", "fr_FR.UTF-8")],
        &plan_args(&d, &["--shoot", &shoot]),
    );

    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    assert!(
        stderr(&out).contains("chemin trop long"),
        "{}",
        stderr(&out)
    );
}

// Review of PR #23 (round 1).

#[test]
fn a_file_name_whose_part_would_be_too_long_is_refused() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    let prefix = "é".repeat(121); // txo-like name of 242 + "_0001.jpg" = 251 bytes; .part = 256

    let out = plan(
        &d,
        &[
            "--shoot",
            "s",
            "--prefix",
            &prefix,
            "--shoot-path",
            "{shoot}",
            "--file-path",
            "{prefix}_{n:4}.{ext}",
        ],
    );

    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    assert!(stderr(&out).contains("invalid name"), "{}", stderr(&out));
}

#[test]
fn path_length_counts_like_windows_and_names_every_cause() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    // 62 + 60 emoji: 147 characters, but 269 UTF-16 units as Windows counts them.
    let shoot = "📷".repeat(62);
    let prefix = "🎞".repeat(60);

    let out = plan(&d, &["--shoot", &shoot, "--prefix", &prefix]);

    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    let err = stderr(&out);
    assert!(err.contains("path too long"), "{err}");
    assert!(err.contains("prefix"), "{err}");
}

#[test]
fn the_path_message_shows_no_count_the_user_cannot_see() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    let shoot = "📷".repeat(62);
    let prefix = "🎞".repeat(60);

    let out = plan(&d, &["--shoot", &shoot, "--prefix", &prefix]);

    // 147 characters on screen, 273 UTF-16 units: neither number helps, so none is shown.
    let err = stderr(&out);
    assert!(err.contains("path too long"), "{err}");
    assert!(!err.contains("273") && !err.contains("147"), "{err}");
}
