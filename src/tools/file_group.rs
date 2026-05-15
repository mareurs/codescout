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

/// Truncate a flat item list to fit `budget`, preserving file diversity.
///
/// Policy: Round-robin across files, prioritizing hotter (more frequent) files.
/// Each pass, offer one item to each file in count-desc order (most frequent first)
/// until budget is exhausted or all files are depleted.
///
/// Returns the visible items, plus the un-truncated `total` and `files` counts
/// (callers anchor "N hits in M files" headers to these).
pub fn cap_grouped(items: Vec<Value>, budget: usize) -> (Vec<Value>, usize, usize) {
    let total = items.len();
    if total == 0 {
        return (items, 0, 0);
    }

    use std::collections::BTreeMap;
    let mut by_file: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (idx, item) in items.iter().enumerate() {
        if let Some(file) = item.get("file").and_then(|v| v.as_str()) {
            by_file.entry(file.to_string()).or_default().push(idx);
        }
    }
    let files = by_file.len();

    if budget >= total {
        return (items, total, files);
    }

    let mut buckets: Vec<(String, Vec<usize>)> = by_file.into_iter().collect();
    buckets.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));

    let mut picked: Vec<usize> = Vec::with_capacity(budget);
    let mut cursors: Vec<usize> = vec![0; buckets.len()];

    // Round-robin: each pass, offer one item to each file (in count-desc order)
    // until budget exhausted or all files depleted.
    loop {
        let mut picked_any = false;
        for i in 0..buckets.len() {
            if picked.len() >= budget {
                break;
            }
            if cursors[i] < buckets[i].1.len() {
                picked.push(buckets[i].1[cursors[i]]);
                cursors[i] += 1;
                picked_any = true;
            }
        }
        if !picked_any || picked.len() >= budget {
            break;
        }
    }

    picked.sort();
    let picked_set: std::collections::HashSet<usize> = picked.into_iter().collect();
    let visible: Vec<Value> = items
        .into_iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            if picked_set.contains(&idx) {
                Some(item)
            } else {
                None
            }
        })
        .collect();

    (visible, total, files)
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

    #[test]
    fn cap_grouped_round_robin_first() {
        // 4 files with counts {3, 2, 1, 1}, budget=3 → top 3 files each get 1 hit.
        let items: Vec<Value> = ["a.rs", "a.rs", "a.rs", "b.rs", "b.rs", "c.rs", "d.rs"]
            .iter()
            .map(|f| item(f))
            .collect();
        let (visible, total, files) = cap_grouped(items, 3);
        assert_eq!(total, 7);
        assert_eq!(files, 4);
        assert_eq!(visible.len(), 3);
        let visible_files: Vec<&str> = visible
            .iter()
            .map(|v| v["file"].as_str().unwrap())
            .collect();
        assert_eq!(visible_files, vec!["a.rs", "b.rs", "c.rs"]);
    }

    #[test]
    fn cap_grouped_fills_hot_after_breadth() {
        // {6, 3, 1}, budget=8 → round-robin (hot-first) picks: a, b, c, a, b, a, b, a
        let mut items: Vec<Value> = vec![];
        for _ in 0..6 {
            items.push(item("a.rs"));
        }
        for _ in 0..3 {
            items.push(item("b.rs"));
        }
        items.push(item("c.rs"));
        let (visible, total, files) = cap_grouped(items, 8);
        assert_eq!(total, 10);
        assert_eq!(files, 3);
        assert_eq!(visible.len(), 8);
        let counts: std::collections::HashMap<&str, usize> =
            visible
                .iter()
                .fold(std::collections::HashMap::new(), |mut acc, v| {
                    let f = v["file"].as_str().unwrap();
                    *acc.entry(f).or_insert(0) += 1;
                    acc
                });
        assert_eq!(counts["a.rs"], 4);
        assert_eq!(counts["b.rs"], 3);
        assert_eq!(counts["c.rs"], 1);
    }

    #[test]
    fn cap_grouped_budget_exceeds_total() {
        let items: Vec<Value> = vec![item("a.rs"), item("b.rs")];
        let (visible, total, files) = cap_grouped(items, 100);
        assert_eq!(visible.len(), 2);
        assert_eq!(total, 2);
        assert_eq!(files, 2);
    }

    #[test]
    fn cap_grouped_zero_budget() {
        let items: Vec<Value> = vec![item("a.rs"), item("b.rs")];
        let (visible, total, files) = cap_grouped(items, 0);
        assert_eq!(visible.len(), 0);
        assert_eq!(total, 2);
        assert_eq!(files, 2);
    }
}
