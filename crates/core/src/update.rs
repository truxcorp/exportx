//! Auto-update (ADR 0007): is a newer release out? Shared by the app and the CLI.

use semver::Version;

fn parse(version: &str) -> Option<Version> {
    Version::parse(version.trim().trim_start_matches('v')).ok()
}

/// Whether `latest` is a newer release than `current` (semver; never for an unreadable one).
pub fn newer(current: &str, latest: &str) -> bool {
    match (parse(current), parse(latest)) {
        (Some(current), Some(latest)) => latest > current,
        _ => false,
    }
}

/// The GitHub page of the release `version`.
pub fn release_page(version: &str) -> String {
    format!(
        "https://github.com/truxcorp/exportx/releases/tag/v{}",
        version.trim_start_matches('v')
    )
}
