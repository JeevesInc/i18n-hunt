use std::{
    fs::read_to_string,
    path::{Path, PathBuf},
};

use oxc_allocator::Allocator;
use oxc_ast::ast::{Argument, CallExpression, Expression, TemplateLiteral};
use oxc_ast_visit::{Visit, walk};
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
    fn push_usage(&mut self, kind: UsageKind) {
        self.usages.push(Usage {
            namespaces: self.namespaces.clone(),
            kind,
        });
    }

    fn handle_use_translation<'a>(&mut self, expr: &CallExpression<'a>) {
        self.namespaces = extract_namespaces(expr);
    }

    fn handle_t_call<'a>(&mut self, expr: &CallExpression<'a>) {
        let Some(first_arg) = expr.arguments.first() else {
            return;
        };

        let kind = match first_arg {
            // t("welcome")
            Argument::StringLiteral(s) => UsageKind::Static(s.value.to_string()),

            // t("auth.${action}")
            Argument::TemplateLiteral(tpl) => classify_template_literal(tpl),

            // t(key), t(buildKey()), etc.
            _ => UsageKind::Dynamic,
        };

        self.push_usage(kind);
    }
}

fn extract_namespaces(expr: &CallExpression<'_>) -> Vec<String> {
    let Some(first_arg) = expr.arguments.first() else {
        return Vec::new();
    };

    match first_arg {
        Argument::StringLiteral(s) => vec![s.value.to_string()],
        Argument::ArrayExpression(arr) => arr
            .elements
            .iter()
            .filter_map(|element| {
                if let oxc_ast::ast::ArrayExpressionElement::StringLiteral(s) = element {
                    Some(s.value.to_string())
                } else {
                    None
                }
            })
            .collect(),
        _ => {
            // dynamic namespace: ignore for now
            Vec::new()
        }
    }
}

impl Default for CallCollector {
    fn default() -> Self {
        Self {
            namespaces: Vec::new(),
            usages: Vec::new(),
        }
    }
}

impl<'a> Visit<'a> for CallCollector {
    fn visit_call_expression(&mut self, expr: &CallExpression<'a>) {
        if let Expression::Identifier(ident) = &expr.callee {
            match ident.name.as_str() {
                "useTranslation" => self.handle_use_translation(expr),
                "t" => self.handle_t_call(expr),
                _ => {}
            }
        }

        walk::walk_call_expression(self, expr);
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

    let mut collector = CallCollector::default();

    collector.visit_program(&ret.program);
    Ok(collector.usages)
}

fn classify_template_literal(tpl: &TemplateLiteral<'_>) -> UsageKind {
    let prefix = tpl
        .quasis
        .first()
        .map(|q| q.value.raw.as_str())
        .unwrap_or("");

    if tpl.expressions.is_empty() {
        UsageKind::Static(prefix.to_string())
    } else if prefix.is_empty() {
        UsageKind::Dynamic
    } else {
        UsageKind::Prefix(prefix.to_string())
    }
}
