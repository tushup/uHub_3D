use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::LibError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub library_root: PathBuf,
    pub port: u16,
}

impl Default for Settings {
    fn default() -> Self {
        let root = dirs::document_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("uHub3D")
            .join("Database");
        Self {
            library_root: root,
            port: 47913,
        }
    }
}

/// Location of the settings file: `<config>/uHub3D/settings.json`.
pub fn settings_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("uHub3D")
        .join("settings.json")
}

impl Settings {
    /// Missing or invalid files fall back to the defaults.
    pub fn load(path: &Path) -> Settings {
        let Ok(text) = fs::read_to_string(path) else {
            return Settings::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), LibError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| LibError::Parse(e.to_string()))?;
        fs::write(path, json)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_root_in_documents() {
        assert!(Settings::default().library_root.ends_with("uHub3D/Database"));
        assert_eq!(Settings::default().port, 47913);
    }

    #[test]
    fn load_missing_is_default() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(Settings::load(&d.path().join("nope.json")), Settings::default());
    }

    #[test]
    fn roundtrip() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("settings.json");
        let settings = Settings {
            library_root: d.path().join("library"),
            port: 12345,
        };
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path), settings);
    }

    #[test]
    fn load_corrupt_is_default() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("settings.json");
        std::fs::write(&path, "{").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
    }
}
