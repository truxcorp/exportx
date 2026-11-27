//! CLI parity for auto-update (ADR 0007): the version, and whether a newer one is out.
mod common;
use common::*;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A local update endpoint answering every request with `latest.json` = `body`.
fn serve(body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/latest.json", listener.local_addr().unwrap());
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
                line.clear();
            }
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    url
}

/// A URL nothing listens on.
fn closed_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    format!("http://{}/latest.json", listener.local_addr().unwrap())
}

#[test]
fn version_prints_the_version() {
    let out = exportpro(&["--version"]);

    assert_ok(&out);
    assert_eq!(stdout(&out).trim(), format!("exportx {VERSION}"));
}

#[test]
fn update_check_tells_a_newer_version_and_its_page() {
    let url = serve(r#"{"version":"99.0.0","notes":"","platforms":{}}"#);

    let out = exportpro_with(
        &[("EXPORTX_UPDATE_URL", &url)],
        &["update-check", "--lang", "en"],
    );

    assert_ok(&out);
    let text = stdout(&out);
    assert!(text.contains("ExportX 99.0.0 is available"), "{text}");
    assert!(
        text.contains("https://github.com/truxcorp/exportx/releases/tag/v99.0.0"),
        "{text}"
    );
}

#[test]
fn update_check_says_up_to_date_when_no_newer_version() {
    let url = serve(r#"{"version":"0.0.1","platforms":{}}"#);

    let out = exportpro_with(
        &[("EXPORTX_UPDATE_URL", &url)],
        &["update-check", "--lang", "fr"],
    );

    assert_ok(&out);
    let text = stdout(&out);
    assert!(
        text.contains(&format!("ExportX {VERSION} est à jour")),
        "{text}"
    );
}

#[test]
fn update_check_fails_without_network() {
    let url = closed_url();

    let out = exportpro_with(
        &[("EXPORTX_UPDATE_URL", &url)],
        &["update-check", "--lang", "en"],
    );

    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains("could not check for a new version"),
        "{}",
        stderr(&out)
    );
}
