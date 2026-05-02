# Codescout Refactoring Plan

**Created:** 2026-04-22
**Branch:** `refactoring` (off `master`)
**Status:** Phases 0–7 complete. Phase 6 complete (2026-05-02). Phase 8 partial (8.3 deferred).
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

**Status:** ✅ Complete (Phase 1 + 1b, 2026-04-23). 16 atomic commits on `refactoring`;
1751 tests green and clippy clean between every commit. Phase 1b shrank
`src/tools/symbol/mod.rs` from 6793 → 28 lines by extracting `path_helpers.rs`,
`symbol_query.rs`, `edit_helpers.rs`, and relocating the test block to `tests.rs`.
See `docs/plans/2026-04-23-codescout-refactoring-plan-phase-1b.md` for details.

| # | Commit | Tool | Helpers bumped to `pub(super)` |
|---|--------|------|--------------------------------|
| 1 | `9b8a9a0` | `Hover` | LspTimer, require_path_param, resolve_read_path, get_lsp_client, tag_external_path |
| 2 | `9fa646a` | `GotoDefinition` | uri_to_path |
| 3 | `4a10403` | `FindReferences` | resolve_library_roots, classify_reference_path, find_unique_symbol_by_name_path, path_in_excluded_dir |
| 4 | `a41a25b` | `RemoveSymbol` | resolve_write_path, guard_not_markdown, fetch_validated_symbol, editing_start_line, editing_end_line, clamp_range_to_parent, write_lines, find_parent_symbol |
| 5 | `3dcd33c` | `InsertCode` | (reuses #4's helpers) |
| 6 | `85420b6` | `ReplaceSymbol` | count_symbols_by_name_path, collect_all_name_paths, find_ast_name_path |
| 7 | `97a79c8` | `FindSymbol` | get_path_param, is_glob, resolve_glob, format_library_path, matches_kind_filter, collect_matching, symbol_to_json, validate_symbol_range, resolve_range_via_document_symbols, symbol_name_matches |
| 8 | `88876ce` | `RenameSymbol` | apply_text_edits, text_sweep, TextualMatch |
| 9 | `a81ece2` | `ListSymbols` | filter_variable_symbols; list-mode helpers kept private to `list_symbols.rs` |

**What was achieved:**
- `src/tools/symbol/` is now a directory module with one file per tool.
- `hover.rs` (152), `goto_definition.rs` (171), `find_references.rs` (147),
  `remove_symbol.rs` (95), `insert_code.rs` (97), `replace_symbol.rs` (176),
  `find_symbol.rs` (494), `rename_symbol.rs` (264), `list_symbols.rs` (618).
- `display.rs` (432) holds all `format_compact` helpers.
- `mod.rs` (6793) now contains only shared helpers + the existing ~5600-line
  test suite. It is no longer mixed with per-tool `impl Tool` bodies.

**What was deferred (Phase 1.10 → 1b):**
- Extraction of shared helpers out of `mod.rs` into `helpers.rs` +
  `edit_helpers.rs`.
- Tests intermingled with helpers (~5600 lines of tests reaching helpers via
  `use super::*;`) make the consolidation higher-risk than any single move
  done in Phase 1. A dedicated phase — Phase 1b — is planned.

**Lesson carried forward (Yak):**
- Per-tool moves fell into a repeatable rhythm: read bodies → create file with
  copied impl → `remove_symbol` for impl + struct → edit mod.rs to add
  `mod X; pub use X::Tool;` → bump helper visibilities → `cargo build && fmt
  && clippy && test` → commit. The mechanical loop is the key; deviating
  from it (bundling two tools, editing in prose instead of `remove_symbol`)
  would have produced rollbacks.
- Two consistent friction points: (1) tests in `mod.rs` referencing display
  formatters had to be re-imported after each tool move; (2) `remove_symbol`
  leaves orphan blank lines that clippy sometimes flags — always read ±5
  lines around the removal and clean up.

---
### Phase 2 — Decompose `src/tools/workflow.rs`

**Status:** ✅ DONE (commits `3947cd8` → `6f1bf8f`).

**Goal:** Split `Onboarding` and `RunCommand` into separate files. Lift
hardware-probe code out of `tools/` entirely (it isn't a tool).

**Entry conditions:** ✅ Phase 1 + 1b complete on `refactoring` (HEAD `ae8abd2`).
Current `src/tools/workflow.rs` is 7275 lines (3383 helpers/impls + 3892 tests).

**Next-session resume checklist:**
1. Branch is `refactoring`, working tree clean. Run
   `cargo test --lib` to confirm 1751 tests still green.
2. Execute the four sub-phases below as separate commits, in order. Each
   sub-phase should follow the Phase 1b rhythm: read bodies → create new
   file → `remove_symbol` from workflow.rs → fix imports → `cargo fmt &&
   clippy --lib -- -D warnings && test --lib` → commit.
3. After all four sub-phases land, `src/tools/workflow.rs` should be deleted
   and `src/tools/mod.rs` should declare `mod hardware;` (at crate root) +
   `mod onboarding;` + `mod run_command;` + `mod prompts/builders;`.

**Sub-phase plan (one commit each):**

- **2.1 `src/hardware.rs`** — move: `HardwareContext`, `GpuInfo`,
  `ModelOption`, `ollama_tcp_addr`, `probe_ollama`, `probe_nvidia`,
  `probe_amd`, `probe_ram`, `detect_hardware_context`,
  `model_options_for_hardware`. Add `mod hardware;` to `src/lib.rs` (crate
  root). Update workflow.rs to `use crate::hardware::{...}`.
- **2.2 `src/prompts/builders.rs`** — move: `language_navigation_hints`,
  `language_patterns`, `build_language_patterns_memory`,
  `build_system_prompt_draft`, `build_subagent_preamble`,
  `build_subagent_epilogue`, `build_per_project_prompt`,
  `build_synthesis_prompt`, `build_workspace_instructions`,
  `build_buffered_onboarding_instructions`,
  `build_buffered_refresh_instructions`,
  `build_prompt_refresh_subagent_prompt`, `build_heading_map`. Add
  `pub mod builders;` to `src/prompts/mod.rs`.
- **2.3 `src/tools/onboarding.rs`** — move `Onboarding` struct,
  `impl Tool for Onboarding`, `handle_refresh_prompt`,
  `handle_already_onboarded`, `perform_full_onboarding`,
  `gather_protected_memory_state`, `gather_project_context`,
  `GatheredContext`, `format_onboarding`, `ONBOARDING_VERSION`,
  `onboarding_version_stale`, `client_name`, `is_subagent_capable`. Add
  `mod onboarding; pub use onboarding::Onboarding;` to `src/tools/mod.rs`.
- **2.4 `src/tools/run_command.rs`** — move `RunCommand` + `impl Tool for
  RunCommand` + all `run_command_*` helpers (run_command_inner,
  run_command_interactive, handle_successful_output, spawn_background_command,
  inject_tee, resolve_work_dir, parse_timeout_input, get_timeout_u64,
  truncate_output, looks_like_ack_handle, rebuild_buffered_summary,
  format_run_command, TmpfileGuard, AbortOnDrop). Move tests block too.
  Add `mod run_command; pub use run_command::RunCommand;` to
  `src/tools/mod.rs`. **Delete `src/tools/workflow.rs`** in the same commit.

**Watch points (from Phase 1b experience):**
- `remove_symbol` of large symbols leaves orphan blank lines — always read
  ±5 lines after each removal.
- Tests inside the moved code must move with it. workflow.rs has a single
  3892-line `tests` module at L3383 — split it across the four target files
  (or copy then prune by referenced helper, like Phase 1b.4).
- `ToolContext`, `RecoverableError`, `Tool` trait imports become
  `crate::tools::{...}` once the code lives outside the tools module.

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

**Status:** ✅ DONE (commits `ad9f70e` → `b79d09b`).

**Goal:** One file operation per file.

**Entry conditions:** ✅ Phase 2 complete.

**Sub-phases (one commit each, all landed):**

- **3.1 `src/tools/read_file.rs`** (`ad9f70e`) — ReadFile + read helpers
  (`strip_buffer_ref_quotes`, `read_from_buffer`, `validate_read_nav_params`,
  `compute_source_tag`, `read_file_text`, `read_json_path_nav`,
  `read_toml_yaml_key`, `read_with_line_range`, `read_full_file`) +
  format helpers (`markdown_coverage`, `format_read_file`,
  `format_read_file_summary`). `markdown.rs` + `memory.rs` updated.
- **3.2 `src/tools/list_dir.rs`** (`ec7ab97`) — ListDir + `format_list_dir`,
  `format_list_dir_tree_body`, `common_path_prefix`.
- **3.3 `src/tools/grep.rs`** (`f0f7cc3`) — Grep + `format_grep`,
  `format_search_simple_mode`, `format_search_context_mode`.
- **3.4 `src/tools/create_file.rs`** (`9248db7`) — CreateFile (no helpers).
- **3.5 `src/tools/glob.rs`** (`dc6e8de`) — Glob + private `format_glob`.
- **3.6 `src/tools/edit_file.rs`** (`b79d09b`) — `git mv file.rs edit_file.rs`:
  only EditFile + `perform_edit` + `def_keywords_for_lang`,
  `find_def_keyword`, `detect_lsp_language`, `infer_edit_hint` remained.
  Bundled tests stayed with edit_file.rs — sibling tools reached via
  explicit `super::super::{read_file,list_dir,grep,create_file,glob}`
  imports added sub-phase by sub-phase.

**Deviation from plan:** No `file_helpers.rs` emerged — each helper stayed
with its primary tool. The Lion's watch point was observed: no shared-file
was preserved for its own sake.

**Deferred cleanup:** The ~3800-line `tests` mod still lives inside
`edit_file.rs`. Splitting it per-tool was judged high-churn for a
follow-up cleanup commit.

**Exit conditions:** ✅ `src/tools/file.rs` deleted; per-op files in place;
1751 tests pass; clippy clean.
### Phase 4 — Partition `src/agent/mod.rs` impl

**Status:** ✅ DONE (commit `e98c8ec`).

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

**Status:** ✅ DONE (commits `6cc878d` → `28a2932`).

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

**Status:** ✅ COMPLETE — 6.1 (symbol) + 6.2 (fs) done. 6.3 (text) deferred:
single-caller helpers, not worth `src/text/` at current scale. 6.4
(tool-file thinning) done 2026-05-02: split `semantic.rs` (4 tools, 2198 lines)
and `markdown.rs` (2 tools, 1982 lines) into directory modules — each tool now
in its own file, tests extracted to `tests.rs`. See `docs/TODO-phase6-provider-lifts.md`.

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

**Status:** ✅ DONE (commit `555b1ac` — step 1 only; step 2 templating
not pursued). CLAUDE.md's manual-grep rule downgraded to a pointer at
the test.

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

### Phase 7b — `src/tools/` file splits (DONE)

**Status:** ✅ DONE (2026-05-02). Tracked in `docs/trackers/tools-mod-refactor-2026-05.md`.

Three mechanical splits, done B→C→A:
- **B** — `file_summary.rs` tests extracted to `file_summary/tests.rs` (directory module pattern)
- **C** — `run_command/mod.rs` (~1196 lines) split into `inner.rs` / `interactive.rs` / `output.rs`
- **A** — `tools/mod.rs` (~1487 lines) split into `core/types.rs` + `core/params.rs` + `core/guards.rs` + `core/tests.rs`

No file in `src/tools/` exceeds ~600 lines. All callers unaffected (re-export facade preserves `crate::tools::X` paths).

---

### Phase 8 — Documentation reorg (Frog's phase)

**Status:** 🟡 PARTIAL. 8.1 + 8.2 done (commits `4f242c7`, `1c98e00`):
archived 8 shipped plans + 885-line bug log. 8.5 done 2026-05-02: remaining
shipped plans archived; only live plan + 2 deferred ideas remain in `docs/plans/`.
Plan's other sub-tasks turned out to be already-done on the ground (README 133
lines, CONTRIBUTING.md exists) so they were not executed. 8.3 (trim CLAUDE.md)
deferred — user hand-edits that file. 8.4 dropped (ARCHITECTURE.md removed;
architecture knowledge lives in codescout memory). 8.6 dropped (misbehaviors
file stays as live tracker per user instruction).

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
4. ~~**Author canonical `docs/ARCHITECTURE.md`**~~ — dropped; architecture
   knowledge lives in codescout memory (`architecture`). No doc needed.
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
