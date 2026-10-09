//! The narrowing candidates of a `grep` overflow hint name a real file's path, and a hint is text a
//! caller copies and runs: `path="<file>" (N matches)`.
//!
//! A `"` or `\` in the file's name written raw ends the string literal early or starts an escape,
//! so the candidate stops being the argument it claims to be. Every test here reads the path back
//! with a real JSON parser ([`json_string_after`]) and compares it with the file's, byte for
//! byte. The test outside [`unix_only`] feeds the builder a `&str` that is not a real file, so it
//! runs on every platform; the ones in [`unix_only`] drive the real `Grep` tool against files
//! whose names NTFS forbids (`"`, `\`), so they are `cfg(unix)`.

use super::path_candidate;
use crate::util::text::json_string_after;

/// What a path can hold that a raw `path="…"` cannot carry. Not files: the builder takes a `&str`.
const HAZARDS: [&str; 12] = [
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
    "line\nbreak.rs",
    "/abs/with \"quote\"/and\\back/x.rs",
];

/// `path_candidate` writes the file as a JSON string literal and the tally after it: the path reads
/// back exactly, and the tally is what follows.
#[test]
fn a_candidate_reads_back_to_the_exact_path_it_names() {
    for file in HAZARDS {
        let candidate = path_candidate(file, "3 matches");
        let (path, rest) = json_string_after(&candidate, "path=");
        assert_eq!(path, file, "{candidate}");
        assert_eq!(rest, " (3 matches)", "{candidate}");
        // A candidate is one line of a hint whatever the name holds.
        assert!(
            !candidate.contains('\n'),
            "a line break in a name must be escaped: {candidate:?}"
        );
    }
}

#[cfg(unix)]
mod unix_only {
    use super::*;
    use crate::agent::Agent;
    use crate::lsp::LspManager;
    use crate::tools::grep::Grep;
    use crate::tools::Tool;
    use serde_json::json;
    use std::path::Path;
    use std::sync::Arc;

    /// Legal Linux file names that break a raw `path="…"`: `"`, `'`, a space, a tab, non-ASCII.
    ///
    /// No backslash: `grep` reports every file as `to_forward_slash(path)`, which rewrites a `\` in
    /// a name to `/` (documented on that function), so the path in the hint names no file for such
    /// a name whatever the quoting. The backslash is covered by the test above, on a `&str`.
    const NAMES: [&str; 6] = [
        "q\"uote.rs",
        "\"lead.rs",
        "a\"\"b.rs",
        "sp ace it's.rs",
        "tab\there.rs",
        "ünï 日本 \"x\" 'y'.rs",
    ];

    async fn project_ctx(root: &Path) -> crate::tools::ToolContext {
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        crate::tools::ToolContext {
            agent: Agent::new(Some(root.to_path_buf())).await.unwrap(),
            lsp: LspManager::new_arc(),
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(50)),
            progress: None,
            peer: None,
            section_coverage: Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
            guide_hints_emitted: Arc::new(parking_lot::Mutex::new(
                crate::tools::guide_ledger::GuideLedger::mid_session(),
            )),
            workspace_override: None,
        }
    }

    /// An overflowing search in a directory whose only file has an awkward name offers that file as
    /// the narrowing candidate, and the candidate reads back to the file: in the grouped mode
    /// (`context_lines` 0) and in the context mode, which build the candidate in two places.
    #[tokio::test]
    async fn the_narrowing_candidate_of_an_overflowing_search_reads_back_to_the_file() {
        for context_lines in [0u64, 1] {
            for name in NAMES {
                let dir = tempfile::tempdir().unwrap();
                let root = dir.path().canonicalize().unwrap();
                let body: String = (0..400)
                    .map(|i| format!("fn target_{i}() {{}}\n"))
                    .collect();
                std::fs::write(root.join(name), body).unwrap();
                let ctx = project_ctx(&root).await;

                let mut input = json!({
                    "pattern": "target",
                    "path": root.to_str().unwrap(),
                    "limit": 5,
                });
                if context_lines > 0 {
                    input["context_lines"] = json!(context_lines);
                }
                let result = Grep.call(input, &ctx).await.unwrap();
                let hint = result["overflow"]["hint"].as_str().unwrap_or_else(|| {
                    panic!("limit 5 against 400 matches must overflow with a hint: {result}")
                });

                let (path, rest) = json_string_after(hint, "path=");
                assert_eq!(
                    std::fs::canonicalize(&path).unwrap_or_else(|e| panic!(
                        "context_lines={context_lines} {name:?}: the candidate names {path:?}, \
                         which is no file ({e}): {hint}"
                    )),
                    root.join(name),
                    "context_lines={context_lines} {name:?}: {hint}"
                );
                assert!(
                    rest.starts_with(" ("),
                    "context_lines={context_lines} {name:?}: the tally must follow the path \
                     literal: {hint}"
                );
            }
        }
    }
}
