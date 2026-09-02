---
id: cbfd691ec212fab3
kind: plan
status: active
title: Cross-host entry-id collision Implementation Plan
tags:
- librarian
- append_entry
- doctor
- entry-id
- cross-machine
topic: entry id allocation across hosts
---

# Cross-host entry-id collision Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect entry-id collisions that a cross-host merge produces, and refuse allocation against a ledger whose commits are unpushed.

**Architecture:** Two independent components in the librarian. Component A is a new read-only `doctor` check that re-derives entry definitions from raw headings and reports any token defined twice inside one ledger. Component B is a refusal in `append_entry`, sited beside the existing worktree guard, that fires when the ledger's own file has commits in `@{upstream}..HEAD`. Neither touches the entry-id grammar.

**Tech Stack:** Rust, `rusqlite`, `git2` (already a dependency), `regex`, `pulldown-cmark` (via existing helpers).

**Spec:** `docs/superpowers/specs/2026-09-02-entry-id-cross-host-collision-design.md`

## Global Constraints

- **The gate is four commands in this order, and the order is load-bearing:** `cargo fmt`; `cargo clippy --workspace --all-targets --features local-embed -- -D warnings`; `cargo test --workspace --no-default-features`; `cargo test --workspace`. **Chain the two test lanes with `;`, never `&&`.**
- **This is a shared checkout.** Commit by pathspec (`git commit -m "..." -- <paths>`), never a bare `git commit`. Never `git reset` or `git restore --staged`.
- **Component A must NOT read `DocExtract.definitions`.** `src/librarian/tools/link_scan/extract.rs:399` and `:440` both guard on a shared `seen_defs: BTreeSet`, so a duplicate token is discarded at parse time. Reading `ex.definitions` yields a check whose positive case is unrepresentable. See `bug-fix-session-log:F-99`.
- **A ledger is DECLARED, never inferred** — an artifact with `entry_prefix` in frontmatter. Never infer from body content.
- **Entry-id grammar is fixed:** `[A-Z]{1,3}-\d+`. Do not add suffixes or partition the numeric space.

---

### Task 1: Duplicate-definition detector (pure function)

**Files:**
- Modify: `src/librarian/tools/doctor.rs` (add function + its `#[cfg(test)]` tests in the existing `mod tests`)

**Interfaces:**
- Consumes: `crate::librarian::preview::headings::parse(text)` — returns headings each with `.line: usize` and `.text: String`.
- Produces: `fn duplicate_definitions(text: &str, prefixes: &[String]) -> Vec<(String, Vec<u32>)>` — token → the lines defining it, only for tokens defined 2+ times, sorted by token.

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn duplicate_definitions_finds_a_token_defined_twice() {
        let body = "# L\n\n## R-147 — first\n\ntext\n\n## R-147 — second\n";
        let got = duplicate_definitions(body, &["R".to_string()]);
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!(got[0].0, "R-147");
        assert_eq!(got[0].1.len(), 2, "both lines reported: {got:?}");
    }

    /// The negative direction. Without it the function could return every token
    /// it sees and still pass the test above — an existence assertion is monotone
    /// under widening.
    #[test]
    fn duplicate_definitions_is_silent_on_a_clean_ledger() {
        let body = "# L\n\n## R-147 — first\n\n## R-148 — second\n";
        assert!(
            duplicate_definitions(body, &["R".to_string()]).is_empty()
        );
    }

    /// A prefix the ledger does not declare is not this ledger's namespace.
    #[test]
    fn duplicate_definitions_ignores_an_undeclared_prefix() {
        let body = "# L\n\n## T-1 — a\n\n## T-1 — b\n";
        assert!(duplicate_definitions(body, &["R".to_string()]).is_empty());
    }

    /// `## A-9 Addendum` has no dash separator, so it defines nothing —
    /// two of them are two sections ABOUT A-9, not a duplicate definition.
    #[test]
    fn duplicate_definitions_needs_the_dash_separator() {
        let body = "# L\n\n## R-9 Addendum\n\n## R-9 Addendum\n";
        assert!(duplicate_definitions(body, &["R".to_string()]).is_empty());
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --workspace duplicate_definitions`
Expected: FAIL — `cannot find function 'duplicate_definitions' in this scope`.

- [ ] **Step 3: Write the implementation**

Add near the other entry-family scans in `src/librarian/tools/doctor.rs`:

```rust
/// Tokens this text defines more than once, with every defining line.
///
/// Re-derives definitions from raw headings rather than reading
/// `DocExtract.definitions`: `link_scan`'s extractor de-duplicates at parse time
/// (`link_scan/extract.rs:399` and `:440` both guard one `seen_defs` set), so a
/// same-file duplicate is DISCARDED before it reaches any consumer. Reading that
/// vector here would produce a check whose positive case is unrepresentable.
/// `bug-fix-session-log:F-99`.
///
/// The definition grammar is `extract.rs::def_re`'s, repeated rather than shared
/// because that function is private and making it `pub(crate)` would widen a
/// parser's surface for one caller. If the two ever disagree, the pinned test
/// `duplicate_definitions_needs_the_dash_separator` is what fails.
fn duplicate_definitions(text: &str, prefixes: &[String]) -> Vec<(String, Vec<u32>)> {
    use std::collections::BTreeMap;
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"^\s*([A-Z]{1,3}-\d+)\s+[—–-]\s+").unwrap());

    let mut seen: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for h in crate::librarian::preview::headings::parse(text) {
        let Some(m) = re.captures(&h.text) else { continue };
        let token = m.get(1).unwrap().as_str().to_string();
        let Some(prefix) = token.split('-').next() else { continue };
        if !prefixes.iter().any(|p| p == prefix) {
            continue;
        }
        seen.entry(token).or_default().push(h.line as u32);
    }
    seen.into_iter().filter(|(_, lines)| lines.len() > 1).collect()
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --workspace duplicate_definitions`
Expected: PASS, 4 tests.

- [ ] **Step 5: Mutation-check this site**

Change `lines.len() > 1` to `lines.len() > 0` and re-run. Expected: `duplicate_definitions_is_silent_on_a_clean_ledger` FAILS. Revert. Then delete the `if !prefixes.iter().any(...)` guard and re-run. Expected: `duplicate_definitions_ignores_an_undeclared_prefix` FAILS. Revert.

Record both outcomes in the commit message. If either mutation does not fail, the test does not discriminate and must be strengthened before proceeding.

- [ ] **Step 6: Run the gate and commit**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
git add src/librarian/tools/doctor.rs
git commit -m "feat(doctor): duplicate_definitions, re-derived from headings not DocExtract" -- src/librarian/tools/doctor.rs
```

---

### Task 2: Wire `entry_defined_twice` as a doctor check

**Files:**
- Modify: `src/librarian/tools/doctor.rs` — `declare_checks!` block (line ~172), a new `scan_entry_defined_twice`, and the `call` dispatch (line ~305-324)

**Interfaces:**
- Consumes: `duplicate_definitions(text, prefixes)` from Task 1; `Violation::new(check: &str, artifact_id: Option<String>, path: impl Into<String>, detail: impl Into<String>)`; `crate::util::librarian_guard::declared_entry_prefixes(text) -> Vec<String>`.
- Produces: check name `"entry_defined_twice"` on the wire; `fn scan_entry_defined_twice(conn: &rusqlite::Connection) -> Result<Vec<Violation>>`.

- [ ] **Step 1: Add the registry entry**

In the `declare_checks! { ... }` block, alphabetically between `EntryDatedStale` and `EntryWithoutDefinition`:

```rust
    EntryDefinedTwice => "entry_defined_twice",
```

This is not optional bookkeeping: `Violation::new` carries `debug_assert!(Check::from_wire(check).is_some(), ...)`, so an unregistered name panics across the whole test suite.

- [ ] **Step 2: Write the failing integration test**

In the existing `mod tests`, beside the other `scan_*` tests. Use the module's existing `seed_ledger` helper (see `ledger_defines_nothing`'s tests around line 8483 for its call shape):

```rust
    /// A ledger with two `## R-147 — …` headings. The FIXTURE'S LOAD-BEARING DETAIL
    /// is the repeated token with different titles: the collision a cross-host merge
    /// produces is two entries that were each written independently, not a copy.
    #[tokio::test]
    async fn entry_defined_twice_fires_on_a_duplicated_token() {
        let (ctx, tmp, cat) = mk_ctx();
        seed_ledger(
            &cat,
            "ledger",
            &tmp.path().join("ledger.md"),
            "---\nentry_prefix: R\n---\n\n# L\n\n## R-147 — desktop\n\nbody\n\n## R-147 — laptop\n\nbody\n",
        );
        let out = run(&ctx).await;
        let found = violations_named(&out, "entry_defined_twice");
        assert_eq!(found.len(), 1, "{out:#?}");
        assert!(found[0]["detail"].as_str().unwrap().contains("R-147"));
    }

    /// The negative direction, and the one that makes the test above discriminating.
    #[tokio::test]
    async fn entry_defined_twice_is_silent_on_a_clean_ledger() {
        let (ctx, tmp, cat) = mk_ctx();
        seed_ledger(
            &cat,
            "ledger",
            &tmp.path().join("ledger.md"),
            "---\nentry_prefix: R\n---\n\n# L\n\n## R-147 — one\n\n## R-148 — two\n",
        );
        let out = run(&ctx).await;
        assert!(violations_named(&out, "entry_defined_twice").is_empty(), "{out:#?}");
    }

    /// Spec success criterion 2. A non-ledger with two identical definition headings
    /// declares no `entry_prefix`, so it owns no namespace and defines nothing.
    /// 24 of 27 trackers under docs/trackers/ are in this category.
    #[tokio::test]
    async fn entry_defined_twice_is_silent_on_a_non_ledger() {
        let (ctx, tmp, cat) = mk_ctx();
        seed_ledger(
            &cat,
            "notaledger",
            &tmp.path().join("notaledger.md"),
            "---\nkind: doc\n---\n\n# Design\n\n## R-147 — quoted\n\n## R-147 — quoted again\n",
        );
        let out = run(&ctx).await;
        assert!(violations_named(&out, "entry_defined_twice").is_empty(), "{out:#?}");
    }
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test --workspace entry_defined_twice`
Expected: FAIL — the first test asserts 1 violation and gets 0.

- [ ] **Step 4: Write the scan**

```rust
/// `entry_defined_twice`: one ledger defines the same `PREFIX-N` token twice.
///
/// This is what a cross-host merge produces. Two clones read their own committed
/// `entry_high_water_<PREFIX>`, allocate the same id, and the merge lands two
/// `## PREFIX-N — <title>` headings in one file —
/// `docs/issues/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md`.
///
/// Unreachable at two independent layers before this check, which is why neither
/// `link_scan` nor `scan_undefined_entries` reports it: the extractor de-duplicates
/// definitions at parse time, and `resolve.rs`'s `EntryToken` arm short-circuits to
/// `SelfCite` whenever a definer shares the citing artifact's id — the commonest
/// citation shape inside a ledger.
///
/// Scope is ONE artifact. A token defined in both a live ledger and its archive
/// companion is the compaction ladder working; cross-artifact duplication is
/// `link_scan`'s `ambiguous` and `prefix_conflicts`.
///
/// Read-only; there is no `fix=`. Renumbering rewrites a citable token, which
/// silently re-points every existing citation — a separate decision.
fn scan_entry_defined_twice(conn: &rusqlite::Connection) -> Result<Vec<Violation>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.abs_path FROM artifact a \
         WHERE a.missing_since IS NULL ORDER BY a.abs_path",
    )?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<std::result::Result<_, _>>()?;

    let mut out = Vec::new();
    for (id, abs_path) in rows {
        let Ok(text) = std::fs::read_to_string(&abs_path) else { continue };
        let prefixes = crate::util::librarian_guard::declared_entry_prefixes(&text);
        if prefixes.is_empty() {
            continue;
        }
        for (token, lines) in duplicate_definitions(&text, &prefixes) {
            let lines_str = lines.iter().map(|l| l.to_string()).collect::<Vec<_>>().join(", ");
            out.push(Violation::new(
                "entry_defined_twice",
                Some(id.clone()),
                abs_path.clone(),
                format!(
                    "`{token}` is defined {} times in this ledger, at lines {lines_str}. \
                     One token with two active definitions is what a cross-host merge \
                     produces: two clones allocated it from their own committed \
                     `entry_high_water_` mark. Every citation of `{token}` now resolves \
                     to whichever definition the reader reaches first, and nothing else \
                     reports this state — the extractor de-duplicates definitions at \
                     parse time. Give the LATER entry a fresh id; never a suffix \
                     (`{token}b` is not a valid token and can be neither defined nor \
                     cited), and re-point citations that meant it.",
                    lines.len()
                ),
            ));
        }
    }
    Ok(out)
}
```

- [ ] **Step 5: Wire it into `call`**

Beside the sibling entry-family scans (after the `scan_undefined_entries` line, ~311):

```rust
    // One ledger defining a token twice — what a cross-host merge produces. Sits
    // beside scan_undefined_entries because the two ask opposite questions of the
    // same bodies (never defined / defined twice). See `scan_entry_defined_twice`.
    all_violations.extend(scan_entry_defined_twice(&cat.conn)?);
```

- [ ] **Step 6: Run to verify all three pass**

Run: `cargo test --workspace entry_defined_twice`
Expected: PASS, 3 tests.

- [ ] **Step 7: Mutation-check this site**

This site is distinct from Task 1's and must be mutated separately — a kill there says nothing about here. Delete the `if prefixes.is_empty() { continue; }` guard and re-run. Expected: `entry_defined_twice_is_silent_on_a_non_ledger` FAILS. Revert and record.

- [ ] **Step 8: Run the gate and commit**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
git add src/librarian/tools/doctor.rs
git commit -m "feat(doctor): entry_defined_twice — the cross-host merge collision" -- src/librarian/tools/doctor.rs
```

---

### Task 3: Upstream-freshness helper

**Files:**
- Modify: `src/librarian/tools/append_entry.rs` (add function + tests in its existing `mod tests`)

**Interfaces:**
- Consumes: `git2` (already a workspace dependency; precedent `src/retrieval/index_state.rs:327` `behind_count`).
- Produces: `fn ledger_has_unpushed_commits(abs_path: &std::path::Path) -> bool`.

- [ ] **Step 1: Write the failing tests**

```rust
    /// A repo with NO configured upstream must ALLOW. A repo with no remote has no
    /// second host, so refusing there is a pure false positive with no recoverable
    /// reading. This is spec § Error handling, and it is the arm most likely to be
    /// dropped as an edge case — it is the common case for a fresh clone.
    #[test]
    fn no_upstream_reports_no_unpushed_commits() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        let led = tmp.path().join("ledger.md");
        std::fs::write(&led, "x").unwrap();
        commit_all(&repo, "first");
        assert!(!ledger_has_unpushed_commits(&led));
    }

    /// A path outside any git repository must ALLOW, not panic.
    #[test]
    fn non_git_root_reports_no_unpushed_commits() {
        let tmp = tempfile::tempdir().unwrap();
        let led = tmp.path().join("ledger.md");
        std::fs::write(&led, "x").unwrap();
        assert!(!ledger_has_unpushed_commits(&led));
    }

    /// THE DISCRIMINATION THAT MATTERS. A branch-wide check passes a refusal-only
    /// test; measured on codescout 2026-09-02, HEAD was 34 commits ahead of
    /// origin/experiments while 2 of 3 ledgers had ZERO unpushed commits touching
    /// them. Only a per-file check separates these two assertions, so removing
    /// either one makes the pair satisfiable by an unusable implementation.
    #[test]
    fn unpushed_is_per_file_not_per_branch() {
        let (tmp, origin_clone) = repo_with_upstream();
        let ledger = origin_clone.join("ledger.md");
        let other = origin_clone.join("other.md");
        std::fs::write(&other, "changed").unwrap();
        commit_path(&origin_clone, "other.md", "touch other");

        assert!(
            !ledger_has_unpushed_commits(&ledger),
            "an unpushed commit on ANOTHER file must not block this ledger"
        );

        std::fs::write(&ledger, "changed").unwrap();
        commit_path(&origin_clone, "ledger.md", "touch ledger");
        assert!(
            ledger_has_unpushed_commits(&ledger),
            "an unpushed commit on THIS ledger must be reported"
        );
        let _ = tmp;
    }
```

Add these two helpers to the same `mod tests` (the module already builds repos for `append_from_worktree_lands_on_shadow_not_main`; follow its `git2` usage):

```rust
    fn commit_all(repo: &git2::Repository, msg: &str) {
        let mut idx = repo.index().unwrap();
        idx.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None).unwrap();
        idx.write().unwrap();
        let tree = repo.find_tree(idx.write_tree().unwrap()).unwrap();
        let sig = git2::Signature::now("t", "t@e").unwrap();
        let parents: Vec<git2::Commit> =
            repo.head().ok().and_then(|h| h.peel_to_commit().ok()).into_iter().collect();
        let refs: Vec<&git2::Commit> = parents.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &refs).unwrap();
    }

    fn commit_path(root: &std::path::Path, _rel: &str, msg: &str) {
        let repo = git2::Repository::open(root).unwrap();
        commit_all(&repo, msg);
    }

    /// Bare origin + a clone whose branch tracks it, both holding `ledger.md` and
    /// `other.md`. TWO repos is the load-bearing detail: with one, the per-file and
    /// per-branch implementations are indistinguishable and both pass.
    fn repo_with_upstream() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let origin = tmp.path().join("origin.git");
        git2::Repository::init_bare(&origin).unwrap();

        let work = tmp.path().join("work");
        let repo = git2::Repository::init(&work).unwrap();
        std::fs::write(work.join("ledger.md"), "base").unwrap();
        std::fs::write(work.join("other.md"), "base").unwrap();
        commit_all(&repo, "base");

        repo.remote("origin", origin.to_str().unwrap()).unwrap();
        let head = repo.head().unwrap();
        let branch_name = head.shorthand().unwrap().to_string();
        repo.find_remote("origin")
            .unwrap()
            .push(&[&format!("refs/heads/{branch_name}:refs/heads/{branch_name}")], None)
            .unwrap();
        let mut branch = repo.find_branch(&branch_name, git2::BranchType::Local).unwrap();
        branch.set_upstream(Some(&format!("origin/{branch_name}"))).unwrap();
        (tmp, work)
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --workspace ledger_has_unpushed -- --include-ignored; cargo test --workspace unpushed_is_per_file`
Expected: FAIL — `cannot find function 'ledger_has_unpushed_commits'`.

- [ ] **Step 3: Write the implementation**

```rust
/// Does this ledger's own file carry commits in `@{upstream}..HEAD`?
///
/// PER-FILE, not per-branch, and that is the whole design. Measured on codescout
/// 2026-09-02: HEAD was 34 commits ahead of `origin/experiments` — the normal state
/// on a branch that is pushed rarely — while 2 of 3 ledgers had zero unpushed
/// commits touching them. A branch-wide check refuses every ledger permanently and
/// gets disabled within a day.
///
/// EVERY FAILURE PATH ALLOWS. No repository, no configured upstream, an unreadable
/// ref: each returns `false`. A repo with no remote has no second host, so refusing
/// there is a false positive with no recoverable reading, and this guard is partial
/// by construction — degrading it to a hard failure trades a real capability for no
/// safety.
fn ledger_has_unpushed_commits(abs_path: &std::path::Path) -> bool {
    let Ok(repo) = git2::Repository::discover(abs_path) else { return false };
    let Ok(head) = repo.head() else { return false };
    let Some(shorthand) = head.shorthand() else { return false };
    let Ok(branch) = repo.find_branch(shorthand, git2::BranchType::Local) else { return false };
    let Ok(upstream) = branch.upstream() else { return false };
    let (Some(head_oid), Some(up_oid)) =
        (head.target(), upstream.get().target()) else { return false };
    if head_oid == up_oid {
        return false;
    }

    let Ok(workdir) = repo.workdir().ok_or(()) else { return false };
    let Ok(rel) = abs_path.strip_prefix(workdir) else { return false };
    let rel = rel.to_string_lossy().replace('\\', "/");

    let mut walk = match repo.revwalk() {
        Ok(w) => w,
        Err(_) => return false,
    };
    if walk.push(head_oid).is_err() || walk.hide(up_oid).is_err() {
        return false;
    }
    for oid in walk.flatten() {
        let Ok(commit) = repo.find_commit(oid) else { continue };
        let Ok(tree) = commit.tree() else { continue };
        let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
        let Ok(diff) = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None) else {
            continue;
        };
        let touched = diff.deltas().any(|d| {
            d.new_file().path().map(|p| p.to_string_lossy() == rel).unwrap_or(false)
                || d.old_file().path().map(|p| p.to_string_lossy() == rel).unwrap_or(false)
        });
        if touched {
            return true;
        }
    }
    false
}
```

- [ ] **Step 4: Run to verify all three pass**

Run: `cargo test --workspace unpushed`
Expected: PASS, 3 tests.

- [ ] **Step 5: Mutation-check**

Replace the whole revwalk body with `return true;` (i.e. make it per-branch). Expected: the first assertion in `unpushed_is_per_file_not_per_branch` FAILS. Revert. Then change `branch.upstream()`'s `else { return false }` to `else { return true }`. Expected: `no_upstream_reports_no_unpushed_commits` FAILS. Revert. Record both.

- [ ] **Step 6: Run the gate and commit**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
git add src/librarian/tools/append_entry.rs
git commit -m "feat(append_entry): per-file upstream-freshness helper" -- src/librarian/tools/append_entry.rs
```

---

### Task 4: The `append_entry` refusal

**Files:**
- Modify: `src/librarian/tools/append_entry.rs:93-103` region (immediately after the worktree guard) and its `mod tests`

**Interfaces:**
- Consumes: `ledger_has_unpushed_commits` from Task 3; `RecoverableError::with_hint(String, String)`; `artifact::get(&cat, &a.id)` returning a row with `.abs_path`.
- Produces: no new public surface — a new early-return in `call`.

- [ ] **Step 1: Write the failing test**

```rust
    /// Refusal names the PUSH remedy, not the refusal. The guard does not prevent
    /// the collision — a peer at origin collides with these unpushed entries whether
    /// or not this caller is refused. What it converts is an INVISIBLE divergence into
    /// a pushed one, so the hint is the entire value and the assertion is on the hint.
    #[tokio::test]
    async fn allocation_is_refused_while_the_ledger_has_unpushed_commits() {
        let (tmp, work) = repo_with_upstream();
        let ledger = work.join("ledger.md");
        std::fs::write(&ledger, "---\nentry_prefix: R\n---\n\n# L\n\n## R-1 — a\n").unwrap();
        commit_path(&work, "ledger.md", "add ledger");

        let (ctx, cat) = mk_ctx_at(&work);
        seed_prose(&ctx, "led", &ledger);

        let err = call(&ctx, json!({
            "id": "led", "id_prefix": "R",
            "anchor_heading": "## L", "title": "t", "body": "b"
        })).await.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("push"), "hint must name the remedy: {msg}");
        let _ = (tmp, cat);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --workspace allocation_is_refused_while_the_ledger`
Expected: FAIL — the call succeeds and `unwrap_err` panics.

- [ ] **Step 3: Add the refusal**

Immediately after the existing worktree refusal's closing brace, inside the same `if let Some(row) = artifact::get(&cat, &a.id)?` block so it reuses the row already fetched:

```rust
                // Same ordering constraint as the worktree guard above: BEFORE
                // resolve_write_target, or a refused call still leaves a shadow row,
                // augmentation, fork event and lineage link (the 2026-07-17 regression).
                //
                // PARTIAL BY CONSTRUCTION, and labelled so. This does not prevent the
                // collision — a peer at origin allocates from origin's mark and collides
                // with these unpushed entries whether or not this caller is refused. What
                // it converts is an invisible divergence into a pushed one, which is why
                // the hint names pushing rather than the refusal.
                // docs/issues/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md
                if ledger_has_unpushed_commits(std::path::Path::new(&row.abs_path)) {
                    return Err(RecoverableError::with_hint(
                        "append_entry: this ledger has commits that are not on its upstream \
                         branch, so its `entry_high_water_` mark is ahead of what any other \
                         host can see"
                            .to_string(),
                        "Push this ledger's commits, then allocate. Another clone reads its \
                         own committed high-water mark, so until yours is pushed both hosts \
                         resolve the same next id and the collision is only visible after \
                         the branches merge — as one token with two definitions."
                            .to_string(),
                    ));
                }
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test --workspace allocation_is_refused_while_the_ledger`
Expected: PASS.

- [ ] **Step 5: Confirm the allow-path still allocates**

Run: `cargo test --workspace append_entry`
Expected: PASS, all pre-existing `append_entry` tests included. Any pre-existing test that now fails means the guard fires in a repo with no upstream — Task 3's `no_upstream_reports_no_unpushed_commits` arm is wrong, not this test.

- [ ] **Step 6: Run the gate and commit**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
git add src/librarian/tools/append_entry.rs
git commit -m "feat(append_entry): refuse allocation against an unpushed ledger" -- src/librarian/tools/append_entry.rs
```

---

### Task 5: Close the bug record

**Files:**
- Modify: `docs/issues/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md`

- [ ] **Step 1: Add `## Fix provenance` with BOTH identifiers for every commit**

For each of Tasks 1-4's commits: `git show <sha> | git patch-id --stable`. Write a `- **SHA:**` / `- **patch-id:**` pair per commit — `structured_fix_pointers` (`doctor.rs:4651`) reads only that triple, so prose provenance is NOT VERIFIED rather than verified-and-passing.

- [ ] **Step 2: Set frontmatter through the catalog**

```
artifact(action="update", id="86738aa79de6a5cf",
         patch={status: "mitigated",
                extra: {closed: "<today>",
                        unverified: "Detection is complete; PREVENTION is partial by construction and stays so. The guard catches only the direction where THIS host is ahead: `@{upstream}` is a remote-tracking ref, stale until someone fetches, so a peer who allocates and pushes while this host has not fetched still collides undetected. An unpushed peer commit is unreachable by any local check. Status is `mitigated`, not `fixed`, for that reason."}})
```

A direct frontmatter edit does not reach the catalog (BL-48).

- [ ] **Step 3: Commit**

```bash
git add docs/issues/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md
git commit -m "docs(issues): entry-id collision detected; prevention stays partial" -- docs/issues/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md
```

Do NOT archive. The record is `mitigated` with a live `unverified:`, and the peer-ahead direction remains open.

---

## Self-review notes

- **Spec coverage:** § Component A → Tasks 1-2. § Component B → Tasks 3-4. § Error handling (allow on no-upstream / non-git) → Task 3 Steps 1, 3. § Testing (both directions, discrimination, per-site mutation) → Tasks 1 Step 5, 2 Step 7, 3 Step 5. § Success criteria 1-2 → Task 2's three tests; 3 → Task 3's `unpushed_is_per_file_not_per_branch` plus Task 4; 4 (no grammar change, no new dependency) → holds by construction, `git2` and `regex` are existing dependencies.
- **Out of scope, per spec:** renumber/repair, fetching before comparison, host-partitioned id spaces. No task implements any of them.
- **Type consistency:** `duplicate_definitions(&str, &[String]) -> Vec<(String, Vec<u32>)>` is defined in Task 1 and consumed in Task 2 with that signature. `ledger_has_unpushed_commits(&Path) -> bool` is defined in Task 3 and consumed in Task 4 with that signature.
- **Known gap the executor must resolve, not guess:** Task 2's tests call `mk_ctx`, `seed_ledger`, `run`, `violations_named`, and Task 4's call `mk_ctx_at`, `seed_prose`. The first four exist in `doctor.rs`'s test module (`violations_named` at `:10762`); `seed_prose` exists in `append_entry.rs`'s (`:537`). **`mk_ctx_at` does not exist** — `append_entry.rs`'s `mk_ctx` (`:294`) takes no path. Task 4 Step 1 must first add `mk_ctx_at(root: &Path)` following `mk_ctx`'s body with the tempdir replaced by `root`. This is named here rather than left to discovery because a missing helper reads like a broken test.

