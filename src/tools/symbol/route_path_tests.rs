//! The `references(symbol=…, path=…)` call that `edit_code` names after a `rename` or `replace`,
//! and the `path=…` that a `symbols` overflow hint offers, carry a real file's path (and a symbol's
//! name). Both are text a caller copies and runs, so each must read back as the argument it claims
//! to be: written between two raw quotes, a `"` or `\` in the path ends the literal early or starts
//! an escape.
//!
//! Every test parses the text with a real JSON parser ([`json_string_after`]) and compares the
//! value with the input, byte for byte. The tests outside [`unix_only`] feed the builders a `&str`
//! that is not a real file, so they run on every platform; the ones in [`unix_only`] drive the real
//! `EditCode` tool against files whose names NTFS forbids (`"`, `\`), so they are `cfg(unix)`.

use super::edit_code::verify_callers_hint;
use super::symbols::make_search_symbols_hint;
use crate::util::text::json_string_after;

/// What a path or a name can hold that a raw `"…"` cannot carry. Not files: the builders take a `&str`.
const HAZARDS: [&str; 11] = [
    "plain.rs",
    "sp ace.rs",
    "q\"uote.rs",
    "it's.rs",
    "back\\slash.rs",
    "tab\there.rs",
    "ünï€😀 日本.rs",
    "a\"b\\c 'd' $e `f` ü.rs",
    "e\\\"\\\\\"",
    "ctl\u{1}\u{7f}.rs",
    "src/dir \"x\"/and\\back/line\nbreak.rs",
];

/// The overflow hint of a symbol search offers the top file as the `path=` to narrow with.
#[test]
fn the_by_file_hint_reads_back_to_the_top_file() {
    for file in HAZARDS {
        let by_file = vec![(file.to_string(), 7), ("z.rs".to_string(), 1)];
        let hint = make_search_symbols_hint(9, &by_file);
        let (path, rest) = json_string_after(&hint, "filter by file: add path=");
        assert_eq!(path, file, "{hint}");
        // The literal ends its line, so the line after it is the kind hint, whatever the name held.
        assert!(
            rest.starts_with("\n\u{2022} filter by kind:"),
            "the path literal must end its own line: {hint:?}"
        );
    }
}

/// With no file to offer, the placeholder is a literal too, and stays what it was.
#[test]
fn the_by_file_hint_without_a_file_names_the_placeholder() {
    let hint = make_search_symbols_hint(9, &[]);
    assert!(hint.contains("add path=\"path/to/file.rs\"\n"), "{hint}");
}

/// The `references(...)` call of a successful edit reads back to the symbol and the path, each in
/// its own argument.
#[test]
fn the_verify_callers_hint_reads_back_to_the_symbol_and_the_path() {
    for path in HAZARDS {
        for symbol in HAZARDS {
            let hint = verify_callers_hint(symbol, path);
            let (got_symbol, rest) = json_string_after(&hint, "references(symbol=");
            assert_eq!(got_symbol, symbol, "{hint}");
            let (got_path, rest) = json_string_after(rest, ", path=");
            assert_eq!(got_path, path, "{hint}");
            assert_eq!(rest, ")", "{hint}");
        }
    }
}

/// The overview hint's scan-the-whole-tree call carries the directory as the JSON string literal of
/// its `path` argument. Fed a `&str` that is not a directory, so it runs on every platform.
#[test]
fn the_full_tree_call_reads_back_to_the_directory_glob() {
    for dir in HAZARDS {
        let call = super::list_overview::full_tree_call(dir);
        let (glob, rest) = json_string_after(&call, "symbols(path=");
        assert_eq!(glob, format!("{dir}/**/*"), "{call}");
        assert_eq!(rest, ")", "{call}");
    }
}

#[cfg(unix)]
mod unix_only {
    use super::*;
    use crate::agent::Agent;
    use crate::lsp::{mock::MockLspClient, mock::MockLspProvider, SymbolInfo, SymbolKind};
    use crate::tools::symbol::{EditCode, Symbols};
    use crate::tools::{Tool, ToolContext};
    use serde_json::json;
    use std::sync::Arc;

    /// Legal Linux file names that break a raw `path="…"`: `"`, `\`, `'`, a space, a tab, non-ASCII.
    const NAMES: [&str; 6] = [
        "q\"uote.rs",
        "back\\slash.rs",
        "sp ace it's.rs",
        "tab\there.rs",
        "e\\\"\\\\\".rs",
        "a\"b\\c 'd' ünï 日本.rs",
    ];

    /// A project whose `src/<name>` holds `pub fn f() -> i32 { 1 }`, and a context whose language
    /// server answers with that one symbol. The key of the mock is the canonical file, the form the
    /// tool resolves to.
    async fn ctx_with_function_in(name: &str) -> (tempfile::TempDir, ToolContext, String) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let rel = format!("src/{name}");
        let file = root.join(&rel);
        std::fs::write(&file, "pub fn f() -> i32 {\n    1\n}\n").unwrap();
        let symbol = SymbolInfo {
            name: "f".to_string(),
            name_path: "f".to_string(),
            kind: SymbolKind::Function,
            file: file.clone(),
            start_line: 0,
            end_line: 2,
            start_col: 7,
            children: vec![],
            range_start_line: None,
            detail: None,
        };
        let lsp =
            MockLspProvider::with_client(MockLspClient::new().with_symbols(&file, vec![symbol]));
        let ctx = ToolContext {
            agent: Agent::new(Some(dir.path().to_path_buf())).await.unwrap(),
            lsp,
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(50)),
            progress: None,
            peer: None,
            section_coverage: Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
            guide_hints_emitted: Arc::new(parking_lot::Mutex::new(Default::default())),
            workspace_override: None,
        };
        (dir, ctx, rel)
    }

    fn assert_hint_names(hint: &str, rel: &str, what: &str) {
        let (symbol, rest) = json_string_after(hint, "references(symbol=");
        assert_eq!(symbol, "f", "{what}: {hint}");
        let (path, rest) = json_string_after(rest, ", path=");
        assert_eq!(path, rel, "{what}: {hint}");
        assert_eq!(rest, ")", "{what}: {hint}");
    }

    /// A `replace` and a `rename` of a symbol in a file with an awkward name each end with a hint
    /// whose `references(...)` call reads back to that file. They build the hint at two sites.
    #[tokio::test]
    async fn the_hint_of_a_replace_and_of_a_rename_reads_back_to_the_file() {
        for name in NAMES {
            let (_dir, ctx, rel) = ctx_with_function_in(name).await;

            let replaced = EditCode
                .call(
                    json!({
                        "action": "replace",
                        "symbol": "f",
                        "path": rel,
                        "body": "pub fn f() -> i32 {\n    2\n}",
                    }),
                    &ctx,
                )
                .await
                .unwrap_or_else(|e| panic!("replace in {name:?}: {e}"));
            let hint = replaced["hint"]
                .as_str()
                .unwrap_or_else(|| panic!("replace in {name:?} gave no hint: {replaced}"));
            assert_hint_names(hint, &rel, &format!("replace in {name:?}"));

            let renamed = EditCode
                .call(
                    json!({
                        "action": "rename",
                        "symbol": "f",
                        "path": rel,
                        "new_name": "g",
                    }),
                    &ctx,
                )
                .await
                .unwrap_or_else(|e| panic!("rename in {name:?}: {e}"));
            let hint = renamed["hint"]
                .as_str()
                .unwrap_or_else(|| panic!("rename in {name:?} gave no hint: {renamed}"));
            assert_hint_names(hint, &rel, &format!("rename in {name:?}"));
        }
    }
    /// A directory-overview hint offers `symbols(path=<dir>/**/*)` to scan the whole tree, in both
    /// of the modes that print it (31-80 files with subdirectories, and more than 80). The call
    /// reads back to the directory the caller named, whatever its name holds. The tool is run for
    /// real: the directories are legal Linux names, and no language server is involved.
    #[tokio::test]
    async fn the_scan_the_full_tree_call_of_an_overview_reads_back_to_the_directory() {
        for name in NAMES {
            let dir_name = name.trim_end_matches(".rs");
            for (files_per_subdir, mode) in [(20usize, "class_overview"), (50, "directory_map")] {
                let dir = tempfile::tempdir().unwrap();
                std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
                for sub in ["a", "b"] {
                    let d = dir.path().join(dir_name).join(sub);
                    std::fs::create_dir_all(&d).unwrap();
                    for i in 0..files_per_subdir {
                        std::fs::write(d.join(format!("f{i}.rs")), format!("pub struct S{i};\n"))
                            .unwrap();
                    }
                }
                let ctx = ToolContext {
                    agent: Agent::new(Some(dir.path().to_path_buf())).await.unwrap(),
                    lsp: crate::lsp::LspManager::new_arc(),
                    output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(50)),
                    progress: None,
                    peer: None,
                    section_coverage: Arc::new(std::sync::Mutex::new(
                        crate::tools::section_coverage::SectionCoverage::new(),
                    )),
                    guide_hints_emitted: Arc::new(parking_lot::Mutex::new(Default::default())),
                    workspace_override: None,
                };

                let result = Symbols
                    .call(json!({ "path": dir_name }), &ctx)
                    .await
                    .unwrap_or_else(|e| panic!("{dir_name:?} {mode}: {e}"));
                assert_eq!(result["mode"], mode, "{dir_name:?}: {result}");
                let hint = result["hint"].as_str().unwrap();
                // The first call in the hint is the placeholder `symbols(path="<subdir>")`.
                let after_placeholder = hint
                    .split_once("symbols(path=\"<subdir>\")")
                    .unwrap_or_else(|| panic!("{dir_name:?} {mode}: {hint}"))
                    .1;
                let (glob, rest) = json_string_after(after_placeholder, "symbols(path=");
                assert_eq!(
                    glob,
                    format!("{dir_name}/**/*"),
                    "{dir_name:?} {mode}: {hint}"
                );
                assert!(
                    rest.starts_with(") to scan the full tree"),
                    "{dir_name:?} {mode}: {hint}"
                );
            }
        }
    }
}
