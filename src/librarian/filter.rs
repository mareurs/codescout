use anyhow::Result;
use crate::tools::RecoverableError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

    let ops = ops
        .as_object()
        .ok_or_else(|| RecoverableError::with_hint("ops must be an object", "Leaf op shape is `{field: {op: value}}`, e.g. `{\"kind\": {\"eq\": \"tracker\"}}`."))?;
    if ops.len() != 1 {
        return Err(RecoverableError::with_hint(
            format!("exactly one op per leaf, got {}", ops.len()),
            "Wrap multiple ops on the same field with `and`/`or`, e.g. {\"and\":[{\"f\":{\"gt\":1}},{\"f\":{\"lt\":9}}]}.",
        )
        .into());
    }
    let (op_name, value) = ops.iter().next().unwrap();
    let op = op_name
        .parse::<LeafOp>()
        .map_err(|_| RecoverableError::with_hint(format!("unknown op `{op_name}`"), "valid ops: eq, ne, in, nin, gt, lt, gte, lte, contains, prefix"))?;

    let is_array_col = matches!(field.as_str(), "tags" | "owners");
    if op == LeafOp::Contains && is_array_col {
        let lit = json_value_to_sql(value)?;
        return Ok(SqlFragment {
            sql: format!("EXISTS (SELECT 1 FROM json_each({sql_field}) WHERE value = ?)"),
            params: vec![lit],
        });
    }

    match op {
        LeafOp::In | LeafOp::Nin => {
            let arr = value
                .as_array()
                .ok_or_else(|| RecoverableError::with_hint("`in` expects an array", "Provide a JSON array, e.g. `{\"in\": [\"a\", \"b\"]}`."))?;
            if arr.is_empty() {
                return Err(RecoverableError::with_hint(
                    "`in` expects a non-empty array",
                    "Provide at least one value, e.g. `{\"in\": [\"a\", \"b\"]}`.",
                )
                .into());
            }
            let placeholders = std::iter::repeat_n("?", arr.len())
                .collect::<Vec<_>>()
                .join(", ");
            let params = arr
                .iter()
                .map(json_value_to_sql)
                .collect::<Result<Vec<_>>>()?;
            Ok(SqlFragment {
                sql: format!("{sql_field} {} ({})", op.sql(), placeholders),
                params,
            })
        }
        LeafOp::Contains => {
            let s = value
                .as_str()
                .ok_or_else(|| RecoverableError::with_hint("`contains` expects a string", "Provide a string value, e.g. `{\"contains\": \"docs/trackers\"}`."))?;
            Ok(SqlFragment {
                sql: format!("{sql_field} LIKE ?"),
                params: vec![rusqlite::types::Value::Text(format!("%{s}%"))],
            })
        }
        LeafOp::Prefix => {
            let s = value
                .as_str()
                .ok_or_else(|| RecoverableError::with_hint("`prefix` expects a string", "Provide a string value, e.g. `{\"prefix\": \"docs/\"}`."))?;
            let escaped = s
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_");
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
    fn rel_path_filter_compiles_to_abs_path_sql() {
        let node = parse(json!({"rel_path": {"contains": "docs/trackers"}}));
        let f = compile(&node).unwrap();
        assert_eq!(f.sql, "abs_path LIKE ?");
        assert_eq!(
            f.params,
            vec![rusqlite::types::Value::Text("%docs/trackers%".into())]
        );
    }

    #[test]
    fn repo_filter_rejected() {
        let node = parse(json!({"repo": {"eq": "codescout"}}));
        assert!(compile(&node).is_err());
    }
}
