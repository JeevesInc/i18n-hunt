//! Locale-file loading and key flattening utilities.
//!
//! Locale JSON objects are flattened into dotted keys such as
//! `auth.login.title`.

use std::{
    collections::HashSet,
    fs::read_to_string,
    path::{Path, PathBuf},
};

use serde_json::Value;
use walkdir::WalkDir;

use crate::core::error::I18nError;

/// Parsed locale file metadata and extracted key set.
pub struct LocaleFile {
    /// Namespace derived from the file path relative to the locale root.
    pub namespace: String,
    /// Path to the locale file.
    pub path: PathBuf,
    /// Flattened key paths whose terminal values are strings.
    pub keys: HashSet<String>,
}

impl LocaleFile {
    fn from_file(path: &Path, base_dir: &Path) -> Result<Self, I18nError> {
        let content = read_to_string(path)?;

        let json: Value = serde_json::from_str(&content)?;

        let mut keys = HashSet::new();
        let mut buffer = String::new();

        flatten_into(&json, &mut buffer, &mut keys);

        let namespace = derive_namespace(base_dir, path)?;

        Ok(Self {
            namespace,
            path: path.to_path_buf(),
            keys,
        })
    }
}

/// Recursively loads locale JSON files from `dir`.
///
/// # Arguments
///
/// * `dir` - Root locale directory.
///
/// # Returns
///
/// Parsed locale files with namespaces and flattened keys.
///
/// # Errors
///
/// Returns [`I18nError`] if traversal, file reading, parsing, or namespace
/// derivation fails.
pub fn load_locales(dir: &PathBuf) -> Result<Vec<LocaleFile>, I18nError> {
    let mut locales: Vec<LocaleFile> = vec![];

    for entry in WalkDir::new(&dir) {
        let entry = entry?;
        let path = entry.path();

        if is_json_file(path) {
            let locale_file = LocaleFile::from_file(path, dir)?;

            locales.push(locale_file);
        }
    }

    Ok(locales)
}

/// Returns whether a path points to a `.json` file.
fn is_json_file(path: &Path) -> bool {
    matches!(path.extension().and_then(|ext| ext.to_str()), Some("json"))
}

/// Flattens nested JSON objects into dotted keys.
///
/// Only terminal string values are emitted as valid translation keys.
fn flatten_into(value: &Value, buf: &mut String, out: &mut HashSet<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let previus_state = buf.len();

                if !buf.is_empty() {
                    buf.push('.');
                }

                buf.push_str(&k);

                flatten_into(v, buf, out);

                buf.truncate(previus_state);
            }
        }
        Value::String(_) => {
            if !buf.is_empty() {
                out.insert(buf.clone());
            }
        }
        _ => {}
    }
}

/// Derives a locale namespace from `file` relative to `base`.
///
/// The `.json` extension is removed and separators are normalized to `/`.
///
/// # Errors
///
/// Returns [`I18nError::InvalidPath`] when `file` is not under `base`.
fn derive_namespace(base: &Path, file: &Path) -> Result<String, I18nError> {
    let relative = file
        .strip_prefix(base)
        .map_err(|_| I18nError::InvalidPath {
            path: file.to_path_buf(),
            message: format!("could not strip base prefix '{}'", base.display()),
        })?;

    let mut namespace = relative.to_string_lossy().to_string();

    if let Some(stripped) = namespace.strip_suffix(".json") {
        namespace = stripped.to_string();
    }

    namespace = namespace.replace('\\', "/");

    Ok(namespace)
}
