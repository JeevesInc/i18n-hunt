//! Analysis logic that maps locale keys to observed translation usages.

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use crate::core::{
    locale::LocaleFile,
    source::{Usage, UsageKind},
};

#[derive(Default)]
struct NamespaceAnalysis {
    used_static: HashSet<String>,
    prefixes: HashSet<String>,
    dynamic_count: usize,
}

impl NamespaceAnalysis {
    fn record_usage(&mut self, kind: &UsageKind) {
        match kind {
            UsageKind::Static(key) => {
                self.used_static.insert(key.clone());
            }
            UsageKind::Prefix(prefix) => {
                self.prefixes.insert(prefix.clone());
            }
            UsageKind::Dynamic => {
                self.dynamic_count += 1;
            }
        }
    }

    fn protects_key(&self, key: &str) -> bool {
        self.used_static.contains(key) || self.prefixes.iter().any(|prefix| key.starts_with(prefix))
    }
}

pub struct UnusedKey {
    /// Namespace in which the unused key is defined.
    pub namespace: String,
    /// Flattened translation key that appears unused.
    pub key: String,
    /// Locale file path where the key is defined.
    pub path: PathBuf,
}

/// Result of a full unused-key analysis run.
pub struct AnalysisResult {
    /// All locale keys not matched by observed usage.
    pub unused_keys: Vec<UnusedKey>,
}

/// Computes unused translation keys from locale definitions and source usages.
///
/// A key is considered protected when a static key usage matches exactly, or
/// when a template-literal usage contributes a prefix that the key starts with.
///
/// # Arguments
///
/// * `locales` - Locale files with namespaces and flattened keys.
/// * `usages` - Collected translation usages from source scanning.
///
/// # Returns
///
/// An [`AnalysisResult`] containing keys that appear to be unused.
pub fn analyze(locales: &[LocaleFile], usages: &[Usage]) -> AnalysisResult {
    // TODO: maybe we should check these clones?

    let mut usage_index: HashMap<String, NamespaceAnalysis> = HashMap::new();

    for usage in usages {
        for namespace in &usage.namespaces {
            usage_index
                .entry(namespace.clone())
                .or_default()
                .record_usage(&usage.kind);
        }
    }

    let mut unused_keys = Vec::new();

    for locale in locales {
        let analysis = usage_index.get(&locale.namespace);

        for key in &locale.keys {
            let is_used = analysis.map(|a| a.protects_key(key)).unwrap_or(false);

            if !is_used {
                unused_keys.push(UnusedKey {
                    namespace: locale.namespace.clone(),
                    key: key.clone(),
                    path: locale.path.clone(),
                });
            }
        }
    }

    AnalysisResult { unused_keys }
}
