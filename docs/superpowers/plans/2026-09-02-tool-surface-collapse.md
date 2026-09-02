# Tool Surface Collapse Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Collapse 26 MCP tools to 21: rename `artifact` to `doc` and fold `artifact_event`, `artifact_augment` and `artifact_refresh` into it as actions; fold `read_markdown` into `read_file` and `edit_markdown` into `edit_file`; hard cut, no aliases.

**Architecture:** `Doc::call` stays the one `match action` dispatcher in `src/librarian/tools/artifact.rs` and gains five arms that call the module functions the three retired tools already own, flattening two nested objects (`event`, `augment`) into the flat `Args` those modules expect. `ReadFile::call` and `EditFile::call` replace their `.md` refusals with a route into `markdown::read` / `markdown::edit`, which are the retired tools' bodies with the `impl Tool` wrapper removed. Every name-keyed list a rename would orphan is either updated or replaced by a trait method (`Tool::description_cap`).

**Tech Stack:** Rust (rmcp MCP server, serde_json, clap CLI), Python probe scripts, Node companion hooks in `../claude-plugins/codescout-companion`.

**Spec:** `docs/superpowers/specs/2026-09-02-tool-surface-collapse-design.md`

## Global Constraints

- **Worktree.** All work on branch `tool-collapse` in `.worktrees/tool-collapse`, cut from `experiments`. First call in every session there: `workspace(action="activate", path="/home/marius/work/claude/codescout/.worktrees/tool-collapse")` — the companion's worktree guard refuses writes otherwise.
- **No ledger writes from the worktree.** No `append_entry`, no bug-file moves, no tracker-entry edits. Those happen post-merge from the main checkout (final section).
- **Hard cut.** No alias for `artifact`, `artifact_event`, `artifact_augment`, `artifact_refresh`, `read_markdown`, `edit_markdown` — not in the registry, not in `call_tool_inner`, not in the CLI.
- **Internals keep their names.** Rust modules (`artifact.rs`, `augment.rs`), types (`Artifact`, `ArtifactRow`), the SQL table `artifact`, and every file under `docs/issues/archive/`, `docs/trackers/archive/`, past session-log entries and superseded specs are not edited.
- **Gate, in this order, chained with `;` never `&&`:** `cargo fmt` ; `cargo clippy --workspace --all-targets --features local-embed -- -D warnings` ; `cargo test --workspace --no-default-features` ; `cargo test --workspace`. Run from the worktree root. Per `CLAUDE.md` § Development Commands.
- **Budgets are measured, never estimated.** `TOOL_SURFACE_CHAR_BUDGET` (`src/server.rs`) is set to the number `cargo test --lib tool_surface_report_lengths -- --nocapture` prints, after all tools are removed. `STATIC_SLICE_CHAR_BUDGET` stays 1900; `ONBOARDING_VERSION` (`src/tools/onboarding.rs:32`) goes 29 → 30.
- **Description caps:** 300 chars default, 1,800 for `doc` and `librarian`, expressed by `Tool::description_cap()` after Task 1.
- **TDD per task.** Write the failing test, run it red, implement, run it green, commit. Every new guard is mutation-tested once and the mutation is named in its step.
- **Commit messages** follow the repo convention (`feat(scope): …`, `refactor(scope): …`, `docs(scope): …`, imperative, lowercase after the colon). Commits are on the worktree branch only.

**Deviations from the spec, decided while planning (each small, each recorded):**

1. `src/usage/db.rs` keeps its `read_markdown` / `edit_markdown` error classifiers. They re-classify *historical* `error_msg` rows on backfill (`open_db`), so deleting them would silently reclassify pre-cutover data. They are dead for new rows and annotated as such. The spec said "loses"; this is "keeps, dead, labelled".
2. Iron Laws 4 and 5 are not renumbered. Laws 1–3 and 6 are cited by number across guides, hooks and trackers; the two slots become one line marking them retired and stating what replaced them.
3. The action-label completeness gate covers `doc` only. `librarian` has six params labelled by *fix mode* rather than action (`root`, `old_root`, `new_root`, `confirm`, `write`, `include_archived`); relabelling them is a `librarian` schema change the spec puts out of scope. The gate takes a tool list, so adding `librarian` later is one line.
4. The `required` ↔ `call()` probe is not registry-wide. A generic probe needs a per-tool minimal valid input for every tool, which is a test harness of its own. This plan pins the one measured defect both ways — `workspace`'s schema (Task 3 gate) and behaviour (Task 3 Step 5) — and leaves the registry-wide form as a follow-up named in the spec's *Revisit-when*.

---

## File structure

| file | responsibility after this plan |
|---|---|
| `src/tools/core/types.rs` | `Tool` trait gains `description_cap()` (Task 1) |
| `src/librarian/tools/artifact.rs` | the `doc` tool: name, description, 17-action schema, dispatcher, two flatten helpers, probe tests (Tasks 2–6) |
| `src/librarian/tools/event_create.rs`, `timeline.rs`, `augment.rs`, `refresh.rs`, `refresh_stale.rs` | unchanged module functions; `augment.rs` loses its `impl Tool` and struct (Task 5) |
| `src/librarian/tools/artifact_event.rs`, `artifact_refresh.rs` | **deleted** (Tasks 4, 6); their tests move into `artifact.rs` |
| `src/librarian/tools/mod.rs` | `all_tools()` shrinks to `Artifact`, `Librarian` |
| `src/librarian/tools/create.rs` | `AugmentSpec` gains `params_path` and refuses it (Task 5) |
| `src/librarian/adapter.rs` | `is_write` keyed on `doc` + actions; `librarian_compact_summary` keyed on `doc` (Task 2) |
| `src/tools/markdown/read_markdown.rs` | `pub(crate) async fn read(input, ctx)`, `pub(crate) fn format_read(result)`, `pub(crate) fn is_markdown_target(path, ctx)`; `ReadMarkdown` struct and `impl Tool` deleted (Task 7) |
| `src/tools/markdown/edit_markdown.rs` | `pub(crate) async fn edit(input, ctx)`, `pub(crate) const LONG_DOCS`; `EditMarkdown` deleted (Task 8) |
| `src/tools/markdown/mod.rs` | re-exports `read`, `edit`, `format_read`, `is_markdown_target` |
| `src/tools/read_file.rs` | route to `markdown::read`; schema gains `heading`, `headings`; `format_compact` dispatches on `format == "markdown"` (Task 7) |
| `src/tools/edit_file/mod.rs` | route to `markdown::edit`; union schema; mixed-grammar refusal; `long_docs` (Task 8) |
| `src/server.rs` | registry loses `EditMarkdown`, `ReadMarkdown`; tests: cap tests replaced, new schema gates, registry pin (Tasks 1, 3, 7, 13) |
| `src/prompts/source.md`, `builders.rs`, `mod.rs`, `guides/*.md` | Iron Laws 4/5 retired; `serves:` annotations; `DEPRECATED_TOOL_NAMES`; banner fix (Task 9) |
| `src/tools/onboarding.rs` | `ONBOARDING_VERSION` 30 (Task 9) |
| `src/main.rs`, `src/cli/{doc,doc_event,doc_refresh,doc_augment}.rs`, `tests/cli_doc.rs` | `codescout doc …` (Task 11) |
| `CLAUDE.md`, `docs/TAXONOMY.md`, `docs/PROGRESSIVE_DISCOVERABILITY.md`, `docs/PROBES.md`, `docs/manual/src/**`, `README.md`, `CONTRIBUTING.md`, `CHANGELOG.md`, `docs/architecture/companion-plugin.md` | docs sweep (Task 10) |
| `../claude-plugins/codescout-companion/**` | paired plugin change (Task 12) |

---

### Task 1: `Tool::description_cap()` replaces the name-prefix exemption

**Files:**
- Create: `.worktrees/tool-collapse` (git worktree)
- Modify: `src/tools/core/types.rs` (trait `Tool`, after `fn description`)
- Modify: `src/librarian/tools/artifact.rs:15-26`, `src/librarian/tools/librarian.rs` (the `impl Tool for Librarian` block)
- Modify: `src/server.rs:2438-2470` (`is_librarian_tool`, `tool_descriptions_stay_under_budget`, `tool_descriptions_report_lengths`), `src/server.rs:3006-3020` (`every_tool_description_under_cap`)

**Interfaces:**
- Produces: `fn description_cap(&self) -> usize` on `Tool`, default `300`; `Artifact` and `Librarian` return `1_800`.

- [ ] **Step 1: Create the worktree and activate it**

```bash
cd /home/marius/work/claude/codescout
git worktree add .worktrees/tool-collapse -b tool-collapse experiments
```

Then, as the first MCP call in the session: `workspace(action="activate", path="/home/marius/work/claude/codescout/.worktrees/tool-collapse")`. Every later step runs in that directory.

- [ ] **Step 2: Write the two failing tests in `src/server.rs` tests module**

Add next to `tool_descriptions_stay_under_budget`:

```rust
    /// One cap per tool, declared by the tool. Replaces the name-prefix list
    /// `is_librarian_tool`, which `artifact → doc` would have silently emptied:
    /// `doc` matches none of its prefixes, so the 300 cap would have applied and
    /// the test would have failed for the wrong reason — or, had the list been
    /// widened by reflex, passed for none. Characters, not bytes: the same unit
    /// `TOOL_SURFACE_CHAR_BUDGET` uses, for the same em-dash reason.
    #[tokio::test]
    async fn every_tool_description_is_under_its_cap() {
        let (_dir, server) = make_server().await;
        let over: Vec<String> = server
            .tools
            .iter()
            .filter(|t| t.description().chars().count() > t.description_cap())
            .map(|t| {
                format!(
                    "{} is {} chars, cap {}",
                    t.name(),
                    t.description().chars().count(),
                    t.description_cap()
                )
            })
            .collect();
        assert!(
            over.is_empty(),
            "descriptions over their cap:\n  {}",
            over.join("\n  ")
        );
    }

    /// The raised cap is a property of multi-action dispatchers, not a convenience.
    /// Pin the set so a third tool cannot raise its own cap without appearing here.
    #[tokio::test]
    async fn only_the_two_dispatchers_raise_the_description_cap() {
        let (_dir, server) = make_server().await;
        let mut raised: Vec<&str> = server
            .tools
            .iter()
            .filter(|t| t.description_cap() > 300)
            .map(|t| t.name())
            .collect();
        raised.sort();
        assert_eq!(raised, vec!["artifact", "librarian"]);
    }
```

- [ ] **Step 3: Run them red**

Run: `cargo test --lib every_tool_description_is_under_its_cap only_the_two_dispatchers_raise 2>&1`
Expected: compile error `no method named description_cap`.

- [ ] **Step 4: Add the trait method**

In `src/tools/core/types.rs`, directly after `fn description(&self) -> &str;`:

```rust
    /// Cap on `description()` length in **characters**, enforced by
    /// `server::tests::every_tool_description_is_under_its_cap`. 300 by default.
    /// A multi-action dispatcher whose description must name every action
    /// (`doc`, `librarian`) overrides to 1_800 — Claude Code truncates a tool
    /// description near 2,000 bytes (`docs/architecture/mcp-channel-caps.md`), so
    /// 1_800 leaves margin. This replaces `is_librarian_tool`, a name-prefix list
    /// in `server.rs` tests that a rename silently orphaned.
    fn description_cap(&self) -> usize {
        300
    }
```

In `src/librarian/tools/artifact.rs` inside `impl Tool for Artifact`, after `description()`:

```rust
    fn description_cap(&self) -> usize {
        1_800
    }
```

Same three lines inside `impl Tool for Librarian` in `src/librarian/tools/librarian.rs`.

- [ ] **Step 5: Delete the old exemption and tests; retarget the report**

In `src/server.rs` tests: delete `fn is_librarian_tool` (`:2438-2446`), `tool_descriptions_stay_under_budget` (`:2448-2463`) and `every_tool_description_under_cap` (`:3006-3020`). In `tool_descriptions_report_lengths`, replace any `is_librarian_tool(t.name())` use with `t.description_cap()` so the printed row shows `name`, `chars`, `cap`.

- [ ] **Step 6: Run green, then mutate once**

Run: `cargo test --lib description 2>&1`
Expected: both new tests PASS.

Mutation: change `Librarian`'s override to `300`, run `every_tool_description_is_under_its_cap` — Expected: FAIL naming `librarian is 1621 chars, cap 300`. Restore `1_800`.

- [ ] **Step 7: Commit**

```bash
git add src/tools/core/types.rs src/librarian/tools/artifact.rs src/librarian/tools/librarian.rs src/server.rs
git commit -m "refactor(tools): Tool::description_cap replaces the is_librarian_tool name-prefix list"
```

---

### Task 2: Rename `artifact` → `doc` (tool name, audit verb, name-keyed lists, hint strings)

**Files:**
- Modify: `src/librarian/tools/artifact.rs:11-13` (name), `:215-225` (audit verb), tests `:250-527` (probe labels)
- Modify: `src/librarian/adapter.rs:276`, `:466`
- Modify: `src/server.rs` — the `is_write` regression test near `:6292-6330`; the `find_tool("artifact")` test near `:2686`
- Modify: `tests/librarian/companion_hint.rs:10` (`REAL_TOOLS`)
- Modify: every `src/**/*.rs` file emitting an `artifact(` hint — list in Step 4

**Interfaces:**
- Produces: MCP tool name `"doc"`; audit verbs `doc.<action>`.

- [ ] **Step 1: Write the failing registry test** (`src/server.rs` tests, `#[cfg(feature = "librarian")]`)

```rust
    #[cfg(feature = "librarian")]
    #[tokio::test]
    async fn the_document_tool_is_named_doc() {
        let (_dir, server) = make_server().await;
        assert!(server.find_tool("doc").is_some(), "doc must be registered");
        assert!(
            server.find_tool("artifact").is_none(),
            "artifact was renamed to doc on 2026-09-02; nothing may register the old name"
        );
    }
```

- [ ] **Step 2: Run red**

Run: `cargo test --lib the_document_tool_is_named_doc 2>&1`
Expected: FAIL `doc must be registered`.

- [ ] **Step 3: Rename in `artifact.rs`**

`fn name(&self) -> &'static str { "doc" }`. In `call`, `set_audit_verb(&format!("doc.{action}"))`. In `description()`, replace the leading `Artifact CRUD and query.` with `Document CRUD and query.` (the full rewrite is Task 6 Step 6). In the tests module, every `"artifact"` first argument to `assert_all_honored` / `assert_required_are_advertised` becomes `"doc"`; in `dispatch_stamps_the_audit_verb` the expected verb becomes `doc.<action>`.

- [ ] **Step 4: Update the name-keyed lists**

`src/librarian/adapter.rs:276`: `"artifact" =>` → `"doc" =>`. `:466`: `let is_artifact = inner_name == "doc";`. `src/server.rs` `is_write` regression test (`:6292` onward): replace the literal `"artifact"` tool name with `"doc"` in every case it constructs. `:2686` `find_tool("artifact")` → `find_tool("doc")`. `tests/librarian/companion_hint.rs:10`: `"artifact",` → `"doc",`.

Now the hint strings. From the worktree root:

```bash
grep -rlE '\bartifact\((action=|get|find|create|update|move|delete|graft|link|graph|state_at|append_entry|update_entry)' src --include='*.rs' > /tmp/claude-1000/artifact-hint-files.txt
xargs -a /tmp/claude-1000/artifact-hint-files.txt sed -i -E 's/\bartifact\((action=|get|find|create|update|move|delete|graft|link|graph|state_at|append_entry|update_entry)/doc(\1/g'
grep -rnE '\bartifact\(' src --include='*.rs'
```

The final grep must print nothing. `\b` before `artifact` does not match `create_artifact(` or `is_librarian_artifact(` because `_` is a word character — check `git diff --stat` shows only string-literal lines changed (`git diff | grep '^[-+]' | grep -v 'doc(\|artifact(' | grep -c 'fn '` must be `0`).

- [ ] **Step 5: Build and run the librarian tests**

Run: `cargo test --lib librarian 2>&1`
Expected: PASS, including `src/util/librarian_guard.rs` tests whose hint assertions now read `doc(action=`.

- [ ] **Step 6: Update the dispatcher pin from Task 1 and run the full lib suite**

`only_the_two_dispatchers_raise_the_description_cap`: `vec!["doc", "librarian"]`.

Run: `cargo test --lib 2>&1`
Expected: PASS except `prompt_surfaces_reference_only_real_tools` and `tests/doc_tool_refs.rs`-class failures, which name `artifact(` in guides and docs. Those are Tasks 9 and 10; do not touch guides here.

- [ ] **Step 7: Commit**

```bash
git add -A src tests/librarian/companion_hint.rs
git commit -m "feat(librarian): rename the artifact tool to doc — name, audit verb, adapter classifier, hints"
```

---

### Task 3: `doc` schema quality — labels, descriptions, `patch`, `state_at`, and the four registry-wide gates

**Files:**
- Modify: `src/librarian/tools/artifact.rs:28-207` (schema), tests (`probe_required`)
- Modify: `src/librarian/tools/state_at.rs:157-162` (hint text)
- Modify: `src/tools/symbol/symbols.rs`, `src/tools/symbol/call_graph/mod.rs:372-374`, `src/tools/symbol/edit_code.rs`, `src/tools/memory/mod.rs`, `src/tools/semantic/semantic_search.rs`, `src/librarian/tools/librarian.rs:58`, `src/tools/semantic/index.rs:836`, `src/tools/config/mod.rs:46`
- Test: `src/server.rs` tests module (four new gates)

**Interfaces:**
- Produces: `doc` params `state_at` addressed by `id`; every property described; label grammar `<action>[/<action>]: <text>` or `all: <text>`.

- [ ] **Step 1: Write the four failing gates in `src/server.rs` tests**

```rust
    /// Every advertised property carries a description. A param with type and default
    /// only is one the model cannot choose to use. Measured 2026-09-02: 12 such params
    /// across 7 tools; this test listed them and each was described in the same change.
    #[tokio::test]
    async fn every_property_has_a_description() {
        let (_dir, server) = make_server().await;
        let mut blank = Vec::new();
        for t in &server.tools {
            let schema = t.input_schema();
            let Some(props) = schema.get("properties").and_then(|p| p.as_object()) else {
                continue;
            };
            for (name, def) in props {
                let has = def
                    .get("description")
                    .and_then(|d| d.as_str())
                    .is_some_and(|d| !d.trim().is_empty());
                if !has {
                    blank.push(format!("{}.{}", t.name(), name));
                }
            }
        }
        assert!(
            blank.is_empty(),
            "params with no description:\n  {}",
            blank.join("\n  ")
        );
    }

    /// `doc` says which action a param serves through a `<action>[/<action>]:` prefix, or
    /// `all:`. The honesty direction is already tested (labelled ⇒ honoured, in
    /// `artifact.rs`); this is completeness: every param carries a well-formed label and
    /// every action has at least one param labelled for it. Measured 2026-09-02 before the
    /// rewrite: `delete` had zero and four params had no label at all.
    #[cfg(feature = "librarian")]
    #[tokio::test]
    async fn every_doc_param_is_labelled_and_every_action_has_a_param() {
        let (_dir, server) = make_server().await;
        let doc = server.find_tool("doc").expect("doc is registered");
        let schema = doc.input_schema();
        let props = schema["properties"].as_object().unwrap();
        let actions: Vec<String> = props["action"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let mut unlabelled = Vec::new();
        let mut covered: std::collections::HashSet<String> = Default::default();
        for (name, def) in props {
            if name == "action" {
                continue;
            }
            let desc = def["description"].as_str().unwrap_or("");
            let label = desc.split(':').next().unwrap_or("");
            let tokens: Vec<&str> = label.split('/').map(str::trim).collect();
            let well_formed = !label.is_empty()
                && !label.contains(' ')
                && tokens
                    .iter()
                    .all(|t| *t == "all" || actions.iter().any(|a| a == t));
            if !well_formed {
                unlabelled.push(format!("{name}: {label:?}"));
                continue;
            }
            for t in tokens {
                if t == "all" {
                    covered.extend(actions.iter().cloned());
                } else {
                    covered.insert(t.to_string());
                }
            }
        }
        let uncovered: Vec<&String> = actions.iter().filter(|a| !covered.contains(*a)).collect();
        assert!(
            unlabelled.is_empty() && uncovered.is_empty(),
            "unlabelled params: {unlabelled:?}\nactions with no labelled param: {uncovered:?}"
        );
    }

    /// A description that enumerates its actions ("Actions: `a` …, `b` …") must name every
    /// value of the `action` enum. `index` described three of four for two weeks while
    /// `verify` was called 16 times. Scoped to descriptions that enumerate — `doc` and
    /// `librarian` describe by theme and are not held to this.
    #[tokio::test]
    async fn a_description_that_enumerates_actions_names_every_action() {
        let (_dir, server) = make_server().await;
        let mut missing = Vec::new();
        for t in &server.tools {
            let d = t.description();
            if !d.contains("Actions:") {
                continue;
            }
            let schema = t.input_schema();
            let Some(e) = schema["properties"]["action"]["enum"].as_array() else {
                continue;
            };
            for v in e {
                let v = v.as_str().unwrap();
                if !d.contains(&format!("`{v}`")) {
                    missing.push(format!("{}.{}", t.name(), v));
                }
            }
        }
        assert!(missing.is_empty(), "in the enum, absent from the description: {missing:?}");
    }

    /// `workspace` declared `action` required while its code accepted `post_compact` alone
    /// (31 of 120 such calls in 30 days). `required` is a claim about `call()`; pin the
    /// schema half here and the behaviour half in `config/mod.rs`.
    #[tokio::test]
    async fn workspace_does_not_require_action() {
        let (_dir, server) = make_server().await;
        let ws = server.find_tool("workspace").expect("workspace is registered");
        let schema = ws.input_schema();
        let required: Vec<&str> = schema["required"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();
        assert!(
            !required.contains(&"action"),
            "workspace accepts post_compact=true with no action; `action` must not be in required: {required:?}"
        );
        let d = schema["properties"]["action"]["description"].as_str().unwrap();
        assert!(d.contains("post_compact"), "action's description must say when it may be omitted: {d}");
    }
```

- [ ] **Step 2: Run red**

Run: `cargo test --lib every_property_has_a_description every_doc_param_is_labelled a_description_that_enumerates workspace_does_not_require 2>&1`
Expected: all four FAIL. The first lists 12 params; the second lists `include_archived`, `limit`, `offset`, `include_observations`, `anchor_heading` (its label contains a comma and space) and `delete` uncovered.

- [ ] **Step 3: Rewrite the `doc` schema properties named below** (`src/librarian/tools/artifact.rs`)

Replace these entries in `input_schema()`; leave every other property as it is (Tasks 4–6 add more):

```rust
                "action": {
                    "type": "string",
                    "enum": ["find", "get", "create", "update", "move", "delete", "graft", "link", "graph", "state_at", "append_entry", "update_entry"],
                    "description": "Operation to perform"
                },
                "id": {
                    "type": "string",
                    "description": "get/update/move/delete/graph/state_at/append_entry/update_entry: document id (16-hex). find and create take none."
                },
                "include_archived": {
                    "type": "boolean",
                    "default": false,
                    "description": "find: include archived and superseded rows, which the default scope hides."
                },
                "limit": {
                    "type": "integer",
                    "default": 50,
                    "maximum": 500,
                    "description": "find: max rows (default 50, max 500)."
                },
                "offset": {
                    "type": "integer",
                    "default": 0,
                    "maximum": 100000,
                    "description": "find: rows to skip for paging (default 0)."
                },
                "include_observations": {
                    "type": "boolean",
                    "default": false,
                    "description": "get: include observation rows recorded against the document (default false)."
                },
                "anchor_heading": {
                    "type": "string",
                    "description": "append_entry: prose ledgers — pass with `title` + `body` (all three or none; a partial set is refused naming what is missing) and the server writes `## <ID> — <title>` itself, before this heading, in the same write that records the high-water mark. Must name a heading that exists verbatim; a bad anchor writes nothing at all. Why prefer it over reserving an id: get_guide(\"tracker-conventions\") § Entry ids."
                },
                "commit": { "type": "string", "description": "state_at: git commit hash as time-travel cutoff. Exactly one of commit or timestamp." },
                "timestamp": { "type": "integer", "description": "state_at: unix epoch ms as time-travel cutoff. Exactly one of commit or timestamp." },
                "patch": {
                    "type": "object",
                    "description": "update: the fields to change. Accepted keys: status, title, owners, tags, topic, time_scope, extra, body, body_edits, params (any other key returns RecoverableError). Top-level status/title/owners/tags/topic/time_scope/extra are lifted into patch and reported under `corrections`; an update that changes nothing is refused. Body editing — three modes: (1) `body_edits`, edit_file's heading-addressed batch shape applied atomically, RECOMMENDED for tracker maintenance; (2) `body` for total overwrite, gated by the 50% shrink guard unless `force=true`; (3) frontmatter-only changes via the scalar keys. `body` and `body_edits` are mutually exclusive. `params` is RFC 7396 merge-patched into the augmentation params — arrays are REPLACED whole, so use update_entry to change one row. Body mutations emit `field_patch` events."
                },
```

Delete the `"artifact_id"` property entirely. In `call`, before the `match`, add the mapping so `state_at::Args` (which keeps its `artifact_id` field — internals stay) receives what it expects:

```rust
        // `state_at` predates the `id` convention and its Args still reads `artifact_id`.
        // The schema says `id` like every other action; translate here so the module's
        // field name stays an internal detail.
        let args = if action == "state_at" {
            let mut a = args;
            if let (Some(id), Some(obj)) = (a.get("id").cloned(), a.as_object_mut()) {
                obj.entry("artifact_id").or_insert(id);
            }
            a
        } else {
            args
        };
```

In `src/librarian/tools/state_at.rs:157-162`, the hint: `doc(action="state_at") requires 'id': {e}` and `e.g. doc(action="state_at", id="<16-hex>", commit="<sha>"). …`. In `artifact.rs` tests, `probe_required("state_at")` inserts `"id"` instead of `"artifact_id"`.

- [ ] **Step 4: Describe the twelve blank params**

| file | property | description to set |
|---|---|---|
| `src/tools/symbol/symbols.rs` | `include_body` | `Include each symbol's full source text in the result (default false).` |
| `src/tools/symbol/call_graph/mod.rs:372` | `direction` | add `"type": "string"`; `Traversal direction: callers (who reaches this symbol — blast radius, default), callees (what it reaches), or both.` |
| `src/tools/symbol/call_graph/mod.rs:374` | `detail_level` | `'exploring' (default): compact edge list. 'full': every node with its file and line.` |
| `src/tools/symbol/edit_code.rs` | `path` | `File containing the symbol. Required — a symbol name alone is not an address.` |
| `src/tools/symbol/edit_code.rs` | `action` | `replace: overwrite the symbol's body. insert: inject `body` before/after it (see position). remove: delete it. rename: rename it across the codebase via LSP (needs new_name).` |
| `src/tools/memory/mod.rs` | `action` | `read/write/list/delete: topic memories addressed by a path-like key. remember/recall/forget: the semantic store. refresh_anchors: re-resolve the code anchors a topic cites (needs topic).` |
| `src/tools/semantic/semantic_search.rs` | `limit` | `Max results (default 10).` |
| `src/librarian/tools/librarian.rs:58` | `include_archived` | `context/workspace_state_at: include archived rows (default false).` |

The four `doc` params were done in Step 3.

- [ ] **Step 5: `index` names `verify`; `workspace` stops requiring `action`**

`src/tools/semantic/index.rs:836`: append to the description, before the closing quote: `` , `verify` (check index coverage — report project files the store skipped) ``. Keep the whole description under 300 chars (it is 243 today; the addition is ~70, so trim the `cancel` clause to `` `cancel` (abort an in-flight reindex) `` if needed).

`src/tools/config/mod.rs:46`: delete the line `"required": ["action"]`. `:31-36` `action` description: `Operation to perform. Required unless post_compact=true, which implies status.`

In `src/tools/config/mod.rs`, in a `#[cfg(test)] mod tests` (add one if the file has none), constructing the context the way `src/tools/edit_file/tests.rs:152` `project_ctx` does:

```rust
    async fn project_ctx() -> (tempfile::TempDir, ToolContext) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        (
            dir,
            ToolContext {
                agent,
                lsp: crate::lsp::LspManager::new_arc(),
                output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
                progress: None,
                peer: None,
                section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                    crate::tools::section_coverage::SectionCoverage::new(),
                )),
                guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
                workspace_override: None,
            },
        )
    }

    #[tokio::test]
    async fn post_compact_alone_is_a_status_call_and_empty_input_is_refused() {
        let (_dir, ctx) = project_ctx().await;
        let ok = Workspace.call(json!({"post_compact": true}), &ctx).await;
        assert!(ok.is_ok(), "post_compact=true with no action must run status: {ok:?}");
        let err = Workspace.call(json!({}), &ctx).await;
        assert!(err.is_err(), "an empty workspace call must still be refused");
    }
```

(Copy the `use` lines `project_ctx` needs from `edit_file/tests.rs:1-39`. `Workspace::call` already implements exactly this at `config/mod.rs:51-58`; the test pins it.)

- [ ] **Step 6: Run green**

Run: `cargo test --lib 2>&1`
Expected: the four gates PASS; `every_required_param_is_advertised` and `every_action_labelled_schema_key_is_honored_by_that_action` PASS with `state_at` now probed by `id`.

Mutation for the label gate: change `id`'s label to `get/update/graph/append_entry:` — Expected: FAIL listing `delete`, `move`, `state_at`, `update_entry` as uncovered. Restore.

- [ ] **Step 7: Commit**

```bash
git add -A src
git commit -m "fix(doc): complete the action labels, describe every param, retire the stale patch text; four registry-wide schema gates"
```

---

### Task 4: Fold `artifact_event` into `doc` as `event_create` / `event_list`

**Files:**
- Modify: `src/librarian/tools/artifact.rs` (schema, dispatcher, helpers, tests)
- Modify: `src/librarian/tools/event_create.rs:57` (`ALLOWED_KINDS` visibility)
- Modify: `src/librarian/tools/mod.rs:378` (`pub mod artifact_event;`), `:396-404` (`all_tools`)
- Modify: `src/librarian/adapter.rs` (`is_write` doc arm)
- Modify: `tests/librarian/companion_hint.rs:11`
- Delete: `src/librarian/tools/artifact_event.rs`

**Interfaces:**
- Consumes: `event_create::call(ctx, flat_args)` expecting `artifact_id`, `kind`, `payload`, …; `timeline::call(ctx, args)` expecting `artifact_id`, `kinds`, `since`, `until`, `limit`.
- Produces: `doc(action="event_create", id, event={kind, payload, author?, anchor_commit?, head_commit?, parent_event_id?, resolves_intent_event_id?, also_mutates?, source?})`; `doc(action="event_list", id, kinds?, since?, until?, limit?)`.

- [ ] **Step 1: Write the failing tests in `artifact.rs` tests**

Extend the probe: `PROBE_ACTIONS` becomes `[&str; 14]` with `"event_create", "event_list"` appended; in `probe_required`:

```rust
            "event_list" => {
                m.insert("id".into(), json!(PROBE_NO_SUCH_ID));
            }
            "event_create" => {
                m.insert("id".into(), json!(PROBE_NO_SUCH_ID));
                m.insert("event".into(), json!({"kind": "note", "payload": {"text": "probe"}}));
            }
```

Move `every_required_payload_field_is_enforced_and_advertised` from `artifact_event.rs:235-283` into `artifact.rs` tests verbatim, changing its first statement to:

```rust
        let desc = Artifact.input_schema()["properties"]["event"]["properties"]["payload"]["description"]
```

Add a row-seeding helper and two routing tests:

```rust
    /// One catalog row, no file. `TestArtifactRowBuilder` is what `timeline.rs` tests use.
    fn seed_row(ctx: &ToolContext, id: &str) {
        use crate::librarian::catalog::artifact::{upsert, TestArtifactRowBuilder};
        let cat = ctx.catalog.lock();
        upsert(&cat, &TestArtifactRowBuilder::new(id).build()).unwrap();
    }

    #[tokio::test]
    async fn event_create_lifts_the_nested_event_and_event_list_reads_it_back() {
        let ctx = mk_ctx();
        let id = "aaaaaaaaaaaaaaaa";
        seed_row(&ctx, id);
        let created = Artifact
            .call(&ctx, json!({"action": "event_create", "id": id,
                               "event": {"kind": "note", "payload": {"text": "hello"}}}))
            .await
            .expect("event_create succeeds");
        assert!(created["event_id"].is_string(), "{created}");
        let listed = Artifact
            .call(&ctx, json!({"action": "event_list", "id": id}))
            .await
            .expect("event_list succeeds");
        let events = listed["items"].as_array().expect("items array");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["kind"], "note");
    }

    #[tokio::test]
    async fn event_create_without_an_event_object_is_refused_with_the_shape() {
        let err = Artifact
            .call(&mk_ctx(), json!({"action": "event_create", "id": "aaaaaaaaaaaaaaaa", "kind": "note"}))
            .await
            .unwrap_err();
        let re = err.downcast_ref::<RecoverableError>().expect("recoverable");
        assert!(re.hint().unwrap().contains("event={kind:"), "{re:?}");
    }
```

(`mk_ctx` exists at `artifact.rs:250`. `event_create::call` returns `{event_id, parent_event_id, anchor_commit, head_commit}`; `timeline::call` returns `{items, count, truncated}` — both read from the source, not guessed.)

- [ ] **Step 2: Run red**

Run: `cargo test --lib librarian::tools::artifact 2>&1`
Expected: FAIL — `unknown action 'event_create'`.

- [ ] **Step 3: Schema additions in `artifact.rs`**

`action` enum gains `"event_create", "event_list"`. `id`'s label gains `event_create/event_list`. `limit`'s description becomes `find: max rows (default 50, max 500). event_list: max events (default 50).` Add:

```rust
                "event": {
                    "type": "object",
                    "description": "event_create: the event to append — an immutable record anchored to git, distinct from a field patch. `kind` lives inside this object so it never shares a key with the document `kind`.",
                    "required": ["kind", "payload"],
                    "additionalProperties": false,
                    "properties": {
                        "kind": {
                            "type": "string",
                            "enum": super::event_create::ALLOWED_KINDS,
                            "description": "event kind"
                        },
                        "payload": {
                            "type": "object",
                            "description": format!("event payload (a JSON object). {}", super::event_create::payload_requirements_sentence())
                        },
                        "author": { "type": "string", "description": "event author" },
                        "anchor_commit": { "type": "string", "description": "git commit to anchor the event to" },
                        "head_commit": { "type": "string", "description": "HEAD commit at write time — pass it explicitly when the task produces no commit of its own" },
                        "parent_event_id": { "type": "string", "description": "parent event id for threading" },
                        "resolves_intent_event_id": { "type": "string", "description": "intent event id this verdict resolves" },
                        "also_mutates": { "type": "array", "items": { "type": "string" }, "description": "additional document ids mutated by this event" },
                        "source": {
                            "type": "object",
                            "description": "external signal source",
                            "properties": { "uri": { "type": "string" }, "kind": { "type": "string" }, "payload": {} },
                            "required": ["uri", "kind"]
                        }
                    }
                },
                "kinds": { "type": "array", "items": { "type": "string" }, "description": "event_list: filter to these event kinds" },
                "since": { "type": "integer", "format": "int64", "description": "event_list: return events after this ms epoch" },
                "until": { "type": "integer", "format": "int64", "description": "event_list: return events before this ms epoch" },
```

`src/librarian/tools/event_create.rs:57`: `pub(crate) const ALLOWED_KINDS`.

- [ ] **Step 4: Dispatcher arms and the flatten helper**

Above `impl Tool for Artifact` in `artifact.rs`:

```rust
/// `event_create` arrives as `{id, event: {kind, payload, …}}` so the event kind never
/// shares a key with the document `kind`. `event_create::Args` is flat and reads
/// `artifact_id`; lift the object and carry the id under that name.
fn flatten_event_args(args: &Value) -> Result<Value> {
    let id = args["id"].as_str().ok_or_else(|| {
        RecoverableError::with_hint(
            "doc(action=\"event_create\") requires 'id'",
            "e.g. doc(action=\"event_create\", id=\"<16-hex>\", event={kind: \"note\", payload: {text: \"…\"}})",
        )
    })?;
    let mut flat = match args.get("event") {
        Some(Value::Object(m)) => m.clone(),
        _ => {
            return Err(RecoverableError::with_hint(
                "doc(action=\"event_create\") requires an `event` object",
                "event={kind: <note|reviewed|status_change|field_patch|superseded_by|external_signal|intent|verdict>, payload: {…}}",
            )
            .into())
        }
    };
    flat.insert("artifact_id".into(), json!(id));
    Ok(Value::Object(flat))
}

/// `event_list` says `id`; `timeline::Args` reads `artifact_id`. Copy, don't rename the
/// module's field — internals keep their names.
fn id_as_artifact_id(args: &Value) -> Value {
    let mut a = args.clone();
    if let (Some(id), Some(obj)) = (args.get("id").cloned(), a.as_object_mut()) {
        obj.entry("artifact_id").or_insert(id);
    }
    a
}
```

Arms in `call`, and the two `action required` / `unknown action` messages gain the two names:

```rust
                "event_create" => super::event_create::call(ctx, flatten_event_args(&args)?).await,
                "event_list"   => super::timeline::call(ctx, id_as_artifact_id(&args)).await,
```

Replace the Task 3 `state_at` block with `let args = if action == "state_at" { id_as_artifact_id(&args) } else { args };`.

- [ ] **Step 5: Delete the tool and its registrations**

Delete `src/librarian/tools/artifact_event.rs`. In `mod.rs` remove `pub mod artifact_event;` and the `Arc::new(artifact_event::ArtifactEvent),` line of `all_tools()`. In `adapter.rs` replace the `"artifact_event"` arm by adding `"event_create"` to the `doc` write set:

```rust
            "doc" => matches!(
                action,
                Some("create" | "update" | "move" | "delete" | "link" | "graft" | "append_entry" | "update_entry" | "event_create")
            ),
```

(`graft`, `append_entry`, `update_entry` write today and were missing from the set — the `is_write` regression test in `server.rs` should assert them; add cases.) Remove `"artifact_event",` from `tests/librarian/companion_hint.rs`.

- [ ] **Step 6: Run green**

Run: `cargo test --lib librarian 2>&1 ; cargo test --test tool_reachability 2>&1`
Expected: PASS. `tool_reachability` passes because the `ArtifactEvent` type no longer exists — no `KNOWN_DELEGATION_ONLY` entry is added.

Mutation: in `flatten_event_args`, replace `flat.insert("artifact_id".into(), json!(id));` with nothing — Expected: `event_create_lifts_the_nested_event…` FAILS (event_create errors on missing `artifact_id`). Restore.

- [ ] **Step 7: Commit**

```bash
git add -A src tests/librarian/companion_hint.rs
git commit -m "feat(doc): fold artifact_event in as event_create/event_list with a nested event object"
```

---

### Task 5: Fold `artifact_augment` into `doc` as `augment`

**Files:**
- Modify: `src/librarian/tools/augment.rs:8`, `:258-537` (struct + `impl Tool` → `pub(crate) async fn call`), tests `:541-1603`
- Modify: `src/librarian/tools/artifact.rs` (schema `augment` object, `merge`, arm, helper, tests)
- Modify: `src/librarian/tools/create.rs:49-60` (`AugmentSpec`)
- Modify: `src/librarian/tools/mod.rs:400`, `src/librarian/adapter.rs`, `tests/librarian/companion_hint.rs:12`

**Interfaces:**
- Consumes: `augment::call(ctx, flat)` where `flat` deserialises to `augment::Args` (`id`, `merge`, `prompt`, `params`, `params_path`, `render_template`, `params_schema`, `append_mode`, `history_cap`, `entry_collection`).
- Produces: `doc(action="augment", id, merge?, augment={…})`.

- [ ] **Step 1: Failing tests**

In `artifact.rs` tests: `PROBE_ACTIONS` gains `"augment"`; `probe_required("augment")` inserts `id` and `"augment": json!({"prompt": "probe"})`. Add:

```rust
    #[tokio::test]
    async fn augment_round_trips_through_the_nested_object_and_merge_preserves_prompt() {
        let ctx = mk_ctx();
        let id = "bbbbbbbbbbbbbbbb";
        seed_row(&ctx, id);
        Artifact
            .call(&ctx, json!({"action": "augment", "id": id, "augment": {"prompt": "keep me"}}))
            .await
            .expect("attach");
        Artifact
            .call(&ctx, json!({"action": "augment", "id": id, "merge": true,
                               "augment": {"params": {"n": 1}}}))
            .await
            .expect("merge");
        let cat = ctx.catalog.lock();
        let aug = crate::librarian::catalog::augmentation::get(&cat, id).unwrap().unwrap();
        assert_eq!(aug.prompt, "keep me", "merge=true must not touch the prompt");
        let params: Value = serde_json::from_str(&aug.params).unwrap();
        assert_eq!(params["n"], 1);
    }
```

Move `augment_schema_does_not_restate_the_merge_rule_per_field` (`augment.rs:1280-1327`) into `artifact.rs` tests; its first two statements become:

```rust
        let schema = Artifact.input_schema();
        let props = schema["properties"]["augment"]["properties"].as_object().unwrap();
```

and where it counted statements on the top-level `merge` param, read `schema["properties"]["merge"]`.

- [ ] **Step 2: Run red** — `cargo test --lib librarian::tools::artifact 2>&1` → FAIL `unknown action 'augment'`.

- [ ] **Step 3: `augment.rs` — from tool to function**

Delete `pub struct ArtifactAugment;` (`:8`) and the `impl Tool for ArtifactAugment` header/`name`/`description`/`input_schema` (`:258-321`). Turn `call` into a free function, keeping its body:

```rust
/// The augmentation write. Reached through `doc(action="augment")`, whose dispatcher
/// flattens the `augment` object onto `id`/`merge` before calling here.
pub(crate) async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let mut a: Args = serde_json::from_value(args).map_err(|e| {
        crate::tools::RecoverableError::with_hint(
            format!("doc(action=\"augment\") requires 'id': {e}"),
            "e.g. doc(action=\"augment\", id=\"<16-hex>\", augment={prompt: \"…\"}). Pass merge=true to patch an existing augmentation — merge=false (the default) REPLACES all seven shape fields, silently resetting any you omit.",
        )
    })?;
    // (delete the `set_audit_verb("artifact_augment")` block — the doc dispatcher stamps `doc.augment`)
    … rest of the body unchanged …
}
```

In `augment.rs` tests: `ArtifactAugment\s*\.call\(&` → `call(&` (`sed -i -E 's/ArtifactAugment\s*\.call\(/call(/g' src/librarian/tools/augment.rs`); delete `dispatch_stamps_the_audit_verb` there (the doc dispatcher's own test covers the verb). Remove the `use crate::tools::Tool` import if now unused.

- [ ] **Step 4: `artifact.rs` schema and arm**

Replace the `augment` object and add `merge`:

```rust
                "augment": {
                    "type": "object",
                    "description": "create/augment: the augmentation shape — create attaches it atomically; augment attaches or (merge=true) patches it on an existing document. `prompt` is required except when merge=true. Unknown keys are REJECTED, not ignored.",
                    "additionalProperties": false,
                    "properties": {
                        "prompt": { "type": "string", "description": "Persistent instruction: what to maintain and how to format it." },
                        "params": { "type": "object", "description": "Data payload on the augmentation row. merge=false replaces it; merge=true RFC 7396 merge-patches it (arrays replaced whole)." },
                        "params_path": { "type": "string", "description": "Path to a JSON file holding params, read server-side. Mutually exclusive with params; for payloads too large to pass inline (≳9 KB). augment only — create refuses it." },
                        "render_template": { "type": "string", "description": "MiniJinja template projecting params into markdown for librarian(action=\"context\")." },
                        "params_schema": { "type": "object", "description": "JSON Schema validating params on every write." },
                        "entry_collection": { "type": "string", "description": "Name of the params array holding this tracker's entry rows (enables entry_filter, append_entry, update_entry)." },
                        "append_mode": { "type": "boolean", "description": "Refresh prepends a dated section instead of replacing the body." },
                        "history_cap": { "type": "integer", "minimum": 1, "description": "Max dated sections retained under append_mode." }
                    }
                },
                "merge": {
                    "type": "boolean",
                    "default": false,
                    "description": "augment: true patches only the fields you pass (params merge-patched, siblings overlaid, omitted fields preserved; prompt optional; requires an existing augmentation). false, the default, replaces all seven shape fields — an omitted field resets."
                },
```

`entry_collection`'s existing top-level description gains the `augment` note only if the label gate demands it (it does not: the top-level `entry_collection` still serves `append_entry/update_entry`). Add the arm and helper:

```rust
/// `augment` arrives as `{id, merge?, augment: {prompt, params, …}}`, reusing the object
/// `create` already accepts. `augment::Args` is flat (it predates the fold), so lift the
/// object and carry `id` and `merge` alongside.
fn flatten_augment_args(args: &Value) -> Result<Value> {
    let id = args["id"].as_str().ok_or_else(|| {
        RecoverableError::with_hint(
            "doc(action=\"augment\") requires 'id'",
            "e.g. doc(action=\"augment\", id=\"<16-hex>\", augment={prompt: \"…\"}) — merge=true to patch an existing augmentation",
        )
    })?;
    let mut flat = match args.get("augment") {
        Some(Value::Object(m)) => m.clone(),
        _ => {
            return Err(RecoverableError::with_hint(
                "doc(action=\"augment\") requires an `augment` object",
                "augment={prompt, params | params_path, render_template, params_schema, entry_collection, append_mode, history_cap}",
            )
            .into())
        }
    };
    flat.insert("id".into(), json!(id));
    if let Some(m) = args.get("merge") {
        flat.insert("merge".into(), m.clone());
    }
    Ok(Value::Object(flat))
}
```

```rust
                "augment" => super::augment::call(ctx, flatten_augment_args(&args)?).await,
```

`create.rs` `AugmentSpec` (`:49`): add `pub params_path: Option<String>,` (with `#[serde(default)]`), and where `create` consumes the spec, refuse it: `if spec.params_path.is_some() { return Err(RecoverableError::new("create does not read params_path — create the document, then doc(action=\"augment\", id=…, augment={params_path: …})").into()); }`.

- [ ] **Step 5: Registrations**

`mod.rs` `all_tools()`: remove `Arc::new(augment::ArtifactAugment),`. `adapter.rs`: delete the `"artifact_augment" => true,` arm; add `"augment"` to the `doc` write set. `companion_hint.rs`: remove `"artifact_augment",`.

- [ ] **Step 6: Run green**

Run: `cargo test --lib librarian 2>&1 ; cargo test --test tool_reachability 2>&1`
Expected: PASS, including the moved A-27 test.

Mutation: in `flatten_augment_args` drop the `merge` copy — Expected: the round-trip test FAILS (second call is treated as replace and refuses for missing prompt). Restore.

- [ ] **Step 7: Commit**

```bash
git add -A src tests/librarian/companion_hint.rs
git commit -m "feat(doc): fold artifact_augment in as augment, reusing create's nested augment object"
```

---

### Task 6: Fold `artifact_refresh` into `doc` as `gather` / `list_stale`; write the 17-action description

**Files:**
- Modify: `src/librarian/tools/artifact.rs` (schema, arms, description, tests)
- Modify: `src/librarian/tools/mod.rs:379`, `:401`, `src/librarian/adapter.rs`, `tests/librarian/companion_hint.rs:13`
- Delete: `src/librarian/tools/artifact_refresh.rs`

- [ ] **Step 1: Failing tests** — `PROBE_ACTIONS` gains `"gather", "list_stale"` (now `[&str; 17]`); `probe_required("gather")` inserts `id`; `list_stale` inserts nothing. Move `list_stale_action_routes_correctly` from `artifact_refresh.rs:144-153` into `artifact.rs` tests, calling `Artifact.call(&mk_ctx(), json!({"action": "list_stale"}))`.

- [ ] **Step 2: Run red** — FAIL `unknown action 'gather'`.

- [ ] **Step 3: Schema, arms, description**

Enum gains `"gather", "list_stale"`. Add `"threshold_hours": { "type": "integer", "default": 24, "description": "list_stale: hours since last refresh to count as stale (default 24)." }`. `scope`'s description: `find/list_stale: project (default), repo, umbrella, or all.` `limit`'s description gains ` list_stale: max documents (default 10, max 50).` `id`'s label gains `gather`. Arms:

```rust
                "gather"     => super::refresh::call(ctx, args).await,
                "list_stale" => super::refresh_stale::call(ctx, args).await,
```

Replace `description()` with (1,236 chars; cap 1,800):

```rust
    fn description(&self) -> &'static str {
        "Document catalog: find/get/create/update/move/delete markdown documents (specs, plans, ADRs, trackers, bug files) with YAML frontmatter, plus their events, augmentations and entries. Defaults: scope=project; archived/superseded hidden unless the filter constrains status; kind/status shortcuts AND with filter. Trackers are kind=tracker documents that may carry an augmentation (persistent prompt + params) — call librarian(tracker_design) before creating one. Entries: append_entry assigns the next PREFIX-N id atomically — with entry_collection it appends a params row; without it the ledger is prose and, given anchor_heading+title+body, the server writes the `## PREFIX-N — title` section itself. update_entry patches ONE row in place — use it rather than patch={params:…}, whose RFC 7396 array semantics replace the whole collection. Events: event_create appends an immutable record (kind inside the `event` object); event_list reads them newest-first. augment attaches or (merge=true) patches the augmentation; gather collects refresh context without writing (write back with update, commit_refresh=true); list_stale lists augmentations older than threshold_hours. graph walks links; link adds a manual rel; graft folds one row's history into another; state_at shows a document as of a commit or timestamp."
    }
```

- [ ] **Step 4: Delete and deregister** — delete `artifact_refresh.rs`; remove `pub mod artifact_refresh;` and `Arc::new(artifact_refresh::ArtifactRefreshTool),`; delete the `"artifact_refresh" => false,` adapter arm (gather/list_stale are reads and fall to the `doc` arm's `matches!` default of false); remove `"artifact_refresh",` from `companion_hint.rs`.

- [ ] **Step 5: Run green** — `cargo test --lib librarian 2>&1 ; cargo test --test tool_reachability 2>&1 ; cargo test --lib description 2>&1`. Expected: PASS; `every_tool_description_is_under_its_cap` passes with `doc` at ~1,236.

- [ ] **Step 6: Commit** — `git add -A src tests/librarian/companion_hint.rs && git commit -m "feat(doc): fold artifact_refresh in as gather/list_stale; describe all seventeen actions"`

---

### Task 7: Fold `read_markdown` into `read_file`

**Files:**
- Modify: `src/tools/markdown/read_markdown.rs:9`, `:478-668` (struct + impl → functions)
- Modify: `src/tools/markdown/mod.rs` (re-exports)
- Modify: `src/tools/read_file.rs:18-22` (description), `:28-44` (schema), `:74-113` (route, delete gate), `:161-167` (`format_compact`), `:573`, `:622` (hints)
- Modify: `src/server.rs:330` (registry), `src/tools/markdown/tests.rs`, `tests/e2e/edit_eval/cases.rs:184`
- Modify: `src/usage/db.rs:262` (comment only)

**Interfaces:**
- Produces: `crate::tools::markdown::read(input: Value, ctx: &ToolContext) -> Result<Value>` (results carry `"format": "markdown"`); `crate::tools::markdown::format_read(result: &Value) -> Option<String>`; `crate::tools::markdown::is_markdown_target(path: &str, ctx: &ToolContext) -> bool`.

- [ ] **Step 1: Failing route tests in `src/tools/read_file.rs` tests module** (`ctx_with_file(dir, name, body)` already exists there at `:2226` and activates the project at `dir`; add a test-local `const MD_FIXTURE: &str` holding `# Title`, `## A`, `## B` and twenty numbered body lines)

```rust
    #[tokio::test]
    async fn read_file_on_markdown_returns_the_heading_map_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_with_file(dir.path(), "notes.md", MD_FIXTURE).await;
        let out = ReadFile.call(json!({"path": "notes.md"}), &ctx).await.unwrap();
        assert_eq!(out["format"], "markdown");
        assert!(out["headings"].is_array(), "{out}");
    }

    #[tokio::test]
    async fn read_file_on_markdown_serves_heading_and_headings() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_with_file(dir.path(), "notes.md", MD_FIXTURE).await;
        let one = ReadFile.call(json!({"path": "notes.md", "heading": "## A"}), &ctx).await.unwrap();
        assert!(one["content"].as_str().unwrap().contains("## A"));
        let two = ReadFile.call(json!({"path": "notes.md", "headings": ["## A", "## B"]}), &ctx).await.unwrap();
        assert_eq!(two["sections"].as_array().map(|s| s.len()), Some(2));
    }

    #[tokio::test]
    async fn read_file_on_markdown_honours_offset_and_limit() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_with_file(dir.path(), "notes.md", MD_FIXTURE).await;
        let out = ReadFile.call(json!({"path": "notes.md", "offset": 5, "limit": 3}), &ctx).await.unwrap();
        let content = out["content"].as_str().unwrap();
        assert_eq!(content.lines().count(), 3, "{content}");
        assert!(out.get("headings").is_none(), "a line range must not return the heading map");
    }

    #[tokio::test]
    async fn read_file_force_on_markdown_is_a_raw_line_range() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_with_file(dir.path(), "notes.md", MD_FIXTURE).await;
        let out = ReadFile.call(json!({"path": "notes.md", "start_line": 1, "end_line": 2, "force": true}), &ctx).await.unwrap();
        assert!(out.get("format").is_none(), "force skips the markdown path: {out}");
    }

    #[tokio::test]
    async fn heading_on_a_non_markdown_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_with_file(dir.path(), "main.rs", "fn main() {}\n").await;
        let err = ReadFile.call(json!({"path": "main.rs", "heading": "## A"}), &ctx).await.unwrap_err();
        assert!(err.to_string().contains("markdown"), "{err}");
    }

    #[tokio::test]
    async fn read_file_refuses_a_managed_ledger_and_names_doc() {
        let text = "---\nid: '0123456789abcdef'\nentry_prefix: R\n---\n## R-1 — x\n";
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs/trackers")).unwrap();
        let ctx = ctx_with_file(dir.path(), "docs/trackers/x.md", text).await;
        let err = ReadFile.call(json!({"path": "docs/trackers/x.md"}), &ctx).await.unwrap_err();
        assert!(err.to_string().contains("doc(action=\"get\""), "{err}");
    }

    /// Without this guard the markdown route would swallow `json_path` silently — a new
    /// instance of the very class Task 7 closes for `offset`/`limit`.
    #[tokio::test]
    async fn json_path_on_markdown_is_refused_not_silently_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_with_file(dir.path(), "notes.md", MD_FIXTURE).await;
        let err = ReadFile.call(json!({"path": "notes.md", "json_path": "$.a"}), &ctx).await.unwrap_err();
        assert!(err.to_string().contains("only supported for JSON"), "{err}");
    }
```

- [ ] **Step 2: Run red** — `cargo test --lib read_file_on_markdown 2>&1` → FAIL `Use read_markdown for markdown files`.

- [ ] **Step 3: `read_markdown.rs` — from tool to functions**

Delete `pub struct ReadMarkdown;` (`:9`). Replace `impl Tool for ReadMarkdown { … }` (`:478-668`) with three functions whose bodies are the former methods:

```rust
/// Heading-addressed markdown read. Reached through `read_file`, which routes here for
/// `.md`/`.markdown` paths, `@file_` buffers that came from one, or any call carrying
/// `heading`/`headings`. Results carry `"format": "markdown"` so `ReadFile::format_compact`
/// can pick the markdown renderer.
pub(crate) async fn read(input: Value, ctx: &ToolContext) -> Result<Value> {
    let path = crate::tools::require_str_param_or_hint(
        &input,
        "path",
        crate::fs::PATH_PARAM_ALIASES,
        "read_file(path=\"docs/x.md\") — add heading=\"## Section\" to read one section.",
    )?;
    let (resolved, text) = resolve_markdown_source(path, ctx).await?;
    crate::util::librarian_guard::guard_not_librarian_managed(
        path, &text, Some(&resolved), crate::util::librarian_guard::Access::Read,
    )?;
    // … the param extraction and the three exclusivity checks exactly as in the old `call` …
    let res = if let Some(headings_arr) = headings_param {
        read_markdown_multi_heading(&text, &resolved, ctx, &headings_arr)
    } else if let Some(heading_query) = heading {
        read_markdown_single_heading(&text, &resolved, ctx, heading_query)
    } else if let (Some(start), Some(end)) = (start_line, end_line) {
        read_markdown_line_range(path, &text, &resolved, ctx, start, end)
    } else {
        read_markdown_default_tiers(&text, &resolved, ctx)
    };
    let mut res = res?;
    if let Some(obj) = res.as_object_mut() {
        obj.insert("format".into(), json!("markdown"));
    }
    Ok(res)
}

/// True when `read_file` should take the markdown path: a markdown extension, or an
/// `@file_` buffer whose source was one.
pub(crate) fn is_markdown_target(path: &str, ctx: &ToolContext) -> bool {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".md") || lower.ends_with(".markdown") {
        return true;
    }
    if path.starts_with("@file_") {
        return ctx
            .output_buffer
            .get(path)
            .and_then(|b| b.source_path.clone())
            .is_some_and(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("md")));
    }
    false
}

/// The former `ReadMarkdown::format_compact`, unchanged.
pub(crate) fn format_read(result: &Value) -> Option<String> { … }
```

In `resolve_markdown_source` (`:35-41`) reword the non-`.md` refusal: message `heading/headings address markdown sections, and '<path>' is not a markdown file`, hint `Drop heading/headings to read it as text, or pass a .md path.` Delete `relevant_guide_topic` and `output_form` (ReadFile already declares both).

`src/tools/markdown/mod.rs`: add `pub(crate) use read_markdown::{format_read, is_markdown_target, read};`.

- [ ] **Step 4: `read_file.rs` route, schema, description, formatter**

Delete the gate at `:106-113`. After `let path = strip_buffer_ref_quotes(raw_path);` (`:74`) insert:

```rust
        // Markdown: heading-addressed reads live in `markdown::read`, which `read_markdown`
        // used to wrap. Route there when the caller asked for headings, or the target is a
        // markdown file or a buffer that came from one — unless `force=true`, which keeps
        // its meaning of "raw line range, skip the smart path", or a format selector is
        // present, which falls through to the typed-format error below rather than being
        // silently ignored. `offset`/`limit` were already normalised above, so the markdown
        // path sees `start_line`/`end_line`.
        let wants_headings = input.get("heading").is_some() || input.get("headings").is_some();
        let wants_format = input.get("json_path").is_some() || input.get("toml_key").is_some();
        let force = input["force"].as_bool().unwrap_or(false);
        if wants_headings
            || (!force && !wants_format && crate::tools::markdown::is_markdown_target(path, ctx))
        {
            return crate::tools::markdown::read(input, ctx).await;
        }
```

Schema: add after `end_line`:

```rust
                "heading": { "type": "string", "description": "Markdown only: return one section by heading (e.g. \"## Auth\")." },
                "headings": { "type": "array", "items": { "type": "string" }, "description": "Markdown only: return several sections. Mutually exclusive with heading." },
```

Description:

```rust
        "Read a file. Large output → @file_* buffer. Markdown: heading map by default; heading=/headings= for sections; start_line/end_line or offset/limit for a slice; force=true for raw lines. Format-aware: json_path (JSON), toml_key (TOML/YAML). Source: a line range overlapping a symbol redirects to symbols(include_body=true); force=true bypasses."
```

`format_compact`:

```rust
    fn format_compact(&self, result: &Value) -> Option<String> {
        if result.get("format").and_then(|f| f.as_str()) == Some("markdown") {
            return crate::tools::markdown::format_read(result);
        }
        Some(format_read_file(result))
    }
```

Hints at `:573` and `:622`: replace `For Markdown files use read_markdown` with `For Markdown files pass heading= or headings=`.

- [ ] **Step 5: Registry, tests, fixtures**

`src/server.rs:330`: delete `Arc::new(ReadMarkdown),` and its import. `src/tools/markdown/tests.rs`: `sed -i -E 's/super::ReadMarkdown\s*\.call\(/super::read(/g; s/crate::tools::markdown::read_markdown::ReadMarkdown/crate::tools::markdown::read_markdown::read/g' src/tools/markdown/tests.rs`, then fix by hand any test that called `ReadMarkdown.input_schema()` or `.format_compact(` (→ `format_read(`). `tests/e2e/edit_eval/cases.rs:184`: `"symbol": "format_read"`. `src/usage/db.rs:262`: prepend to the comment block: `// read_markdown was folded into read_file on 2026-09-02; these branches classify HISTORICAL rows on backfill and are dead for new ones.`

- [ ] **Step 6: Run green**

Run: `cargo test --lib read_file 2>&1 ; cargo test --lib markdown 2>&1 ; cargo test --test tool_reachability 2>&1`
Expected: PASS.

Mutation: in the route, drop `|| (!force && …is_markdown_target…)` — Expected: `read_file_on_markdown_returns_the_heading_map_by_default` FAILS (raw text returned). Restore.

- [ ] **Step 7: Commit** — `git add -A src tests/e2e/edit_eval/cases.rs && git commit -m "feat(read_file): markdown reads by heading, folding read_markdown in; offset/limit now reach the markdown path"`

---

### Task 8: Fold `edit_markdown` into `edit_file`

**Files:**
- Modify: `src/tools/markdown/edit_markdown.rs:1209-1526` (struct + impl → `edit` fn + `LONG_DOCS`)
- Modify: `src/tools/markdown/mod.rs`
- Modify: `src/tools/edit_file/mod.rs:370-445` (description, schema, route), add `long_docs`
- Modify: `src/server.rs:329`, `src/tools/markdown/tests.rs`, `tests/bug_regression.rs:15,659,701`, `tests/edit_markdown_catalog_sync.rs:94,118`, `src/mcp_resources/tool_guide.rs:133-134`

**Interfaces:**
- Produces: `crate::tools::markdown::edit(input: Value, ctx: &ToolContext) -> Result<Value>` — caller has already run `guard_worktree_write` and `maybe_replay_ack`; `crate::tools::markdown::LONG_DOCS: &str`.

- [ ] **Step 1: Failing route tests in `src/tools/edit_file/tests.rs`** (`project_ctx()` at `:152` returns `(TempDir, ToolContext)` with the project activated at the temp dir; write each fixture into `dir.path()` before calling)

```rust
    #[tokio::test]
    async fn edit_file_on_markdown_replaces_a_section_by_heading() {
        let (dir, ctx) = project_ctx().await;
        std::fs::write(dir.path().join("n.md"), "# T\n\n## A\nold\n\n## B\nkeep\n").unwrap();
        EditFile.call(json!({"path": "n.md", "heading": "## A", "action": "replace", "content": "new\n"}), &ctx).await.unwrap();
        let text = std::fs::read_to_string(dir.path().join("n.md")).unwrap();
        assert!(text.contains("## A\nnew") && text.contains("## B\nkeep"), "{text}");
    }

    #[tokio::test]
    async fn edit_file_on_markdown_sets_frontmatter() {
        let (dir, ctx) = project_ctx().await;
        std::fs::write(dir.path().join("n.md"), "---\nstatus: open\n---\n# T\n").unwrap();
        EditFile.call(json!({"path": "n.md", "frontmatter": {"set": {"status": "fixed"}}}), &ctx).await.unwrap();
        assert!(std::fs::read_to_string(dir.path().join("n.md")).unwrap().contains("status: fixed"));
    }

    #[tokio::test]
    async fn edit_file_refuses_a_batch_that_mixes_the_two_grammars() {
        let (dir, ctx) = project_ctx().await;
        std::fs::write(dir.path().join("n.md"), "# T\n\n## A\nx\n").unwrap();
        let err = EditFile.call(json!({"path": "n.md", "edits": [
            {"heading": "## A", "action": "replace", "content": "y"},
            {"old_string": "x", "new_string": "z"}
        ]}), &ctx).await.unwrap_err();
        assert!(err.to_string().contains("mixes"), "{err}");
    }

    #[tokio::test]
    async fn heading_grammar_on_a_non_markdown_file_is_refused() {
        let (dir, ctx) = project_ctx().await;
        std::fs::write(dir.path().join("m.rs"), "fn a() {}\n").unwrap();
        let err = EditFile.call(json!({"path": "m.rs", "heading": "## A", "action": "remove"}), &ctx).await.unwrap_err();
        assert!(err.to_string().contains("markdown"), "{err}");
    }

    #[tokio::test]
    async fn plain_string_edits_on_markdown_still_work() {
        let (dir, ctx) = project_ctx().await;
        std::fs::write(dir.path().join("n.md"), "# T\nfoo foo\n").unwrap();
        EditFile.call(json!({"path": "n.md", "old_string": "foo", "new_string": "bar", "replace_all": true}), &ctx).await.unwrap();
        assert_eq!(std::fs::read_to_string(dir.path().join("n.md")).unwrap(), "# T\nbar bar\n");
        // and a single non-replace_all edit — which the old gate refused — now works too
        EditFile.call(json!({"path": "n.md", "old_string": "# T", "new_string": "# U"}), &ctx).await.unwrap();
    }
```

- [ ] **Step 2: Run red** — FAIL `Use edit_markdown for markdown files`.

- [ ] **Step 3: `edit_markdown.rs` — from tool to function**

Delete `pub struct EditMarkdown;` and the `impl Tool` header, `name`, `is_write`, `description`, `input_schema`. Move the `long_docs` string to `pub(crate) const LONG_DOCS: &str = "…";` with every `read_markdown(` → `read_file(` and `edit_markdown(` → `edit_file(` inside it. Turn `call` into:

```rust
/// Heading-addressed markdown edit. Reached through `edit_file`, which has already run
/// `guard_worktree_write` and `maybe_replay_ack` and decided the call is markdown grammar.
pub(crate) async fn edit(input: Value, ctx: &ToolContext) -> Result<Value> {
    let path = crate::tools::require_str_param_or_hint(
        &input, "path", crate::fs::PATH_PARAM_ALIASES,
        "edit_file(path=\"docs/x.md\", heading=\"## Section\", action=\"replace\", content=\"...\"). path is required on every call.",
    )?;
    // (the `.md`-only gate is deleted — the caller routed here because the path is markdown)
    let resolved = match crate::tools::resolve_write_or_capture(ctx, "edit_file", &input, path).await? { … };
    … rest of the body unchanged …
}
```

Search the body for remaining `"edit_markdown"` literals and replace with `"edit_file"`. `mod.rs`: `pub(crate) use edit_markdown::{edit, LONG_DOCS};`.

- [ ] **Step 4: `edit_file/mod.rs` route, schema, description, long_docs**

Replace the gate block (`:416-445`) with:

```rust
        // Markdown heading grammar lives in `markdown::edit`, which `edit_markdown` used to
        // wrap. Route there when the call carries any heading-grammar key. The two grammars
        // never mix in one batch: their atomicity and shrink-guard semantics differ.
        let is_md = path.ends_with(".md") || path.ends_with(".markdown");
        let edits_arr = input["edits"].as_array();
        let heading_items = edits_arr
            .map(|e| e.iter().filter(|x| x.get("heading").is_some()).count())
            .unwrap_or(0);
        let plain_items = edits_arr.map(|e| e.len()).unwrap_or(0) - heading_items;
        let heading_grammar = input["heading"].is_string()
            || input["action"].is_string()
            || input["frontmatter"].is_object()
            || heading_items > 0;
        if heading_grammar && !is_md {
            return Err(super::RecoverableError::with_hint(
                "heading, action, frontmatter and heading-addressed edits[] apply to markdown files only",
                "For non-markdown files use old_string/new_string, insert, or replace_all.",
            )
            .into());
        }
        if heading_items > 0 && plain_items > 0 {
            return Err(super::RecoverableError::with_hint(
                "edits[] mixes heading-addressed items with old_string/new_string items",
                "Send one grammar per call: every item with `heading`+`action`, or every item with `old_string`+`new_string`.",
            )
            .into());
        }
        if heading_grammar {
            return crate::tools::markdown::edit(input, ctx).await;
        }
```

Schema: add to `properties` the eight markdown params copied from `edit_markdown.rs:1254-1281` (`heading`, `occurrence`, `action`, `content`, `at`, `include_subsections`, `force`, `frontmatter`), each description prefixed `Markdown only: `. Replace `edits.items` with the union — drop its `required`, and give the item description: `One grammar per batch: text items {old_string, new_string, replace_all?} or, on markdown, heading items {heading, action, content?, at?, occurrence?, old_string?, new_string?, replace_all?, include_subsections?}.` Description:

```rust
        "Edit a file. Text: exact old_string/new_string (whitespace-sensitive; re-indent retry in brace languages), insert prepend/append, replace_all, or edits[] applied atomically. Markdown: heading+action (replace/insert_before/insert_after/remove/edit), edits[] of heading items, frontmatter {set, delete} — one atomic write."
```

Add `fn long_docs(&self) -> Option<&str> { Some(crate::tools::markdown::LONG_DOCS) }`.

- [ ] **Step 5: Registry and callers**

`src/server.rs:329`: delete `Arc::new(EditMarkdown),` and the import. `src/tools/markdown/tests.rs`: `sed -i -E 's/super::edit_markdown::EditMarkdown\s*\.call\(/super::edit_markdown::edit(/g; s/EditMarkdown\s*\.call\(/edit(/g' src/tools/markdown/tests.rs`; tests that used `EditMarkdown.input_schema()` read `crate::tools::edit_file::EditFile.input_schema()` instead. `tests/bug_regression.rs:15`: `use codescout::tools::edit_file::EditFile;` and `:659`, `:701` `EditMarkdown` → `EditFile` (inputs unchanged — they carry `heading`). `tests/edit_markdown_catalog_sync.rs:94,118`: same substitution. `src/mcp_resources/tool_guide.rs:133-134`: `use crate::tools::edit_file::EditFile; let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(EditFile)];`.

- [ ] **Step 6: Run green**

Run: `cargo test --lib edit_file 2>&1 ; cargo test --lib markdown 2>&1 ; cargo test --test bug_regression 2>&1 ; cargo test --test edit_markdown_catalog_sync 2>&1 ; cargo test --test tool_reachability 2>&1`
Expected: PASS.

Mutation: delete the `guard_not_librarian_managed` call inside `markdown::edit` — Expected: the moved catalog-sync test and `edit_markdown`'s managed-file tests FAIL. Restore.

- [ ] **Step 7: Commit** — `git add -A src tests && git commit -m "feat(edit_file): markdown edits by heading, folding edit_markdown in; one grammar per batch"`

---

### Task 9: Prompt surfaces — Iron Laws, guides, deprecated names, banner, onboarding version

**Files:**
- Modify: `src/prompts/source.md` (lines 5-27), `src/prompts/builders.rs:287-290`, `:314-317`, `:557-570`, `src/prompts/mod.rs:203`, `:1939-1947`
- Modify: `src/prompts/guides/librarian.md`, `tracker-conventions.md`, `librarian-runtime.md`, `iron-laws-detail.md`, `progressive-disclosure.md`, `project-activation-bootstrap.md`, `untrusted-content.md`
- Modify: `src/tools/onboarding.rs:32`

- [ ] **Step 1: Grow the deprecated list (the failing test)**

`src/prompts/mod.rs:1939` `DEPRECATED_TOOL_NAMES` gains, sorted into the list: `"artifact("`, `"artifact_augment"`, `"artifact_event"`, `"artifact_refresh"`, `"edit_markdown"`, `"read_markdown"`. (`"artifact("` with the paren, not the bare word: the gate is a substring check and the noun legitimately survives in prose and in Rust/SQL names.)

Run: `cargo test --lib prompt 2>&1`
Expected: `prompt_surfaces_reference_only_real_tools`, `claude_md_contains_no_deprecated_tool_names` and the rendered-instructions gate FAIL, listing the surfaces to fix.

- [ ] **Step 2: `source.md` Iron Laws and quickref**

Replace lines 4–5 of the law list with one line, keeping 1–3 and 6 numbered as they are:

```
4–5. Retired 2026-09-02: read_file/edit_file handle .md by heading —
   read_file(path) is the heading map, heading=/headings= a section;
   edit_file(path, heading, action). Managed trackers → doc(get/update).
```

Quickref: add `- Markdown section → read_file(path, heading=…) | doc(get, id, heading=…) for a tracker`. Run `cargo test --lib source_md_under_cap` — Expected PASS (the slice shrank).

- [ ] **Step 3: `builders.rs`** — at `:287-290` and `:314-317` replace `read_markdown("path")` with `read_file("path")` and `read_markdown("@file_ref", heading=…)` with `read_file("@file_ref", heading=…)`; at `:557-570` every `read_markdown(` → `read_file(`.

- [ ] **Step 4: Banner** — `src/prompts/mod.rs:203`: `"\nUse `project_id: \"<id>\"` in `semantic_search` / `memory`, or the `workspace` pin on any tool, to scope to a specific project.\n"`.

- [ ] **Step 5: Guides**

```bash
cd src/prompts/guides
sed -i -E 's/<!-- serves: artifact\./<!-- serves: doc./g; s/, artifact\./, doc./g' librarian.md
sed -i 's/<!-- serves: artifact_augment, artifact_refresh.gather, artifact_refresh.list_stale -->/<!-- serves: doc.augment, doc.gather, doc.list_stale -->/; s/<!-- serves: artifact_event.create, artifact_event.list -->/<!-- serves: doc.event_create, doc.event_list -->/' librarian.md
sed -i -E 's/\bartifact\(/doc(/g' *.md
grep -n 'artifact_event\|artifact_augment\|artifact_refresh\|read_markdown\|edit_markdown' *.md
```

Rewrite by hand every line the last grep prints: `artifact_event(action="create", artifact_id=…, kind=…, payload=…)` → `doc(action="event_create", id=…, event={kind: …, payload: …})`; `artifact_augment(id=…, merge=true, params=…)` → `doc(action="augment", id=…, merge=true, augment={params: …})`; `artifact_refresh(action="gather"|"list_stale")` → `doc(action="gather"|"list_stale")`; `read_markdown(` → `read_file(`; `edit_markdown(` → `edit_file(`. In `iron-laws-detail.md` replace the bodies of `## Iron Law 4: markdown reads → `read_markdown`` and `## Iron Law 5: markdown edits → `edit_markdown`` (keep the headings so citations of the law numbers still resolve) with:

```markdown
## Iron Law 4: markdown reads → `read_markdown`

**Retired 2026-09-02.** `read_markdown` was folded into `read_file`. The law existed to
keep line-range reads off heading-structured files; the tool now does that by default —
`read_file(path)` on a `.md` returns the heading map, `heading="## Section"` one section,
`headings=[…]` several, `start_line`/`end_line` (or native-Read `offset`/`limit`) a slice,
and `force=true` a raw line range. No gate fires; there is nothing left to route away from.

**Librarian-managed documents are still refused** — a tracker or bug file with a stamped
`id:` or a declared `entry_prefix` is read through `doc(action="get", id=…)`, and
`read_file` says so in its refusal.

## Iron Law 5: markdown edits → `edit_markdown`

**Retired 2026-09-02.** `edit_markdown` was folded into `edit_file`. On a `.md` path,
`heading` + `action` (`replace | insert_before | insert_after | remove | edit`), an
`edits[]` batch of heading items, or `frontmatter={set, delete}` select the heading
grammar; `old_string`/`new_string`, `insert`, and `replace_all` still work as text edits.
A batch may not mix the two grammars — the call is refused rather than half-applied,
because their atomicity and shrink-guard semantics differ.

**Librarian-managed documents are still refused on every branch** — use
`doc(action="update", id=…, patch={body_edits: […]})`, whose entries are the heading
grammar's batch shape. A direct write bypasses the catalog: no `field_patch` event, no
shrink guard, stale `updated_at`.
```

- [ ] **Step 6: `ONBOARDING_VERSION`** — `src/tools/onboarding.rs:32`: `30`.

- [ ] **Step 7: Run green** — `cargo test --lib prompt 2>&1 ; cargo test --lib guide 2>&1`. Expected: PASS including the `serves:` gate (the five new actions are in `doc`'s enum).

- [ ] **Step 8: Commit** — `git add -A src && git commit -m "docs(prompts): retire Iron Laws 4-5, point the guides at doc/read_file/edit_file, deprecate the six retired names"`

---

### Task 10: Docs sweep, surface budget ratchet, full gate

**Files:**
- Modify: `CLAUDE.md`, `docs/TAXONOMY.md`, `docs/PROGRESSIVE_DISCOVERABILITY.md`, `docs/PROBES.md`, `docs/RELEASE.md`, `README.md`, `CONTRIBUTING.md`, `CHANGELOG.md`, `docs/manual/src/**/*.md`, `docs/architecture/companion-plugin.md`, `src/prompts/README.md`
- Modify: `src/server.rs` (`TOOL_SURFACE_CHAR_BUDGET`)

- [ ] **Step 1: Run the doc gates red**

Run: `cargo test --test doc_tool_refs 2>&1 ; cargo test --lib claude_md 2>&1`
Expected: FAIL listing every `artifact(`, `read_markdown(`, `edit_markdown(`, `artifact_*` call in the manual, root docs and `CLAUDE.md`.

- [ ] **Step 2: Sweep**

```bash
files=$(grep -rlE '\bartifact\(|artifact_event|artifact_augment|artifact_refresh|read_markdown|edit_markdown' CLAUDE.md README.md CONTRIBUTING.md CHANGELOG.md docs/TAXONOMY.md docs/PROGRESSIVE_DISCOVERABILITY.md docs/PROBES.md docs/RELEASE.md docs/architecture/companion-plugin.md src/prompts/README.md docs/manual/src)
echo "$files"
sed -i -E 's/\bartifact\(/doc(/g; s/read_markdown\(/read_file(/g; s/edit_markdown\(/edit_file(/g' $files
grep -nE 'artifact_event|artifact_augment|artifact_refresh|read_markdown|edit_markdown|codescout artifact' $files
```

Rewrite each remaining line by hand with the `doc(action="event_create"…)`, `doc(action="augment"…)`, `doc(action="gather"|"list_stale")`, `codescout doc …` forms. `CHANGELOG.md`: leave released sections alone (the gate caps them); under the Unreleased heading add:

```markdown
### Changed — tool surface collapse (26 → 21 tools)
- `artifact` is now `doc`. `artifact_event`, `artifact_augment` and `artifact_refresh` are
  `doc` actions: `event_create` / `event_list` (the event fields under one `event` object),
  `augment` (reusing the `augment` object `create` already took, plus `merge`), `gather` /
  `list_stale`. `state_at` takes `id` like every other action.
- `read_markdown` and `edit_markdown` are gone: `read_file` returns the heading map for a
  `.md` and takes `heading`/`headings`; `edit_file` takes the heading grammar and
  `frontmatter` on a `.md`. Iron Laws 4 and 5 are retired. `offset`/`limit` now work on
  markdown reads.
- No aliases: the six old names return the MCP unknown-tool error. CLI: `codescout doc …`.
- `Tool::description_cap()` replaces the librarian-family name-prefix exemption.
``` `docs/PROBES.md`: in the `probe_tool_surface.py` row add *"usage.db `tool_name` cut over on the ship date: `artifact` → `doc`, `artifact_*` → `doc` actions, `read_markdown`/`edit_markdown` → `read_file`/`edit_file`; a 30-day join spanning that date must union both names."* `docs/architecture/companion-plugin.md`: remove `il4-deny-hook` from the hook inventory. `CLAUDE.md` § Companion Plugin: the sentence listing `read_markdown`/`edit_markdown` among preferred tools becomes `read_file`/`edit_file`.

- [ ] **Step 3: Ratchet the budget**

Run: `cargo test --lib tool_surface_report_lengths -- --nocapture 2>&1`
Read the `TOTAL` line. Set `TOOL_SURFACE_CHAR_BUDGET` (`src/server.rs`) to exactly that number and add above the constant: `/// Ratcheted 2026-09-02 from 56_519 to <N> by the tool-surface collapse (26 → 21 tools; spec docs/superpowers/specs/2026-09-02-tool-surface-collapse-design.md).`

- [ ] **Step 4: Full gate, in order**

```bash
cargo fmt ; cargo clippy --workspace --all-targets --features local-embed -- -D warnings ; cargo test --workspace --no-default-features ; cargo test --workspace
```

Expected: all four exit 0. Read each exit code; do not rely on the last.

- [ ] **Step 5: Commit** — `git add -A && git commit -m "docs: sweep the read surfaces to doc/read_file/edit_file; ratchet the tool-surface budget"`

---

### Task 11: CLI — `codescout doc …`

**Files:**
- Rename: `src/cli/artifact.rs` → `src/cli/doc.rs`; `artifact_event.rs` → `doc_event.rs`; `artifact_refresh.rs` → `doc_refresh.rs`; `artifact_augment.rs` → `doc_augment.rs` (`git mv`)
- Modify: `src/cli/mod.rs:1-16`, `src/main.rs:176-197`, `:435-450`
- Rename: `tests/cli_artifact.rs` → `tests/cli_doc.rs`

- [ ] **Step 1: Failing CLI test** — in the renamed `tests/cli_doc.rs`, change every `run_cmd(&["artifact", …])` to `run_cmd(&["doc", …])`, `["artifact-event", "list", …]` to `["doc", "event", "list", …]`, `["artifact-refresh", "list-stale"]` to `["doc", "refresh", "list-stale"]`, and add:

```rust
#[test]
fn the_old_artifact_subcommand_is_gone() {
    let out = run_cmd(&["artifact", "find"]);
    assert!(!out.status.success(), "codescout artifact must not exist after 2026-09-02");
}
```

Run: `cargo test --test cli_doc 2>&1` → FAIL (`doc` unrecognised).

- [ ] **Step 2: Rename modules and subcommands**

`src/cli/mod.rs`: `pub mod doc; pub mod doc_augment; pub mod doc_event; pub mod doc_refresh;` and the header comment `codescout doc …`. In `src/cli/doc.rs` `Verb` gains three variants:

```rust
    /// Event log: `doc event list|create`.
    Event { #[command(subcommand)] verb: super::doc_event::Verb },
    /// Attach or merge an augmentation.
    Augment(super::doc_augment::AugmentArgs),
    /// Refresh lifecycle: `doc refresh gather|list-stale`.
    Refresh { #[command(subcommand)] verb: super::doc_refresh::Verb },
```

and `dispatch` gains the three arms:

```rust
        Verb::Event { verb } => super::doc_event::dispatch(verb).await,
        Verb::Augment(args) => super::doc_augment::run(args).await,
        Verb::Refresh { verb } => super::doc_refresh::dispatch(verb).await,
``` Each of those builds the **new** tool input: `doc_event::run_create` → `{"action": "event_create", "id": artifact_id, "event": {kind, payload, author, …}}`; `run_list` → `{"action": "event_list", "id": …, kinds, since, until, limit}`; `doc_augment::run` → `{"action": "augment", "id", "merge", "augment": {prompt, params, params_schema, render_template, append_mode, history_cap}}`; `doc_refresh` → `{"action": "gather"|"list_stale", …}`; `doc::run_state_at` sends `id` (not `artifact_id`). All call `Artifact.call`. `src/main.rs`: `Commands::Artifact{verb}` → `Commands::Doc{verb}` dispatching `codescout::cli::doc::dispatch(verb)`; delete the `ArtifactEvent`, `ArtifactRefresh`, `ArtifactAugment` variants and their arms. Rename the `--artifact-id` flags in `doc_event.rs` to `--id`.

- [ ] **Step 3: Run green** — `cargo build ; cargo test --test cli_doc 2>&1`. Expected: PASS.

- [ ] **Step 4: Commit** — `git add -A && git commit -m "feat(cli): codescout doc replaces codescout artifact*; event/augment/refresh become doc subcommands"`

---

### Task 12: Companion plugin (paired change in `../claude-plugins`)

**Files (all under `/home/marius/work/claude/claude-plugins/codescout-companion/`):**
- Delete: `hooks/il4-deny-hook.mjs`, `hooks/il4-deny-hook.test.sh`
- Modify: `hooks/hooks.json:97-106` (the `mcp__.*__read_file` matcher block), `hooks/pre-tool-guard.mjs:246-255`, `hooks/session-start.mjs:401`, `hooks/subagent-guidance.mjs:80`, `hooks/session-start.test.sh`, `skills/tracker-hygiene/SKILL.md`, `skills/tracker-hygiene/references/tracker-hygiene-log-template.md`, `skills/reconnaissance/SKILL.md`, `skills/reconnaissance/references/reconnaissance-patterns-template.md`, `skills/explore-project/SKILL.md`, `hooks/worktree-write-guard.mjs`, `hooks/worktree-write-guard.test.sh`, `hooks/cs-activate-project.mjs`, `hooks/explore-inject.mjs`, `hooks/worktree-activate.mjs`, `README.md`

- [ ] **Step 1: Branch** — `cd /home/marius/work/claude/claude-plugins && git checkout -b tool-collapse`.

- [ ] **Step 2: Remove the IL-4 hook** — `git rm hooks/il4-deny-hook.mjs hooks/il4-deny-hook.test.sh`; in `hooks/hooks.json` delete the whole object whose `matcher` is `"mcp__.*__read_file"` (lines 97–106 today), keeping the JSON valid (`node -e 'JSON.parse(require("fs").readFileSync("hooks/hooks.json","utf8"))'`).

- [ ] **Step 3: Text surfaces**

`hooks/pre-tool-guard.mjs:248-255`: replace the four `read_markdown(…)` recipe lines with `read_file(path="${relPath}")` (heading map), `read_file(path="${relPath}", heading="## Section")`, `read_file(path="${relPath}", headings=["## A", "## B"])`, and the sentence `read_file works on absolute cross-repo paths too.` `hooks/session-start.mjs:401` and `hooks/subagent-guidance.mjs:80`: replace the bullet with `• Markdown: read_file returns the heading map (heading= for a section); edit_file edits by heading — managed trackers go through doc`. `hooks/session-start.test.sh`: update the assertion that quoted the old bullet. Skills and remaining hooks:

```bash
grep -rlE '\bartifact\(|artifact_event|artifact_augment|artifact_refresh|read_markdown|edit_markdown' hooks skills README.md | xargs sed -i -E 's/\bartifact\(/doc(/g; s/read_markdown\(/read_file(/g; s/edit_markdown\(/edit_file(/g'
grep -rnE 'artifact_event|artifact_augment|artifact_refresh|read_markdown|edit_markdown' hooks skills README.md
```

Rewrite by hand what the last grep prints, with the `doc(action="…")` forms from Task 9 Step 5.

- [ ] **Step 4: Run the plugin's tests** — `for t in hooks/*.test.sh; do bash "$t" || echo "FAILED $t"; done`. Expected: every script passes; none references the deleted hook.

- [ ] **Step 5: Commit** — `git add -A && git commit -m "feat: follow codescout's tool collapse — doc replaces artifact, il4 hook retired, read_file/edit_file handle markdown"`

---

### Task 13: Registry pin, reachability, final gate, PR

**Files:**
- Test: `src/server.rs` tests module

- [ ] **Step 1: Write the registry pin**

```rust
    /// The advertised surface after the 2026-09-02 collapse. A new tool is a deliberate
    /// addition to this list, not a side effect; a retired name reappearing is a regression.
    #[tokio::test]
    async fn the_registry_is_exactly_the_twenty_one_tools() {
        let (_dir, server) = make_server().await;
        let mut names: Vec<&str> = server.tools.iter().map(|t| t.name()).collect();
        names.sort();
        let mut expected = vec![
            "approve_write", "call_graph", "create_file", "edit_code", "edit_file", "get_guide",
            "grep", "index", "library", "memory", "onboarding", "read_file", "references",
            "run_command", "semantic_search", "symbol_at", "symbols", "tree", "workspace",
        ];
        #[cfg(feature = "librarian")]
        expected.extend(["doc", "librarian"]);
        expected.sort();
        assert_eq!(names, expected);
        for dead in ["artifact", "artifact_event", "artifact_augment", "artifact_refresh", "read_markdown", "edit_markdown"] {
            assert!(server.find_tool(dead).is_none(), "{dead} was retired on 2026-09-02");
        }
    }
```

(`peer` registers only when enabled and `__probe_description_cap__` only under `CODESCOUT_PROBE=1`; `make_server` sets neither. If it does in this tree, add the conditional the same way as `librarian`.)

- [ ] **Step 2: Run** — `cargo test --lib the_registry_is_exactly 2>&1 ; cargo test --test tool_reachability 2>&1`. Expected: PASS. `no_stale_tolerance_entries` in `tool_reachability` must also pass — nothing was added to `KNOWN_DELEGATION_ONLY`.

- [ ] **Step 3: Full gate in order** (as Task 10 Step 4). Expected: four zeros.

- [ ] **Step 4: Live check** — `cargo build --release` in the worktree is not the live MCP binary; instead run the probe against the worktree's debug build: `python3 scripts/probe_tool_surface.py --binary target/debug/codescout` — Expected: `tools 21`, `TOTAL` equal to `TOOL_SURFACE_CHAR_BUDGET`, no row named `artifact*`, `read_markdown` or `edit_markdown`.

- [ ] **Step 5: Commit and open the PR**

```bash
git add -A && git commit -m "test(server): pin the 21-tool registry after the collapse"
git push -u origin tool-collapse
gh pr create --base experiments --title "Tool surface collapse: doc replaces the artifact family; markdown folds into read_file/edit_file" --body-file docs/superpowers/specs/2026-09-02-tool-surface-collapse-design.md
```

---

## After merge — from the main checkout, in this order

1. `librarian(action="merge_worktree", root="/home/marius/work/claude/codescout/.worktrees/tool-collapse")` — before `git worktree remove .worktrees/tool-collapse`. Read the report; `doctor` must show no `pending_merge`.
2. `cargo rb` then `/mcp` so the live server is the new binary.
3. Ledger instruction text (worktree could not write these): `grep -rlE '\bartifact\(|read_markdown|edit_markdown' docs/trackers/*.md` → for each **active** tracker whose preamble or template tells an agent which call to make, `doc(action="update", id=…, patch={body_edits: [{heading: "…", action: "edit", old_string: "artifact(", new_string: "doc(", replace_all: true}]})`. Archived trackers and past entries are left as written. Record the file list in the compaction session log.
4. Archive the six 2026-09-02 bug files, each with the fix SHA and `git show <sha> | git patch-id --stable`: `read-markdown-silently-ignores-offset-and-limit` (Task 7), `activation-banner-names-a-project-param-symbols-does-not-have` (Task 9), `workspace-schema-requires-an-action-the-code-does-not` (Task 3), `artifact-patch-schema-describes-a-failure-that-no-longer-happens` (Task 3), `index-description-omits-the-verify-action` (Task 3), `artifact-action-labels-omit-delete-move-and-update-entry` (Task 3). Use `doc(action="move", …)`, then re-point citations per `get_guide("tracker-conventions")` § Bug files.
5. Hamsa row: `doc(action="append_entry", id="59ebeebb6ed05c89", id_prefix="A", …)` recording the Iron Law 4/5 removal as forced-by-removal with the two-week usage.db measurement pre-registered: share of `read_file` calls on `.md` carrying `heading`/`headings`, against `read_markdown`'s share in the 30 days before the ship.
6. Session-log entries in `docs/trackers/prompt-surface-compaction-session-log.md` (`03464a8808345846`): the re-measured surface total and the `n` of tools, and any friction met during execution.
7. Sibling repos: `grep -rlE '\bartifact\(|read_markdown|edit_markdown' ../prompt-engineering ../researcher --include='*.md' --include='*.mjs' --include='*.json' --include='*.py'` and fix each hit; their CLAUDE.md files call the tools by name.
8. Update `.codescout/usage.db` consumers: `scripts/probe_tool_surface.py` needs no change (it reads the live registry); `/analyze-usage` reports spanning the ship date must union old and new names — note this in the report the first time it is run.
