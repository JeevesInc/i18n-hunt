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

pub struct UnusedKey {
    pub namespace: String,
    pub key: String,
    pub path: PathBuf,
}

pub struct AnalysisResult {
    pub unused: Vec<UnusedKey>,
}

pub fn analyze(locales: &[LocaleFile], usages: &[Usage]) -> AnalysisResult {
    // TODO: maybe we should check these clones?

    let mut usage_index: HashMap<String, NamespaceAnalysis> = HashMap::new();

    for usage in usages {
        for namespace in &usage.namespaces {
            let entry = usage_index.entry(namespace.clone()).or_default();

            match &usage.kind {
                UsageKind::Static(key) => {
                    entry.used_static.insert(key.clone());
                }
                UsageKind::Prefix(prefix) => {
                    entry.prefixes.insert(prefix.clone());
                }
                UsageKind::Dynamic => {
                    entry.dynamic_count += 1;
                }
            }
        }
    }

    let mut unused = Vec::new();

    for locale in locales {
        let analysis = usage_index.get(&locale.namespace);

        for key in &locale.keys {
            let is_used_static = analysis
                .map(|a| a.used_static.contains(key))
                .unwrap_or(false);

            let is_protected_by_prefix = analysis
                .map(|a| a.prefixes.iter().any(|prefix| key.starts_with(prefix)))
                .unwrap_or(false);

            if !is_used_static && !is_protected_by_prefix {
                unused.push(UnusedKey {
                    namespace: locale.namespace.clone(),
                    key: key.clone(),
                    path: locale.path.clone(),
                });
            }
        }
    }

    AnalysisResult { unused }
}
