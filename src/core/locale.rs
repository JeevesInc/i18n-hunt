//! Locale-file loading and key flattening utilities.
//!
//! Locale JSON objects are flattened into dotted keys such as
//! `auth.login.title`.

use std::{
    collections::HashSet,
    fs::read_to_string,
    path::{Path, PathBuf},
};

use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use serde_json::Value;

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
pub fn load_locales(
    dir: &PathBuf,
    exclude_patterns: &[String],
) -> Result<Vec<LocaleFile>, I18nError> {
    let mut locales: Vec<LocaleFile> = vec![];
    let excludes = build_exclude_globset(exclude_patterns)?;
    let (walk_root, namespace_base, only_file) = resolve_locale_roots(dir);

    for entry in WalkBuilder::new(&walk_root).hidden(false).build() {
        let entry = entry?;

        if !entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            continue;
        }

        let path = entry.path();
        if let Some(target_file) = &only_file {
            if path != target_file {
                continue;
            }
        }

        if is_excluded(path, &walk_root, &excludes) {
            continue;
        }

        if is_json_file(path) {
            let locale_file = LocaleFile::from_file(path, &namespace_base)?;

            locales.push(locale_file);
        }
    }

    Ok(locales)
}

fn build_exclude_globset(patterns: &[String]) -> Result<GlobSet, I18nError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern).map_err(|err| {
            I18nError::Config(format!(
                "invalid locales_exclude pattern '{}': {}",
                pattern, err
            ))
        })?;
        builder.add(glob);
    }

    builder.build().map_err(|err| {
        I18nError::Config(format!(
            "failed to compile locales_exclude patterns: {}",
            err
        ))
    })
}

fn is_excluded(path: &Path, root: &Path, excludes: &GlobSet) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };

    matches_relative(relative, excludes)
}

fn matches_relative(relative: &Path, set: &GlobSet) -> bool {
    if set.is_match(relative) {
        return true;
    }

    let normalized = relative.to_string_lossy().replace('\\', "/");
    set.is_match(&normalized)
}

fn resolve_locale_roots(target: &Path) -> (PathBuf, PathBuf, Option<PathBuf>) {
    let walk_root = if target.is_file() {
        target
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        target.to_path_buf()
    };

    let namespace_base = find_locales_base(target).unwrap_or_else(|| walk_root.clone());
    let only_file = target.is_file().then(|| target.to_path_buf());

    (walk_root, namespace_base, only_file)
}

fn find_locales_base(path: &Path) -> Option<PathBuf> {
    for ancestor in path.ancestors() {
        if ancestor.file_name().is_some_and(|name| name == "locales") {
            return Some(ancestor.to_path_buf());
        }
    }
    None
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
