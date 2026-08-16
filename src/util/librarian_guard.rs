/// Guard that rejects direct reads/edits on librarian-managed artifact files.
///
/// Librarian artifacts have YAML frontmatter with an `id: <16-hex>` field. Agents
/// should use `artifact(action="get"/"update")` instead of reading/editing the
/// backing file directly — the raw file lacks catalog metadata (link graph,
/// augmentation state, event history).
use crate::tools::RecoverableError;

/// Returns a `RecoverableError` if `text` looks like a librarian-managed artifact.
/// Call this after the file has been read, before any read or mutation logic.
pub fn guard_not_librarian_managed(
    path: &str,
    text: &str,
    abs_path: Option<&std::path::Path>,
) -> Result<(), anyhow::Error> {
    guard_with_oracle(path, text, abs_path, oracle())
}

/// Answers whether a path is an **augmented** librarian artifact — one whose
/// structured `params` live in the catalog while the file holds only a rendered
/// snapshot of them.
///
/// Augmentation, not catalog membership, is the property that makes a direct read
/// or edit wrong. Measured 2026-08-16: the catalog holds 500+ rows including
/// `docs/RELEASE.md`, `CONTRIBUTING.md` and every ADR, but only **16** artifacts
/// are augmented, all of them trackers. Guarding by membership would refuse the
/// entire documentation set; guarding by augmentation refuses exactly the files
/// whose state does not live in the file.
pub trait AugmentedArtifactOracle: Send + Sync {
    fn is_augmented(&self, abs_path: &std::path::Path) -> bool;
}

static ORACLE: std::sync::OnceLock<std::sync::Arc<dyn AugmentedArtifactOracle>> =
    std::sync::OnceLock::new();

/// Install the process-wide oracle. Called once, when the librarian runtime is
/// built; later calls are ignored. Left unset (tests, `--no-default-features`)
/// the guard degrades to its frontmatter check rather than failing open loudly.
pub fn install_augmented_oracle(oracle: std::sync::Arc<dyn AugmentedArtifactOracle>) {
    let _ = ORACLE.set(oracle);
}

fn oracle() -> Option<&'static dyn AugmentedArtifactOracle> {
    ORACLE.get().map(|o| o.as_ref())
}

/// The testable core: same decision, with the oracle passed explicitly so a test
/// never has to install into the process-wide `OnceLock`.
fn guard_with_oracle(
    path: &str,
    text: &str,
    abs_path: Option<&std::path::Path>,
    oracle: Option<&dyn AugmentedArtifactOracle>,
) -> Result<(), anyhow::Error> {
    // Two independent reasons a file is off-limits, and neither implies the other:
    // a stamped frontmatter id says the librarian wrote this file; augmentation says
    // the file is not where its state lives. An augmented tracker can carry no id at
    // all, and a plain bug file carries one without being augmented.
    let augmented = matches!((abs_path, oracle), (Some(p), Some(o)) if o.is_augmented(p));
    if !augmented && !is_librarian_artifact(text) {
        return Ok(());
    }
    let why = if augmented {
        " (augmented — its params live in the catalog, and this file is only a \
         rendered snapshot of them)"
    } else {
        ""
    };
    Err(RecoverableError::with_hint(
        format!("'{path}' is a librarian-managed artifact{why} — do not read or edit it directly"),
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
        if let Some(val) = line.strip_prefix("id:") {
            return is_librarian_id(val.trim());
        }
    }
    false
}

/// `true` for a librarian id — exactly 16 lowercase hex characters — accepting
/// whatever quoting the frontmatter happened to be serialised with.
///
/// Quoting is not a property of the artifact. The same id round-trips as
/// `id: abc…` or `id: 'abc…'` depending only on which writer last emitted the
/// file, so testing the raw token's length made *protection* depend on that
/// accident: a quoted id is 18 characters, failed the length test, and the file
/// read as unmanaged. Measured 2026-08-16, `docs/trackers/` alone had 12 files
/// guarded and 15 unguarded on that basis — including the active work queue.
/// BL-33 / `docs/issues/2026-08-16-librarian-guard-misses-quoted-frontmatter-ids.md`.
///
/// This is only half the guard, and the cheaper half. It validates *shape*, never
/// *value*, so a well-formed but stale id keeps the guard on — the safe direction
/// to be wrong in — and a file carrying no `id:` at all is invisible to it. The
/// dangerous case it misses is an **augmented** tracker with no stamped id, where
/// the file is merely a snapshot of params held in the catalog; that is what
/// [`AugmentedArtifactOracle`] covers.
///
/// The two together are a union, not a fallback: a stamped id says the librarian
/// *wrote* this file, augmentation says the file is not where its state *lives*,
/// and neither implies the other.
fn is_librarian_id(val: &str) -> bool {
    let val = strip_matching_quotes(val);
    val.len() == 16 && val.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Strip one layer of matching `'` or `"`. An unbalanced or mismatched pair is
/// left alone, so a malformed value fails the hex test rather than being coerced
/// through it.
fn strip_matching_quotes(val: &str) -> &str {
    for q in ['\'', '"'] {
        if let Some(inner) = val.strip_prefix(q).and_then(|v| v.strip_suffix(q)) {
            return inner;
        }
    }
    val
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
        let err = guard_not_librarian_managed("docs/trackers/foo.md", text, None).unwrap_err();
        let re = err.downcast_ref::<RecoverableError>().unwrap();
        assert!(re.message.contains("librarian-managed artifact"));
        assert!(re.hint().unwrap().contains("artifact(action="));
    }

    #[test]
    fn guard_passes_for_plain_markdown() {
        let text = "# A plain markdown file\nNo frontmatter.\n";
        assert!(guard_not_librarian_managed("docs/notes.md", text, None).is_ok());
    }

    /// BL-33 / `docs/issues/2026-08-16-librarian-guard-misses-quoted-frontmatter-ids.md`.
    ///
    /// The predicate tested the raw token's length, so `'9a892c2a5976e296'` — 18
    /// characters once YAML quotes it — read as unmanaged and the guard stayed silent.
    /// Quoting is not a property of the artifact; it is whichever form the writer that
    /// last serialised the frontmatter happened to emit. Measured 2026-08-16 in
    /// `docs/trackers/`: 12 files protected, **15 not**, the unprotected set including
    /// the active work queue.
    ///
    /// Table-driven over both forms deliberately. The `bare` row was green before the
    /// fix, which is how this survived the `47abcb6d` hardening pass — a test written
    /// with only the unquoted form cannot fail, however many write paths it covers.
    ///
    /// Mutation caught: dropping the quote-stripping restores the silent hole.
    #[test]
    fn an_id_is_recognised_whatever_yaml_quoting_it_was_serialised_with() {
        for (label, line) in [
            ("bare", "id: 9a892c2a5976e296"),
            ("single-quoted", "id: '9a892c2a5976e296'"),
            ("double-quoted", "id: \"9a892c2a5976e296\""),
            ("extra spacing", "id:   9a892c2a5976e296"),
            ("no space", "id:9a892c2a5976e296"),
        ] {
            let text = format!("---\n{line}\nkind: tracker\n---\n\n# Body\n");
            assert!(
                is_librarian_artifact(&text),
                "{label}: a librarian id must be recognised regardless of YAML quoting \
                 — got false for `{line}`"
            );
        }
    }

    /// The other half of the same change: accepting quotes must not loosen what counts
    /// as an id. Without this, `strip` could turn any short or malformed token into a
    /// match and start guarding files that are not artifacts at all.
    #[test]
    fn stripping_quotes_does_not_loosen_the_id_rule() {
        for (label, line) in [
            ("too short", "id: 'abc123'"),
            ("uppercase", "id: '9A892C2A5976E296'"),
            ("non-hex", "id: 'zzzzzzzzzzzzzzzz'"),
            ("mismatched quotes", "id: '9a892c2a5976e296\""),
            ("unterminated", "id: '9a892c2a5976e296"),
            ("empty quotes", "id: ''"),
            ("quotes only", "id: '"),
        ] {
            let text = format!("---\n{line}\nkind: tracker\n---\n");
            assert!(
                !is_librarian_artifact(&text),
                "{label}: the 16-lowercase-hex rule must survive quote-stripping \
                 — got true for `{line}`"
            );
        }
    }

    /// Wiring check at the guard's own entry point rather than the predicate, since
    /// that is the function all three call sites (`read_markdown`, `edit_markdown`,
    /// `edit_file`) share.
    #[test]
    fn guard_fires_on_a_quoted_id_the_way_it_does_on_a_bare_one() {
        let quoted = "---\nid: '9a892c2a5976e296'\nkind: tracker\n---\n";
        let err =
            guard_not_librarian_managed("docs/trackers/open-issue-work-queue.md", quoted, None)
                .expect_err("a quoted id is still a librarian id — the guard must refuse");
        let re = err.downcast_ref::<RecoverableError>().unwrap();
        assert!(re.message.contains("librarian-managed artifact"));
        assert!(re.hint().unwrap().contains("artifact(action="));
    }

    /// A tracker can be **augmented** — `params` in the catalog, the file only a
    /// rendered snapshot — while carrying no frontmatter `id:` at all, which the
    /// text predicate cannot see however its quoting is handled. That is the most
    /// dangerous class of file, not the least: editing it desynchronises file from
    /// params, and reading it returns a snapshot with no staleness signal.
    ///
    /// Measured 2026-08-16: 16 augmented artifacts repo-wide, of which
    /// `docs/trackers/artifact-augmentation-followups.md` carries no id.
    #[test]
    fn an_augmented_artifact_is_guarded_even_with_no_frontmatter_id() {
        struct OnlyThisPath(&'static str);
        impl AugmentedArtifactOracle for OnlyThisPath {
            fn is_augmented(&self, p: &std::path::Path) -> bool {
                p.ends_with(self.0)
            }
        }

        let text = "---\nkind: tracker\nstatus: active\n---\n\n# Followups\n";
        assert!(
            !is_librarian_artifact(text),
            "precondition: this file has no frontmatter id for the text check to find"
        );

        let oracle = OnlyThisPath("artifact-augmentation-followups.md");
        let abs = std::path::Path::new("/repo/docs/trackers/artifact-augmentation-followups.md");
        let err = guard_with_oracle(
            "docs/trackers/artifact-augmentation-followups.md",
            text,
            Some(abs),
            Some(&oracle),
        )
        .expect_err("an augmented artifact must be refused whatever its frontmatter looks like");
        let re = err.downcast_ref::<RecoverableError>().unwrap();
        assert!(re.message.contains("librarian-managed artifact"));
    }

    /// The other side, and the reason the predicate is **augmentation** rather than
    /// catalog membership: `docs/RELEASE.md`, `CONTRIBUTING.md` and every ADR are
    /// catalog rows, and CLAUDE.md documents `edit_markdown(...)` as the way to
    /// append to `docs/trackers/skill-frictions.md` — itself a catalog row with no
    /// frontmatter id. Guarding by membership would refuse all of them. Guarding by
    /// augmentation refuses none.
    ///
    /// Green before the oracle was wired, deliberately: it pins the behaviour the
    /// fix must not break while widening the guard.
    #[test]
    fn a_catalogued_but_unaugmented_file_stays_directly_editable() {
        struct NothingIsAugmented;
        impl AugmentedArtifactOracle for NothingIsAugmented {
            fn is_augmented(&self, _: &std::path::Path) -> bool {
                false
            }
        }

        for (label, display, text) in [
            (
                "prose tracker",
                "docs/trackers/skill-frictions.md",
                "---\nkind: tracker\nstatus: active\ntitle: Skill Frictions Tracker\n---\n\n## `/x`\n",
            ),
            (
                "plain doc",
                "docs/RELEASE.md",
                "---\nkind: unknown\nstatus: unknown\ntitle: Release & Ship Procedures\n---\n\n# Release\n",
            ),
        ] {
            let abs = std::path::PathBuf::from("/repo").join(display);
            assert!(
                guard_with_oracle(display, text, Some(&abs), Some(&NothingIsAugmented)).is_ok(),
                "{label}: a catalogued but unaugmented file must stay directly editable"
            );
        }
    }
}
