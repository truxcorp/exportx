//! Slice 5: edge cases a copy-then-erase tool must not get quietly wrong.
mod common;
use common::*;

use std::fs;

#[test]
fn a_symlinked_file_in_the_source_is_copied_like_the_others() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    let elsewhere = d.home.join("real");
    mk(&elsewhere, "2024-03-05 09:05", &["b.jpg"]);
    std::os::unix::fs::symlink(elsewhere.join("b.jpg"), d.src.join("b.jpg")).unwrap();

    let out = plan(&d, &["--shoot", "s"]);

    assert_ok(&out);
    assert_eq!(
        moves(&out),
        [
            "a.jpg -> 2024/03/05/s/p/o/jpg/txo_0001.jpg",
            "b.jpg -> 2024/03/05/s/p/o/jpg/txo_0002.jpg",
        ]
    );
}

#[test]
fn a_file_whose_name_cannot_be_read_is_reported_not_skipped_quietly() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);
    fs::write(
        d.src.join(OsStr::from_bytes(b"caf\xe9.jpg")),
        "latin-1 name",
    )
    .unwrap();

    let out = plan(&d, &["--shoot", "s"]);

    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    assert!(stderr(&out).contains("caf"), "stderr: {}", stderr(&out));
    assert_eq!(moves(&out), Vec::<String>::new());
}

#[test]
fn a_shoot_folder_that_cannot_be_read_refuses_the_plan() {
    use std::os::unix::fs::PermissionsExt;
    let d = Dirs::new();
    let shoot = d.dest.join("2024/03/05/s");
    mk(
        &shoot.join("p/o/jpg"),
        "2024-03-05 08:00",
        &["txo_0001.jpg"],
    );
    fs::set_permissions(&shoot, fs::Permissions::from_mode(0o000)).unwrap();
    mk(&d.src, "2024-03-05 09:00", &["a.jpg"]);

    let out = plan(&d, &["--shoot", "s"]);

    fs::set_permissions(&shoot, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    assert!(
        stderr(&out).contains("2024/03/05/s"),
        "stderr: {}",
        stderr(&out)
    );
}
