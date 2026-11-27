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
}

/// `<config folder>/settings.toml`, e.g. `~/.config/exportpro/settings.toml` on Linux.
pub fn path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "ExportPro")
        .map(|d| d.config_dir().join("settings.toml"))
}

impl Settings {
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
