use oxc_allocator::Allocator;
use oxc_ast::ast::{Argument, CallExpression, Expression, SourceType};
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use std::{collections::HashSet, fs::read_to_string, path::PathBuf};
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

// TODO: remove these Debugs
#[derive(Debug)]
enum UsageKind {
    Static(String),
    Prefix(String),
    Dynamic,
}

#[derive(Debug)]
struct Usage {
    // namespace: String,
    kind: UsageKind,
}

#[derive(Debug)]
struct CallCollector {
    usages: Vec<Usage>,
}

impl CallCollector {
    fn push(&mut self, kind: UsageKind) {
        self.usages.push(Usage { kind });
    }
}

impl<'a> Visit<'a> for CallCollector {
    fn visit_call_expression(&mut self, expr: &CallExpression<'a>) {
        if let Expression::Identifier(ident) = &expr.callee {
            if ident.name.as_str() == "t" {
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
        }

        oxc_ast_visit::walk::walk_call_expression(self, expr);
    }
}

fn main() {
    // TODO: based on user input or config file
    let locales_dir = "./fixtures/locales";
    let mut locales: Vec<LocaleFile> = vec![];

    // TODO: evaluate and handle unwraps
    for entry in WalkDir::new(locales_dir) {
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
            let relative = entry.path().strip_prefix(locales_dir).unwrap();
            let without_ext = relative.with_extension("");
            let normalized = without_ext.to_string_lossy().replace('\\', "/");

            // TODO: should we do impl for new
            let locale_file = LocaleFile {
                namespace: normalized,
                path: entry.path().to_path_buf(),
                keys: out,
            };

            println!("File: {}", locale_file.path.display());
            println!("Namespace: {}", locale_file.namespace);
            println!("Keys: {:?}", locale_file.keys);
            println!();

            locales.push(locale_file);
        }
    }

    // Extract usage from source
    let file_path = "./fixtures/src/login.ts";
    // TODO: handle unwraps
    let source_text = read_to_string(file_path).unwrap();
    println!("source text: {}", source_text);
    // TODO: OPTIMIZATION - do we need to install all the oxc library?
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(file_path).unwrap();
    let parser = Parser::new(&allocator, &source_text, source_type);
    let ret = parser.parse();

    if !ret.errors.is_empty() {
        println!("Parse errors:");
        for err in ret.errors {
            println!("{:?}", err);
        }
    }

    let mut collector = CallCollector { usages: Vec::new() };
    collector.visit_program(&ret.program);

    println!("{:#?}", collector.usages);
}
