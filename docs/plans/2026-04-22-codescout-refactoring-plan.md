# Codescout Refactoring Plan

**Created:** 2026-04-22
**Branch:** `refactoring` (off `master`)
**Status:** Phase 1 in progress (Hooves 1–2 complete)
**Source review:** see conversation that produced this plan — three voices (Architecture Snow Lion, Refactoring Yak, Docs Lotus Frog).

---

## Goals

Make codescout's structure match its quality. Today the bones are good (clean
`Tool` trait, single `ToolContext`, `RecoverableError` discipline, panic=abort,
`LspProvider` testability seam), but the surface has accumulated fat:

- `src/tools/symbol.rs` — 9 tools + 36 helpers + 5600 lines of tests in one 9200-line file
- `src/tools/workflow.rs` — 282KB, two unrelated tools fused with hardware probe + prompt builders
- `src/tools/file.rs` — 204KB, all file ops fused
- `src/agent/mod.rs` — 30+ methods on one impl block (coincidental cohesion)
- `src/server.rs::call_tool_inner` — 200-line function with 6 distinct concerns
- 3 prompt surfaces requiring manual sync on every tool rename

The end state: each tool in its own file (10–40KB), provider modules holding
domain logic, `server.rs` reads as a 6-line pipeline, `Agent` impl is partitioned
by concern, and prompt-surface sync is mechanised.

**Non-goals:** rewriting LSP lifecycle, embedding pipeline, or `RecoverableError`
shape. These are essential complexity and the tests have invested in them.

---

## How To Use This Plan

This is a **high-level plan**. Details emerge as each phase starts. Each phase
is sized to be one work session or one PR. Phases are independent in execution
order *except* Phase 1 must complete before Phase 6.

**Before starting any phase:**

1. Read this file and the phase's "Entry conditions" section.
2. Summon the bodhisattvas (instructions below).
3. Verify the entry conditions are met (clean working tree on `refactoring`,
   tests green, clippy clean).
4. Create a phase tracking checklist.

**Working discipline (Yak's law, applies to every phase):**

- One atomic commit per move. Tests green between commits.
- `cargo fmt && cargo clippy -- -D warnings && cargo test --lib` before every commit.
- Push to `refactoring` after each commit (or batch within one session).
- Never combine refactor + behavior change in one commit.
- If tests fail after a move: revert to last green, make the move smaller.
- If a move requires touching > 8 files, split the move.

---

## Summoning Bodhisattvas

Two specialists watch this refactoring:

```
/buddy:summon yak    — Refactoring Yak: structural moves, atomicity, test discipline
/buddy:summon lion   — Architecture Snow Lion: boundaries, coupling, abstraction integrity
```

**The Yak watches each move.** It interrupts when:
- A move is too big (> 8 files, > 1 commit's worth of work)
- Tests fail and the urge is to "press on" instead of revert
- A "refactor" is sneaking in a behavior change
- An extraction has no clear name (sign that the abstraction is wrong)

**The Lion watches the larger shape.** It interrupts when:
- A boundary is being drawn that creates a cycle
- A new abstraction is appearing without a coupling problem to justify it
- A module is acquiring responsibilities outside its name
- The phase output would leave a worse boundary than it found

**Operating mode:** both stay summoned for the duration of a phase. Dismiss
between phases (`/buddy:dismiss yak lion`) so each phase begins with fresh
attention rather than accumulated voice. Re-summon at the start of the next
phase.

If a phase produces a contested judgment call (e.g. "should this helper move
with its tool or into shared helpers?"), the Lion's voice is decisive — boundary
choices outweigh extraction mechanics.

---

## Phases

Each phase has: **Goal** · **Entry conditions** · **Approach** · **Exit
conditions**. Specifics emerge during execution.

### Phase 0 — Foundation (DONE)

**Status:** ✅ Complete. Commits `15cabec` (Hoof 1: dir module) and `775eb22`
(Hoof 2: format extraction). Branch `refactoring` pushed.

What it bought us:
- `src/tools/symbol/` is now a directory module
- `src/tools/symbol/display.rs` holds the 10 `format_compact` helpers
- Pattern proven: per-tool decomposition is safe and mechanical

---

### Phase 1 — Decompose `src/tools/symbol/`

**Goal:** One tool per file. `mod.rs` becomes a thin re-export hub.

**Entry conditions:** Phase 0 complete. Branch `refactoring` clean and pushed.

**Approach:** For each of the 9 tools (`ListSymbols`, `FindSymbol`,
`FindReferences`, `GotoDefinition`, `Hover`, `ReplaceSymbol`, `RemoveSymbol`,
`InsertCode`, `RenameSymbol`):

1. Identify which `mod.rs` helpers the tool uses.
2. Bump those helpers to `pub(super) fn`.
3. Create `src/tools/symbol/<tool>.rs` containing the tool struct, its `impl
   Tool for X`, and any helpers used *only* by this tool.
4. Remove the migrated code from `mod.rs`.
5. Re-export from `mod.rs` so external imports (`tools::symbol::ListSymbols`)
   keep working.
6. Test, commit, push.

After all 9 tools are extracted, surviving helpers in `mod.rs` are by
definition shared. Move them to `src/tools/symbol/helpers.rs` (Phase 1 final
step).

**Order (smallest blast radius first):**
1. `Hover` (~140 lines, leaf)
2. `GotoDefinition` (~165 lines)
3. `FindReferences` (~130 lines + `tag_external_path`)
4. `RemoveSymbol` (~85 lines)
5. `InsertCode` (~100 lines + `classify_file`, `classify_sort_key`, `text_sweep`)
6. `ReplaceSymbol` (~160 lines + edit-shared helpers)
7. `FindSymbol` (~440 lines + `build_by_file`, `make_find_symbol_hint`)
8. `RenameSymbol` (~250 lines + `utf16_to_byte_offset`, `apply_text_edits`)
9. `ListSymbols` (~430 lines + `LIST_SYMBOLS_*` constants, `find_split_point`,
   `count_files_by_subdir`, `ast_class_names_for_dir`, `flat_symbol_count`)
10. **Final:** consolidate surviving shared helpers → `helpers.rs`

**Watch points (Yak):**
- Edit-tool helpers (`editing_start_line`, `editing_end_line`,
  `clamp_range_to_parent`, `collect_all_name_paths`, `find_ast_name_path`,
  `find_insert_before_line`) are shared by 4 edit tools — keep them in one
  place once the edit tools are extracted, likely as `edit_helpers.rs`.
- `fetch_validated_symbol` calls `find_unique_symbol_by_name_path` — both
  must move together or both stay in shared scope.

**Watch points (Lion):**
- Resist the urge to introduce a `ToolBase` trait or shared abstraction during
  this phase. The `Tool` trait IS the abstraction. Per-tool files should
  remain plain.
- The natural seam is **per-tool**, not per-category (read/write). Resist
  intermediate groupings like `query.rs`/`edit.rs` — those are arbitrary.

**Exit conditions:**
- 9 files in `src/tools/symbol/` (one per tool) + `display.rs` + `helpers.rs`
  + `edit_helpers.rs` + `mod.rs` (re-exports only)
- `mod.rs` < 100 lines
- All 1751 tests passing
- Clippy clean

---

### Phase 2 — Decompose `src/tools/workflow.rs`

**Goal:** Split `Onboarding` and `RunCommand` into separate files. Lift
hardware-probe code out of `tools/` entirely (it isn't a tool).

**Entry conditions:** Phase 1 complete.

**Approach:**
1. Extract hardware probes (`probe_ollama`, `probe_nvidia`, `probe_amd`,
   `probe_ram`, `detect_hardware_context`, `model_options_for_hardware`,
   `HardwareContext`, `GpuInfo`, `ModelOption`) to **`src/hardware.rs`**.
   This is a provider, not a tool.
2. Extract prompt builders (`build_system_prompt_draft`,
   `build_subagent_preamble`, `build_subagent_epilogue`,
   `build_per_project_prompt`, `build_workspace_instructions`,
   `language_navigation_hints`, `language_patterns`,
   `build_language_patterns_memory`) to **`src/prompts/builders.rs`**.
3. Move `Onboarding` to **`src/tools/onboarding.rs`**.
4. Move `RunCommand` to **`src/tools/run_command.rs`**.
5. Delete `src/tools/workflow.rs`.

**Watch points (Lion):**
- `src/prompts/` already exists for prompt assets. Adding `builders.rs` is
  natural. Don't create a new module just for the builders.
- The hardware module sits at crate root — that's fine for a leaf utility.
  Don't overstructure it (no `src/hardware/mod.rs` directory unless probes
  grow further).

**Exit conditions:**
- `src/tools/workflow.rs` deleted
- `src/hardware.rs` + `src/prompts/builders.rs` created
- `src/tools/onboarding.rs` + `src/tools/run_command.rs` exist
- Tests passing, clippy clean

---

### Phase 3 — Decompose `src/tools/file.rs`

**Goal:** One file operation per file.

**Entry conditions:** Phase 2 complete.

**Approach:** Same per-tool pattern as Phase 1. Likely splits into:
- `read_file.rs`, `write_file.rs`, `create_file.rs`, `edit_file.rs`,
  `glob.rs`, `grep.rs`, `list_dir.rs`
- `file_helpers.rs` for shared utilities

Audit which helpers in `path_security` belong with the tool layer vs the
crate-level `util/`.

**Watch points (Yak):**
- `edit_file` is large and has the gate logic for blocking structural edits.
  Keep that logic intact during the move — it's load-bearing.

**Watch points (Lion):**
- After this split, see if `file_helpers.rs` is genuinely shared or whether
  helpers should live with their primary tool. Don't preserve a shared file
  for the sake of having one.

**Exit conditions:**
- `src/tools/file.rs` deleted, replaced by per-op files
- Tests passing, clippy clean

---

### Phase 4 — Partition `src/agent/mod.rs` impl

**Goal:** Split the 30-method `impl Agent` block into 4–5 cohesive impl blocks.
Same struct, same methods, same behavior — just organised by concern.

**Entry conditions:** Phases 1–3 complete (this is independent but cleaner
when tools are already organized).

**Approach:**
1. Identify clusters by reading the impl methods:
   - **Lifecycle:** `new`, `activate`, `activate_within_workspace`,
     `require_project_root`, `resolve_root`, `switch_focus`,
     `is_project_explicitly_activated`
   - **Embedding:** `get_or_create_embedder`, `embedding_semaphore` access
   - **Indexing:** indexing state, `library_index_states`, `should_nudge`
   - **Workspace/discovery:** `discovered_projects`, `workspace_summary`,
     `is_home`, `home_root`, `workspace_project_memories`
   - **Project/files:** `with_project`, `project_root`, `project_status`,
     `mark_file_dirty`, `dirty_file_count`, `dirty_files_arc`,
     `reload_config_if_project_toml`
   - **Configuration:** `security_config`, `lsp_mux_override`,
     `library_registry`, `save_library_registry`
2. Split into separate `impl Agent { ... }` blocks, ordered top-to-bottom by
   cluster. Add a section comment above each.
3. **Do not split the struct yet.** That's Phase 4b if signals warrant.

**Watch points (Lion):**
- If after partitioning some clusters share zero fields with others, that's
  the signal to split the struct. Until then: don't.
- Resist creating new types just to "tidy" this. The whole-Agent struct
  exists because tools take a single `&Agent` handle. Keep that contract.

**Watch points (Yak):**
- Pure mechanical move. Each cluster extraction is one commit. Tests green
  every time.

**Exit conditions:**
- 4–5 impl blocks in `src/agent/mod.rs`, each labeled
- Tests passing, clippy clean

---

### Phase 5 — Extract `server.rs::call_tool_inner` pipeline

**Goal:** Reduce `call_tool_inner` from 200 lines to ~10 lines that call 6
named helpers.

**Entry conditions:** None (independent).

**Approach:** Extract per concern:
1. `resolve_tool(&req.name)` — tool lookup
2. `parse_input(req.arguments)` — JSON parsing
3. `check_tool_access(&req.name)` — security gate
4. `build_context(progress, peer)` — `ToolContext` construction
5. `acquire_write_guard_if_writing(&req.name, &input)` — write lock
6. `race_against_cancel(tool, input, ctx, cancel_token, timeout)` —
   the cancellation/timeout dance
7. `post_process(result, &req.name)` — root stripping + path note

Each helper is a method on `CodeScoutServer`. The cancellation/timeout
helper is the load-bearing one — keep its `tokio::select! + biased + pending`
shape exactly.

**Watch points (Lion):**
- Don't introduce a `Pipeline` struct. Methods on `CodeScoutServer` are
  fine. The shape *is* a sequence; let the function body show that sequence.

**Watch points (Yak):**
- Each helper extraction is one commit. After all 7, the body should read
  as a flat list of calls.
- The cancellation comment block (`docs/issues/2026-04-16-...`) must
  travel with `race_against_cancel` — it's the load-bearing context.

**Exit conditions:**
- `call_tool_inner` body fits on a screen
- Tests passing, clippy clean

---

### Phase 6 — Lift providers out of tool code

**Goal:** Tool files become thin adapters. Domain logic lives in provider
modules.

**Entry conditions:** Phase 1 complete (tools must already be per-file).

**Approach:** After Phases 1–3, each tool file has helpers that are clearly
domain logic, not tool logic. Lift them to provider modules:

- **`src/symbol/`** — `resolve_glob`, `symbol_to_json`, `validate_symbol_range`,
  `validate_symbol_position`, `classify_reference_path`, `find_*` family
- **`src/fs/`** — path resolution, glob matching, read-path/write-path gates
- **`src/hardware.rs`** — already created in Phase 2
- **`src/text/`** — `utf16_to_byte_offset`, `apply_text_edits`,
  `is_lead_in_line`, `format_line_range`

After this phase, `src/tools/<tool>.rs` files read as: parse input → call
provider → format output. Nothing else.

**Watch points (Lion):**
- This phase draws the cleanest boundary in the codebase. Be deliberate.
  A helper that's used by exactly one tool stays with that tool.
  A helper used by 2+ tools moves to a provider — but ONLY if it has a
  coherent name in the provider's namespace.
- Test imports may need updating. That's fine. Don't preserve test ergonomics
  by leaving production code in the wrong place.

**Watch points (Yak):**
- Extract one provider at a time. Symbol provider first (largest).
  Test after each provider extraction.

**Exit conditions:**
- New `src/symbol/`, `src/fs/`, `src/text/` modules exist
- Tools call into them via `crate::symbol::...`
- `src/tools/*.rs` files are ≤ 100 lines each (typical case)
- Tests passing, clippy clean

---

### Phase 7 — Mechanise prompt-surface sync

**Goal:** Eliminate the manual "grep all three surfaces when renaming a tool"
discipline that CLAUDE.md currently enforces by hand.

**Entry conditions:** Phases 1–6 complete (tool boundaries stable).

**Approach (smallest first):**
1. **Quick win:** add a `cargo test` that scans
   `src/prompts/server_instructions.md`,
   `src/prompts/onboarding_prompt.md`, and
   `build_system_prompt_draft` for tool names, asserting every name maps
   to a real tool registered in `CodeScoutServer::from_parts`. Breaks
   the build at rename time, not at prod-runtime.
2. **Optional medium step:** template the surfaces. Tool list comes from
   `CodeScoutServer::tools`, rendered into shared markdown templates at
   build time.

Step 1 alone is enough. Step 2 is a Phase 8 candidate if needed.

**Watch points (Lion):**
- Don't generate prompts from tool metadata at runtime. That couples the
  prompt language to the tool schema and prevents per-prompt copy-tuning.
  Build-time templating is fine; runtime generation is not.

**Exit conditions:**
- Prompt-surface drift produces a build failure
- CLAUDE.md's manual-grep rule can be deleted (or downgraded to "the test
  catches this")
- Tests passing, clippy clean

---

### Phase 8 — Documentation reorg (Frog's phase)

**Goal:** README serves new readers, CONTRIBUTING serves contributors,
CLAUDE.md serves the LLM, ARCHITECTURE.md is the canonical structural
reference. No file serves three audiences.

**Entry conditions:** Phase 7 complete (the architecture is now stable).

**Approach:**
1. **Slim README.md** to: what it is (1 sentence), install (1 command),
   use (1 example), where to learn more (links), how to contribute (link).
2. **Create `CONTRIBUTING.md`** with: branch strategy, git workflow,
   commit discipline, release cycle, ship sequence — extracted from
   today's CLAUDE.md.
3. **Trim `CLAUDE.md`** to LLM-runtime guidance only: tool-selection
   rules, prompt-surface review rule, `json!("ok")` rule, Iron Laws.
4. **Author canonical `docs/ARCHITECTURE.md`**: the six-box diagram, the
   three load-bearing abstractions (`Tool`, `ToolContext`,
   `RecoverableError`), LSP lifecycle, embedding pipeline, write-gate
   flow. Replaces scattered architecture knowledge in CLAUDE.md +
   memories + module preambles.
5. **Audit `docs/plans/`**: delete shipped plans (point to commit ranges
   in CHANGELOG.md instead).
6. **Refactor `docs/TODO-tool-misbehaviors.md`**: open bugs as GitHub
   issues, move LLM-relevant quirks into a "Known Tool Quirks" section
   in CLAUDE.md, delete the file.

**Watch points (Frog):**
- Resist preserving stale process docs. If a plan shipped, the plan is
  done — its lessons live in the code or in a memory.
- Every new doc must name its reader in the first paragraph.

**Exit conditions:**
- Five top-level audiences each have exactly one home
- No file > 500 lines (Frog's lily-pad limit; humans skim past that)

---

## Tracking & Cadence

Each phase produces one PR (or one merge to `master` after review). After
each phase merges:

1. Rebase `refactoring` onto `master`.
2. Update this file's "Status" header for the next phase.
3. Update the codescout `architecture` memory if structure changed
   meaningfully.
4. Dismiss the phase's bodhisattvas; summon fresh ones for the next phase.

If a phase reveals that the plan was wrong, update **this file**, then
proceed. The plan is the artifact; the conversation that produced it is
ephemeral.

---

## What This Plan Does Not Cover

- **Performance work.** That's the Lammergeier's domain. If profiling
  reveals hot paths during refactoring, note them in
  `docs/issues/` for separate triage.
- **New features.** Refactoring is structure-only. Behavior changes are
  separate commits, separate PRs.
- **Test improvements.** If tests reveal weakness during refactoring
  (flaky, slow, implementation-coupled), note in
  `docs/TODO-tool-misbehaviors.md` (or its successor) for separate work.
- **LSP lifecycle restructuring.** Essential complexity. Out of scope.
- **Embedding pipeline restructuring.** Essential complexity. Out of scope.
