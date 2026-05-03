/// Guard that rejects direct reads/edits on librarian-managed artifact files.
///
/// Librarian artifacts have YAML frontmatter with an `id: <16-hex>` field. Agents
/// should use `artifact(action="get"/"update")` instead of reading/editing the
/// backing file directly — the raw file lacks catalog metadata (link graph,
/// augmentation state, event history).
use crate::tools::RecoverableError;

/// Returns a `RecoverableError` if `text` looks like a librarian-managed artifact.
/// Call this after the file has been read, before any read or mutation logic.
pub fn guard_not_librarian_managed(path: &str, text: &str) -> Result<(), anyhow::Error> {
    if !is_librarian_artifact(text) {
        return Ok(());
    }
    Err(RecoverableError::with_hint(
        format!(
            "'{}' is a librarian-managed artifact — do not read or edit it directly",
            path
        ),
        "Use artifact tools instead:\n\
         • Read:   artifact(action=\"get\", id=\"<id>\")\n\
         • Find:   artifact(action=\"find\", semantic=\"<topic>\")\n\
         • Edit:   artifact(action=\"update\", id=\"<id>\", patch={...})\n\
         Full guide: resources/read doc://librarian-guide",
    )
    .into())
}

/// Returns `true` when the file begins with YAML frontmatter that contains
/// an `id:` field matching the 16-char lowercase hex format used by librarian.
pub fn is_librarian_artifact(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("---\n") else {
        return false;
    };
    for line in rest.lines() {
        if line == "---" {
            break;
        }
        if let Some(val) = line.strip_prefix("id: ") {
            let val = val.trim();
            return val.len() == 16 && val.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_librarian_artifact() {
        let text = "---\nid: 79a6276776a1b5da\nkind: tracker\n---\n# Body\n";
        assert!(is_librarian_artifact(text));
    }

    #[test]
    fn ignores_non_frontmatter_file() {
        let text = "# Just a heading\nNo frontmatter here.\n";
        assert!(!is_librarian_artifact(text));
    }

    #[test]
    fn ignores_wrong_id_length() {
        let text = "---\nid: abc123\nkind: spec\n---\n";
        assert!(!is_librarian_artifact(text));
    }

    #[test]
    fn ignores_uppercase_hex_id() {
        let text = "---\nid: 79A6276776A1B5DA\nkind: tracker\n---\n";
        assert!(!is_librarian_artifact(text));
    }

    #[test]
    fn ignores_non_hex_id() {
        let text = "---\nid: xxxxxxxxxxxxxxxx\nkind: spec\n---\n";
        assert!(!is_librarian_artifact(text));
    }

    #[test]
    fn guard_returns_recoverable_error_for_artifact() {
        let text = "---\nid: abc513d3ee0f0b50\nkind: tracker\n---\n";
        let err = guard_not_librarian_managed("docs/trackers/foo.md", text).unwrap_err();
        let re = err.downcast_ref::<RecoverableError>().unwrap();
        assert!(re.message.contains("librarian-managed artifact"));
        assert!(re.hint().unwrap().contains("artifact(action="));
    }

    #[test]
    fn guard_passes_for_plain_markdown() {
        let text = "# A plain markdown file\nNo frontmatter.\n";
        assert!(guard_not_librarian_managed("docs/notes.md", text).is_ok());
    }
}
