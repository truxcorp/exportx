//! Not enough room at the destination: refused before anything is copied.
mod common;
use common::*;

use std::fs;

/// A sparse file: `len` bytes long, almost no room on disk.
fn huge(dir: &std::path::Path, name: &str, len: u64) {
    fs::create_dir_all(dir).unwrap();
    let f = fs::File::create(dir.join(name)).unwrap();
    f.set_len(len).unwrap();
}

#[test]
fn a_copy_that_does_not_fit_is_refused_with_the_space_missing() {
    let d = Dirs::new();
    huge(&d.src, "big.mov", 8_000_000_000_000); // 8 TB, sparse (ext4 caps files at 16 TiB)
    // Under /proc nothing can be written, not even by root: should the check ever be
    // missing, the copy fails at once instead of writing 8 TB of zeros.
    let src = d.src.to_str().unwrap();
    let args = [
        "apply",
        "--source",
        src,
        "--dest",
        "/proc/exportpro-test",
        "--shoot",
        "s",
        "--date",
        "2024-03-05",
    ];

    let out = exportpro_with(&[("LANG", "fr_FR.UTF-8")], &args);

    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    let err = stderr(&out);
    assert!(
        err.contains("Pas assez de place sur la destination : il manque"),
        "{err}"
    );
    assert!(err.contains("8,0 To nécessaires"), "{err}");
    assert!(err.contains("libres"), "{err}");
}

#[test]
fn the_preview_already_refuses_it_in_english_too() {
    let d = Dirs::new();
    huge(&d.src, "big.mov", 8_000_000_000_000);

    let out = plan(&d, &["--shoot", "s", "--date", "2024-03-05"]);

    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    let err = stderr(&out);
    assert!(err.contains("Not enough room at the destination:"), "{err}");
    assert!(err.contains("missing (8.0 TB needed"), "{err}");
}
