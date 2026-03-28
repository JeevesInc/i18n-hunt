use oxc_ast::ast::{Argument, CallExpression, Expression};
use oxc_ast_visit::Visit;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

mod cli;
mod core;

struct LocaleFile {
    namespace: String,
    path: PathBuf,
    keys: HashSet<String>,
}

enum UsageKind {
    Static(String),
    Prefix(String),
    Dynamic,
}

struct Usage {
    namespaces: Vec<String>,
    kind: UsageKind,
}

struct CallCollector {
    namespaces: Vec<String>,
    usages: Vec<Usage>,
}

impl CallCollector {
    fn push(&mut self, kind: UsageKind) {
        self.usages.push(Usage {
            namespaces: self.namespaces.clone(),
            kind,
        });
    }
}

impl<'a> Visit<'a> for CallCollector {
    fn visit_call_expression(&mut self, expr: &CallExpression<'a>) {
        if let Expression::Identifier(ident) = &expr.callee {
            match ident.name.as_str() {
                "useTranslation" => {
                    if let Some(first_arg) = expr.arguments.first() {
                        match first_arg {
                            Argument::StringLiteral(s) => {
                                self.namespaces.push(s.value.to_string());
                            }
                            Argument::ArrayExpression(arr) => {
                                for element in &arr.elements {
                                    if let oxc_ast::ast::ArrayExpressionElement::StringLiteral(s) =
                                        element
                                    {
                                        self.namespaces.push(s.value.to_string());
                                    }
                                }
                            }
                            _ => {
                                // dynamic namespace;
                                // For initial version we can just ignore.
                            }
                        }
                    }
                }
                "t" => {
                    if let Some(first_arg) = expr.arguments.first() {
                        match first_arg {
                            // t("welcome")
                            Argument::StringLiteral(s) => {
                                self.push(UsageKind::Static(s.value.to_string()))
                            }

                            // t("auth.${action}")
                            Argument::TemplateLiteral(tpl) => {
                                let prefix = tpl
                                    .quasis
                                    .first()
                                    .map(|q| q.value.raw.as_str())
                                    .unwrap_or("");

                                if tpl.expressions.is_empty() {
                                    self.push(UsageKind::Static(prefix.to_string()))
                                } else if prefix.is_empty() {
                                    self.push(UsageKind::Dynamic);
                                } else {
                                    self.push(UsageKind::Prefix(prefix.to_string()));
                                }
                            }

                            // t(key), t(buildKey()), etc.
                            _ => {
                                self.push(UsageKind::Dynamic);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        oxc_ast_visit::walk::walk_call_expression(self, expr);
    }
}

#[derive(Default)]
struct NamespaceAnalysis {
    used_static: HashSet<String>,
    prefixes: HashSet<String>,
    dynamic_count: usize,
}

struct UnusedKey {
    namespace: String,
    key: String,
    path: PathBuf,
}

struct AnalysisResult {
    unused: Vec<UnusedKey>,
}

fn analyze(locales: &[LocaleFile], usages: &[Usage]) -> AnalysisResult {
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

fn main() {
    // TODO: handle file case
    // TODO: refactor main
    // TODO: better report
    // TODO: fail on unused
    // TODO: verbose mode
    // TODO: improve dynamic
    // TODO: auto remove unused keys
    // TODO: run for staged files
    // TODO: consider root (by file - default or paramenter) to get the namespace
    // TODO: improve code docs
    // TODO: improve CLI docs

    let args = cli::parse();
    let config = args.into_config();

    let result = core::run(&config).unwrap();

    let idk = analyze(&result.locales, &result.usages);

    println!("Unused keys:");
    for item in idk.unused {
        println!(
            "[{}] {} -> {}",
            item.namespace,
            item.path.display(),
            item.key
        );
    }
}
