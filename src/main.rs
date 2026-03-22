use clap::Parser;
use oxc_allocator::Allocator;
use oxc_ast::ast::{Argument, CallExpression, Expression, SourceType};
use oxc_ast_visit::Visit;
use oxc_parser::Parser as OxcParser;
use std::{
    collections::{HashMap, HashSet},
    fs::read_to_string,
    path::PathBuf,
};
use walkdir::WalkDir;

use serde_json::Value;

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

#[derive(Parser)]
#[command(name = "i18n-hunt")]
#[command(about = "Detect unused i18n keys using AST analysis")]
struct Args {
    #[arg(long)]
    locales: PathBuf,

    #[arg(long)]
    src: PathBuf,
}

fn main() {
    let args = Args::parse();

    // TODO: based on user input or config file
    let locales_dir = args.locales;
    let source_dir = args.src;

    let mut locales: Vec<LocaleFile> = vec![];

    // TODO: evaluate and handle unwraps
    for entry in WalkDir::new(&locales_dir) {
        let entry = entry.unwrap();
        // TODO: add better check for json
        if entry.file_type().is_file() {
            let mut buf = String::new();
            let mut out = HashSet::new();

            let content = read_to_string(entry.path()).unwrap();
            let deserialized: Value = serde_json::from_str(&content).unwrap();
            flatten_into(&deserialized, &mut buf, &mut out);

            // TODO: refactor to  derive_namespace function?
            // TODO: hadle unwraps and possible errors
            let relative = entry.path().strip_prefix(&locales_dir).unwrap();
            let without_ext = relative.with_extension("");
            let normalized = without_ext.to_string_lossy().replace('\\', "/");

            // TODO: should we do impl for new
            let locale_file = LocaleFile {
                namespace: normalized,
                path: entry.path().to_path_buf(),
                keys: out,
            };

            locales.push(locale_file);
        }
    }

    let mut all_usages: Vec<Usage> = vec![];

    // Extract usage from source
    // TODO: handle unwraps
    for entry in WalkDir::new(source_dir) {
        let entry = entry.unwrap();

        if entry.file_type().is_file() {
            let path = entry.path();

            let is_supported = matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("ts") | Some("tsx") | Some("js") | Some("jsx")
            );

            if !is_supported {
                continue;
            }

            let source_text = read_to_string(path).unwrap();

            let allocator = Allocator::default();
            let source_type = SourceType::from_path(path).unwrap();
            let parser = OxcParser::new(&allocator, &source_text, source_type);
            let ret = parser.parse();

            if !ret.errors.is_empty() {
                println!("Parse errors:");
                for err in ret.errors {
                    println!("{:?}", err);
                }
            }

            let mut collector = CallCollector {
                namespaces: Vec::new(),
                usages: Vec::new(),
            };

            collector.visit_program(&ret.program);

            all_usages.extend(collector.usages);
        }
    }

    let result = analyze(&locales, &all_usages);

    println!("Unused keys:");
    for item in result.unused {
        println!(
            "[{}] {} -> {}",
            item.namespace,
            item.path.display(),
            item.key
        );
    }
}
