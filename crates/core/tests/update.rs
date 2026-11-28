//! Auto-update (ADR 0007): the version comparison the app and the CLI share.

use exportpro_core::update::{newer, release_page};

#[test]
fn a_higher_release_is_newer() {
    assert!(newer("0.1.0", "0.2.0"));
    assert!(newer("0.9.9", "1.0.0"));
    assert!(newer("1.2.3", "1.2.10"), "numbers, not text");
}

#[test]
fn the_same_or_an_older_release_is_not_newer() {
    assert!(!newer("1.2.3", "1.2.3"));
    assert!(!newer("1.2.3", "1.2.2"));
    assert!(!newer("2.0.0", "1.9.9"));
}

#[test]
fn a_pre_release_comes_before_its_release() {
    assert!(newer("1.2.3-rc.1", "1.2.3"));
    assert!(!newer("1.2.3", "1.2.3-rc.1"));
    assert!(newer("1.2.3-rc.1", "1.2.3-rc.2"));
}

#[test]
fn a_leading_v_is_accepted() {
    assert!(newer("0.1.0", "v0.2.0"));
}

#[test]
fn an_unreadable_version_is_never_newer() {
    assert!(!newer("0.1.0", "latest"));
    assert!(!newer("0.1.0", ""));
    assert!(!newer("dev", "0.2.0"));
}

#[test]
fn the_release_page_is_the_tag_on_github() {
    assert_eq!(
        release_page("0.2.0"),
        "https://github.com/truxcorp/exportx/releases/tag/v0.2.0"
    );
}
