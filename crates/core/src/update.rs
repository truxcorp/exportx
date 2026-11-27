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

/// Where the latest release is described (the updater's `latest.json`, ADR 0007).
pub const ENDPOINT: &str =
    "https://github.com/truxcorp/exportx/releases/latest/download/latest.json";

/// The update endpoint: `EXPORTX_UPDATE_URL` when set (tests, a mirror), else [`ENDPOINT`].
/// An install is still checked against the app's update key, whatever the endpoint.
pub fn endpoint() -> String {
    std::env::var("EXPORTX_UPDATE_URL")
        .ok()
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| ENDPOINT.to_owned())
}

/// The latest release's version, read from the `latest.json` at `url`; why not otherwise.
pub fn latest(url: &str) -> Result<String, String> {
    #[derive(serde::Deserialize)]
    struct Manifest {
        version: String,
    }
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(15)))
        .build()
        .into();
    let text = agent
        .get(url)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    let manifest: Manifest = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let version = manifest.version.trim().trim_start_matches('v').to_owned();
    parse(&version)
        .map(|_| version)
        .ok_or_else(|| format!("unreadable version {:?}", manifest.version))
}

/// The newer release's version if one is out, compared with `current`.
pub fn check(url: &str, current: &str) -> Result<Option<String>, String> {
    let latest = latest(url)?;
    Ok(newer(current, &latest).then_some(latest))
}
