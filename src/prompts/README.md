# Prompt surfaces — editing guide

Read this when touching `source.md` (the single source for the `server_instructions` and `onboarding_prompt` surfaces) or `builders.rs`. This file is the **canonical home** for prompt-surface rules: which surfaces exist (§ Surfaces), when to bump `ONBOARDING_VERSION` (§ Versioning), the writing style guide (§ Rules), and the shared-branch slice hazard (§ Verify the slice). `CLAUDE.md` carries only a one-line pointer here; memory `conventions` § Prompt Surface Consistency has the short version.

**Any change to tool behavior or signatures requires a prompt-surface review** — adding/renaming tools, changing parameter semantics, new error/fallback modes, or changed response shapes. Ask: "Does the LLM need to know this to use the tool correctly?" If yes, update all surfaces in the same commit. The build-time test `server::tests::prompt_surfaces_reference_only_real_tools` catches stale tool-name mentions across the three surfaces; `prompts::tests::claude_md_contains_no_deprecated_tool_names` guards `CLAUDE.md`. ("Distance from change": files closer to a rename get updated, distant ones accumulate stale refs — the tests are the backstop.)

## Surfaces

- `src/prompts/source.md` — the **single editable document** for the next two surfaces. `build.rs` slices it into `OUT_DIR` at compile time; `src/prompts/source.rs::extract_surface` is the matching runtime parser. Edit here.
  - `server_instructions` surface — injected **once at MCP session start**, not per-request. Token cost is session-scoped, not per-call — invest in clarity over brevity.
  - `onboarding_prompt` surface — one-time onboarding, read only when a project is activated for the first time.
- `build_system_prompt_draft()` in `src/prompts/builders.rs` — generated per-project and embedded into the project's system prompt via onboarding.
- **`tools/list`** — every tool's `description()` + `input_schema()`, delivered **on every request of every session**. The largest surface by an order of magnitude and the only one with a per-request cost: **55,519 characters** as of 2026-09-03. Budgeted — see § The tool-surface budget. **Derive these numbers, don't cite them:** `cargo test --lib tool_surface_report_lengths -- --nocapture` prints the per-tool map, and `python3 scripts/probe_tool_surface.py` prints both cuts below plus the per-parameter table.

  **There are two cuts of that total. They answer different questions, and the second is the one that tells you what to trim.**

  | cut | figures | answers |
  |---|---|---|
  | by **field** | schema 48,522 (87.4%) / desc 6,997 (12.6%) | where the bytes sit on the wire |
  | by **authorship** | **prose 36,715 (66.1%)** / machine 18,804 (33.9%) | how much of it a human wrote |

  **The field cut hides the largest bucket in the surface.** `desc` is *tool* descriptions only. The parameter descriptions — 289 strings once nested object fields are counted, **29,718 chars, 53.5% of everything** — are serialized inside `input_schema` and land in the "schema" column. So the surface is two-thirds prose, and the schema/desc ratio is a fact about JSON nesting rather than about where the writing is.

  *(This bullet said the opposite until 2026-09-03: "**schema is 86% of it**, and an instinct to tighten descriptions is aimed at the wrong 14%." Both its figures were correct and its advice was backwards, because "descriptions" silently means two different populations one clause apart — the `desc` field in one, every human-written string in the other. Nothing could have caught it: the numbers reconciled, and the only tell was that the surface's own instrument had no way to express the second population. That is why `probe_tool_surface.py` now emits the authorship cut — so the correction is **derivable**, not remembered.)*

  Numeric drift here is expected and is not a defect to chase: these figures read `54,976 / 47,549 / 7,427 as of 2026-09-01`, and `57,148 / 48,627 as of 2026-08-18` before that, against a surface ratcheted down *and* grown again — the two-representations-one-truth seam this file's own § budget warns about, occurring in the file that warns about it.
## Rules for editing the `server_instructions` surface

1. **Cap hard rules at 5–8.** Beyond 8 behavioral constraints, compliance on all drops. Consolidate, don't accumulate.
2. **No triple-layer repetition.** A rule in Iron Laws should NOT be restated in Anti-Patterns AND Rules. Max 2 appearances: once as a law, optionally once as a closing reminder (for the 1–2 most-violated rules only).
3. **Tables > prose** for decision-matrix content. Claude scans tables faster.
4. **End of prompt = highest compliance.** Put the most-violated rule(s) in the closing `## Rules` section — that's closest to generation.
5. **Don't document every param.** Pagination (`offset`, `limit`, `detail_level`) and aliases (`file_path`, `limit`) are discoverable from the tool schema. Only document params that change behavior in non-obvious ways.
6. **Prompt caching matters.** Keep section order stable between releases so the static prefix benefits from automatic caching. Don't reorganize for cosmetic reasons.
7. **You are the consumer.** When writing or reviewing prompt changes, think as the agent who will read this mid-task. Ask: "Would this have helped me find the right tool chain naturally?" Test by simulating a realistic task and checking whether the prompt guided you to the right flow. Usage data (`usage.db`) is the ground truth — if a tool has near-zero calls despite being useful, the prompt isn't surfacing it.
8. **1900-CHARACTER hard cap on the static slice.** The `server_instructions` slice is delivered as the MCP `initialize.instructions` field, which Claude Code silently truncates at **2048 characters** — measured 2026-08-16 by locating a live session's own cut point inside the rendered slice (byte 2092 / char 2048, mid-token). The cap is enforced by `prompts::redesign_invariants::source_md_under_cap` with `STATIC_SLICE_CHAR_BUDGET = 1900`; the remainder funds the dynamic `## Project Status` block, and `build_server_instructions` now *guarantees* the total fits by trimming that block at a line boundary with an explicit note rather than letting the client cut the Iron Laws.

   **Two things about the old rule were wrong, and both stayed green for months.** The cap was `2200` — *above* the 2048 cliff it existed to protect. And it was compared against `String::len()`, which counts **bytes**: this surface is dense with em-dashes and arrows, so the same slice measured 2127 bytes and 2081 chars. Count characters, not bytes, and never raise the number. (BL-9, `docs/issues/archive/2026-08-15-server-instructions-truncated-before-reaching-the-model.md`.) When the test fails, do NOT raise the cap — author a `get_guide(topic)` entry, reference it from the slice, **and give the topic a trigger** (a tool's `relevant_guide_topic()`) or record it in `prompts::PULL_ONLY_GUIDE_TOPICS` with the reason it is pull-only. This instruction used to stop at "author the entry", and that one missing step is why 7 of 10 topics — 47,343 of 75,441 bytes, 63% of the guide corpus — fired for nothing: a guide nothing triggers is not filed, it is removed from the agent's view. `server::tests::every_guide_topic_is_triggered_or_declared_pull_only` now fails the build rather than letting the omission pass, so the choice is forced but not made for you. Note the budget tension before reaching for a trigger: `librarian` alone is 19.9 KB and already auto-injects on a routine `artifact` call. (Don't put a literal "EDITOR NOTE" HTML comment containing the surface/end marker strings into `source.md` itself; the extractor at `src/prompts/source.rs::extract_surface` does a substring `find` and will match the comment first, breaking the slice — F-5 in `docs/trackers/archive/prompt-guide-refactor-session-log.md`.)

## The tool-surface budget

`tools/list` is the fourth prompt surface and the only one with a **per-request** cost. Measured 2026-08-18 across four Claude Code sessions and three models, **100.0% of input reads are cache hits**, so this block is re-read at cache-read rates for the life of a session — about 5% of a long session's cached prefix, 10% of a short one's.

**Budget: `TOOL_SURFACE_CHAR_BUDGET` in `src/server.rs`**, enforced by `server::tests::tool_surface_under_budget`, with `tool_surface_report_lengths` as the per-tool map (`cargo test --lib tool_surface_report_lengths -- --nocapture`). Same instruction as rule 8 above: **do not raise it — find the bytes.** Ratchet it *down* whenever a trim frees room. It has already been paid down once: declaring `anchor_heading` cost +808 and was funded by compressing the injected `workspace` description.

Two things about it are load-bearing.

- **The budget is on the payload, not the item.** Descriptions were already capped per tool — 300 chars by default, 1800 for `artifact` and `librarian` via `Tool::description_cap` — and the surface still reached ~59K characters, because **a per-item cap does not bound a sum**: growth moved sideways into the then-unmeasured `input_schema()`, and N items may each sit at their own limit. Do not answer a breach by adding a per-tool schema cap; that is the mechanism that produced the breach.
- **Measure what `list_tools` builds.** `advertised_surface()` reproduces the `availability(&caps)` filter and the `inject_workspace_param()` injection, both of which happen *after* `input_schema()` returns — the injection alone is **3,818 characters across 23 pinnable tools** (2026-09-01: 23 × one identical 166-char `json!()` literal, `src/server.rs`; `pinnable()` is a name-based exclusion list, never overridden per tool). Summing raw `input_schema()` would measure a string nobody receives, which is the defect `production_render_fits_the_client_channel` exists to prevent on the sibling surface. Keep the helper in step with `list_tools` or the gate is decorative. *(This read `6,528 characters across 24 pinnable tools` until 2026-09-01 — wrong on both numbers: the injected description had since been compressed, and a tool was deleted. A per-tool count written into prose is a snapshot, and the tool count moves whenever the registry or `pinnable()`'s exclusion list does.)*

**Moving prose from a schema into a `get_guide` topic is not a saving by default.** Both land in the same cached prefix. A guide fired at turn K costs `X × (N−K) × cache_read + X × cache_write`, against the schema's `X × N × cache_read` — break-even at **K ≈ 12.5 turns**, and `librarian` auto-injects on the first `artifact` call. It wins only for sessions that never trigger the guide at all.

Full derivation, the rejected alternatives, and the open routing experiment: `docs/superpowers/specs/2026-08-18-tool-surface-budget-design.md`.
## Versioning — when to bump ONBOARDING_VERSION

Bump `ONBOARDING_VERSION` in `src/tools/onboarding.rs` when changing a surface that produces the **stored per-project system prompt** — the `onboarding_prompt` slice of `source.md`, or `build_system_prompt_draft()` in `builders.rs`. The bump triggers automatic system-prompt regeneration for all projects onboarded with the previous version.

**Do NOT bump for `server_instructions` changes** — that surface is injected fresh at every MCP session start, with no separate cache of its own to invalidate. But "live on next connect" needs a qualifier a `/mcp` reconnect does not satisfy: see the note below.

**`/mcp` reconnect does NOT refresh `server_instructions` in the same conversation.** MCP delivers `instructions` once, in the `initialize` response; the host composes it into the system prompt at conversation start, and a mid-conversation `/mcp` reconnect re-runs `initialize` against the new binary for tool schemas and behavior only — the system prompt already built for this conversation is not rebuilt from it. Measured 2026-08-17 across three reconnects and two rebuilds: tool schemas refreshed each time (`librarian` gained `archetype`, `edit_markdown`'s `new_string` guard went live), while the `instructions` text stayed byte-identical to what the conversation's *first* connection served — in one case, a whole section the source had deleted the previous day. See `docs/issues/archive/2026-08-17-mcp-reconnect-does-not-refresh-server-instructions.md`.

Practical consequence: **never claim a `server_instructions`/`onboarding_prompt` edit is live from inside the session that authored it — that session is structurally the one observer that cannot see it.** Two ways to verify instead:

- Tool code and schemas: `cargo rb` + `/mcp` in the same session, then re-probe.
- Prompt surfaces: check the fixture (`grep -c '<the new text>' tests/fixtures/prompt_surfaces/server_instructions.md` + `cargo test --lib prompt_surfaces`) — the authoritative check, and the one CI runs. To eyeball the live-injected text itself, start a genuinely **new** conversation against the rebuilt binary.

| Surface | How delivered | Bump needed? |
|---|---|---|
| `server_instructions` slice of `source.md` | Loaded fresh at every MCP session start | **No** — live for the next NEW conversation's first connect (not a same-conversation `/mcp` reconnect — see the note above) |
| `onboarding_prompt` slice of `source.md` | Drives stored system-prompt generation | **Yes** — cached per project |
| `build_system_prompt_draft()` in `builders.rs` | Same — generates stored system prompt | **Yes** — cached per project |

**Bump when:** tool names change (rename/consolidate); parameter semantics change in the `onboarding_prompt` surface or `builders.rs`; onboarding templates change in ways affecting the generated system prompt.

**Do NOT bump for:** any `server_instructions` change (however significant); bug fixes that don't change tool behavior; internal refactors; memory-template changes (memories are re-read during refresh anyway).

## Verify the slice before committing (shared-branch hazard)

The `server_instructions` slice is under a hard **1900-character cap** (rule 8, enforced by `prompts::redesign_invariants::source_md_under_cap`), sitting below the measured 2048-char client limit. Two ways it bites:

- **Run `cargo test --lib prompt` before any prompt-surface edit is ready to commit.** If `source_md_under_cap` fails, do NOT raise the cap or bless the snapshot to match — move content to a `get_guide(topic)` and leave a pointer in the slice — then finish the move by wiring the topic's trigger or declaring it pull-only, per rule 8. Moving content into an untriggered guide deletes it from the agent's view while looking like filing.
- **On a shared branch, re-measure the slice on *current* HEAD.** A concurrent commit can grow the slice under you. `git log --oneline -1` first, then re-check the byte count before trusting any earlier measurement or running `UPDATE_PROMPT_SNAPSHOTS=1` — otherwise you bless the over-cap state into the fixture and ship a truncated slice. Datapoints: F-4 (2026-05-28) and F-8/W-5 (2026-05-31) in `docs/trackers/archive/prompt-guide-refactor-session-log.md`.

## Measure before shipping — the subtract-and-measure protocol

Editing style/caps is covered above; whether a prompt-surface **change ships at all** is governed by the subtract-and-measure protocol (P-1..P-8) in `docs/trackers/prompt-hamsa-audit-log.md` § Protocol — read it via `doc(action="get", id="59ebeebb6ed05c89", heading="Protocol — subtract-and-measure (P-1..P-8)")`. Short form: name a locally-observed failure first; pre-register the audit row with a numeric ship/no-ship rule; run the **base arm first** (no change) — at ceiling, don't ship; deletions instead prove the cut regresses nothing; mechanical trace checks over judges; mutation-test the checker; pin the generator model. Harness: `prompt-tdd` in `../prompt-engineering/` (template: `scenarios/fable-tidying/`).
## Research

Evidence behind these rules:

- `docs/research/2026-03-21-claude-prompt-engineering.md`
- `docs/research/2026-03-21-superpowers-prompt-patterns.md`
