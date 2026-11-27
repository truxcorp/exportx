//! Behaviours 9 and 10: copy, verify and log (ported from tests/test_exportpro.sh, the
//! move checks of cases 1-3, now copies).
mod common;
use common::*;

use std::fs;
use std::path::Path;

use serde_json::Value;
use sha2::{Digest, Sha256};

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn log_lines(shoot: &Path) -> Vec<Value> {
    fs::read_to_string(shoot.join("exportx.log"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{e}: {l}")))
        .collect()
}

fn content(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn empty_dir(path: &Path) -> bool {
    path.is_dir() && fs::read_dir(path).unwrap().next().is_none()
}

#[test]
fn files_are_copied_verified_and_logged_and_originals_stay() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["A.CR2", "A.JPG"]);
    mk(&d.src, "2024-03-05 09:05", &["MVI_1.MOV", "MVI_1.THM"]);

    let out = apply(&d, &["--shoot", "concert"]);

    assert_ok(&out);
    let shoot = d.dest.join("2024/03/05/concert");
    let copies = [
        ("A.CR2", "p/o/cr2/txo_0001.cr2"),
        ("A.JPG", "p/o/jpg/txo_0001.jpg"),
        ("MVI_1.MOV", "v/o/mov/txo_0002.mov"),
        ("MVI_1.THM", "v/o/thm/txo_0002.thm"),
    ];
    for (original, copy) in copies {
        assert_eq!(content(&shoot.join(copy)), original, "{copy}");
        assert_eq!(
            content(&d.src.join(original)),
            original,
            "original {original} changed"
        );
    }

    let log = log_lines(&shoot);
    assert_eq!(log.len(), copies.len(), "{log:?}");
    for ((original, copy), line) in copies.iter().zip(&log) {
        assert_eq!(line["v"], 1, "{line}");
        assert_eq!(line["event"], "copied", "{line}");
        assert_eq!(
            line["source"],
            d.src.join(original).to_str().unwrap(),
            "{line}"
        );
        assert_eq!(line["dest"], *copy, "{line}");
        assert_eq!(line["size"], original.len(), "{line}");
        assert_eq!(line["sha256"], sha256_hex(original.as_bytes()), "{line}");
        assert_eq!(
            line["layout"]["shoot"], "{yyyy}/{mm}/{dd}/{shoot}",
            "{line}"
        );
        assert_eq!(
            line["layout"]["original"], "{kind}/o/{ext}/{prefix}_{n:4}.{ext}",
            "{line}"
        );
        assert_eq!(line["layout"]["photo"], "p", "{line}");
        assert_eq!(line["layout"]["video"], "v", "{line}");
    }
}

#[test]
fn empty_folders_for_edited_files_are_created() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["A.CR2", "A.JPG"]);
    mk(&d.src, "2024-03-05 09:05", &["MVI_1.MOV", "MVI_1.THM"]);
    mk(&d.src, "2024-03-05 09:10", &["clip.mp4"]);

    let out = apply(&d, &["--shoot", "concert"]);

    assert_ok(&out);
    let shoot = d.dest.join("2024/03/05/concert");
    assert!(empty_dir(&shoot.join("p/s/1/jpg")));
    assert!(empty_dir(&shoot.join("v/s/1/mov")));
    assert!(empty_dir(&shoot.join("v/s/1/mp4")));
    assert!(!shoot.join("v/s/1/thm").exists());
}

#[test]
fn photos_only_get_no_video_tree() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);

    let out = apply(&d, &["--shoot", "s", "--preset", "lisible"]);

    assert_ok(&out);
    let shoot = d.dest.join("2024/03/05/s");
    assert!(empty_dir(&shoot.join("photo/retouches/1/jpg")));
    assert!(!shoot.join("video").exists());
}

#[test]
fn a_refused_plan_copies_nothing() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    mk(&d.src, "2024-03-07 09:00", &["b.jpg"]);

    let out = apply(&d, &["--shoot", "s"]);

    assert_eq!(out.status.code(), Some(2));
    assert_eq!(fs::read_dir(&d.dest).unwrap().count(), 0);
}

#[test]
fn a_failed_copy_is_reported_and_logged_and_the_others_are_kept() {
    use std::os::unix::fs::PermissionsExt;
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    mk(&d.src, "2024-03-05 09:01", &["b.jpg"]);
    mk(&d.src, "2024-03-05 09:02", &["c.jpg"]);
    fs::set_permissions(d.src.join("b.jpg"), fs::Permissions::from_mode(0o000)).unwrap();

    let out = apply(&d, &["--shoot", "s"]);

    fs::set_permissions(d.src.join("b.jpg"), fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    assert!(stderr(&out).contains("b.jpg"), "stderr: {}", stderr(&out));
    let shoot = d.dest.join("2024/03/05/s");
    assert_eq!(content(&shoot.join("p/o/jpg/txo_0001.jpg")), "a.jpg");
    assert!(!shoot.join("p/o/jpg/txo_0002.jpg").exists());
    assert_eq!(content(&shoot.join("p/o/jpg/txo_0003.jpg")), "c.jpg");
    for name in ["a.jpg", "b.jpg", "c.jpg"] {
        assert_eq!(content(&d.src.join(name)), name);
    }
    let events: Vec<(String, String)> = log_lines(&shoot)
        .iter()
        .map(|l| {
            (
                l["event"].as_str().unwrap().to_owned(),
                l["dest"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        events,
        [
            ("copied".to_owned(), "p/o/jpg/txo_0001.jpg".to_owned()),
            ("failed".to_owned(), "p/o/jpg/txo_0002.jpg".to_owned()),
            ("copied".to_owned(), "p/o/jpg/txo_0003.jpg".to_owned()),
        ]
    );
}

#[test]
fn a_copy_that_cannot_be_logged_counts_as_failed_once() {
    use std::os::unix::fs::PermissionsExt;
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    let shoot = d.dest.join("2024/03/05/s");
    fs::create_dir_all(&shoot).unwrap();
    fs::write(shoot.join("exportpro.log"), "").unwrap();
    fs::set_permissions(
        shoot.join("exportpro.log"),
        fs::Permissions::from_mode(0o444),
    )
    .unwrap();

    let out = apply(&d, &["--shoot", "s"]);

    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("0 file(s) copied, 1 failed"),
        "stderr: {}",
        stderr(&out)
    );
}
