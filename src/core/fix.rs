//! Auto-fix support for removing unused translation keys from locale JSON files.

use std::{
    collections::BTreeMap,
    fs::{read_to_string, write},
    path::PathBuf,
};

use serde_json::Value;

use crate::core::{analysis::UnusedKey, error::I18nError};

/// Summary of auto-fix mutations.
pub struct FixResult {
    /// Number of keys removed from locale files.
    pub removed_keys: usize,
    /// Number of locale files modified.
    pub touched_files: usize,
}

/// Applies auto-fixes by deleting unused keys from locale JSON files.
pub fn apply(unused_keys: &[UnusedKey]) -> Result<FixResult, I18nError> {
    let mut by_file: BTreeMap<PathBuf, Vec<String>> = BTreeMap::new();

    for unused in unused_keys {
        by_file
            .entry(unused.path.clone())
            .or_default()
            .push(unused.key.clone());
    }

    let mut removed_keys = 0usize;
    let mut touched_files = 0usize;

    for (path, keys) in by_file {
        let content = read_to_string(&path)?;
        let mut json: Value = serde_json::from_str(&content)?;

        let mut file_removed = 0usize;
        for key in keys {
            if remove_key(&mut json, &key) {
                file_removed += 1;
            }
        }

        if file_removed > 0 {
            let updated = serde_json::to_string_pretty(&json)?;
            write(&path, format!("{updated}\n"))?;
            removed_keys += file_removed;
            touched_files += 1;
        }
    }

    Ok(FixResult {
        removed_keys,
        touched_files,
    })
}

fn remove_key(root: &mut Value, dotted_key: &str) -> bool {
    let segments = dotted_key.split('.').collect::<Vec<_>>();
    remove_path(root, &segments)
}

fn remove_path(value: &mut Value, segments: &[&str]) -> bool {
    if segments.is_empty() {
        return false;
    }

    let Value::Object(obj) = value else {
        return false;
    };

    if segments.len() == 1 {
        return obj.remove(segments[0]).is_some();
    }

    let Some(next) = obj.get_mut(segments[0]) else {
        return false;
    };

    let removed = remove_path(next, &segments[1..]);

    if removed && is_empty_object(next) {
        obj.remove(segments[0]);
    }

    removed
}

fn is_empty_object(value: &Value) -> bool {
    value.as_object().is_some_and(|map| map.is_empty())
}
