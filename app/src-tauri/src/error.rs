use std::path::PathBuf;

use serde::Serializer;

#[derive(Debug, thiserror::Error)]
pub enum LibError {
    #[error("Invalid name: {0}")]
    InvalidName(String),
    #[error("An asset named {0} already exists")]
    NameExists(String),
    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("Failed to parse model: {0}")]
    Parse(String),
    #[error("Missing file: {}", .0.display())]
    MissingFile(PathBuf),
    #[error("Path is outside the library: {}", .0.display())]
    OutsideLibrary(PathBuf),
    #[error("Bridge error: {0}")]
    Bridge(String),
    #[error("Render cancelled")]
    Cancelled,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// Tauri commands return errors as plain message strings.
impl serde::Serialize for LibError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
