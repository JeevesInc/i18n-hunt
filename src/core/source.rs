//! Source scanning for translation namespaces and key usages.
//!
//! JavaScript and TypeScript files are parsed into an AST and inspected for
//! `useTranslation(...)` and `t(...)` calls.

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs::read_to_string,
    path::{Path, PathBuf},
};

use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, BindingPattern, CallExpression, ConditionalExpression,
    Expression, Function, FunctionBody, JSXAttributeItem, JSXAttributeName, JSXAttributeValue,
    JSXElementName, JSXExpression, JSXOpeningElement, LogicalExpression, ObjectPropertyKind,
    Program, PropertyKey,
    Statement, TemplateLiteral, VariableDeclaration, VariableDeclarationKind,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::SourceType;

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
    /// Source file where the usage was found.
    pub path: PathBuf,
    /// 1-based source line where the usage was found.
    pub line: usize,
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
    translator_scopes: Vec<HashMap<String, String>>,
    map_scopes: Vec<HashMap<String, HashMap<String, InferredValue>>>,
    file_path: PathBuf,
    line_starts: Vec<usize>,
}

impl CallCollector {
    fn new(
        function_values: HashMap<String, InferredValue>,
        file_path: PathBuf,
        source_text: &str,
    ) -> Self {
        Self {
            namespaces: Vec::new(),
            usages: Vec::new(),
            function_values,
            scopes: vec![HashMap::new()],
            translator_scopes: vec![HashMap::new()],
            map_scopes: vec![HashMap::new()],
            file_path,
            line_starts: collect_line_starts(source_text),
        }
    }

    fn push_usage(&mut self, kind: UsageKind, line: usize) {
        self.usages.push(Usage {
            namespaces: self.namespaces.clone(),
            kind,
            path: self.file_path.clone(),
            line,
        });
    }

    fn push_usage_with_namespaces(
        &mut self,
        namespaces: Vec<String>,
        kind: UsageKind,
        line: usize,
    ) {
        self.usages.push(Usage {
            namespaces,
            kind,
            path: self.file_path.clone(),
            line,
        });
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
        self.translator_scopes.push(HashMap::new());
        self.map_scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
        self.translator_scopes.pop();
        self.map_scopes.pop();
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

    fn bind_translator_namespace(&mut self, name: String, namespace: String) {
        if let Some(scope) = self.translator_scopes.last_mut() {
            scope.insert(name, namespace);
        }
    }

    fn resolve_translator_namespace(&self, name: &str) -> Option<String> {
        self.translator_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    fn bind_map(&mut self, name: String, map: HashMap<String, InferredValue>) {
        if let Some(scope) = self.map_scopes.last_mut() {
            scope.insert(name, map);
        }
    }

    fn resolve_map(&self, name: &str) -> Option<&HashMap<String, InferredValue>> {
        self.map_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
    }

    fn handle_use_translation<'a>(&mut self, expr: &CallExpression<'a>) {
        self.namespaces = extract_namespaces(expr);
    }

    fn handle_t_call<'a>(&mut self, expr: &CallExpression<'a>) {
        self.handle_t_call_with_base_ns(expr, None);
    }

    fn handle_t_call_with_base_ns<'a>(&mut self, expr: &CallExpression<'a>, base_ns: Option<&str>) {
        let Some(first_arg) = expr.arguments.first() else {
            return;
        };
        let ns_override = expr
            .arguments
            .get(1)
            .and_then(extract_ns_override)
            .or_else(|| base_ns.map(ToString::to_string));
        let line = self.line_for_offset(expr.span.start);

        let inferred = self.infer_argument(first_arg);

        for kind in inferred.into_usage_kinds() {
            self.push_resolved_usage(kind, ns_override.as_deref(), line);
        }
    }

    fn push_resolved_usage(&mut self, kind: UsageKind, ns_override: Option<&str>, line: usize) {
        match kind {
            UsageKind::Static(key) => {
                if let Some((ns, raw_key)) = split_colon_namespace(&key) {
                    self.push_usage_with_namespaces(vec![ns], UsageKind::Static(raw_key), line);
                } else if let Some(override_ns) = ns_override {
                    self.push_usage_with_namespaces(
                        vec![override_ns.to_string()],
                        UsageKind::Static(key),
                        line,
                    );
                } else {
                    self.push_usage(UsageKind::Static(key), line);
                }
            }
            UsageKind::Prefix(prefix) => {
                if let Some((ns, raw_prefix)) = split_colon_namespace(&prefix) {
                    self.push_usage_with_namespaces(vec![ns], UsageKind::Prefix(raw_prefix), line);
                } else if let Some(override_ns) = ns_override {
                    self.push_usage_with_namespaces(
                        vec![override_ns.to_string()],
                        UsageKind::Prefix(prefix),
                        line,
                    );
                } else {
                    self.push_usage(UsageKind::Prefix(prefix), line);
                }
            }
            UsageKind::Dynamic => {
                if let Some(override_ns) = ns_override {
                    self.push_usage_with_namespaces(
                        vec![override_ns.to_string()],
                        UsageKind::Dynamic,
                        line,
                    );
                } else {
                    self.push_usage(UsageKind::Dynamic, line);
                }
            }
        }
    }

    fn line_for_offset(&self, offset: u32) -> usize {
        let offset = offset as usize;

        match self.line_starts.binary_search(&offset) {
            Ok(index) => index + 1,
            Err(index) => index.max(1),
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

    fn track_server_translator_bindings<'a>(&mut self, decl: &VariableDeclaration<'a>) {
        for declarator in &decl.declarations {
            let Some(init) = &declarator.init else {
                continue;
            };

            let Some(namespace) = extract_server_translator_namespace_from_expression(init) else {
                continue;
            };

            let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
                continue;
            };

            self.bind_translator_namespace(binding.name.to_string(), namespace);
        }
    }

    fn track_const_map_bindings<'a>(&mut self, decl: &VariableDeclaration<'a>) {
        if decl.kind != VariableDeclarationKind::Const {
            return;
        }

        for declarator in &decl.declarations {
            let Some(init) = &declarator.init else {
                continue;
            };

            let init = strip_ts_expression_wrappers(init);
            let Expression::ObjectExpression(obj) = init else {
                continue;
            };

            let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
                continue;
            };

            let mut map = HashMap::new();

            for property in &obj.properties {
                let ObjectPropertyKind::ObjectProperty(prop) = property else {
                    continue;
                };

                if prop.method {
                    continue;
                }

                let value = self.infer_expression(&prop.value);
                for key in self.infer_object_property_keys(prop) {
                    map.insert(key, value.clone());
                }
            }

            if !map.is_empty() {
                self.bind_map(binding.name.to_string(), map);
            }
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
            Argument::ArrayExpression(array) => self.infer_array_expression(array),
            Argument::Identifier(ident) => self
                .resolve_local(ident.name.as_str())
                .unwrap_or_else(InferredValue::dynamic),
            Argument::ComputedMemberExpression(member) => self.infer_computed_member(member),
            Argument::StaticMemberExpression(member) => self.infer_static_member(member),
            Argument::CallExpression(call) => self.infer_call(call),
            Argument::ConditionalExpression(cond) => self.infer_conditional(cond),
            Argument::LogicalExpression(logical) => self.infer_logical(logical),
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
            Expression::ArrayExpression(array) => self.infer_array_expression(array),
            Expression::Identifier(ident) => self
                .resolve_local(ident.name.as_str())
                .unwrap_or_else(InferredValue::dynamic),
            Expression::ComputedMemberExpression(member) => self.infer_computed_member(member),
            Expression::StaticMemberExpression(member) => self.infer_static_member(member),
            Expression::CallExpression(call) => self.infer_call(call),
            Expression::ConditionalExpression(cond) => self.infer_conditional(cond),
            Expression::LogicalExpression(logical) => self.infer_logical(logical),
            Expression::TSAsExpression(ts) => self.infer_expression(&ts.expression),
            Expression::TSSatisfiesExpression(ts) => self.infer_expression(&ts.expression),
            Expression::TSTypeAssertion(ts) => self.infer_expression(&ts.expression),
            Expression::TSNonNullExpression(ts) => self.infer_expression(&ts.expression),
            _ => InferredValue::dynamic(),
        }
    }

    fn infer_computed_member<'a>(
        &self,
        member: &oxc_ast::ast::ComputedMemberExpression<'a>,
    ) -> InferredValue {
        let Expression::Identifier(object_ident) = &member.object else {
            return InferredValue::dynamic();
        };

        let Some(map) = self.resolve_map(object_ident.name.as_str()) else {
            return InferredValue::dynamic();
        };

        let property = self.infer_expression(&member.expression);
        let mut out = InferredValue::default();

        for key in &property.statics {
            if let Some(mapped_value) = map.get(key) {
                out.merge(mapped_value.clone());
            } else {
                out.dynamic = true;
            }
        }

        if property.dynamic || property.statics.is_empty() {
            // Unknown indexes in const maps still imply any mapped key may be used.
            for mapped_value in map.values() {
                out.merge(mapped_value.clone());
            }
            out.dynamic = true;
        }

        out
    }

    fn infer_static_member<'a>(
        &self,
        member: &oxc_ast::ast::StaticMemberExpression<'a>,
    ) -> InferredValue {
        let Expression::Identifier(object_ident) = &member.object else {
            return InferredValue::dynamic();
        };

        let Some(map) = self.resolve_map(object_ident.name.as_str()) else {
            return InferredValue::dynamic();
        };

        map.get(member.property.name.as_str())
            .cloned()
            .unwrap_or_else(InferredValue::dynamic)
    }

    fn infer_conditional<'a>(&self, cond: &ConditionalExpression<'a>) -> InferredValue {
        let mut inferred = self.infer_expression(&cond.consequent);
        inferred.merge(self.infer_expression(&cond.alternate));
        inferred
    }

    fn infer_logical<'a>(&self, logical: &LogicalExpression<'a>) -> InferredValue {
        let left = self.infer_expression(&logical.left);
        let right = self.infer_expression(&logical.right);

        if logical.operator.is_or() || logical.operator.is_coalesce() {
            // `a || b` and `a ?? b` can reach either side at runtime.
            let mut inferred = left;
            inferred.merge(right);
            return inferred;
        }

        // `a && b` may evaluate to a non-key value from the left side, so keep
        // behavior conservative and avoid adding extra static usages.
        InferredValue::dynamic()
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

    fn infer_array_expression<'a>(
        &self,
        array: &oxc_ast::ast::ArrayExpression<'a>,
    ) -> InferredValue {
        let mut inferred = InferredValue::default();

        for element in &array.elements {
            match element {
                ArrayExpressionElement::StringLiteral(s) => {
                    inferred.statics.insert(s.value.to_string());
                }
                ArrayExpressionElement::TemplateLiteral(tpl) if tpl.expressions.is_empty() => {
                    let value = tpl
                        .quasis
                        .first()
                        .map(|q| q.value.raw.as_str())
                        .unwrap_or("");
                    inferred.statics.insert(value.to_string());
                }
                _ => inferred.dynamic = true,
            }
        }

        if inferred.statics.is_empty() && !inferred.dynamic {
            InferredValue::dynamic()
        } else {
            inferred
        }
    }

    fn infer_object_property_keys<'a>(
        &self,
        property: &oxc_ast::ast::ObjectProperty<'a>,
    ) -> Vec<String> {
        if !property.computed {
            return object_property_key_to_string(&property.key)
                .into_iter()
                .collect();
        }

        let Some(key_expr) = property.key.as_expression() else {
            return Vec::new();
        };

        self.infer_expression(key_expr)
            .statics
            .into_iter()
            .collect()
    }

    fn iterator_callback_binding<'a>(
        &self,
        expr: &CallExpression<'a>,
    ) -> Option<(String, InferredValue)> {
        let Expression::StaticMemberExpression(member) = &expr.callee else {
            return None;
        };

        let method = member.property.name.as_str();
        if method != "map" && method != "forEach" {
            return None;
        }

        let iterable_values = self.infer_iterator_values(&member.object)?;
        let callback = expr.arguments.first()?;
        let param_name = extract_callback_first_param_name(callback)?;

        Some((param_name, iterable_values))
    }

    fn infer_iterator_values<'a>(&self, expression: &Expression<'a>) -> Option<InferredValue> {
        match strip_ts_expression_wrappers(expression) {
            Expression::ArrayExpression(array) => Some(self.infer_array_expression(array)),
            Expression::Identifier(ident) => self.resolve_local(ident.name.as_str()),
            _ => None,
        }
    }
}

fn extract_callback_first_param_name(argument: &Argument<'_>) -> Option<String> {
    let params = match argument {
        Argument::ArrowFunctionExpression(arrow) => &arrow.params.items,
        Argument::FunctionExpression(function) => &function.params.items,
        _ => return None,
    };

    let first = params.first()?;
    let BindingPattern::BindingIdentifier(binding) = &first.pattern else {
        return None;
    };

    Some(binding.name.to_string())
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
        self.track_server_translator_bindings(decl);
        self.track_const_map_bindings(decl);
        self.track_const_bindings(decl);
        walk::walk_variable_declaration(self, decl);
    }

    fn visit_call_expression(&mut self, expr: &CallExpression<'a>) {
        if let Some((param_name, values)) = self.iterator_callback_binding(expr) {
            self.push_scope();
            self.bind_local(param_name, values);
            walk::walk_call_expression(self, expr);
            self.pop_scope();
            return;
        }

        match &expr.callee {
            Expression::Identifier(ident) => match ident.name.as_str() {
                "useTranslation" => self.handle_use_translation(expr),
                "t" => {
                    let ns = self.resolve_translator_namespace("t");
                    self.handle_t_call_with_base_ns(expr, ns.as_deref());
                }
                _ => {
                    if let Some(namespace) = self.resolve_translator_namespace(ident.name.as_str())
                    {
                        self.handle_t_call_with_base_ns(expr, Some(namespace.as_str()));
                    }
                }
            },
            Expression::StaticMemberExpression(member) => {
                if is_i18next_t_member_call(member) {
                    self.handle_t_call(expr);
                }
            }
            _ => {}
        }

        walk::walk_call_expression(self, expr);
    }

    fn visit_jsx_opening_element(&mut self, element: &JSXOpeningElement<'a>) {
        if !is_trans_element(element) {
            walk::walk_jsx_opening_element(self, element);
            return;
        }

        let Some(i18n_key) = extract_jsx_string_attr(&element.attributes, "i18nKey") else {
            walk::walk_jsx_opening_element(self, element);
            return;
        };

        let ns_override = extract_jsx_string_attr(&element.attributes, "ns");
        let line = self.line_for_offset(element.span.start);
        self.push_resolved_usage(UsageKind::Static(i18n_key), ns_override.as_deref(), line);

        walk::walk_jsx_opening_element(self, element);
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
pub fn collect_usages(
    source_dir: &PathBuf,
    exclude_patterns: &[String],
) -> Result<Vec<Usage>, I18nError> {
    let mut all_usages: Vec<Usage> = vec![];
    let excludes = build_exclude_globset(exclude_patterns)?;
    let (walk_root, only_file) = resolve_walk_target(source_dir);

    for entry in WalkBuilder::new(&walk_root).hidden(false).build() {
        let entry = entry?;

        if !entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            continue;
        }

        let path = entry.path();
        if let Some(target_file) = &only_file {
            if path != target_file {
                continue;
            }
        }

        if !is_supported_source_file(path) {
            continue;
        }
        if is_excluded(path, &walk_root, &excludes) {
            continue;
        }

        let file_usages = parse_source_file(path)?;
        all_usages.extend(file_usages);
    }

    Ok(all_usages)
}

fn build_exclude_globset(patterns: &[String]) -> Result<GlobSet, I18nError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern).map_err(|err| {
            I18nError::Config(format!(
                "invalid src_exclude pattern '{}': {}",
                pattern, err
            ))
        })?;
        builder.add(glob);
    }

    builder.build().map_err(|err| {
        I18nError::Config(format!("failed to compile src_exclude patterns: {}", err))
    })
}

fn is_excluded(path: &Path, root: &Path, excludes: &GlobSet) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };

    matches_relative(relative, excludes)
}

fn matches_relative(relative: &Path, set: &GlobSet) -> bool {
    if set.is_match(relative) {
        return true;
    }

    let normalized = relative.to_string_lossy().replace('\\', "/");
    set.is_match(&normalized)
}

fn resolve_walk_target(target: &Path) -> (PathBuf, Option<PathBuf>) {
    if target.is_file() {
        let root = target
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        (root, Some(target.to_path_buf()))
    } else {
        (target.to_path_buf(), None)
    }
}

/// Returns whether `path` is a supported source file extension.
fn is_supported_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("ts") | Some("tsx") | Some("js") | Some("jsx")
    )
}

fn strip_ts_expression_wrappers<'a>(expression: &'a Expression<'a>) -> &'a Expression<'a> {
    match expression {
        Expression::TSAsExpression(ts) => strip_ts_expression_wrappers(&ts.expression),
        Expression::TSSatisfiesExpression(ts) => strip_ts_expression_wrappers(&ts.expression),
        Expression::TSTypeAssertion(ts) => strip_ts_expression_wrappers(&ts.expression),
        Expression::TSNonNullExpression(ts) => strip_ts_expression_wrappers(&ts.expression),
        _ => expression,
    }
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
    let mut collector = CallCollector::new(function_values, path.to_path_buf(), &source_text);

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

fn collect_line_starts(source_text: &str) -> Vec<usize> {
    let mut starts = vec![0];

    for (index, byte) in source_text.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(index + 1);
        }
    }

    starts
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

fn object_property_key_to_string(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(ident) => Some(ident.name.to_string()),
        PropertyKey::StringLiteral(string) => Some(string.value.to_string()),
        PropertyKey::TemplateLiteral(tpl) if tpl.expressions.is_empty() => {
            tpl.quasis.first().map(|q| q.value.raw.as_str().to_string())
        }
        _ => None,
    }
}

fn is_i18next_t_member_call(member: &oxc_ast::ast::StaticMemberExpression<'_>) -> bool {
    member.property.name == "t"
        && matches!(&member.object, Expression::Identifier(ident) if ident.name == "i18next")
}

fn extract_server_translator_namespace_from_expression(
    expression: &Expression<'_>,
) -> Option<String> {
    match expression {
        Expression::CallExpression(call) => extract_server_translator_namespace_from_call(call),
        Expression::AwaitExpression(await_expr) => {
            extract_server_translator_namespace_from_expression(&await_expr.argument)
        }
        _ => None,
    }
}

fn extract_server_translator_namespace_from_call(call: &CallExpression<'_>) -> Option<String> {
    let Expression::Identifier(callee) = &call.callee else {
        return None;
    };

    if callee.name != "getServerTranslate" {
        return None;
    }

    let first_arg = call.arguments.first()?;

    match first_arg {
        Argument::StringLiteral(s) => Some(s.value.to_string()),
        Argument::TemplateLiteral(tpl) if tpl.expressions.is_empty() => {
            tpl.quasis.first().map(|q| q.value.raw.as_str().to_string())
        }
        _ => None,
    }
}

fn is_trans_element(element: &JSXOpeningElement<'_>) -> bool {
    match &element.name {
        JSXElementName::Identifier(ident) => ident.name == "Trans",
        JSXElementName::IdentifierReference(ident) => ident.name == "Trans",
        _ => false,
    }
}

fn extract_jsx_string_attr(attributes: &[JSXAttributeItem<'_>], name: &str) -> Option<String> {
    for item in attributes {
        let JSXAttributeItem::Attribute(attribute) = item else {
            continue;
        };

        let JSXAttributeName::Identifier(attr_name) = &attribute.name else {
            continue;
        };

        if attr_name.name != name {
            continue;
        }

        let value = attribute.value.as_ref()?;
        return jsx_attribute_value_to_string(value);
    }

    None
}

fn jsx_attribute_value_to_string(value: &JSXAttributeValue<'_>) -> Option<String> {
    match value {
        JSXAttributeValue::StringLiteral(s) => Some(s.value.to_string()),
        JSXAttributeValue::ExpressionContainer(container) => {
            jsx_expression_to_static_string(&container.expression)
        }
        _ => None,
    }
}

fn jsx_expression_to_static_string(expression: &JSXExpression<'_>) -> Option<String> {
    match expression {
        JSXExpression::StringLiteral(s) => Some(s.value.to_string()),
        JSXExpression::TemplateLiteral(tpl) if tpl.expressions.is_empty() => {
            tpl.quasis.first().map(|q| q.value.raw.as_str().to_string())
        }
        _ => None,
    }
}
