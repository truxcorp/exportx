//! The CLI shows the same progress line as the app, on a terminal only. Linux only: the
//! test drives a terminal through util-linux `script` (BSD's takes other options).
#![cfg(target_os = "linux")]
mod common;
use common::*;

use std::process::Command;

#[test]
fn apply_shows_the_progress_line_on_a_terminal_and_not_in_a_pipe() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 10:00", &["a.jpg"]);
    let (src, dest) = (d.src.to_str().unwrap(), d.dest.to_str().unwrap());
    let args = [
        "apply",
        "--source",
        src,
        "--dest",
        dest,
        "--shoot",
        "s",
        "--date",
        "2024-03-05",
    ];

    let piped = exportpro(&args);
    assert!(piped.status.success(), "{}", stderr(&piped));
    assert!(!stderr(&piped).contains("1 / 1 file"), "{}", stderr(&piped));

    // `script` runs the command on a pseudo-terminal and prints what it shows.
    // Each word quoted for the shell `script` hands the line to.
    let quote = |w: &str| format!("'{}'", w.replace('\'', r"'\''"));
    let words = std::iter::once(env!("CARGO_BIN_EXE_exportpro")).chain(args);
    let line = words.map(quote).collect::<Vec<_>>().join(" ");
    let config = tempfile::tempdir().unwrap();
    std::fs::remove_dir_all(&d.dest).unwrap();
    let shown = Command::new("script")
        .args(["-qec", &line, "/dev/null"])
        .env("TZ", "UTC")
        .env("LANG", "en_US.UTF-8")
        .env("XDG_CONFIG_HOME", config.path())
        .output()
        .unwrap();
    let screen = String::from_utf8_lossy(&shown.stdout);
    assert!(shown.status.success(), "{screen}");
    assert!(screen.contains("1 / 1 file · 5 B / 5 B"), "{screen}");
}
