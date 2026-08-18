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
    // Clone the Arc out first — see `read_from` on why the lock must not be held
    // across `is_augmented`.
    let oracle = oracle();
    guard_with_oracle(path, text, abs_path, oracle.as_deref())
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

/// The slot an oracle lives in. Named so a test can own one and prove the
/// install semantics without touching the process-wide `ORACLE`.
type OracleSlot = std::sync::RwLock<Option<std::sync::Arc<dyn AugmentedArtifactOracle>>>;

/// Last-writer-wins, mirroring `src/heartbeat.rs`'s `CURRENT_OP` — this project's
/// only other global mutable state, which chose those semantics deliberately.
///
/// The value is per-**server**, not per-process, and the difference is observable:
/// `CodeScoutServer::from_parts_with_env` has three test-helper callers
/// (`src/server.rs:1686`, `:3713`, `:4084`), each building a server with its own
/// catalog many times per test binary. A first-writer-wins `OnceLock` pinned
/// whichever ran first and silently discarded every later install, so a safety
/// guard's behaviour depended on test ordering. Production is unaffected either
/// way — one server, built at `src/server.rs:1510` (stdio) or `:1571` (HTTP, which
/// builds once and clones per session). F-51 in `docs/trackers/bug-fix-session-log.md`.
static ORACLE: OracleSlot = std::sync::RwLock::new(None);

/// Install the process-wide oracle. Called once, when the librarian runtime is
/// built; later calls are ignored. Left unset (tests, `--no-default-features`)
/// the guard degrades to its frontmatter check rather than failing open loudly.
pub fn install_augmented_oracle(oracle: std::sync::Arc<dyn AugmentedArtifactOracle>) {
    install_into(&ORACLE, oracle);
}

/// Write an oracle into `slot`. Split out from [`install_augmented_oracle`] so the
/// replacement semantics can be tested against a caller-owned slot — no test ever
/// mutates the process-wide `ORACLE`, so no test can perturb another.
fn install_into(slot: &OracleSlot, oracle: std::sync::Arc<dyn AugmentedArtifactOracle>) {
    if let Ok(mut current) = slot.write() {
        *current = Some(oracle);
    }
}

/// Read the oracle out of `slot`, cloning the `Arc` so the lock is released before
/// the caller uses it. Load-bearing: `is_augmented` takes the catalog lock, and
/// holding this one across that call would nest two locks for no reason.
fn read_from(slot: &OracleSlot) -> Option<std::sync::Arc<dyn AugmentedArtifactOracle>> {
    slot.read().ok().and_then(|current| current.clone())
}

fn oracle() -> Option<std::sync::Arc<dyn AugmentedArtifactOracle>> {
    read_from(&ORACLE)
}

/// The testable core: same decision, with the oracle passed explicitly so a test
/// never has to install into the process-wide `OnceLock`.
fn guard_with_oracle(
    path: &str,
    text: &str,
    abs_path: Option<&std::path::Path>,
    oracle: Option<&dyn AugmentedArtifactOracle>,
) -> Result<(), anyhow::Error> {
    // Three independent reasons a file is off-limits, and none implies another: a
    // stamped frontmatter id says the librarian wrote this file; augmentation says
    // the file is not where its state lives; a declared `entry_prefix` says the file
    // owns an id namespace whose counter only the server may advance. An augmented
    // tracker can carry no id at all, a plain bug file carries one without being
    // augmented, and a prose ledger is routinely neither.
    let augmented = matches!((abs_path, oracle), (Some(p), Some(o)) if o.is_augmented(p));
    let stamped = is_librarian_artifact(text);
    let ledger = !declared_entry_prefixes(text).is_empty();
    if !augmented && !ledger && !stamped {
        return Ok(());
    }
    let why = if augmented {
        " (augmented — its params live in the catalog, and this file is only a \
         rendered snapshot of them)"
    } else if ledger {
        " (a ledger — it declares an entry_prefix, and its PREFIX-N ids are \
         allocated by the server)"
    } else {
        ""
    };
    // A ledger that is NEITHER augmented nor stamped is the class this guard newly
    // covers, and it is the only one whose file is still where its state lives — so
    // its refusal must route two different intents, not one. An entry goes through
    // the allocator; anything else (prose, a typo, a heading) is an ordinary edit
    // that `body_edits` performs section-scoped. Naming only the first would send
    // every prose edit to the wrong tool, and a guard whose hint cannot be followed
    // is what trains callers to route around it.
    let hint = if ledger && !augmented && !stamped {
        "This file is a ledger — it owns a PREFIX-N id namespace.\n\
         • Add an entry:  artifact(action=\"append_entry\", id=\"<id>\", id_prefix=\"<PREFIX>\")\n\
         \x20 then write the section yourself with the id it returns.\n\
         • Edit anything else (prose, a heading, a typo):\n\
         \x20 artifact(action=\"update\", id=\"<id>\", patch={body_edits: [{heading: \"## X\", \
         action: \"edit\", old_string: \"...\", new_string: \"...\"}]})"
            .to_string()
    } else {
        "Use artifact tools instead:\n\
         • Read:   artifact(action=\"get\", id=\"<id>\")\n\
         • Find:   artifact(action=\"find\", semantic=\"<topic>\")\n\
         • Edit:   artifact(action=\"update\", id=\"<id>\", patch={...})\n\
         Full guide: resources/read doc://librarian-guide"
            .to_string()
    };
    Err(RecoverableError::with_hint(
        format!("'{path}' is a librarian-managed artifact{why} — do not read or edit it directly"),
        hint,
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

/// The id namespaces a file's frontmatter declares via `entry_prefix`, or empty
/// when it declares none.
///
/// A **ledger** — an artifact owning a `PREFIX-N` id namespace — is the third
/// reason a markdown file is librarian-managed, and it is orthogonal to the other
/// two. It carries no stamped `id:` (the catalog derives ids from the path and does
/// not need one) and needs no augmentation: `allocate_entry_id`'s own error hint
/// tells authors "No augmentation and no entry_collection are needed". So the
/// documented way to create a ledger produced a file both existing predicates were
/// blind to, and its entry headings could be hand-written straight past the
/// allocator — which is how the R-N ledger came to reuse nine ids.
///
/// **Hand-parsed, not `serde_yml`, on purpose.** `librarian` is a Cargo feature
/// (`src/lib.rs:32`), and this guard compiles and must keep working under
/// `--no-default-features`, where `crate::librarian::frontmatter` does not exist.
/// That is also why [`is_librarian_artifact`] hand-parses `id:`.
///
/// Accepts all three YAML forms `allocate_entry_id` honours
/// (`src/librarian/catalog/augmentation.rs`) — scalar, inline flow, and block
/// sequence — because quoting and flow style are properties of whichever writer last
/// emitted the file, never of the artifact. Making protection depend on that accident
/// is exactly the quoted-id defect (BL-33). That agreement is held by
/// `both_entry_prefix_readers_agree_on_every_yaml_form`, beside the allocator: two
/// readers are forced by the feature boundary, so the parity is enforced by a test
/// rather than by this sentence.
pub(crate) fn declared_entry_prefixes(text: &str) -> Vec<String> {
    let Some(rest) = text.strip_prefix("---\n") else {
        return Vec::new();
    };
    let mut lines = rest.lines();
    while let Some(line) = lines.next() {
        if line == "---" {
            return Vec::new();
        }
        let Some(val) = line.strip_prefix("entry_prefix:") else {
            continue;
        };
        let val = val.trim();
        // Inline flow: `entry_prefix: [F, W]`.
        if let Some(inner) = val.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
            return inner.split(',').filter_map(clean_prefix).collect();
        }
        // Scalar: `entry_prefix: R`.
        if !val.is_empty() {
            return clean_prefix(val).into_iter().collect();
        }
        // Block sequence: the key's value is the indented `- F` lines that follow.
        // Stops at the first line that is not one, so a sibling key cannot be
        // swallowed as a list item.
        let mut out = Vec::new();
        for next in lines {
            if next == "---" {
                break;
            }
            let t = next.trim_start();
            let Some(item) = t.strip_prefix("- ") else {
                break;
            };
            if next.len() == t.len() {
                // Not indented — a top-level sequence, so this key is not its parent.
                break;
            }
            out.extend(clean_prefix(item));
        }
        return out;
    }
    Vec::new()
}

/// One declared prefix, unquoted and validated. `None` for anything that is not a
/// usable namespace — empty after trimming, or carrying characters the entry-token
/// grammar (`\b[A-Z]{1,3}-\d+\b`) cannot represent.
///
/// The validation is load-bearing rather than defensive: an empty prefix would make
/// every numbered heading in the file read as an entry.
fn clean_prefix(raw: &str) -> Option<String> {
    let p = strip_matching_quotes(raw.trim()).trim();
    let ok = !p.is_empty() && p.len() <= 3 && p.bytes().all(|b| b.is_ascii_uppercase());
    ok.then(|| p.to_string())
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
pub(crate) fn is_librarian_id(val: &str) -> bool {
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

    /// F-51 / `docs/trackers/bug-fix-session-log.md`.
    ///
    /// The oracle is per-**server**, not per-process. `OnceLock` made it the latter:
    /// `install` discarded every call after the first (`let _ = ORACLE.set(…)`), so
    /// in a test binary — where `from_parts_with_env` builds a fresh server with its
    /// own catalog at three call sites — whichever ran first pinned its catalog for
    /// the whole run, and the discarded `Err` meant nothing surfaced it.
    ///
    /// `heartbeat.rs`'s `CURRENT_OP` had already chosen last-writer-wins for this
    /// project's only other global, with the reasoning written beside it. This test
    /// pins the same semantics here.
    ///
    /// Runs against a caller-owned slot, never the process-wide `ORACLE`, so it
    /// cannot perturb a concurrent test — the same discipline as
    /// `guard_with_oracle` taking its oracle explicitly.
    #[test]
    fn installing_an_oracle_replaces_the_one_before_it() {
        use std::sync::Arc;

        struct Tagged(&'static str);
        impl AugmentedArtifactOracle for Tagged {
            fn is_augmented(&self, p: &std::path::Path) -> bool {
                p.ends_with(self.0)
            }
        }

        let slot: OracleSlot = std::sync::RwLock::new(None);
        let first: Arc<dyn AugmentedArtifactOracle> = Arc::new(Tagged("first.md"));
        let second: Arc<dyn AugmentedArtifactOracle> = Arc::new(Tagged("second.md"));

        install_into(&slot, Arc::clone(&first));
        assert!(
            Arc::ptr_eq(&read_from(&slot).expect("installed"), &first),
            "the first install must land"
        );

        install_into(&slot, Arc::clone(&second));
        assert!(
            Arc::ptr_eq(&read_from(&slot).expect("still installed"), &second),
            "a later install must REPLACE the earlier one — a discarded second install \
             makes the guard's behaviour depend on which server was built first"
        );
    }

    /// A prose ledger declares its id namespace in frontmatter and needs no
    /// augmentation and no stamped id — `allocate_entry_id`'s own error hint says
    /// exactly that ("No augmentation and no entry_collection are needed"). So the
    /// documented way to create a ledger produced a file both existing predicates
    /// were blind to, and its `PREFIX-N` headings could be hand-written straight
    /// past the allocator.
    ///
    /// Verified before this test existed: a `entry_prefix: ZZ` file with
    /// `entry_high_water_ZZ: 3` accepted an `## ZZ-4` heading via `edit_markdown`
    /// and left the mark at 3, which is the input compaction later reads back to
    /// reissue `ZZ-4`.
    /// docs/issues/archive/2026-08-17-librarian-guard-blind-to-artifacts-with-no-frontmatter-id.md
    #[test]
    fn a_declared_ledger_is_guarded_with_no_id_and_no_augmentation() {
        struct NothingIsAugmented;
        impl AugmentedArtifactOracle for NothingIsAugmented {
            fn is_augmented(&self, _: &std::path::Path) -> bool {
                false
            }
        }

        let text = "---\nkind: tracker\nstatus: active\nentry_prefix: ZZ\nentry_high_water_ZZ: 3\n---\n\n## ZZ-3 — an entry\n";
        assert!(
            !is_librarian_artifact(text),
            "precondition: no stamped id for the text predicate to find"
        );

        let abs = std::path::Path::new("/repo/docs/trackers/probe-ledger.md");
        let err = guard_with_oracle(
            "docs/trackers/probe-ledger.md",
            text,
            Some(abs),
            Some(&NothingIsAugmented),
        )
        .expect_err("a declared ledger must be guarded");
        let re = err.downcast_ref::<RecoverableError>().unwrap();
        assert!(
            re.message.contains("librarian-managed artifact"),
            "got: {}",
            re.message
        );
    }

    /// A session log legitimately owns two namespaces (F-N frictions, W-N wins), and
    /// YAML serialises one key three different ways depending only on which writer
    /// last touched the file. `allocate_entry_id` honours all three
    /// (`src/librarian/catalog/augmentation.rs:798-802`); a guard that honoured
    /// fewer would make protection depend on that formatting accident — the same
    /// mistake the quoted-id bug made (BL-33).
    #[test]
    fn every_yaml_form_of_entry_prefix_is_recognised() {
        for (label, text, want) in [
            (
                "scalar",
                "---\nkind: tracker\nentry_prefix: R\n---\n\n# L\n",
                vec!["R"],
            ),
            (
                "block sequence",
                "---\nkind: tracker\nentry_prefix:\n  - F\n  - W\n---\n\n# L\n",
                vec!["F", "W"],
            ),
            (
                "inline flow",
                "---\nkind: tracker\nentry_prefix: [F, W]\n---\n\n# L\n",
                vec!["F", "W"],
            ),
            (
                "quoted scalar",
                "---\nkind: tracker\nentry_prefix: 'HY'\n---\n\n# L\n",
                vec!["HY"],
            ),
        ] {
            assert_eq!(
                declared_entry_prefixes(text),
                want.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                "{label}: entry_prefix must parse whatever YAML form it was written in"
            );
        }
    }

    /// The declaration is a FRONTMATTER fact. A tracker that merely discusses
    /// `entry_prefix` in its prose — every convention doc in `docs/` does — owns no
    /// namespace, and inferring one from body text would make every such doc a
    /// ledger. Same boundary `allocate_entry_id` draws by scanning `fm.extra`
    /// rather than the document.
    #[test]
    fn entry_prefix_outside_frontmatter_declares_nothing() {
        for (label, text) in [
            (
                "after the closing delimiter",
                "---\nkind: tracker\n---\n\n# Guide\n\nDeclare it with `entry_prefix: R`.\n",
            ),
            (
                "no frontmatter block at all",
                "# Guide\n\nentry_prefix: R\n",
            ),
        ] {
            assert!(
                declared_entry_prefixes(text).is_empty(),
                "{label}: only a frontmatter key declares a ledger"
            );
        }
    }

    /// An empty or valueless declaration is not a namespace. Returning a prefix here
    /// would let the entry-heading regex match every numbered heading in the file.
    #[test]
    fn a_valueless_entry_prefix_declares_nothing() {
        for (label, text) in [
            (
                "bare key",
                "---\nkind: tracker\nentry_prefix:\n---\n\n# L\n",
            ),
            (
                "empty string",
                "---\nkind: tracker\nentry_prefix: ''\n---\n\n# L\n",
            ),
            (
                "empty flow list",
                "---\nkind: tracker\nentry_prefix: []\n---\n\n# L\n",
            ),
        ] {
            assert!(
                declared_entry_prefixes(text).is_empty(),
                "{label}: a valueless declaration owns no namespace"
            );
        }
    }

    /// A ledger's refusal is the only thing routing an author who wanted to edit its
    /// prose — the heading-scoped guard the bug proposed was cut, because `body_edits`
    /// already performs a section-scoped swap on any catalog row (measured on
    /// `skill-frictions.md`, a row with no `id:` and no augmentation). So both routes
    /// must appear: `append_entry` for a new entry, `body_edits` for everything else.
    /// A hint naming only one of them is the failure mode being avoided, not a
    /// smaller version of it.
    #[test]
    fn a_ledger_refusal_names_both_the_entry_route_and_the_prose_route() {
        struct NothingIsAugmented;
        impl AugmentedArtifactOracle for NothingIsAugmented {
            fn is_augmented(&self, _: &std::path::Path) -> bool {
                false
            }
        }

        let text = "---\nkind: tracker\nentry_prefix: R\n---\n\n## Prose\n";
        let abs = std::path::Path::new("/repo/docs/trackers/recon.md");
        let err = guard_with_oracle(
            "docs/trackers/recon.md",
            text,
            Some(abs),
            Some(&NothingIsAugmented),
        )
        .expect_err("a ledger must be guarded");
        let re = err.downcast_ref::<RecoverableError>().unwrap();
        let hint = re.hint().unwrap_or_default();
        assert!(
            hint.contains("append_entry"),
            "the entry route must be named: {hint}"
        );
        assert!(
            hint.contains("body_edits"),
            "the prose route must be named: {hint}"
        );
    }

    /// The widened hint belongs to the ledger arm alone. An augmented artifact's file
    /// is a rendered snapshot of catalog params, so `body_edits` is the ONLY correct
    /// route there and offering `append_entry` beside it would be wrong; a stamped id
    /// says the librarian wrote the file, which the ledger routing does not describe
    /// either. Both keep the original generic hint.
    #[test]
    fn the_ledger_hint_does_not_leak_into_the_augmented_or_stamped_arms() {
        struct EverythingIsAugmented;
        impl AugmentedArtifactOracle for EverythingIsAugmented {
            fn is_augmented(&self, _: &std::path::Path) -> bool {
                true
            }
        }
        struct NothingIsAugmented;
        impl AugmentedArtifactOracle for NothingIsAugmented {
            fn is_augmented(&self, _: &std::path::Path) -> bool {
                false
            }
        }

        let abs = std::path::Path::new("/repo/docs/trackers/t.md");

        // Augmented AND a declared ledger — augmentation wins, generic hint.
        let augmented_ledger = "---\nkind: tracker\nentry_prefix: R\n---\n\n## Prose\n";
        let err = guard_with_oracle(
            "docs/trackers/t.md",
            augmented_ledger,
            Some(abs),
            Some(&EverythingIsAugmented),
        )
        .expect_err("an augmented artifact must be guarded");
        let hint = err
            .downcast_ref::<RecoverableError>()
            .unwrap()
            .hint()
            .unwrap_or_default()
            .to_string();
        assert!(
            !hint.contains("append_entry"),
            "an augmented file's params live in the catalog — append_entry is not its \
             route: {hint}"
        );

        // Stamped id, not a ledger — generic hint.
        let stamped = "---\nkind: bug\nid: 0123456789abcdef\n---\n\n## Summary\n";
        let err = guard_with_oracle(
            "docs/issues/b.md",
            stamped,
            Some(abs),
            Some(&NothingIsAugmented),
        )
        .expect_err("a stamped artifact must be guarded");
        let hint = err
            .downcast_ref::<RecoverableError>()
            .unwrap()
            .hint()
            .unwrap_or_default()
            .to_string();
        assert!(
            !hint.contains("append_entry"),
            "a stamped non-ledger owns no id namespace: {hint}"
        );
    }
}
