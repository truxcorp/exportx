//! `apply --move`: the app's Move, in the terminal (ADR 0006).
mod common;
use common::*;

#[test]
fn apply_move_moves_on_the_same_disk_and_undo_puts_the_files_back() {
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 10:00", &["a.jpg", "b.jpg"]);
    // Next to the source: the same disk.
    let dest = d.src.parent().unwrap().join("photos");
    let (src, dest_s) = (d.src.to_str().unwrap(), dest.to_str().unwrap());
    let args = [
        "apply",
        "--source",
        src,
        "--dest",
        dest_s,
        "--shoot",
        "s",
        "--date",
        "2024-03-05",
        "--move",
    ];

    let out = exportpro(&args);

    assert!(out.status.success(), "{}", stderr(&out));
    let shoot = dest.join("2024/03/05/s");
    let text = stdout(&out);
    assert!(text.contains("a.jpg -> 2024/03/05/s/"), "{text}");
    // The command printed to put them back names this move (--run) and its shoot folder.
    let printed = text
        .lines()
        .find_map(|l| l.strip_prefix("2 file(s) moved; to put them back: exportx "))
        .unwrap_or_else(|| panic!("{text}"));
    let words: Vec<&str> = printed.split(' ').collect();
    assert_eq!(words.len(), 4, "{printed}");
    assert_eq!(&words[..2], ["undo", "--run"]);
    assert_eq!(words[3], shoot.to_str().unwrap());
    assert!(!d.src.join("a.jpg").exists());

    let undo = exportpro(&words);

    assert!(undo.status.success(), "{}", stderr(&undo));
    assert_eq!(
        std::fs::read_to_string(d.src.join("a.jpg")).unwrap(),
        "a.jpg"
    );
}

#[test]
fn apply_move_to_another_disk_is_refused_with_the_reason() {
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
        "--move",
    ];

    let out = exportpro(&args);

    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("the destination is on another disk than the source"),
        "{}",
        stderr(&out)
    );
    assert!(d.src.join("a.jpg").exists());
}

#[test]
fn a_partly_failed_move_says_what_moved_and_quotes_the_undo_command() {
    use std::os::unix::fs::PermissionsExt;
    let d = Dirs::new();
    mk(&d.src, "2024-03-05 10:00", &["a.jpg"]);
    // A link is copied, not moved: to a file nobody may read, its copy fails.
    let secret = d.src.parent().unwrap().join("secret.jpg");
    std::fs::write(&secret, "secret").unwrap();
    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o000)).unwrap();
    std::os::unix::fs::symlink(&secret, d.src.join("b.jpg")).unwrap();
    let dest = d.src.parent().unwrap().join("my photos");
    let (src, dest_s) = (d.src.to_str().unwrap(), dest.to_str().unwrap());
    let args = [
        "apply",
        "--source",
        src,
        "--dest",
        dest_s,
        "--shoot",
        "s",
        "--date",
        "2024-03-05",
        "--move",
    ];

    let out = exportpro(&args);

    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    let shoot = dest.join("2024/03/05/s");
    assert!(
        stdout(&out).contains("exportx undo --run "),
        "{}",
        stdout(&out)
    );
    assert!(
        stdout(&out).ends_with(&format!(" '{}'\n", shoot.display())),
        "{}",
        stdout(&out)
    );
    let err = stderr(&out);
    assert!(err.contains("1 file(s) moved, 1 failed"), "{err}");
    assert!(!err.contains("untouched"), "{err}");
}
