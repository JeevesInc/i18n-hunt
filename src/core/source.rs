//! Source scanning for translation namespaces and key usages.
//!
//! JavaScript and TypeScript files are parsed into an AST and inspected for
//! `useTranslation(...)` and `t(...)` calls.

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs::read_to_string,
    path::{Path, PathBuf},
};

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, BindingPattern, CallExpression, ConditionalExpression, Expression, Function,
    FunctionBody, ObjectPropertyKind, Program, PropertyKey, Statement, TemplateLiteral,
    VariableDeclaration, VariableDeclarationKind,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::SourceType;
use walkdir::WalkDir;

use crate::core::error::I18nError;

/// Classification of how precisely a translation key usage is known.
pub enum UsageKind {
    /// Exact key from a string literal.
    Static(String),
    /// Prefix extracted from a template literal with expressions.
    Prefix(String),
    /// Key is dynamic and cannot be resolved statically.
    Dynamic,
}

/// A discovered translation key usage scoped to one or more namespaces.
pub struct Usage {
    /// Namespaces in scope when the usage was recorded.
    pub namespaces: Vec<String>,
    /// The usage classification.
    pub kind: UsageKind,
}

#[derive(Clone, Default)]
struct InferredValue {
    statics: BTreeSet<String>,
    prefixes: BTreeSet<String>,
    dynamic: bool,
}

impl InferredValue {
    fn dynamic() -> Self {
        Self {
            statics: BTreeSet::new(),
            prefixes: BTreeSet::new(),
            dynamic: true,
        }
    }

    fn from_usage_kind(kind: UsageKind) -> Self {
        let mut value = Self::default();

        match kind {
            UsageKind::Static(key) => {
                value.statics.insert(key);
            }
            UsageKind::Prefix(prefix) => {
                value.prefixes.insert(prefix);
            }
            UsageKind::Dynamic => {
                value.dynamic = true;
            }
        }

        value
    }

    fn merge(&mut self, other: Self) {
        self.statics.extend(other.statics);
        self.prefixes.extend(other.prefixes);
        self.dynamic |= other.dynamic;
    }

    fn into_usage_kinds(self) -> Vec<UsageKind> {
        let mut kinds = Vec::new();

        for key in self.statics {
            kinds.push(UsageKind::Static(key));
        }

        for prefix in self.prefixes {
            kinds.push(UsageKind::Prefix(prefix));
        }

        if self.dynamic || kinds.is_empty() {
            kinds.push(UsageKind::Dynamic);
        }

        kinds
    }
}

struct CallCollector {
    namespaces: Vec<String>,
    usages: Vec<Usage>,
    function_values: HashMap<String, InferredValue>,
    scopes: Vec<HashMap<String, InferredValue>>,
}

impl CallCollector {
    fn new(function_values: HashMap<String, InferredValue>) -> Self {
        Self {
            namespaces: Vec::new(),
            usages: Vec::new(),
            function_values,
            scopes: vec![HashMap::new()],
        }
    }

    fn push_usage(&mut self, kind: UsageKind) {
        self.usages.push(Usage {
            namespaces: self.namespaces.clone(),
            kind,
        });
    }

    fn push_usage_with_namespaces(&mut self, namespaces: Vec<String>, kind: UsageKind) {
        self.usages.push(Usage { namespaces, kind });
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn bind_local(&mut self, name: String, value: InferredValue) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, value);
        }
    }

    fn resolve_local(&self, name: &str) -> Option<InferredValue> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    fn handle_use_translation<'a>(&mut self, expr: &CallExpression<'a>) {
        self.namespaces = extract_namespaces(expr);
    }

    fn handle_t_call<'a>(&mut self, expr: &CallExpression<'a>) {
        let Some(first_arg) = expr.arguments.first() else {
            return;
        };
        let ns_override = expr.arguments.get(1).and_then(extract_ns_override);

        let inferred = self.infer_argument(first_arg);

        for kind in inferred.into_usage_kinds() {
            self.push_resolved_usage(kind, ns_override.as_deref());
        }
    }

    fn push_resolved_usage(&mut self, kind: UsageKind, ns_override: Option<&str>) {
        match kind {
            UsageKind::Static(key) => {
                if let Some((ns, raw_key)) = split_colon_namespace(&key) {
                    self.push_usage_with_namespaces(vec![ns], UsageKind::Static(raw_key));
                } else if let Some(override_ns) = ns_override {
                    self.push_usage_with_namespaces(
                        vec![override_ns.to_string()],
                        UsageKind::Static(key),
                    );
                } else {
                    self.push_usage(UsageKind::Static(key));
                }
            }
            UsageKind::Prefix(prefix) => {
                if let Some((ns, raw_prefix)) = split_colon_namespace(&prefix) {
                    self.push_usage_with_namespaces(vec![ns], UsageKind::Prefix(raw_prefix));
                } else if let Some(override_ns) = ns_override {
                    self.push_usage_with_namespaces(
                        vec![override_ns.to_string()],
                        UsageKind::Prefix(prefix),
                    );
                } else {
                    self.push_usage(UsageKind::Prefix(prefix));
                }
            }
            UsageKind::Dynamic => {
                if let Some(override_ns) = ns_override {
                    self.push_usage_with_namespaces(
                        vec![override_ns.to_string()],
                        UsageKind::Dynamic,
                    );
                } else {
                    self.push_usage(UsageKind::Dynamic);
                }
            }
        }
    }

    fn track_const_bindings<'a>(&mut self, decl: &VariableDeclaration<'a>) {
        if decl.kind != VariableDeclarationKind::Const {
            return;
        }

        for declarator in &decl.declarations {
            let Some(init) = &declarator.init else {
                continue;
            };

            let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
                continue;
            };

            let inferred = self.infer_expression(init);
            self.bind_local(binding.name.to_string(), inferred);
        }
    }

    fn infer_argument<'a>(&self, arg: &Argument<'a>) -> InferredValue {
        match arg {
            Argument::StringLiteral(s) => {
                InferredValue::from_usage_kind(UsageKind::Static(s.value.to_string()))
            }
            Argument::TemplateLiteral(tpl) => {
                InferredValue::from_usage_kind(classify_template_literal(tpl))
            }
            Argument::Identifier(ident) => self
                .resolve_local(ident.name.as_str())
                .unwrap_or_else(InferredValue::dynamic),
            Argument::CallExpression(call) => self.infer_call(call),
            Argument::ConditionalExpression(cond) => self.infer_conditional(cond),
            _ => InferredValue::dynamic(),
        }
    }

    fn infer_expression<'a>(&self, expr: &Expression<'a>) -> InferredValue {
        match expr {
            Expression::StringLiteral(s) => {
                InferredValue::from_usage_kind(UsageKind::Static(s.value.to_string()))
            }
            Expression::TemplateLiteral(tpl) => {
                InferredValue::from_usage_kind(classify_template_literal(tpl))
            }
            Expression::Identifier(ident) => self
                .resolve_local(ident.name.as_str())
                .unwrap_or_else(InferredValue::dynamic),
            Expression::CallExpression(call) => self.infer_call(call),
            Expression::ConditionalExpression(cond) => self.infer_conditional(cond),
            _ => InferredValue::dynamic(),
        }
    }

    fn infer_conditional<'a>(&self, cond: &ConditionalExpression<'a>) -> InferredValue {
        let mut inferred = self.infer_expression(&cond.consequent);
        inferred.merge(self.infer_expression(&cond.alternate));
        inferred
    }

    fn infer_call<'a>(&self, call: &CallExpression<'a>) -> InferredValue {
        let Expression::Identifier(ident) = &call.callee else {
            return InferredValue::dynamic();
        };

        self.function_values
            .get(ident.name.as_str())
            .cloned()
            .unwrap_or_else(InferredValue::dynamic)
    }
}

/// Extracts namespaces from a `useTranslation(...)` call expression.
///
/// Dynamic namespace expressions are ignored and return an empty namespace list.
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

impl<'a> Visit<'a> for CallCollector {
    fn visit_block_statement(&mut self, block: &oxc_ast::ast::BlockStatement<'a>) {
        self.push_scope();
        walk::walk_block_statement(self, block);
        self.pop_scope();
    }

    fn visit_function_body(&mut self, function_body: &FunctionBody<'a>) {
        self.push_scope();
        walk::walk_function_body(self, function_body);
        self.pop_scope();
    }

    fn visit_variable_declaration(&mut self, decl: &VariableDeclaration<'a>) {
        self.track_const_bindings(decl);
        walk::walk_variable_declaration(self, decl);
    }

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

/// Recursively collects translation key usages from supported source files.
///
/// Supported extensions are `.ts`, `.tsx`, `.js`, and `.jsx`.
///
/// # Arguments
///
/// * `source_dir` - Root directory to scan.
///
/// # Returns
///
/// A flat vector of discovered [`Usage`] entries.
///
/// # Errors
///
/// Returns [`I18nError`] when traversal, file reading, or source parsing fails.
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

/// Returns whether `path` is a supported source file extension.
fn is_supported_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("ts") | Some("tsx") | Some("js") | Some("jsx")
    )
}

/// Parses one source file and extracts translation usages from its AST.
///
/// # Errors
///
/// Returns [`I18nError`] if source type detection or parsing fails.
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

    let function_values = infer_function_values(&ret.program);
    let mut collector = CallCollector::new(function_values);

    collector.visit_program(&ret.program);
    Ok(collector.usages)
}

fn infer_function_values<'a>(program: &'a Program<'a>) -> HashMap<String, InferredValue> {
    let functions = collect_function_declarations(program);
    let mut cache = HashMap::new();
    let mut visiting = HashSet::new();

    for name in functions.keys() {
        let inferred = infer_function_by_name(name, &functions, &mut cache, &mut visiting);
        cache.insert(name.clone(), inferred);
    }

    cache
}

fn collect_function_declarations<'a>(
    program: &'a Program<'a>,
) -> HashMap<String, &'a Function<'a>> {
    let mut functions = HashMap::new();

    for stmt in &program.body {
        if let Statement::FunctionDeclaration(function) = stmt {
            if let Some(id) = &function.id {
                functions.insert(id.name.to_string(), function.as_ref());
            }
        }
    }

    functions
}

fn infer_function_by_name<'a>(
    name: &str,
    functions: &HashMap<String, &'a Function<'a>>,
    cache: &mut HashMap<String, InferredValue>,
    visiting: &mut HashSet<String>,
) -> InferredValue {
    if let Some(existing) = cache.get(name) {
        return existing.clone();
    }

    if !visiting.insert(name.to_string()) {
        return InferredValue::dynamic();
    }

    let inferred = functions
        .get(name)
        .map(|function| infer_function_returns(function, functions, cache, visiting))
        .unwrap_or_else(InferredValue::dynamic);

    visiting.remove(name);

    inferred
}

fn infer_function_returns<'a>(
    function: &'a Function<'a>,
    functions: &HashMap<String, &'a Function<'a>>,
    cache: &mut HashMap<String, InferredValue>,
    visiting: &mut HashSet<String>,
) -> InferredValue {
    let mut out = InferredValue::default();

    let Some(body) = &function.body else {
        return InferredValue::dynamic();
    };

    for statement in &body.statements {
        infer_returns_from_statement(statement, functions, cache, visiting, &mut out);
    }

    if out.statics.is_empty() && out.prefixes.is_empty() && !out.dynamic {
        out.dynamic = true;
    }

    out
}

fn infer_returns_from_statement<'a>(
    statement: &'a Statement<'a>,
    functions: &HashMap<String, &'a Function<'a>>,
    cache: &mut HashMap<String, InferredValue>,
    visiting: &mut HashSet<String>,
    out: &mut InferredValue,
) {
    match statement {
        Statement::ReturnStatement(ret) => {
            if let Some(argument) = &ret.argument {
                out.merge(infer_expression_with_functions(
                    argument, functions, cache, visiting,
                ));
            }
        }
        Statement::BlockStatement(block) => {
            for inner in &block.body {
                infer_returns_from_statement(inner, functions, cache, visiting, out);
            }
        }
        Statement::IfStatement(if_statement) => {
            infer_returns_from_statement(&if_statement.consequent, functions, cache, visiting, out);

            if let Some(alternate) = &if_statement.alternate {
                infer_returns_from_statement(alternate, functions, cache, visiting, out);
            }
        }
        _ => {}
    }
}

fn infer_expression_with_functions<'a>(
    expression: &'a Expression<'a>,
    functions: &HashMap<String, &'a Function<'a>>,
    cache: &mut HashMap<String, InferredValue>,
    visiting: &mut HashSet<String>,
) -> InferredValue {
    match expression {
        Expression::StringLiteral(s) => {
            InferredValue::from_usage_kind(UsageKind::Static(s.value.to_string()))
        }
        Expression::TemplateLiteral(tpl) => {
            InferredValue::from_usage_kind(classify_template_literal(tpl))
        }
        Expression::ConditionalExpression(cond) => {
            let mut out =
                infer_expression_with_functions(&cond.consequent, functions, cache, visiting);
            out.merge(infer_expression_with_functions(
                &cond.alternate,
                functions,
                cache,
                visiting,
            ));
            out
        }
        Expression::CallExpression(call) => {
            let Expression::Identifier(ident) = &call.callee else {
                return InferredValue::dynamic();
            };

            let name = ident.name.as_str();
            let inferred = infer_function_by_name(name, functions, cache, visiting);
            cache.insert(name.to_string(), inferred.clone());
            inferred
        }
        _ => InferredValue::dynamic(),
    }
}

/// Classifies template literal usage into static key, prefix, or dynamic usage.
///
/// A non-empty leading quasi with expressions is treated as a stable prefix.
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

fn split_colon_namespace(value: &str) -> Option<(String, String)> {
    let (namespace, key) = value.split_once(':')?;

    if namespace.is_empty() || key.is_empty() {
        return None;
    }

    Some((namespace.to_string(), key.to_string()))
}

fn extract_ns_override(arg: &Argument<'_>) -> Option<String> {
    let Argument::ObjectExpression(obj) = arg else {
        return None;
    };

    for property in &obj.properties {
        let ObjectPropertyKind::ObjectProperty(prop) = property else {
            continue;
        };

        if prop.computed || prop.method {
            continue;
        }

        if !property_key_is_ns(&prop.key) {
            continue;
        }

        match &prop.value {
            Expression::StringLiteral(s) => return Some(s.value.to_string()),
            Expression::TemplateLiteral(tpl) if tpl.expressions.is_empty() => {
                let value = tpl
                    .quasis
                    .first()
                    .map(|q| q.value.raw.as_str())
                    .unwrap_or("");
                return Some(value.to_string());
            }
            _ => return None,
        }
    }

    None
}

fn property_key_is_ns(key: &PropertyKey<'_>) -> bool {
    match key {
        PropertyKey::StaticIdentifier(ident) => ident.name == "ns",
        PropertyKey::StringLiteral(string) => string.value == "ns",
        _ => false,
    }
}
