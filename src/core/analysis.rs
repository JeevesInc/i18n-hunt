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
            usage_index
                .entry(namespace.clone())
                .or_default()
                .record_usage(&usage.kind);
        }
    }

    let mut unused = Vec::new();

    for locale in locales {
        let analysis = usage_index.get(&locale.namespace);

        for key in &locale.keys {
            let is_used = analysis.map(|a| a.protects_key(key)).unwrap_or(false);

            if !is_used {
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
