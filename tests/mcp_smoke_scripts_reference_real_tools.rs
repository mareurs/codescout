//! `tests/mcp-smoke-*.sh` drive a live MCP server by hand and sit outside
//! `cargo test`, clippy, and every markdown-scoped gate — nothing re-reads them
//! when a tool is renamed or removed.
//! `docs/issues/archive/2026-09-11-mcp-smoke-scripts-call-a-parameter-and-a-tool-that-do-not-exist.md`
//! is the bug this closes: both scripts called `get_symbols_overview`, a tool
//! folded into `symbols` months earlier, and neither mistake errored loudly —
//! a `symbols` call with no name argument silently falls back to the path
//! overview, which still returns symbols and satisfies a naive "found
//! something" assertion.
//!
//! **Static, not live.** This does not spin up an MCP server — these scripts
//! exist precisely because a unit test cannot drive stdio+LSP through one
//! interactive scenario per case. It cross-references every `call <name>`
//! invocation against `fn name(&self) -> &str { "<literal>" }` bodies under
//! `src/tools/`, the same literal every registered tool's `Tool::name()`
//! returns — cheap, mechanical, and the same source of truth a live registry
//! reads from.
//!
//! **Ceiling, stated rather than left implicit:** misses a tool whose `name()`
//! returns something other than a string literal (`Self::NAME`, `self.name`) —
//! `ActivateProject` is the one in-tree example today. Neither smoke script
//! calls it, so the miss costs nothing now; if one ever does, this check needs
//! widening rather than trusting a false pass.

use std::collections::BTreeSet;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn smoke_scripts() -> Vec<PathBuf> {
    ["tests/mcp-smoke-rust.sh", "tests/mcp-smoke-kotlin.sh"]
        .iter()
        .map(|p| repo_root().join(p))
        .collect()
}

/// Every tool name declared via a string-literal `fn name(&self) -> &str { "..." }`
/// body anywhere under `src/tools/`.
fn declared_tool_names() -> BTreeSet<String> {
    let re = regex::Regex::new(r#"fn name\(&self\) -> &str \{\s*"([a-z_][a-z_0-9]*)""#).unwrap();
    let mut names = BTreeSet::new();
    let mut stack = vec![repo_root().join("src/tools")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let Ok(src) = std::fs::read_to_string(&path) else {
                    continue;
                };
                for cap in re.captures_iter(&src) {
                    names.insert(cap[1].to_string());
                }
            }
        }
    }
    names
}

/// Every `call <name>` invocation in a script, as `(1-indexed line, name)`.
fn called_tool_names(content: &str) -> Vec<(usize, String)> {
    let re = regex::Regex::new(r"^\s*call ([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
    content
        .lines()
        .enumerate()
        .filter_map(|(i, line)| re.captures(line).map(|c| (i + 1, c[1].to_string())))
        .collect()
}

#[test]
fn mcp_smoke_scripts_call_only_registered_tool_names() {
    let declared = declared_tool_names();
    let mut offenders = Vec::new();

    for script in smoke_scripts() {
        let content = std::fs::read_to_string(&script)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", script.display()));
        let rel = script
            .strip_prefix(repo_root())
            .unwrap_or(&script)
            .display()
            .to_string();
        for (line, name) in called_tool_names(&content) {
            if !declared.contains(&name) {
                offenders.push(format!("{rel}:{line} — call {name}"));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "these smoke-script calls name a tool this crate does not register:\n  {}\n\n\
         Either the tool was renamed/removed and the script is stale (fix the call), \
         or `declared_tool_names`'s literal-only scan needs widening for a tool whose \
         `name()` is not a bare string literal.",
        offenders.join("\n  ")
    );
}

/// Non-vacuity: the population this gate checks must actually be non-empty in both
/// directions, or an empty result reads as "nothing wrong" when it might mean
/// "found nothing to check" — the same false-green class as a mistyped directory in
/// `tests/committed_paths.rs`'s own non-vacuity test.
#[test]
fn the_call_and_declared_name_populations_are_not_vacuous() {
    let declared = declared_tool_names();
    assert!(
        declared.len() > 20,
        "expected >20 registered tool names from src/tools/'s name() literals; got {} — \
         the scan is looking in the wrong place or the regex stopped matching",
        declared.len()
    );
    assert!(
        declared.contains("symbols") && declared.contains("read_file") && declared.contains("grep"),
        "expected well-known tool names among the declared set; got: {declared:?}"
    );

    let mut total_calls = 0usize;
    for script in smoke_scripts() {
        let content = std::fs::read_to_string(&script)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", script.display()));
        total_calls += called_tool_names(&content).len();
    }
    assert!(
        total_calls > 10,
        "expected >10 `call <tool>` invocations across both smoke scripts; got {total_calls} — \
         the line regex stopped matching the scripts' actual call shape"
    );
}
