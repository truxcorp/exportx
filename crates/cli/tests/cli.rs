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
