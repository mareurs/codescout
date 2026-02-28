//! OutputGuard — progressive disclosure for tool output.
//!
//! Tools that produce potentially unbounded output (file lists, search results,
//! symbol trees) use `OutputGuard` to cap and paginate results so the LLM
//! receives a manageable amount of context.

use serde_json::{json, Value};

/// Controls how much detail a tool should return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// Compact output, capped at `max_results` / `max_files`.
    Exploring,
    /// Full detail with offset/limit pagination.
    Focused,
}

/// Metadata about items that were omitted from the response.
#[derive(Debug, Clone)]
pub struct OverflowInfo {
    pub shown: usize,
    pub total: usize,
    pub hint: String,
    /// In focused mode, the offset for the next page (None in exploring mode).
    pub next_offset: Option<usize>,
    /// Per-file result counts, sorted by count descending. Only for multi-file searches.
    /// Capped at 15 entries — see `by_file_overflow` for how many were omitted.
    pub by_file: Option<Vec<(String, usize)>>,
    /// Number of additional files omitted from `by_file` due to the 15-entry cap.
    pub by_file_overflow: usize,
}

/// Guards tool output size by capping or paginating results.
#[derive(Debug, Clone)]
pub struct OutputGuard {
    pub mode: OutputMode,
    pub max_files: usize,
    pub max_results: usize,
    pub offset: usize,
    pub limit: usize,
}

impl Default for OutputGuard {
    fn default() -> Self {
        Self {
            mode: OutputMode::Exploring,
            max_files: 200,
            max_results: 200,
            offset: 0,
            limit: 50,
        }
    }
}

/// Paginate a vec with offset/limit, returning overflow metadata when items are omitted.
fn paginate<T>(
    items: Vec<T>,
    offset: usize,
    limit: usize,
    hint: &str,
) -> (Vec<T>, Option<OverflowInfo>) {
    let total = items.len();
    let start = offset.min(total);
    let end = (start + limit).min(total);
    let page: Vec<T> = items.into_iter().skip(start).take(end - start).collect();
    let overflow = if page.len() < total {
        let next = if end < total { Some(end) } else { None };
        Some(OverflowInfo {
            shown: page.len(),
            total,
            hint: hint.to_string(),
            next_offset: next,
            by_file: None,
            by_file_overflow: 0,
        })
    } else {
        None
    };
    (page, overflow)
}

impl OutputGuard {
    /// Build an `OutputGuard` from a tool's JSON input.
    ///
    /// Reads optional fields:
    /// - `detail_level`: `"full"` → [`OutputMode::Focused`], anything else → [`OutputMode::Exploring`]
    /// - `offset`: pagination start (default 0)
    /// - `limit`: page size (default 50). When explicitly provided, also caps exploring mode.
    pub fn from_input(input: &Value) -> Self {
        let mode = match input["detail_level"].as_str() {
            Some("full") => OutputMode::Focused,
            _ => OutputMode::Exploring,
        };
        let offset = input["offset"].as_u64().unwrap_or(0) as usize;
        let limit = input["limit"].as_u64().unwrap_or(50) as usize;

        // If the caller explicitly specifies a limit, honour it in exploring mode too.
        let (max_files, max_results) = if input["limit"].is_number() {
            (limit, limit)
        } else {
            (200, 200)
        };

        Self {
            mode,
            offset,
            limit,
            max_files,
            max_results,
        }
    }

    /// Whether the tool should include full bodies (source code, etc.).
    pub fn should_include_body(&self) -> bool {
        self.mode == OutputMode::Focused
    }

    /// Cap a list of items according to the current mode.
    ///
    /// - **Exploring**: keeps the first `max_results` items.
    /// - **Focused**: applies `offset`/`limit` pagination.
    ///
    /// Returns the (possibly truncated) vec and optional overflow metadata.
    pub fn cap_items<T>(&self, items: Vec<T>, hint: &str) -> (Vec<T>, Option<OverflowInfo>) {
        let total = items.len();
        match self.mode {
            OutputMode::Exploring => {
                if total <= self.max_results {
                    (items, None)
                } else {
                    let kept: Vec<T> = items.into_iter().take(self.max_results).collect();
                    let overflow = OverflowInfo {
                        shown: self.max_results,
                        total,
                        hint: hint.to_string(),
                        next_offset: None,
                        by_file: None,
                        by_file_overflow: 0,
                    };
                    (kept, Some(overflow))
                }
            }
            OutputMode::Focused => paginate(items, self.offset, self.limit, hint),
        }
    }

    /// Cap a list of files according to the current mode.
    ///
    /// - **Exploring**: keeps the first `max_files` entries.
    /// - **Focused**: applies `offset`/`limit` pagination.
    pub fn cap_files<T>(&self, files: Vec<T>, hint: &str) -> (Vec<T>, Option<OverflowInfo>) {
        let total = files.len();
        match self.mode {
            OutputMode::Exploring => {
                if total <= self.max_files {
                    (files, None)
                } else {
                    let kept: Vec<T> = files.into_iter().take(self.max_files).collect();
                    let overflow = OverflowInfo {
                        shown: self.max_files,
                        total,
                        hint: hint.to_string(),
                        next_offset: None,
                        by_file: None,
                        by_file_overflow: 0,
                    };
                    (kept, Some(overflow))
                }
            }
            OutputMode::Focused => paginate(files, self.offset, self.limit, hint),
        }
    }

    /// Serialize overflow metadata to JSON for inclusion in tool responses.
    pub fn overflow_json(info: &OverflowInfo) -> Value {
        let mut obj = json!({
            "shown": info.shown,
            "total": info.total,
            "hint": info.hint
        });
        if let Some(next) = info.next_offset {
            obj["next_offset"] = json!(next);
        }
        if let Some(by_file) = &info.by_file {
            // Array format preserves count-descending sort order (BTreeMap would alphabetize).
            obj["by_file"] = json!(by_file
                .iter()
                .map(|(f, c)| json!({"file": f, "count": c}))
                .collect::<Vec<_>>());
            if info.by_file_overflow > 0 {
                obj["by_file_overflow"] = json!(info.by_file_overflow);
            }
        }
        obj
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_exploring() {
        let guard = OutputGuard::default();
        assert_eq!(guard.mode, OutputMode::Exploring);
        assert_eq!(guard.max_files, 200);
        assert_eq!(guard.max_results, 200);
    }

    #[test]
    fn from_input_default_exploring() {
        let guard = OutputGuard::from_input(&json!({}));
        assert_eq!(guard.mode, OutputMode::Exploring);
    }

    #[test]
    fn from_input_focused_mode() {
        let guard = OutputGuard::from_input(&json!({ "detail_level": "full" }));
        assert_eq!(guard.mode, OutputMode::Focused);
    }

    #[test]
    fn from_input_parses_offset_limit() {
        let guard = OutputGuard::from_input(&json!({
            "detail_level": "full",
            "offset": 10,
            "limit": 25
        }));
        assert_eq!(guard.offset, 10);
        assert_eq!(guard.limit, 25);
    }

    #[test]
    fn should_include_body_by_mode() {
        let exploring = OutputGuard::default();
        assert!(!exploring.should_include_body());

        let focused = OutputGuard::from_input(&json!({ "detail_level": "full" }));
        assert!(focused.should_include_body());
    }

    #[test]
    fn cap_items_exploring_under_limit() {
        let guard = OutputGuard {
            max_results: 5,
            ..OutputGuard::default()
        };
        let items: Vec<i32> = vec![1, 2, 3];
        let (result, overflow) = guard.cap_items(items, "use offset/limit");
        assert_eq!(result, vec![1, 2, 3]);
        assert!(overflow.is_none());
    }

    #[test]
    fn cap_items_exploring_over_limit() {
        let guard = OutputGuard {
            max_results: 3,
            ..OutputGuard::default()
        };
        let items: Vec<i32> = vec![1, 2, 3, 4, 5];
        let (result, overflow) = guard.cap_items(items, "use offset/limit to paginate");
        assert_eq!(result, vec![1, 2, 3]);
        let info = overflow.unwrap();
        assert_eq!(info.shown, 3);
        assert_eq!(info.total, 5);
        assert_eq!(info.hint, "use offset/limit to paginate");
    }

    #[test]
    fn cap_items_focused_pagination() {
        let guard = OutputGuard {
            mode: OutputMode::Focused,
            offset: 3,
            limit: 4,
            ..OutputGuard::default()
        };
        let items: Vec<i32> = (0..10).collect();
        let (result, overflow) = guard.cap_items(items, "next page");
        assert_eq!(result, vec![3, 4, 5, 6]);
        let info = overflow.unwrap();
        assert_eq!(info.shown, 4);
        assert_eq!(info.total, 10);
    }

    #[test]
    fn cap_items_focused_last_page_no_overflow() {
        let guard = OutputGuard {
            mode: OutputMode::Focused,
            offset: 0,
            limit: 50,
            ..OutputGuard::default()
        };
        let items: Vec<i32> = vec![1, 2, 3];
        let (result, overflow) = guard.cap_items(items, "hint");
        assert_eq!(result, vec![1, 2, 3]);
        assert!(overflow.is_none());
    }

    #[test]
    fn cap_files_exploring() {
        let guard = OutputGuard {
            max_files: 2,
            ..OutputGuard::default()
        };
        let files = vec!["a.rs", "b.rs", "c.rs", "d.rs"];
        let (result, overflow) = guard.cap_files(files, "use find_file with offset");
        assert_eq!(result, vec!["a.rs", "b.rs"]);
        let info = overflow.unwrap();
        assert_eq!(info.shown, 2);
        assert_eq!(info.total, 4);
    }

    #[test]
    fn cap_files_focused_pagination() {
        let guard = OutputGuard {
            mode: OutputMode::Focused,
            offset: 1,
            limit: 2,
            ..OutputGuard::default()
        };
        let files = vec!["a.rs", "b.rs", "c.rs", "d.rs"];
        let (result, overflow) = guard.cap_files(files, "next page");
        assert_eq!(result, vec!["b.rs", "c.rs"]);
        let info = overflow.unwrap();
        assert_eq!(info.shown, 2);
        assert_eq!(info.total, 4);
    }

    #[test]
    fn next_offset_in_focused_mode() {
        // Mid-stream: next_offset = end of current page
        let guard = OutputGuard {
            mode: OutputMode::Focused,
            offset: 0,
            limit: 3,
            ..OutputGuard::default()
        };
        let items: Vec<i32> = (0..10).collect();
        let (result, overflow) = guard.cap_items(items, "next page");
        assert_eq!(result, vec![0, 1, 2]);
        let info = overflow.unwrap();
        assert_eq!(info.next_offset, Some(3));

        // Last page: next_offset = None
        let guard = OutputGuard {
            mode: OutputMode::Focused,
            offset: 8,
            limit: 5,
            ..OutputGuard::default()
        };
        let items: Vec<i32> = (0..10).collect();
        let (_, overflow) = guard.cap_items(items, "next page");
        let info = overflow.unwrap();
        assert_eq!(info.next_offset, None);
    }

    #[test]
    fn next_offset_none_in_exploring_mode() {
        let guard = OutputGuard {
            max_results: 3,
            ..OutputGuard::default()
        };
        let items: Vec<i32> = vec![1, 2, 3, 4, 5];
        let (_, overflow) = guard.cap_items(items, "hint");
        let info = overflow.unwrap();
        assert_eq!(info.next_offset, None);
    }

    #[test]
    fn overflow_json_format() {
        let info = OverflowInfo {
            shown: 50,
            total: 1234,
            hint: "pass offset=50 for next page".to_string(),
            next_offset: Some(50),
            by_file: None,
            by_file_overflow: 0,
        };
        let j = OutputGuard::overflow_json(&info);
        assert_eq!(j["shown"], 50);
        assert_eq!(j["total"], 1234);
        assert_eq!(j["hint"], "pass offset=50 for next page");
        assert_eq!(j["next_offset"], 50);

        // Without next_offset, key is absent
        let info_no_next = OverflowInfo {
            shown: 200,
            total: 500,
            hint: "narrow query".to_string(),
            next_offset: None,
            by_file: None,
            by_file_overflow: 0,
        };
        let j2 = OutputGuard::overflow_json(&info_no_next);
        assert!(j2.get("next_offset").is_none());
    }

    #[test]
    fn overflow_json_includes_by_file() {
        let info = OverflowInfo {
            shown: 50,
            total: 90,
            hint: "narrow".to_string(),
            next_offset: None,
            by_file: Some(vec![
                ("src/a.rs".to_string(), 30),
                ("src/b.rs".to_string(), 20),
            ]),
            by_file_overflow: 0,
        };
        let json = OutputGuard::overflow_json(&info);
        // Array format preserves count-descending order
        let by_file = json["by_file"]
            .as_array()
            .expect("by_file should be an array");
        assert_eq!(by_file.len(), 2);
        assert_eq!(by_file[0]["file"], "src/a.rs");
        assert_eq!(by_file[0]["count"], 30);
        assert_eq!(by_file[1]["file"], "src/b.rs");
        assert_eq!(by_file[1]["count"], 20);
        assert!(
            json.get("by_file_overflow").is_none(),
            "zero overflow should be omitted"
        );
    }

    #[test]
    fn overflow_json_includes_by_file_overflow_when_nonzero() {
        let info = OverflowInfo {
            shown: 50,
            total: 200,
            hint: "narrow".to_string(),
            next_offset: None,
            by_file: Some(vec![("src/a.rs".to_string(), 10)]),
            by_file_overflow: 42,
        };
        let json = OutputGuard::overflow_json(&info);
        assert_eq!(json["by_file_overflow"], 42);
    }

    #[test]
    fn overflow_json_omits_by_file_when_none() {
        let info = OverflowInfo {
            shown: 10,
            total: 20,
            hint: "hint".to_string(),
            next_offset: None,
            by_file: None,
            by_file_overflow: 0,
        };
        let json = OutputGuard::overflow_json(&info);
        assert!(json.get("by_file").is_none());
        assert!(json.get("by_file_overflow").is_none());
    }
}
