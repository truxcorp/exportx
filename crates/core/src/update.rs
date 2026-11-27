//! Auto-update (ADR 0007): is a newer release out? Shared by the app and the CLI.

/// Whether `latest` is a newer release than `current` (semver; never for an unreadable one).
pub fn newer(_current: &str, _latest: &str) -> bool {
    false
}

/// The GitHub page of the release `version`.
pub fn release_page(_version: &str) -> String {
    String::new()
}
