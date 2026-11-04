---
id: '95abaa31c77daa39'
kind: spec
status: draft
title: Harness-delivered hints — design
owners:
- marius
---

# Harness-delivered hints — design

Status: draft for approval. Written 2026-11-04 after Stages 1–4 of the hint channel eval (`docs/evals/harness-channel-hint-authority-preregistration.md`, id `cffe594cbf26fd72`, AM-1..AM-13). Every choice marked **My call** was made by the author and not agreed with the operator.

## Intent

The operator wants codescout's hints to reach the agent through the Claude Code harness when the companion plugin is present, with a statement that the user endorses them, because a hint on that channel may carry more authority than one inside a tool result. The eval measured whether that is true and what it costs. This design turns the measurements into a delivery mechanism that gives authority only to hints whose claim has been checked.

## What the measurements say

All Sonnet 5.5, frozen CLI 2.1.309 unless noted; the metric is a lookup of the named function before the first edit.

- A hook hint without a declaration is mostly ignored: late lookup 1/11 and 3/10 (Stages 2–3). The server's own `verify callers` hint, in the result, was followed in 1 of 943 cases within 8 calls (`hint-census/follow-results.txt`, observational).
- One declaration, "messages marked `[cs-hint]` are endorsed by the user", raises compliance to about 0.56–0.89, in `CLAUDE.md` (5/9) or in a tool result (6/8); the placements are not ranked.
- The same declaration makes the agent follow a WRONG hint (a lookup of an unrelated function): 12/25 and 16/25; floor 0/75. Endorsement removes the agent's ability to tell a right hint from a wrong one.
- The tag text alone in file content did not confer authority: 0/20 decoy lookups, 0/10 with the declaration in `CLAUDE.md` (Stage 4). Authority follows the channel.
- A PostToolUse hook can move a hint from a result to hook context on CLI 2.1.318: it strips `hint` with `updatedToolOutput` and adds `additionalContext`; with no hook, or a hook that exits 1, the hint stays in the result (a stub server, three Haiku runs).
- Census of the server's `hint` field (usage.db, 30 days, 5,207 deliveries): about 64% are protocol pointers the agent needs in the result (job-started 1,738, buffer pointers 1,145, approve-write 430). Advice hints are the minority: `verify callers` 937, run_command advice 202, unread sections in `edit_file` 84.
- Today `verify_callers_hint` is set on every successful `rename` and `replace` without checking that the symbol has callers, or that the edit changed anything callers depend on (`src/tools/symbol/edit_code.rs`, lines 275 and 330). On `replace` it overwrites the "edit NOT verified against dropped symbols" hint set at line 1512 (bug `053ca236198f1b9b`); on `rename` it names the old symbol (bug `f5e732b0efeebd9e`). Two hook hints in the `buddy` plugin name tools that no longer exist (bugs `9419db55285a2f81`, `67ee3305dce3224c`).

## Goals

1. When the companion is present, advice hints whose claim the server has verified reach the agent as hook context, tagged and endorsed by the user, and are removed from the result.
2. When the companion is absent, dead or on another harness, every hint stays in the result exactly as today.
3. No hint is endorsed unless the server checked, at emit time, that its claim is true.
4. The first verified class is `verify callers`: it is sent only when the edit changed the symbol's declaration (signature or visibility) and the symbol has callers outside its own file.

## Non-goals

- Moving protocol pointers (job-started, buffer, approve-write). They name handles the agent uses in the next call.
- Guide injection (`_guide_hint`, 2,127 deliveries). A different carrier and size.
- Fixing the `buddy` hints. Tracked in the two bugs.
- Other harnesses. They keep the in-result hint.

## Anti-goals (a failure even if it works)

- Endorsing a hint class that has no emit-time check.
- A plugin writing the endorsement into the user's `CLAUDE.md` or into a tool result without the user's explicit consent. An in-band claim of user endorsement has the shape of an injection, and Stage 2 showed any MCP server could make one.
- The server dropping or moving hints on the strength of the companion-presence signal (`hook_at`). That signal latches on after a hook goes quiet (`src/tools/rendezvous.rs`; archived issue 2026-08-19), so a dead hook would make hints vanish.

## Constraints

- The server must always emit the hint (single source of truth). The companion repo has a standing rule against a mirror of a server predicate: "do not restore an advisory mirror of a server-side predicate" (`docs/architecture/companion-plugin.md`, on the retired `il3-warn-hook`).
- The hook must preserve every content block of the result. The probe rebuilt only the first block.
- `updatedToolOutput` is verified on 2.1.309 and 2.1.318 only. A failing hook leaves the result unchanged; only `exit 1` was tested.
- Stage 3 measured a hint that arrives BEFORE the edit. `verify callers` arrives after the edit. Its effect is not measured.
- Every effect above is one model (Sonnet 5.5).
- Prompt-surface rules apply if server instructions mention hints (`src/prompts/README.md`). Python and node tests, and the full gate, apply to each slice.

## Approaches considered

- **A. The server emits classed, verified hints; a companion hook relocates them.** The server adds two fields next to `hint`; the hook moves a hint iff it is `advice` and `verified`. Fail-open by construction. One new hint class touches only the server. Cost: a contract between two repos, and the server runs a check at emit time.
- **B. The server detects the companion and relocates or drops hints itself.** Rejected: the presence signal latches (see anti-goals), and the server cannot move a hint into hook context at all; only a hook can.
- **C. The hook recomputes advice from the tool's input and output.** Rejected: it needs the LSP for the callers claim and it duplicates a server predicate, which is the pattern the companion doc forbids and measured to produce false positives (19 of 703 refusals for `il3`).
- **D. Only fix `verify callers` at the source and keep it in the result.** Valid and cheaper. It makes the hint correct but, by the observational rate (1/943), not more followed. It is slice 1 of A, not an alternative to it.

Chosen: A, with D as its first slice. **My call.**

## Decision record

**Decision:** The server reports advice in a separate result field, `advice`, with a `verified` flag set only when it checked the claim at emit time; a companion PostToolUse hook relocates verified advice to tagged hook context, removing it from the result, and leaves everything else alone.
**Context:** Endorsement raises compliance with right and wrong hints alike, the tag in file content has no force, 64% of hints are protocol pointers, and `hint` is a single string that two writers already collide on.
**Alternatives considered:** B (latching signal; the server cannot write hook context), C (mirror of a server predicate), D alone (no compliance gain), a class flag on `hint` (cannot carry a safety warning and an advice in one result), a hidden `verified_advice` field that only a hook can surface (a host without the hook would never show the advice, which breaks Goal 2).
**Consequences:**
- now easier: adding a verified class; testing the hint's claim in the server where the facts are; degrading to today's behaviour; a safety warning and an advice in one result.
- now harder: a two-repo contract; a callers check at emit time for `verify callers`; the hook must edit a JSON text block without disturbing the rest.
**Change scenarios absorbed:** a new hint class (server only); a change in how the harness renders hook context (hook only); the companion absent or another harness (the advice stays in the result); the user withdrawing endorsement (delete one line in `CLAUDE.md`; advice stays correct and degrades to advisory).
**Revisit-when:** the CLI changes `updatedToolOutput`; the declaration's effect fails to replicate with the new tag; hooks do not fire for subagent tool calls; a model other than Sonnet 5.5 behaves differently; a host is found that accepts `updatedToolOutput` and drops `additionalContext`.
**Confidence:** medium. The mechanism is measured; the product effect of a post-edit verified hint is not.

## The parts

**1. Server contract.** One optional field in the tool-result envelope: `advice`, an object `{"text": "...", "verified": true|false}`. It always stays in the result for the agent to read; a host without the companion shows it as it showed the old hint. `verified` is true only when the server checked the claim at emit time. `hint` keeps its meaning: a single string for protocol pointers and safety warnings, unchanged. A result carries at most one `advice` for now. A separate field beats a class flag on `hint` for two reasons: a `replace` result can need a safety warning and a callers advice at once (today the second overwrites the first), and the overflow envelope replaces `hint` with a buffer pointer. Where the server renders a compact text form that appends `hint` (`edit_code.rs`, near line 226), it must also append `advice.text`.

**2. First class: `verify callers`, on `replace`.** The claim is: this edit changed the declaration of `X` (signature or visibility), and a call to `X` by name appears in another file of the same language. The second half is a syntactic match, not a resolved reference, and the advice text says so. Two checks at emit time, both before the edit is written. (i) The declaration changed: compare the declaration header of the text being replaced with that of the replacement text (the header runs from the first line that is not an attribute, decorator or comment to the first line holding `{`, ending in `:` or ending in `;`, with whitespace collapsed); tree-sitter symbols carry no signature, so the AST is not used for this. If no header can be read from either text, the answer is "cannot tell". (ii) A call by name outside the file: a whole-word search for `X` across files of the same extension, then a tree-sitter walk of each candidate file for call nodes whose callee is `X`. The language server's `references` is not used: in this checkout's `usage.db` its successful calls took a median of 2,181 ms and a 90th percentile of 16,607 ms (163 calls), against 103 ms and 721 ms (95th percentile) for `edit_code` and 13 ms and 465 ms for `grep`. The whole check has a budget of 500 ms. Three outcomes. Definite yes: `advice` with `verified: true`. Definite no (the search finished within the budget, covered every candidate file and found no call, or the declaration did not change): no `advice`. Cannot tell (the budget ran out, there are more candidate files than the check inspects, a language without call classification, no readable header): `advice` with `verified: false`, which the hook leaves in the result. A body-only edit sends no advice.

**`rename`.** Not relocated. The LSP rename has already rewritten the call sites. Its text stays in `hint`, unverified, and is corrected to name the new symbol (bug `f5e732b0efeebd9e`).

**3. Companion hook.** A PostToolUse hook on `mcp__codescout__.*`. It acts only when all hold: (a) the result carries `advice` with `verified: true`; (b) the host is Claude Code. A 2026-11-04 probe on 2.1.318 showed the hook input carrying `permission_mode`, `prompt_id` and `effort`, and the environment carrying `CLAUDECODE` and `CLAUDE_CODE_ENTRYPOINT`; the environment names may have been inherited from the outer session, so a clean-environment run is still needed. No version variable was seen. The companion also runs under VS Code and Copilot, which ignore matchers. When it acts it returns `updatedToolOutput` with every content block kept and `advice` removed from the JSON text of the result, and `additionalContext` of the form `[cs-verified] <text>`. In every other case it prints nothing. Any failure prints nothing and exits 0, which leaves the advice in the result. Prior art: `tools/hint-channel-eval/hook.py` and the relocation probe in `docs/evals/data/2026-11-04-harness-channel-hints/relocation-probe/`. Relocation is verified on 2.1.309 and 2.1.318 only.

**4. A new tag, `[cs-verified]`. My call.** The eval endorsed `[cs-hint]`. Two plugins already send `[cs-hint]` text without a check (`buddy`, the companion's own advisory hooks). Endorsing that tag would endorse them. The new tag is sent only by part 3. The bytes of the declaration and the tag have not been measured; slice 4 does that.

**5. The declaration, with consent.** The user adds a short statement to their own `CLAUDE.md` saying messages marked `[cs-verified]` are endorsed by them. Codescout's onboarding and the plugin's docs may offer the exact text. Nothing writes it for the user.

## Rollout

1. **Server only.** `verify callers` sent as `advice` for a `replace`, `verified` only when the declaration changed and outside callers exist; the three-way outcome; the safety warning no longer overwritten; the `rename` hint names the new symbol; Rust tests. No channel change, so no new risk.
2. **Hook.** Relocation of verified advice; fail-open; node tests with mutation probes, including a result with several content blocks.
3. **Consent text.** The snippet and where it is offered.
4. **Measure the product path.** The Stage 3 arms with the new tag and wording and with the real post-edit hint, on the same tasks, to see whether the effect replicates and whether a wrong verified hint exists to test against. Functional outcomes belong to the separate effort-comparison plan (`docs/superpowers/plans/2026-11-04-hint-authority-effort-comparison.md`); do not duplicate it.
5. **More classes**, each with its own check: a worktree read that misses the file, polling a running `@bg` job, `while … pgrep -f` wait loops (candidates in `hint-census/mining-report.md`).

## What is left to the builder

- Confirm the Claude Code signals in a clean environment (`env -i` plus the needed variables); check that PostToolUse hooks fire for subagent tool calls and that relocated context survives compaction.
- Test hook failure modes beyond `exit 1`: timeout, `exit 2`, malformed JSON.
- Decide how `advice` behaves when the result is buffered into a `@tool_*` handle, and the compact text rendering.
- Decide where the contract is documented and where the consent text is offered.

## Calls I made after the builder check

A fresh reader raised five questions. These are my answers; each can be overridden.

1. **What "verified" certifies.** Callers existing is not enough, because most `replace` calls change only a body, and then the callers are not affected. I made the claim two-part: the declaration changed AND outside callers exist. Body-only edits send no advice. The eval's tasks were all signature changes, so the product hint fires less often than it did there.
2. **`rename`.** Not relocated; its text stays in `hint`, unverified, and is fixed to name the new symbol.
3. **Contract shape.** A separate `advice` field with a `verified` flag, one per result, kept in the result for hosts without the hook; the hook strips it only when it relocates. Not a class flag on `hint`, and not a field only a hook can surface. A list can come later if two advices ever need to share a result. The collision on `hint` is a real bug, filed separately (`053ca236198f1b9b`).
4. **Other hosts.** The hook acts only where the Claude Code signals are present. Elsewhere, and whenever the hook fails or is unsure, the advice stays in the result. The hook has no version signal to check, so relocation rests on the two measured versions and on the revisit trigger.
5. **Real zero versus could-not-tell.** Only a completed check on a ready server may say "no callers"; everything else is "cannot tell" and yields `advice` with `verified: false`, following `docs/adrs/2026-08-27-negative-results-name-their-scope.md`.

**Changed while writing the server plan, from data (2026-11-04).**

6. **The callers check is syntactic, not `references`.** The latency numbers in Part 2 show a synchronous language-server call would multiply the time of a signature-changing `replace` by 20 at the median and by 160 at the 90th percentile. The claim and the advice text now say "called by name", which is true as stated, instead of "has callers", which only the language server can establish.
7. **The declaration check compares header text, not the AST.** Tree-sitter symbols carry no signature (`SymbolInfo.detail` is `None` for them).

## How we will know it works

- With the companion absent or disabled, a result carries `advice` in the result, and a `replace` whose dropped-symbol check was skipped still carries its safety warning in `hint` (a recorded-result test for each).
- With the companion present, a verified `verify callers` appears as `[cs-verified]` context and is removed from the result; all other blocks and fields of the result are unchanged.
- A symbol with no outside callers, and a body-only edit, get no advice.
- A cannot-tell outcome yields `advice` with `verified: false`, and the hook leaves it in the result.
- A mutation that makes the hook move an unverified or protocol hint fails a test.
- The slice-4 measurement is registered before its data, as AM-8, AM-10 and AM-12 were.
