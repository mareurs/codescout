//! Group flat tool results by `file` field. Used by `symbols(search)`,
//! `references`, and `grep` for both LLM-facing text rendering and (for refs
//! and grep) the JSON output shape.
//!
//! File-only grouping by design. If a future tool needs `group_by_kind` or
//! `group_by_directory`, introduce a new abstraction then; do not parameterize
//! this one. (See 2026-05-15-grouped-tool-output-design.md.)

use serde_json::Value;

/// A group of items sharing the same `file` field.
///
/// Holds borrowed references into the input slice — callers keep ownership.
pub struct FileGroup<'a> {
    pub file: &'a str,
    pub items: Vec<&'a Value>,
}

/// Group items by their `file` field. Sort: group size desc, ties by path asc.
/// Within a group, original order is preserved (stable partition).
///
/// Items lacking a `file` field are dropped silently; callers should not pass
/// such items to this function.
pub fn group_by_file(items: &[Value]) -> Vec<FileGroup<'_>> {
    use std::collections::BTreeMap;
    let mut by_file: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
    for item in items {
        if let Some(file) = item.get("file").and_then(|v| v.as_str()) {
            by_file.entry(file).or_default().push(item);
        }
    }
    let mut groups: Vec<FileGroup<'_>> = by_file
        .into_iter()
        .map(|(file, items)| FileGroup { file, items })
        .collect();
    // Stable: BTreeMap iteration is path-asc; sort_by with reverse on count
    // preserves alphabetical order among ties.
    groups.sort_by(|a, b| b.items.len().cmp(&a.items.len()));
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn item(file: &str) -> Value {
        json!({ "file": file })
    }

    #[test]
    fn groups_sorted_by_count_desc() {
        let items = vec![
            item("a.rs"),
            item("b.rs"),
            item("b.rs"),
            item("c.rs"),
            item("c.rs"),
            item("c.rs"),
        ];
        let groups = group_by_file(&items);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].file, "c.rs");
        assert_eq!(groups[0].items.len(), 3);
        assert_eq!(groups[1].file, "b.rs");
        assert_eq!(groups[1].items.len(), 2);
        assert_eq!(groups[2].file, "a.rs");
        assert_eq!(groups[2].items.len(), 1);
    }

    #[test]
    fn groups_tie_break_by_path_asc() {
        let items = vec![item("z.rs"), item("a.rs"), item("m.rs")];
        let groups = group_by_file(&items);
        assert_eq!(groups[0].file, "a.rs");
        assert_eq!(groups[1].file, "m.rs");
        assert_eq!(groups[2].file, "z.rs");
    }

    #[test]
    fn drops_items_without_file_field() {
        let items = vec![item("a.rs"), json!({ "no_file": true })];
        let groups = group_by_file(&items);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].file, "a.rs");
    }
}
