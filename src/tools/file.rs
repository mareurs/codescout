//! File system tools: read, write, search, list.

use anyhow::Result;
use serde_json::{json, Value};

use super::{Tool, ToolContext};
use crate::util::text::extract_lines;

// ── read_file ────────────────────────────────────────────────────────────────

pub struct ReadFile;

#[async_trait::async_trait]
impl Tool for ReadFile {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file. Optionally restrict to a line range."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": { "type": "string", "description": "File path relative to project root" },
                "start_line": { "type": "integer", "description": "First line to return (1-indexed)" },
                "end_line": { "type": "integer", "description": "Last line to return (1-indexed, inclusive)" }
            }
        })
    }

    async fn call(&self, input: Value, _ctx: &ToolContext) -> Result<Value> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing path"))?;
        let text = std::fs::read_to_string(path)?;
        let content = match (input["start_line"].as_u64(), input["end_line"].as_u64()) {
            (Some(start), Some(end)) => extract_lines(&text, start as usize, end as usize),
            _ => text,
        };
        Ok(json!({ "content": content }))
    }
}

// ── list_dir ────────────────────────────────────────────────────────────────

pub struct ListDir;

#[async_trait::async_trait]
impl Tool for ListDir {
    fn name(&self) -> &str {
        "list_dir"
    }

    fn description(&self) -> &str {
        "List files and directories. Pass recursive=true for a full tree."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": { "type": "string" },
                "recursive": { "type": "boolean", "default": false }
            }
        })
    }

    async fn call(&self, input: Value, _ctx: &ToolContext) -> Result<Value> {
        let path = input["path"].as_str().unwrap_or(".");
        let recursive = input["recursive"].as_bool().unwrap_or(false);
        let depth = if recursive { usize::MAX } else { 1 };

        let entries: Vec<String> = walkdir::WalkDir::new(path)
            .max_depth(depth)
            .into_iter()
            .flatten()
            .filter(|e| e.depth() > 0)
            .map(|e| {
                let suffix = if e.file_type().is_dir() { "/" } else { "" };
                format!("{}{}", e.path().display(), suffix)
            })
            .collect();

        Ok(json!({ "entries": entries }))
    }
}

// ── search_for_pattern ───────────────────────────────────────────────────────

pub struct SearchForPattern;

#[async_trait::async_trait]
impl Tool for SearchForPattern {
    fn name(&self) -> &str {
        "search_for_pattern"
    }

    fn description(&self) -> &str {
        "Search the codebase for a regex pattern. Returns matching lines with file and line number."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["pattern"],
            "properties": {
                "pattern": { "type": "string", "description": "Regex pattern" },
                "path": { "type": "string", "description": "Directory to search (default: project root)" },
                "max_results": { "type": "integer", "default": 50 }
            }
        })
    }

    async fn call(&self, input: Value, _ctx: &ToolContext) -> Result<Value> {
        let pattern = input["pattern"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing pattern"))?;
        let search_path = input["path"].as_str().unwrap_or(".");
        let max = input["max_results"].as_u64().unwrap_or(50) as usize;

        let re = regex::Regex::new(pattern)?;
        let mut matches = vec![];

        let walker = ignore::WalkBuilder::new(search_path).build();
        'outer: for entry in walker.flatten() {
            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            for (i, line) in text.lines().enumerate() {
                if re.is_match(line) {
                    matches.push(json!({
                        "file": entry.path().display().to_string(),
                        "line": i + 1,
                        "content": line
                    }));
                    if matches.len() >= max {
                        break 'outer;
                    }
                }
            }
        }

        Ok(json!({ "matches": matches, "total": matches.len() }))
    }
}

// ── create_text_file ────────────────────────────────────────────────────────

pub struct CreateTextFile;

#[async_trait::async_trait]
impl Tool for CreateTextFile {
    fn name(&self) -> &str {
        "create_text_file"
    }

    fn description(&self) -> &str {
        "Create or overwrite a file with the given content. Creates parent directories as needed."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path", "content"],
            "properties": {
                "path": { "type": "string", "description": "File path (relative or absolute)" },
                "content": { "type": "string", "description": "Content to write" }
            }
        })
    }

    async fn call(&self, input: Value, _ctx: &ToolContext) -> Result<Value> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'path' parameter"))?;
        let content = input["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'content' parameter"))?;
        crate::util::fs::write_utf8(std::path::Path::new(path), content)?;
        Ok(json!({ "status": "ok", "path": path, "bytes": content.len() }))
    }
}

// ── find_file ───────────────────────────────────────────────────────────────

pub struct FindFile;

#[async_trait::async_trait]
impl Tool for FindFile {
    fn name(&self) -> &str {
        "find_file"
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
                "max_results": { "type": "integer", "default": 100 }
            }
        })
    }

    async fn call(&self, input: Value, _ctx: &ToolContext) -> Result<Value> {
        let pattern = input["pattern"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'pattern' parameter"))?;
        let search_path = input["path"].as_str().unwrap_or(".");
        let max = input["max_results"].as_u64().unwrap_or(100) as usize;

        let glob = globset::GlobBuilder::new(pattern)
            .literal_separator(false)
            .build()?
            .compile_matcher();

        let mut matches = vec![];
        let walker = ignore::WalkBuilder::new(search_path).build();
        for entry in walker.flatten() {
            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(search_path)
                .unwrap_or(entry.path());
            if glob.is_match(rel) {
                matches.push(entry.path().display().to_string());
                if matches.len() >= max {
                    break;
                }
            }
        }

        Ok(json!({ "files": matches, "total": matches.len() }))
    }
}

// ── replace_content ─────────────────────────────────────────────────────────

pub struct ReplaceContent;

#[async_trait::async_trait]
impl Tool for ReplaceContent {
    fn name(&self) -> &str {
        "replace_content"
    }

    fn description(&self) -> &str {
        "Find and replace text in a file. Supports regex or literal matching."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path", "old", "new"],
            "properties": {
                "path": { "type": "string", "description": "File path" },
                "old": { "type": "string", "description": "Text or regex pattern to find" },
                "new": { "type": "string", "description": "Replacement text" },
                "is_regex": { "type": "boolean", "default": false },
                "replace_all": { "type": "boolean", "default": true }
            }
        })
    }

    async fn call(&self, input: Value, _ctx: &ToolContext) -> Result<Value> {
        let path = input["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'path' parameter"))?;
        let old = input["old"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'old' parameter"))?;
        let new_text = input["new"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing 'new' parameter"))?;
        let is_regex = input["is_regex"].as_bool().unwrap_or(false);
        let replace_all = input["replace_all"].as_bool().unwrap_or(true);

        let content = std::fs::read_to_string(path)?;

        let (replaced, count) = if is_regex {
            let re = regex::Regex::new(old)?;
            if replace_all {
                let result = re.replace_all(&content, new_text);
                let c = re.find_iter(&content).count();
                (result.into_owned(), c)
            } else {
                let c = if re.is_match(&content) { 1 } else { 0 };
                (re.replace(&content, new_text).into_owned(), c)
            }
        } else if replace_all {
            let c = content.matches(old).count();
            (content.replace(old, new_text), c)
        } else {
            let c = if content.contains(old) { 1 } else { 0 };
            (content.replacen(old, new_text, 1), c)
        };

        std::fs::write(path, &replaced)?;
        Ok(json!({ "status": "ok", "replacements": count, "path": path }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use crate::lsp::LspManager;
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::tempdir;

    async fn test_ctx() -> ToolContext {
        ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: Arc::new(LspManager::new()),
        }
    }

    // ── ReadFile ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn read_file_returns_full_content() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("hello.txt");
        std::fs::write(&file, "hello world").unwrap();

        let result = ReadFile
            .call(json!({ "path": file.to_str().unwrap() }), &ctx)
            .await
            .unwrap();
        assert_eq!(result["content"], "hello world");
    }

    #[tokio::test]
    async fn read_file_with_line_range() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("lines.txt");
        std::fs::write(&file, "line1\nline2\nline3\nline4\nline5").unwrap();

        let result = ReadFile
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "start_line": 2,
                    "end_line": 4
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["content"], "line2\nline3\nline4");
    }

    #[tokio::test]
    async fn read_file_missing_errors() {
        let ctx = test_ctx().await;
        let result = ReadFile
            .call(json!({ "path": "/no/such/file.txt" }), &ctx)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn read_file_missing_path_param_errors() {
        let ctx = test_ctx().await;
        let result = ReadFile.call(json!({}), &ctx).await;
        assert!(result.is_err());
    }

    // ── ListDir ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn list_dir_returns_shallow_entries() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "").unwrap();
        std::fs::write(dir.path().join("b.rs"), "").unwrap();

        let result = ListDir
            .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        let entries: Vec<&str> = result["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|e| e.ends_with("a.rs")));
        assert!(entries.iter().any(|e| e.ends_with("b.rs")));
    }

    #[tokio::test]
    async fn list_dir_shallow_does_not_descend() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("deep.rs"), "").unwrap();

        let result = ListDir
            .call(
                json!({ "path": dir.path().to_str().unwrap(), "recursive": false }),
                &ctx,
            )
            .await
            .unwrap();

        let entries: Vec<&str> = result["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        // Should see the sub/ directory but not deep.rs inside it
        assert!(!entries.iter().any(|e| e.ends_with("deep.rs")));
    }

    #[tokio::test]
    async fn list_dir_recursive_descends() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("deep.rs"), "").unwrap();

        let result = ListDir
            .call(
                json!({ "path": dir.path().to_str().unwrap(), "recursive": true }),
                &ctx,
            )
            .await
            .unwrap();

        let entries: Vec<&str> = result["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(entries.iter().any(|e| e.ends_with("deep.rs")));
    }

    // ── SearchForPattern ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn search_finds_matching_line() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("code.rs"), "fn main() {}\nlet x = 42;\n").unwrap();

        let result = SearchForPattern
            .call(
                json!({ "pattern": "fn main", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();

        let matches = result["matches"].as_array().unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0]["line"], 1);
        assert!(matches[0]["content"].as_str().unwrap().contains("fn main"));
    }

    #[tokio::test]
    async fn search_returns_no_matches_when_absent() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("code.rs"), "fn main() {}").unwrap();

        let result = SearchForPattern
            .call(
                json!({ "pattern": "xyz_not_present", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["matches"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn search_respects_max_results() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let content = (0..20)
            .map(|i| format!("match_{}", i))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(dir.path().join("data.txt"), &content).unwrap();

        let result = SearchForPattern
            .call(
                json!({
                    "pattern": "match_",
                    "path": dir.path().to_str().unwrap(),
                    "max_results": 5
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["matches"].as_array().unwrap().len(), 5);
    }

    #[tokio::test]
    async fn search_invalid_regex_errors() {
        let ctx = test_ctx().await;
        let result = SearchForPattern
            .call(json!({ "pattern": "[invalid" }), &ctx)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn search_missing_pattern_errors() {
        let ctx = test_ctx().await;
        let result = SearchForPattern.call(json!({}), &ctx).await;
        assert!(result.is_err());
    }

    // ── CreateTextFile ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn create_text_file_writes_content() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("new.txt");

        let result = CreateTextFile
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "content": "hello file"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["status"], "ok");
        assert_eq!(result["bytes"], 10);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "hello file");
    }

    #[tokio::test]
    async fn create_text_file_creates_parent_dirs() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("a").join("b").join("deep.txt");

        CreateTextFile
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "content": "nested"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(std::fs::read_to_string(&file).unwrap(), "nested");
    }

    #[tokio::test]
    async fn create_text_file_missing_params_errors() {
        let ctx = test_ctx().await;
        assert!(CreateTextFile.call(json!({}), &ctx).await.is_err());
        assert!(CreateTextFile
            .call(json!({ "path": "/tmp/x" }), &ctx)
            .await
            .is_err());
    }

    // ── FindFile ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn find_file_matches_glob() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("foo.rs"), "").unwrap();
        std::fs::write(dir.path().join("bar.rs"), "").unwrap();
        std::fs::write(dir.path().join("baz.txt"), "").unwrap();

        let result = FindFile
            .call(
                json!({
                    "pattern": "*.rs",
                    "path": dir.path().to_str().unwrap()
                }),
                &ctx,
            )
            .await
            .unwrap();

        let files = result["files"].as_array().unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| f.as_str().unwrap().ends_with(".rs")));
    }

    #[tokio::test]
    async fn find_file_recursive_glob() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let sub = dir.path().join("src");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("lib.rs"), "").unwrap();
        std::fs::write(dir.path().join("main.rs"), "").unwrap();

        let result = FindFile
            .call(
                json!({
                    "pattern": "**/*.rs",
                    "path": dir.path().to_str().unwrap()
                }),
                &ctx,
            )
            .await
            .unwrap();

        let files = result["files"].as_array().unwrap();
        assert_eq!(files.len(), 2);
    }

    #[tokio::test]
    async fn find_file_respects_max_results() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        for i in 0..10 {
            std::fs::write(dir.path().join(format!("f{}.rs", i)), "").unwrap();
        }

        let result = FindFile
            .call(
                json!({
                    "pattern": "*.rs",
                    "path": dir.path().to_str().unwrap(),
                    "max_results": 3
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["files"].as_array().unwrap().len(), 3);
        assert_eq!(result["total"], 3);
    }

    #[tokio::test]
    async fn find_file_no_matches() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("readme.md"), "").unwrap();

        let result = FindFile
            .call(
                json!({
                    "pattern": "*.rs",
                    "path": dir.path().to_str().unwrap()
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["files"].as_array().unwrap().len(), 0);
    }

    // ── ReplaceContent ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn replace_content_literal() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("code.rs");
        std::fs::write(&file, "let x = 1;\nlet y = 2;\nlet x = 3;\n").unwrap();

        let result = ReplaceContent
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "old": "let x",
                    "new": "let z",
                    "replace_all": true
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["replacements"], 2);
        let content = std::fs::read_to_string(&file).unwrap();
        assert_eq!(content, "let z = 1;\nlet y = 2;\nlet z = 3;\n");
    }

    #[tokio::test]
    async fn replace_content_literal_first_only() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("code.rs");
        std::fs::write(&file, "aaa bbb aaa").unwrap();

        let result = ReplaceContent
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "old": "aaa",
                    "new": "ccc",
                    "replace_all": false
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["replacements"], 1);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "ccc bbb aaa");
    }

    #[tokio::test]
    async fn replace_content_regex() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("data.txt");
        std::fs::write(&file, "foo123bar456baz").unwrap();

        let result = ReplaceContent
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "old": r"\d+",
                    "new": "NUM",
                    "is_regex": true,
                    "replace_all": true
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["replacements"], 2);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "fooNUMbarNUMbaz");
    }

    #[tokio::test]
    async fn replace_content_regex_first_only() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("data.txt");
        std::fs::write(&file, "aaa111bbb222").unwrap();

        let result = ReplaceContent
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "old": r"\d+",
                    "new": "X",
                    "is_regex": true,
                    "replace_all": false
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["replacements"], 1);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "aaaXbbb222");
    }

    #[tokio::test]
    async fn replace_content_no_match() {
        let ctx = test_ctx().await;
        let dir = tempdir().unwrap();
        let file = dir.path().join("data.txt");
        std::fs::write(&file, "hello world").unwrap();

        let result = ReplaceContent
            .call(
                json!({
                    "path": file.to_str().unwrap(),
                    "old": "xyz",
                    "new": "abc"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result["replacements"], 0);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "hello world");
    }

    #[tokio::test]
    async fn replace_content_missing_params_errors() {
        let ctx = test_ctx().await;
        assert!(ReplaceContent.call(json!({}), &ctx).await.is_err());
        assert!(ReplaceContent
            .call(json!({ "path": "/tmp/x", "old": "a" }), &ctx)
            .await
            .is_err());
    }
}
