use std::{
    fs::read_to_string,
    path::{Path, PathBuf},
};

use oxc_allocator::Allocator;
use oxc_ast::ast::{Argument, CallExpression, Expression};
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_span::SourceType;
use walkdir::WalkDir;

use crate::core::error::I18nError;

pub enum UsageKind {
    Static(String),
    Prefix(String),
    Dynamic,
}

pub struct Usage {
    pub namespaces: Vec<String>,
    pub kind: UsageKind,
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

pub fn collect_usages(source_dir: &PathBuf) -> Result<Vec<Usage>, I18nError> {
    let mut all_usages: Vec<Usage> = vec![];

    for entry in WalkDir::new(source_dir) {
        let entry = entry?;

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();

        if !is_supported_source_file(path) {
            continue;
        }

        let file_usages = parse_source_file(path)?;
        all_usages.extend(file_usages);
    }

    Ok(all_usages)
}

fn is_supported_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("ts") | Some("tsx") | Some("js") | Some("jsx")
    )
}

fn parse_source_file(path: &Path) -> Result<Vec<Usage>, I18nError> {
    let source_text = read_to_string(path)?;

    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).map_err(|_| I18nError::SourceParse {
        path: path.to_path_buf(),
        message: "failed to infer source type".to_string(),
    })?;
    let parser = Parser::new(&allocator, &source_text, source_type);
    let ret = parser.parse();

    if !ret.errors.is_empty() {
        let message = ret
            .errors
            .into_iter()
            .map(|err| err.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        return Err(I18nError::SourceParse {
            path: path.to_path_buf(),
            message,
        });
    }

    let mut collector = CallCollector {
        namespaces: Vec::new(),
        usages: Vec::new(),
    };

    collector.visit_program(&ret.program);
    Ok(collector.usages)
}
