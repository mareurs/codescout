# Legibility Scan Action + Auto-Reconciling Backlog (Phase 2b) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `librarian(action="legibility_scan")` — runs the Phase-2a engine, groups defects per target, and reconciles a `docs/trackers/legibility-backlog.md` augmented artifact that **auto-updates as refactoring proceeds** (open → closed with a before→after delta when a defect is gone).

**Architecture:** A new librarian action handler `src/librarian/tools/legibility_scan/` mirrors `audit_doc_refs/` (the proven "action runs analysis → reconciles an augmented-artifact tracker's params via `artifact_augment(merge=true)`" template). The Phase-2a engine (`crate::legibility::scan` and the pure detector/lane functions) is the substrate and is **not modified** except for one additive `pub` helper (`measure_target`) used to compute auto-close deltas. Per-defect `Candidate`s are grouped by key into one row carrying a `defects: [...]` array (decided 2026-06-13). The reconcile has two passes: upsert current candidates, then auto-close prior-open rows absent from the current scan.

**Tech Stack:** Rust, `serde`, `serde_json`, `rusqlite`, `chrono`, MiniJinja (via the librarian's `render_template`). Phase 2b of `docs/superpowers/specs/2026-06-13-dzo-friction-probes-design.md`.

**Depends on:** Phase 2a (`src/legibility/mod.rs` — `scan`, `parse_project`, the three detectors, `score_and_rank`, `Candidate`, `Defect`, `Tier`, `Friction`, `FileSymbols`) and Phase 1 (the `usage.db` friction columns). Both shipped on `experiments`.

**Decisions taken (2026-06-13):** (1) multi-defect target → **defects array**, one row per key; 2a stays frozen and 2b groups. (2) `retries` friction → **deferred to v2**; `cost.edit_fails` + `cost.truncations` carry the biting-now signal.

---

## Reused APIs (verified against the codebase 2026-06-13)

**Phase-2a engine (all `pub` in `src/legibility/mod.rs`):**
- `parse_project(root: &Path) -> Vec<FileSymbols>`
- `over_budget_bodies(&[FileSymbols]) -> Vec<StructuralDefect>`, `name_collisions(...)`, `un_mappable_files(...)`
- `recorder_lane(conn: &rusqlite::Connection, project_root: &str) -> rusqlite::Result<HashMap<String, Friction>>`
- `score_and_rank(Vec<StructuralDefect>, &HashMap<String, Friction>) -> Vec<Candidate>`
- `Candidate { key, rel_file, name_path, defect, tier, tokens, budget, lines, friction, score }`; `Defect` (Serialize, snake_case; `OverBudgetBody|NameCollision|UnMappableFile`); `Tier` (with `rank() -> u8`); `Friction { truncations, retries, code_class_edit_fails, other, sessions }`.
- `crate::usage::db::open_db(project_root: &Path) -> rusqlite::Result<Connection>` (creates `.codescout/usage.db` + schema if absent → "missing db" degrades to an empty recorder lane automatically).
- `crate::tools::MAX_INLINE_TOKENS` (= 2500).

**Librarian internals (the `audit_doc_refs` template — `src/librarian/tools/audit_doc_refs/mod.rs`):**
- Dispatch: `src/librarian/tools/librarian.rs` `impl Tool for Librarian::call` (the `match action` at L88-95) + the two error-message action lists (L84, L96) + `description` (L15-33) + `input_schema` (L35-80).
- `crate::librarian::tools::find::call(ctx, args) -> Result<Value>`, `create::call`, `get::call`.
- `crate::librarian::tools::augment::ArtifactAugment.call(ctx, args) -> Result<...>` (params reconcile; `merge:true` patch, `merge:false` full-replace foot-gun).
- `ToolContext` has `ctx.current_project: Option<...>` with `.abs_path: PathBuf`.
- `RecoverableError::new(msg)` / `RecoverableError::with_hint(msg, hint)` → `isError:false`.

## Pre-execution corrections (2026-06-13 recon, caught before dispatch)

1. **Imports** — `ToolContext` and `RecoverableError` are the **librarian's own** types (defined in `src/librarian/tools/mod.rs`), re-exported from `crate::librarian::tools` and DISTINCT from `crate::tools::core::types::ToolContext`. Every file under `src/librarian/tools/` uses `use crate::librarian::tools::{RecoverableError, ToolContext};` (as `audit_doc_refs` does). The Task 1 skeleton import is corrected accordingly. Using `crate::tools::core::ToolContext` would be the WRONG type and fail to compile against `find/create/get/augment::call`.

2. **Test harness** — the real ctx helper is `mk_smoke_ctx(root: std::path::PathBuf) -> ToolContext` (`audit_doc_refs/mod.rs` tests, ~L652). It builds an **in-memory** catalog (`Catalog::open_in_memory()`), so there is **NO `EnvGuard`/`LIBRARIAN_DB` and NO `#[serial_test::serial]`** — each ctx is self-isolated. **Wherever this plan says `mk_project_ctx()` (Tasks 5/6/7/9), instead copy `mk_smoke_ctx` + its imports (`std::sync::Arc`, `parking_lot::Mutex`, `Catalog`, `WorkspaceConfig`, `Root`, `CurrentProject`, `tempfile::TempDir`) from the audit_doc_refs tests** and write bodies as: `let tmp = TempDir::new().unwrap(); /* fixtures under tmp.path() */ let ctx = mk_smoke_ctx(tmp.path().to_path_buf());`. **Drop all `#[serial_test::serial]` attributes shown in this plan's test snippets** — unnecessary with an in-memory catalog. (Note: `mk_smoke_ctx` returns the `ctx` only, not a tuple — keep the `TempDir` alive in a local binding so the temp dir is not dropped mid-test.)
## File structure

| File | Responsibility |
|---|---|
| `src/librarian/tools/legibility_scan/mod.rs` (new) | `LegibilityScanArgs`, `group_by_key`, params structs (`BacklogParams`/`CandidateRow`/`Measure`/`Cost`/`ScanMeta`), `reconcile`, tracker create/load/write, `call` handler, response builder. |
| `src/librarian/tools/legibility_scan/render_prompt.md` (new) | LLM-facing refresh instruction (params are machine-written; mostly descriptive). |
| `src/librarian/tools/legibility_scan/render_template.j2` (new) | MiniJinja template projecting `candidates[]` into the backlog table at the top of the body. |
| `src/librarian/tools/mod.rs` (modify) | `pub mod legibility_scan;` beside `pub mod audit_doc_refs;`. |
| `src/librarian/tools/librarian.rs` (modify) | dispatch arm + error lists + description + input_schema action enum. |
| `src/legibility/mod.rs` (modify) | ONE additive `pub fn measure_target(...)` + its private defect-rank helper for grouping order. Nothing existing is changed. |

---

### Task 1: Module + args + dispatch registration (routing skeleton)

**Files:**
- Create: `src/librarian/tools/legibility_scan/mod.rs`
- Modify: `src/librarian/tools/mod.rs` (add `pub mod legibility_scan;`)
- Modify: `src/librarian/tools/librarian.rs` (dispatch arm + error lists)

- [ ] **Step 1: Write the failing test** — add to `librarian.rs`'s `mod tests` (mirror `audit_doc_refs_action_routes` at L143):

```rust
    #[tokio::test]
    async fn legibility_scan_action_routes() {
        let ctx = mk_ctx();
        let args = json!({ "action": "legibility_scan", "write": false });
        // No active project in mk_ctx → RecoverableError, NOT "unknown action".
        let err = Librarian.call(&ctx, args).await.unwrap_err();
        let msg = format!("{err}");
        assert!(!msg.contains("unknown action"), "should route, got: {msg}");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib librarian::tools::librarian::tests::legibility_scan_action_routes`
Expected: FAIL — `unknown action 'legibility_scan'` (not yet registered).

- [ ] **Step 3: Register the module and the dispatch arm**

In `src/librarian/tools/mod.rs`, add beside `pub mod audit_doc_refs;`:

```rust
pub mod legibility_scan;
```

In `src/librarian/tools/librarian.rs`, add the arm to the `match action` block (after the `audit_doc_refs` arm):

```rust
                "legibility_scan"    => super::legibility_scan::call(ctx, args).await,
```

Add `legibility_scan` to BOTH error-message action lists in the same function (the `action required` message and the `unknown action` message) so the enumerations stay truthful.

- [ ] **Step 4: Create the handler skeleton** — `src/librarian/tools/legibility_scan/mod.rs`:

```rust
//! `librarian(action="legibility_scan")` — runs the Phase-2a legibility engine and
//! reconciles the `docs/trackers/legibility-backlog.md` augmented artifact.
//! Phase 2b of docs/superpowers/specs/2026-06-13-dzo-friction-probes-design.md.

use crate::librarian::tools::{RecoverableError, ToolContext};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct LegibilityScanArgs {
    /// Absolute path; defaults to the active project. Scopes the recorder lane.
    #[serde(default)]
    pub project: Option<String>,
    /// true (default) = reconcile the backlog tracker; false = dry-run JSON only.
    #[serde(default = "default_true")]
    pub write: bool,
    /// Cap candidates returned/written.
    #[serde(default)]
    pub limit: Option<usize>,
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let args: LegibilityScanArgs = serde_json::from_value(args).map_err(|e| {
        RecoverableError::with_hint(
            format!("legibility_scan: bad args: {e}"),
            "see librarian(action=\"legibility_scan\") input schema",
        )
    })?;
    let repo_root = ctx
        .current_project
        .as_ref()
        .ok_or_else(|| {
            RecoverableError::new("legibility_scan: no active project; activate one first")
        })?
        .abs_path
        .clone();
    let _ = (&args, &repo_root); // wired in later tasks
    Ok(json!({ "ok": true }))
}
```

> **Scout note:** confirm the exact import path for `ToolContext` and `RecoverableError` by reading the top of `src/librarian/tools/audit_doc_refs/mod.rs` (`symbols(name="audit_doc_refs", ...)` then read its `use` lines) and copy them verbatim — they are the authoritative paths. Adjust the two `use` lines above to match.

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --lib librarian::tools::librarian::tests::legibility_scan_action_routes`
Expected: PASS (routes to the handler; errors with "no active project", not "unknown action").

- [ ] **Step 6: Commit**

```bash
git add src/librarian/tools/legibility_scan/mod.rs src/librarian/tools/mod.rs src/librarian/tools/librarian.rs
git commit -m "feat(legibility): register librarian(action=legibility_scan) + handler skeleton"
```

---

### Task 2: Group per-defect candidates by key (defects array)

**Files:**
- Modify: `src/librarian/tools/legibility_scan/mod.rs`

- [ ] **Step 1: Write the failing test** — add a `#[cfg(test)] mod tests` to the module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::legibility::{Candidate, Defect, Friction, Tier};

    fn cand(key: &str, defect: Defect, tokens: usize, score: u32, fr: Friction) -> Candidate {
        Candidate {
            key: key.to_string(),
            rel_file: "src/lsp/manager.rs".to_string(),
            name_path: "LspManager/get_or_start".to_string(),
            defect,
            tier: if fr.is_empty() { Tier::Latent } else { Tier::BitingNow },
            tokens,
            budget: 2500,
            lines: 242,
            friction: fr,
            score,
        }
    }

    #[test]
    fn group_by_key_unions_defects_for_same_target() {
        let fr = Friction { truncations: 14, ..Default::default() };
        let k = "src/lsp/manager.rs::LspManager/get_or_start";
        let cands = vec![
            cand(k, Defect::OverBudgetBody, 4180, 42, fr.clone()),
            cand(k, Defect::NameCollision, 0, 42, fr.clone()),
        ];
        let grouped = group_by_key(cands);
        assert_eq!(grouped.len(), 1, "same key collapses to one row");
        let g = &grouped[0];
        assert_eq!(g.defects, vec![Defect::OverBudgetBody, Defect::NameCollision]);
        assert_eq!(g.tokens, 4180, "max structural magnitude across defects");
        assert_eq!(g.tier, Tier::BitingNow);
        assert_eq!(g.score, 42);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib librarian::tools::legibility_scan::tests::group_by_key`
Expected: FAIL — `group_by_key` / `GroupedCandidate` not defined.

- [ ] **Step 3: Implement** — add above the test module:

```rust
use crate::legibility::{Candidate, Defect, Friction, Tier};
use std::collections::BTreeMap;

/// One backlog target after collapsing its per-defect `Candidate`s. `defects` holds
/// every structural defect on the target (decided 2026-06-13: defects-array, not a
/// single dominant defect). Friction is identical across same-key candidates (the
/// recorder lane keys by `name_path`), so it is taken from the first.
pub struct GroupedCandidate {
    pub key: String,
    pub rel_file: String,
    pub name_path: String,
    pub defects: Vec<Defect>,
    pub tier: Tier,
    pub tokens: usize,
    pub budget: usize,
    pub lines: u32,
    pub friction: Friction,
    pub score: u32,
}

/// Stable defect ordering for deterministic `defects` arrays.
fn defect_rank(d: Defect) -> u8 {
    match d {
        Defect::OverBudgetBody => 0,
        Defect::NameCollision => 1,
        Defect::UnMappableFile => 2,
    }
}

/// Collapse per-defect candidates sharing a key into one target carrying all defects.
/// Output is sorted: tier asc, score desc, tokens desc, key asc (same total order as
/// the 2a scorer, applied across grouped rows).
pub fn group_by_key(cands: Vec<Candidate>) -> Vec<GroupedCandidate> {
    let mut map: BTreeMap<String, GroupedCandidate> = BTreeMap::new();
    for c in cands {
        let g = map.entry(c.key.clone()).or_insert_with(|| GroupedCandidate {
            key: c.key.clone(),
            rel_file: c.rel_file.clone(),
            name_path: c.name_path.clone(),
            defects: Vec::new(),
            tier: c.tier,
            tokens: 0,
            budget: c.budget,
            lines: c.lines,
            friction: c.friction.clone(),
            score: c.score,
        });
        if !g.defects.contains(&c.defect) {
            g.defects.push(c.defect);
        }
        g.tokens = g.tokens.max(c.tokens);
        g.lines = g.lines.max(c.lines);
    }
    let mut out: Vec<GroupedCandidate> = map.into_values().collect();
    for g in &mut out {
        g.defects.sort_by_key(|d| defect_rank(*d));
    }
    out.sort_by(|a, b| {
        a.tier
            .rank()
            .cmp(&b.tier.rank())
            .then(b.score.cmp(&a.score))
            .then(b.tokens.cmp(&a.tokens))
            .then(a.key.cmp(&b.key))
    });
    out
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib librarian::tools::legibility_scan::tests::group_by_key`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/librarian/tools/legibility_scan/mod.rs
git commit -m "feat(legibility): group per-defect candidates into defects-array rows"
```

---

### Task 3: `measure_target` (2a additive helper) for auto-close deltas

**Files:**
- Modify: `src/legibility/mod.rs` (additive `pub fn` only — no existing code changes)

- [ ] **Step 1: Write the failing test** — add to `src/legibility/mod.rs`'s `tests` module (after the last test):

```rust
    #[test]
    fn measure_target_returns_body_size_for_a_symbol() {
        let big = sym("Foo/big", SymbolKind::Method, 0, 70);
        let files = vec![file_with("src/foo.rs", 71, vec![big])];
        let (tokens, lines) = measure_target(&files, "src/foo.rs", "Foo/big").unwrap();
        assert!(tokens > crate::tools::MAX_INLINE_TOKENS);
        assert_eq!(lines, 71);
        assert!(measure_target(&files, "src/foo.rs", "Foo/missing").is_none());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib legibility::tests::measure_target`
Expected: FAIL — `measure_target` not defined.

- [ ] **Step 3: Implement** — add to `src/legibility/mod.rs` (above the `tests` module, after `scan`):

```rust
/// Re-measure a single target's current cost (tokens, lines), independent of whether
/// it is still a defect. Used by Phase 2b to fill the `after` delta when a candidate
/// auto-closes (its defect is gone). For a symbol key, measures the body; for an
/// un-mappable file (`name_path == "(file)"`), measures the overview size.
pub fn measure_target(files: &[FileSymbols], rel_file: &str, name_path: &str) -> Option<(usize, u32)> {
    let f = files.iter().find(|f| f.rel_file == rel_file)?;
    if name_path == "(file)" {
        let mut all = Vec::new();
        collect_all(&f.symbols, &mut all);
        return Some((overview_bytes(&all) / 4, f.lines.len() as u32));
    }
    let mut all = Vec::new();
    collect_all(&f.symbols, &mut all);
    let sym = all.iter().find(|s| s.name_path == name_path)?;
    let (body, lines) = body_text(&f.lines, sym);
    Some((body.len() / 4, lines))
}
```

> Note: `collect_all`, `overview_bytes`, `body_text` are private helpers already in the module — `measure_target` reuses them. This is the ONLY change to `src/legibility/mod.rs` in this plan, and it is purely additive (no existing symbol is touched).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib legibility::tests::measure_target`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/legibility/mod.rs
git commit -m "feat(legibility): add pub measure_target helper for 2b auto-close deltas"
```

---

### Task 4: Params structs + the reconcile (the auto-update heart)

**Files:**
- Modify: `src/librarian/tools/legibility_scan/mod.rs`
- Modify: `src/legibility/mod.rs` (optional `tests_helpers` — see note)

- [ ] **Step 1: Write the failing test** — the **reconcile sandwich** (the spec's stale→fresh regression for the auto-close path), added to the handler's `tests` module:

```rust
    use crate::legibility::FileSymbols;

    fn grouped(key: &str, np: &str, tokens: usize, fr: Friction) -> GroupedCandidate {
        GroupedCandidate {
            key: key.to_string(),
            rel_file: "src/foo.rs".to_string(),
            name_path: np.to_string(),
            defects: vec![Defect::OverBudgetBody],
            tier: if fr.is_empty() { Tier::Latent } else { Tier::BitingNow },
            tokens,
            budget: 2500,
            lines: 242,
            friction: fr,
            score: 42,
        }
    }

    #[test]
    fn reconcile_opens_then_auto_closes_with_delta() {
        let key = "src/foo.rs::Foo/big";
        // scan 1: candidate is over budget → open, before captured
        let g1 = grouped(key, "Foo/big", 4180, Friction { truncations: 14, ..Default::default() });
        let rows1 = reconcile(&BacklogParams::default(), &[g1], &[], "2026-06-13");
        assert_eq!(rows1.len(), 1);
        assert_eq!(rows1[0].status, "open");
        assert_eq!(rows1[0].before.tokens, 4180);
        assert!(rows1[0].after.is_none());

        // scan 2: the function was refactored under budget → NOT a candidate anymore.
        // Provide a parsed file whose body is now small so measure_target finds it.
        let small = crate::legibility::tests_helpers::sym_pub("Foo/big", 0, 3);
        let file = crate::legibility::tests_helpers::file_pub("src/foo.rs", 4, vec![small]);
        let prior = BacklogParams { candidates: rows1, scan_meta: Default::default() };
        let rows2 = reconcile(&prior, &[], &[file], "2026-06-14");
        assert_eq!(rows2.len(), 1, "closed rows stay for history");
        assert_eq!(rows2[0].status, "closed");
        assert_eq!(rows2[0].closed_at.as_deref(), Some("2026-06-14"));
        assert_eq!(rows2[0].before.tokens, 4180, "before preserved");
        let after = rows2[0].after.as_ref().expect("after delta recorded");
        assert!(after.tokens < 2500, "after is the now-sub-budget measure");
    }
```

> **The test references `crate::legibility::tests_helpers::{sym_pub, file_pub}`** — small NON-`#[cfg(test)]` `pub` constructors so 2b tests can build `FileSymbols`/`SymbolInfo` fixtures without duplicating 2a's private test helpers. Add them in Step 3 of THIS task. If you prefer not to expand 2a's surface, instead build the `FileSymbols` inline in the test using `crate::lsp::symbols::{SymbolInfo, SymbolKind}` directly (all fields are `pub`); pick one approach and keep the test self-contained.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib librarian::tools::legibility_scan::tests::reconcile_opens_then_auto_closes`
Expected: FAIL — `reconcile` / `BacklogParams` not defined.

- [ ] **Step 3: Implement** — add the params structs + reconcile to the handler module:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Measure {
    pub tokens: usize,
    pub budget: usize,
    pub lines: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cost {
    pub truncations: u32,
    pub edit_fails: u32,
    pub sessions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateRow {
    pub key: String,
    pub rel_file: String,
    pub name_path: String,
    pub defects: Vec<String>, // snake_case defect names
    pub tier: u8,             // 1 = biting-now, 2 = latent
    pub status: String,       // "open" | "closed"
    pub measure: Measure,
    pub cost: Cost,
    pub score: u32,
    pub first_seen: String,
    pub before: Measure,
    pub after: Option<Measure>,
    pub closed_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanMeta {
    pub last_scan_at: Option<String>,
    pub last_scan_commit: Option<String>,
    pub n_candidates: u32,
    pub project_root: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BacklogParams {
    pub candidates: Vec<CandidateRow>,
    pub scan_meta: ScanMeta,
}

fn defect_str(d: Defect) -> &'static str {
    match d {
        Defect::OverBudgetBody => "over_budget_body",
        Defect::NameCollision => "name_collision",
        Defect::UnMappableFile => "un_mappable_file",
    }
}

/// Reconcile the prior backlog with the current scan. Two passes:
/// 1. upsert every current candidate (update in place / insert new, preserving
///    `first_seen` and `before`; re-open a regressed closed row);
/// 2. auto-close every prior `open` row whose key is absent from the current scan —
///    its defect is gone — recording `after` (re-measured) and `closed_at`.
/// Closed rows are retained for history.
pub fn reconcile(
    prior: &BacklogParams,
    current: &[GroupedCandidate],
    files: &[crate::legibility::FileSymbols],
    today: &str,
) -> Vec<CandidateRow> {
    use std::collections::HashSet;
    let current_keys: HashSet<&str> = current.iter().map(|c| c.key.as_str()).collect();
    let mut rows = prior.candidates.clone();

    for c in current {
        let measure = Measure { tokens: c.tokens, budget: c.budget, lines: c.lines };
        let cost = Cost {
            truncations: c.friction.truncations,
            edit_fails: c.friction.code_class_edit_fails,
            sessions: c.friction.sessions,
        };
        let defects: Vec<String> = c.defects.iter().map(|d| defect_str(*d).to_string()).collect();
        if let Some(row) = rows.iter_mut().find(|r| r.key == c.key) {
            row.defects = defects;
            row.tier = c.tier.rank();
            row.measure = measure;
            row.cost = cost;
            row.score = c.score;
            if row.status == "closed" {
                row.status = "open".to_string(); // regression: defect returned
                row.after = None;
                row.closed_at = None;
            }
        } else {
            rows.push(CandidateRow {
                key: c.key.clone(),
                rel_file: c.rel_file.clone(),
                name_path: c.name_path.clone(),
                defects,
                tier: c.tier.rank(),
                status: "open".to_string(),
                measure: measure.clone(),
                cost,
                score: c.score,
                first_seen: today.to_string(),
                before: measure,
                after: None,
                closed_at: None,
                extra: serde_json::Map::new(),
            });
        }
    }

    for row in rows.iter_mut() {
        if row.status == "open" && !current_keys.contains(row.key.as_str()) {
            row.status = "closed".to_string();
            row.closed_at = Some(today.to_string());
            row.after = crate::legibility::measure_target(files, &row.rel_file, &row.name_path)
                .map(|(tokens, lines)| Measure { tokens, budget: crate::tools::MAX_INLINE_TOKENS, lines });
        }
    }
    rows
}
```

If you chose the `tests_helpers` route, add to `src/legibility/mod.rs` (NOT under `#[cfg(test)]`, so the test build of dependent modules can use it):

```rust
/// Minimal public fixture constructors so other modules' tests can build
/// `FileSymbols`/`SymbolInfo` without duplicating private test helpers.
pub mod tests_helpers {
    use super::*;
    pub fn sym_pub(name_path: &str, start: u32, end: u32) -> SymbolInfo {
        SymbolInfo {
            name: name_path.rsplit('/').next().unwrap_or(name_path).to_string(),
            name_path: name_path.to_string(),
            kind: SymbolKind::Method,
            file: std::path::PathBuf::from("x.rs"),
            start_line: start,
            end_line: end,
            range_start_line: None,
            start_col: 0,
            children: vec![],
            detail: None,
        }
    }
    pub fn file_pub(rel: &str, body_lines: usize, syms: Vec<SymbolInfo>) -> FileSymbols {
        FileSymbols {
            rel_file: rel.to_string(),
            lines: (0..body_lines).map(|_| "x".repeat(40)).collect(),
            symbols: syms,
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib librarian::tools::legibility_scan::tests::reconcile_opens_then_auto_closes`
Expected: PASS — open on scan 1, closed-with-`after`-delta on scan 2, `before` preserved.

- [ ] **Step 5: Commit**

```bash
git add src/librarian/tools/legibility_scan/mod.rs src/legibility/mod.rs
git commit -m "feat(legibility): backlog params + reconcile (upsert + auto-close with delta)"
```

---

### Task 5: Tracker create-if-absent + render template + prompt

**Files:**
- Create: `src/librarian/tools/legibility_scan/render_prompt.md`
- Create: `src/librarian/tools/legibility_scan/render_template.j2`
- Modify: `src/librarian/tools/legibility_scan/mod.rs`

- [ ] **Step 1: Write the failing test**

```rust
    // Integration-style: an EnvGuard-isolated ctx with a temp project (see
    // `audit_doc_refs/mod.rs` tests for the ctx-with-temp-project pattern). Assert
    // that ensure_tracker creates an artifact whose params are an empty backlog.
    #[tokio::test]
    #[serial_test::serial]
    async fn ensure_tracker_creates_backlog_artifact() {
        let (ctx, _guard, _dir) = mk_project_ctx();
        let (id, rel) = ensure_tracker(&ctx).await.unwrap();
        assert!(!id.is_empty());
        assert_eq!(rel, "docs/trackers/legibility-backlog.md");
        let prior = load_backlog(&ctx, &id).await.unwrap_or_default();
        assert!(prior.candidates.is_empty());
    }
```

> **Scout the ctx-with-temp-project test harness** in `src/librarian/tools/audit_doc_refs/mod.rs` `mod tests` (it builds a `ToolContext` whose `current_project` points at a temp dir, with an `EnvGuard` for `LIBRARIAN_*` env per `docs/conventions/test-env-isolation.md`). Copy that harness into this module's `tests` as `mk_project_ctx() -> (ToolContext, EnvGuard, TempDir)`. Carry `#[serial_test::serial]` on every test that touches the catalog DB.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib librarian::tools::legibility_scan::tests::ensure_tracker_creates`
Expected: FAIL — `ensure_tracker` / `load_backlog` not defined.

- [ ] **Step 3: Create the template files.**

`src/librarian/tools/legibility_scan/render_prompt.md`:

```markdown
This artifact's `params.candidates` array is written mechanically by
`librarian(action="legibility_scan")`. Do NOT hand-synthesize it. On refresh,
the render_template projects the open/closed backlog table from params; the
Dzo's per-key verdict prose lives below the table and is human/Dzo-owned.
```

`src/librarian/tools/legibility_scan/render_template.j2` (MiniJinja — verify syntax against `audit_doc_refs/render_template.j2`, the working exemplar in the sibling dir; match its filter usage):

```jinja2
## Backlog (auto-managed)

Ranked by the legibility engine. Tier 1 = biting-now (structural defect + observed friction); Tier 2 = latent. The Dzo's verdicts are below.

| key | tier | defects | score | tokens/budget | lines | trunc/edit/sess |
|---|---|---|---|---|---|---|
{% for c in candidates if c.status == "open" -%}
| `{{ c.key }}` | {{ c.tier }} | {{ c.defects | join(", ") }} | {{ c.score }} | {{ c.measure.tokens }}/{{ c.measure.budget }} | {{ c.measure.lines }} | {{ c.cost.truncations }}/{{ c.cost.edit_fails }}/{{ c.cost.sessions }} |
{% endfor %}

### Closed (refactored — before → after tokens)

| key | before → after | closed |
|---|---|---|
{% for c in candidates if c.status == "closed" -%}
| `{{ c.key }}` | {{ c.before.tokens }} → {{ c.after.tokens if c.after else "?" }} | {{ c.closed_at }} |
{% endfor %}
```

> MiniJinja's filter set differs slightly from Jinja2. The `{% for c in candidates if ... %}` guard form is widely supported; if it errors, fall back to iterating all rows and emitting per-status. Confirm against the working `audit_doc_refs/render_template.j2` before finalizing. render_template is cosmetic — a template error must not fail the scan (Task 5 Step 4 already ignores attach errors).

- [ ] **Step 4: Implement `ensure_tracker` + `load_backlog` + `write_backlog`** (mirror `audit_doc_refs::ensure_default_tracker` / `load_tracker_params` / `write_tracker_params` exactly — read those three and adapt the literals):

```rust
const TRACKER_REL_PATH: &str = "docs/trackers/legibility-backlog.md";

async fn ensure_tracker(ctx: &ToolContext) -> Result<(String, String)> {
    // 1. Find existing by path suffix (include archived).
    let find_args = json!({
        "action": "find",
        "filter": { "rel_path": { "contains": TRACKER_REL_PATH } },
        "include_archived": true
    });
    if let Ok(v) = crate::librarian::tools::find::call(ctx, find_args).await {
        if let Some(first) = v.get("items").and_then(|x| x.as_array()).and_then(|a| a.first()) {
            if let Some(id) = first.get("id").and_then(|x| x.as_str()) {
                return Ok((id.to_string(), TRACKER_REL_PATH.to_string()));
            }
        }
    }
    // 2. Create the file's parent dir, then create the augmented artifact.
    let project_root = ctx
        .current_project
        .as_ref()
        .ok_or_else(|| RecoverableError::new("legibility_scan: no active project"))?
        .abs_path
        .clone();
    std::fs::create_dir_all(project_root.join("docs/trackers"))?;
    let empty = serde_json::to_value(BacklogParams::default())?;
    let create_args = json!({
        "action": "create",
        "kind": "tracker",
        "title": "Legibility Backlog",
        "rel_path": TRACKER_REL_PATH,
        "tags": ["legibility", "dzo"],
        "body": "Auto-managed by `librarian(action=\"legibility_scan\")`. Dzo verdicts below the table.\n",
        "augment": { "prompt": include_str!("./render_prompt.md"), "params": empty }
    });
    let created = crate::librarian::tools::create::call(ctx, create_args).await?;
    let id = created
        .get("id")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow::anyhow!("artifact create returned no id: {created}"))?
        .to_string();
    // 3. Attach render_template (create's augment spec does not carry it).
    let augment_args = json!({
        "id": id,
        "prompt": include_str!("./render_prompt.md"),
        "params": serde_json::to_value(BacklogParams::default())?,
        "render_template": include_str!("./render_template.j2")
    });
    if let Err(e) = crate::librarian::tools::augment::ArtifactAugment.call(ctx, augment_args).await {
        tracing::warn!("legibility_scan: failed to attach render_template: {e:#}");
    }
    Ok((id, TRACKER_REL_PATH.to_string()))
}

async fn load_backlog(ctx: &ToolContext, id: &str) -> Option<BacklogParams> {
    let v = crate::librarian::tools::get::call(ctx, json!({ "action": "get", "id": id }))
        .await
        .ok()?;
    let params = v.get("augmentation").and_then(|a| a.get("params"))?;
    serde_json::from_value::<BacklogParams>(params.clone()).ok()
}

async fn write_backlog(ctx: &ToolContext, id: &str, params: &BacklogParams) -> Result<()> {
    let augment_args = json!({ "id": id, "merge": true, "params": serde_json::to_value(params)? });
    crate::librarian::tools::augment::ArtifactAugment.call(ctx, augment_args).await?;
    Ok(())
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --lib librarian::tools::legibility_scan::tests::ensure_tracker_creates`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/librarian/tools/legibility_scan/
git commit -m "feat(legibility): backlog tracker create/load/write + render template"
```

---

### Task 6: Wire the handler — run engine, group, reconcile (or dry-run)

**Files:**
- Modify: `src/librarian/tools/legibility_scan/mod.rs`

- [ ] **Step 1: Write the failing test** — the end-to-end write path against a real temp project + usage.db:

```rust
    #[tokio::test]
    #[serial_test::serial]
    async fn scan_writes_ranked_backlog_for_a_real_over_budget_body() {
        let (ctx, _guard, dir) = mk_project_ctx();
        // a real over-budget function in the project
        let mut src = String::from("fn huge() {\n");
        for i in 0..200 {
            src.push_str(&format!("    let v{i} = \"{}\";\n", "x".repeat(80)));
        }
        src.push_str("}\n");
        std::fs::write(dir.path().join("huge.rs"), src).unwrap();
        // friction on the target
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let conn = crate::usage::db::open_db(dir.path()).unwrap();
        crate::usage::db::write_record(&conn, "symbols", 1, "success", true, None, "cs", None,
            "s1", None, None, Some("ccs1"), Some("huge"), Some(3500), None,
            Some(&dir.path().to_string_lossy())).unwrap();
        drop(conn);

        let out = call(&ctx, json!({ "action": "legibility_scan", "write": true })).await.unwrap();
        let id = out.get("tracker_id").and_then(|x| x.as_str()).expect("tracker_id");
        let backlog = load_backlog(&ctx, id).await.unwrap();
        assert!(
            backlog.candidates.iter().any(|c| c.name_path.contains("huge") && c.status == "open"),
            "expected an open backlog row for huge: {:?}", backlog.candidates
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib librarian::tools::legibility_scan::tests::scan_writes_ranked_backlog`
Expected: FAIL — `call` still returns the `{ok:true}` stub.

- [ ] **Step 3: Implement the handler body** — replace the stub tail of `call` (everything after `repo_root` is resolved, i.e. the `let _ = (&args, &repo_root); Ok(json!({ "ok": true }))` lines) with:

```rust
    let project_root = args.project.clone().unwrap_or_else(|| repo_root.to_string_lossy().into_owned());

    // Index lane — parse ONCE, keep `files` for auto-close re-measurement.
    let files = crate::legibility::parse_project(&repo_root);
    let mut structural = crate::legibility::over_budget_bodies(&files);
    structural.extend(crate::legibility::name_collisions(&files));
    structural.extend(crate::legibility::un_mappable_files(&files));

    // Recorder lane — open_db creates an empty db if absent (graceful degrade).
    let conn = crate::usage::db::open_db(&repo_root)?;
    let friction = crate::legibility::recorder_lane(&conn, &project_root).unwrap_or_default();

    let candidates = crate::legibility::score_and_rank(structural, &friction);
    let mut grouped = group_by_key(candidates);
    if let Some(limit) = args.limit {
        grouped.truncate(limit);
    }

    if !args.write {
        return Ok(build_dry_run(&grouped));
    }

    let today = now_date();
    let (id, rel) = ensure_tracker(ctx).await?;
    let prior = load_backlog(ctx, &id).await.unwrap_or_default();
    let new_rows = reconcile(&prior, &grouped, &files, &today);
    let n_open = new_rows.iter().filter(|r| r.status == "open").count() as u32;
    let n_closed = new_rows.iter().filter(|r| r.status == "closed").count();
    let backlog = BacklogParams {
        candidates: new_rows,
        scan_meta: ScanMeta {
            last_scan_at: Some(today.clone()),
            last_scan_commit: git_head(&repo_root),
            n_candidates: n_open,
            project_root,
        },
    };
    write_backlog(ctx, &id, &backlog).await?;

    Ok(json!({
        "ok": true,
        "tracker_id": id,
        "tracker_path": rel,
        "open": n_open,
        "closed": n_closed,
    }))
```

Add the helpers (placed below `call`):

```rust
fn now_date() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

fn git_head(root: &std::path::Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn build_dry_run(grouped: &[GroupedCandidate]) -> Value {
    let rows: Vec<Value> = grouped
        .iter()
        .map(|c| {
            json!({
                "key": c.key,
                "defects": c.defects.iter().map(|d| defect_str(*d)).collect::<Vec<_>>(),
                "tier": c.tier.rank(),
                "tokens": c.tokens,
                "budget": c.budget,
                "lines": c.lines,
                "score": c.score,
                "cost": { "truncations": c.friction.truncations,
                          "edit_fails": c.friction.code_class_edit_fails,
                          "sessions": c.friction.sessions },
            })
        })
        .collect();
    json!({ "ok": true, "dry_run": true, "candidates": rows, "n": rows.len() })
}
```

> Confirm `git_head` against `audit_doc_refs::git_head_commit` (L285) — if that helper is reachable (e.g. a shared `pub(crate)` fn), reuse it instead of re-implementing.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib librarian::tools::legibility_scan::tests::scan_writes_ranked_backlog`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/librarian/tools/legibility_scan/mod.rs
git commit -m "feat(legibility): wire scan handler — engine + group + reconcile/dry-run"
```

---

### Task 7: Error handling + graceful defaults

**Files:**
- Modify: `src/librarian/tools/legibility_scan/mod.rs`

- [ ] **Step 1: Write the tests** (these are characterization guards — much of the behavior already holds from Task 6):

```rust
    #[tokio::test]
    async fn no_active_project_errors_recoverably() {
        let ctx = crate::librarian::tools::librarian::tests::mk_ctx(); // no project
        let err = call(&ctx, json!({ "action": "legibility_scan" })).await.unwrap_err();
        assert!(format!("{err}").contains("no active project"));
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn missing_usage_db_still_runs_index_lane() {
        let (ctx, _guard, dir) = mk_project_ctx();
        let mut src = String::from("fn huge() {\n");
        for i in 0..200 { src.push_str(&format!("    let v{i} = \"{}\";\n", "x".repeat(80))); }
        src.push_str("}\n");
        std::fs::write(dir.path().join("huge.rs"), src).unwrap();
        // NO usage.db rows written at all.
        let out = call(&ctx, json!({ "action": "legibility_scan", "write": false })).await.unwrap();
        let cands = out.get("candidates").and_then(|c| c.as_array()).unwrap();
        // present as latent (tier 2 — structural defect, zero friction)
        assert!(cands.iter().any(|c| c["tier"] == 2 && c["key"].as_str().unwrap().contains("huge")));
    }
```

- [ ] **Step 2: Run** — `cargo test --lib librarian::tools::legibility_scan::tests::no_active_project legibility_scan::tests::missing_usage_db`. Both should already PASS against the Task-6 code (the project guard exists; `open_db` creates an empty db so `recorder_lane` returns empty → the over-budget body presents as tier 2). If `missing_usage_db` fails, check that `recorder_lane(...).unwrap_or_default()` is in place.

- [ ] **Step 3: Harden the write path** so a tracker failure does not fail the whole scan (mirror `audit_doc_refs::call`'s tracker-failure isolation). Replace the `write_backlog(ctx, &id, &backlog).await?;` line in `call` with:

```rust
    if let Err(e) = write_backlog(ctx, &id, &backlog).await {
        tracing::warn!("legibility_scan: backlog write failed: {e:#}");
        return Ok(json!({
            "ok": true,
            "tracker_error": format!("{e:#}"),
            "open": n_open,
            "closed": n_closed,
        }));
    }
```

- [ ] **Step 4: Run** — `cargo test --lib librarian::tools::legibility_scan::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/librarian/tools/legibility_scan/mod.rs
git commit -m "feat(legibility): graceful degrade — missing db, recoverable bad-project, tracker-failure isolation"
```

---

### Task 8: Librarian schema/description + prompt-surface review

**Files:**
- Modify: `src/librarian/tools/librarian.rs` (`description`, `input_schema`)
- Possibly modify: `src/prompts/source.md`, `src/prompts/builders.rs`, the `get_guide("librarian")` content — only if they enumerate librarian actions.

- [ ] **Step 1: Update the Librarian tool's self-description.** Read `impl Tool for Librarian::description` (L15-33) and `input_schema` (L35-80). Add `legibility_scan` to the action `enum` in `input_schema` and to the description's action listing. Add the `write` (bool), `project` (string), `limit` (integer) params to the schema as optional. Keep edits minimal and consistent with existing entries.

- [ ] **Step 2: Prompt-surface review** (per `CLAUDE.md` § "Prompt Surface Consistency"). `legibility_scan` is an *action of `librarian`*, not a top-level tool, so the `prompt_surfaces_reference_only_real_tools` test (tool-name check) will not trip — but verify:
  - `grep` `src/prompts/source.md` for `audit_doc_refs`/`tracker_design`. If the librarian's actions are enumerated in the `server_instructions` slice, add `legibility_scan` — but the slice is under a hard **2200-byte cap** (`source_md_under_cap`). If the addition would exceed it, move detail to `get_guide("librarian")` and leave only a pointer.
  - `grep` for the `get_guide("librarian")` body and add a one-line `legibility_scan` mention if other actions are listed there.
  - Do NOT bump `ONBOARDING_VERSION` unless the `onboarding_prompt` surface or `builders.rs` changed (action lists live in `server_instructions`, which is live-on-connect → no bump).

- [ ] **Step 3: Run the guards** — `cargo test --lib prompt && cargo test --lib librarian::tools::librarian`
Expected: PASS. If `source_md_under_cap` fails, move content to a guide topic (see `src/prompts/README.md` rule 8) rather than raising the cap. Re-measure the slice byte count on current HEAD first (shared-branch hazard).

- [ ] **Step 4: Commit**

```bash
git add src/librarian/tools/librarian.rs src/prompts/
git commit -m "docs(legibility): advertise legibility_scan in librarian schema + prompt surfaces"
```

---

### Task 9: Full gate + end-to-end reconcile integration test

**Files:**
- Modify: `src/librarian/tools/legibility_scan/mod.rs` (one integration test)

- [ ] **Step 1: Write the end-to-end reconcile test** (the spec's headline behavior — create, then auto-close on re-scan after a shrink):

```rust
    #[tokio::test]
    #[serial_test::serial]
    async fn end_to_end_scan_creates_then_auto_closes_on_refactor() {
        let (ctx, _guard, dir) = mk_project_ctx();
        let path = dir.path().join("huge.rs");
        // scan 1: over budget
        let mut src = String::from("fn huge() {\n");
        for i in 0..200 { src.push_str(&format!("    let v{i} = \"{}\";\n", "x".repeat(80))); }
        src.push_str("}\n");
        std::fs::write(&path, &src).unwrap();
        let out1 = call(&ctx, json!({ "action": "legibility_scan", "write": true })).await.unwrap();
        let id = out1["tracker_id"].as_str().unwrap().to_string();
        let b1 = load_backlog(&ctx, &id).await.unwrap();
        assert!(b1.candidates.iter().any(|c| c.name_path.contains("huge") && c.status == "open"));

        // scan 2: refactor under budget (tiny body)
        std::fs::write(&path, "fn huge() {\n    let v = 1;\n}\n").unwrap();
        let _out2 = call(&ctx, json!({ "action": "legibility_scan", "write": true })).await.unwrap();
        let b2 = load_backlog(&ctx, &id).await.unwrap();
        let row = b2.candidates.iter().find(|c| c.name_path.contains("huge")).unwrap();
        assert_eq!(row.status, "closed", "auto-closed after refactor");
        assert!(row.after.as_ref().map(|m| m.tokens < 2500).unwrap_or(false), "after-delta recorded");
        assert!(row.before.tokens > 2500, "before preserved");
    }
```

- [ ] **Step 2: Run it** — should pass directly against the Task-6/7 handler. If it fails, debug the reconcile/auto-close path (this is the integration proof that the unit-level reconcile sandwich holds through the real tracker round-trip).

Run: `cargo test --lib librarian::tools::legibility_scan::tests::end_to_end_scan_creates_then_auto_closes`
Expected: PASS.

- [ ] **Step 3: Full gate** (triage clippy — fix ONLY `src/legibility/` and `src/librarian/tools/legibility_scan/`; a concurrent session edits other `src/librarian/*` files):

```bash
cargo test --lib legibility:: && cargo test --lib librarian::tools::legibility_scan && cargo clippy --lib -- -D warnings 2>&1
```
Expected: green. If clippy fails ONLY in files outside `src/legibility/` and `src/librarian/tools/legibility_scan/`, record them and proceed (out of scope). Then `cargo fmt --check 2>&1 | grep -E 'legibility' || echo CLEAN`; if it flags the new files, `rustfmt --edition 2021 <those files>` (never workspace `cargo fmt`).

- [ ] **Step 4: Commit**

```bash
git add src/librarian/tools/legibility_scan/mod.rs
git commit -m "test(legibility): end-to-end scan → backlog → auto-close on refactor"
```

---

## Self-Review

**Spec coverage** (against `2026-06-13-dzo-friction-probes-design.md` § "Deliverable 2", "The tracker", "Reconcile", "Error handling", "Testing"):
- Action `legibility_scan` with `project`/`write`/`limit` → Tasks 1, 6, 8. ✓
- Recorder + index lanes wired (reuse 2a) → Task 6. ✓
- Scorer/gate/tiers (reuse 2a `score_and_rank`) + per-key grouping with **defects array** → Task 2. ✓
- Tracker as augmented artifact (params machine, body Dzo, render_template) → Task 5. ✓
- Reconcile: open→update, new→insert, **open→closed with before→after delta** (auto-close), closed retained → Task 4 (unit sandwich) + Task 9 (e2e). ✓
- `merge=true` always (never the `merge=false` foot-gun) → Task 5 `write_backlog`. ✓
- Error handling: missing db (graceful), bad project (RecoverableError), tracker failure isolated → Task 7. ✓
- F-1 `project_root` scoping → inherited from 2a `recorder_lane` (already regression-tested in 2a); Task 6 passes `project_root`. ✓
- No-line-count for un-mappable → inherited from 2a `un_mappable_files` (already tested). ✓
- Prompt-surface review → Task 8. ✓

**Decisions encoded:** defects-array (Task 2); retries deferred — `Cost` has no `retries` field, `Friction.retries` stays 0 (Task 4).

**Out of scope (per spec):** retry-chain detection, grep-roulette, CI-gating, cross-project rollups, the Dzo SKILL one-line pointer (a thin UX wrapper added separately). `limit` caps the dry-run / ranked head; backlog history is never dropped.

**Placeholder scan:** the three "scout note" callouts (ToolContext/RecoverableError import path; the ctx-with-temp-project test harness; the MiniJinja filter fallback) are genuine in-repo lookups naming the exact source file to copy from, not content gaps. The `tests_helpers` vs inline-fixture choice in Task 4 is an explicit either/or with both paths specified. All code blocks are complete.

**Type consistency:** `BacklogParams`/`CandidateRow`/`Measure`/`Cost`/`ScanMeta`/`GroupedCandidate` defined in Tasks 2-4 and used identically in Tasks 5-9; `group_by_key`/`reconcile`/`ensure_tracker`/`load_backlog`/`write_backlog`/`build_dry_run`/`measure_target`/`now_date`/`git_head`/`defect_str`/`defect_rank` signatures stable across defining and calling tasks.
