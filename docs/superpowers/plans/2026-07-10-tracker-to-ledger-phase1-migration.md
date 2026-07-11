# Tracker → Ledger — Phase 1 Full Migration Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wholesale rename of codescout's librarian augmented-artifacts from "tracker" to "ledger" across all four blast-radius tiers — stored data, global config, API/prompt surface, and prose — plus the three `codescout-ecosystem` sibling repos, with a dual-read compatibility window so nothing breaks mid-migration.

**Architecture:** Land the already-built `ledger-eval` dual-read alias onto `experiments` as the compatibility layer, make `ledger` the canonical name at every surface, add a version-guarded `migrate_v9` that flips stored `kind` (catalog + on-disk frontmatter), rename `docs/trackers/ → docs/ledgers/` **through the librarian `artifact(move)`** (which preserves `id` and re-points events/augmentation — never bare `git mv`), then sweep config, prose, and siblings. The dual-read alias stays one release, then is removed.

**Tech Stack:** Rust (codescout: librarian catalog migrations, MCP tool schemas, prompt surfaces), TOML (global `~/.config/librarian/workspace.toml`), Markdown (docs + memories + CLAUDE.md), and the sibling repos (prompt-engineering, claude-plugins, llm-proxy).

**Decision basis:** Eval-first gate (spec `docs/superpowers/specs/2026-07-10-tracker-to-ledger-rename-design.md`) returned NOT-MET on agent behavior but the user elected to proceed on human-semantic grounds; the eval also showed **no snapshot regression**, so the taxonomy is **wholesale** (one name for all augmented artifacts), not split.

## Global Constraints

- **`master` is protected.** All work on `experiments`; cherry-pick to `master` only after `cargo fmt` + `cargo clippy -- -D warnings` + `cargo test` + `cargo rb` + `/mcp` verify. Never commit in-progress work to `master`.
- **Pre-commit gate (every codescout task):** `cargo fmt && cargo clippy -- -D warnings && cargo test`.
- **Dual-read compatibility is load-bearing during the migration:** every `kind` gate accepts `"tracker" || "ledger"`, and `tracker_design` continues to route, until Task 12 removes them in a follow-up release. Removing the alias before old catalogs are migrated is a breaking change.
- **Directory/file moves go through `artifact(action="move")` ONLY** — it preserves `id` (`sha256` of the *old* path is retained on the row; events/augmentation keyed by `artifact_id` stay linked). A bare `git mv` orphans the catalog row (`id = sha256(abs_path)`); it is prohibited in this plan.
- **`kind` is NOT updatable via `artifact(update)` patch** — flipping it requires the `migrate_v9` path (catalog column + frontmatter rewrite), not a runtime update call.
- **Prompt-surface consistency:** the three surfaces (`server_instructions` + `onboarding_prompt` slices of `src/prompts/source.md`, and `build_system_prompt_draft()` in `builders.rs`) must stay tool-name-consistent; bump `ONBOARDING_VERSION` when their content changes. Two CI gates enforce vocabulary: `prompt_surfaces_reference_only_real_tools` and `claude_md_contains_no_deprecated_tool_names`.
- **Frozen history is out of scope:** `docs/superpowers/plans/*`, `docs/**/archive/*`, and dated session logs are historical records — do NOT rewrite them. Only live docs are swept (Task 10).
- **Naming:** the canonical kind value is `ledger`; the canonical librarian action is `ledger_design`; the canonical guide topic is `ledger-conventions`; the canonical docs dir is `docs/ledgers/`. Old names remain accepted (not emitted) during the compatibility window.

---

### Task 1: Land the dual-read alias on `experiments`

**Files:**
- Source of truth: branch `ledger-eval` commits `83727a34` (alias + dual-read kind) and `079ac3ff` (description flip). These are eval-only [eval-only] commits.
- Modify (via cherry-pick then adjust): `src/librarian/tools/librarian.rs`, `src/librarian/tools/create.rs`, `src/librarian/tools/artifact.rs`.

**Interfaces:**
- Produces: on `experiments`, `librarian(action="ledger_design")` routes to `tracker_design::call`; `kind == "tracker" || "ledger"` at every gate; the Artifact/Librarian tool descriptions mention `ledger`. Relied on by all later tasks.

- [ ] **Step 1: Cherry-pick the alias commits onto experiments**

Run: `git checkout experiments && git cherry-pick 83727a34 079ac3ff`
Expected: both apply cleanly (they touch only `librarian.rs`, `create.rs`, `artifact.rs`, which are unchanged on `experiments` in that region). If a conflict arises, resolve by keeping the dual-read/alias additions.

- [ ] **Step 2: Rewrite the eval-only commit messages into a real feature commit**

Run: `git reset --soft HEAD~2 && git commit -m "feat(ledger): dual-read kind alias + ledger_design action + description flip"`
Expected: one commit on `experiments` with the alias.

- [ ] **Step 3: Verify the suite + lint**

Run: `cargo test 2>&1` (query `@cmd_*` for `FAILED`), then `cargo clippy -- -D warnings 2>&1`.
Expected: green (the known `server::guide_hint_tests` parallel-race flake is unrelated — ignore if it appears). `ledger_design_aliases_tracker_design` and `ledger_kind_triggers_ledger_hint` pass.

---

### Task 2: Make `ledger` the canonical `kind` at every gate (keep `tracker` accepted)

**Files:**
- Modify: `src/librarian/tools/create.rs` (already dual-reads; ensure emitted default/docs say `ledger`).
- Audit + modify: every `kind == "tracker"` / `.with_kind("tracker")` / `"kind": "tracker"` site that EMITS or DEFAULTS to tracker (not test fixtures). Find with:
  `grep(pattern='"tracker"', glob="*.rs")` — 83 hits, 31 files; classify each as EMIT (flip to ledger) vs ACCEPT (leave dual-read) vs TEST (Task 11).
- Modify: `src/librarian/classify.rs` (classification defaults), `src/librarian/tools/doctor.rs` (any kind defaulting).

**Interfaces:**
- Consumes: Task 1 dual-read.
- Produces: codescout emits `kind: ledger` for new augmented artifacts; still accepts `tracker`.

- [ ] **Step 1: Write a failing test** — in `src/librarian/tools/create.rs` tests, assert that `artifact(create, kind unspecified-for-augmented)` or the archetype path yields `kind: "ledger"` (mirror an existing create test). If create requires explicit kind, instead assert `classify.rs` maps a `docs/ledgers/` path to `kind: "ledger"`.

```rust
#[tokio::test]
async fn augmented_artifact_default_kind_is_ledger() {
    // construct the create/classify path that previously produced "tracker";
    // assert the emitted kind is now "ledger"
}
```

- [ ] **Step 2: Run it — expect FAIL** (`cargo test augmented_artifact_default_kind_is_ledger`).
- [ ] **Step 3: Flip EMIT sites** — for each classified EMIT hit, change the literal `"tracker"` → `"ledger"`. Leave ACCEPT (dual-read `||`) sites and TEST sites. Use `edit_code`/`edit_file` per site.
- [ ] **Step 4: Run the test — expect PASS**, then `cargo clippy -- -D warnings`.
- [ ] **Step 5: Commit** — `git commit -m "feat(ledger): emit ledger as canonical kind (tracker still accepted)"`.

---

### Task 3: Promote `ledger_design` to canonical action; deprecate `tracker_design`

**Files:**
- Modify: `src/librarian/tools/librarian.rs` (dispatch ~103, enum ~52, description ~17/20, error strings ~100/113). Make `ledger_design` the documented action; keep `tracker_design` as an accepted (undocumented) alias arm.
- Rename module: `src/librarian/tools/tracker_design.rs` → `ledger_design.rs` (via `edit_code` rename + `mod.rs` update at `src/librarian/tools/mod.rs:269`), OR keep the file and re-export — choose rename for cleanliness; update the `pub mod` and the two `super::tracker_design::call` references.
- Modify: `src/server.rs:1645` (`is_librarian_tool` — add `ledger_design`), `src/librarian/adapter.rs:95` (comment).

**Interfaces:**
- Consumes: Task 1 alias.
- Produces: `librarian(ledger_design)` is canonical; `tracker_design` still routes (deprecated). `tracker_design::archetypes()` referenced by `tests/librarian/goal_eval.rs:72` becomes `ledger_design::archetypes()` (update that reference).

- [ ] **Step 1: Rename the module** — `edit_code(action="rename")` won't move files; use `run_command("git mv src/librarian/tools/tracker_design.rs src/librarian/tools/ledger_design.rs")` (source file, not a librarian artifact — plain git mv is correct here), then update `mod.rs:269` `pub mod tracker_design;` → `pub mod ledger_design;`, and both dispatch arms to `super::ledger_design::call`.
- [ ] **Step 2: Update the routing test** — rename `tracker_design_routes_correctly` accordingly; keep `ledger_design_aliases_tracker_design` asserting the deprecated alias still routes.
- [ ] **Step 3: Update `tests/librarian/goal_eval.rs:72,74`** (`use ...tracker_design` → `ledger_design`; `tracker_design::archetypes()` → `ledger_design::archetypes()`).
- [ ] **Step 4: Flip descriptions + enum + error strings** in `librarian.rs` so `ledger_design` is listed and `tracker_design` is not (but still accepted in the match).
- [ ] **Step 5: `cargo test && cargo clippy -- -D warnings`** — expect green.
- [ ] **Step 6: Commit** — `git commit -m "refactor(ledger): ledger_design is canonical action; tracker_design deprecated alias"`.

---

### Task 4: Rename the guide topic `tracker-conventions` → `ledger-conventions`

**Files:**
- Modify: `src/tools/guide.rs` (topic registry — 7 hits per earlier grep) — register `ledger-conventions`, keep `tracker-conventions` as an alias returning the same guide (compat).
- Rename guide content file if one exists under `src/prompts/guides/` (find via `grep(pattern="tracker-conventions", glob="*.md")` in `src/prompts/`).
- Update every `get_guide("tracker-conventions")` reference in live surfaces (CLAUDE.md, guides) — Task 8/10 handle docs; here fix the code-level registry + any `_guide_hint` emission.

**Interfaces:**
- Produces: `get_guide("ledger-conventions")` returns the conventions guide; `get_guide("tracker-conventions")` still works (alias).

- [ ] **Step 1: Failing test** — a guide test asserting `get_guide("ledger-conventions")` returns non-empty content (mirror an existing guide test in `guide.rs`).
- [ ] **Step 2: Run — FAIL.**
- [ ] **Step 3: Register `ledger-conventions`** as the canonical topic; alias `tracker-conventions` to it. Flip the guide body's own vocabulary (tracker→ledger concept word; preserve any literal paths).
- [ ] **Step 4: Run — PASS**, `cargo clippy -- -D warnings`.
- [ ] **Step 5: Commit** — `git commit -m "feat(ledger): ledger-conventions guide topic (tracker-conventions aliased)"`.

---

### Task 5: Flip the three prompt surfaces + bump `ONBOARDING_VERSION`

**Files:**
- Modify: `src/prompts/source.md` (the `server_instructions` + `onboarding_prompt` slices — flip tracker→ledger concept word, `tracker_design`→`ledger_design`, `docs/trackers`→`docs/ledgers` where it appears as guidance).
- Modify: `src/prompts/builders.rs` (`build_system_prompt_draft()` — same vocabulary).
- Modify: wherever `ONBOARDING_VERSION` is defined (`grep(pattern="ONBOARDING_VERSION", glob="*.rs")`) — bump it.
- Respect the 2200-byte slice cap (`src/prompts/README.md`).

**Interfaces:**
- Produces: all three prompt surfaces speak "ledger"; `ONBOARDING_VERSION` bumped so clients re-onboard.

- [ ] **Step 1:** Read `src/prompts/README.md` for the slice-cap + surface rules.
- [ ] **Step 2:** Flip vocabulary in `source.md` slices and `builders.rs`, staying under the 2200-byte slice cap (verify byte counts).
- [ ] **Step 3:** Bump `ONBOARDING_VERSION`.
- [ ] **Step 4:** `cargo test 2>&1` — the `prompt_surfaces_reference_only_real_tools` gate must pass (all three surfaces reference only real tool/action names). Fix any mismatch.
- [ ] **Step 5: Commit** — `git commit -m "feat(ledger): flip prompt surfaces to ledger; bump ONBOARDING_VERSION"`.

---

### Task 6: `migrate_v9` — flip stored `kind` (catalog + frontmatter)

**Files:**
- Create: `src/librarian/catalog/migrate_v9.rs` (mirror `migrate_v6.rs` structure: a guarded function + tests).
- Modify: `src/librarian/catalog/mod.rs` — add `mod migrate_v9;` and a guarded call inside `run_migrations` (after the existing v-blocks), plus stamp `schema_version` 9.

**Interfaces:**
- Consumes: the dual-read gates (so mid-migration reads of not-yet-flipped rows still work).
- Produces: on catalog open, every `kind = 'tracker'` row → `kind = 'ledger'` in the catalog AND its on-disk frontmatter `kind:` rewritten. Idempotent (a second run is a no-op once no tracker rows remain).

- [ ] **Step 1: Write failing tests** in `migrate_v9.rs` `mod tests` (mirror `migrate_v6` test harness — seed a DB with a `kind='tracker'` artifact row + a matching frontmatter file):

```rust
#[test]
fn v9_flips_catalog_kind_tracker_to_ledger() { /* seed tracker row; run; assert kind==ledger */ }
#[test]
fn v9_rewrites_frontmatter_kind_on_disk() { /* seed file with `kind: tracker`; run; assert file frontmatter kind: ledger */ }
#[test]
fn v9_is_idempotent() { /* run twice; second is no-op; no error */ }
#[test]
fn v9_leaves_non_tracker_kinds_untouched() { /* spec/plan/bug rows unchanged */ }
```

- [ ] **Step 2: Run — FAIL** (`cargo test migrate_v9`).
- [ ] **Step 3: Implement `migrate_v9::run(conn, ws)`**:
  1. `UPDATE artifacts SET kind = 'ledger', updated_at = ?now WHERE kind = 'tracker'` (catalog).
  2. For each affected row (or every `kind='ledger'` row whose file still has `kind: tracker`), read the file, rewrite its frontmatter `kind:` via `crate::librarian::frontmatter` (the same writer `create.rs` uses), recompute `file_sha256`, upsert. Guard with a "does any tracker row / any tracker-frontmatter file exist" precondition so it's a fast no-op when done.
  3. `INSERT OR IGNORE INTO schema_version (version) VALUES (9)`.
- [ ] **Step 4: Wire into `run_migrations`** (`src/librarian/catalog/mod.rs`) after the v8 block; call `migrate_v9::run(conn, ws)?;` (needs `ws` for file paths, like `backfill`). If `ws` is `None` (the `Catalog::open` no-workspace path), do only the catalog `UPDATE` and defer frontmatter rewrite to the workspace-aware open (mirror how `backfill` is gated on `Some(ws)`).
- [ ] **Step 5: Run — PASS**, `cargo clippy -- -D warnings`, full `cargo test`.
- [ ] **Step 6: Commit** — `git commit -m "feat(ledger): migrate_v9 flips stored kind tracker→ledger (catalog + frontmatter)"`.

---

### Task 7: One-time directory rename `docs/trackers/ → docs/ledgers/` via `artifact(move)`

**Files:**
- Create: `src/cli/migrate_ledgers.rs` (a one-time CLI subcommand `codescout migrate-ledgers`) following the `src/cli/artifact.rs` clap pattern; register it in the top-level command enum (read `src/main.rs` for the registration site).
- Reuse: `src/librarian/tools/mv.rs::call` logic (preserves `id`, re-points linkage).

**Interfaces:**
- Consumes: Task 6 (kind already flipped). The move preserves `id` (`mv::call` upserts the same `a.id` with the new `abs_path`), so events/augmentation stay linked.
- Produces: every `docs/trackers/*.md` (and `archive/`) artifact relocated to `docs/ledgers/…` with catalog + linkage intact; the directory renamed.

- [ ] **Step 1: Write a failing integration test** (`tests/`): seed a catalog with a `docs/trackers/foo.md` artifact + an augmentation row + an event; run the migrate-ledgers command; assert (a) file now at `docs/ledgers/foo.md`, (b) same `id`, (c) augmentation + event still resolve for that `id`, (d) no orphan rows.
- [ ] **Step 2: Run — FAIL.**
- [ ] **Step 3: Implement** — the subcommand enumerates catalog rows whose `abs_path` contains `/docs/trackers/`, and for each calls the move logic with `new_rel_path` = the path with `docs/trackers` → `docs/ledgers`. Print a summary (moved N, skipped M).
- [ ] **Step 4: Run — PASS.**
- [ ] **Step 5: Reconciliation guard** — after the moves, run `librarian(action="doctor")` (or the CLI equivalent) in the test and assert zero collisions/orphans. If the reindexer re-derives `id` from the new path and would duplicate, the test must catch it; if so, add a `graft`/reseat step (see memory `worktree-merge-catalog-reconciliation`).
- [ ] **Step 6:** `cargo test && cargo clippy -- -D warnings`.
- [ ] **Step 7: Commit** — `git commit -m "feat(ledger): migrate-ledgers CLI renames docs/trackers→docs/ledgers via artifact(move)"`.
- [ ] **Step 8: Run it on this repo** — `cargo rb` then `codescout migrate-ledgers` against codescout's own catalog; verify `docs/ledgers/` populated, `docs/trackers/` empty, `git status` shows renames, catalog doctor clean. Commit the moved files: `git add -A docs/ledgers docs/trackers && git commit -m "chore(ledger): relocate docs/trackers → docs/ledgers"`.

---

### Task 8: Global config — `~/.config/librarian/workspace.toml` classification globs

**Files:**
- Modify: `~/.config/librarian/workspace.toml` — the three `[[rule]]` globs (`**/docs/trackers/**/*.md`, `**/*_TRACKER.md`, `**/*-tracker.md`) → add `**/docs/ledgers/**/*.md`, `**/*_LEDGER.md`, `**/*-ledger.md`; KEEP the tracker globs during the compat window (so un-migrated peers still classify).

**Interfaces:**
- Produces: files under `docs/ledgers/` classify as `kind = ledger`; old `docs/trackers/` still classifies (compat).

- [ ] **Step 1:** Read the current `[[rule]]` block (it's global config, not in-repo — read via `run_command("cat ~/.config/librarian/workspace.toml")`).
- [ ] **Step 2:** Add the three ledger globs (kind = "ledger") alongside the tracker ones. This file is global config, edited with `edit_file` (not a source file).
- [ ] **Step 3: Verify** — `cargo rb` then create a scratch `docs/ledgers/x.md` in a test repo and confirm `artifact(find)` classifies it `kind: ledger`. (No repo commit — global config is not version-controlled here; note the change in the plan's completion log.)

---

### Task 9: CLAUDE.md, memories, and the CI vocabulary gates

**Files:**
- Modify: `CLAUDE.md` (codescout) — flip the concept word tracker→ledger, `tracker_design`→`ledger_design`, `docs/trackers`→`docs/ledgers`, `tracker-conventions`→`ledger-conventions`, and the `TAXONOMY.md` / tracker-section prose. The `claude_md_contains_no_deprecated_tool_names` gate keys off this file.
- Modify: codescout memories that mention trackers (`architecture`, `conventions`, `onboarding`, `system-prompt`, `reconnaissance`) — via `memory(action="write")`.
- Modify: `docs/TAXONOMY.md`, `src/prompts/README.md` references.
- Verify: the two gates `prompt_surfaces_reference_only_real_tools` and `claude_md_contains_no_deprecated_tool_names` pass.

- [ ] **Step 1:** Flip CLAUDE.md vocabulary (concept word + tool/topic/dir names). Preserve historical references and the bug-tracking `docs/issues/` section (bugs are `kind=bug`, unaffected).
- [ ] **Step 2:** Update the listed memories (read each, rewrite tracker→ledger where it's the augmented-artifact concept; leave unrelated uses).
- [ ] **Step 3:** `cargo test 2>&1` — both vocabulary gates green. Fix any residual deprecated-name hit they flag.
- [ ] **Step 4: Commit** — `git commit -m "docs(ledger): CLAUDE.md + memories + TAXONOMY vocabulary → ledger"`.

---

### Task 10: Prose sweep of live docs (mechanical)

**Files:**
- Modify: live docs only — `docs/architecture/*.md`, `docs/conventions/*.md`, `docs/RELEASE.md`, `docs/PROGRESSIVE_DISCOVERABILITY.md`, `CONTRIBUTING.md`, and any `docs/` file that is NOT under `docs/superpowers/plans/`, `docs/**/archive/`, or a dated session log.
- Explicitly EXCLUDE frozen history.

- [ ] **Step 1:** Enumerate candidates: `grep(pattern="tracker", glob="*.md", mode="files")` minus the excluded paths.
- [ ] **Step 2:** For each live doc, flip the concept word + tool/topic/dir names. Leave code samples' unrelated identifiers and historical references intact.
- [ ] **Step 3: Verify** — the `audit_doc_refs` gate (`cargo test` / CLI) shows no new broken doc-link drift.
- [ ] **Step 4: Commit** — `git commit -m "docs(ledger): sweep live docs tracker→ledger"`.

---

### Task 11: Test fixtures + remaining `"tracker"` literals

**Files:**
- Modify: the TEST-classified `"tracker"` hits from Task 2's audit (e.g. `doctor.rs` `.with_kind("tracker")`, `create.rs` test fixtures, `append_entry.rs:53`, `server.rs` guide_hint tests, `tests/**`). Update to `ledger` where the test asserts canonical behavior; keep a couple asserting the `tracker` compat-alias path still works.

- [ ] **Step 1:** Re-run `grep(pattern='"tracker"', glob="*.rs")`; for each remaining hit, flip to `ledger` unless it deliberately tests the deprecated alias.
- [ ] **Step 2:** Keep/確認 at least one test each for: dual-read `kind=="tracker"` still accepted, `tracker_design` action still routes, `get_guide("tracker-conventions")` still resolves (the compat guarantees).
- [ ] **Step 3:** `cargo test && cargo clippy -- -D warnings` — full green.
- [ ] **Step 4: Commit** — `git commit -m "test(ledger): migrate fixtures to ledger; retain compat-alias tests"`.

---

### Task 12 (follow-up release — DO NOT run with Tasks 1–11): remove the compat aliases

**Files:** `librarian.rs` (drop `tracker_design` arm), `create.rs`/gates (drop `|| "ledger"`… i.e. drop `"tracker"`), `guide.rs` (drop `tracker-conventions` alias), `workspace.toml` (drop tracker globs).

- [ ] Gate on evidence that all live catalogs have run `migrate_v9` (dual-read no longer needed). Ship in a later release, not this branch. Left as a checklist for a future plan.

---

### Task 13: Cross-repo siblings (`codescout-ecosystem`)

**Files (each its own repo, its own branch + commit):**
- `prompt-engineering`: references to codescout's `kind=tracker`, `tracker_design`, `docs/trackers/`, `tracker-conventions` in scenarios/docs/memories. NB: the `ledger-vs-tracker` eval scenarios stay as-is (historical). Flip only live guidance.
- `claude-plugins/codescout-companion`: skill text (`tracker-hygiene`, reconnaissance session-log template) referencing the concept.
- `llm-proxy`: any references.

- [ ] **Step 1:** In each repo, `grep` for the four names; flip live references (not historical/eval).
- [ ] **Step 2:** Pathspec-scoped commit per repo (these repos have concurrent uncommitted work — never `git add -A`).
- [ ] **Step 3:** Note in each repo's commit that it tracks codescout's ledger rename.

---

## Self-Review

**Spec coverage (all four tiers):**
- Tier 1 stored data → Tasks 1, 2, 6, 7 (dual-read, canonical kind, migrate_v9, dir rename). ✓
- Tier 2 global config → Task 8. ✓
- Tier 3 API/prompt surface → Tasks 3, 4, 5, 9. ✓
- Tier 4 prose → Tasks 10, 11 (fixtures). ✓
- Cross-repo siblings → Task 13. ✓
- Dual-read window + later removal → Global Constraints + Task 12. ✓
- `id`-preserving move (no orphaning) → Task 7 + Global Constraints. ✓

**Placeholder scan:** the novel/risky tasks (6, 7) carry concrete SQL/logic and test lists; mechanical tasks (9, 10, 11, 13) name exact files + a verification gate rather than inline text (the text is a mechanical concept-word flip, verified by the CI gates). The one genuine unknown — whether post-move reindex re-derives `id` and needs a `graft`/reseat — is isolated to Task 7 Step 5 with a named mitigation (the `worktree-merge-catalog-reconciliation` memory), not hand-waved.

**Type/name consistency:** `ledger` (kind), `ledger_design` (action + module), `ledger-conventions` (guide topic), `docs/ledgers/` (dir), `migrate_v9` (migration), `migrate-ledgers` (CLI) — used consistently across tasks.

**Ordering:** alias (1) → canonical surfaces (2–5) → data migration (6) → dir rename (7, depends on 6's kind flip) → config/docs/tests (8–11) → cross-repo (13). Task 12 is explicitly deferred to a later release.
