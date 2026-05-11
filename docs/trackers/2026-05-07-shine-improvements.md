# Shine-In-All-Directions Tracker

**Created:** 2026-05-07 · **Status:** open · **Scope:** codescout + claude-plugins (buddy, codescout-companion)

Punch-list of changes that would lift the project from "above industry mean" to
"shines in all directions." Items prioritized P0 → P3 by impact × ease.

**Reviewed 2026-05-07 by:**
- Architecture Snow Lion — added I-18 (state contract), reframed I-12, paused I-08
- Refactoring Yak — added Y-A/Y-B/Y-C prerequisite safety nets, phased I-01 and I-05, parked I-08

Each item: surface, observed problem, what fixing requires, why it matters now.

## Shipped — 2026-05-08 session

| ID | Item | Evidence |
|---|---|---|
| Y-C | Prompt-surfaces roundtrip snapshot test | `src/prompts/mod.rs::tests` — 3 new tests (`prompt_surfaces_server_instructions_snapshot`, `prompt_surfaces_onboarding_snapshot`, `prompt_surfaces_system_prompt_draft_empty_snapshot`) compare each surface against fixtures in `tests/fixtures/prompt_surfaces/`. `UPDATE_PROMPT_SNAPSHOTS=1` env var regenerates intentionally. Locks the byte-level contract that I-01 must preserve. |
| I-01 Phase 1a | Single-source-of-truth `source.md` for two .md surfaces | `src/prompts/source.md` (new — concatenation of `server_instructions.md` + `onboarding_prompt.md` with `<!-- @surface NAME -->` / `<!-- @end -->` tags). `src/prompts/source.rs::extract_surface` slices a named surface byte-for-byte. 5 unit tests assert `extract_surface(SOURCE, "server_instructions") == SERVER_INSTRUCTIONS` and same for `onboarding_prompt`. Phase 1b: switch `include_str!` to read source.md slices. Phase 1c: handle `build_system_prompt_draft` skeleton. |
| I-01 Phase 2 | Switch `SERVER_INSTRUCTIONS` / `ONBOARDING_PROMPT` to source.md slices | `build.rs::emit_prompt_surfaces` slices `src/prompts/source.md` into `OUT_DIR/{surface}.md` files at compile time; `src/prompts/mod.rs` constants `include_str!` from `OUT_DIR`. `pub const &str` semantics preserved — no call-site changes. cargo:rerun-if-changed pinned to source.md + build.rs. |
| I-01 Phase 3 | Delete unused originals + redirect callers | `src/prompts/server_instructions.md` and `src/prompts/onboarding_prompt.md` deleted (now sourced from `source.md`). `src/server.rs::prompt_surfaces_reference_only_real_tools` redirects to runtime constants. `src/prompts/README.md` and `src/tools/onboarding.rs` doc references updated to point at source.md. cargo test --lib: 1893 passed (zero regression). |
| I-01 Phase 3b | Allowlist tripwire + tightening | `src/server.rs::prompt_surfaces_reference_only_real_tools` now also asserts every allowlist entry actually appears backticked in some surface — stale entries fail the test with names listed. Initial fire dropped 20/49 entries (`code`, `detail_level`, `domain_glossary`, `fn`, `include_body`, `language_patterns`, `name`, `new_body`, `new_string`, `old_string`, `path`, `project_overview`, `query`, `read_only`, `replace_all`, `scope`, `symbol`, `system_prompt`, `timeout_secs`, `toml_key`) — defensive bloat from earlier surface revisions. Allowlist now 29 entries; future surface edits are forced to keep it tight. |
| Y-B | Shell-level test for post-tool-use hook (closes the gap) | `claude-plugins/buddy/tests/test_hooks_post_tool_use.sh` (new, 9 assertions) — pipes synthetic events into `bash hooks/post-tool-use.sh`, verifies `state.json` + `narrative.jsonl` side effects, malformed-JSON resilience, missing-session_id fallback. `pre-tool-use.sh` intentionally not duplicated — `test_pre_tool_hook.py` already subprocess()s the bash hook with 11 integration assertions; companion hook tests cover every event hook. Y-B unblocks I-05/I-06/I-07/I-11/I-13. |
| Y-B (extension) | Characterization test for `detect-tools.sh` | `claude-plugins/tests/test-detect-tools.sh` (new, 20 assertions) — locks the bash detection layer's exported variables under fixtures covering each path: empty cwd, routing override, .mcp.json command/args, `.codescout/` vs `.code-explorer/` precedence, project.toml onboarding marker, memories listing, system-prompt detection, BLOCK_READS routing, legacy routing config name fallbacks. Required gating before I-11 (Python rewrite) — without it the conversion silently loses CS_SYSTEM_PROMPT bytes through eval-quoting. All 10 companion suites green: 94 assertions. |
| I-11 | Port `detect-tools.sh` → `scripts/detect.py` | `claude-plugins/codescout-companion/scripts/detect.py` (new, ~200 LOC, stdlib-only) emits shell-eval lines on stdout (default) or `--json` for testing; `shlex.quote` produces bash-safe quoting that survives multi-line / unicode `CS_SYSTEM_PROMPT`. `detect-tools.sh` shrunk from 125 → 18 lines as a thin shim — `eval "$(CWD=$CWD HOME=$HOME CLAUDE_CONFIG_DIR=${CLAUDE_CONFIG_DIR-} python3 .../detect.py)"`. Exported-variable contract unchanged (16 vars including previously-implicit `ROUTING_CONFIG`/`CS_CONFIG_FILE`/`CS_MEMORIES_DIR`). 20 pytest unit tests in `tests/test_detect.py` cover edges shell can't reach: malformed JSON falls through, args-array regex match, ~ expansion, multi-line/unicode prompt round-trip via `bash -c eval`, CLAUDE_CONFIG_DIR vs HOME precedence, shlex.quote round-trip parametrized over 5 hostile values. Total: 94 shell + 20 pytest = 114 green. Surfaced and fixed a latent fixture bug: `${2:-{}}` in `tests/lib/fixtures.sh::write_routing_config` produced invalid JSON (extra trailing `}`) which `jq` silently accepted; Python rejects strictly — separate commit. Bash-impl cmd-name pitfall recorded in commit message: `${VAR:+VAR=$VAR}` cannot produce inline env assignment via param expansion — must use fixed `KEY=${VAR-}`. claude-plugins commits `4eef855` (fixture fix) + `dd06afc` (I-11). |

## Shipped — 2026-05-07 session

The following items landed in this session. Tracker entries below remain for
historical context; the work is done.

| ID | Item | Evidence |
|---|---|---|
| Y-A | Characterization tests for `symbols()` cold-start behavior | `src/tools/symbol/tests.rs` — 2 new tests (`symbols_overview_falls_back_to_treesitter_when_lsp_returns_empty`, `symbols_overview_returns_empty_for_empty_file_via_treesitter`) |
| I-04 | Fix `symbols()` silent empty result during LSP cold-start indexing | `src/tools/symbol/list_overview.rs` single-file branch — tree-sitter fallback when LSP returns empty Vec for a non-empty file. Logged as BUG-054 / #11 in `docs/issues/bug-tracker.md`. |
| I-14 | Delete duplicate `## Retrieval Stack` section | `README.md` — single canonical copy retained |
| I-18 | State-protocol document | `docs/state-protocol.md` — enumerates every shared filesystem path, writer, readers, schema, and compatibility expectations across codescout / codescout-companion / buddy |
| I-12 | Replace three duplicated URI↔path helpers with a `FileAddress` newtype | `src/util/file_address.rs` (new), `src/lsp/client.rs`, `src/fs/mod.rs`, `src/tools/symbol/call_edges/resolver.rs` (now delegate). 4 new roundtrip tests. |
| I-09 | Document dev-mcp loop | `CONTRIBUTING.md` — section explains pointing MCP config at `target/debug/codescout` for ~3s incremental rebuilds |
| I-03 | Cap and rotate `narrative.jsonl` (hard cap, judge-independent) | `claude-plugins/buddy/scripts/narrative.py` — `MAX_ENTRIES_HARD_CAP=200`, `MAX_BYTES_HARD_CAP=1MB`, `enforce_narrative_cap()` called from `append_entry`. 4 new tests. |
| I-15 | Pre-commit config | `.pre-commit-config.yaml` — fmt + clippy + test (test on pre-push only) |

All Rust changes pass `cargo fmt && cargo clippy -- -D warnings && cargo test --lib`
(1911 tests, 0 failures). All buddy changes pass `python3 -m pytest tests/` (298 tests).

## P0 — Prerequisite safety nets (must land before refactors)

These items are not improvements themselves — they are the rope on the wall.
Refactors that follow assert behavior preservation against these tests.

| ID | Scope | Surface | What it locks down | Fix | Unblocks |
|---|---|---|---|---|---|
| Y-A | codescout | `src/tools/symbol/tests.rs` | Current `symbols("src/tools/mod.rs")` returns 0 module declarations in compact mode. Test asserts this baseline; will assert `>0` after I-04 ships. | Add `compact_mode_returns_module_declarations` test calling `Symbols.call` with default `OutputGuard`. Today the assertion is "actual count > 0" and is expected to FAIL — gives I-04 a green target. | I-04 |
| Y-B | claude-plugins | `buddy/tests/test_hooks_*.sh`, `companion/tests/` | Behavior of every hook script under a known input event JSON. Output: state.json shape, stdout text. | Extend the existing `test_hooks_session_start.sh` pattern to every hook in buddy/ and companion/. Each test pipes a fixture event into the hook and snapshots state file diff + stdout. | I-05, I-06, I-07, I-11, I-13 |
| Y-C | codescout | `tests/prompt_surfaces_roundtrip.rs` | The three current prompt surfaces (`server_instructions.md`, `onboarding_prompt.md`, `builders.rs::build_system_prompt_draft`) reproduce byte-for-byte from a notional source-of-truth document with surface tags. | Write the test before I-01. It seeds with a hand-crafted source.md that yields the existing surfaces. Test must pass against current files. After I-01 lands, the test passes against generated files — proving zero content drift through the refactor. | I-01, I-02 |

## P0 — Strategic / permanent payoff

| ID | Scope | Surface | Problem | Fix | Why P0 |
|---|---|---|---|---|---|
| I-01 | codescout | `src/prompts/server_instructions.md` + `onboarding_prompt.md` + `builders.rs::build_system_prompt_draft()` | Three parallel prompt surfaces drift; the `prompt_surfaces_reference_only_real_tools` test catches stale tool names but uses an allowlist that decays. CLAUDE.md names "distance from change" as the failure mode. | **Phase 1**: introduce `src/prompts/source.md` with surface tags. Build script reads it and emits the three surfaces to a side directory. Diff against existing files until byte-identical (Y-C asserts this). **Phase 2**: switch `from_parts` and `build_system_prompt_draft` to read generated files. **Phase 3**: delete originals + tripwire allowlist. Each phase a green commit. | Eliminates an entire class of drift bugs. Saves coordination cost on every tool rename. Token cost can shrink because shared sections deduplicate. |
| I-02 | codescout | `src/prompts/server_instructions.md` (~23 sections, injected on every MCP request) | Iron Laws + Anti-Patterns + Decision trees + Workflows + MCP Resources docs all bundled into the always-on injection. Token cost permanent; mostly the model needs ~10% on any given turn. | **Follow-on to I-01, not parallel.** Once `source.md` exists, tag sections as `lazy:true`. Build emits only `lazy:false` content into the always-injected surface; the rest flows to `mcp_resources/`. Trim hot path to ≤1500 tokens. | Permanent context-window savings. Compounds across years. **Sequence after I-01 ships — Yak Reaction 2: do not bundle refactor and feature.** |
| I-03 | claude-plugins/buddy | `buddy/hooks/post-tool-use.sh::accumulate_narrative` → `narrative.jsonl` | Append-on-every-PostToolUse, no rotation. Long sessions = unbounded growth. Judge prompt ingests the whole file → quadratic cost in judge calls. | Cap by sliding window of last N entries, age (drop entries >1h old), and max-bytes ceiling with hash-based dedup of repeated tool calls. Update `judge.py::build_judge_prompt` to read the capped tail only. | Judge becomes safe to enable by default. Removes a footgun that bites long-session users. |
| I-04 | codescout | `src/tools/symbol/symbols` (compact mode) | `symbols(path)` in default `Exploring` mode returns empty for files whose top-level is mostly `pub mod` declarations (`tools/mod.rs`, `agent/mod.rs`, `output.rs` empirically). Only `detail_level="full"` surfaces them. | **Step 1**: log to `docs/issues/bug-tracker.md` (per CLAUDE.md mandate). **Step 2**: with Y-A green, fix the compact-mode filter to include module declarations or emit an explicit "N module decls hidden — pass detail_level=full" hint. | Symbols is the most-used codescout tool. Silent inconsistency erodes trust in every other tool by association. |

## P1 — Architecture / DX scaling

| ID | Scope | Surface | Problem | Fix | Why P1 |
|---|---|---|---|---|---|
| I-05 | claude-plugins (buddy + companion) | `buddy/hooks/*.sh`, `companion/hooks/session-start.sh` | Hook scripts mix bash with `python3 -c "<heredoc>"` blocks. Hard to lint, untestable, fail silently per buddy iron rule. | **Phased per hook (11 files > 8).** Pick largest heredoc first (`buddy/post-tool-use.sh`). Extract to `scripts/hook_<event>.py`. Bash wrapper becomes 3 lines. Run Y-B tests. Commit. Move to next hook. One hoof at a time. | Hooks become testable. Stack traces become readable. Foundation for richer hook logic. |
| I-06 | claude-plugins/companion | `hooks/detect-tools.sh` | Parses jq across 5–7 config files on every session start. Sourced by other hooks → multiplied cost. | **Sequence after I-11**: do detect.py replacement first, then add caching to it. Cache file `~/.cache/claude-plugins/codescout-detect.json` keyed by `(cwd, mtime of configs)`. | Halves session-start latency. Removes brittle `source` pattern. |
| I-07 | claude-plugins/companion | `hooks/pre-tool-guard.sh::enforce` | Aggressive `WRONG TOOL. STOP.` denial messages on Bash/Grep/Glob. Effective short-term but trains context toward defensive shape. | Tier the response: (a) first violation per session → full denial + redirect, (b) subsequent → one-line deny reason, (c) post-tool warning instead of pre-tool block when ambiguous. Keep hard block for egregious cases. | Less context pollution. Less adversarial UX. Models that learned the rule stop getting shouted at. |
| I-09 | codescout | `cargo build --release` required to test live MCP changes | Dev iteration requires release build (slow) + `/mcp` restart. CLAUDE.md documents this as friction. | Add a `dev-mcp` mode that runs `cargo run --bin codescout` directly. Document the trade-off. Optional: settings.json snippet that points at `target/debug/codescout` when env var set. | Cuts inner loop from ~30s to ~3s. Compounds across project lifetime. |
| I-18 | cross-plugin (codescout + companion + buddy) | `.codescout/`, `.buddy/`, `~/.claude/buddy/`, `cc_session_id` shared between three processes that do not share a compiler | Each plugin reads and writes the others' state files directly. The companion's `session-start.sh` hard-codes the structure of `embeddings.db.meta` and `drift_report`. Buddy's hooks hard-code `.buddy/<sid>/state.json` schema. **No contract, only convention.** Strongest form of coupling: implicit, untyped, runtime-only, distributed. | `docs/state-protocol.md` enumerating every shared path: writer, readers, schema, compatibility expectations. Pair with `state_contract` integration test in each plugin that round-trips read+write across version boundaries. | **Snow Lion's missing wall.** Without it, I-10 (kill backwards-compat fossils) is dangerous — schema changes break silently across plugin boundaries. Documented contract is what makes deletion safe. |
| I-19 | codescout | `src/prompts/source.md` (`server_instructions` surface) + `src/prompts/builders.rs::language_navigation_hints` + `src/prompts/mod.rs` | Server instructions still embed a static 6-language navigation block, plus a parallel `language_navigation_hints` emitter in `builders.rs` for the per-project draft prompt. Two impls drift; static block can't react to workspace dominant language. Iron Law 8 + Impact Analysis sections also accumulated 5+ scattered `call_graph` one-liners. | **Plan exists** at `docs/superpowers/plans/2026-05-07-server-instructions-consolidation.md` — needs file-path retarget from `server_instructions.md` → `source.md` `<!-- @surface server_instructions -->` block (post-I-01 reality). 11 tasks: (1) `language_nav.rs` skeleton, (2) all six languages, (3) workspace ranker, (4) renderer with lead-in + generic block, (5) `{{symbol_navigation_block}}` token + substitution in `mod.rs`, (6) delete dead emitter from `builders.rs`, (7) Iron Law 8 rewrite (promote `call_graph`), (8) Impact Analysis canonical demo, (9) prune scattered one-liners, (10) cross-prompt consistency test update, (11) final verification. | Eliminates dual-source drift in language nav. Workspace-aware language ordering. Cleaner Iron Law surface. **Pre-req: I-01 shipped** (✓). **Sequence: paths-stale plan must be retargeted before execution.** |
| I-20 | codescout | `src/prompts/source.md` (`onboarding_prompt` surface) + `src/prompts/workspace_onboarding_prompt.md` + `src/prompts/mod.rs` + `src/tools/onboarding.rs` | Live audit of mirela-backend-kotlin onboarding revealed 7 specific bugs: (1) root subagent forgot `project_id` → memories landed at workspace scope and got partially overwritten by synthesis; (2) root content (`dev.sh`, `docker-compose.yml`, generic navigation) not captured anywhere; (3) CLAUDE.md gaps detected at end of run but never applied — assistant ended on an unanswered question; (4) per-project subagent coverage inconsistent (ktor 6 memories, python-services 3, eduplanner-mcp 2); (5) HARD-GATE only verified `project-overview` per project — eduplanner-mcp passed with two memories total; (6) stale `mcp-server` / `mcp-server-deprecated` workspace projects retained from a rename never proposed for cleanup; (7) workspace synthesis tried to read `## Phase 0: Embedding Model Selection` from the single-project prompt — fragile cross-prompt heading reference. | **Spec + plan exist** at `docs/superpowers/specs/2026-05-07-onboarding-refactor-design.md` (302 lines) and `docs/superpowers/plans/2026-05-07-onboarding-refactor.md` (1230 lines, 6 tasks). Both need file-path retarget from `onboarding_prompt.md` → `source.md` `<!-- @surface onboarding_prompt -->` block. Architecture: new `src/prompts/memory-templates.md` as single source of truth for the 7 memory definitions × 2 scopes; `{{include: memory-templates.md}}` markers substituted at compile time by `load_prompt()` (`include_str!` + `String::replace`). Workspace flow restructured into 6 numbered phases with read-back verification (Phase 4) and CLAUDE.md auto-refresh (Phase 6). 6-mandatory-memory rule per project with canonical empty-stub for `domain-glossary`/`gotchas` when nothing project-specific exists. Bumps `ONBOARDING_VERSION` per CLAUDE.md § Prompt Surface Consistency. | Fixes audit-confirmed bugs that produce silent partial onboarding. Predictable per-project memory set for downstream tools. **Pre-req: I-01 shipped** (✓). **Sequence: paths-stale plan must be retargeted before execution; conflicts with I-19 — both touch `mod.rs` and `source.md`'s onboarding/server-instructions surfaces, so land them serially not in parallel.** |

## P2 — Debt / cleanup

| ID | Scope | Surface | Problem | Fix | Why P2 |
|---|---|---|---|---|---|
| I-10 | codescout + companion | `.codescout` / `.code-explorer` directory names; `codescout-companion.json` / `codescout-routing.json` / `code-explorer-routing.json` config names | Project rename left fossils. `detect-tools.sh` carries 3-fallback paths. Server.rs/tools also probe both. | **Sequence after I-18.** Pick a freeze date. After date X: codescout-companion v2.0 hard-removes fallbacks, emits one-time rename warning then exits. New users only see canonical name. | Less surface area. Forcing migration is cheaper than carrying fallbacks forever. **Cannot ship safely until I-18 catalogs every reader/writer of the old paths.** |
| I-11 | claude-plugins/companion | `hooks/detect-tools.sh` | Source-via-`.` pattern silently breaks if any export is renamed. No compile-time check. | Replace with `detect.py` emitting JSON to stdout. Hooks consume via `eval "$(python3 -m scripts.detect)"`. Detect logic gets pytest tests. **I-06 caching layered on top of this.** | Detection becomes a tested unit with explicit contract. Pairs with I-05/I-06. |
| I-12 | codescout | `src/lsp/client.rs::uri_to_path` / `path_to_uri` | LSP-flavored helpers but URIs appear elsewhere (dashboard routes, librarian artifact links). Currently every consumer rewrites the conversion. | **Reframed by Snow Lion**: not a `util/uri.rs`. Extract a `FileAddress` newtype that carries the canonical form and exposes `as_uri()` / `as_path()`. The type *is* the centralization. Move test cases with it. | Tiny. Earns a domain concept rather than a utils-bag entry (Heuristic 6: utils packages signal a missing domain concept). |
| I-13 | claude-plugins/buddy | `buddy/hooks/judge.env` sourced by all hook subprocesses | Brittle (typos silently fail; secrets leak in `ps`-visible env). | Move config to `buddy/data/judge.json` validated against a schema. Hooks read it via `python3 -m scripts.config get judge.model`. Keep `judge.env` for one release with deprecation warning. | Hardens judge surface. JSON gives schema validation; `.env` gives nothing. **Sequence after I-05 — easier to refactor config in Python than bash.** |
| I-14 | docs | `README.md` (codescout root) | The "Retrieval Stack" section appears twice verbatim (lines ~50-70 and ~75-95). Editor mistake during a Phase 6 doc edit. | Delete the second copy. Verify docs build. | Trivial. Tracker entry exists so it doesn't get forgotten. |

## P3 — Polish

| ID | Scope | Surface | Problem | Fix | Why P3 |
|---|---|---|---|---|---|
| I-15 | codescout | Pre-commit gate (`cargo fmt && clippy -D warnings && test`) | CLAUDE.md states verbally. No `pre-commit` config installs it locally. | Add `.pre-commit-config.yaml`. Document `pre-commit install` in CONTRIBUTING.md. CI already runs the gate; this catches issues earlier locally. | Catches issues before push. Reduces "fix clippy" churn commits. |
| I-16 | claude-plugins/buddy | `skills/<specialist>/SKILL.md` "Voice" section | Voice prose is craft-level but unverified. Pure tokens otherwise. | A/B for one specialist (Hamsa best subject — has eval framing): 5 prompt-critique tasks scored by separate judge, with vs. without Voice. Decide per-specialist whether to keep, trim, or move to lore loaded only via `/buddy:legend`. | Either confirms voice-as-behavior-shaping or trims it. Either outcome wins. |
| I-17 | claude-plugins/buddy | Memory consolidation auto-trigger (`.claude/buddy.json::consolidation`) | Opt-in via JSON config. Most users won't find it. Memory grows unbounded across sessions. | Default `auto_dry_run_on_session_start: true` for new installs. Bump default thresholds (60d / 50 entries / 24h debounce). Add `--opt-out` line to `/buddy:summon` first-run output. | Memory hygiene by default. Dry-run gate still requires explicit user action — no risk of silent destructive changes. |

## Parked — abstractions awaiting duplication

| ID | Scope | Surface | Why parked | Re-open trigger |
|---|---|---|---|---|
| I-08 | codescout | `src/server.rs::CodeScoutServer::from_parts` — flat `Vec<Arc<dyn Tool>>` with one `cfg(feature = "librarian")` block | Yak Heuristic 1: name the structural defect. Today: one conditional in one vec. Heuristic 6: rule of three — wait until duplication forces the abstraction. Building ToolRegistry now is a guess at a shape duplication has not yet revealed. | A second `cfg`-gated tool family lands in `from_parts`. At that point the duplication will tell you the registry's actual shape. |

## Execution ordering (Yak's revised first-five)

1. **Y-A + Y-C** — write the safety nets. Half a day each. Cheap.
2. **I-04** — with Y-A green, fix the symbols compact-mode bug. Same PR.
3. **I-14** — delete the duplicate README section. Five minutes.
4. **I-18** — write the state-protocol document. Pure prose; no code change. Unblocks I-10.
5. **I-01 Phase 1** — introduce `prompts/source.md`. Diff-test against existing files (Y-C). Commit when byte-identical.

Then the longer arcs:

6. **I-01 Phase 2 + Phase 3** — switch readers, then delete originals.
7. **I-02** — tier server_instructions on top of I-01.
8. **I-19** before **I-02** — server-instructions consolidation (language_nav module + content rewrites). Cleaning the surface before lazy-tiering avoids two passes at the same content. **Plan needs path retarget from `server_instructions.md` → `source.md` surface block before execution.**
9. **I-20** independent of I-19, can land either order — onboarding refactor (memory-templates extraction + 6-phase workspace flow + ONBOARDING_VERSION bump). **Plan needs path retarget from `onboarding_prompt.md` → `source.md` surface block before execution.** Both touch `src/prompts/mod.rs` so do not run in parallel.
10. **Y-B** before **I-05** — characterization tests for hooks, then bash→python migration phased per hook.
11. **I-11** before **I-06** — detect.py replacement, then caching layered on top.
12. **I-13** after **I-05** — judge.json refactor easier in Python than bash.
13. **I-07** after **I-05** — softer PreToolUse easier to iterate in Python.
14. **I-18** must precede **I-10** — contract before deletion.
15. **I-03**, **I-09**, **I-12** — independent; pick up between larger items.
16. **I-15, I-16, I-17** — polish; schedule when an arc lands.

## Decision log

- **2026-05-07** — Tracker opened from architecture review. P0 list intentionally
  short so it stays actionable.
- **2026-05-07** — Reviewed by Architecture Snow Lion: added I-18 (state-contract
  doc) as the missing wall between the three components; reframed I-12 from
  `util/uri.rs` to a `FileAddress` newtype (Heuristic 6 — utils signal missing
  domain concept); flagged I-08 as premature abstraction.
- **2026-05-07** — Reviewed by Refactoring Yak: added Y-A/Y-B/Y-C prerequisite
  safety nets; phased I-01 (>8 files) and I-05 (11 hook files); separated I-02
  from I-01 (refactor before feature); parked I-08 until rule-of-three triggers.

## Cross-references

- Architecture review: in-context only. Key signals: `src/tools/output.rs`
  (OutputGuard), `src/server.rs::from_parts` (tool registration),
  `src/prompts/server_instructions.md` (Iron Laws),
  `claude-plugins/buddy/scripts/judge.py`, `claude-plugins/codescout-companion/hooks/`.
- Related living trackers:
  - `docs/trackers/skill-frictions.md`
  - `docs/trackers/tool-usage-patterns.md`
  - `docs/issues/bug-tracker.md`
- Append findings here when surfaced during sessions, same discipline as the
  other trackers in this directory.
