//! Template expression evaluator.
//!
//! Takes a template JSON tree (with `$ref`, `$each`, `$if`, operators) and a
//! data JSON object, resolves all expressions, and produces a plain document
//! JSON tree that can be deserialized into a `Document` and rendered.
//!
//! This enables a hosted API workflow: store template JSON + dynamic data →
//! produce PDFs without a JavaScript runtime.

use serde_json::{Map, Value};
use std::collections::HashMap;

use crate::FormeError;

/// Evaluation context that holds data and scoped bindings (from `$each`).
pub struct EvalContext {
    data: Value,
    scope: HashMap<String, Value>,
}

impl EvalContext {
    pub fn new(data: Value) -> Self {
        EvalContext {
            data,
            scope: HashMap::new(),
        }
    }

    /// Resolve a dot-separated path against scope first, then root data.
    fn resolve_ref(&self, path: &str) -> Option<&Value> {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.is_empty() {
            return None;
        }

        // Check scope first (for $each bindings like "$item")
        if let Some(scoped) = self.scope.get(parts[0]) {
            return traverse(scoped, &parts[1..]);
        }

        // Fall back to root data
        traverse(&self.data, &parts)
    }

    /// Create a child context with an additional binding.
    fn with_binding(&self, key: &str, value: Value) -> EvalContext {
        let mut scope = self.scope.clone();
        scope.insert(key.to_string(), value);
        EvalContext {
            data: self.data.clone(),
            scope,
        }
    }
}

/// Traverse a JSON value by dot-path segments.
fn traverse<'a>(value: &'a Value, parts: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for part in parts {
        match current {
            Value::Object(map) => {
                current = map.get(*part)?;
            }
            Value::Array(arr) => {
                let idx: usize = part.parse().ok()?;
                current = arr.get(idx)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

/// Evaluate a template JSON tree with data, producing a resolved document.
pub fn evaluate_template(template: &Value, data: &Value) -> Result<Value, FormeError> {
    let ctx = EvalContext::new(data.clone());
    evaluate_node(template, &ctx).ok_or_else(|| {
        FormeError::TemplateError("Template evaluation produced no output".to_string())
    })
}

/// Evaluate a single node in the template tree.
fn evaluate_node(node: &Value, ctx: &EvalContext) -> Option<Value> {
    match node {
        Value::Object(map) => {
            // Check for expression nodes first
            if let Some(result) = evaluate_expr_object(map, ctx) {
                return result;
            }

            // Regular object — evaluate all values recursively
            let mut result = Map::new();
            for (key, val) in map {
                if let Some(evaluated) = evaluate_node(val, ctx) {
                    result.insert(key.clone(), evaluated);
                }
            }
            Some(Value::Object(result))
        }
        Value::Array(arr) => {
            let mut result = Vec::new();
            for item in arr {
                if let Some(evaluated) = evaluate_node(item, ctx) {
                    // $each results get flattened
                    if is_flatten_marker(&evaluated) {
                        if let Value::Array(inner) =
                            evaluated.get("__flatten").unwrap_or(&Value::Null)
                        {
                            result.extend(inner.clone());
                        }
                    } else {
                        result.push(evaluated);
                    }
                }
            }
            Some(Value::Array(result))
        }
        // Primitives pass through unchanged
        _ => Some(node.clone()),
    }
}

fn is_flatten_marker(v: &Value) -> bool {
    matches!(v, Value::Object(map) if map.contains_key("__flatten"))
}

/// Try to evaluate an object as an expression node.
/// Returns `Some(Some(value))` if it was an expression that produced a value,
/// `Some(None)` if it was an expression that produced nothing (e.g. false $if),
/// `None` if this is not an expression object.
fn evaluate_expr_object(map: &Map<String, Value>, ctx: &EvalContext) -> Option<Option<Value>> {
    // $ref — data lookup
    if let Some(path) = map.get("$ref") {
        if let Value::String(path_str) = path {
            return Some(ctx.resolve_ref(path_str).cloned());
        }
        return Some(None);
    }

    // $each — array iteration
    if let Some(source) = map.get("$each") {
        return Some(evaluate_each(source, map, ctx));
    }

    // $if — conditional rendering
    if let Some(condition) = map.get("$if") {
        return Some(evaluate_if(condition, map, ctx));
    }

    // $cond — ternary value [condition, if_true, if_false]
    if let Some(args) = map.get("$cond") {
        return Some(evaluate_cond(args, ctx));
    }

    // Comparison operators
    if let Some(args) = map.get("$eq") {
        return Some(evaluate_comparison(args, ctx, CompareOp::Eq));
    }
    if let Some(args) = map.get("$ne") {
        return Some(evaluate_comparison(args, ctx, CompareOp::Ne));
    }
    if let Some(args) = map.get("$gt") {
        return Some(evaluate_comparison(args, ctx, CompareOp::Gt));
    }
    if let Some(args) = map.get("$lt") {
        return Some(evaluate_comparison(args, ctx, CompareOp::Lt));
    }
    if let Some(args) = map.get("$gte") {
        return Some(evaluate_comparison(args, ctx, CompareOp::Gte));
    }
    if let Some(args) = map.get("$lte") {
        return Some(evaluate_comparison(args, ctx, CompareOp::Lte));
    }

    // Arithmetic operators
    if let Some(args) = map.get("$add") {
        return Some(evaluate_arithmetic(args, ctx, |a, b| a + b));
    }
    if let Some(args) = map.get("$sub") {
        return Some(evaluate_arithmetic(args, ctx, |a, b| a - b));
    }
    if let Some(args) = map.get("$mul") {
        return Some(evaluate_arithmetic(args, ctx, |a, b| a * b));
    }
    if let Some(args) = map.get("$div") {
        return Some(evaluate_arithmetic(args, ctx, |a, b| {
            if b != 0.0 {
                a / b
            } else {
                0.0
            }
        }));
    }

    // String transforms
    if let Some(arg) = map.get("$upper") {
        return Some(evaluate_string_transform(arg, ctx, |s| s.to_uppercase()));
    }
    if let Some(arg) = map.get("$lower") {
        return Some(evaluate_string_transform(arg, ctx, |s| s.to_lowercase()));
    }

    // $concat — string concatenation
    if let Some(args) = map.get("$concat") {
        return Some(evaluate_concat(args, ctx));
    }

    // $format — number formatting [value, format_string]
    if let Some(args) = map.get("$format") {
        return Some(evaluate_format(args, ctx));
    }

    // $count — array length
    if let Some(arg) = map.get("$count") {
        return Some(evaluate_count(arg, ctx));
    }

    // Not an expression object
    None
}

// ─── Expression evaluators ──────────────────────────────────────────

fn evaluate_each(source: &Value, map: &Map<String, Value>, ctx: &EvalContext) -> Option<Value> {
    let resolved_source = evaluate_node(source, ctx)?;
    let arr = match &resolved_source {
        Value::Array(a) => a,
        _ => return Some(Value::Array(vec![])),
    };

    if arr.is_empty() {
        let mut marker = Map::new();
        marker.insert("__flatten".to_string(), Value::Array(vec![]));
        return Some(Value::Object(marker));
    }

    let binding_name = map.get("as").and_then(|v| v.as_str()).unwrap_or("$item");

    let template = map.get("template")?;

    let mut results = Vec::new();
    for item in arr {
        let child_ctx = ctx.with_binding(binding_name, item.clone());
        if let Some(evaluated) = evaluate_node(template, &child_ctx) {
            results.push(evaluated);
        }
    }

    // Return a flatten marker so parent arrays can flatten these results
    let mut marker = Map::new();
    marker.insert("__flatten".to_string(), Value::Array(results));
    Some(Value::Object(marker))
}

fn evaluate_if(condition: &Value, map: &Map<String, Value>, ctx: &EvalContext) -> Option<Value> {
    let resolved_cond = evaluate_node(condition, ctx)?;
    if is_truthy(&resolved_cond) {
        map.get("then").and_then(|t| evaluate_node(t, ctx))
    } else {
        map.get("else").and_then(|e| evaluate_node(e, ctx))
    }
}

fn evaluate_cond(args: &Value, ctx: &EvalContext) -> Option<Value> {
    let arr = args.as_array()?;
    if arr.len() != 3 {
        return None;
    }
    let condition = evaluate_node(&arr[0], ctx)?;
    if is_truthy(&condition) {
        evaluate_node(&arr[1], ctx)
    } else {
        evaluate_node(&arr[2], ctx)
    }
}

enum CompareOp {
    Eq,
    Ne,
    Gt,
    Lt,
    Gte,
    Lte,
}

fn evaluate_comparison(args: &Value, ctx: &EvalContext, op: CompareOp) -> Option<Value> {
    let arr = args.as_array()?;
    if arr.len() != 2 {
        return None;
    }
    let a = evaluate_node(&arr[0], ctx)?;
    let b = evaluate_node(&arr[1], ctx)?;
    Some(Value::Bool(compare_values(&a, &b, &op)))
}

/// Compare two JSON values. Numbers compared as f64, otherwise equality only.
fn compare_values(a: &Value, b: &Value, op: &CompareOp) -> bool {
    match (as_f64(a), as_f64(b)) {
        (Some(na), Some(nb)) => match op {
            CompareOp::Eq => na == nb,
            CompareOp::Ne => na != nb,
            CompareOp::Gt => na > nb,
            CompareOp::Lt => na < nb,
            CompareOp::Gte => na >= nb,
            CompareOp::Lte => na <= nb,
        },
        _ => match op {
            CompareOp::Eq => a == b,
            CompareOp::Ne => a != b,
            // Non-numeric ordered comparisons: compare string representations
            CompareOp::Gt | CompareOp::Lt | CompareOp::Gte | CompareOp::Lte => {
                match (a.as_str(), b.as_str()) {
                    (Some(sa), Some(sb)) => match op {
                        CompareOp::Gt => sa > sb,
                        CompareOp::Lt => sa < sb,
                        CompareOp::Gte => sa >= sb,
                        CompareOp::Lte => sa <= sb,
                        _ => unreachable!(),
                    },
                    _ => false,
                }
            }
        },
    }
}

fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        _ => None,
    }
}

fn evaluate_arithmetic(args: &Value, ctx: &EvalContext, op: fn(f64, f64) -> f64) -> Option<Value> {
    let arr = args.as_array()?;
    if arr.len() != 2 {
        return None;
    }
    let a = evaluate_node(&arr[0], ctx).and_then(|v| as_f64(&v))?;
    let b = evaluate_node(&arr[1], ctx).and_then(|v| as_f64(&v))?;
    let result = op(a, b);
    Some(serde_json::Number::from_f64(result).map_or(Value::Null, Value::Number))
}

fn evaluate_string_transform(
    arg: &Value,
    ctx: &EvalContext,
    transform: fn(&str) -> String,
) -> Option<Value> {
    let resolved = evaluate_node(arg, ctx)?;
    let s = value_to_string(&resolved)?;
    Some(Value::String(transform(&s)))
}

fn evaluate_concat(args: &Value, ctx: &EvalContext) -> Option<Value> {
    let arr = args.as_array()?;
    let mut result = String::new();
    for item in arr {
        let resolved = evaluate_node(item, ctx)?;
        result.push_str(&value_to_string(&resolved)?);
    }
    Some(Value::String(result))
}

fn evaluate_format(args: &Value, ctx: &EvalContext) -> Option<Value> {
    let arr = args.as_array()?;
    if arr.len() != 2 {
        return None;
    }
    let value = evaluate_node(&arr[0], ctx).and_then(|v| as_f64(&v))?;
    let format_str = evaluate_node(&arr[1], ctx)?;
    let fmt = format_str.as_str()?;

    // Parse format string like "0.00" to determine decimal places
    let decimal_places = if let Some(dot_pos) = fmt.find('.') {
        fmt.len() - dot_pos - 1
    } else {
        0
    };

    Some(Value::String(format!(
        "{:.prec$}",
        value,
        prec = decimal_places
    )))
}

fn evaluate_count(arg: &Value, ctx: &EvalContext) -> Option<Value> {
    let resolved = evaluate_node(arg, ctx)?;
    match &resolved {
        Value::Array(arr) => Some(Value::Number(serde_json::Number::from(arr.len()))),
        _ => Some(Value::Number(serde_json::Number::from(0))),
    }
}

// ─── Helpers ────────────────────────────────────────────────────────

/// Determine if a JSON value is truthy.
fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(_) => true,
    }
}

/// Convert a JSON value to a string for string operations.
fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => Some("".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_resolve_ref_simple() {
        let ctx = EvalContext::new(json!({"name": "Alice"}));
        assert_eq!(ctx.resolve_ref("name"), Some(&json!("Alice")));
    }

    #[test]
    fn test_resolve_ref_nested() {
        let ctx = EvalContext::new(json!({"user": {"name": "Bob"}}));
        assert_eq!(ctx.resolve_ref("user.name"), Some(&json!("Bob")));
    }

    #[test]
    fn test_resolve_ref_scope_first() {
        let ctx = EvalContext::new(json!({"name": "root"}));
        let child = ctx.with_binding("$item", json!({"name": "scoped"}));
        assert_eq!(child.resolve_ref("$item.name"), Some(&json!("scoped")));
    }

    #[test]
    fn test_resolve_ref_missing() {
        let ctx = EvalContext::new(json!({"name": "Alice"}));
        assert_eq!(ctx.resolve_ref("missing"), None);
    }

    #[test]
    fn test_is_truthy() {
        assert!(!is_truthy(&json!(null)));
        assert!(!is_truthy(&json!(false)));
        assert!(!is_truthy(&json!(0)));
        assert!(!is_truthy(&json!("")));
        assert!(!is_truthy(&json!([])));

        assert!(is_truthy(&json!(true)));
        assert!(is_truthy(&json!(1)));
        assert!(is_truthy(&json!("hello")));
        assert!(is_truthy(&json!([1])));
        assert!(is_truthy(&json!({"a": 1})));
    }

    #[test]
    fn test_evaluate_ref() {
        let template = json!({"$ref": "name"});
        let data = json!({"name": "Alice"});
        let result = evaluate_template(&template, &data).unwrap();
        assert_eq!(result, json!("Alice"));
    }

    #[test]
    fn test_passthrough() {
        let template = json!({"type": "Text", "content": "hello"});
        let data = json!({});
        let result = evaluate_template(&template, &data).unwrap();
        assert_eq!(result, json!({"type": "Text", "content": "hello"}));
    }
}
