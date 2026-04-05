//! Error types used by the i18n analysis pipeline.

use core::fmt;
use std::{io, path::PathBuf};

/// Unified error enum for filesystem, parsing, and path-derivation failures.
#[derive(Debug)]
pub enum I18nError {
    /// A low-level I/O error.
    Io(io::Error),
    /// Invalid JSON while reading a locale file.
    Json(serde_json::Error),
    /// Invalid path relationship or normalization issue.
    InvalidPath { path: PathBuf, message: String },
    /// Source file parsing failure.
    SourceParse { path: PathBuf, message: String },
    /// Recursive directory traversal failure.
    WalkDir(String),
}

impl fmt::Display for I18nError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            I18nError::Io(err) => write!(f, "io error: {err}"),
            I18nError::Json(err) => write!(f, "json error: {err}"),
            I18nError::InvalidPath { path, message } => {
                write!(f, "invalid path '{}': {}", path.display(), message)
            }
            I18nError::SourceParse { path, message } => {
                write!(
                    f,
                    "failed to parse source file '{}': {}",
                    path.display(),
                    message
                )
            }
            I18nError::WalkDir(message) => write!(f, "walkdir error: {message}"),
        }
    }
}

impl std::error::Error for I18nError {}

impl From<io::Error> for I18nError {
    fn from(err: io::Error) -> Self {
        I18nError::Io(err)
    }
}

impl From<serde_json::Error> for I18nError {
    fn from(err: serde_json::Error) -> Self {
        I18nError::Json(err)
    }
}

impl From<walkdir::Error> for I18nError {
    fn from(err: walkdir::Error) -> Self {
        I18nError::WalkDir(err.to_string())
    }
}
