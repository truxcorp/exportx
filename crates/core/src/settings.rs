//! What the app and the CLI remember between runs (behaviour 15): the last destination,
//! prefix and layout preset. Stored as TOML in the OS config folder.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub dest: Option<PathBuf>,
    pub prefix: Option<String>,
    pub preset: Option<String>,
    /// "fr" or "en"; unset follows the system.
    pub lang: Option<String>,
    /// The app looks for a new version at startup; unset means yes.
    pub check_updates: Option<bool>,
}

/// `<config folder>/settings.toml`, e.g. `~/.config/exportx/settings.toml` on Linux. The
/// first time, the settings ExportPro saved (`~/.config/exportpro/`) are copied there.
pub fn path() -> Option<PathBuf> {
    let at = |name| {
        directories::ProjectDirs::from("", "", name).map(|d| d.config_dir().join("settings.toml"))
    };
    let path = at("ExportX")?;
    // Once: an ExportX folder means ExportX ran here (settings deleted since = a reset).
    if let Some(old) = at("ExportPro")
        && let Some(folder) = path.parent()
        && !folder.exists()
        && old.is_file()
    {
        // Copied under another name, then renamed: never a half-written settings file.
        // Not copied: the defaults then, as with no settings at all.
        let part = folder.join(format!(".settings.toml.{}", std::process::id()));
        let _ = fs::create_dir_all(folder)
            .and_then(|()| fs::copy(&old, &part))
            .and_then(|_| fs::rename(&part, &path));
        let _ = fs::remove_file(&part);
    }
    Some(path)
}

impl Settings {
    /// Whether the app looks for a new version at startup (yes unless turned off).
    pub fn checks_updates(&self) -> bool {
        self.check_updates.unwrap_or(true)
    }

    /// The saved settings; defaults when there are none or they can't be read.
    pub fn load() -> Settings {
        path().map(|p| Settings::load_from(&p)).unwrap_or_default()
    }

    pub fn save(&self) -> io::Result<()> {
        let path = path().ok_or_else(|| io::Error::other("no config folder"))?;
        self.save_to(&path)
    }

    /// The settings saved at `path`; defaults when there are none or they can't be read.
    pub fn load_from(path: &Path) -> Settings {
        fs::read_to_string(path)
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save_to(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, toml::to_string(self).map_err(io::Error::other)?)
    }
}
