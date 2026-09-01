# Request-Aware Response Envelopes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop codescout's read surfaces from billing a caller the un-narrowed advisory payload when the caller narrowed the request.

**Architecture:** Three independent one-condition gates, each at an existing decision point, each reusing a discriminator the code already computes. No new types, no signature changes, no new tool parameters. Task 1 branches on `body_selected` (already in scope); Task 2 branches on `scoped_body_hint` (already written, shipped `bb4688fd`); Task 3 reads the `result` the method already receives and currently ignores.

**Tech Stack:** Rust, `serde_json::Value`, `tokio::test`, existing `mod tests` blocks in each touched file.

**Spec:** `docs/superpowers/specs/2026-09-01-request-aware-response-envelope-design.md`

## Global Constraints

- **Gate, in this exact order — the order is load-bearing** (CLAUDE.md § Development Commands). The lean lane runs THIRD and the default lane LAST, because the lean lane leaves a librarian-less binary in the shared `target/` and `tests/cli_artifact.rs` resolves it by path at run time:
  1. `cargo fmt`
  2. `cargo clippy --workspace --all-targets --features local-embed -- -D warnings`
  3. `cargo test --workspace --no-default-features`
  4. `cargo test --workspace`
- **No signature changes.** The spec's original widening of `relevant_guide_topic` was withdrawn: `src/tools/core/types.rs:1433-1438` documents that cloning `input` is "paid on 100% of tool calls to benefit the ~3% that inject a guide." Do not reintroduce it.
- **No new tool parameters.** A caller-declared verbosity flag was considered and rejected in the spec.
- **Every test must be shown to fail before it is made to pass.** Each task carries an explicit mutation step. A test never observed red is not evidence.
- **Commit by pathspec, and verify AFTER — never a bare `git commit`.** This checkout is shared with ~10 live agent sessions that stage concurrently. A bare `git commit` commits the **entire index**, so a peer's staged files land in your commit. Then run `git show --stat HEAD` and confirm the file list is exactly yours.

  **Argument order is load-bearing — `-m` BEFORE `--`:**

  ```bash
  git commit -m "msg" -- <your paths>      # correct
  git commit -- <your paths> -m "msg"      # INVALID — exits 1
  ```

  After `--`, git treats everything as a pathspec, so the second form fails with
  `error: pathspec '-m' did not match any file(s) known to git`. Verified in a throwaway
  repo 2026-09-01: form 1 exit 0, form 2 exit 1. An earlier revision of this plan carried
  the invalid form.

  Checking `git status` *before* committing does not close the peer-capture gap — the gap
  is between the check and the commit. Measured 2026-09-01 by a peer session: staging 2
  paths, then listing seconds later, showed **6** staged files; 4 were another session's.

  If a commit does capture someone else's file: **stop, do not `reset` or `amend`** — that
  can destroy their work, and an amend rewrites a commit a peer may already have built on.
  Report it.
- **Review scope is YOUR commits, not a recorded BASE.** On this checkout a BASE recorded
  before dispatch over-collects everything peers landed meanwhile — measured on Task 1:
  14 commits in the recorded range, **1** of them ours. Identify your own with the
  `Session-Id:` trailer (`git log --grep='Session-Id: <id>'`), and scope the review
  package to those.
- **Bug file to update on completion:** `docs/issues/archive/2026-09-01-a-scoped-read-is-billed-the-full-heading-map.md` — record fix SHA **and** `git show <sha> | git patch-id --stable`, label the branch `experiments`.

---

## File Structure

| File | Responsibility | Task |
|---|---|---|
| `src/librarian/tools/get.rs` | Modify `call()` at the preview assignment; add private `stub_preview` helper + 2 tests | 1 |
| `src/librarian/adapter.rs` | Modify `librarian_compact_summary` section-list branch; add 2 tests | 2 |
| `src/tools/markdown/read_markdown.rs` | Modify `relevant_guide_topic`; add 2 tests | 3 |

Three files, three independent gates. No shared new code — deliberately, so each task can be reviewed and reverted alone, and so the three mutation runs the spec requires are genuinely independent.

---

### Task 1: Stub the preview on a body-selected `artifact(get)`

**Files:**
- Modify: `src/librarian/tools/get.rs` — the `out["preview"] = if stub_this_preview { … }` assignment (in `get::call`)
- Modify: `src/librarian/tools/get.rs` — add `stub_preview` beside `apply_soft_cap`
- Test: `src/librarian/tools/get.rs` `mod tests` (existing)
- Test: `src/librarian/adapter.rs` `mod tests` — two folded tests from dropped Task 2 (Step 1b). **Test code only; do not change adapter.rs logic.**

**Interfaces:**
- Consumes: `body_selected: bool`, already computed near the top of `get::call` from `a.full || a.heading || a.headings(non-empty) || a.start_line || a.end_line`. **Do not recompute or hoist it — it is already in scope at the preview line.**
- Produces: `fn stub_preview(full: &Value) -> Value` (private to this module; no other task consumes it).

**Design note the implementer must not "fix":** `entry_filter` is deliberately absent from `body_selected` and must stay absent here. It filters params rows rather than selecting body, so a caller using it has *not* named a section and still benefits from the heading map.

- [ ] **Step 1: Write the two failing tests**

Add to `mod tests` in `src/librarian/tools/get.rs`. They are a **pair**: the first is monotone under removal (a dead preview builder satisfies it), so it is not evidence without the second.

```rust
    /// A scoped read already named what it wanted. Shipping the whole heading map with it
    /// answers a question the caller did not ask — measured 2026-09-01 at ~2,611 bytes of
    /// preview against a 3,210-byte requested section.
    /// docs/issues/archive/2026-09-01-a-scoped-read-is-billed-the-full-heading-map.md
    #[tokio::test]
    async fn preview_is_stubbed_when_a_body_selector_is_present() {
        let cat = Catalog::open_in_memory().unwrap();
        let mut row = mk_row("a");
        row.kind = "doc".into();
        artifact::upsert(&cat, &row).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        std::fs::write(
            dir.path().join("a.md"),
            "# T\n\n## One\n\nalpha\n\n## Two\n\nbravo\n\n## Three\n\ncharlie\n",
        )
        .unwrap();

        let v = call(&ctx, json!({"id": "a", "heading": "## Two"})).await.unwrap();

        assert_eq!(
            v["preview"]["headings"].as_str(),
            Some(HEADINGS_OMITTED_NOTE),
            "a body-selected read must not ship the heading array"
        );
        // Magnitude is RETAINED, not just absence — reporting only absence would be the
        // IC-21 shape (an instrument omitting the dimension that grows).
        assert_eq!(v["preview"]["total_headings"], 4);
        assert!(
            v["body"].as_str().unwrap().contains("bravo"),
            "the requested section must still be returned"
        );
    }

    /// The positive twin. Without this, a mutation that deletes the preview builder
    /// entirely passes the test above — absence assertions are monotone under removal.
    #[tokio::test]
    async fn preview_headings_are_still_shipped_when_no_body_selector() {
        let cat = Catalog::open_in_memory().unwrap();
        let mut row = mk_row("a");
        row.kind = "doc".into();
        artifact::upsert(&cat, &row).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        std::fs::write(
            dir.path().join("a.md"),
            "# T\n\n## One\n\nalpha\n\n## Two\n\nbravo\n\n## Three\n\ncharlie\n",
        )
        .unwrap();

        let v = call(&ctx, json!({"id": "a"})).await.unwrap();

        let headings = v["preview"]["headings"]
            .as_array()
            .expect("an unscoped read must keep the heading map");
        assert_eq!(headings.len(), 4);
    }
```

- [ ] **Step 1b: Add the folded subsumption test (from dropped Task 2)**

Add to `mod tests` in `src/librarian/adapter.rs`. This is the one survivor of Task 2, which
was dropped by controller Ruling 2 because Task 1 subsumes it. It asserts the *consequence*
rather than re-implementing the gate, and it is what catches a regression if Task 1's stub
ever stops replacing the `headings` key.

```rust
    /// `section_headings_summary` requires `preview.headings` to be an ARRAY. Once a
    /// body-selected read stubs that key to a string, the summary can no longer lead with
    /// the heading map — so the fix in `get.rs` delivers this for free and no second gate
    /// is needed here.
    /// Controller Ruling 2,
    /// docs/issues/archive/2026-09-01-a-scoped-read-is-billed-the-full-heading-map.md
    #[test]
    fn a_body_selected_read_summary_cannot_lead_with_the_heading_map() {
        // Shaped as a POST-Task-1 scoped result: `headings` is the stub string.
        let result = json!({
            "body": "## Index\n\nthe section the caller asked for",
            "body_meta": { "heading": "## Index", "line_count": 2, "bytes": 40 },
            "preview": {
                "shape": "default",
                "headings": "omitted (body selector present) — call artifact(get, id=…) with no body selector for the map",
                "total_headings": 12
            }
        });

        let summary = librarian_compact_summary("artifact", &result).expect("a summary");

        assert!(
            !summary.contains("sections:"),
            "a body-selected read must not lead with the heading map, got: {summary}"
        );
    }

    /// The positive twin: an UNSCOPED result still carries an array, and the summary must
    /// still lead with the map. Without this, deleting `section_headings_summary`'s call
    /// site passes the test above while destroying the map for everyone.
    #[test]
    fn an_unscoped_read_summary_still_leads_with_the_heading_map() {
        let result = json!({
            "body": "# T\n\n## Alpha\n\n## Index\n",
            "preview": { "headings": [
                { "level": 2, "text": "Alpha", "line": 3 },
                { "level": 2, "text": "Index", "line": 5 }
            ]}
        });

        let summary = librarian_compact_summary("artifact", &result).expect("a summary");

        assert!(
            summary.contains("sections:"),
            "an unscoped read must keep the heading map, got: {summary}"
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib librarian::tools::get::tests::preview_ -- --nocapture`

Expected: `preview_is_stubbed_when_a_body_selector_is_present` FAILS to compile with `cannot find value HEADINGS_OMITTED_NOTE in this scope`. `preview_headings_are_still_shipped_when_no_body_selector` PASSES (it describes current behaviour).

- [ ] **Step 3: Add the constant and helper**

Insert after `apply_soft_cap` (ends `src/librarian/tools/get.rs:85`):

```rust
/// What a stubbed preview puts where the heading array was.
const HEADINGS_OMITTED_NOTE: &str =
    "omitted (body selector present) — call artifact(get, id=…) with no body selector for the map";

/// Strip the heavy fields from a preview when the caller already named what they wanted.
///
/// Key-driven rather than shape-driven, so a preview shape this function does not know
/// about (`plan`, `spec`, `memory`) keeps its current behaviour rather than silently
/// losing a field: the worst case is no improvement, never a regression.
///
/// `line_count`, `total_headings` and `headings_truncated` are RETAINED deliberately —
/// they report the magnitude withheld, so a caller who needs the map learns it exists and
/// how big it is. Reporting only its absence would be the `IC-21` shape.
/// docs/issues/archive/2026-09-01-a-scoped-read-is-billed-the-full-heading-map.md
fn stub_preview(full: &Value) -> Value {
    let Some(obj) = full.as_object() else {
        return full.clone();
    };
    let mut stub = serde_json::Map::with_capacity(obj.len());
    for (k, v) in obj {
        match k.as_str() {
            "headings" => {
                stub.insert(k.clone(), json!(HEADINGS_OMITTED_NOTE));
            }
            "summary" | "last_heading" => {}
            _ => {
                stub.insert(k.clone(), v.clone());
            }
        }
    }
    Value::Object(stub)
}
```

- [ ] **Step 4: Branch the preview assignment**

Replace the preview assignment in `get::call` (currently one line):

```rust
        out["preview"] = crate::librarian::preview::extract(&row.kind, &row, body, ctx);
```

with:

```rust
        let preview = crate::librarian::preview::extract(&row.kind, &row, body, ctx);
        // `body_selected` is already computed earlier in `get::call` and in scope — do not recompute.
        out["preview"] = if body_selected {
            stub_preview(&preview)
        } else {
            preview
        };
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --lib librarian::tools::get::tests::preview_ -- --nocapture`
Expected: PASS, 3 tests (the two new ones plus the pre-existing `preview_present_by_default`).

- [ ] **Step 6: Mutate this site and confirm the pair kills it**

Temporarily change Step 4's branch to `out["preview"] = preview;` (pre-change behaviour).

Run: `cargo test --lib librarian::tools::get::tests::preview_`
Expected: `preview_is_stubbed_when_a_body_selector_is_present` FAILS; the twin PASSES.

Then temporarily change it to `out["preview"] = stub_preview(&preview);` (unconditional).
Expected: `preview_headings_are_still_shipped_when_no_body_selector` FAILS; the first PASSES.

**Both mutations must produce exactly one failure each.** If either kills both or neither, the pair is not discriminating — stop and fix the tests before restoring. Restore the Step 4 code.

- [ ] **Step 7: Run the full gate**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features
cargo test --workspace
```

- [ ] **Step 8: Commit**

```bash
# Pathspec form — commits ONLY these paths, ignoring whatever else peers have staged.
# `-m` MUST come BEFORE `--`: after `--` everything is read as a pathspec.
git commit -m "fix(librarian): a body-selected artifact(get) no longer ships the whole heading map

A scoped read already named what it wanted; the preview answered a question the
caller did not ask. Measured 2026-09-01: ~2,611 bytes of preview against a
3,210-byte requested section. total_headings is retained so the magnitude
withheld stays visible.

docs/issues/archive/2026-09-01-a-scoped-read-is-billed-the-full-heading-map.md" \
  -- src/librarian/tools/get.rs src/librarian/adapter.rs

# Verify AFTER, not only before — the index can change between check and commit.
git show --stat HEAD
```

The `git show --stat HEAD` output must list exactly the files you edited. Paste it into
your report as commit evidence. If it names anything else, stop and report — do not
`reset` or `amend` on a shared checkout.

---

### Task 2: The compact summary leads with the requested section

> **DROPPED by controller Ruling 2 at preflight. Do not implement. Number retained so
> Task 3 / Task 4 references stay stable.**

**Why it was dropped — verified, not assumed.** `section_headings_summary`
(`src/librarian/adapter.rs`) opens with:

```rust
let headings = result.get("preview")?.get("headings")?.as_array()?;
```

It requires an **array**. Task 1 replaces that key with a *string* whenever
`body_selected` is true, so the function already returns `None` on exactly the reads this
task was written to gate. And Task 2's trigger implies Task 1's: `body_meta` is emitted
only for a `heading`, `headings`, or line-slice read, and each of those sets
`body_selected` (computed once, near the top of `get::call`). There is no input on which Task 2
would fire and Task 1 had not already suppressed the map.

Implementing it anyway would add an unreachable branch guarded by a test whose fixture —
`body_meta` alongside an *array* of headings — describes a state that cannot occur once
Task 1 lands. That is the `declared-not-wired` (IC-3) and `assertion-that-cannot-fail`
(IC-16) shape at once, and this repo has laws against both.

**What survived:** two tests, relocated into **Task 1, Step 1b**. They assert the
*consequence* (a body-selected read's summary carries no `sections:` line; an unscoped
one still does) rather than re-implementing the gate, so they catch a regression if Task
1's stub ever stops replacing the `headings` key.

**If a reviewer disagrees:** the claim to attack is "no path emits `body_meta` without
setting `body_selected`." Find one and Task 2 comes back.
---

### Task 3: A guide about handling overflow ships only on a call that overflowed

> **DROPPED by controller Ruling 5 at pre-dispatch reconnaissance. Do not implement.**
> Recorded as `response-envelope-session-log:F-1`. Number retained so Task 4's references
> stay stable.

**The premise was false, and the prescribed fix would have caused a silent regression.**
Two findings from scouting `call_content` before dispatch, either one fatal:

**1 — The overflow gate already exists, centrally.** `src/tools/core/types.rs:1271-1279`
special-cases this exact topic:

```rust
let should = match topic {
    "progressive-disclosure" => {
        exceeds_inline_limit(&json)
            || val.as_object().and_then(|o| o.get("output_id"))
                  .and_then(|v| v.as_str()).is_some()
    }
    _ => true,
};
```

`candidates` is seeded from `relevant_guide_topic`'s return (`:1255`, called at `:1195`),
so `read_markdown`'s unconditional `Some(...)` is already filtered downstream. The guide
does **not** ship on a non-overflowing read. There was nothing to fix.

**2 — The proposed gate would have been permanently false.** `let mut val =
self.call(input, ctx).await?` at `:851`; `relevant_guide_topic(&val)` at `:1195`;
`"output_id": ref_id` first inserted at `:1385`, inside the **buffered** branch. Only
tools that pre-buffer themselves (`run_command`) carry it in the raw result.
`read_markdown` does not — so `result.get("output_id").is_some()` evaluates `None` on
every call, including overflowing ones, permanently disabling the guide for that tool.

**Why the tests in this task would not have caught it.** Both assert
`relevant_guide_topic` in isolation against a hand-built fixture that supplies `output_id`
directly, so neither exercises the `call_content` path where the key does not exist. Green
suite, clean review, silent regression. **The paired-twin discipline does not help here** —
both twins inherit the same false fixture premise. That is worth carrying forward: pairing
guards against a mutation in the code, not against a fixture unreachable from production.

**If someone later wants** `read_markdown`'s method to stop over-claiming, that is a
clarity refactor with no behavioural change, and it must be written against
`src/tools/core/types.rs:1271` — not against `output_id`.
---

### Task 4: Close out the bug file

**Files:**
- Modify: `docs/issues/archive/2026-09-01-a-scoped-read-is-billed-the-full-heading-map.md`

- [ ] **Step 1: Capture both fix identifiers**

```bash
git log -3
git show <task-1-sha> | git patch-id --stable
```

The SHA is positional and dies when `experiments` is rebased; the patch-id is a content hash of the diff and survives rebase and cherry-pick. Record **both**. Never record an empty patch-id.

- [ ] **Step 2: Update `## Fix` and `## Tests added` through the catalog**

The file is a catalogued bug, so write through the librarian, not a raw frontmatter edit — a direct edit does not reach the catalog (BL-48).

```
artifact(action="update", id="70bb5f85a1ba6a22",
         patch={status: "fixed", body_edits: [...]})
```

`## Tests added` must name all ten tests with `path:line`, and state that each was mutation-verified with which mutation killed which — the standard the sibling hint fix set.

- [ ] **Step 3: Archive only after the gate is green on `experiments`**

```
artifact(action="move", id="70bb5f85a1ba6a22",
         new_rel_path="docs/issues/archive/2026-09-01-a-scoped-read-is-billed-the-full-heading-map.md")
```

The move **mints a new id** and returns `previous_id` + `id_changed: true`. Re-point every citation of the old path *and* the old id in the same commit — the spec and `docs/trackers/issue-clusters.md` § IC-22 both cite this file.

- [ ] **Step 4: Re-run the cluster gate**

Run: `cargo test --test issue_clusters`
Expected: 16 passed. `every_index_count_matches_the_corpus` reads `git ls-files`, so the archive move must be staged for the count to stay at 2.

---

## Self-Review

**Spec coverage.** Change 1 → Task 1. Change 1b → Task 2. Change 2 → closed by probe, no task, correctly absent. Change 3 → Task 3, in its withdrawn-signature form. The spec's "compile-time guard making the selector a required parameter" is **deliberately dropped**: it presupposed the signature change that was withdrawn, and with three independent gates and no shared envelope builder there is no single constructor to make it a parameter of. Recorded here rather than silently omitted — if a fourth surface appears, revisit.

**Placeholder scan.** No TBD/TODO. Every code step carries the actual code. Every test step carries the actual test and the exact expected failure text.

**Type consistency.** `stub_preview(&Value) -> Value` (Task 1) is private and consumed only by Task 1. `scoped_body_hint(&Value) -> Option<String>` (Task 2) is pre-existing at `src/librarian/adapter.rs:209` and is read-only here. `relevant_guide_topic(&self, result: &Value) -> Option<&str>` (Task 3) keeps its arity — only the binding changes from `_result` to `result`. `HEADINGS_OMITTED_NOTE` is defined in Task 1 Step 3 and used in Task 1 Step 1's test, both in `src/librarian/tools/get.rs`.

**Ordering.** The three tasks are independent and may be done in any order or in parallel. Task 4 depends on Task 1 only.
