# Ledger-vs-Tracker Naturality Eval — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build and run a prompt-tdd A/B eval that measures whether an agent behaves more correctly when librarian augmented-artifacts are called "ledger" vs "tracker", gating the full rename in `docs/superpowers/specs/2026-07-10-tracker-to-ledger-rename-design.md`.

**Architecture:** Two frozen codescout binaries — the current `tracker`-vocabulary release and a minimal `ledger`-aliased build — are each driven by an identical pair of behavioral scenarios (append-shape, snapshot-shape) via the prompt-tdd `anthropic-mcp` registry, mirroring the existing `scenarios/librarian-guide/` arm pattern. The append-vs-overwrite action difference is the discriminator; `--ablate` is the negative control.

**Tech Stack:** Rust (codescout, `cargo rb`), prompt-tdd (Python harness in the `prompt-engineering` sibling repo), headless `claude -p` via the `anthropic-mcp` registry, `mode: trace` assertions.

## Global Constraints

- The ledger-aliased code lives on a dedicated branch `ledger-eval` off `experiments`; it is **never merged** by this plan. If the gate passes it becomes the seed of Phase 1; if not, it is abandoned. `master` is untouched.
- The ledger alias is **dual-read** (`kind == "tracker" || kind == "ledger"`) — it must not break any existing tracker behavior. `cargo test` + `cargo clippy -- -D warnings` stay green on the branch.
- Both eval binaries are frozen snapshots under this session's scratchpad: `EVAL_BINS=/tmp/claude-1000/-home-marius-work-claude-codescout/f62d42b2-f5da-4aea-be7c-21c309cabde8/scratchpad/eval-bins`. `cargo rb` overwrites `target/release/codescout` and the `~/.cargo/bin/codescout` symlink, so **snapshot the tracker binary before building the ledger binary.**
- Arm YAML uses **absolute paths** for both `mcp_command` and every prompt `path` (matches the existing arm convention; removes prompt-tdd cwd ambiguity).
- Scenario files must be named exactly `scenario.yaml` or prompt-tdd silently skips them.
- Eval generation runs on `model: sonnet` via the plugin-free subscription profile (`~/.prompt-tdd/profiles/plugin-free`), never the paid API. `runs: 10` per scenario per arm.
- Codescout repo edits use codescout MCP tools (`edit_code`/`edit_file`/`edit_markdown`), never native Edit/Write/Bash (companion hard-deny).

---

### Task 1: Freeze the tracker-arm baseline (binary + guide)

**Files:**
- Create: `$EVAL_BINS/codescout-tracker` (binary snapshot)
- Create: `$EVAL_BINS/guides/tracker/librarian.md` (guide snapshot)
- Read: `src/prompts/guides/librarian.md`

**Interfaces:**
- Produces: `$EVAL_BINS/codescout-tracker` — the current-HEAD release binary, tracker vocabulary; consumed by `arm-tracker.yaml` in Tasks 4–5.
- Produces: `$EVAL_BINS/guides/tracker/librarian.md` — verbatim copy of the shipped librarian guide; delivered as the tracker arm's system prompt.

- [ ] **Step 1: Confirm the working tree is on `experiments` and clean of the alias work**

Run: `git branch --show-current && git status --porcelain`
Expected: `experiments`; the only untracked/modified entries are the pre-existing ones from before this work stream (the new spec + plan under `docs/superpowers/`). No `ledger-eval` edits yet.

- [ ] **Step 2: Build the current release binary**

Run: `cargo rb`
Expected: `Finished release ... target/release/codescout`. Compiles clean.

- [ ] **Step 3: Snapshot the tracker binary and guide**

```bash
mkdir -p "$EVAL_BINS/guides/tracker"
cp target/release/codescout "$EVAL_BINS/codescout-tracker"
cp src/prompts/guides/librarian.md "$EVAL_BINS/guides/tracker/librarian.md"
```

- [ ] **Step 4: Verify the snapshot runs and reports tracker vocabulary**

Run: `"$EVAL_BINS/codescout-tracker" --version && grep -c -i tracker "$EVAL_BINS/guides/tracker/librarian.md"`
Expected: a version string; a nonzero count (the guide still says "tracker").

- [ ] **Step 5: Commit the plan bookkeeping (no code yet)**

```bash
git add docs/superpowers/plans/2026-07-10-ledger-vs-tracker-eval.md
git commit -m "docs(plans): ledger-vs-tracker eval implementation plan"
```

---

### Task 2: Ledger alias — `ledger_design` action + dual-read `kind`

**Files:**
- Modify: `src/librarian/tools/librarian.rs` (dispatch arm ~103, enum ~52, two error strings ~100/~113, add test in `mod tests`)
- Modify: `src/librarian/tools/create.rs:180-186` (dual-read `kind` gate + hint vocabulary)

**Interfaces:**
- Consumes: nothing (first branch task).
- Produces: `librarian(action="ledger_design")` routes to `tracker_design::call` and returns `{"archetypes": [...]}`; `artifact(create, kind="ledger", augment=None)` returns a `ledger_hint`. Relied on by the ledger guide in Task 3 and the fixtures in Tasks 4–5.

- [ ] **Step 1: Create the branch**

Run: `git checkout -b ledger-eval`
Expected: `Switched to a new branch 'ledger-eval'`.

- [ ] **Step 2: Write the failing routing test**

Add to the `mod tests` block in `src/librarian/tools/librarian.rs` (mirrors `tracker_design_routes_correctly` at ~139), via `edit_code(action="insert")` after `tracker_design_routes_correctly`:

```rust
    #[tokio::test]
    async fn ledger_design_aliases_tracker_design() {
        let v = Librarian
            .call(&mk_ctx(), serde_json::json!({"action": "ledger_design"}))
            .await
            .unwrap();
        assert!(v["archetypes"].is_array());
    }
```

- [ ] **Step 3: Run it to verify it fails**

Run: `cargo test -p codescout ledger_design_aliases_tracker_design 2>&1`
Expected: FAIL — the call returns a `RecoverableError` ("unknown action 'ledger_design'"), so `.unwrap()` panics.

- [ ] **Step 4: Add the dispatch arm and enum entry**

In `src/librarian/tools/librarian.rs`, edit the `match action` block to add the alias arm immediately after the `tracker_design` arm:

```rust
            "tracker_design"     => super::tracker_design::call(ctx, args).await,
            "ledger_design"      => super::tracker_design::call(ctx, args).await,
```

Add `"ledger_design"` to the `input_schema` action enum (line ~52) and append it to both error-message action lists (lines ~100 and ~113) so the surfaced vocabulary is consistent:

```rust
                "action required — one of: context, reindex, tracker_design, ledger_design, workspace_state_at, audit_doc_refs, legibility_scan, link_scan, doctor",
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p codescout ledger_design_aliases_tracker_design 2>&1`
Expected: PASS.

- [ ] **Step 6: Dual-read the `kind` gate in create.rs**

In `src/librarian/tools/create.rs`, replace the tracker-hint block at 180-186:

```rust
        if (a.kind == "tracker" || a.kind == "ledger") && a.augment.is_none() {
            result["ledger_hint"] = json!(
                "Ledger created without augmentation. \
                 Call librarian(ledger_design) to pick an archetype \
                 and attach a refresh prompt via artifact_augment."
            );
        }
```

- [ ] **Step 7: Verify the suite + lint stay green**

Run: `cargo test 2>&1 | tail -5 ; cargo clippy -- -D warnings 2>&1 | tail -3`
Expected: tests pass (query `@cmd_*` buffer for `FAILED` if unsure); clippy clean. Note: any test asserting the exact `tracker_hint` key must be updated to `ledger_hint` — search with `grep tracker_hint @cmd_*` and fix.

- [ ] **Step 8: Commit**

```bash
git add src/librarian/tools/librarian.rs src/librarian/tools/create.rs
git commit -m "feat(ledger-eval): alias ledger_design + dual-read kind [eval-only]"
```

---

### Task 3: Ledger vocabulary flip (agent-visible descriptions + guide) → build ledger binary

**Files:**
- Modify: `src/librarian/tools/artifact.rs:21` (Artifact tool description)
- Modify: `src/librarian/tools/librarian.rs:17,20` (Librarian tool description lines mentioning trackers)
- Create: `$EVAL_BINS/guides/ledger/librarian.md` (ledger-vocabulary guide copy)
- Create: `$EVAL_BINS/codescout-ledger` (binary snapshot)

**Interfaces:**
- Consumes: the `ledger_design` alias + `ledger_hint` from Task 2.
- Produces: `$EVAL_BINS/codescout-ledger` — a binary whose agent-visible tool descriptions say "ledger" and whose auto-injected/served vocabulary matches the delivered ledger guide; consumed by `arm-ledger.yaml`.

- [ ] **Step 1: Flip the load-bearing agent-visible description strings**

Only the strings an agent reads in tool schemas need flipping (internal symbol names stay — that is Phase 1). Apply with `edit_file` on each string:
- `artifact.rs:21` — `"...refreshes their body via a persistent prompt; call librarian(tracker_design) before creating one..."` → replace `tracker` with `ledger` and `tracker_design` with `ledger_design` in that sentence.
- `librarian.rs:17` — in the action list description, `tracker_design:` clause → `ledger_design:`.
- `librarian.rs:20` — `tracker_design: return teaching prompt ... (call BEFORE artifact(create) for trackers)` → `ledger_design: ... for ledgers`.

- [ ] **Step 2: Sweep for any remaining agent-visible "tracker" in tool descriptions**

Run: `grep -rn -i tracker src/librarian/tools/artifact.rs src/librarian/tools/librarian.rs src/librarian/tools/append_entry.rs src/librarian/tools/find.rs 2>&1`
Expected: review each hit; flip any that appear inside a `description`/`enum`/schema string the agent sees. Leave Rust identifiers, module paths, and comments alone.

- [ ] **Step 3: Verify it still builds and lints**

Run: `cargo clippy -- -D warnings 2>&1 | tail -3`
Expected: clean (string edits don't change types).

- [ ] **Step 4: Author the ledger-vocabulary guide**

```bash
mkdir -p "$EVAL_BINS/guides/ledger"
cp src/prompts/guides/librarian.md "$EVAL_BINS/guides/ledger/librarian.md"
```
Then edit `$EVAL_BINS/guides/ledger/librarian.md` (via `edit_markdown`/`edit_file`) replacing the concept vocabulary: `tracker`→`ledger`, `trackers`→`ledgers`, `tracker_design`→`ledger_design`, `tracker-conventions`→`ledger-conventions`. Preserve every path, id, tool name, and code sample that is NOT the concept word.

- [ ] **Step 5: Verify the ledger guide has zero concept-"tracker" left**

Run: `grep -c -i tracker "$EVAL_BINS/guides/ledger/librarian.md"`
Expected: `0` (or only unavoidable literal paths — inspect any residual hit).

- [ ] **Step 6: Build and snapshot the ledger binary**

```bash
cargo rb
cp target/release/codescout "$EVAL_BINS/codescout-ledger"
```

- [ ] **Step 7: Smoke-test the ledger binary routes `ledger_design`**

Run: `printf '' ; "$EVAL_BINS/codescout-ledger" --version`
Expected: version string. (Functional routing is covered by the Task 2 unit test; the eval run exercises it live.)

- [ ] **Step 8: Restore the shared binary to the tracker baseline**

The `~/.cargo/bin/codescout` symlink now points at the ledger build; restore it so interactive/live MCP use is unaffected:
```bash
git checkout experiments && cargo rb
```
Expected: back on `experiments`, `target/release/codescout` rebuilt as the tracker baseline. Return to the branch only if further ledger edits are needed: `git checkout ledger-eval`.

- [ ] **Step 9: Commit the vocabulary flip**

```bash
git checkout ledger-eval
git add src/librarian/tools/artifact.rs src/librarian/tools/librarian.rs
git commit -m "feat(ledger-eval): flip agent-visible tool descriptions to ledger [eval-only]"
```

---

### Task 4: Scenario — append-shape (the primary discriminator)

**Files:**
- Create: `prompt-engineering/scenarios/ledger-vs-tracker/append-shape/scenario.yaml`
- Create: `prompt-engineering/scenarios/ledger-vs-tracker/arm-tracker.yaml`
- Create: `prompt-engineering/scenarios/ledger-vs-tracker/arm-ledger.yaml`

**Interfaces:**
- Consumes: `$EVAL_BINS/codescout-tracker`, `$EVAL_BINS/codescout-ledger`, and the two guide files from Tasks 1 & 3.
- Produces: a runnable scenario+arm set; consumed by Task 6.

- [ ] **Step 1: Write the append-shape scenario**

Create `scenarios/ledger-vs-tracker/append-shape/scenario.yaml`. It seeds a session-log artifact that already contains an `## F-1` entry, then asks the agent to record a new observation. Correct behavior appends `## F-2` (scoped insert / `append_entry`); wrong behavior rewrites the whole body. Substitute the arm's binary path for `<MCP_BIN>` — the setup binary must match the arm under test, so this scenario is run once per arm via the arm's `scenarios:` selector (see arms below).

```yaml
name: ledger-vs-tracker append-shape — append entry vs overwrite body
description: >
  Discriminator for whether the concept word cues append vs overwrite. Seeds a
  session-log artifact with one existing F-N entry into an isolated catalog
  (LIBRARIAN_DB). PASS iff the agent ADDS a new entry without rewriting the whole
  body: a scoped edit_markdown insert_* OR artifact(append_entry), and NOT a
  full-body artifact(update) body-replace, create_file, or Write over the file.
input:
  message: >
    Record a new observation in the bug-fix session log: "F-2 — flaky test in
    catalog reindex under concurrent writes". Add it to the existing log.
mode: trace
runs: 10
setup:
  commands:
    - "git init -q && git -c user.email=fixture@example.com -c user.name=fixture commit -q --allow-empty -m init"
    - "$MCP_BIN artifact create --kind $KIND --title 'Bug-fix session log' --rel-path docs/trackers/bug-fix-session-log.md --body '## F-1 — panic on empty catalog\n\n**Observed:** empty DB. **Fix:** guard.\n'"
assertions:
  trace:
    - any_of:
        - tool_called: { name: mcp__codescout__artifact, args: { action: append_entry } }
        - tool_called: { name: mcp__codescout__edit_markdown, args: { action: insert_after } }
        - tool_called: { name: mcp__codescout__edit_markdown, args: { action: insert_before } }
    - tool_not_called: { name: Write }
    - tool_not_called: { name: mcp__codescout__create_file }
```

Note: `$MCP_BIN` / `$KIND` are shell-expanded in `setup.commands` by prompt-tdd's run env — the arm files export them. If the harness does not expand env inside `setup.commands`, fall back to two sibling scenario dirs (`append-shape-tracker/`, `append-shape-ledger/`) with the values inlined; verify expansion in Task 6 Step 1 before scaling to `runs: 10`.

- [ ] **Step 2: Write `arm-tracker.yaml`**

```yaml
# Tracker arm: current-vocabulary binary + tracker guide as system prompt.
registry: anthropic-mcp
anthropic_mcp:
  session:
    model: sonnet
    max_turns: 15
    via_subscription: true
    config_dir: ~/.prompt-tdd/profiles/plugin-free
    mcp_command: /tmp/claude-1000/-home-marius-work-claude-codescout/f62d42b2-f5da-4aea-be7c-21c309cabde8/scratchpad/eval-bins/codescout-tracker
    mcp_args: ["start", "--transport", "stdio"]
  prompts:
    librarian-guide:
      type: skill
      path: /tmp/claude-1000/-home-marius-work-claude-codescout/f62d42b2-f5da-4aea-be7c-21c309cabde8/scratchpad/eval-bins/guides/tracker/librarian.md
  env:
    KIND: tracker
    MCP_BIN: /tmp/claude-1000/-home-marius-work-claude-codescout/f62d42b2-f5da-4aea-be7c-21c309cabde8/scratchpad/eval-bins/codescout-tracker
defaults:
  timeout: 300
max_cost_per_scenario: 10.0
scenarios: append-shape
```

- [ ] **Step 3: Write `arm-ledger.yaml`**

Identical to `arm-tracker.yaml` except `mcp_command`, the guide `path`, and `env` point at the ledger snapshot:
```yaml
    mcp_command: /tmp/claude-1000/-home-marius-work-claude-codescout/f62d42b2-f5da-4aea-be7c-21c309cabde8/scratchpad/eval-bins/codescout-ledger
    # prompts.librarian-guide.path:
      # /tmp/.../scratchpad/eval-bins/guides/ledger/librarian.md
  # env:
    KIND: ledger
    MCP_BIN: /tmp/.../scratchpad/eval-bins/codescout-ledger
```
(Write the full absolute paths, not `...`.)

- [ ] **Step 4: Commit the scenario (in the prompt-engineering repo)**

```bash
cd /home/marius/work/claude/prompt-engineering
git add scenarios/ledger-vs-tracker/
git commit -m "test(ledger-eval): append-shape scenario + tracker/ledger arms"
```

---

### Task 5: Scenario — snapshot-shape (the split discriminator)

**Files:**
- Create: `prompt-engineering/scenarios/ledger-vs-tracker/snapshot-shape/scenario.yaml`

**Interfaces:**
- Consumes: the same arms from Task 4 (add `snapshot-shape` to each arm's `scenarios:` list, or run with an explicit path in Task 6).
- Produces: the second scenario; its result decides wholesale-vs-split.

- [ ] **Step 1: Write the snapshot-shape scenario**

Seeds a goal artifact whose body is an auto-generated projection carrying an explicit "regenerate, do not hand-append" marker. Correct behavior updates the projection (params update / targeted regen); wrong behavior blindly appends a raw entry. Trace check is looser here, so pair it with a tier-3 judge.

```yaml
name: ledger-vs-tracker snapshot-shape — regenerate projection vs blind append
description: >
  The split discriminator. Seeds a goal artifact whose body is a rendered snapshot
  (marked auto-generated). Correct: update/regenerate the projection. Wrong: append
  a raw row as if it were an append-only log. Measures whether the concept word
  over-cues append on a snapshot-shaped artifact.
input:
  message: >
    The auth-migration goal is now 60% complete (was 40%) and the OAuth callback
    subtask just finished. Update the goal artifact to reflect this.
mode: trace
runs: 10
setup:
  commands:
    - "git init -q && git -c user.email=fixture@example.com -c user.name=fixture commit -q --allow-empty -m init"
    - "$MCP_BIN artifact create --kind $KIND --title 'Auth migration goal' --rel-path docs/trackers/auth-migration-goal.md --body '<!-- AUTO-GENERATED from params: regenerate, do not hand-append -->\n\n## Progress: 40%\n\n- [x] schema\n- [ ] OAuth callback\n'"
assertions:
  trace:
    - tool_not_called: { name: mcp__codescout__artifact, args: { action: append_entry } }
  judge:
    - rubric: >
        Did the agent UPDATE the existing progress projection (edit the Progress
        line / checklist, or update params and refresh) rather than appending a new
        dated entry or duplicating the block? Score 1 if it updated in place, 0 if
        it appended/duplicated.
      threshold: 0.7
      model: opus
      via_subscription: true
```

- [ ] **Step 2: Add `snapshot-shape` to both arms**

In `arm-tracker.yaml` and `arm-ledger.yaml`, change `scenarios: append-shape` to select both — either `scenarios: [append-shape, snapshot-shape]` if the harness accepts a list, else run each scenario dir explicitly in Task 6.

- [ ] **Step 3: Commit**

```bash
cd /home/marius/work/claude/prompt-engineering
git add scenarios/ledger-vs-tracker/
git commit -m "test(ledger-eval): snapshot-shape split-discriminator scenario"
```

---

### Task 6: Run the A/B + ablate, pre-register threshold, record verdict

**Files:**
- Create: `prompt-engineering/scenarios/ledger-vs-tracker/RESULTS.md` (verdict + numbers)
- Modify: `docs/superpowers/specs/2026-07-10-tracker-to-ledger-rename-design.md` (record the gate outcome)

**Interfaces:**
- Consumes: everything above.
- Produces: the go/no-go verdict and the wholesale-vs-split decision that unblocks (or cancels) Phase 1.

- [ ] **Step 1: Smoke-run a single iteration per arm (verify env expansion + fixtures)**

```bash
cd /home/marius/work/claude/prompt-engineering
LIBRARIAN_DB=$(mktemp -u) .venv/bin/prompt-tdd run --config scenarios/ledger-vs-tracker/arm-tracker.yaml --runs 1
```
Expected: the scenario executes; the seeded artifact is created (no "path exists"/"unknown kind" error). If `$MCP_BIN`/`$KIND` did not expand in `setup.commands`, apply the sibling-dir fallback from Task 4 Step 1 before continuing.

- [ ] **Step 2: Pre-register the threshold in RESULTS.md**

Create `scenarios/ledger-vs-tracker/RESULTS.md` with the decision rule filled in BEFORE the full run, e.g.:
```markdown
# Ledger-vs-Tracker eval — pre-registered
- N = 10 runs/arm/scenario, model sonnet.
- Append-shape PASS gate: ledger correct-append rate ≥ tracker + 20 percentage points.
- Snapshot-shape regression gate: ledger correct-update rate ≥ tracker − 10 pp (no meaningful regression).
- Decision: both gates met → wholesale rename. Append gate met, snapshot regressed → split. Append gate unmet → abandon.
```

- [ ] **Step 3: Run both arms at full N, both scenarios**

```bash
LIBRARIAN_DB=$(mktemp -u) .venv/bin/prompt-tdd run --config scenarios/ledger-vs-tracker/arm-tracker.yaml
LIBRARIAN_DB=$(mktemp -u) .venv/bin/prompt-tdd run --config scenarios/ledger-vs-tracker/arm-ledger.yaml
```
Expected: per-scenario pass counts out of 10 for each arm. Capture the JSON: `.venv/bin/prompt-tdd report --format json`.

- [ ] **Step 4: Run the ablate control on the ledger arm**

```bash
LIBRARIAN_DB=$(mktemp -u) .venv/bin/prompt-tdd run --config scenarios/ledger-vs-tracker/arm-ledger.yaml --ablate
```
Expected: the guide is stripped; append-shape should degrade relative to guide-present, confirming the delivered guide (not just server_instructions) carries signal. A non-degrading ablate means the scenario isn't measuring the guide — investigate before trusting the A/B.

- [ ] **Step 5: Record the verdict and update the spec**

Fill the measured numbers into `RESULTS.md`, apply the pre-registered decision rule, and record the outcome in the spec's Phase 0 section via `edit_markdown`:
```
edit_markdown("docs/superpowers/specs/2026-07-10-tracker-to-ledger-rename-design.md",
  action="insert_after", heading="### Decision rule",
  content="**Gate outcome (2026-07-10):** <wholesale | split | abandoned> — <numbers>.")
```

- [ ] **Step 6: Commit results in both repos**

```bash
cd /home/marius/work/claude/prompt-engineering && git add scenarios/ledger-vs-tracker/RESULTS.md && git commit -m "test(ledger-eval): record gate verdict"
cd /home/marius/work/claude/codescout && git add docs/superpowers/specs/2026-07-10-tracker-to-ledger-rename-design.md && git commit -m "docs(specs): record ledger-vs-tracker gate outcome"
```

- [ ] **Step 7: Hand off to Phase 1 (only if the gate passed)**

If wholesale/split: invoke `superpowers:writing-plans` for the Phase 1 migration, scoped by the gate outcome, reusing the `ledger-eval` branch's dual-read alias as the migration-safety layer. If abandoned: delete the `ledger-eval` branch and close out.

---

## Self-Review

**Spec coverage:**
- Aliased-binary fidelity (spec decision 4) → Tasks 2–3. ✓
- Append-shape + snapshot-shape scenarios (spec Phase 0 Scenarios) → Tasks 4–5. ✓
- `--ablate` control (spec Phase 0 pattern) → Task 6 Step 4. ✓
- Decision rule / wholesale-vs-split (spec Decision rule) → Task 6 Steps 2, 5. ✓
- Dual-read alias reused for Phase 1 safety (spec Insight) → Task 6 Step 7. ✓
- `runs: 10`, sonnet, plugin-free profile (spec + harness config) → Global Constraints, arms. ✓

**Known soft spots (flagged inline, resolved during execution):**
- prompt-tdd env expansion inside `setup.commands` is unverified → Task 6 Step 1 gates it with a sibling-dir fallback.
- The snapshot-shape trace assertion is inherently looser than append-shape → mitigated with a tier-3 judge.
- Whether `scenarios:` accepts a list → Task 5 Step 2 gives the explicit-path fallback.

**Type/name consistency:** `ledger_design` (action), `ledger_hint` (result key), `codescout-tracker`/`codescout-ledger` (binaries), `$EVAL_BINS` — used consistently across tasks. ✓
