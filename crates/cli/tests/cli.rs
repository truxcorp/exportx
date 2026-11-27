//! The command line itself.
mod common;
use common::*;

#[test]
fn an_unknown_command_fails() {
    let out = exportpro(&["paln"]);

    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("usage"), "stderr: {}", stderr(&out));
}

#[test]
fn no_command_prints_the_usage() {
    let out = exportpro(&[]);

    assert_ok(&out);
    assert!(stdout(&out).contains("usage"), "stdout: {}", stdout(&out));
}

#[test]
fn help_lists_the_video_formats_and_the_dated_ones() {
    let out = exportpro(&["--help"]);

    let text = stdout(&out);
    assert!(
        text.contains("Videos (video folder): mov, mp4, m4v, avi,"),
        "{text}"
    );
    assert!(
        text.contains("Every other file is copied as a photo,"),
        "{text}"
    );
    assert!(
        text.contains("a file sharing its base name with a video (IMG_1.THM next to IMG_1.MOV) goes with the video"),
        "{text}"
    );
    assert!(
        text.contains("Files without an extension are skipped."),
        "{text}"
    );
    assert!(
        text.contains("Capture date checked on: jpg, heic, cr2, cr3, nef, arw, dng, mov, mp4"),
        "{text}"
    );
    assert!(
        text.contains("other files: the date stored in them if found, else their file date"),
        "{text}"
    );
}
