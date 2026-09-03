use crate::tools::RecoverableError;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;

/// Recursive filter AST ported from redis/agent-memory-server filters.py
/// (Apache-2.0). See CREDITS.md.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FilterNode {
    And { and: Vec<FilterNode> },
    Or { or: Vec<FilterNode> },
    Not { not: Box<FilterNode> },
    Leaf(serde_json::Map<String, Value>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeafOp {
    Eq,
    Ne,
    In,
    Nin,
    Gt,
    Lt,
    Gte,
    Lte,
    Contains,
    Prefix,
}

impl std::str::FromStr for LeafOp {
    type Err = ();
    fn from_str(s: &str) -> std::result::Result<Self, ()> {
        Ok(match s {
            "eq" => Self::Eq,
            "ne" => Self::Ne,
            "in" => Self::In,
            "nin" => Self::Nin,
            "gt" => Self::Gt,
            "lt" => Self::Lt,
            "gte" => Self::Gte,
            "lte" => Self::Lte,
            "contains" => Self::Contains,
            "prefix" => Self::Prefix,
            _ => return Err(()),
        })
    }
}

impl LeafOp {
    fn sql(self) -> &'static str {
        match self {
            Self::Eq => "=",
            Self::Ne => "!=",
            Self::In => "IN",
            Self::Nin => "NOT IN",
            Self::Gt => ">",
            Self::Lt => "<",
            Self::Gte => ">=",
            Self::Lte => "<=",
            Self::Contains => "LIKE",
            Self::Prefix => "LIKE",
        }
    }
}

pub struct SqlFragment {
    pub sql: String,
    pub params: Vec<rusqlite::types::Value>,
}

// rel_path is accepted as a filter alias for abs_path (schema v6 dropped
// the separate rel_path column; abs_path is now the single path field).
// repo was also dropped in v6 — use the `scope` param to narrow by repo.
const ALLOWED_FIELDS: &[&str] = &[
    "kind",
    "status",
    "topic",
    "time_scope",
    "updated_at",
    "created_at",
    "confidence",
    "tags",
    "owners",
    "rel_path",
    "abs_path",
    "title",
    "id",
];

pub fn compile(node: &FilterNode) -> Result<SqlFragment> {
    match node {
        FilterNode::And { and } => compile_composition("AND", and),
        FilterNode::Or { or } => compile_composition("OR", or),
        FilterNode::Not { not } => {
            let inner = compile(not)?;
            Ok(SqlFragment {
                sql: format!("NOT ({})", inner.sql),
                params: inner.params,
            })
        }
        FilterNode::Leaf(map) => compile_leaf(map),
    }
}

fn compile_composition(op: &str, children: &[FilterNode]) -> Result<SqlFragment> {
    if children.is_empty() {
        return Err(RecoverableError::with_hint(
            format!("empty composition `{op}`"),
            "`and` / `or` / `not` require at least one child filter — drop the composition or add children.",
        )
        .into());
    }
    let mut parts = Vec::new();
    let mut params = Vec::new();
    for c in children {
        let f = compile(c)?;
        parts.push(f.sql);
        params.extend(f.params);
    }
    Ok(SqlFragment {
        sql: format!("({})", parts.join(&format!(" {op} "))),
        params,
    })
}

fn compile_leaf(map: &serde_json::Map<String, Value>) -> Result<SqlFragment> {
    if map.len() != 1 {
        return Err(RecoverableError::with_hint(
            format!("leaf must have exactly one field, got {}", map.len()),
            "Each leaf has shape `{field: {op: value}}`. Wrap multiple fields with `and`/`or`.",
        )
        .into());
    }
    let (field, ops) = map.iter().next().unwrap();

    if !ALLOWED_FIELDS.contains(&field.as_str()) {
        return Err(RecoverableError::with_hint(
            format!("unknown field `{}`", field),
            format!("allowed fields: {:?}", ALLOWED_FIELDS),
        )
        .into());
    }

    // rel_path was dropped in schema v6; abs_path is the DB column now.
    // Remap here so documented filter examples continue to work.
    let sql_field = if field == "rel_path" {
        "abs_path"
    } else {
        field.as_str()
    };

    let ops = ops.as_object().ok_or_else(|| {
        RecoverableError::with_hint(
            "ops must be an object",
            "Leaf op shape is `{field: {op: value}}`, e.g. `{\"kind\": {\"eq\": \"tracker\"}}`.",
        )
    })?;
    if ops.len() != 1 {
        return Err(RecoverableError::with_hint(
            format!("exactly one op per leaf, got {}", ops.len()),
            "Wrap multiple ops on the same field with `and`/`or`, e.g. {\"and\":[{\"f\":{\"gt\":1}},{\"f\":{\"lt\":9}}]}.",
        )
        .into());
    }
    let (op_name, value) = ops.iter().next().unwrap();
    let op = op_name.parse::<LeafOp>().map_err(|_| {
        RecoverableError::with_hint(
            format!("unknown op `{op_name}`"),
            "valid ops: eq, ne, in, nin, gt, lt, gte, lte, contains, prefix",
        )
    })?;

    // `tags` / `owners` are stored as JSON arrays, so EVERY membership op on
    // them has to go through `json_each`. Comparing the column directly
    // compares its raw JSON *text* (`["a","b"]`) against a scalar, which is
    // never equal and fails silently: `in` returned nothing and `nin` returned
    // everything. Gating this on the op was the bug (BL-47); it is now gated on
    // the column, with the op selecting the shape.
    let is_array_col = matches!(field.as_str(), "tags" | "owners");
    if is_array_col {
        match op {
            LeafOp::Contains => {
                let lit = json_value_to_sql(value)?;
                return Ok(SqlFragment {
                    sql: format!("EXISTS (SELECT 1 FROM json_each({sql_field}) WHERE value = ?)"),
                    params: vec![lit],
                });
            }
            LeafOp::In | LeafOp::Nin => {
                let params = in_list_params(value)?;
                let placeholders = std::iter::repeat_n("?", params.len())
                    .collect::<Vec<_>>()
                    .join(", ");
                let exists = format!(
                    "EXISTS (SELECT 1 FROM json_each({sql_field}) WHERE value IN ({placeholders}))"
                );
                return Ok(SqlFragment {
                    sql: if op == LeafOp::In {
                        exists
                    } else {
                        // Holding none of the listed values includes holding
                        // none at all, so an empty array matches `nin`.
                        format!("NOT {exists}")
                    },
                    params,
                });
            }
            _ => {}
        }
    }

    // `rel_path` is a caller-facing ALIAS, not a column: `abs_path` holds an
    // ABSOLUTE forward-slash path (`/root/docs/x.md`) while every documented
    // filter value is repo-relative (`docs/x.md`). Remapping the field NAME
    // alone left the two in different coordinate systems, so nine of the ten
    // ops silently compared unrelated strings — `eq`/`in`/`prefix` matched
    // nothing, `ne`/`nin` matched everything including the rows they named.
    // That is the same failure BL-47 fixed for `tags`/`owners` directly above,
    // and it takes the same shape of remedy: gate on the COLUMN, let the op
    // pick the form. Anchoring each comparison at a `/` boundary makes it
    // root-agnostic, so it stays correct for an umbrella query spanning several
    // repos — where no single root exists to normalise against.
    //
    // Residual imprecision, stated rather than hidden: `%/docs/trackers%` also
    // matches a nested `…/vendor/docs/trackers/…`. Under the default
    // `scope="project"` the AND'd scope clause already pins the root, so this
    // needs a second `docs/trackers` deeper in the same tree to bite.
    if field == "rel_path" {
        let as_str = |v: &Value| -> Result<String> {
            v.as_str().map(str::to_owned).ok_or_else(|| {
                RecoverableError::with_hint(
                    format!("`{op_name}` on `rel_path` expects a string"),
                    "Provide a repo-relative path, e.g. `{\"eq\": \"docs/plans/x.md\"}`.",
                )
                .into()
            })
        };
        // Matches a row whose repo-relative path IS `s` — i.e. whose absolute
        // path ends with `/s`.
        let suffix_like = |s: &str| -> (String, rusqlite::types::Value) {
            let escaped = crate::librarian::util::escape_like_pattern(s);
            (
                "abs_path LIKE ? ESCAPE '\\'".to_string(),
                rusqlite::types::Value::Text(format!("%/{escaped}")),
            )
        };

        match op {
            // Already correct: a substring match is position-independent, so it
            // never depended on where the relative part begins.
            LeafOp::Contains => {}
            LeafOp::Gt | LeafOp::Lt | LeafOp::Gte | LeafOp::Lte => {
                return Err(RecoverableError::with_hint(
                    format!("`{op_name}` is not meaningful on `rel_path`"),
                    "`rel_path` is an alias for the absolute `abs_path` column, so ordering \
                     a repo-relative value against it compares a different string than the \
                     one you wrote. Use `prefix` for a directory, `eq` for one file, \
                     `contains` for a fragment — or order on `updated_at` / `created_at` \
                     if a range was what you meant.",
                )
                .into());
            }
            LeafOp::Prefix => {
                let escaped = crate::librarian::util::escape_like_pattern(&as_str(value)?);
                return Ok(SqlFragment {
                    sql: "abs_path LIKE ? ESCAPE '\\'".to_string(),
                    params: vec![rusqlite::types::Value::Text(format!("%/{escaped}%"))],
                });
            }
            LeafOp::Eq | LeafOp::Ne => {
                let (sql, param) = suffix_like(&as_str(value)?);
                return Ok(SqlFragment {
                    sql: if op == LeafOp::Eq {
                        sql
                    } else {
                        format!("NOT ({sql})")
                    },
                    params: vec![param],
                });
            }
            LeafOp::In | LeafOp::Nin => {
                let items = value.as_array().ok_or_else(|| -> anyhow::Error {
                    RecoverableError::with_hint(
                        format!("`{op_name}` on `rel_path` expects an array"),
                        "e.g. `{\"in\": [\"docs/a.md\", \"docs/b.md\"]}`.",
                    )
                    .into()
                })?;
                if items.is_empty() {
                    return Err(RecoverableError::with_hint(
                        format!("`{op_name}` on `rel_path` requires a non-empty array"),
                        "An empty list matches nothing (`in`) or everything (`nin`); say \
                         which you meant.",
                    )
                    .into());
                }
                let mut clauses = Vec::with_capacity(items.len());
                let mut params = Vec::with_capacity(items.len());
                for item in items {
                    let (sql, param) = suffix_like(&as_str(item)?);
                    clauses.push(sql);
                    params.push(param);
                }
                let any = format!("({})", clauses.join(" OR "));
                return Ok(SqlFragment {
                    sql: if op == LeafOp::In {
                        any
                    } else {
                        format!("NOT {any}")
                    },
                    params,
                });
            }
        }
    }

    match op {
        LeafOp::In | LeafOp::Nin => {
            let params = in_list_params(value)?;
            let placeholders = std::iter::repeat_n("?", params.len())
                .collect::<Vec<_>>()
                .join(", ");
            Ok(SqlFragment {
                sql: format!("{sql_field} {} ({})", op.sql(), placeholders),
                params,
            })
        }
        LeafOp::Contains => {
            let s = value.as_str().ok_or_else(|| {
                RecoverableError::with_hint(
                    "`contains` expects a string",
                    "Provide a string value, e.g. `{\"contains\": \"docs/trackers\"}`.",
                )
            })?;
            // Escape the caller's value and declare an escape character, as the
            // `Prefix` arm below does. Without both, `%` and `_` inside the
            // value are live SQL wildcards and there is NO way to write either
            // literally — a caller passing `\%` binds an ordinary backslash into
            // a pattern with no escape char declared, so the match gets
            // narrower and stays wrong. `_` is the expensive one: it is common
            // in identifiers and filenames, and `contains` is the op this
            // repo's own docs recommend for path filters.
            let escaped = crate::librarian::util::escape_like_pattern(s);
            Ok(SqlFragment {
                sql: format!("{sql_field} LIKE ? ESCAPE '\\'"),
                params: vec![rusqlite::types::Value::Text(format!("%{escaped}%"))],
            })
        }
        LeafOp::Prefix => {
            let s = value.as_str().ok_or_else(|| {
                RecoverableError::with_hint(
                    "`prefix` expects a string",
                    "Provide a string value, e.g. `{\"prefix\": \"docs/\"}`.",
                )
            })?;
            let escaped = crate::librarian::util::escape_like_pattern(s);
            Ok(SqlFragment {
                sql: format!("{sql_field} LIKE ? ESCAPE '\\'"),
                params: vec![rusqlite::types::Value::Text(format!("{escaped}%"))],
            })
        }
        _ => Ok(SqlFragment {
            sql: format!("{sql_field} {} ?", op.sql()),
            params: vec![json_value_to_sql(value)?],
        }),
    }
}

/// Repair the inverted-leaf mistake — `{op: {field, value}}` written instead of
/// the canonical `{field: {op: value}}` — in place, returning a description of
/// each correction applied. usage.db shows this is the most common filter error;
/// the generic "unknown field" error used to force a retry (a second LLM call).
/// Repairing lets the query run on the first try while the returned note teaches
/// the canonical shape. A leaf is treated as inverted only when its single key
/// parses as an op AND its value is an object carrying a `field` key — an
/// unambiguous, deterministic signature, so the transform never guesses.
pub fn repair_inverted_leaves(node: &mut FilterNode) -> Vec<String> {
    let mut corrections = Vec::new();
    repair_node(node, &mut corrections);
    corrections
}

fn repair_node(node: &mut FilterNode, corrections: &mut Vec<String>) {
    match node {
        FilterNode::And { and } => {
            for child in and.iter_mut() {
                repair_node(child, corrections);
            }
        }
        FilterNode::Or { or } => {
            for child in or.iter_mut() {
                repair_node(child, corrections);
            }
        }
        FilterNode::Not { not } => repair_node(not, corrections),
        FilterNode::Leaf(map) => {
            if map.len() != 1 {
                return;
            }
            // Clone the single (key, value) so the map borrow ends before we mutate.
            let (key, val) = {
                let (k, v) = map.iter().next().unwrap();
                (k.clone(), v.clone())
            };
            // Canonical leaves key on a field name; only an op-keyed leaf whose
            // value carries `field` is the inverted shape.
            if key.parse::<LeafOp>().is_err() {
                return;
            }
            let Some(inner) = val.as_object() else {
                return;
            };
            let Some(field) = inner.get("field").and_then(|v| v.as_str()) else {
                return;
            };
            let inner_value = inner
                .get("value")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let mut op_map = serde_json::Map::new();
            op_map.insert(key.clone(), inner_value);
            let mut corrected = serde_json::Map::new();
            corrected.insert(field.to_string(), serde_json::Value::Object(op_map));
            let before = serde_json::Value::Object({
                let mut m = serde_json::Map::new();
                m.insert(key.clone(), val.clone());
                m
            });
            let after = serde_json::Value::Object(corrected.clone());
            corrections.push(format!("inverted filter leaf repaired: {before} → {after}"));
            *map = corrected;
        }
    }
}

/// Evaluate a filter AST against one entry object, in memory.
///
/// Sibling to `compile` (which emits SQL). No `ALLOWED_FIELDS` gate —
/// matching a JSON map has no injection surface and entry fields are
/// arbitrary by design. Op semantics mirror `compile_leaf`.
pub fn eval(node: &FilterNode, entry: &serde_json::Map<String, Value>) -> Result<bool> {
    match node {
        FilterNode::And { and } => {
            if and.is_empty() {
                return Err(empty_composition_err("and"));
            }
            for c in and {
                if !eval(c, entry)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        FilterNode::Or { or } => {
            if or.is_empty() {
                return Err(empty_composition_err("or"));
            }
            for c in or {
                if eval(c, entry)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        FilterNode::Not { not } => Ok(!eval(not, entry)?),
        FilterNode::Leaf(map) => eval_leaf(map, entry),
    }
}
/// Collect the set of field names referenced by any leaf in the filter AST.
///
/// Used by the `entry_filter` path to warn when a filter names a field that is
/// absent from every entry. The in-memory [`eval`] path intentionally has no
/// field allowlist (unlike the SQL [`compile`] path, which errors on unknown
/// columns), so a typo'd field silently matches nothing. Surfacing the
/// referenced fields lets the caller distinguish a typo from a true zero-match.
/// See the metadata-filtering session log, F-7.
pub fn referenced_fields(node: &FilterNode) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    collect_referenced_fields(node, &mut out);
    out
}

fn collect_referenced_fields(node: &FilterNode, out: &mut std::collections::BTreeSet<String>) {
    match node {
        FilterNode::And { and } => and.iter().for_each(|n| collect_referenced_fields(n, out)),
        FilterNode::Or { or } => or.iter().for_each(|n| collect_referenced_fields(n, out)),
        FilterNode::Not { not } => collect_referenced_fields(not, out),
        FilterNode::Leaf(map) => {
            for k in map.keys() {
                out.insert(k.clone());
            }
        }
    }
}

fn empty_composition_err(op: &str) -> anyhow::Error {
    RecoverableError::with_hint(
        format!("empty composition `{op}`"),
        "`and` / `or` / `not` require at least one child filter.",
    )
    .into()
}

fn eval_leaf(
    map: &serde_json::Map<String, Value>,
    entry: &serde_json::Map<String, Value>,
) -> Result<bool> {
    if map.len() != 1 {
        return Err(RecoverableError::with_hint(
            format!("leaf must have exactly one field, got {}", map.len()),
            "Each leaf has shape `{field: {op: value}}`. Wrap multiple fields with `and`/`or`.",
        )
        .into());
    }
    let (field, ops) = map.iter().next().unwrap();
    let ops = ops.as_object().ok_or_else(|| {
        RecoverableError::with_hint(
            "ops must be an object",
            "Leaf op shape is `{field: {op: value}}`.",
        )
    })?;
    if ops.len() != 1 {
        return Err(RecoverableError::with_hint(
            format!("exactly one op per leaf, got {}", ops.len()),
            "Wrap multiple ops on the same field with `and`/`or`.",
        )
        .into());
    }
    let (op_name, value) = ops.iter().next().unwrap();
    let op = op_name.parse::<LeafOp>().map_err(|_| {
        RecoverableError::with_hint(
            format!("unknown op `{op_name}`"),
            "valid ops: eq, ne, in, nin, gt, lt, gte, lte, contains, prefix",
        )
    })?;

    // Missing field never matches (mirrors SQL NULL comparison semantics).
    let Some(actual) = entry.get(field) else {
        return Ok(false);
    };

    Ok(match op {
        LeafOp::Eq => json_eq(actual, value),
        LeafOp::Ne => !json_eq(actual, value),
        LeafOp::In | LeafOp::Nin => {
            let arr = value.as_array().ok_or_else(|| {
                RecoverableError::with_hint(
                    "`in`/`nin` expects an array",
                    "Provide a JSON array, e.g. `{\"in\": [\"a\", \"b\"]}`.",
                )
            })?;
            if arr.is_empty() {
                return Err(RecoverableError::with_hint(
                    "`in`/`nin` expects a non-empty array",
                    "Provide at least one value, e.g. `{\"in\": [\"a\", \"b\"]}`.",
                )
                .into());
            }
            // Type-driven, mirroring the `Contains` arm below: an array field
            // means membership ("holds any of these"), never whole-value
            // equality. Without this, `{"tags": {"in": [...]}}` compares the
            // entire array against each scalar and is always false — the eval
            // twin of the SQL defect above (BL-47). The two engines answer the
            // same AST and must not disagree.
            let hit = match actual {
                Value::Array(items) => items
                    .iter()
                    .any(|item| arr.iter().any(|v| json_eq(item, v))),
                _ => arr.iter().any(|v| json_eq(actual, v)),
            };
            if op == LeafOp::In {
                hit
            } else {
                !hit
            }
        }
        LeafOp::Gt => json_cmp(actual, value) == Some(Ordering::Greater),
        LeafOp::Lt => json_cmp(actual, value) == Some(Ordering::Less),
        LeafOp::Gte => matches!(
            json_cmp(actual, value),
            Some(Ordering::Greater | Ordering::Equal)
        ),
        LeafOp::Lte => matches!(
            json_cmp(actual, value),
            Some(Ordering::Less | Ordering::Equal)
        ),
        LeafOp::Contains => match actual {
            Value::String(s) => {
                let needle = value.as_str().ok_or_else(|| {
                    RecoverableError::with_hint(
                        "`contains` on a string field expects a string value",
                        "Provide a string, e.g. `{\"contains\": \"foo\"}`.",
                    )
                })?;
                s.to_ascii_lowercase()
                    .contains(&needle.to_ascii_lowercase())
            }
            Value::Array(items) => items.iter().any(|v| json_eq(v, value)),
            _ => false,
        },
        LeafOp::Prefix => {
            let pfx = value.as_str().ok_or_else(|| {
                RecoverableError::with_hint(
                    "`prefix` expects a string",
                    "Provide a string value, e.g. `{\"prefix\": \"docs/\"}`.",
                )
            })?;
            actual.as_str().map(|s| s.starts_with(pfx)).unwrap_or(false)
        }
    })
}

/// JSON value equality with numeric coercion (`2` == `2.0`).
fn json_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            matches!((x.as_f64(), y.as_f64()), (Some(p), Some(q)) if p == q)
        }
        _ => a == b,
    }
}

/// Ordering for gt/lt/gte/lte: numbers numerically, strings lexically.
/// Mismatched or non-orderable types → None (treated as non-match).
fn json_cmp(a: &Value, b: &Value) -> Option<Ordering> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64()?.partial_cmp(&y.as_f64()?),
        (Value::String(x), Value::String(y)) => Some(x.cmp(y)),
        _ => None,
    }
}

/// Validate an `in` / `nin` operand and lower it to SQL params.
///
/// Shared by the scalar and array-column paths deliberately: the defect this
/// exists to prevent was a second `in` code path that never learned what the
/// first one knew. `tags`/`owners` are JSON arrays, and `is_array_col` was
/// consulted only on the `Contains` arm, so `in` fell through to
/// `tags IN (?)` — comparing the column's raw JSON text against a scalar,
/// which is never equal. See `open-issue-work-queue:BL-47`.
fn in_list_params(value: &Value) -> Result<Vec<rusqlite::types::Value>> {
    let arr = value.as_array().ok_or_else(|| {
        RecoverableError::with_hint(
            "`in` expects an array",
            "Provide a JSON array, e.g. `{\"in\": [\"a\", \"b\"]}`.",
        )
    })?;
    if arr.is_empty() {
        return Err(RecoverableError::with_hint(
            "`in` expects a non-empty array",
            "Provide at least one value, e.g. `{\"in\": [\"a\", \"b\"]}`.",
        )
        .into());
    }
    arr.iter().map(json_value_to_sql).collect()
}

fn json_value_to_sql(v: &Value) -> Result<rusqlite::types::Value> {
    Ok(match v {
        Value::Null => rusqlite::types::Value::Null,
        Value::Bool(b) => rusqlite::types::Value::Integer(i64::from(*b)),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                rusqlite::types::Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                rusqlite::types::Value::Real(f)
            } else {
                return Err(RecoverableError::with_hint(
                    format!("unrepresentable number: {n}"),
                    "Filter values must be finite integers or floats.",
                )
                .into())
            }
        }
        Value::String(s) => rusqlite::types::Value::Text(s.clone()),
        _ => {
            return Err(RecoverableError::with_hint(
                "arrays/objects not allowed in leaf op",
                "Leaf-op values must be scalars (string, number, bool, null). Use `in`/`nin` for arrays.",
            )
            .into())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(v: Value) -> FilterNode {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn compiles_simple_eq() {
        let node = parse(json!({"kind": {"eq": "spec"}}));
        let f = compile(&node).unwrap();
        assert_eq!(f.sql, "kind = ?");
        assert_eq!(f.params.len(), 1);
    }

    #[test]
    fn compiles_in_list() {
        let node = parse(json!({"status": {"in": ["active", "blocked"]}}));
        let f = compile(&node).unwrap();
        assert_eq!(f.sql, "status IN (?, ?)");
        assert_eq!(f.params.len(), 2);
    }

    #[test]
    fn compiles_and_composition() {
        let node = parse(json!({"and": [
            {"kind": {"eq": "spec"}},
            {"status": {"eq": "active"}}
        ]}));
        let f = compile(&node).unwrap();
        assert_eq!(f.sql, "(kind = ? AND status = ?)");
        assert_eq!(f.params.len(), 2);
    }

    #[test]
    fn compiles_or() {
        let node = parse(json!({"or": [
            {"kind": {"eq": "spec"}},
            {"kind": {"eq": "plan"}}
        ]}));
        let f = compile(&node).unwrap();
        assert_eq!(f.sql, "(kind = ? OR kind = ?)");
    }

    #[test]
    fn compiles_not() {
        let node = parse(json!({"not": {"status": {"eq": "archived"}}}));
        let f = compile(&node).unwrap();
        assert_eq!(f.sql, "NOT (status = ?)");
    }

    #[test]
    fn compiles_tags_contains_via_json_each() {
        let node = parse(json!({"tags": {"contains": "embedding"}}));
        let f = compile(&node).unwrap();
        assert!(f.sql.contains("json_each(tags)"));
        assert_eq!(f.params.len(), 1);
    }

    /// `in` on an array column must mean "holds ANY of these", the same
    /// membership `contains` already gets — not a comparison against the
    /// column's raw JSON text.
    ///
    /// The defect this pins: `is_array_col` was consulted only on the
    /// `Contains` arm, so `in` fell through to `tags IN (?)`, comparing
    /// `["session-log","bug-fix"]` (the stored JSON *string*) against the
    /// scalar `"session-log"`. Never equal, so the result was a silent zero —
    /// and `get_guide("librarian")` § Filter Syntax teaches exactly this form.
    /// `docs/issues/archive/2026-08-28-tags-in-filter-returns-zero.md`,
    /// `open-issue-work-queue:BL-47`.
    ///
    /// End-to-end against a real `json()` column rather than a compile-shape
    /// assertion, because the shape assertion is what a wrong SQL string would
    /// still satisfy. Note the sibling `eval_matches_compile_on_fixture` cannot
    /// cover this: its fixture table has no array column at all, so it stays
    /// green whatever the array branch does.
    #[test]
    fn tags_in_matches_a_row_holding_any_listed_tag() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE a (id TEXT, tags TEXT)", [])
            .unwrap();
        for (id, tags) in [
            ("t1", r#"["session-log","bug-fix"]"#),
            ("t2", r#"["research"]"#),
            ("t3", "[]"),
        ] {
            conn.execute("INSERT INTO a (id, tags) VALUES (?1, ?2)", (id, tags))
                .unwrap();
        }

        let node = parse(json!({"tags": {"in": ["session-log", "research"]}}));
        let frag = compile(&node).unwrap();
        let sql = format!("SELECT id FROM a WHERE {} ORDER BY id", frag.sql);
        let mut stmt = conn.prepare(&sql).unwrap();
        let ids: Vec<String> = stmt
            .query_map(rusqlite::params_from_iter(frag.params.iter()), |r| {
                r.get::<_, String>(0)
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(
            ids,
            vec!["t1".to_string(), "t2".to_string()],
            "`in` on an array column must match rows holding ANY listed tag; \
             t3 holds none and must not match"
        );
    }

    /// `nin` is the exact complement, and the empty-tags row is the case worth
    /// pinning: holding none of the listed tags INCLUDES holding no tags at all.
    /// Asserted explicitly so it is a decision rather than whatever the SQL
    /// happened to do.
    #[test]
    fn tags_nin_excludes_only_rows_that_hold_a_listed_tag() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE a (id TEXT, tags TEXT)", [])
            .unwrap();
        for (id, tags) in [
            ("t1", r#"["session-log","bug-fix"]"#),
            ("t2", r#"["research"]"#),
            ("t3", "[]"),
        ] {
            conn.execute("INSERT INTO a (id, tags) VALUES (?1, ?2)", (id, tags))
                .unwrap();
        }

        let node = parse(json!({"tags": {"nin": ["session-log"]}}));
        let frag = compile(&node).unwrap();
        let sql = format!("SELECT id FROM a WHERE {} ORDER BY id", frag.sql);
        let mut stmt = conn.prepare(&sql).unwrap();
        let ids: Vec<String> = stmt
            .query_map(rusqlite::params_from_iter(frag.params.iter()), |r| {
                r.get::<_, String>(0)
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(
            ids,
            vec!["t2".to_string(), "t3".to_string()],
            "`nin` must exclude only rows holding a listed tag; the empty-tags \
             row holds none of them and therefore matches"
        );
    }

    /// Engine parity. `eval` is the in-memory twin used by `entry_filter`, and
    /// its `Contains` arm already does type-driven array membership
    /// (`Value::Array(items) => items.iter().any(...)`). `in`/`nin` must agree,
    /// or the same filter means two different things depending on which engine
    /// answers it.
    #[test]
    fn eval_in_on_an_array_field_means_membership_like_the_sql_side() {
        let mut entry = serde_json::Map::new();
        entry.insert("tags".into(), json!(["session-log", "bug-fix"]));

        let hit = parse(json!({"tags": {"in": ["research", "session-log"]}}));
        assert!(
            eval(&hit, &entry).unwrap(),
            "an array field holding a listed value must match `in`"
        );

        let miss = parse(json!({"tags": {"in": ["research"]}}));
        assert!(
            !eval(&miss, &entry).unwrap(),
            "an array field holding none of the listed values must not match"
        );

        let nin = parse(json!({"tags": {"nin": ["research"]}}));
        assert!(
            eval(&nin, &entry).unwrap(),
            "`nin` must be the complement of `in` on the same array field"
        );
    }

    #[test]
    fn compiles_gt_integer() {
        let node = parse(json!({"updated_at": {"gt": 1700000000}}));
        let f = compile(&node).unwrap();
        assert_eq!(f.sql, "updated_at > ?");
    }

    #[test]
    fn rejects_unknown_op() {
        let node = parse(json!({"kind": {"bogus": "x"}}));
        assert!(compile(&node).is_err());
    }

    #[test]
    fn rejects_unknown_field_prevents_injection() {
        let node = parse(json!({"1); DROP TABLE artifact; --": {"eq": "x"}}));
        match compile(&node) {
            Err(e) => assert!(
                e.to_string().contains("unknown field"),
                "unexpected error: {e}"
            ),
            Ok(_) => panic!("expected error for injected field name"),
        }
    }

    #[test]
    fn rejects_non_allowlisted_column() {
        let node = parse(json!({"sqlite_master": {"eq": "x"}}));
        assert!(compile(&node).is_err());
    }

    #[test]
    fn repair_fixes_inverted_op_keyed_leaf() {
        // {op: {field, value}} → {field: {op: value}}; repaired node compiles.
        let mut node = parse(json!({"contains": {"field": "title", "value": "or-tools"}}));
        let corrections = repair_inverted_leaves(&mut node);
        assert_eq!(
            corrections.len(),
            1,
            "one correction expected: {corrections:?}"
        );
        let f = compile(&node).unwrap();
        assert_eq!(f.sql, "title LIKE ? ESCAPE '\\'");
    }

    #[test]
    fn repair_leaves_canonical_leaf_untouched() {
        let mut node = parse(json!({"title": {"contains": "x"}}));
        assert!(repair_inverted_leaves(&mut node).is_empty());
        // Still the canonical shape.
        assert!(compile(&node).is_ok());
    }

    #[test]
    fn repair_recurses_into_composition() {
        let mut node = parse(json!({"or": [
            {"eq": {"field": "status", "value": "active"}},
            {"kind": {"eq": "tracker"}}
        ]}));
        let corrections = repair_inverted_leaves(&mut node);
        assert_eq!(corrections.len(), 1, "only the inverted child repaired");
        assert!(
            compile(&node).is_ok(),
            "whole composition compiles after repair"
        );
    }

    #[test]
    fn repair_ignores_op_keyed_leaf_without_field() {
        // {"contains": "x"} is ambiguous (no `field` key) — do not guess.
        let mut node = parse(json!({"contains": "x"}));
        assert!(repair_inverted_leaves(&mut node).is_empty());
    }

    #[test]
    fn rel_path_filter_compiles_to_abs_path_sql() {
        let node = parse(json!({"rel_path": {"contains": "docs/trackers"}}));
        let f = compile(&node).unwrap();
        // The ESCAPE clause is not decoration: without it `%` and `_` inside a
        // caller's value are live wildcards with no way to write either
        // literally. Pinned by
        // `contains_treats_percent_and_underscore_as_literal_characters`, which
        // asserts the resulting ROWS rather than this string.
        assert_eq!(f.sql, "abs_path LIKE ? ESCAPE '\\'");
        assert_eq!(
            f.params,
            vec![rusqlite::types::Value::Text("%docs/trackers%".into())]
        );
    }

    /// The catalog stores an ABSOLUTE path; every documented `rel_path` filter
    /// value is repo-relative. This test runs real queries and asserts on the
    /// ROWS returned, because the sibling test above asserts on the generated
    /// SQL string and neither live failure is expressible that way.
    ///
    /// Both directions are load-bearing and they fail oppositely: `prefix`/`eq`
    /// over-exclude to nothing, `ne`/`nin` under-exclude and return the rows
    /// they were asked to drop. A test carrying only the first half is monotone
    /// under a fix that repairs nothing about the second.
    #[test]
    fn rel_path_filter_matches_rows_whose_stored_path_is_absolute() {
        use rusqlite::Connection;

        // LOAD-BEARING: these paths are absolute, exactly as `artifact.abs_path`
        // stores them. Rewriting them as repo-relative deletes the coordinate
        // mismatch this test exists to catch — it would still pass, and would no
        // longer discriminate.
        let rows: &[(&str, &str)] = &[
            ("t1", "/home/u/repo/docs/trackers/issue-clusters.md"),
            ("t2", "/home/u/repo/docs/trackers/bug-fix-session-log.md"),
            ("b1", "/home/u/repo/docs/issues/2026-09-04-x.md"),
        ];

        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE e (id TEXT, abs_path TEXT)", [])
            .unwrap();
        for (id, abs_path) in rows {
            conn.execute(
                "INSERT INTO e (id, abs_path) VALUES (?1, ?2)",
                rusqlite::params![id, abs_path],
            )
            .unwrap();
        }

        let run = |fj: Value| -> Vec<String> {
            let frag = compile(&parse(fj)).unwrap();
            let sql = format!("SELECT id FROM e WHERE {} ORDER BY id", frag.sql);
            let mut stmt = conn.prepare(&sql).unwrap();
            stmt.query_map(rusqlite::params_from_iter(frag.params.iter()), |r| {
                r.get::<_, String>(0)
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
        };

        // Over-exclusion. This exact call is the worked example served from
        // `src/prompts/guides/librarian.md:77`.
        assert_eq!(
            run(json!({"rel_path": {"prefix": "docs/trackers"}})),
            vec!["t1", "t2"],
            "`prefix` on rel_path must reach the rows under that directory"
        );
        assert_eq!(
            run(json!({"rel_path": {"eq": "docs/issues/2026-09-04-x.md"}})),
            vec!["b1"],
            "`eq` on rel_path must name exactly one row"
        );
        assert_eq!(
            run(json!({"rel_path": {"in": ["docs/issues/2026-09-04-x.md"]}})),
            vec!["b1"],
            "`in` on rel_path is the list form of `eq`"
        );

        // Under-exclusion — the more dangerous half: the excluded row comes back.
        assert_eq!(
            run(json!({"rel_path": {"ne": "docs/trackers/issue-clusters.md"}})),
            vec!["b1", "t2"],
            "`ne` on rel_path must drop the row it names"
        );
        assert_eq!(
            run(json!({"rel_path": {"nin": ["docs/trackers/issue-clusters.md"]}})),
            vec!["b1", "t2"],
            "`nin` on rel_path is the list form of `ne`"
        );

        // Regression anchor: `contains` is the one op that was already correct,
        // and must stay correct. Without this line a fix could repair the others
        // by breaking the op every caller currently relies on.
        assert_eq!(
            run(json!({"rel_path": {"contains": "docs/trackers"}})),
            vec!["t1", "t2"],
            "`contains` was already correct and must remain so"
        );
    }

    /// The four ordering ops compare a repo-relative string against an absolute
    /// stored path lexicographically, which is not so much a wrong answer to the
    /// caller's question as an answer to a different one. Refuse rather than
    /// return it.
    #[test]
    fn rel_path_rejects_the_ordering_ops_it_cannot_mean() {
        for op in ["gt", "lt", "gte", "lte"] {
            let node = parse(json!({"rel_path": {op: "docs/trackers"}}));
            let err = match compile(&node) {
                Ok(_) => panic!("`{op}` on rel_path must be refused"),
                Err(e) => e.to_string(),
            };
            assert!(
                err.contains("rel_path"),
                "`{op}` refusal must name the field: {err}"
            );
        }
    }

    /// `contains` binds its value into a SQL `LIKE` pattern, so `%` and `_`
    /// inside it were live wildcards with no way to escape either — the sibling
    /// `prefix` arm called `escape_like_pattern` and this one did not.
    ///
    /// Asserts the ROWS, not parity with `eval`. Parity is the cheaper test and
    /// it is not sufficient: `eval` implements `contains` as a literal substring
    /// search, so a future change making IT wildcard-aware would restore parity
    /// while leaving both engines wrong. The expected id lists below are the
    /// claim; agreement is a consequence.
    #[test]
    fn contains_treats_percent_and_underscore_as_literal_characters() {
        use rusqlite::Connection;

        // LOAD-BEARING titles. `Docs%Frog` and `D_cs` match row "a" under a
        // WILDCARD reading and nothing under a LITERAL one — that is the whole
        // discrimination, and it needs the metacharacter to sit BETWEEN literal
        // text. A trailing one does not discriminate: `contains "50%"` compiles
        // to `LIKE '%50%%'`, which is equivalent to `LIKE '%50%'`, so it returns
        // the same row either way and cannot fail. Row "b" carries a real
        // percent sign so the positive control below has something to find.
        let rows: &[(&str, &str)] = &[
            ("a", "Docs Lotus Frog"),
            ("b", "50% off sale"),
            ("c", "hardware roadmap"),
        ];

        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE e (id TEXT, title TEXT)", [])
            .unwrap();
        for (id, title) in rows {
            conn.execute(
                "INSERT INTO e (id, title) VALUES (?1, ?2)",
                rusqlite::params![id, title],
            )
            .unwrap();
        }

        let run = |fj: Value| -> Vec<String> {
            let frag = compile(&parse(fj)).unwrap();
            let sql = format!("SELECT id FROM e WHERE {} ORDER BY id", frag.sql);
            let mut stmt = conn.prepare(&sql).unwrap();
            stmt.query_map(rusqlite::params_from_iter(frag.params.iter()), |r| {
                r.get::<_, String>(0)
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
        };
        let none: Vec<String> = vec![];

        // The wildcards must not fire. Both halves were observed RED
        // independently before the fix (each returned ["a"]) — the first
        // assertion short-circuits the second, so checking only one would have
        // claimed coverage of a metacharacter never exercised.
        assert_eq!(
            run(json!({"title": {"contains": "Docs%Frog"}})),
            none,
            "`%` inside a contains value must be a literal percent sign"
        );
        assert_eq!(
            run(json!({"title": {"contains": "D_cs"}})),
            none,
            "`_` inside a contains value must be a literal underscore"
        );

        // POSITIVE CONTROL. Without these, "escaped correctly" is
        // indistinguishable from "matches nothing at all" — the two assertions
        // above are absence assertions and so are monotone under a change that
        // breaks `contains` outright.
        assert_eq!(
            run(json!({"title": {"contains": "50%"}})),
            vec!["b"],
            "a literal percent sign must still be findable"
        );
        assert_eq!(
            run(json!({"title": {"contains": "50% off"}})),
            vec!["b"],
            "a literal percent sign mid-value must still be findable"
        );
        assert_eq!(
            run(json!({"title": {"contains": "Lotus"}})),
            vec!["a"],
            "ordinary substrings are unaffected"
        );
    }

    #[test]
    fn repo_filter_rejected() {
        let node = parse(json!({"repo": {"eq": "codescout"}}));
        assert!(compile(&node).is_err());
    }
    fn entry(json: Value) -> serde_json::Map<String, Value> {
        json.as_object().unwrap().clone()
    }

    #[test]
    fn eval_eq_and_missing_field() {
        let e = entry(json!({"status": "open", "priority": 2}));
        assert!(eval(&parse(json!({"status": {"eq": "open"}})), &e).unwrap());
        assert!(!eval(&parse(json!({"status": {"eq": "done"}})), &e).unwrap());
        assert!(!eval(&parse(json!({"owner": {"eq": "x"}})), &e).unwrap()); // missing field
    }

    #[test]
    fn eval_in_gt_contains_prefix() {
        let e = entry(json!({"cat": "hardware", "priority": 2, "tags": ["gpu", "thermal"]}));
        assert!(eval(&parse(json!({"cat": {"in": ["hardware", "software"]}})), &e).unwrap());
        assert!(eval(&parse(json!({"priority": {"gt": 1}})), &e).unwrap());
        assert!(!eval(&parse(json!({"priority": {"lt": 1}})), &e).unwrap());
        assert!(eval(&parse(json!({"cat": {"contains": "hard"}})), &e).unwrap()); // string substring
        assert!(eval(&parse(json!({"tags": {"contains": "gpu"}})), &e).unwrap()); // array membership
        assert!(eval(&parse(json!({"cat": {"prefix": "hard"}})), &e).unwrap());
    }

    #[test]
    fn eval_and_or_not() {
        let e = entry(json!({"cat": "hardware", "status": "open"}));
        assert!(eval(
            &parse(json!({"and": [{"cat": {"eq": "hardware"}}, {"status": {"eq": "open"}}]})),
            &e
        )
        .unwrap());
        assert!(eval(&parse(json!({"not": {"status": {"eq": "done"}}})), &e).unwrap());
        assert!(!eval(
            &parse(json!({"or": [{"cat": {"eq": "software"}}, {"status": {"eq": "done"}}]})),
            &e
        )
        .unwrap());
    }
    #[test]
    fn referenced_fields_collects_across_composites() {
        let node: FilterNode = serde_json::from_value(json!({
            "and": [
                {"status": {"eq": "open"}},
                {"or": [
                    {"severity": {"eq": "high"}},
                    {"not": {"owner": {"eq": "x"}}}
                ]}
            ]
        }))
        .unwrap();
        let fields = referenced_fields(&node);
        assert!(fields.contains("status"));
        assert!(fields.contains("severity"));
        assert!(fields.contains("owner"));
        assert_eq!(fields.len(), 3);
    }

    #[test]
    fn eval_contains_is_case_insensitive() {
        let e = entry(json!({"title": "Docs Lotus Frog"}));
        // mirrors SQLite LIKE '%v%' ASCII case-folding
        assert!(eval(&parse(json!({"title": {"contains": "frog"}})), &e).unwrap());
        assert!(eval(&parse(json!({"title": {"contains": "FROG"}})), &e).unwrap());
        assert!(eval(&parse(json!({"title": {"contains": "lotus"}})), &e).unwrap());
    }
    #[test]
    fn eval_matches_compile_on_fixture() {
        use rusqlite::Connection;

        // (id, status: Option<&str>, confidence: i64, title: &str)
        // status=None models a missing/NULL field. Only eq/ne/in/nin reference it
        // (those agree across engines on a missing field — both exclude it).
        let rows: &[(&str, Option<&str>, i64, &str)] = &[
            ("a", Some("open"), 1, "Docs Lotus Frog"),
            ("b", Some("done"), 3, "50% off sale"),
            ("c", Some("open"), 5, "hardware roadmap"),
            ("d", None, 2, "untriaged item"), // missing status
        ];

        // SQL side: temp table; None -> NULL status.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE e (id TEXT, status TEXT, confidence INTEGER, title TEXT)",
            [],
        )
        .unwrap();
        for (id, status, confidence, title) in rows {
            conn.execute(
                "INSERT INTO e (id, status, confidence, title) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![id, status, confidence, title],
            )
            .unwrap();
        }

        // eval side: JSON entries; omit "status" key for the None row.
        let entries: Vec<serde_json::Map<String, Value>> = rows
            .iter()
            .map(|(id, status, confidence, title)| {
                let mut m = serde_json::Map::new();
                m.insert("id".into(), json!(id));
                if let Some(s) = status {
                    m.insert("status".into(), json!(s));
                }
                m.insert("confidence".into(), json!(confidence));
                m.insert("title".into(), json!(title));
                m
            })
            .collect();

        let filters = [
            json!({"status": {"eq": "open"}}),
            json!({"status": {"ne": "open"}}), // missing-status row excluded by both
            json!({"confidence": {"gt": 2}}),
            json!({"confidence": {"gte": 3}}),
            json!({"confidence": {"lt": 5}}),
            json!({"and": [{"status": {"eq": "open"}}, {"confidence": {"lt": 5}}]}),
            json!({"not": {"title": {"eq": "50% off sale"}}}), // not over an ALWAYS-present field
            json!({"status": {"in": ["open", "flaky"]}}),
            json!({"status": {"nin": ["done"]}}),
            json!({"title": {"contains": "FROG"}}), // case-insensitivity parity
            json!({"title": {"contains": "lotus"}}),
            json!({"title": {"prefix": "50%"}}), // %-escape parity
            json!({"title": {"contains": "50%"}}),
            json!({"title": {"contains": "Docs%Frog"}}),
            json!({"title": {"contains": "D_cs"}}),
        ];

        // `rel_path` is deliberately absent, and adding it here would fail: it
        // is a catalog alias that `compile` anchors onto the absolute
        // `abs_path` column, while `eval` runs over augmentation params rows
        // where `rel_path` would be an ordinary user-defined field. The two
        // engines answer different questions for that one name, on purpose.

        for fj in filters {
            let node = parse(fj.clone());

            let mut eval_ids: Vec<String> = entries
                .iter()
                .filter(|e| eval(&node, e).unwrap())
                .map(|e| e["id"].as_str().unwrap().to_string())
                .collect();
            eval_ids.sort();

            let frag = compile(&node).unwrap();
            let sql = format!("SELECT id FROM e WHERE {} ORDER BY id", frag.sql);
            let mut stmt = conn.prepare(&sql).unwrap();
            let sql_ids: Vec<String> = stmt
                .query_map(rusqlite::params_from_iter(frag.params.iter()), |r| {
                    r.get::<_, String>(0)
                })
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();

            assert_eq!(eval_ids, sql_ids, "engine disagreement on filter {fj}");
        }
    }

    #[test]
    fn eval_and_compile_both_reject_empty_in() {
        let node = parse(json!({"status": {"in": []}}));
        assert!(compile(&node).is_err());
        let e = json!({"status": "open"}).as_object().unwrap().clone();
        assert!(eval(&node, &e).is_err());
    }
}
