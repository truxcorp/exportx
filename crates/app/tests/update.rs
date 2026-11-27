//! Auto-update at the app's command layer (ADR 0007): what the startup check and
//! Options → "Check now" offer, by how the app was installed.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread;

use exportpro_app::commands::{Bundle, UpdateOffer, check_update};

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

const NEWER: &str = r#"{"version":"2.0.0","platforms":{}}"#;

#[test]
fn u1_an_appimage_or_a_mac_app_is_offered_the_update() {
    let url = serve(NEWER);

    for bundle in [Bundle::AppImage, Bundle::MacApp] {
        assert_eq!(
            check_update(&url, "1.0.0", bundle),
            Ok(Some(UpdateOffer::Install {
                version: "2.0.0".into()
            }))
        );
    }
}

#[test]
fn u3_a_deb_or_another_install_gets_the_release_page() {
    let url = serve(NEWER);

    assert_eq!(
        check_update(&url, "1.0.0", Bundle::Other),
        Ok(Some(UpdateOffer::Link {
            version: "2.0.0".into(),
            page: "https://github.com/truxcorp/exportx/releases/tag/v2.0.0".into(),
        }))
    );
}

#[test]
fn no_offer_when_the_app_is_up_to_date() {
    let url = serve(NEWER);

    assert_eq!(check_update(&url, "2.0.0", Bundle::AppImage), Ok(None));
}

#[test]
fn u4_no_network_is_an_error_the_screen_decides_to_show() {
    let closed = {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}/latest.json", listener.local_addr().unwrap())
    };

    assert!(check_update(&closed, "1.0.0", Bundle::AppImage).is_err());
}
