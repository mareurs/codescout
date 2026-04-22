//! `glob` tool and format helper.

use anyhow::Result;
use serde_json::{json, Value};

use super::{optional_u64_param, RecoverableError, Tool, ToolContext};

// ── glob ───────────────────────────────────────────────────────────────

pub struct Glob;

#[async_trait::async_trait]
impl Tool for Glob {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files matching a glob pattern (e.g. '**/*.rs', 'src/**/mod.rs'). Respects .gitignore."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["pattern"],
            "properties": {
                "pattern": { "type": "string", "description": "Glob pattern" },
                "path": { "type": "string", "description": "Directory to search (default: current dir)" },
                "limit": { "type": "integer", "default": 100, "description": "Maximum files to return" }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        let pattern = super::require_str_param(&input, "pattern")?;
        let raw_path = input["path"].as_str().unwrap_or(".");
        let project_root = ctx.agent.project_root().await;
        let security = ctx.agent.security_config().await;
        let search_path = crate::util::path_security::validate_read_path(
            raw_path,
            project_root.as_deref(),
            &security,
        )?;
        let max = optional_u64_param(&input, "limit").unwrap_or(100) as usize;

        let glob = globset::GlobBuilder::new(pattern)
            .literal_separator(false)
            .build()
            .map_err(|e| {
                RecoverableError::with_hint(
                    format!("invalid glob pattern: {e}"),
                    "Use glob syntax: * matches anything, ** crosses directories, ? matches one char",
                )
            })?
            .compile_matcher();

        let mut matches = vec![];
        let mut hit_cap = false;
        let walker = ignore::WalkBuilder::new(&search_path)
            .hidden(true)
            .git_ignore(true)
            .build();
        for entry in walker.flatten() {
            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(&search_path)
                .unwrap_or(entry.path());
            if glob.is_match(rel) {
                matches.push(entry.path().display().to_string());
                if matches.len() >= max {
                    hit_cap = true;
                    break;
                }
            }
        }

        let mut result = json!({ "files": matches, "total": matches.len() });
        if hit_cap {
            result["overflow"] = json!({
                "shown": matches.len(),
                "hint": format!(
                    "Showing first {} files (cap hit). Narrow with a more specific pattern or path=<dir>.",
                    matches.len()
                )
            });
        }
        Ok(result)
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_glob(result))
    }
}

// ── format_compact helpers ────────────────────────────────────────────────────

fn format_glob(result: &Value) -> String {
    let total = result["total"].as_u64().unwrap_or(0);
    let overflow = result["overflow"].is_object();
    let cap_note = if overflow {
        " (cap hit — narrow pattern)"
    } else {
        ""
    };
    format!("{total} files{cap_note}")
}
