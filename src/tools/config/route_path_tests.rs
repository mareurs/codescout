//! The hints of `workspace(action='activate')` and its write-block refusal name a directory in a
//! call a caller copies and runs: `workspace(action='activate', path=<home>)` to go back, and the
//! per-call `workspace=<root>` pin. A directory name with a `"` or `\` written between two raw
//! quotes ends the literal early or starts an escape, so the call stops being the one it claims to
//! be.
//!
//! Every test parses the text with a real JSON parser ([`json_string_after`]) and compares the
//! value with the input, byte for byte. The test outside [`unix_only`] feeds the builder a `&str`
//! that is not a real directory, so it runs on every platform; the ones in [`unix_only`] drive the
//! real `ActivateProject` tool against a home directory whose name NTFS forbids (`"`, `\`), so
//! they are `cfg(unix)`.

use super::workspace_pin_contrast;
use crate::util::text::json_string_after;

/// What a directory can hold that a raw `"…"` cannot carry. Not directories: the builder takes a `&str`.
const HAZARDS: [&str; 10] = [
    "/home/me/plain",
    "/home/me/sp ace",
    "/home/me/q\"uote",
    "/home/me/it's",
    "/home/me/back\\slash",
    "/home/me/tab\there",
    "/home/me/ünï€😀 日本",
    "/home/me/a\"b\\c 'd' $e `f` ü",
    "/home/me/e\\\"\\\\\"",
    "C:\\Users\\me\\work\\\"x\"",
];

/// The pin notice names the project root as the `workspace=` pin and the home directory as the
/// path to `activate`, each as a JSON string literal that reads back. The two differ, so a root
/// written where the home belongs (or the other way) is seen.
#[test]
fn the_pin_notice_reads_back_to_the_root_and_to_the_home() {
    for (i, root) in HAZARDS.iter().enumerate() {
        let home = HAZARDS[(i + 1) % HAZARDS.len()];
        let text = workspace_pin_contrast("Switched project (read-write). CWD: x", root, home);

        let (pinned, _) = json_string_after(&text, "pass workspace=");
        assert_eq!(&pinned, root, "{text}");

        let (activated, rest) =
            json_string_after(&text, "remember to workspace(action='activate', path=");
        assert_eq!(activated, home, "{text}");
        assert_eq!(rest, ") when done.", "{text}");
    }
}

#[cfg(unix)]
mod unix_only {
    use super::*;
    use crate::agent::Agent;
    use crate::tools::config::ActivateProject;
    use crate::tools::{Tool, ToolContext};
    use serde_json::json;
    use std::sync::Arc;

    /// Legal Linux directory names that break a raw `path="…"`: `"`, `\`, `'`, a space, a tab,
    /// non-ASCII.
    const HOMES: [&str; 6] = [
        "h\"ome",
        "ho\\me",
        "ho me it's",
        "ho\tme",
        "e\\\"\\\\\"",
        "a\"b\\c 'd' ünï 日本",
    ];

    /// Activating another project from a home directory with an awkward name ends with the way back:
    /// a `workspace(action='activate', path=<home>)` call that reads back to the home. The
    /// read-only browse and the read-write switch build it at two sites; neither depends on the
    /// pin-notice gate, which changes the sentence around the call and not the call.
    #[tokio::test]
    async fn the_way_back_to_an_awkward_home_reads_back_to_the_home() {
        for name in HOMES {
            let parent = tempfile::tempdir().unwrap();
            let home = parent.path().join(name);
            let away = parent.path().join("away");
            for dir in [&home, &away] {
                std::fs::create_dir_all(dir.join(".codescout")).unwrap();
            }
            let home = std::fs::canonicalize(&home).unwrap();
            let away = std::fs::canonicalize(&away).unwrap();
            let ctx = ToolContext {
                agent: Agent::new(Some(home.clone())).await.unwrap(),
                lsp: crate::lsp::LspManager::new_arc(),
                output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
                progress: None,
                peer: None,
                section_coverage: Arc::new(std::sync::Mutex::new(
                    crate::tools::section_coverage::SectionCoverage::new(),
                )),
                guide_hints_emitted: Arc::new(parking_lot::Mutex::new(Default::default())),
                workspace_override: None,
            };

            for (read_only, opens) in [(true, "Browsing "), (false, "Switched project")] {
                let result = ActivateProject
                    .call(
                        json!({ "path": away.to_str().unwrap(), "read_only": read_only }),
                        &ctx,
                    )
                    .await
                    .unwrap_or_else(|e| panic!("{name:?} read_only={read_only}: {e}"));
                let hint = result["hint"].as_str().unwrap();
                assert!(
                    hint.starts_with(opens),
                    "read_only={read_only} must reach the {opens:?} hint, got: {hint}"
                );

                let (back, rest) =
                    json_string_after(hint, "remember to workspace(action='activate', path=");
                assert_eq!(
                    back,
                    crate::util::fs::to_forward_slash(&home),
                    "{name:?} read_only={read_only}: {hint}"
                );
                assert_eq!(
                    rest, ") when done.",
                    "{name:?} read_only={read_only}: {hint}"
                );

                // Back home, so the next round starts from the same state.
                ActivateProject
                    .call(json!({ "path": home.to_str().unwrap() }), &ctx)
                    .await
                    .unwrap();
            }
        }
    }
}
