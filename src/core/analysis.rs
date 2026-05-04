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
            UsageKind::Dynamic => {}
        }
    }

    fn protects_key(&self, key: &str) -> bool {
        self.used_static.contains(key) || self.prefixes.iter().any(|prefix| key.starts_with(prefix))
    }
}

pub struct UnusedKey {
    /// Locale file path where the key is defined.
    pub path: PathBuf,
    /// Flattened translation key that appears unused.
    pub key: String,
}

pub struct DynamicUsageSite {
    /// Source file where unresolved dynamic usage was found.
    pub path: PathBuf,
    /// 1-based source line for the unresolved usage.
    pub line: usize,
    /// Namespaces in scope (or overridden) for this usage.
    pub namespaces: Vec<String>,
}

/// Result of a full unused-key analysis run.
pub struct AnalysisResult {
    /// Total number of locale keys loaded for this run.
    pub total_keys: usize,
    /// All locale keys not matched by observed usage.
    pub unused_keys: Vec<UnusedKey>,
    /// Usage sites where translation key is dynamic/unresolved.
    pub dynamic_usages: Vec<DynamicUsageSite>,
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
    let mut dynamic_usages = Vec::new();
    let locale_key_index: HashMap<String, &HashSet<String>> = locales
        .iter()
        .map(|locale| (locale.namespace.clone(), &locale.keys))
        .collect();

    for usage in usages {
        if matches!(usage.kind, UsageKind::Dynamic) {
            dynamic_usages.push(DynamicUsageSite {
                path: usage.path.clone(),
                line: usage.line,
                namespaces: usage.namespaces.clone(),
            });
        }

        for namespace in resolve_usage_namespaces(usage, &locale_key_index) {
            usage_index
                .entry(namespace)
                .or_default()
                .record_usage(&usage.kind);
        }
    }

    let mut unused_keys = Vec::new();
    let mut total_keys = 0usize;

    for locale in locales {
        let analysis = usage_index.get(&locale.namespace);
        total_keys += locale.keys.len();

        for key in &locale.keys {
            let is_used = analysis.map(|a| a.protects_key(key)).unwrap_or(false);

            if !is_used {
                unused_keys.push(UnusedKey {
                    path: locale.path.clone(),
                    key: key.clone(),
                });
            }
        }
    }

    AnalysisResult {
        total_keys,
        unused_keys,
        dynamic_usages,
    }
}

fn resolve_usage_namespaces(
    usage: &Usage,
    locale_key_index: &HashMap<String, &HashSet<String>>,
) -> Vec<String> {
    let fallback = usage
        .namespaces
        .first()
        .cloned()
        .into_iter()
        .collect::<Vec<_>>();

    let resolved = match &usage.kind {
        UsageKind::Static(key) => usage
            .namespaces
            .iter()
            .filter(|namespace| {
                locale_key_index
                    .get(*namespace)
                    .is_some_and(|keys| keys.contains(key))
            })
            .cloned()
            .collect::<Vec<_>>(),
        UsageKind::Prefix(prefix) => usage
            .namespaces
            .iter()
            .filter(|namespace| {
                locale_key_index
                    .get(*namespace)
                    .is_some_and(|keys| keys.iter().any(|key| key.starts_with(prefix)))
            })
            .cloned()
            .collect::<Vec<_>>(),
        UsageKind::Dynamic => fallback.clone(),
    };

    if resolved.is_empty() {
        fallback
    } else {
        resolved
    }
}
