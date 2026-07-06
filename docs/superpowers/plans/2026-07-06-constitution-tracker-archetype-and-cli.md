# Constitution Tracker — Archetype + CLI Query (codescout) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the `constitution` tracker archetype to codescout's archetype library, and a fast, read-only `codescout constitution-check --path <path>` CLI subcommand that a companion-plugin hook (a separate plan, `docs/superpowers/plans/2026-07-06-constitution-tracker-companion-hooks.md`) will shell into for mechanical, path-conditional rule injection.

**Architecture:** `constitution` is data-shape-only, like the other 7 archetypes — it doesn't self-enable enforcement. A tracker becomes enforceable by (1) using this archetype's shape (`params.rules` + `entry_collection: "rules"`) and (2) carrying the tag `"constitution"` in its frontmatter `tags` — that tag is how the CLI query finds it, since "archetype" is a design-time template, not a persisted, queryable artifact field. The CLI subcommand queries the catalog directly (bypassing the MCP tool layer's buffering, since it must be fast and synchronous for a hook) and glob-matches each active rule's `paths` against the given path.

**Tech Stack:** Rust, the existing `src/librarian/catalog/find.rs` + `augmentation.rs`, `globset` (already a dependency, used elsewhere for path matching — see `src/librarian/tools/audit_doc_refs/severity.rs::matches_memory`), `clap` for the CLI subcommand.

## Global Constraints

- `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` (not `--lib`) must all pass before any task is done.
- `librarian` is a default Cargo feature — no `--features` flag needed.
- New errors use `RecoverableError`, never `anyhow::bail!`, in any MCP-facing code path. The CLI subcommand itself returns plain `anyhow::Result<()>` like `doctor.rs`/`audit_doc_refs.rs` — CLI commands aren't MCP tools and don't go through `RecoverableError`'s `isError: false` convention.
- Path-less (global) rules are explicitly out of scope for the CLI subcommand built here — they're a different injection channel (`UserPromptSubmit`, handled entirely in the companion-hooks plan) and must never be returned by `find_matching_rules`.

---

### Task 1: Add the `constitution` archetype

**Files:**
- Modify: `src/librarian/tools/tracker_design.rs`
  - `archetypes()` (lines 29–39) — register the new archetype
  - `SYSTEM_PROMPT` (lines 379–481) — bump the archetype count, add a decision-sketch bullet
  - `tests::goal_archetype_present_and_registered` (lines 739–748) — its hardcoded count of 7 becomes stale
- Test: same file's `mod tests` block (starts line 531)

**Interfaces:**
- Produces: `fn archetype_constitution() -> Value` (private to this file, mirrors `archetype_failure_table`) returning an object with keys `name`, `when_to_use`, `params_shape_example`, `params_schema_example`, `render_template_example`, `body_skeleton`, `prompt_template`, `entry_collection`. This is the exact shape every other `archetype_*` function returns — nothing new is introduced.

- [ ] **Step 1: Write the failing tests**

Add to the `mod tests` block in `src/librarian/tools/tracker_design.rs` (after `failure_table_archetype_has_entry_collection_field`, which this mirrors):

```rust
#[test]
fn constitution_archetype_has_entry_collection_field() {
    let v = archetype_constitution();
    assert_eq!(
        v["entry_collection"].as_str(),
        Some("rules"),
        "constitution archetype must advertise entry_collection = \"rules\""
    );
}

#[tokio::test]
async fn constitution_archetype_present_and_registered() {
    let v = archetypes();
    let arr = v.as_array().unwrap();
    let names: Vec<&str> = arr.iter().map(|a| a["name"].as_str().unwrap()).collect();
    assert!(
        names.contains(&"constitution"),
        "constitution archetype missing from archetypes() — got {names:?}"
    );
}
```

Then update the now-stale count assertion in the existing `goal_archetype_present_and_registered` test (line ~745):

```rust
assert_eq!(arr.len(), 7, "expected 7 archetypes including goal");
```
to:
```rust
assert_eq!(arr.len(), 8, "expected 8 archetypes including goal and constitution");
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test constitution_archetype`
Expected: FAIL to compile — `archetype_constitution` is not yet defined.

Run: `cargo test goal_archetype_present_and_registered`
Expected: FAIL — `assertion 'left == right' failed / left: 7, right: 8` (still 7 archetypes registered at this point).

- [ ] **Step 3: Implement `archetype_constitution` and register it**

Add this function to `src/librarian/tools/tracker_design.rs`, near `archetype_failure_table` (it can go anywhere among the sibling `archetype_*` functions — e.g. right after `archetype_goal`, lines 286–377):

```rust
fn archetype_constitution() -> Value {
    json!({
        "name": "constitution",
        "when_to_use": "Rules the agent MUST follow no matter what, enforced mechanically rather than by prose trust — not a place for advisory/'should' guidance (use a regular tracker or memory for that). Tag the tracker artifact's `tags` with `\"constitution\"` so codescout-companion's enforcement hooks can find it — the archetype shape alone does not enable enforcement. Examples: 'solver invariants (path-scoped)', 'never commit secrets (global, no paths)'.",
        "params_shape_example": {
            "rules": [
                {
                    "id": "C-1",
                    "paths": ["**/solver/**", "**/*Constraint*.kt"],
                    "title": "Never disable a constraint via weight 0",
                    "rule": "A constraint_profiles weight of 0 or 1 is a sentinel, not disabled — read the lambda before touching it.",
                    "status": "active"
                },
                {
                    "id": "C-2",
                    "title": "Never commit secrets",
                    "rule": "Never stage .env, credentials.json, or any file matching *_key*/*_secret* without explicit user confirmation.",
                    "status": "active"
                }
            ]
        },
        "params_schema_example": {
            "type": "object",
            "required": ["rules"],
            "properties": {
                "rules": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["id", "title", "rule", "status"],
                        "properties": {
                            "id":     { "type": "string", "pattern": "^C-\\d+$" },
                            "paths":  { "type": "array", "items": { "type": "string" } },
                            "title":  { "type": "string" },
                            "rule":   { "type": "string" },
                            "status": { "type": "string", "enum": ["active", "superseded"] }
                        }
                    }
                }
            }
        },
        "render_template_example": "**Active rules:** {{ rules|selectattr(\"status\",\"equalto\",\"active\")|list|length }} / {{ rules|length }}\n\n| id | scope | title |\n|----|-------|-------|\n{% for r in rules %}| {{ r.id }} | {{ \"path-scoped\" if r.paths else \"global\" }} | {{ r.title }} |\n{% endfor %}",
        "body_skeleton": "## Why this constitution exists\n\n_What domain these rules guard, and why prose alone wasn't enough._\n\n## Per-rule detail\n\n_`## C-N` sections: why / how to apply / evidence._\n\n## History\n\n_### YYYY-MM-DD — <event>_",
        "prompt_template": "This tracker holds rules the agent must follow no matter what — single-tier, mechanically enforced (path-scoped rules via a PreToolUse deny, global rules via a UserPromptSubmit injection), never prose-trust alone. To act: if a tool call was denied citing a C-N rule, read that rule's body section before retrying. To maintain: add new C-N entries via the `append_entry` primitive (never hand-pick the next integer — see docs/superpowers/specs/2026-07-06-librarian-atomic-index-allocation-design.md); never delete an entry — supersede a wrong one with status=superseded plus a pointer to its replacement. This artifact's `tags` must include `\"constitution\"` for the enforcement hooks to find it.",
        "entry_collection": "rules"
    })
}
```

Register it in `archetypes()` (line 29):

```rust
pub fn archetypes() -> Value {
    json!([
        archetype_deployment_state(),
        archetype_failure_table(),
        archetype_metric_baseline(),
        archetype_audit_issues(),
        archetype_task_list(),
        archetype_reflective(),
        archetype_goal(),
        archetype_constitution(),
    ])
}
```

In `SYSTEM_PROMPT`, change:
```
Match the user's intent to one of the 7 archetypes. Use this decision sketch:
```
to:
```
Match the user's intent to one of the 8 archetypes. Use this decision sketch:
```
and add a new bullet to the decision sketch list (after the `reflective` bullet):
```
- **Does it hold rules the agent must follow no matter what, mechanically enforced rather than just documented?** → `constitution`. Remember to tag the artifact `"constitution"` — the archetype shape alone doesn't enable enforcement.
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test constitution_archetype`
Expected: PASS

Run: `cargo test archetype`
Expected: PASS (all archetype-related tests, including the now-updated `goal_archetype_present_and_registered`, `each_archetype_self_consistent`, and `each_archetype_template_renders_against_example_params` — the latter two are generic over `archetypes()` and will validate `constitution`'s example params against its own schema and template automatically)

- [ ] **Step 5: Commit**

```bash
git add src/librarian/tools/tracker_design.rs
git commit -m "feat(librarian): add constitution tracker archetype"
```

---

### Task 2: `find_matching_rules` — catalog query + path glob-matching

**Files:**
- Create: `src/librarian/tools/constitution_check.rs`
- Modify: `src/librarian/tools/mod.rs` — add `pub mod constitution_check;` (this is a plain utility module, not an MCP-dispatched action, so it is **not** wired into `artifact.rs`'s action enum — only Task 3's CLI subcommand calls it directly)
- Test: same new file, `#[cfg(test)] mod tests`

**Interfaces:**
- Produces: `pub fn find_matching_rules(cat: &Catalog, path: &str) -> anyhow::Result<Vec<MatchedRule>>`, `pub fn find_global_rules(cat: &Catalog) -> anyhow::Result<Vec<MatchedRule>>`, and `pub struct MatchedRule { pub id: String, pub tracker_id: String, pub title: String, pub rule: String }` (all fields `pub`, `#[derive(Debug, Clone, serde::Serialize)]` on the struct so Task 3 can print either function's output as JSON directly). `find_global_rules` is the companion-hooks plan's dependency for its `UserPromptSubmit` channel — path-scoped and global rules are mutually exclusive by construction (a rule either has `paths` or it doesn't), so the two functions never return overlapping entries.
- Consumes: `crate::librarian::catalog::find::{find, FindOpts}`, `crate::librarian::catalog::augmentation::get`, `crate::librarian::catalog::Catalog` (all existing).

- [ ] **Step 1: Write the failing tests**

Create `src/librarian/tools/constitution_check.rs`:

```rust
use crate::librarian::catalog::{augmentation, find, Catalog};
use anyhow::Result;
use serde_json::Value;

#[derive(Debug, Clone, serde::Serialize)]
pub struct MatchedRule {
    pub id: String,
    pub tracker_id: String,
    pub title: String,
    pub rule: String,
}

/// Finds active, path-scoped constitution rules whose `paths` glob-match
/// `path`. Path-less (global) rules are never returned — they're surfaced
/// through a different channel (UserPromptSubmit), not this one.
pub fn find_matching_rules(cat: &Catalog, path: &str) -> Result<Vec<MatchedRule>> {
    let opts = find::FindOpts {
        filter: Some(serde_json::from_value(serde_json::json!({
            "and": [
                {"kind": {"eq": "tracker"}},
                {"status": {"eq": "active"}},
                {"tags": {"contains": "constitution"}}
            ]
        }))?),
        limit: 500,
        offset: 0,
    };
    let trackers = find::find(cat, &opts)?;
    let target = std::path::Path::new(path);

    let mut matches = Vec::new();
    for t in trackers {
        let Some(aug) = augmentation::get(cat, &t.id)? else {
            continue;
        };
        if aug.entry_collection.as_deref() != Some("rules") {
            continue;
        }
        let params: Value = serde_json::from_str(&aug.params).unwrap_or(Value::Null);
        let Some(rules) = params.get("rules").and_then(|v| v.as_array()) else {
            continue;
        };
        for r in rules {
            if r.get("status").and_then(|v| v.as_str()) != Some("active") {
                continue;
            }
            let Some(pattern_strs) = r.get("paths").and_then(|v| v.as_array()) else {
                continue; // global rule — not this channel's concern
            };
            let mut builder = globset::GlobSetBuilder::new();
            for p in pattern_strs {
                if let Some(s) = p.as_str() {
                    if let Ok(g) = globset::Glob::new(s) {
                        builder.add(g);
                    }
                }
            }
            let is_match = builder
                .build()
                .map(|set| set.is_match(target))
                .unwrap_or(false);
            if is_match {
                matches.push(MatchedRule {
                    id: r.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    tracker_id: t.id.clone(),
                    title: r
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    rule: r.get("rule").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                });
            }
        }
    }
    Ok(matches)
}

/// Finds active, path-less (global) constitution rules — the companion of
/// `find_matching_rules`. A rule with no `paths` field is always relevant
/// regardless of what's being touched (e.g. "never commit secrets"); it is
/// surfaced through a session-level channel (UserPromptSubmit), not a
/// tool-call-targeted one, so this function ignores `paths` entirely and
/// returns every active rule that *lacks* it.
pub fn find_global_rules(cat: &Catalog) -> Result<Vec<MatchedRule>> {
    let opts = find::FindOpts {
        filter: Some(serde_json::from_value(serde_json::json!({
            "and": [
                {"kind": {"eq": "tracker"}},
                {"status": {"eq": "active"}},
                {"tags": {"contains": "constitution"}}
            ]
        }))?),
        limit: 500,
        offset: 0,
    };
    let trackers = find::find(cat, &opts)?;

    let mut matches = Vec::new();
    for t in trackers {
        let Some(aug) = augmentation::get(cat, &t.id)? else {
            continue;
        };
        if aug.entry_collection.as_deref() != Some("rules") {
            continue;
        }
        let params: Value = serde_json::from_str(&aug.params).unwrap_or(Value::Null);
        let Some(rules) = params.get("rules").and_then(|v| v.as_array()) else {
            continue;
        };
        for r in rules {
            if r.get("status").and_then(|v| v.as_str()) != Some("active") {
                continue;
            }
            if r.get("paths").and_then(|v| v.as_array()).is_some() {
                continue; // path-scoped — not this channel's concern
            }
            matches.push(MatchedRule {
                id: r.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                tracker_id: t.id.clone(),
                title: r
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                rule: r.get("rule").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            });
        }
    }
    Ok(matches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{upsert as art_upsert, ArtifactRow};
    use crate::librarian::catalog::augmentation::{upsert as aug_upsert, AugmentationRow};

    fn sample_art(id: &str, tags: Vec<String>) -> ArtifactRow {
        let now = chrono::Utc::now().timestamp_millis();
        ArtifactRow {
            id: id.to_string(),
            abs_path: std::path::PathBuf::from(format!("/test/{id}.md")),
            kind: "tracker".to_string(),
            status: "active".to_string(),
            title: Some("T".to_string()),
            owners: vec![],
            tags,
            topic: None,
            time_scope: None,
            source: None,
            created_at: now,
            updated_at: now,
            file_mtime: now,
            file_sha256: "x".to_string(),
            confidence: 1.0,
        }
    }

    fn aug(id: &str, params: &str) -> AugmentationRow {
        AugmentationRow {
            artifact_id: id.to_string(),
            prompt: "test".to_string(),
            params: params.to_string(),
            last_refreshed_at: None,
            refresh_count: 0,
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
            render_template: None,
            params_schema: None,
            append_mode: false,
            history_cap: None,
            entry_collection: Some("rules".to_string()),
            refreshed_at_commit: None,
        }
    }

    #[test]
    fn matches_path_scoped_rule_on_glob_hit() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("c1", vec!["constitution".to_string()])).unwrap();
        aug_upsert(
            &cat,
            &aug(
                "c1",
                r#"{"rules":[{"id":"C-1","paths":["**/solver/**"],"title":"T","rule":"R","status":"active"}]}"#,
            ),
        )
        .unwrap();

        let hits = find_matching_rules(&cat, "src/solver/PinningEngine.kt").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "C-1");
        assert_eq!(hits[0].tracker_id, "c1");
    }

    #[test]
    fn skips_non_matching_path() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("c1", vec!["constitution".to_string()])).unwrap();
        aug_upsert(
            &cat,
            &aug(
                "c1",
                r#"{"rules":[{"id":"C-1","paths":["**/solver/**"],"title":"T","rule":"R","status":"active"}]}"#,
            ),
        )
        .unwrap();

        let hits = find_matching_rules(&cat, "src/ui/Button.kt").unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn skips_global_path_less_rules() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("c1", vec!["constitution".to_string()])).unwrap();
        aug_upsert(
            &cat,
            &aug(
                "c1",
                r#"{"rules":[{"id":"C-2","title":"Never commit secrets","rule":"R","status":"active"}]}"#,
            ),
        )
        .unwrap();

        let hits = find_matching_rules(&cat, "anything.txt").unwrap();
        assert!(hits.is_empty(), "global rules must never surface via path matching");
    }

    #[test]
    fn skips_superseded_rules() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("c1", vec!["constitution".to_string()])).unwrap();
        aug_upsert(
            &cat,
            &aug(
                "c1",
                r#"{"rules":[{"id":"C-1","paths":["**/solver/**"],"title":"T","rule":"R","status":"superseded"}]}"#,
            ),
        )
        .unwrap();

        let hits = find_matching_rules(&cat, "src/solver/x.kt").unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn skips_trackers_without_constitution_tag() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("c1", vec!["some-other-tag".to_string()])).unwrap();
        aug_upsert(
            &cat,
            &aug(
                "c1",
                r#"{"rules":[{"id":"C-1","paths":["**/solver/**"],"title":"T","rule":"R","status":"active"}]}"#,
            ),
        )
        .unwrap();

        let hits = find_matching_rules(&cat, "src/solver/x.kt").unwrap();
        assert!(hits.is_empty(), "trackers not tagged `constitution` must never match");
    }

    #[test]
    fn find_global_rules_returns_only_path_less_rules() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("c1", vec!["constitution".to_string()])).unwrap();
        aug_upsert(
            &cat,
            &aug(
                "c1",
                r#"{"rules":[
                    {"id":"C-1","paths":["**/solver/**"],"title":"T1","rule":"R1","status":"active"},
                    {"id":"C-2","title":"Never commit secrets","rule":"R2","status":"active"}
                ]}"#,
            ),
        )
        .unwrap();

        let hits = find_global_rules(&cat).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "C-2");
    }

    #[test]
    fn find_global_rules_skips_superseded_and_untagged() {
        let cat = Catalog::open_in_memory().unwrap();
        art_upsert(&cat, &sample_art("c1", vec!["constitution".to_string()])).unwrap();
        aug_upsert(
            &cat,
            &aug(
                "c1",
                r#"{"rules":[{"id":"C-2","title":"T","rule":"R","status":"superseded"}]}"#,
            ),
        )
        .unwrap();
        art_upsert(&cat, &sample_art("c2", vec!["some-other-tag".to_string()])).unwrap();
        aug_upsert(
            &cat,
            &aug(
                "c2",
                r#"{"rules":[{"id":"C-3","title":"T","rule":"R","status":"active"}]}"#,
            ),
        )
        .unwrap();

        let hits = find_global_rules(&cat).unwrap();
        assert!(hits.is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test constitution_check`
Expected: FAIL to compile — the module doesn't exist / isn't registered yet.

- [ ] **Step 3: Register the module**

In `src/librarian/tools/mod.rs`, add near the other single-purpose tool modules (e.g. after `pub mod doctor;` at line 181):

```rust
pub mod constitution_check;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test constitution_check`
Expected: PASS (all five tests)

- [ ] **Step 5: Commit**

```bash
git add src/librarian/tools/constitution_check.rs src/librarian/tools/mod.rs
git commit -m "feat(librarian): add constitution-rule path matching"
```

---

### Task 3: `codescout constitution-check --path <path>` CLI subcommand

**Files:**
- Create: `src/cli/constitution_check.rs`
- Modify: `src/cli/mod.rs` — add `pub mod constitution_check;` (after line 20's `pub mod doctor;`)
- Modify: `src/main.rs` — add a `ConstitutionCheck` variant to `Commands` and its dispatch arm

**Interfaces:**
- Consumes: `constitution_check::{find_matching_rules, find_global_rules}` (Task 2), `crate::cli::{open_ctx, CommonOpts}` (existing, see `src/cli/doctor.rs::run` for the identical pattern).
- Produces: a CLI subcommand printing a compact JSON array of `MatchedRule` objects to stdout, one line, always exit 0 (a companion-plugin hook shelling into this must never see a nonzero exit crash a hook pipeline — errors degrade to an empty array, matching the spec's "hook failures never block the tool" requirement). `--path <path>` selects path-scoped mode (`find_matching_rules`); omitting `--path` selects global mode (`find_global_rules`) — this is the single query surface both companion-plugin hooks (`PreToolUse` and `UserPromptSubmit`) depend on.

- [ ] **Step 1: Write the failing test**

Create `src/cli/constitution_check.rs`:

```rust
//! `codescout constitution-check [--path <path>]` — read-only, fast query for
//! codescout-companion's hooks. With `--path`, returns path-scoped rules
//! matching that path (for the PreToolUse hook); without it, returns global
//! (path-less) rules (for the UserPromptSubmit hook). Always exits 0; on any
//! internal error, prints `[]` rather than failing, so a broken query
//! degrades to "no injection" instead of blocking the caller.

use crate::cli::{open_ctx, CommonOpts};
use clap::Args;

#[derive(Debug, Args)]
pub struct ConstitutionCheckArgs {
    /// Project root override. Defaults to current working directory.
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,

    /// The file path a tool is about to touch. Omit to query global
    /// (path-less) rules instead of path-scoped ones.
    #[arg(long)]
    pub path: Option<String>,
}

pub async fn run(args: ConstitutionCheckArgs) {
    let common = CommonOpts {
        project: args.project.clone(),
        ..Default::default()
    };
    let hits = match open_ctx(&common).await {
        Ok(ctx) => {
            let cat = ctx.catalog.lock();
            match &args.path {
                Some(p) => {
                    crate::librarian::tools::constitution_check::find_matching_rules(&cat, p)
                        .unwrap_or_default()
                }
                None => crate::librarian::tools::constitution_check::find_global_rules(&cat)
                    .unwrap_or_default(),
            }
        }
        Err(_) => Vec::new(),
    };
    println!("{}", serde_json::to_string(&hits).unwrap_or_else(|_| "[]".to_string()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn run_never_panics_on_a_project_with_no_constitution_trackers() {
        let dir = tempfile::tempdir().unwrap();
        run(ConstitutionCheckArgs {
            project: Some(dir.path().to_path_buf()),
            path: Some("src/solver/x.kt".to_string()),
        })
        .await;
        // No assertion beyond "did not panic" — this is a smoke test for the
        // always-degrade-gracefully contract `find_matching_rules` already
        // covers in detail (see src/librarian/tools/constitution_check.rs).
    }

    #[tokio::test]
    async fn run_never_panics_in_global_mode() {
        let dir = tempfile::tempdir().unwrap();
        run(ConstitutionCheckArgs {
            project: Some(dir.path().to_path_buf()),
            path: None,
        })
        .await;
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test constitution_check::run_never_panics`
Expected: FAIL to compile — `crate::cli::constitution_check` doesn't exist / isn't registered, and `Commands::ConstitutionCheck` doesn't exist yet.

- [ ] **Step 3: Register the CLI module and wire the subcommand**

In `src/cli/mod.rs`, add after line 20 (`pub mod doctor;`):

```rust
pub mod constitution_check;
```

In `src/main.rs`, add a new `Commands` variant after `Doctor(codescout::cli::doctor::DoctorArgs),` (the last variant in the enum):

```rust
/// Read-only query: which active constitution rules apply to a given
/// path. Used by codescout-companion's PreToolUse hook — not meant for
/// interactive use. Always exits 0; prints `[]` on any internal error.
#[cfg(feature = "librarian")]
ConstitutionCheck(codescout::cli::constitution_check::ConstitutionCheckArgs),
```

Add the dispatch arm after the existing `Commands::Doctor(args) => { ... }` block (around line 372):

```rust
#[cfg(feature = "librarian")]
Commands::ConstitutionCheck(args) => {
    codescout::cli::constitution_check::run(args).await;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test constitution_check`
Expected: PASS (all tests from Task 2 and this task)

- [ ] **Step 5: Manual smoke test**

Run: `cargo run --quiet -- constitution-check --path src/solver/x.kt --project .`
Expected: prints `[]` (no constitution trackers exist in codescout's own catalog yet) and exits 0.

Run: `cargo run --quiet -- constitution-check --project .`
Expected: prints `[]` (global mode, same reason) and exits 0.

- [ ] **Step 6: Run the full verification gate**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: all three pass with no warnings or failures.

- [ ] **Step 7: Commit**

```bash
git add src/cli/constitution_check.rs src/cli/mod.rs src/main.rs
git commit -m "feat(cli): add constitution-check subcommand for hook consumption"
```

---

## Self-Review Notes

- **Spec coverage:** the spec's `constitution` archetype (`params.rules`, `entry_collection: "rules"`, single-tier enforcement) is Task 1. The "paths absent = global, never matched by the path-scoped matcher" requirement is directly tested (`skips_global_path_less_rules`, Task 2) — and its inverse (`find_global_rules` never returns path-scoped rules) is tested by `find_global_rules_returns_only_path_less_rules`. The CLI subcommand's "hook failures never block the tool" error-handling requirement is Task 3's `Err(_) => Vec::new()` fallback plus its smoke tests (both modes).
- **Amendment (added after drafting the companion-hooks plan):** the spec's `UserPromptSubmit` channel needs a way to fetch global (path-less) rules, which the original draft of this plan didn't provide — `find_matching_rules` explicitly filters them out and there was no counterpart. Added `find_global_rules` (Task 2) and made `ConstitutionCheckArgs.path` optional so one CLI subcommand serves both companion-plugin hooks (Task 3), rather than needing a second subcommand.
- **Design decision not fully specified in the spec, resolved here:** the spec doesn't say how a companion-plugin hook (which can't easily know "is this tracker's archetype constitution?" — archetype is a design-time template, not a persisted field) finds constitution trackers. This plan resolves it via a `"constitution"` tag convention, documented in the archetype's own `when_to_use` and `prompt_template` text so a human or LLM authoring one is told to tag it. Flagging this explicitly since it's a plan-level decision, not one the user approved during brainstorming.
- **Type consistency:** `MatchedRule`'s fields (`id`, `tracker_id`, `title`, `rule`) are identical across Task 2 (definition, both functions) and Task 3 (consumption via `serde_json::to_string(&hits)`) — no renaming drift.
- **Dependency on this plan:** `docs/plans/2026-07-06-constitution-tracker-hooks.md` (codescout-companion repo) shells into the `constitution-check` binary this plan produces — that plan cannot be executed before this one lands.
