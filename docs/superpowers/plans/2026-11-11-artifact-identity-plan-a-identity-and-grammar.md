# Artifact Identity Plan A — Identity and Grammar Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the pure core of the new identity model: the repo name and its onboarding confirmation, the document key, the closed citation grammar, and a key index that resolves citations, all tested, with no change to the live catalog.

**Architecture:** Plan A of five (A identity and grammar core, B derived catalog, C writes go to files, D removal, E migration window). Everything here is new code beside the old system (strangler): no existing catalog table, indexer path or `link_scan` behaviour changes. Two live surfaces change: the `librarian` cargo feature is removed, so the librarian is always compiled (operator decision 2026-11-11: the librarian is an integral part of codescout, on even when no semantic search is available), and `onboarding` gains the repo-name confirmation. Plans B and C, and the parallel `codescout-37` ledger work, build on the interfaces below.

**Tech Stack:** Rust 2021, `pulldown-cmark` 0.13.3 with `Options::ENABLE_WIKILINKS` (verified 2026-11-11 by a scratch probe: `[[repo/key:R-91|label]]` arrives as `Tag::Link { link_type: LinkType::WikiLink { has_pothole: true }, dest_url: "repo/key:R-91" }`, inline code arrives as `Event::Code`, fenced text as `Event::Text` inside `Tag::CodeBlock`), `toml`, `serde`, `rmcp` elicitation (`ctx.elicit::<T>(prompt) -> Result<Option<T>>`, `rmcp::elicit_safe!`).

**Spec:** `docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md` (approved v5, `9531150ac`). Read § Identity: the key and § The citation grammar before starting.

## Global Constraints

- Key and repo-name charset: `^[a-z0-9]+(-[a-z0-9]+)*$`. A value of exactly 16 characters of `[0-9a-f]` is refused (it would read as a legacy id).
- Entry token grammar: `^[A-Z]{1,3}-[0-9]+$` (unchanged from today).
- The key is the filename stem in lowercase. The folder-index filename is `readme.md`, matched case-insensitively; a folder index takes its parent folder's name; a root-level readme has the key `readme`.
- Grammar forms: `[[key]]`, `[[key:TOKEN]]`, `[[repo/key]]`, `[[repo/key:TOKEN]]`, optionally followed by `|label`. A bare `TOKEN` resolves only inside its own document.
- Never parsed for citations: YAML frontmatter, fenced and indented code blocks, inline code, the label text of a wiki link, headings.
- Repo-name file: `.codescout/repo.toml`, one key `name`. Its serde struct must **not** use `deny_unknown_fields` (older binaries must survive a future field; see `src/librarian/requirements.rs:8-12` for the incident this avoids).
- The librarian is an integral part of codescout. Task 1 removes the `librarian` cargo feature; no code in this plan carries `#[cfg(feature = "librarian")]`.
- The librarian works without semantic search (no embedder configured): nothing in this plan depends on embeddings.
- No existing catalog schema, indexer, `link_scan`, `doctor` or `doc` behaviour changes in this plan.
- Before completing any task: `./scripts/gate.sh` green (CLAUDE.md § Development Commands). Targeted runs: `scripts/with-slot.sh cargo test --lib <path>`.
- Commit by pathspec with `git commit -F <msgfile> -- <paths>`, following `docs/conventions/shared-checkout-commit-sequence.md`; never a bare `git commit`.

## Review Focus

1. A filename with non-ASCII letters, spaces or dots (`Ünïcode Notes.md`, `v1.2-notes.md`) — expected: `key_of` returns `KeyError::NonCanonical`, never panics, never invents a key. Test in Task 4.
2. A rel path with Windows separators (`docs\trackers\frictions\README.md`) — expected: the same key as the forward-slash form (`frictions`). Test in Task 4.
3. Whitespace inside a wiki link (`[[ tracker-hygiene-log ]]`) — expected: trimmed and parsed as `[[tracker-hygiene-log]]`; inner whitespace (`[[two words]]`) is `Malformed`. Test in Task 5.
4. A document with an unclosed fence — expected: no panic; citations before the fence are found, text after it is treated as code (CommonMark). Test in Task 5.
5. A malformed `.codescout/repo.toml` (bad TOML, or an invalid `name`) — expected: `read` returns an error naming the file, and onboarding reports it instead of overwriting the file. Tests in Tasks 2 and 3.

---

### Task 1: The librarian is unconditional

**Files:**
- Modify: `Cargo.toml` — remove the `librarian` feature; make its dependencies (`serde_yml`, `dirs`, `pulldown-cmark`, `ulid`, `minijinja`, `jsonschema`) non-optional; drop `"librarian"` from `default`.
- Modify: the 23 files that carry `feature = "librarian"` (112 occurrences, measured 2026-11-11), led by `src/server.rs` (51), `tests/doc_tool_refs.rs` (14), `src/main.rs` (10) and `src/cli/mod.rs` (9).
- Modify: `scripts/gate.sh` (comments at L15, L233-234, L294 saying the librarian is off in the lean lane) and `.github/workflows/ci.yml` (L383-384, L419-423).

**Interfaces:**
- Consumes: nothing.
- Produces: `crate::librarian` exists in every build configuration; any module may use it.

Out of scope: the runtime opt-out (`LIBRARIAN_ENABLED=0`, `[librarian] enabled = false`; 5 references in `src/`) is unchanged.

- [ ] **Step 1: Record the starting count**

Run: `grep -rn 'feature *= *"librarian"' --include=*.rs src tests crates | wc -l`
Expected: `112`.

- [ ] **Step 2: Remove the feature.** `#[cfg(feature = "librarian")]` keeps its item and loses the attribute. `#[cfg(not(feature = "librarian"))]` deletes its item. In a compound, `all(feature = "librarian", X)` becomes `X`, and `any(feature = "librarian", X)` is always true, so the attribute goes. `cfg!(feature = "librarian")` becomes `true`, and the dead branch goes.

- [ ] **Step 3: Verify**

Run: the Step 1 grep. Expected: `0`.
Run: `scripts/with-slot.sh cargo clippy --workspace --all-targets --no-default-features -- -D warnings`. Expected: clean. The lean lane's dead-code errors that `scripts/gate.sh:233-234` describes must be gone, not allowed.
Run: `scripts/with-slot.sh cargo test --no-default-features --lib librarian::`. Expected: the librarian's unit tests run (a non-zero count) and pass.

- [ ] **Step 4: Run `./scripts/gate.sh`.** Expected: green; the lean lane now runs the librarian tests too.

- [ ] **Step 5: Commit** — pathspec commit of every touched file: `refactor(build): the librarian is always compiled; remove the librarian feature`.

---

### Task 2: Repo name — the type, the file, the suggestion

**Files:**
- Create: `src/librarian/identity/mod.rs` (`pub mod repo_name;`; later tasks add `key`, `grammar`, `index`), `src/librarian/identity/repo_name.rs`
- Modify: `src/librarian/mod.rs` (add `pub mod identity;` beside `pub mod entry_token;`)
- Test: `src/librarian/identity/repo_name.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub const REPO_FILE_REL: &str = ".codescout/repo.toml";`
  - `pub fn is_canonical_name(s: &str) -> bool` — the charset rule.
  - `pub fn is_hex16(s: &str) -> bool` — exactly 16 of `[0-9a-f]`.
  - `pub struct RepoName(String)` with `pub fn parse(s: &str) -> Result<RepoName, NameError>`, `pub fn as_str(&self) -> &str`, `impl Display`.
  - `pub enum NameError { NonCanonical(String), HexShaped(String) }` with `impl Display` naming the rule.
  - `pub fn canonicalize(s: &str) -> Option<String>` — lowercase; every run of characters outside `[a-z0-9]` becomes one `-`; trim `-`; `None` when empty or hex16.
  - `pub fn read(root: &Path) -> anyhow::Result<Option<RepoName>>` — `Ok(None)` when the file is absent.
  - `pub fn write(root: &Path, name: &RepoName) -> anyhow::Result<PathBuf>` — creates `.codescout/` if needed; writes `name = "<name>"\n`; returns the file path.
  - `pub fn suggest(root: &Path) -> RepoName` — `[workspace] name` from `.codescout/workspace.toml` through `canonicalize`, else the root directory's basename through `canonicalize`, else `repo`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn canonical_names_are_accepted_and_others_refused() {
    assert!(RepoName::parse("codescout").is_ok());
    assert!(RepoName::parse("claude-plugins").is_ok());
    assert!(RepoName::parse("mrv-poc2").is_ok());
    for bad in ["", "Codescout", "claude_plugins", "-x", "x-", "a--b", "two words", "ü"] {
        assert!(matches!(RepoName::parse(bad), Err(NameError::NonCanonical(_))), "{bad}");
    }
    assert!(matches!(RepoName::parse("deadbeefcafe1234"), Err(NameError::HexShaped(_))));
    assert!(RepoName::parse("deadbeefcafe12345").is_ok()); // 17 chars is not id-shaped
}

#[test]
fn canonicalize_folds_case_and_separators() {
    assert_eq!(canonicalize("My_Repo.v2").as_deref(), Some("my-repo-v2"));
    assert_eq!(canonicalize("MRV-poc").as_deref(), Some("mrv-poc"));
    assert_eq!(canonicalize("__"), None);
    assert_eq!(canonicalize("DEADBEEFCAFE1234"), None);
}

#[test]
fn read_is_none_when_absent_and_round_trips_after_write() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(read(dir.path()).unwrap(), None);
    let p = write(dir.path(), &RepoName::parse("codescout").unwrap()).unwrap();
    assert_eq!(p, dir.path().join(REPO_FILE_REL));
    assert_eq!(read(dir.path()).unwrap().unwrap().as_str(), "codescout");
}

#[test]
fn an_unknown_field_does_not_break_read() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    std::fs::write(dir.path().join(REPO_FILE_REL), "name = \"codescout\"\nfuture = 1\n").unwrap();
    assert_eq!(read(dir.path()).unwrap().unwrap().as_str(), "codescout");
}

#[test]
fn a_malformed_file_is_an_error_naming_the_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    std::fs::write(dir.path().join(REPO_FILE_REL), "name = \"Bad Name\"\n").unwrap();
    let err = format!("{:#}", read(dir.path()).unwrap_err());
    assert!(err.contains("repo.toml"), "{err}");
    std::fs::write(dir.path().join(REPO_FILE_REL), "name = \n").unwrap();
    assert!(format!("{:#}", read(dir.path()).unwrap_err()).contains("repo.toml"));
}

#[test]
fn suggest_prefers_the_workspace_name_then_the_basename() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("My_Project");
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    assert_eq!(suggest(&root).as_str(), "my-project");
    std::fs::write(root.join(".codescout/workspace.toml"), "[workspace]\nname = \"Code Scout\"\n").unwrap();
    assert_eq!(suggest(&root).as_str(), "code-scout");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/with-slot.sh cargo test --lib librarian::identity::repo_name`
Expected: FAIL to compile (`identity` module not found).

- [ ] **Step 3: Implement `src/librarian/identity/repo_name.rs`** with the interfaces above. Parse the file with a `#[derive(Deserialize)] struct RepoFile { name: String }` **without** `deny_unknown_fields`, then `RepoName::parse`; wrap every error with `.with_context(|| format!("reading {}", path.display()))`. Read `workspace.toml` as `toml::Value` and take `workspace.name` as a string; ignore a missing or unreadable file.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/with-slot.sh cargo test --lib librarian::identity::repo_name`
Expected: PASS, 6 tests.

- [ ] **Step 5: Commit**

`git add -- src/librarian/identity/mod.rs src/librarian/identity/repo_name.rs src/librarian/mod.rs` and commit by pathspec: `feat(librarian): repo name in .codescout/repo.toml, with a canonical suggestion`.

---

### Task 3: Onboarding confirms the repo name

**Files:**
- Modify: `src/tools/onboarding.rs` (`input_schema` L456-470, `call` L471-508; new private items below)
- Test: `src/tools/onboarding.rs` tests module (or the file's existing test location)

**Interfaces:**
- Consumes: Task 1 (the librarian is always compiled), Task 2 (`crate::librarian::identity::repo_name::{read, write, suggest, RepoName, NameError}`).
- Produces:
  - Input parameter `repo_name: string` on `onboarding`: "Confirm the repo name (lowercase letters, digits and single hyphens). Writes .codescout/repo.toml."
  - Every `onboarding` response carries a `repo_name` object, one of:
    - `{"name": "<n>", "confirmed": true}`
    - `{"name": "<n>", "confirmed": true, "written": "<path>"}` (just written)
    - `{"suggested": "<s>", "confirmed": false, "instruction": "Ask the user to confirm or edit the repo name `<s>`, then call onboarding(repo_name=\"<name>\")."}`
    - `{"error": "<message naming .codescout/repo.toml>", "confirmed": false}` (file unreadable; never overwritten)
  - `fn repo_name_outcome(existing: anyhow::Result<Option<RepoName>>, elicited: Option<String>, suggestion: &RepoName) -> RepoNameOutcome` — pure decision; `enum RepoNameOutcome { Confirmed(RepoName), Write(RepoName), Unconfirmed(RepoName), Unreadable(String), Invalid { input: String, error: NameError } }`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn outcome_keeps_a_confirmed_name_and_never_elicits_over_it() {
    let s = RepoName::parse("suggested").unwrap();
    let have = RepoName::parse("codescout").unwrap();
    assert!(matches!(repo_name_outcome(Ok(Some(have)), Some("other".into()), &s),
        RepoNameOutcome::Confirmed(n) if n.as_str() == "codescout"));
}

#[test]
fn outcome_writes_an_elicited_name_and_reports_an_invalid_one() {
    let s = RepoName::parse("suggested").unwrap();
    assert!(matches!(repo_name_outcome(Ok(None), Some(" codescout ".into()), &s),
        RepoNameOutcome::Write(n) if n.as_str() == "codescout"));
    assert!(matches!(repo_name_outcome(Ok(None), Some("Code Scout".into()), &s),
        RepoNameOutcome::Invalid { .. }));
}

#[test]
fn outcome_is_unconfirmed_without_elicitation_or_with_an_empty_answer() {
    let s = RepoName::parse("suggested").unwrap();
    assert!(matches!(repo_name_outcome(Ok(None), None, &s), RepoNameOutcome::Unconfirmed(n) if n.as_str() == "suggested"));
    assert!(matches!(repo_name_outcome(Ok(None), Some("  ".into()), &s), RepoNameOutcome::Unconfirmed(_)));
}

#[test]
fn outcome_never_overwrites_an_unreadable_file() {
    let s = RepoName::parse("suggested").unwrap();
    let out = repo_name_outcome(Err(anyhow::anyhow!("reading .codescout/repo.toml: bad")), Some("x".into()), &s);
    assert!(matches!(out, RepoNameOutcome::Unreadable(m) if m.contains("repo.toml")));
}
```

Plus one tool-level test in the style of the file's existing tests: calling `onboarding` with `{"repo_name": "Bad Name"}` returns a `RecoverableError` whose message contains `repo.toml` and whose hint states the charset; calling it with `{"repo_name": "codescout"}` on a temp project writes `.codescout/repo.toml` and the response has `repo_name.confirmed == true`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `scripts/with-slot.sh cargo test --lib tools::onboarding`
Expected: FAIL to compile (`repo_name_outcome` not found).

- [ ] **Step 3: Implement**
  - Add `repo_name` to `input_schema`.
  - In `call`, before any other branch: if `input["repo_name"]` is a string, `RepoName::parse` it (on error return `RecoverableError::with_hint("repo name <x> is not canonical: <error> (.codescout/repo.toml not written)", "Use lowercase letters, digits and single hyphens, e.g. claude-plugins.")`), `write` it, and return `{"repo_name": {"name", "confirmed": true, "written"}}`.
  - Otherwise compute the branch result as today, then set `result["repo_name"]` from `async fn repo_name_block(ctx, root) -> Value`. It reads the file; when it is absent, it tries `ctx.elicit::<RepoNameInput>(prompt)` with `RepoNameInput { name: String }` and `rmcp::elicit_safe!(RepoNameInput)`. The prompt names the suggestion, the rule, and that the name labels this repo in citations (`[[repo/key]]`). An elicitation error counts as `None`. Then it maps `repo_name_outcome` to the JSON shapes above, writing on `Write`.
  - The outcome `Invalid` maps to the unconfirmed shape plus `"error"`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `scripts/with-slot.sh cargo test --lib tools::onboarding`
Expected: PASS, including the existing onboarding tests unchanged.

- [ ] **Step 5: Commit**

Pathspec commit of `src/tools/onboarding.rs`: `feat(onboarding): confirm the repo name with the user, via elicitation or a parameter`.

---

### Task 4: The document key

**Files:**
- Create: `src/librarian/identity/key.rs`
- Modify: `src/librarian/identity/mod.rs` (add `pub mod key;`)
- Test: `src/librarian/identity/key.rs`

**Interfaces:**
- Consumes: `super::repo_name::{is_canonical_name, is_hex16}` (Task 2).
- Produces:
  - `pub struct Key(String)` — `Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord`; `as_str()`, `Display`; `pub fn parse(s: &str) -> Result<Key, KeyError>` (charset + hex16 only).
  - `pub enum KeyError { NotMarkdown(String), NonCanonical { rel_path: String, key: String }, HexShaped { rel_path: String, key: String } }`.
  - `pub fn key_of(rel_path: &str) -> Result<Key, KeyError>` — normalizes `\` to `/`; requires a `.md` extension (case-insensitive); stem lowercased; a stem equal to `readme` takes the lowercased parent folder name, or `readme` at the root.
  - `pub fn is_folder_index(rel_path: &str) -> bool` — file name is `readme.md`, case-insensitive.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn the_key_is_the_lowercase_stem() {
    assert_eq!(key_of("docs/trackers/tracker-hygiene-log.md").unwrap().as_str(), "tracker-hygiene-log");
    assert_eq!(key_of("CLAUDE.md").unwrap().as_str(), "claude");
    assert_eq!(key_of("docs/issues/2026-11-11-reindex-deletes.md").unwrap().as_str(), "2026-11-11-reindex-deletes");
}

#[test]
fn a_readme_takes_its_folder_name() {
    assert_eq!(key_of("docs/trackers/ledgers/frictions/readme.md").unwrap().as_str(), "frictions");
    assert_eq!(key_of("docs/research/README.md").unwrap().as_str(), "research");
    assert_eq!(key_of("README.md").unwrap().as_str(), "readme");
    assert_eq!(key_of(r"docs\trackers\frictions\README.md").unwrap().as_str(), "frictions");
    assert!(is_folder_index("x/ReadMe.md"));
    assert!(!is_folder_index("x/readme-notes.md"));
}

#[test]
fn non_canonical_and_hex_shaped_keys_are_errors_not_guesses() {
    assert!(matches!(key_of("docs/Ünïcode Notes.md"), Err(KeyError::NonCanonical { .. })));
    assert!(matches!(key_of("docs/v1.2-notes.md"), Err(KeyError::NonCanonical { .. })));
    assert!(matches!(key_of("docs/issue_clusters.md"), Err(KeyError::NonCanonical { .. })));
    assert!(matches!(key_of("docs/Deadbeefcafe1234.md"), Err(KeyError::HexShaped { .. })));
    assert!(matches!(key_of("docs/a.txt"), Err(KeyError::NotMarkdown(_))));
    assert!(matches!(key_of("docs/Bad_Folder/readme.md"), Err(KeyError::NonCanonical { .. })));
}
```

- [ ] **Step 2: Run to verify failure** — `scripts/with-slot.sh cargo test --lib librarian::identity::key` → FAIL to compile.
- [ ] **Step 3: Implement `key.rs`** with the interfaces above.
- [ ] **Step 4: Run to verify pass** — same command → PASS, 3 tests.
- [ ] **Step 5: Commit** — pathspec commit of `key.rs` and `identity/mod.rs`: `feat(librarian): document key from the lowercase filename stem`.

---

### Task 5: The citation grammar extractor

**Files:**
- Create: `src/librarian/identity/grammar.rs`
- Modify: `src/librarian/identity/mod.rs` (add `pub mod grammar;`)
- Test: `src/librarian/identity/grammar.rs`

**Interfaces:**
- Consumes: `Key::parse` (Task 4), `RepoName::parse` (Task 2), `crate::librarian::frontmatter::parse` (frontmatter offset guard, as `link_scan::extract::extract` uses it at `src/librarian/tools/link_scan/extract.rs:481-500`).
- Produces:
  - `pub enum Reference { File { repo: Option<RepoName>, key: Key }, Entry { repo: Option<RepoName>, key: Key, token: String }, Bare { token: String }, Malformed { raw: String } }` — `Clone, Debug, PartialEq, Eq`.
  - `pub struct Citation { pub reference: Reference, pub line: u32 }` (1-indexed line of the first byte).
  - `pub fn parse_target(dest: &str) -> Reference` — trims ASCII whitespace; splits an optional `repo/` prefix (exactly one `/`) and an optional `:TOKEN` suffix (exactly one `:`); validates each part; anything else is `Malformed { raw: dest.to_string() }`.
  - `pub fn extract(text: &str) -> Vec<Citation>` — runs `pulldown_cmark::Parser::new_ext(text, ENABLE_WIKILINKS | ENABLE_TABLES)` with offsets. A `Tag::Link` with `LinkType::WikiLink` gives `parse_target(dest_url)`. `Event::Text` outside code blocks, wiki-link labels, headings and frontmatter is scanned for bare tokens with `\b[A-Z]{1,3}-[0-9]+\b`. `Event::Code` is never scanned. Order: by position.

- [ ] **Step 1: Write the failing tests**

```rust
fn refs(text: &str) -> Vec<Reference> { extract(text).into_iter().map(|c| c.reference).collect() }
fn k(s: &str) -> Key { Key::parse(s).unwrap() }
fn r(s: &str) -> RepoName { RepoName::parse(s).unwrap() }

#[test]
fn the_four_forms_and_a_label() {
    assert_eq!(refs("[[a-doc]] [[a-doc:F-3]] [[other/b-doc]] [[other/b-doc:R-91|see this]]"), vec![
        Reference::File { repo: None, key: k("a-doc") },
        Reference::Entry { repo: None, key: k("a-doc"), token: "F-3".into() },
        Reference::File { repo: Some(r("other")), key: k("b-doc") },
        Reference::Entry { repo: Some(r("other")), key: k("b-doc"), token: "R-91".into() },
    ]);
}

#[test]
fn malformed_targets_are_reported_not_guessed() {
    for raw in ["arXiv", "two words", "a:b:c", "a/b/c", "a:f-3", "deadbeefcafe1234", ""] {
        assert_eq!(parse_target(raw), Reference::Malformed { raw: raw.into() }, "{raw}");
    }
    assert_eq!(parse_target(" tracker-hygiene-log "), Reference::File { repo: None, key: k("tracker-hygiene-log") });
}

#[test]
fn code_frontmatter_headings_and_labels_are_never_citations() {
    let text = "---\nrests_on: \"[[x-doc]]\"\n---\n# Heading F-1 and [[h-doc]]\n\nInline `[[c-doc]]` and `F-2`.\n\n```toml\n[[rule]]\nF-3\n```\n\n    [[indented-doc]] F-4\n\n[[l-doc|label F-5]] and bare F-6.\n";
    assert_eq!(refs(text), vec![
        Reference::File { repo: None, key: k("h-doc") },
        Reference::File { repo: None, key: k("l-doc") },
        Reference::Bare { token: "F-6".into() },
    ]);
}

#[test]
fn bare_tokens_need_word_boundaries_and_lines_are_one_indexed() {
    let cs = extract("intro\n\nsee R-91, not XR-91a or R-9x.\n");
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].reference, Reference::Bare { token: "R-91".into() });
    assert_eq!(cs[0].line, 3);
}

#[test]
fn an_unclosed_fence_hides_what_follows_and_does_not_panic() {
    assert_eq!(refs("[[a-doc]]\n\n```\n[[b-doc]]\n"), vec![Reference::File { repo: None, key: k("a-doc") }]);
}

#[test]
fn a_wiki_link_in_a_table_cell_is_found() {
    assert_eq!(refs("| a | [[t-doc:B-2]] |\n|---|---|\n| x | y |\n"),
        vec![Reference::Entry { repo: None, key: k("t-doc"), token: "B-2".into() }]);
}
```

Note on the heading case: `[[h-doc]]` inside a heading **is** a citation (a heading may link a document); only bare tokens in headings are skipped, because a heading is where a token is defined.

- [ ] **Step 2: Run to verify failure** — `scripts/with-slot.sh cargo test --lib librarian::identity::grammar` → FAIL to compile.
- [ ] **Step 3: Implement `grammar.rs`.** Track `in_code_block`, `in_heading` and `wiki_depth` from Start/End events; skip every event whose span starts before the frontmatter offset; compute lines from a `line_starts` table.
- [ ] **Step 4: Run to verify pass** — same command → PASS, 6 tests.
- [ ] **Step 5: Commit** — pathspec commit: `feat(librarian): closed citation grammar extractor over pulldown-cmark wiki links`.

---

### Task 6: The key index and resolution

**Files:**
- Create: `src/librarian/identity/index.rs`
- Modify: `src/librarian/identity/mod.rs` (add `pub mod index;`)
- Test: `src/librarian/identity/index.rs`

**Interfaces:**
- Consumes: Tasks 2, 4, 5; `crate::librarian::tools::link_scan::extract::entry_sections` (token definitions; reuse it, do not re-implement the definition rule).
- Produces:
  - `pub struct DocInput { pub rel_path: String, pub text: String }`.
  - `pub fn defined_tokens(text: &str) -> Vec<String>` — the `id`s of `entry_sections(text)`.
  - `pub enum IndexProblem { InvalidKey(KeyError), DuplicateKey { key: Key, rel_paths: Vec<String> } }`.
  - `pub struct KeyIndex` with `pub fn build(this_repo: RepoName, docs: impl IntoIterator<Item = DocInput>) -> KeyIndex`, `pub fn problems(&self) -> &[IndexProblem]`, `pub fn rel_path(&self, key: &Key) -> Option<&str>`, `pub fn owner(&self, key: &Key) -> Option<&Key>`, `pub fn resolve(&self, src_rel_path: &str, reference: &Reference) -> Resolution`.
  - `pub enum Resolution { File { dst: Key }, Entry { dst: Key, token: String }, Local { token: String }, NotLocal, UnknownKey { key: Key }, KeyCollision { key: Key, rel_paths: Vec<String> }, UndefinedToken { key: Key, token: String }, AmbiguousToken { key: Key, token: String, candidates: Vec<Key> }, CrossRepo { repo: RepoName }, Malformed { raw: String } }`.
  - Rules: a document with an invalid key is excluded and reported. A key held by two or more documents is reported as `DuplicateKey` and resolves to `KeyCollision`. **Owner**: the key of the nearest ancestor folder that contains a folder-index document; for a folder-index document the search starts at its parent's parent. **Entry** `[[k:T]]`: candidates are documents whose key is `k` or whose owner is `k`, and which define `T`; 0 gives `UndefinedToken`, 1 gives `Entry`, more give `AmbiguousToken`. **Bare** `T`: `Local` if the source document defines `T`, else `NotLocal` (callers ignore `NotLocal`; it is how `UTF-8` stays noise-free). A `repo` equal to `this_repo` resolves locally; any other repo is `CrossRepo`.

- [ ] **Step 1: Write the failing tests**

```rust
fn doc(p: &str, t: &str) -> DocInput { DocInput { rel_path: p.into(), text: t.into() } }
fn k(s: &str) -> Key { Key::parse(s).unwrap() }
fn index() -> KeyIndex {
    KeyIndex::build(RepoName::parse("codescout").unwrap(), [
        doc("docs/trackers/ledgers/readme.md", "# Ledgers\n"),
        doc("docs/trackers/ledgers/frictions/readme.md", "# Frictions\n\n## FR-1 — in the index\n"),
        doc("docs/trackers/ledgers/frictions/2026-11-11-fr-two.md", "## FR-2 — own file\n"),
        doc("docs/trackers/ledgers/frictions/archive/2026-10-01-fr-three.md", "## FR-3 — archived\n"),
        doc("docs/trackers/ledgers/logs/a-log.md", "## F-1 — a\n"),
        doc("docs/trackers/ledgers/logs/b-log.md", "## F-1 — b\n"),
        doc("docs/one/dup.md", ""),
        doc("docs/two/dup.md", ""),
        doc("docs/Bad Name.md", ""),
    ])
}
fn entry(key: &str, token: &str) -> Reference { Reference::Entry { repo: None, key: k(key), token: token.into() } }

#[test]
fn nearest_folder_index_owns_its_descendants_tokens() {
    let ix = index();
    assert_eq!(ix.owner(&k("2026-10-01-fr-three")), Some(&k("frictions")));
    assert_eq!(ix.owner(&k("frictions")), Some(&k("ledgers")));
    let src = "docs/x.md";
    assert_eq!(ix.resolve(src, &entry("frictions", "FR-1")), Resolution::Entry { dst: k("frictions"), token: "FR-1".into() });
    assert_eq!(ix.resolve(src, &entry("frictions", "FR-2")), Resolution::Entry { dst: k("2026-11-11-fr-two"), token: "FR-2".into() });
    assert_eq!(ix.resolve(src, &entry("frictions", "FR-3")), Resolution::Entry { dst: k("2026-10-01-fr-three"), token: "FR-3".into() });
    // `ledgers` is not the NEAREST index of the friction entries, so it owns none of them.
    assert!(matches!(ix.resolve(src, &entry("ledgers", "FR-2")), Resolution::UndefinedToken { .. }));
}

#[test]
fn a_token_defined_twice_under_one_owner_is_ambiguous_not_guessed() {
    let ix = KeyIndex::build(RepoName::parse("codescout").unwrap(), [
        doc("logs/readme.md", ""), doc("logs/a-log.md", "## F-1 — a\n"), doc("logs/b-log.md", "## F-1 — b\n"),
    ]);
    assert!(matches!(ix.resolve("x.md", &entry("logs", "F-1")),
        Resolution::AmbiguousToken { candidates, .. } if candidates.len() == 2));
    assert_eq!(ix.resolve("x.md", &entry("a-log", "F-1")), Resolution::Entry { dst: k("a-log"), token: "F-1".into() });
}

#[test]
fn bare_tokens_are_local_only() {
    let ix = index();
    let src = "docs/trackers/ledgers/logs/a-log.md";
    assert_eq!(ix.resolve(src, &Reference::Bare { token: "F-1".into() }), Resolution::Local { token: "F-1".into() });
    assert_eq!(ix.resolve(src, &Reference::Bare { token: "FR-1".into() }), Resolution::NotLocal);
    assert_eq!(ix.resolve(src, &Reference::Bare { token: "UTF-8".into() }), Resolution::NotLocal);
}

#[test]
fn collisions_invalid_keys_repos_and_unknowns() {
    let ix = index();
    assert!(ix.problems().iter().any(|p| matches!(p, IndexProblem::DuplicateKey { key, rel_paths } if key.as_str() == "dup" && rel_paths.len() == 2)));
    assert!(ix.problems().iter().any(|p| matches!(p, IndexProblem::InvalidKey(_))));
    assert!(matches!(ix.resolve("x.md", &Reference::File { repo: None, key: k("dup") }), Resolution::KeyCollision { .. }));
    assert_eq!(ix.resolve("x.md", &Reference::File { repo: None, key: k("nope") }), Resolution::UnknownKey { key: k("nope") });
    let here = Reference::File { repo: Some(RepoName::parse("codescout").unwrap()), key: k("frictions") };
    assert_eq!(ix.resolve("x.md", &here), Resolution::File { dst: k("frictions") });
    let there = Reference::File { repo: Some(RepoName::parse("claude-plugins").unwrap()), key: k("frictions") };
    assert!(matches!(ix.resolve("x.md", &there), Resolution::CrossRepo { .. }));
}
```

- [ ] **Step 2: Run to verify failure** — `scripts/with-slot.sh cargo test --lib librarian::identity::index` → FAIL to compile.
- [ ] **Step 3: Implement `index.rs`.** Build: compute each document's key, the set of folders that hold a folder index, each document's owner, and a `token -> Vec<Key>` map per document from `defined_tokens`.
- [ ] **Step 4: Run to verify pass** — same command → PASS, 4 tests.
- [ ] **Step 5: Commit** — pathspec commit: `feat(librarian): key index with nearest-index ownership and local bare tokens`.

---

### Task 7: Census of this repo under the new model

**Files:**
- Create: `src/librarian/identity/census.rs` (`#[cfg(test)]` only) — or a test in `index.rs`; one file, one ignored test.

**Interfaces:**
- Consumes: Tasks 4-6.
- Produces: an `#[ignore]`d test `census_of_this_repo_under_the_new_identity_model` that walks `git ls-files '*.md'` from `env!("CARGO_MANIFEST_DIR")`, builds a `KeyIndex` with `repo_name::read` (or `suggest`), extracts every document's citations, and prints: documents; `InvalidKey` count with up to 20 paths; `DuplicateKey` count with every key; grammar citations by `Resolution` variant. It asserts only that the index built and at least one document was read.

- [ ] **Step 1: Write the ignored test** as above.
- [ ] **Step 2: Run it** — `scripts/with-slot.sh cargo test --lib census_of_this_repo -- --ignored --nocapture` → it prints the census.
- [ ] **Step 3: Record the printed counts** in `docs/trackers/artifact-identity-session-log.md` through `doc(action="append_entry", id="abff63149e3c24e8", id_prefix="F" or "W", …)`. They size Plan E's renames.
- [ ] **Step 4: Commit** — pathspec commit of the test file and the session log: `test(librarian): census of this repo under the new identity model`.

---

## After the plan

- Send `codescout-37` the Interfaces blocks of Tasks 2, 4, 5 and 6 (names and types only), so its ledger work builds on the same `key_of`, `Reference`, `KeyIndex` and `Resolution`.
- Plan B (derived catalog) starts from `KeyIndex::build` as the in-memory core of its document and edge tables.
