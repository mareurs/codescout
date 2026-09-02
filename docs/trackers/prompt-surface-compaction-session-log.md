---
id: '03464a8808345846'
kind: tracker
status: active
title: Prompt Surface Compaction — Session Log
tags:
- prompt-surfaces
- session-log
- compaction
topic: prompt-surfaces
entry_high_water_F: 9
entry_high_water_W: 15
entry_prefix:
- F
- W
---

> **Work stream:** began as an audit of codescout's four prompt surfaces (`tools/list`,
> `server_instructions`, the `get_guide` corpus, onboarding) for correctness and byte cost.
> **It grew, 2026-08-19, into the record-legibility work** — the finding that this project
> does not have a truthfulness problem (authors write their caveats) but a **legibility**
> one: the caveats live in prose and no query reads them.
>
> **State at the 2026-08-19 compaction:**
>
> - **Layer 1 — shipped.** The `unverified:` frontmatter field, across `CLAUDE.md`,
>   `docs/TAXONOMY.md`, `docs/issues/_TEMPLATE.md` and the `tracker-conventions` guide.
>   Plus SHA + patch-id provenance on 54 bug files.
> - **Layer 2 — shipped, complete.** All three `doctor` checks proposed by CAP-7:
>   `declared_root_missing` (`f632e7ef`), `terminal_status_with_caveat` (`067ced2c`),
>   `archived_fix_sha_unresolvable` (`b34bf10e`). CAP-7 carries the table and the substrate
>   corrections.
> - **Layer 3 — not started.** Content-addressed identity lives in **CAP-8**
>   (`docs/trackers/capability-proposals.md`), which carries its own open decisions, a
>   substrate correction, and prior art at
>   `docs/superpowers/specs/2026-07-17-tracker-entry-graph-stage2-design.md`. Its payoff
>   figures are marked **upper bounds** — read that caveat before quoting them.
>
> **What is open elsewhere** (each carries its own `## Resume`, so nothing is owed here):
> the rendezvous latch bug `54a70b49f6f26681`, which this session's `4800c297` **widened**
> by voiding its `/mcp` workaround; `run_command` rewriting pipes inside heredocs
> (`d5cb0c41335b2610`); `grep`'s silent zero on an absolute glob (`8036fbf1666e1603`).
> `librarian(action="doctor")` now reports 8 `terminal_status_with_caveat` findings — that
> list is the fastest way back into this work.
>
> **This is a guarded ledger** — `entry_prefix: [F, W]` is declared in frontmatter,
> so `edit_markdown` is refused. Append via
> `artifact(action="append_entry", id_prefix="F"|"W", title=…, body=…,
> anchor_heading="## Template for new entries")` and let the server write the
> heading. Status vocabulary is the one in `docs/templates/session-log.md`.
> **Wins Index rows are ascending** — append after the last row; prepending to a
> named row put W-7 above W-6 and then W-8 above W-7, twice in one session.
>
> Predecessors (both archived): `archive/prompt-guide-refactor-session-log.md`,
> `archive/mcp-prompt-redesign-session-log.md`.
> **Update, later on 2026-08-19:** of the three open bugs named above, the heredoc
> pipe-rewrite is **fixed and archived** — new id `0de2778e6adac220`, fix `4ea33d15`,
> patch-id `50691255eccd`. The `d5cb0c41335b2610` above was re-keyed by that move and no
> longer resolves. Still open: the rendezvous latch bug and `grep`'s silent zero on an
> absolute glob (`8036fbf1666e1603`).
> **Update 2 (2026-08-19, later) — supersedes the update above.** Of the three bugs the
> header names, **two are now fixed and archived**: the heredoc pipe-rewrite
> (`0de2778e6adac220`, fix `4ea33d15`) and `grep`'s absolute-glob silent zero
> (`a23bdded8539b234`, fix `c38bfd91`). The commit-message backtick substitution is fixed
> and archived too (`5606ab6e35618aea`, fix `26de395c`). Only the **rendezvous latch bug**
> (`54a70b49f6f26681`) is still open of those named. Every id written above this line was
> re-keyed by its archive move and no longer resolves.
>
> Corrections stack here rather than replacing the text above because this block sits in
> the preamble, before the first heading, and `artifact(update, body_edits)` is strictly
> section-scoped — there is no heading to address it by. Filed as a bug.
## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-08-18 | high | prompt-surface | fixed-verified | `append_entry`'s `anchor_heading` is implemented but not advertised in the `artifact` schema |
| F-2 | 2026-08-18 | med | self-friction | fixed-verified | Read wire duplication as source duplication — the `workspace` param is injected once, not authored 24× |
| F-3 | 2026-08-18 | med | prompt-surface | open | 19.2% of the tool surface is bought for 38 calls, and the data cannot say whether those tools are dead or unrouted — field experiment **superseded** by hamsa A-26's controlled arms (0/10, 0/10, control 10/10); routing reverted in `89d32048`, evidence points at *substituted* not *unrouted* |
| F-4 | 2026-08-18 | med | librarian-api | open | `anchor_heading` inserts only *before* a heading, so a ledger that appends at the end cannot use the server-writes path |
| F-5 | 2026-08-19 | high | self-friction | fixed-verified | A guard that names the invariant but never exercises it — forcing the gate open left 62/62 passing; "mutation-tested" is a per-ASSERTION claim, not a per-commit one |
| F-9 | 2026-08-20 | high | substrate-drift | fixed-verified | Three rules promoted into the recon SKILL.md were in force in **zero of three** profiles — no version bump, so every cache served the pre-edit copy; and this session was the one observer that could not see it. Closed at the served bytes in `1.16.12` |
| F-8 | 2026-08-20 | high | process | mitigated | The `git add -A` prohibition existed, was measured, and still did not reach the moment of committing — and the mitigation I switched to is itself the documented anti-pattern one rung up |
| F-7 | 2026-08-19 | high | process | mitigated | A fired `Promote-when` is a zombie win and nothing queries for one — W-4 sat unharvested for a day while its failure recurred 3×, and the one lesson that WAS promoted reached 1 of 3 profiles |
| F-6 | 2026-08-19 | med | substrate-drift | fixed-verified | CAP-7 says check 3 needs no design — but `doctor` cannot reach the `[[project]]` list at all; two same-named `WorkspaceConfig` types, and a gitignored config that a worktree silently inherits from main |
## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-08-18 | med | Scout the generator before editing a generated surface | Would have shipped a "free mechanical dedup" with no valid implementation | validated |
| W-2 | 2026-08-18 | high | Do the cache arithmetic before scoping byte-shaving: a 100% cache_read surface costs its cache_read price, not its byte count | Would have sunk dozens of evals into `artifact`'s 51-param long tail to recover ~$0.0002/request, while the axis that might move (does the prose change behaviour?) went unmeasured | validated |
| W-3 | 2026-08-19 | high | Name the substrate before quoting a verdict — which tree, which binary, which index actually produced the number | Would have published 4222/1 as a gate result without knowing whether it described `HEAD` or a concurrent session's uncommitted mutant | validated |
| W-4 | 2026-08-19 | high | Calibrate a hand-built instrument against a known-good one on the overlapping population before extending it | Six instrument defects, each producing a plausible number and no error — 80% of scanned files were worktree duplicates, and the first patch-id test silently did nothing while exiting 0 | validated |
| W-5 | 2026-08-19 | high | When N records share a defect, fix the surface that instructed them before repairing any of them | Would have caveated 3 bug files while leaving 6 live instruction surfaces teaching the same rule to the next author | validated |
| W-6 | 2026-08-19 | high | Diff a tool's full report against the baseline a document recorded; account for every delta before reading the field you came for | Verification had already SUCCEEDED at its stated purpose — stopping there would have missed an unguarded write into a concurrent session's worktree, and left CAP-8's load-bearing "3 artifacts" figure standing while wrong | validated |
| W-7 | 2026-08-19 | med | Write the verification recipe at archive time — command, field, and the value that counts as proof — before the opportunity to run it exists | A post-hoc "check the ledger looks right" is satisfied by a cleared-then-refilled ledger, the exact state being disproved; the decisive evidence (a stamp OLDER than the slot holding it) was only noticed because the recipe named both fields to compare | validated |
| W-8 | 2026-08-19 | high | Scout the substrate even when the record says the design is settled — a record's substrate claim is a citation, not a fact | CAP-7's substrate check was wrong 4 times out of 4; two of the four would have shipped a confidently wrong diagnostic, including one that would report "fix SHA `12707fe` no longer resolves" about a commit whose own file says "Refactor 12707fe is INNOCENT" | validated |
| W-9 | 2026-08-19 | high | Treat retiring a bookkeeping rule as a schema migration over the record corpus — sweep for text that forward-references the retired step | Three fix pointers sat one rebase from orphaned, with the patch-id that would survive unrecorded because recording it WAS the retired step; all three were invisible to triage because terminal status is what the query filters out, and the measured recovery cost on the ten files this already happened to is 2–153 ambiguous candidates | validated |
| W-10 | 2026-08-19 | med | Reset one well-known element rather than everything — the survivor set is the only evidence the reset was correct | A cleared ledger and an inherited one both present as re-injection, so neither is distinguishable from the outside; because exactly one topic re-arms by design, one-of-five proved 58,963 bytes had been inherited before any state file was opened | validated |
| W-14 | 2026-08-20 | high | `ps \| head -N` returns N arbitrary processes, not the newest N — a "newest" claim needs an explicit sort, not a bigger N | The truncated list showed 01:27:46 as newest against a 02:25:39 build, supporting *"the process predates the build by 58 minutes"*; sorted and untruncated, the real newest was 46 seconds old and 13 minutes NEWER than the build. A verdict inversion, on R-89's own process axis, one hour after R-89 was re-promoted | validated |
| W-13 | 2026-08-19 | med | When one fix carries two separable behaviours, mutate them separately and name which fixture died | M6b passed the fixture written to pin its property — once fences were skipped that fixture's remaining backticks were incidentally even, so per-line pairing had zero discriminating coverage while looking fully covered | validated |
| W-12 | 2026-08-19 | med | Choose a sweep predicate by what it must DISTINGUISH, not by what it must find — a presence check scores a wrong-value defect as healthy | `grep -c SHA` returns ≥1 for 7 of the 9 unanchored records — the hashes are environments and siblings, never the fix — and the `grep -c patch-id` used to measure THIS entry scored two more as anchored on prose mentions | validated |
| W-11 | 2026-08-19 | high | Separate a finding's count from its remedy clause — the count is measured, the remedy is an inference about intent the checker cannot observe | Obeying it would have added 42 headings to an 1100-line tracker for entries with zero citations, contradicting the convention the file documents, and then silenced the check so the false premise became permanent; the same leap was also live on the write path, teaching it to every future author | validated |
| W-15 | 2026-09-02 | med | A substring join on `input_json` reads a value as a key — join on the key (`json_extract`) and read the rows before publishing a count | A bug file would have opened with "8 field instances in 30 days" for a defect with 0 real ones; a second join the same hour read 48 for a real count of 1 | open |
---

## Baseline measurement (2026-08-18)

Ground truth from driving `target/release/codescout start` over stdio with a real
`initialize` + `tools/list` handshake (XDG dirs redirected to a scratchpad so the
probe could not touch the real guide ledger). Recorded here so later compaction
passes have a comparable baseline.

| Surface | Size | Frequency | Size gate |
|---|---|---|---|
| `tools/list` (27 tools) | 58,882 chars — **over-counted, see correction below** | every request | descriptions only (14%) |
| `server_instructions` | 1,827 chars live | once per conversation | `source_md_under_cap` = 1900 ✅ |
| `get_guide` corpus (10 topics) | 90,485 B; 62,451 B auto-triggered | once per topic per session | **none** |
| `onboarding_prompt` + draft | 19,122 B + 2,128 B | once per project | none needed |

Within `tools/list`: descriptions 8,521 chars, schemas 50,361 chars (86%), of which
30,512 chars (61% of schema bytes) is param-description prose. The librarian family
(`artifact`, `librarian`, `artifact_augment`, `artifact_event`, `artifact_refresh`)
is 28,788 chars — 48.9% of the whole tool surface. `artifact` alone is 12,102 chars
across 51 params.

Guide corpus growth, measured with `git cat-file -s` per commit:
`tracker-conventions.md` went 6,804 B (2026-07-07) → 10,377 B (2026-08-16) →
23,252 B (2026-08-18) — 3.4× in six weeks, 2.2× in the last three days. The
byte-budget reasoning in `src/librarian/adapter.rs:197` and
`src/prompts/mod.rs:386` still cites the 10.4 KB figure, which was correct the day
it was written.

**Correction (same day) — take the tool-surface number from the harness, never from a
scratch probe.** The 58,882 above came from a Python probe that re-serialised the payload
with `json.dumps` at its default `ensure_ascii=True`, which expands every em-dash into a
six-character ASCII escape. The over-count therefore tracked prose density, and every
per-tool delta came out a multiple of 5. The authoritative figure comes from
`tool_surface_report_lengths` in `src/server.rs`, which reads the same `serde_json` bytes
the wire carries: **58,572** at the time of this baseline. The per-family splits in the
paragraph above inherit the same inflation and should be re-read from the harness rather
than trusted here.

The surface has moved twice since, both times deliberately and both times with the budget
constant ratcheted to the new total rather than left slack: 58,572 → 57,148 (declaring
`anchor_heading`, +808; compressing the injected `workspace` description, −2,232), then
57,148 → **56,266** (hamsa A-27's cut of five per-field restatements, −882). Run the
harness for the current number.

See [[W-2]] for why the byte total is the *wrong axis* to scope compaction on in the
first place — the surface is 100% `cache_read`, so all 56,266 characters cost ~$0.0043
per request.

Both auto-injects were observed first-hand this session: `tracker-conventions`
(23.2 KB) fired on an `artifact(action="find")` whose results named `docs/trackers/`
paths, and `librarian` (20.5 KB) fired on the following `append_entry` — 43.8 KB,
~11k tokens, for one session that touched trackers.

---

## Re-measurement and full-surface review (2026-09-02)

Ground truth from `scripts/probe_tool_surface.py` against `target/debug/codescout` (stdio
`initialize` + `tools/list`), cross-checked against the per-tool table the probe prints; every
param description below was read from that dump, and every claim about code was verified at the
cited line. Raw payload and per-param report kept for the session at
`/tmp/claude-1000/-home-marius-work-claude-codescout/2cb44cd3-8673-4604-a8ac-5adea75ca54b/scratchpad/`.

| quantity | 2026-08-18 baseline | 2026-09-02 |
|---|---:|---:|
| tools advertised (all caps true) | 27 | 26 |
| description chars | 8,521 | 7,688 |
| schema chars | 47,745 | 48,830 |
| **total** | **58,572** | **56,518** |
| `TOOL_SURFACE_CHAR_BUDGET` | 58,572 | 56,519 — **1 char headroom, flagged DEBT since 2026-08-28** |
| librarian family (`artifact`, `librarian`, `artifact_augment`, `artifact_event`, `artifact_refresh`) | 28,788 (48.9%) | 28,636 (**50.7%**) |
| injected `workspace` pin | 24 × 225 | 23 × 166 = 3,818 (6.8%) |

The schema half **grew** by 1,085 while descriptions shrank by 833: growth keeps moving sideways
into `input_schema()`, as the spec predicted. Six largest params, all prose: `librarian.fix`
1,133 · `artifact.patch` 1,102 · `edit_markdown.edits` 847 · `edit_markdown.frontmatter` 810 ·
`edit_markdown.action` 678 · `artifact.augment` 588.

[[W-2]] still governs: the surface is 100% `cache_read`, so none of what follows is a cost case.
The review is organised by what it found instead — **defects first, legibility second, bytes
last** — and the byte numbers are given so the next cut can be funded, not so it can be justified.

### 1. Defects — six bug files opened, one one-token fix noted

Each was verified at the wire *and* at the code, and two were reproduced live.

| bug file (`docs/issues/2026-09-02-…`) | class | what the surface says vs what the code does |
|---|---|---|
| `read-markdown-silently-ignores-offset-and-limit` | IC-15 | `read_file` normalises native-`Read` `offset`/`limit` (`read_file.rs:466`); `read_markdown` never calls it and returns the heading map. Reproduced live; 2 field instances 2026-08-25, both `success`. |
| `activation-banner-names-a-project-param-symbols-does-not-have` | IC-15 | `src/prompts/mod.rs:203` says pass `project:` to `symbols`/`semantic_search`/`memory`. `symbols` has no such param and dropped it live; `semantic_search`'s is `project_id`; only `memory` honours `project`, via an alias added for a **2026-06-09 bug filed against this same sentence** (`memory/mod.rs:448`). Fixed once, on one of three. |
| `workspace-schema-requires-an-action-the-code-does-not` | IC-11 | `required: ["action"]` (`config/mod.rs:46`); code and the companion hook (`session-start.mjs:338`) use `post_compact=true` alone — 31 of 120 such calls in 30 days, all OK. |
| `artifact-patch-schema-describes-a-failure-that-no-longer-happens` | IC-11 | `patch` opens with 170 chars about `missing field 'patch'`, removed by `60df0d76` (2026-08-27, `#[serde(default)]` at `update.rs:95`). Largest param on the surface, on every request. |
| `index-description-omits-the-verify-action` | IC-11 | description enumerates 3 actions; enum/dispatch have 4; `verify` called 16× in 30 days. |
| `artifact-action-labels-omit-delete-move-and-update-entry` | IC-11 | `id` labelled `get/update/graph/append_entry` while `delete.rs:10`, `mv.rs:11`, `update_entry.rs:9` require it; `delete` has **zero** labelled params. |

**One-token stale reference, no file (commit-message class):** `artifact_augment`'s description
ends *"Replaces artifact_update_params."* — no such tool exists; the only occurrence in `src/` is
that string (`augment.rs:272`). `prompt_surfaces_reference_only_real_tools` scans the three prompt
surfaces, not tool descriptions.

### 2. Legibility — what a reader of the schema cannot learn from it

**Twelve params carry no description at all** (type/enum/default only): `symbols.include_body`,
`call_graph.direction` (also no `type`), `call_graph.detail_level`, `edit_code.path`,
`edit_code.action`, `memory.action`, `semantic_search.limit`, `artifact.include_archived`,
`artifact.limit`, `artifact.offset`, `artifact.include_observations`, `librarian.include_archived`.
`artifact.include_observations` is a `get` param (`get.rs:187`) that nothing on the surface
attributes to any action. `all_tools_have_valid_schemas` asserts `is_object` and `type=="object"`;
an empty description passes it.

**The `<action>:` label convention is checked in one direction only.** The two flattened-union
tools rely on a prose prefix to say which action a param serves.
`every_action_labelled_schema_key_is_honored_by_that_action` (`artifact.rs:361`) proves every
labelled key is honoured; `assert_required_are_advertised` (`tools/mod.rs:578`) proves every
required key is advertised. Nothing proves every honoured key is labelled — the monotone gap
§ *Testing Discipline* names. Computed from the wire: `artifact.delete` 0 labelled params,
`artifact.move` 1 (not `id`), `librarian` 6 unlabelled (`include_archived`, `write`, `root`,
`old_root`, `new_root`, `confirm` — the last four are labelled by *fix mode*, not action, which
is a second labelling grammar inside one tool). `state_at` alone names its artifact `artifact_id`
(`artifact.rs:169`, `state_at.rs:149`); ten other actions say `id`, with no alias between them.

**Aliases are represented three different ways.** `file_path` works on *every* path-bearing tool
by design (`fs/mod.rs:230`, `PATH_PARAM_ALIASES`) but is advertised as a property on 6 of 15
(`read_file`, `grep`, `create_file`, `edit_file`, `edit_markdown`, `read_markdown`); `edit_code`
and `memory` mention their aliases in the canonical param's prose; `symbols` advertises both
spellings as peers and labels one *"alias of"* the other. Usage (30 days) says the labels point
the wrong way in `symbols`: `name` 2,378 vs `query` 87; `name_path` 1,045 vs `symbol` 561 — the
"alias" is the majority spelling in both pairs. `read_file`'s four alias params (`file_path`,
`output_id`, `offset`, `limit` — 464 chars, 29% of its schema) are all earning their place:
364 / 243 / 125 uses. Unadvertised-but-honoured works fine where it is deliberate: `edit_code`
took `file_path` 24 times, `symbol_at` twice, all OK.

**Restatement inside one tool — A-27's shape, untested here.** `artifact` states the prose-ledger
vs `entry_collection` distinction three times (description, `entry_collection`, `anchor_heading`)
and the RFC 7396 array warning twice (description, `patch`; the second is Rule B and should stay).
`patch` restates `edit_markdown.edits`' item shape as prose while `edit_markdown` carries it as
real nested schema. `get_guide`'s description names 7 of the 10 topics its own enum lists.
`artifact.augment` (create) carries the full 7-field nested schema of `artifact_augment` —
the augmentation shape is on the wire twice. A-27 found 20/20 with zero statements of a rule
that had been stated seven times; none of these has been measured, and per [[W-2]] each wants
an arm, not a cut.

**A friction met live, not filed:** `artifact.rel_path` advertises a `find` shorthand *"lifted to
filter=… and reported under corrections"*; using it returns a `hint` that says *use the canonical
form next time*. Advertised and scolded on the same call.

### 3. Tool count — the consolidation question, with numbers

[[F-3]] and A-26 stand: a null does not authorise a deletion, and `call_graph`'s zero was
substitution, not ignorance. Folding is not deletion — capability survives — but every fold
renames a tool that guides, hooks, `CLAUDE.md`, `TAXONOMY.md` and tests cite by name, and that
ripple is the real cost. Counted 2026-09-02 (`grep -rl`, files):

| fold | chars on wire | calls / 30d | docs `.md` | guides | companion | `src/` | verdict |
|---|---:|---:|---:|---:|---:|---:|---|
| `artifact_augment` → `artifact(action="augment")` | 2,924 | 59 | 114 | 4 | 1 | 8 | **only fold with a legibility case**: `artifact.augment` already schemas the same 7 fields; `merge` semantics would join them |
| `artifact_refresh` → `artifact` / `librarian` | 1,151 | 4 | 44 | 2 | 5 | 6 | clean param fit (`scope`, `limit`, `id` already exist); low traffic, mostly explains that `gather` does not write |
| `artifact_event` → `artifact` | 2,258 | 27 | 59 | 1 | 0 | 8 | **blocked**: `kind` collides (artifact kind vs event kind) — IC-6's disambiguator problem |
| `approve_write` → `workspace` | 611 | 8 | 21 | 0 | 1 | 5 | bytes only |
| `onboarding` → `workspace` | 555 | 2 | 249 | 2 | 12 | 10 | bytes only; huge citation ripple |
| `library` → `index` | 825 | 0 | 216 | 2 | 6 | 13 | bytes only; availability gate just fixed (`095088c4`) |
| `read_markdown` → `read_file` | 953 | 2,500 | 160 | 4 | 11 | 10 | **doctrine change** (Iron Laws 4/5, hooks, guard) — not a compaction |

Maximum recoverable by every fold that is not blocked or doctrinal: ~5.5K chars, ~10%, worth
~$0.0004 per request. The structural alternative that *would* change legibility is a per-action
schema (`oneOf` / `if`–`then` on `action`) for `artifact` and `librarian`: it replaces the prose
label grammar with one a machine can check (`required` per action becomes structural, and
§ 2's gap closes by construction). It needs two things before it is a proposal: a probe that
Claude Code passes a nested `oneOf` through to the model unchanged, and an arm measuring
parameter selection against the flat form. Bytes would go **up**.

### 4. What to do, in order

1. **Fix the six filed defects.** They are correctness, not compaction. The `patch` trim alone
   (~−100) funds advertising `offset`/`limit` on `read_markdown` and naming `verify`.
2. **Describe the 12 undescribed params and complete `artifact`'s labels**, then add the two
   gates the review found missing: every property has a non-empty description; for tools whose
   description enumerates actions, every enum value appears in it; and a registry-wide,
   bidirectional `required` ↔ `call()` probe (today's runs one direction, librarian only).
3. **Run A-27-style arms** on the restatements in § 2 before cutting any of them — the
   candidates are named there.
4. **Consolidate only `artifact_augment` and `artifact_refresh`**, if at all, and only after the
   `oneOf` probe says which shape the folded tool should take. Leave `read_markdown` alone.
5. **Ratchet `TOOL_SURFACE_CHAR_BUDGET` down** with every trim; it has been debt for five days.

**Measurement notes for whoever re-runs this.** A `LIKE '%"project"%'` join on `usage.db`
returned 8 `symbols` calls that looked like the banner defect firing; all 8 were searches *for a
symbol named* `project_id`. Join on the param key, then read the rows — see [[W-15]]. And
`LIKE '%state_at%'` over `artifact` calls matched 48 rows of tracker *bodies* mentioning
`workspace_state_at`; the real count of `state_at` calls was 1.

---
## F-1 — `append_entry`'s `anchor_heading` is implemented but not advertised in the `artifact` schema

**Observed:** 2026-08-18, prompt-surface audit — reserving R-106 in `docs/trackers/reconnaissance-patterns.md`.

**When:** Called `artifact(action="append_entry", id=…, id_prefix="R")` to reserve an id, intending to hand-write the `## R-106 — …` section.

**Expected:** The `artifact` tool schema documents `append_entry` thoroughly — `entry`, `entry_collection`, `id_prefix` and `cites` are all described at length (the `entry_collection` description alone is 496 chars and walks through the prose-ledger reservation flow).

**Got:** The response's `next_step` read: *"Next time, pass `title`, `body` and `anchor_heading` to have the server write it and remove that failure mode entirely."* `anchor_heading` is **not a declared property** of the `artifact` input schema — verified against the live `tools/list` dump, which has 51 properties and no `anchor_heading`. `title` and `body` are declared, but described as `"create: artifact title"` / `"create: markdown body"`, with nothing connecting them to `append_entry`.

The parameter is real and load-bearing: `src/librarian/tools/append_entry.rs:34` declares `anchor_heading: Option<String>`; lines 112–132 require all three fields together and refuse a partial set naming what is missing; `src/librarian/catalog/augmentation.rs:836–1088` writes a `def_re`-conformant `## <ID> — <title>` heading at the ledger's own level, validates that the anchor exists verbatim, and writes nothing at all on a bad anchor (pinned by `a_bad_anchor_writes_nothing_at_all_not_even_the_high_water_mark`).

**Probable cause:** The `d3c1e6ed` / `f19d5296` / `758b37dc` line of work made uncitable entries a first-class concern and added the server-side section writer as the structural fix. The tool schema was not updated in the same commit, so the prompt-surface review that `src/prompts/README.md` mandates for "any change to tool behavior or signatures" did not run.

**Why this is the sharp end:** this is the one path that structurally *cannot* produce an uncitable entry, and it is discoverable only by first doing it the fallible way and reading the follow-up hint. Every agent that reserves-then-hand-writes is executing the exact failure mode the feature exists to remove. Measured cost of that mode, from the same work stream: 13 ledgers across five repos carrying entries their body never defines, the largest at 64 of 68, and one namespace resolving to nothing against 117 live citations.

**Workaround:** Use it regardless — it is accepted despite being undeclared. This entry was written with it, which is also the confirmation that the path works end to end.

**Severity:** high — a correctness feature invisible on the only surface an agent reads.

**Status:** fixed-verified — declared in `01194e21`, pinned by `server::tests::artifact_advertises_the_append_entry_section_writer` (mutation-verified: renaming the schema key makes it fail). Gate green, 4,164 passing.

**Fix idea / Pointer:** Declared `anchor_heading` in `Artifact::input_schema()` and re-scoped the `title`/`body` descriptions to name their `append_entry` role. Bug file: `docs/issues/archive/2026-08-18-append-entry-body-writer-undeclared-in-artifact-schema.md` (archived; the earlier pointer here named a slug that was never created). The +808 chars breached `TOOL_SURFACE_CHAR_BUDGET` and were paid by compressing the injected `workspace` description — see the spec, `docs/superpowers/specs/2026-08-18-tool-surface-budget-design.md`.

## F-2 — Read wire duplication as source duplication — the `workspace` param is injected once, not authored 24×

**Observed:** 2026-08-18, prompt-surface audit, immediately after presenting the baseline measurement to the user.

**When:** Ranking compaction targets. I had dumped the live `tools/list` payload and run a duplicate-detector over every property description.

**Expected (what I asserted):** The `workspace` param's 225-char description appears verbatim on 24 tools — 5,400 chars, 5,175 of them redundant, "8.8% of all schema bytes, deletable by mechanical means alone". I ranked it as recommendation #2: *"mechanical, no eval needed, no judgment involved"* and *"a free 5.2 KB with zero eval risk"*.

**Got (scouted reality):** There is exactly **one** copy in the source. `src/server.rs:1024-1026` calls `CodeScoutServer::inject_workspace_param` for every tool whose `pinnable()` returns true, and that function (`src/server.rs:496-508`) inserts a single hard-coded `json!` block into the advertised schema at `list_tools` time, idempotently. The source is already DRY; the duplication is a property of the MCP wire format, where every tool carries its own complete schema.

**Probable cause:** I measured the generated artifact and inferred the shape of its generator. The measurement was correct — the bytes are genuinely paid 24× per request — but "duplicated" and "authored 24 times" are different claims, and only the second licenses "dedupe it".

**Consequence for the plan:** the remedy changes character entirely. There is nothing to deduplicate. The only lever is shortening the one shared string, which trades clarity on 24 tools for ~165 chars × 24 ≈ 3,960 chars — a content judgement with an eval cost, not a free mechanical win. A second, better lever the misread hid completely: audit which tools actually need `pinnable()`, since the cheapest byte is the one not injected.

**Workaround:** Re-ranked before any edit was made. `pinnable_tools_advertise_workspace_param` only asserts the property is present, so shortening the string will not break the gate.

**Severity:** med — no wrong edit shipped, but the recommendation reached the user with a confidence ("zero risk", "mechanical") the evidence did not support, and it would have produced a subagent brief with no valid implementation.

**Status:** fixed-verified — corrected before any edit; the generator is read and cited above.

**Fix idea / Pointer:** Cross-cutting lesson filed as R-106 in `docs/trackers/reconnaissance-patterns.md`. Counterfactual recorded as W-1.

## W-1 — Scouting the generator before editing a generated surface caught a recommendation with no implementation

**Observed:** 2026-08-18, prompt-surface audit. Recon invoked after the baseline report was delivered and before any compaction edit.

**Pattern:** When a compaction/refactor target is identified from a **generated** surface (an MCP `tools/list` payload, a rendered template, a compiled schema, an API response), read the code that generates it before proposing the remedy. Measuring the artifact establishes *cost*; only the generator establishes *authorship*, and the remedy follows from authorship.

**Counterfactual:** Without the scout, recommendation #2 — "dedupe the `workspace` param across 24 tools, free 5.2 KB, zero eval risk" — would have gone forward. There are three concrete costs it would have incurred:

1. **No valid implementation.** A subagent briefed to "remove the 23 duplicate copies" would have found one `json!` literal in `inject_workspace_param` and either stalled or invented a change nobody asked for. Per the project's own dispatch rule, that is a controller defect, not a subagent one.
2. **A miscommunicated risk profile.** It was sold to the user as the zero-judgement item to do first. It is in fact the item with the widest blast radius per byte — one string that renders into 24 advertised schemas.
3. **A better lever stayed hidden.** Reading `src/server.rs:1024` surfaced `t.pinnable()` as the actual gate. Auditing which tools genuinely need a workspace pin removes whole 259-byte blocks rather than trimming one shared sentence — strictly better, and invisible from the wire dump.

**Confirming data points:**
1. F-2 (this session) — wire-vs-source misread on `workspace`, caught pre-edit.
2. F-1 (this session) — the same scout pass, run against `append_entry`'s runtime hint rather than its schema, found `anchor_heading` implemented at `append_entry.rs:34` and absent from the advertised schema. Reading only the advertised surface would have concluded the hint was advertising a nonexistent param; reading only the source would have missed that it is unadvertised. **Both surfaces were needed to state the defect correctly** — which is the same lesson from the other direction.
3. Also this session: two auto-injected guides (`tracker-conventions` 23.2 KB, `librarian` 20.5 KB) were observed firing first-hand rather than inferred from `relevant_guide_topic()` source, confirming the 43.8 KB per tracker-touching session that the baseline predicted.

**Impact:** med — prevented one unimplementable recommendation and one mis-stated risk profile; surfaced a strictly better lever.

**Promote-when:** A second session catches a generated-surface-vs-generator inference error pre-edit. At two datapoints, promote to `docs/trackers/reconnaissance-patterns.md`'s seven-laws distillation as a named clause under law A ("Ground truth is the artifact") — specifically that a *generated* artifact is ground truth about cost and about nothing else.

**Status:** validated

## F-3 — 19.2% of the tool surface is bought for 38 calls, and the data cannot say whether those tools are dead or unrouted

**Observed:** 2026-08-18, tool-surface audit.

**When:** Ranking compaction targets after joining the live `tools/list` payload against `usage.db`.

**Got:** Ten tools carry **11,299 characters — 19.2% of the per-request surface — for 38 calls in 30 days**. Four have *zero* lifetime calls: `onboarding`, `approve_write`, `call_graph`, `library`.

**Why this is not actionable as it stands:** zero calls is ambiguous. Either the tool is dead weight, or nothing routes to it. `src/prompts/README.md` rule 7 states the second reading outright — *"if a tool has near-zero calls despite being useful, the prompt isn't surfacing it"* — and trimming on the first reading saves bytes while foreclosing the fix. The usage data cannot separate the two, and no amount of re-reading it will.

**Severity:** med — 19.2% of a per-request surface, but acting on the wrong reading is worse than waiting.

**Status:** open — the 19.2% question is NOT settled, but the field experiment below is **superseded, closed 2026-08-18**, and the reading has changed.

---

### Outcome of the field experiment: superseded before it started

Hamsa audit **A-26** ran the controlled arms the same day, and they answered in an hour
what this two-week field study would have spent a fortnight failing to detect.

| arm | named `call_graph` |
|---|---:|
| base (no quickref lines) | **0/10** |
| treatment (lines shipped) | **0/10** |
| positive control (MANDATORY directive) | **10/10** |

The routing intervention was reverted in `89d32048`. Two consequences for this entry:

**The hypothesis this entry pre-registered was the wrong one.** F-3 named three readings
of a null — unrouted, never-tempted, **substituted** — and tested only the first. The
arms point at the third: on a question where one hop is provably insufficient, twenty of
twenty runs reached for `references`, byte-identically, with and without the routing
line. That is a strong competing prior, not an ignorance of the tool — whose 1,060-char
description ships on every request and already says *callers (blast radius)*.

**The measurement design was also wrong in a way worth keeping.** This entry proposed a
naturalistic two-week window with `symbols` as a workload-presence control. That could
never have separated the three hypotheses either — it would have observed another null
and licensed nothing, exactly as the 30-day window before it did. A controlled arm with a
positive control settled it in three runs of ten. **Where a controlled arm is available,
a field window is the weaker instrument, not the more realistic one.**

What remains open is the original question for the other nine tools: 11,299 characters,
19.2% of the per-request surface, for 38 calls in 30 days. Nothing here authorises a trim
— a null still does not authorise a deletion — and the next move for any of them is a
controlled arm on a stimulus that tempts the tool, not another observational window.

---

### Pre-registration (written 2026-08-18, before any result)

**Hypothesis.** `call_graph` and `tree` score ~0 because `server_instructions` never routes to them, not because they are useless. The suggestive correlation: **no tool with zero lifetime calls is named anywhere in `server_instructions`.** `references` is named and has 129 calls; `call_graph` serves the same domain, is arguably more useful for impact analysis, is named nowhere, and has 0.

**Intervention.** Two quickref lines — `call_graph` and `tree` — landed in `ba16b16a`. Static slice 1,654 → 1,747 chars against its 1,900 cap. Live for the next **new** conversation against a rebuilt binary (`cargo rb`), not for a `/mcp` reconnect.

**Baseline, all four active `usage.db` files, to 2026-08-18:**

| project | call_graph | tree | references | symbols | total calls |
|---|---:|---:|---:|---:|---:|
| codescout | 0 | 10 | 129 | 2,614 | 25,826 |
| claude-plugins | 0 | 0 | 2 | 9 | 386 |
| prompt-engineering | 0 | 0 | 0 | 16 | 164 |
| researcher | 0 | 3 | 0 | 26 | 329 |

**`call_graph` is 0 across 26,705 calls in four projects** — including `researcher`, the most navigation-shaped workload available, whose second-most-used tool is `symbols`.

**Controls.** `symbols` is the workload-presence control (high volume: did navigation work happen at all in the window?). `references` is the same-domain control (already routed, so it calibrates what "routed and used" looks like). Neither is touched by the intervention.

**Metric.** Calls per tool with `called_at >= '2026-08-19'` — conservative, excluding the pre-change part of 2026-08-18.

```sql
SELECT tool_name, COUNT(*) FROM tool_calls
WHERE called_at >= '2026-08-19'
  AND tool_name IN ('call_graph','tree','references','symbols')
GROUP BY tool_name ORDER BY 2 DESC;
```

Run it against each of the four DBs; `codescout_sha` is available if the window needs tightening to specific builds.

**Decision rule, fixed in advance:**

| Outcome | Reading | Action |
|---|---|---|
| `call_graph` > 0 | it was **unrouted** | keep the lines; the 19.2% question resolves toward *route, don't trim*, and the other 8 tools get the same treatment before any trim |
| `call_graph` == 0 **and** `symbols` ≥ 200 | routed, navigation work happened, still unused | **evidence toward** dead weight — not a mandate; see the asymmetry below |
| `symbols` < 200 | the window contained no navigation workload | **inconclusive** — extend the window, do not conclude |

**Known weaknesses, stated up front rather than discovered later.** n ≈ 10 sessions over two weeks, one developer, one prompt surface, and 96.7% of all historical calls come from a single project. This design **cannot** separate "the tool is dead" from "this developer does not do impact analysis". It also cannot rule out that a quickref line is too weak an intervention where a worked example would not be.

**The asymmetry is the honest reading, and it is pre-committed:** a positive result is strong (something changed when only routing changed); a null result is weak. **A null must never on its own authorise removing a tool** — the same law as `reconnaissance-patterns` R-3 → R-79, this ledger's most-repeated: a search that finds nothing is evidence about the search, and a negative result does not authorise a deletion. Removal needs a positive finding, measured, and preferably a second independent signal.

**Fix idea / Pointer:** Re-run the query on or after 2026-09-02 and record the outcome against the table above. Spec: `docs/superpowers/specs/2026-08-18-tool-surface-budget-design.md` § Revisit-when. Intervention commit `ba16b16a`; gate `598b92f2`.

## F-4 — `anchor_heading` inserts only *before* a heading, so a ledger that appends at the end cannot use the server-writes path

**Observed:** 2026-08-18, pre-registering A-27 into
`docs/trackers/prompt-hamsa-audit-log.md` — the same ledger A-26 went into, using the
`title` + `body` + `anchor_heading` path shipped in `01194e21` (the F-1 fix).

**Expected:** the server writes the `## A-27 — <title>` section itself, which is the
whole point of that path — a hand-written heading missing its dash-and-title defines
no token under `link_scan`, so every citation of the entry dangles.

**Got:** no usable anchor. `anchor_heading` inserts the new section **before** the named
heading (`append_entry.rs:23`), and `## A-26 — …` is the *last* heading in the file
(line 594 of 701; `awk 'NR>594 && /^#{1,4} /'` returns nothing). An append-only ledger
with no trailing section has nothing after the insertion point to name.

**Probable cause:** not a defect in the safety reasoning — `append_entry.rs:106-110`
refuses to infer placement on purpose, citing
`docs/adrs/2026-07-10-repair-and-continue-input-handling.md` ("a write accepts an
explicit target and never infers one"). The gap is that *append at end* is an
**explicit** target too, and the API has no way to say it. The trio-or-nothing guard
then makes the path all-or-nothing rather than degrading.

**Workaround:** the two-step path — `append_entry` with `entry_collection` only (row +
id + high-water mark), then `artifact(action="update", patch={body_edits: [{heading:
"## A-26 — …", action: "insert_after", content: "## A-27 — …"}]})`. This is what A-26
itself used, and what A-27 used. It works, but it hand-writes the heading, which is
exactly the failure mode F-1's fix existed to remove.

**Severity:** med — the safe path is unavailable on precisely the ledger shape that
needs it most (append-only, no trailing template). `prompt-hamsa-audit-log.md` has 26
entries and no trailing section; `prompt-surface-compaction-session-log.md` has
`## Template for new entries` and is therefore fine, which is why F-1's fix verified
green against one ledger and not the other.

**Status:** open

**Fix idea / Pointer:** accept an explicit end-of-body sentinel rather than inferring —
e.g. `anchor_heading: "$end"`, or a sibling `anchor_position: "end"` — so placement
stays caller-stated and the ADR's law is honoured. Cheaper alternative with no schema
cost: none, since the trio guard is what forces the anchor. Note the schema line
already spends 466 characters on this field, so a sentinel is documentable in ~40 more.
Contrast [[F-1]], which shipped this path.

## W-2 — Compression cannot be justified on cost — the surface is 100% cache_read, so the case is legibility and must be measured like one

**Observed:** 2026-08-18, deciding what to compact after the budget gate shipped. The
obvious next move was "the surface is 57,148 characters, cut the biggest tools" — and the
cost arithmetic says that is close to pointless.

**Pattern.** Before scoping any byte-shaving work on a cached surface, do the cost
arithmetic first and state it out loud:

| quantity | value |
|---|---|
| whole tool surface | 57,148 chars ≈ 14,300 tokens |
| observed cache hit rate | **100.0%** across 4 sessions / 3 models |
| cache_read price | $0.30/M |
| **cost of the entire surface** | **~$0.0043 per request** |
| cost of the 882 chars A-27 cut | **~$0.00006 per request** |

The same measurement that refuted *relocation* (moving schema prose into a `get_guide`
topic — break-even K ≈ 12.5 turns, so a cached surface always wins) also undercuts
*compression for cost*. A 100%-cache_read surface is the cheapest place bytes can live.

So the case for cutting is **legibility**, not economics — and legibility is a claim
about how a reader behaves, which is exactly the class of claim that needs an eval rather
than an assertion. That reframing is what made A-27 worth running: not to save 882 bytes,
but to answer a question that generalizes to every schema in the repo — *does per-field
restatement of a global rule improve parameter selection, or is it cargo cult?*

**Counterfactual.** Without this arithmetic the natural plan was a long-tail sweep of
`artifact`'s 51 parameters (12,727 chars, no single fat target), each cut needing its own
arm under P-4's inverted burden. That is dozens of evals to recover a few thousand
characters worth ~$0.0002 per request — effort spent on the axis that does not move,
while the one that might (does the prose change behaviour?) goes unmeasured. A-27 got a
transferable answer out of one tool instead.

**Confirming data points:**
1. A-27 (2026-08-18) — 882 chars cut; the byte win is explicitly disclaimed in the
   pre-registration, and the finding that shipped was the mechanism, not the bytes.
2. The refuted relocation proposal, same session — same cache measurement, opposite
   intervention, same conclusion.

**Impact:** high — it redirects the whole work stream from byte-shaving to
behaviour-measuring, and it is the reason [[F-3]]'s remaining 19.2% question should not
be answered by deletion.

**Promote-when:** a third compaction decision is scoped by this arithmetic rather than by
byte count. At that point promote to `docs/PROGRESSIVE_DISCOVERABILITY.md` as a sizing
rule: *a cached surface's cost is its cache_read price, not its byte count — justify cuts
on legibility and measure them.*

**Status:** validated — two datapoints, both this session, both pointing the same way.
Awaiting the promotion criterion.

## F-5 — A guard that names the invariant but never exercises it — "I mutation-tested it" is a per-ASSERTION claim, not a per-commit one

**Observed:** 2026-08-19, reviewing `b3161def` (the A-29 pin notice) before running any
arm. The commit shipped four assertions and its message reported "mutation-verified …
3 of 3 killed".

**Expected:** `switch_away_hint_carries_no_pin_notice_while_the_gate_is_shut` guards the
default-off invariant — that an unmeasured intervention cannot reach production.

**Got:** it guarded nothing. It checked the gate parser and that the suffix string was
non-empty; it never touched the composed hint. **Forcing
`workspace_pin_notice_enabled()` to return `true` unconditionally left all 62 tests in
the module passing.** The neighbouring `activate_hint_shows_switched_when_away_from_home`
cannot catch it either — it asserts `contains`, which an appended suffix does not disturb.

**Probable cause — and the part worth carrying forward.** The commit *did* mutation-test
three assertions, honestly and successfully. The fourth was the one whose mechanism I had
not thought through, and it is exactly the one that got a coverage claim instead of a
mutation. **Mutation-testing is a claim about an ASSERTION, not about a commit**; reporting
"3 of 3 killed" alongside a fourth untested assertion reads as full coverage and is how a
green bar acquires authority. The tell was available and I did not look: the three tested
assertions each named a *string*, and the untested one named a *behaviour* — and only the
behaviour needed a real call to observe.

**Second defect, found by the same fix.** Once the guard drove a real activation, its
failure message printed the composed hint — which read *"remember to `workspace(...)` when
done"* immediately followed by *"do not activate"*. A flat contradiction, invisible in
source because the two halves sit ~200 lines apart and each is fine alone. It would have
shipped into A-29's arms and measured a muddled instruction while still returning a number.
**Reading the artifact the user actually receives is a different act from reading the code
that builds it**, and only the first one finds this class.

**Workaround:** none needed — both fixed in `b8f6200a`. The guard now drives a real
activation and asserts on the returned hint; the intervention text conditions both
instructions instead of appending one after the other.

**Severity:** high — not for the bug's blast radius (the gate was shut, so nothing
shipped) but for the *method*. An unmeasured prompt intervention reaching production with
a green suite is the precise failure the inverted guards on A-25/A-26/A-27 exist to
prevent, and the guard that was supposed to prevent it was itself the hole.

**Status:** fixed-verified — mutation re-applied after the fix; the new guard fails,
naming the leaked marker.

**Fix idea / Pointer:** two habits, both cheap:

1. **Mutate the invariant, not the helper.** If a guard's name says "X cannot reach
   production", the mutation is *make X reach production* — not *break the parser X reads*.
   If the whole suite still passes, the guard is decorative.
2. **Assert on the artifact the caller receives.** For anything composed from parts
   (hints, prompts, rendered surfaces), a test over the parts cannot see contradiction
   between them. Related: [[W-2]] on measuring the wrong axis, and the same lesson at
   harness level as `prompt-engineering:OP-5`, where a checker missing its exec bit
   reports a clean `0/N` that is character-identical to a genuine floor.

## W-3 — Name the substrate before quoting the verdict — which tree did the gate actually run against?

**Observed:** 2026-08-19, discharging the archived truncation bug's gate-1 condition
(full `cargo test` green on `experiments`).

**Pattern:** Before quoting a test result, a tool report, or a diagnostic count as
*verification*, establish which **substrate** produced it — which tree, which binary,
which index. Check it before *and* after, and read the run's own build lines rather than
assuming continuity across two commands.

**Counterfactual:** A concurrent session held an uncommitted mutation in
`src/tools/config/mod.rs` (hoisting `Agent::activate` above the `switched` computation,
which would pin `switched` false forever). It was present when `cargo test link_scan`
compiled, and reverted before `cargo test --no-fail-fast` ran. Both runs returned
clean-looking numbers and neither mentioned the other's existence. Without the
before/after `git status` plus reading `Compiling …; Finished in 1.42s` out of the run's
own buffer, I would have published **4222 passed / 1 failed** as a gate result while not
knowing whether it described `HEAD` or a mutant.

Note that the two most natural wrong claims were both *available* and both *plausible*:

- *"gate green on my fix"* — right number, unverified tree;
- *"M8 survived the whole suite"* — a coverage claim about someone else's work, drawn
  from a run that never contained their mutation.

The second is the more dangerous, because it is generous-sounding, concerns a colleague's
code, and would have been offered as a favour.

**Confirming data points:**

1. This session — the mutation was present at run 1's compile and absent at run 2's; only
   the recompile line distinguishes them, and nothing surfaces it unprompted.
2. The archived truncation bug's own gate-2 condition exists for exactly this reason:
   `link_scan` executes *inside* the MCP server, so which binary answered had to be
   established before its output could count as evidence in either direction. The bug file
   warns in as many words against reading a stale row as a failed fix.

**Impact:** high — the failure mode emits a **number, never an error**, and the number is
perfectly correct about a tree nobody has. Nothing downstream can detect it, because
there is no defect in the artifact to find.

**Promote-when:** a third instance where a verification's substrate (tree, binary,
database, index) turns out to differ from the one assumed. At 3 datapoints, promote to
`CLAUDE.md` alongside the mutation-apply discipline, as: *name the substrate before
quoting the verdict.* The two rules are the same rule seen from opposite ends — one says
a green bar proves nothing until you mutate it, the other says a measurement proves
nothing until you know what it measured.

**Status:** validated — two datapoints, both in this work stream.

## W-4 — Calibrate a hand-built instrument against a known-good one before believing any number it produces

**Observed:** 2026-08-19, sizing the payoff of the content-addressed-identity proposal
(CAP-8) across 10 repos in 2 umbrellas.

**Pattern:** When you build an ad-hoc measuring instrument (a script, a regex, a walk) and
a **known-good** instrument already covers part of the same population, run both on the
overlap and compare *before* extending yours to the part it does not cover. A ratio near 1
licenses the extension; a large ratio localises what you got wrong.

**Counterfactual:** My scan reported codescout at 7261 ambiguous citations against
`link_scan`'s 423 — **17×**. Every intermediate number I had produced up to that point was
wrong, and none of them looked wrong. Six distinct instrument defects, each found only by
checking:

1. `.claude/worktrees/` not excluded — **80% of codescout's 6858 markdown files were 5–6
   copies of the same content**. This alone accounted for most of the gap.
2. The noise-prefix regex included single-letter prefixes (`T-`, `M-`, `N-`), which ate
   *genuine* ledger tokens. "9% noise" was really **1%**.
3. Self-references counted as breakage — **35% of all mentions**. `link_scan` has a test
   named `self_citation_wins_even_with_other_definers`; a file citing its own entry
   resolves fine.
4. Fenced code blocks not stripped, so quoted examples counted as citations.
5. The archived-loses-to-active tie-break unmodelled, overstating ambiguity.
6. In the *fix* for a stale count, I nearly wrote 16 where recounting gave **17** — and the
   recount also revealed my checking regex had false negatives (`S-NN` and `HY-10` do not
   match a `PREFIX-N` pattern).

After the fixes: self-cites 1.27×, dangling 1.19×, ambiguous 1.73×. **Two of three
converging is what licensed reporting the third** — a uniform 5× gap would have meant a
population mismatch, but two matching while one stays high localises the residual to
something specific (link_scan's dangling is prefix-gated to declared ledgers, and it scans
1072 catalogued artifacts rather than the filesystem).

**Confirming data points:**

1. This session — six defects, zero found by re-reading the script, all six by comparing
   its output to something independent.
2. The `git patch-id` rebase-invariance test: the first run silently did nothing because
   `git cherry-pick` has no `-q` flag, and `exit_code` was **0** because the last command in
   the chain succeeded. The result — "patch-id ALSO CHANGED" — was a broken instrument, not
   a finding, and would have killed a correct design.

**Impact:** high — every one of these produced a *number*, never an error. A wrong number
is publishable, citable, and compounds: it becomes the denominator of the next decision.

**Promote-when:** a third work stream where a hand-built measurement diverged from a
known-good instrument on the overlap. At 3, promote to `CLAUDE.md` next to the
mutation-apply discipline — the three are one rule: **a green bar, a measurement, and a
tool's output all need an independent check before they carry weight.** Pairs with
`prompt-surface-compaction-session-log:W-3` (name the substrate before quoting the verdict);
that one is about *which world* was measured, this one about *whether the ruler is straight*.

3. **Recurrence the same day, 2026-08-19 — and the important one, because knowing the rule
   did not prevent it.** Investigating the rendezvous defect, the slot directory was listed
   with `ls -la … | head -10` and the **cap was reported as the population**: "7 slots, 3
   repos". There are 9, and one of the two the cap hid carried the very `hook_at` stamp
   whose universal absence the finding asserted. The claim "every slot is unstamped" was
   published on truncated output, and only the operator asking *"but the rendezvous is
   present here, right?"* falsified it.

   That error was committed **while writing the bug file that cites this entry**. So the
   datapoint is not another instance of the failure — it is evidence about the *remedy*:
   holding the rule in mind is not the control. The control is a habit at the call site,
   because `head`/`limit`/`take` is where the instrument gets shortened, and nothing about
   that line looks like a measurement decision. Codescout's own tools already model the fix
   — `grep` says "a floor, not a count" when it caps — which is exactly the discipline a
   hand-rolled `| head` skips.

**Fourth datapoint (2026-08-19, evening).** Same failure, sharpest instance yet: spot-checked
**8** of 42 undefined `PV-N` tokens, found 5 with occurrences, and wrote "at least five
undefined entries are cited" into W-11. Five is the *sample's* count. The shipped check,
reading the whole corpus, returned **33**. The instance matters because of where it
happened — inside a correction to W-11, in an entry that already documents the same trap
twice, written by an agent who had just quoted the rule. Holding the rule in mind is
demonstrably not the control; the control is refusing to state a count that a `head`, a
`limit`, or a hand-picked sample produced. Three hand-measurements of one question gave
three different answers before the tool gave the fourth.

**Datapoints 5, 6 and 7 (2026-08-19, evening) — all three AFTER this entry was written,
validated, and quoted.** (5) `grep -c patch-id` reported 7 unanchored records; the
structured predicate found 9, because two files mention patch-ids in prose and one of those
two is the bug file whose SUBJECT is patch-ids. (6) `grep -c 'SHA'` counted the WORD, so the
three heaviest decoy carriers scored zero and were never opened — "three" where the
population held eight. The filter selected *against* the population it was characterising.
(7) `commit_like_hashes` paired backticks across the whole file, and one stray backtick
inside a fenced `grep` pattern voided every hash after it — "7 of 9" where the answer is
8 of 9.

**Promoted 2026-08-19 to all three CLAUDE.md profiles** as *"Measurement — Never State a
Count Your Instrument Did Not Measure"*. The promotion criterion ("at 3, promote to
CLAUDE.md") had fired at datapoint 3 and sat unharvested for a day while the failure
recurred three more times; measured at promotion time, `calibrat|instrument|measurement|sampl`
returned **0 across all three files**. Two of the three profiles were also missing the
Mutation-apply block entirely — this session ran on one of them, and applied mutation
discipline only because codescout's project-instructions path happens to point at the third
profile's copy. All three are now byte-identical (`md5 08f0ef6cb534`).

The promoted wording moves the control from the measurement step to the **publication**
step, because this entry's own evidence is that the measurement step is where it is not
noticed: running a command *feels* like verification, so Conclude Last's "open the artifact
or run the command" never fires — the command ran, it just measured the wrong thing.

**Status:** promoted-to-permanent-docs — 7 datapoints, all 2026-08-18/19, each with the
wrong answer available and plausible. Datapoints 3, 5, 6 and 7 all post-date the entry and
refute the "just remember it" reading of it. See [[F-7]] for the meta-failure the
unharvested criterion belongs to.

**Promoted-to:**
`~/.claude/CLAUDE.md` § Measurement — Never State a Count Your Instrument Did Not Measure
`~/.claude-sdd/CLAUDE.md` § Measurement — Never State a Count Your Instrument Did Not Measure
`~/.claude-kat/CLAUDE.md` § Measurement — Never State a Count Your Instrument Did Not Measure

D11-verified 2026-08-20: the section heading is present in all three, 1 occurrence each, and
the files are byte-identical (`md5 08f0ef6cb5345a3df50a3f4b3b989a96`).

**Anchor notes** — added 2026-08-20. Until then this entry's promotion claim named **no
target at all**, which is exactly the defect
`docs/issues/archive/2026-08-19-no-check-detects-a-fired-unharvested-promote-when.md` describes,
sitting unnoticed on the entry that provoked that bug.

- **Target — all three, named individually:** `~/.claude/CLAUDE.md`,
  `~/.claude-sdd/CLAUDE.md`, `~/.claude-kat/CLAUDE.md`. Named one by one rather than as
  *"the user's global CLAUDE.md"*, because that singular phrasing is precisely what let the
  2026-08-18 promotion land in one file of three and read as done.
- **Section:** `### Measurement — Never State a Count Your Instrument Did Not Measure`
- **Verified 2026-08-20:** all three present and byte-identical,
  `md5 08f0ef6cb5345a3df50a3f4b3b989a96`.
- **Anchor kind:** section-heading match, not a verbatim body quote. A quote is the fragile
  form — [[R-89]]'s promoted text was rewritten this same day, which would have turned a
  quote-based check red on a *correctly* promoted entry. Prefer an anchor that survives the
  rule being reworded: a stable heading, or a back-citation of this entry's id inside the
  target.

## W-5 — When several records make the same mistake, fix the generator — the records were obeying it

**Observed:** 2026-08-19, repairing three bug files that sat unarchived waiting on a
master-side SHA a fast-forward promotion would never mint.

**Pattern:** When N records share a defect, **read the surface that told them to do it
before repairing any of them.** Repairing the instances leaves the generator running; the
next author reproduces the defect faithfully, and the repair looks like carelessness on
their part rather than a working instruction.

**Counterfactual:** I had already caveated the three files and was about to move on. Marius
said *"lets fix taxonomy first"*, and the generator was there in two places:

- `docs/issues/_TEMPLATE.md` **contradicted itself** — its comment block carried the correct
  cherry-pick/fast-forward table, while its `## Fix` section, the prose an author actually
  reads *while writing that section*, said unconditionally to "list **master-side** commit
  SHAs".
- `docs/TAXONOMY.md`'s BUG row said the same thing with **no condition at all**.

The three authors were not careless. They read the instruction and followed it. Repairing
only the three files would have left both surfaces teaching the same thing to the next
author, and this session's other measurement says nothing re-reads a bug file once written.

The sweep found more than expected: **six** live instruction surfaces carried the rule
(`CLAUDE.md`, `docs/RELEASE.md`, `docs/TAXONOMY.md`, `_TEMPLATE.md`, the
`tracker-conventions` guide, and memory `gotchas`, which `RELEASE.md` names as the rule's
concise home). Two leaks appeared *inside my own edits*: a RELEASE.md code block left
contradicting the paragraph I had just rewritten above it, and a TAXONOMY section
(`## SHA-citation rule`) I had never opened, which taught the old rule and cited a CLAUDE.md
heading I had renamed forty minutes earlier — which then broke two further live citations.

**Confirming data points:**

1. This session — 3 instances, 6 generator surfaces, 2 self-inflicted leaks during the
   sweep itself.
2. `docs/trackers/reconnaissance-patterns.md`'s standing law that a declared consolidation
   leaks at call sites nobody swept: *declaring the law is cheap, the whole-tree sweep is
   the actual work, and the sweep is what gets skipped.* Same shape, arriving from the
   documentation side rather than the code side.

**Impact:** high — instance-repair on a live generator has a *negative* half-life: the pile
regrows while the repair makes it look handled.

**Promote-when:** a third instance where repairing records without repairing their source
document would have regenerated the defect. At 3, promote to `CLAUDE.md` as: *before
repairing N records that share a defect, find and fix the surface that instructed them —
then sweep every surface carrying the same rule, including the memories.*

**Status:** validated — 2 datapoints, one of them this session's own edit leaking twice
while sweeping for exactly this failure mode.

## F-6 — CAP-7 says check 3 needs no design — but `doctor` cannot reach the `[[project]]` list at all

**Observed:** 2026-08-19, post-compaction scout before implementing CAP-7 check 3
(`declared_root_missing`) in `docs/trackers/capability-proposals.md`.

**When:** Phase 1 reconnaissance, before writing any code — triggered by that tracker's own
augmentation prompt, which says the `Substrate check` line is the load-bearing part and must
be re-verified before acting.

**Expected (CAP-7 substrate check):** *"**Check 3 is fully specified already**, by the bug it
would have caught … names the assertion, the report shape, and says to site it next to
`abs_path_outside_managed_roots`. Nothing needs designing."* And separately: *"**Genuinely
missing:** no check reads bug-file frontmatter at all, and nothing resolves a git SHA"* —
naming git as the only new dependency, for check 1.

**Got (scouted reality):** the *assertion* is specified. The *substrate* is not, in two ways
neither CAP-7 nor the bug file mentions:

1. **`doctor` has no handle to the project list.** Its `ctx.workspace` is
   `crate::librarian::workspace::WorkspaceConfig` (`src/librarian/workspace.rs:9`), which
   carries `.roots` and umbrellas. The `[[project]]` entries live on a **different type with
   the same name** — `crate::config::workspace::WorkspaceConfig`
   (`src/config/workspace.rs:4-12`), whose `projects: Vec<ProjectEntry>` holds the
   `{id, root}` pairs (`src/config/workspace.rs:39-46`). Nothing threads the latter into
   `ToolContext` (`src/librarian/tools/mod.rs:84-103`). The check must therefore locate,
   read and parse `.codescout/workspace.toml` itself — precedent at
   `src/tools/config/mod.rs:511-515` — or the plumbing must change. A same-named type on
   the context field is exactly the shape that reads as "already available".

2. **The worktree case is a live decision, not an edge case.** The config is gitignored, so
   it does not travel into a linked worktree; when it is absent there, discovery silently
   falls back to the **main** checkout's settings, a state the code already names
   `topology: "inherited"` (`src/tools/config/mod.rs:939-948`). So "the active workspace
   config" is ambiguous in precisely the situation this repo is usually in — five linked
   worktrees exist right now. Which root the declared `root` resolves against, and whether
   an inherited config should be checked at all, has to be decided before the check can
   report anything trustworthy.

**Probable cause:** the substrate check was written from the *bug file*, which specifies the
assertion completely and correctly, and not from `doctor`'s context type. The bug is a
config-state defect, so its author had no reason to look at what a librarian tool can reach.
Two structurally different types sharing the name `WorkspaceConfig` is what let
"`ctx.workspace`" read as sufficient without opening it.

**Workaround:** implement check 3 as a self-contained config read (locate via
`crate::config::workspace::workspace_config_path`, parse, iterate `projects`) rather than off
`ctx`, and record the worktree resolution as a fourth open decision on CAP-7 rather than
deciding it silently inside the check.

**Severity:** med — would not have cascaded, but the first implementation attempt reaches for
`ctx.workspace.projects`, which does not compile, and then has to guess at root resolution
with the worktree fallback invisible. Under subagent dispatch that is at least one failed
task the controller absorbs; the worktree half could have shipped as a wrong-but-green check.

**Status:** fixed-verified — CAP-7's substrate check corrected in the same commit as this
entry, before any code was written.

**Fix idea / Pointer:** CAP-7 in `docs/trackers/capability-proposals.md`;
`docs/issues/archive/2026-08-08-workspace-toml-mis-rooted-declared-sibling-repos-as-projects.md`
§ Resume. The general lesson is W-4's, one level up: *a name is not a calibration*. CAP-8's
method note asks the next author to query the catalog for prior art before proposing; this
asks them to open the **context type** before declaring a check needs no design.

## W-6 — Diff a tool's report against the baseline a document recorded — the deltas are the findings

**Observed:** 2026-08-19, verifying that a newly-shipped `doctor` check (`f632e7ef`) was
live on the rebuilt binary after `/mcp` reconnect.

**Pattern:** when a diagnostic tool reports a *set* of counts and some document already
records an earlier run of the same tool, do not just read the field you came for.
**Diff the whole count vector against the recorded one, and account for every delta.**
The field you came to check is the one you already believe; the deltas are the only part
carrying information you do not have.

Concretely: CAP-7's substrate check had recorded a `by_check` vector from a run earlier
the same day. The verification run — whose only purpose was confirming `declared_roots`
appeared — differed in three entries:

| check | recorded | live |
|---|---:|---:|
| `frontmatter_id_mismatch` | 3 | **4** |
| `missing_file` | 2 | **1** |
| `worktree_scoped_row` | 2 | **3** |

Two of the three were one event. Artifact `aeece182252e710d` fired both new rows: a
backend-kotlin plan created that day in a *linked worktree*, whose
`worktree_scoped_row` detail carried `collision_with: "8dcbd4fcb9fd5ffc"` — **the same
value its frontmatter declared**. The shadow's id was correct; the check calling it *"a
move re-keys the row and this file kept the id it was moved away from"* was wrong.

**Counterfactual.** The verification I set out to run was `declared_roots` present, config
path named, `declared: 1, missing: 0` — and it passed. Stopping there was the natural end
of the task, and it would have read as a completely clean result. Three things would have
been missed:

1. `frontmatter_id_mismatch` conflates stale-after-move ids with live worktree shadows.
2. `fix=repair_frontmatter_id` filters only on `containing_root`, so it would **write to a
   file inside another session's active worktree** — while both sibling fixes
   (`reseat_worktree`, `prune_missing`) carry an active-registration guard and document
   why. The scope comment on the unguarded arm reasons carefully about cross-*repo* blast
   radius, measured from a real 207-file incident; the cross-*worktree* axis was never
   considered. One axis closed, one open, in the same function.
3. CAP-8's substrate check rests on *"the **3** that differ are the only artifacts carrying
   evidence of a prior identity"* — the figure its entire "invert stored-vs-derived ids"
   argument is built on. It is 4, and the population is contaminated, so the real figure is
   unknown until the check is fixed. That claim was written hours earlier and was already
   wrong.

Filed, fixed and archived the same day —
`docs/issues/archive/2026-08-19-repair-frontmatter-id-rewrites-files-in-registered-worktrees.md`,
`f772b8fe`.

**Why this is not just "read the whole output".** The deltas were legible *only* because a
prior run's vector had been written down. A count vector with nothing to compare it to
reads as a description of the world; the same vector against a baseline reads as a set of
events. That is the cheap part of this pattern — CAP-7's substrate check recorded its
`doctor` run in full rather than the one number it needed, and that decision is what made
this findable four hours later.

**Confirming data points:**

1. This session — three deltas, one live write-path defect, one falsified tracker claim.
2. W-3 (same log) — the substrate question, one level down: *which tree produced this
   number*. This is the sequel: *what did the same tree say last time*.

**Impact:** high — a fix that writes into a concurrent session's working tree, found by a
verification step that had already succeeded at its stated purpose.

**Promote-when:** a second instance where diffing a tool's full report against a recorded
baseline surfaces something the targeted check did not. At 2 datapoints, promote to
CLAUDE.md as: *"When re-running a diagnostic a tracker has already recorded, diff the full
report against the recorded one and account for every delta before reading the field you
came for."* Pairs with the verify-open cadence, which is the same discipline over a slower
clock.

**Status:** validated — single datapoint, defect filed and tracker claim corrected in the
same pass. Awaiting the promotion criterion.

## W-7 — Write the verification recipe at archive time — a predicate fixed before the opportunity arrives cannot be rationalised after it

**Observed:** 2026-08-19, archiving
`docs/issues/archive/2026-08-19-mcp-reconnect-leaves-rendezvous-inactive-so-activate-clears-the-ledger.md`
with the fix shipped but not observable yet, then executing the recorded check hours later
when a restart and a `/mcp` finally made it observable.

**Pattern:** when a fix cannot be verified at the moment it ships, do not archive it with
*"verify later"*. **Write the recipe: the exact command, the exact field, and the exact
value that would count as proof.** Then run it verbatim when the opportunity arrives.

What went into the `## Resume` section at archive time:

> *"To verify live: after a SessionStart has stamped the slot, `/mcp`, then check `hook_at`
> is non-null in `~/.local/state/codescout/servers/<new pid>.json`, and that a
> `workspace(activate)` no longer re-injects guides the conversation already holds."*

That is a **pre-registered predicate**. It names the artifact, the field, and the two
observations — one structural, one behavioural — before any of them could be seen.

**Counterfactual — two costs, and the second is the dangerous one.**

1. **Re-derivation.** By the time the chance came, the session had been through a
   compaction, a server crash, a `cargo rb`, a Claude Code restart and a `/mcp`. Without
   the recipe, "verify the fix" would have meant re-reading `publish`, re-deciding which
   file to look at and re-deriving what a passing state looks like. With it: one command.

2. **A post-hoc predicate is satisfiable.** This is the real value. *"Check the ledger
   looks right"* is met by many states — including a ledger that was cleared and then
   refilled by the very guides the check is supposed to prove were **not** re-sent. The
   pre-registered form is not: it demands `hook_at` non-null **in the new pid's slot**, and
   the decisive evidence turned out to be something no post-hoc reading would have thought
   to look for — a stamp of `09:54:19.791Z` sitting in a slot published at `09:57:46.355Z`.
   **A stamp older than the slot holding it** is unforgeable proof of inheritance, and the
   only reason it was noticed is that the recipe said to compare those two fields.

**Confirming data points:**

1. This session — the recipe was executed verbatim and both halves passed, with the
   discriminating evidence (the timestamp gap) falling out of the comparison it prescribed.
2. The same discipline is already validated one domain over: `CLAUDE.md` records that every
   pre-registration in `prompt-hamsa-audit-log` must make failures spot-readable **before a
   count is believed**. That is this pattern applied to evals; this is it applied to a fix.
   The shared claim is that a predicate fixed in advance is worth more than the same
   predicate chosen once the result is visible.

**Impact:** med — it did not prevent a wrong outcome here, it prevented an *unfalsifiable*
one. The fix was correct either way; what the recipe bought was proof rather than a
plausible reading, at roughly zero marginal cost (three sentences written while the context
was already loaded).

**Limit, stated because it is not yet tested.** The opportunity arrived in the same session
that wrote the recipe, so the harder claim — that a *later* session inherits the check and
runs it correctly — is unproven. Nothing here shows the recipe survives handoff, only that
it survives a few hours and several restarts.

**Promote-when:** a **different session** (or agent) executes an archive-time verification
recipe it did not write, and the check is decisive without re-derivation. At that point
promote to `docs/issues/_TEMPLATE.md` as a standing `## Resume` rule: *when a fix ships
unverifiable, record the command, the field, and the value that would count as proof —
never "verify later".* Until then this is one self-executed datapoint and should not be
generalised into the template.

**Status:** validated — single datapoint, executed and decisive, with the cross-session
claim explicitly not yet established.

## W-8 — Scout the substrate even when the record says there is nothing to design — on one entry it was wrong 4 times out of 4

**Observed:** 2026-08-19, implementing all three `doctor` checks proposed by CAP-7 in
`docs/trackers/capability-proposals.md`, scouting each one before writing code.

**Pattern:** a tracker entry's own account of the substrate is the part most likely to be
wrong, and being *told* it needs no verification is not evidence that it doesn't. **Scout
before designing, every time — including, especially, when the entry says the design is
settled.**

**The measurement, which is what makes this more than a restatement of reconnaissance.**
CAP-7 carried a `### Substrate check` section, written the same day by an author with the
code open. Four substrate claims were load-bearing enough to check before implementing.
**All four were wrong:**

| # | The entry said | Reality |
|---|---|---|
| 1 | check 3 is *"fully specified… nothing needs designing"* | `doctor` cannot reach the `[[project]]` list at all — `ctx.workspace` is a **different type with the same name** (`prompt-surface-compaction-session-log:F-6`) |
| 2 | *(implicitly)* a linked worktree is an edge case | The config is gitignored, so a worktree inherits main's — a decision, not an edge case, and one that would have shipped wrong-but-green |
| 3 | *"10 of **63** archived bug files"* | The archived corpus is **350**. The 63 was a subset quoted as a population |
| 4 | *(implicitly)* archived files are scannable for a fix SHA | Only **54** carry a structured pointer; the other 296 use prose naming *reproduction* commits and one explicitly exonerated suspect |

**Counterfactual, and it is not uniform across the four.** Two were merely expensive: #1
fails to compile, #3 mis-sizes an estimate. Two would have **shipped a confidently wrong
tool**:

- #2 would have produced a check that reports on the wrong config in five worktrees, green
  and silent.
- #4 is the worst. A hex sweep over prose would have emitted *"declared fix SHA `12707fe`
  no longer resolves"* about a commit whose own file says **"Refactor 12707fe is
  INNOCENT."** A diagnostic that libels an exonerated commit is worse than no diagnostic,
  because someone will act on it.

**Why the author was not careless.** This is the part worth carrying. CAP-7's substrate
check was written *by the same process that later found it wrong*, hours apart, with no
intervening change to the code. The claims were not guesses — they were readings of a
neighbouring artifact (the bug file, the earlier measurement) that were true **about that
artifact** and false about the substrate. A substrate claim decays the moment it is copied
out of the thing it describes; it is not a fact, it is a citation of one.

**Confirming data points:**

1. This session — 4 of 4, on one entry, in one day.
2. That entry's own augmentation prompt already said so: *"The `Substrate check` line in
   each entry is the load-bearing part… Re-verify it before acting on an entry; substrate
   moves and a stale check turns a proposal into a wrong instruction."* The prompt was
   right and the section it guards was wrong anyway — which is the argument for scouting as
   a **step**, not as a disposition.

**Impact:** high — two of the four would have shipped an incorrect diagnostic, and
diagnostics are believed.

**Promote-when:** a second entry (any tracker, any project) where ≥2 substrate claims fail
verification in one pass. At 2 datapoints, promote to `CLAUDE.md` alongside the
mutation-apply discipline as: *"A record's substrate claim is a citation, not a fact.
Re-derive it from the code before building on it, even when the record says it is
settled."* One entry is not yet a rate.

**Status:** validated — 4 datapoints within one entry, but only one entry; the
cross-entry claim is what the promotion criterion tests.

## W-9 — Retiring a rule orphans every record that was waiting on it — and terminal status hides them from every triage query

**Observed:** 2026-08-19, first work after compaction. `librarian(action="doctor")`'s
`terminal_status_with_caveat` returned 8 findings. Three were bug files whose fix pointer
was a *promise* rather than a value.

**Pattern:** When a project retires a bookkeeping rule, treat it as a schema migration
over the existing record corpus, not as a doc edit. Sweep for records whose text is a
**forward reference to the retired step** — those references are now permanently
unfulfillable, and nothing will ever prompt anyone to revisit them.

The three forms found, all in files whose `status:` was already terminal:

| File | What it said | What was there |
|---|---|---|
| `audit-doc-refs-gate-hides-its-own-cause` | "Experiments-branch SHA: recorded on commit" | no value |
| `run-command-unusable-without-git-bash` | "Fix SHA: `experiments` — recorded once the branch merges" | no value |
| `frontmatter-id-mismatch-asserts-one-cause` | "the fix SHA **below** is already the master-side SHA" | no SHA below |

The third is the sharpest: a claim about the document's own contents that the document
falsifies. None of the three is a lie — each was true-pending when written, and CLAUDE.md
retired the step that would have made it true.

**Why nothing caught them:** every one was `fixed` or `mitigated`, so the canonical triage
query — `kind="bug"` with status in `open`/`investigating` — could not reach any of them.
Terminal status is precisely what makes a record invisible, so **a rule retirement lands
hardest on the records that are already "done"** and softest on the ones anyone is
watching.

**Counterfactual:** all three fix commits are `experiments`-only (`git branch --contains`
shows no `master` on any). `experiments` is rebased after every ship, so `45669701`,
`3f8a43e7` and `51dd9368` were each **one rebase from being orphaned** — and the patch-id
that survives a rebase had not been recorded either, because recording it *was* the
retired step. Measured 2026-08-19 on this repo: 10 of 63 archived bug files had already
lost their SHA exactly this way, and subject-keyword recovery on those ten returned 2–153
ambiguous candidates. These three would have been numbers 11, 12 and 13, with no recovery
path left.

**What made them findable:** Layer 1 (`unverified:`) put the caveat in a *field*; Layer 2
(`terminal_status_with_caveat`) made a query read it. Neither layer knows anything about
rule retirement — the caveat's **author** already knew, and the check only stopped what
they wrote from being invisible. That is the legibility thesis paying out once, on a real
corpus, in a form no other surface would have produced. Recovered and recorded in
`c420e2df`; `archived_fix_shas` coverage moved `scanned` 54 → 57, `unresolvable` 0.

**Related observation (diagnostic design):** establishing that the three CAP-7 checks were
live could not be done from the violations map. `summary.by_check` lists only checks that
*fired*, so "check absent" and "check found zero" are character-identical in it — check 1
was missing from the map because it found zero unresolvable pointers. Only
`catalog_health.archived_fix_shas`, which reports **coverage** (`scanned`, `skipped`,
`unresolvable`) rather than findings, distinguishes the two. A clean diagnostic should
report the denominator, not just the numerator.

**Confirming data points:**

1. This entry (2026-08-19) — 3 of 8 `terminal_status_with_caveat` findings were
   orphaned-by-retirement, recovered inside their last window.
2. Pending: the next rule change that lands on an existing record corpus.

**Impact:** high — three fix pointers recovered while still recoverable; each would
otherwise have joined a population with a measured 2–153-candidate recovery cost.

**Promote-when:** a second rule retirement is found to have left orphaned records behind.
At 2 datapoints, promote to CLAUDE.md as: *"When retiring a bookkeeping rule, sweep the
record corpus for text that forward-references the retired step — terminal-status records
will not surface in any triage query."*

**Status:** validated — single datapoint, acted on and committed (`c420e2df`).

## W-10 — A partial reset is self-evidencing — the one topic deliberately re-armed on reconnect is the integrity probe for the other four

**Observed:** 2026-08-19, a `/mcp` reconnect after a rebuild, same conversation
(`a8acb1cf`). Second live verification of the rendezvous-inheritance fix (archived bug
`3e0f11d4875f0075`).

**Pattern:** When a subsystem resets state on an event, resetting **exactly one
well-known element** rather than all-or-nothing makes the reset externally verifiable.
Afterwards, *"that element re-armed AND its siblings did not"* is a signature only a
correct partial reset can produce. A full reset destroys the evidence that would
distinguish it from a correct inherit-then-retrigger — both look like re-injection.

**Read off the tool responses, before opening any state file:**

- `artifact(find, kind="bug", …)` returned **no** `_guide_hint`. The identical call before
  the reconnect emitted two (`librarian`, `tracker-conventions`).
- `run_command` **did** carry `_guide_hint` for `project-activation-bootstrap`.
- One re-armed, four preserved is only reachable if the ledger reloaded **non-empty**,
  because the server re-arms the session-opening topic *only* on a non-empty reloaded
  ledger. So the re-injection that looks like a failure is the proof of success.

**Confirmed at the bytes afterwards:**

- Slot `3285244.json` — `started_at` `10:56:25.703Z`, `hook_at` **`09:54:19.791Z`**: a
  stamp 62 minutes older than the slot carrying it, which no fresh stamp can produce. The
  same value was observed on slot `2759601` at `09:57:46Z` earlier the same day, so
  inheritance is **transitive across at least two directly-observed server generations
  spanning ~4 hours**, not the single hop the first verification established.
- Predecessor slot `3166062.json` is absent — gc ran *after* the inheritance read, which
  is the ordering the fix depends on (`inherited_stamp` before `gc` in `publish`).
- Ledger `a8acb1cf-….json` — four topics stamped 10:42–10:47 (pre-reconnect),
  `project-activation-bootstrap` stamped 10:56:54 (post).

**Measured saving:** 58,963 bytes not re-sent — `librarian` 20,545, `tracker-conventions`
24,918, `workspace-state` 10,355, `symbol-navigation` 3,145 — against 2,507 re-sent.
95.9% suppressed. The topic that must re-arm is also the **cheapest of the five**, which
is what makes the deliberate exception affordable; had the session-opening topic been the
24 KB one, the exception would have cost more than it protects.

**Counterfactual:** without the fix every reconnect clears the ledger, so all five
re-inject — and verification becomes impossible *in both directions*, because a cleared
ledger and an inherited one both present as re-injection. The fix does not merely save
bytes; it is what makes the state observable at all.

**Confirming data points:**

1. 2026-08-19, first verification — one hop, slot `2759601`, recorded with W-7's
   pre-written recipe.
2. This entry — second reconnect, after a rebuild. Transitivity established, and the
   one-of-five signature was read off tool responses before any file was opened.

**Impact:** med — the fix was already validated; what this adds is a standing probe that
costs nothing to run and a design rule that generalises beyond this subsystem.

**Promote-when:** a third subsystem gains an event-triggered reset. Promote to the
reconnaissance patterns as: *"When designing a reset, reset one well-known element rather
than everything — the survivor set is the only evidence the reset was correct."*

**Status:** validated — 2 datapoints.

**Incidental:** the rebuild was functionally a no-op. `doctor` returns identical results
across it (`scanned` 57, `unresolvable` 0, same `by_check`), and the source tree has not
moved since `b34bf10e` — the two commits after it are docs-only.

## W-11 — A diagnostic's remedy clause is a claim about intent — read the artifact's own policy before obeying it

**Observed:** 2026-08-19. Task was to close `doctor`'s last structural finding in this
repo — give `docs/trackers/provenance-subsystem.md` its 42 missing `## PV-N — <title>`
entry headings. Scouting the file first turned the task into a defect in the diagnostic.

**Pattern:** Separate a finding's **count** from its **remedy clause**. The count is
usually measured. The remedy clause is usually an inference about what the author *meant*,
and a checker that reads only its own two inputs cannot observe intent. Before acting on
it, read the artifact's own stated policy — a ledger, config or schema often documents the
convention that makes the "defect" correct behaviour.

Here the two clauses failed differently:

| Clause | Status |
|---|---|
| "42 of 68 entries have no heading" | measured, correct |
| "every citation of them resolves to nothing" | **vacuously true** — there are no citations |
| "This ledger defines its other entries, so these are omissions — add a heading for each" | **false for this ledger** |

The file states the opposite policy three lines above its first entry heading: the
defining-sections block carries a heading for every `PV-N` *another file references*, added
when the entry is first cited from elsewhere. Define-on-citation is coherent, and the
ledger follows it exactly.

**Measured before believing either side** — the question is not "how many lack headings"
but "is any *cited* entry undefined", since a dangling citation is the only harm the rule
exists to prevent:

- defined by a heading: **26** ids
- cited by any other file: **25** ids — a strict subset
- **cited but undefined: 0**
- `link_scan` over 1075 artifacts and 3883 citations: 538 dangling, 423 ambiguous, and
  **not one carries a `PV-` token**; `cross_repo` likewise.

**Sampling trap avoided on the way:** `grep -c 'PV-'` against the `link_scan` result
buffer returned 0 — but that buffer caps `dangling` and `ambiguous` at 50 rows each
against populations of 538 and 423. A zero from a truncated sample is evidence about the
sample. The conclusion rests on the direct id comparison instead.

**Correction, 2026-08-19 (later the same day).** The measurement above is accurate as
scoped — *cited by any other file* — but the gloss drawn from it, that the 42 undefined
entries are entries nothing references, is **too strong**. It never checked the ledger's
own body. Measured directly:

| token | occurrences in the ledger | defining heading |
|---|---:|---:|
| `PV-12` | 8 | 0 |
| `PV-3` | 3 | 0 |
| `PV-20` | 2 | 0 |
| `PV-1`, `PV-9` | 1 each | 0 |

So at least five undefined entries are cited in the ledger's own prose — `PV-12` eight
times, including inside a section heading. A reader following any of them lands nowhere.
Those are **real** navigational breaks, not policy.

**Second correction, same day: "at least five" was 33.** The table above is a sample of
eight tokens; five of them had occurrences, and that five was written down as though it
were the population. It is not — it is the sample's count, which is **W-4's exact failure,
on the third pass over this one question**, in an entry whose own text already warns about
a truncated sample twice. Measured by the shipped check across the whole corpus:

> 42 of 68 `items` entries have no heading. **Cited despite that: 33** … **Uncited: 9**

The running tally on this single question: *42 omissions* → *0 cited* → *~5 cited* →
**33 cited, 9 uncited**. Every hand-measurement was wrong, in a different direction each
time, and each felt like a correction of the last. The one that held was the one that read
the whole population instead of sampling it — which is the tool, not the reasoning.

What survives unchanged: the ledger's define-on-citation convention is real and documented,
no *external* citation dangles, and the diagnostic still asserted a cause it could not
observe. What does not survive: "add 42 headings" was wrong, but so is "add none" — the
correct answer is the partition the filed bug proposes, and this measurement is now the
strongest argument for building it. The truncated `link_scan` buffer could not have settled
this either way, which is the sampling trap noted below biting a second time on the same
question.

**Counterfactual:** obeying the finding adds 42 headings to an 1100-line tracker for
entries nothing references, contradicts the file's documented convention at 42 places, and
then *silences the diagnostic* — so the false premise becomes invisible and permanent.
Nothing downstream would ever have flagged it.

**Second-order, and the more valuable half:** the same unsupported leap lived on the
**write path** — `undefined_in_body_note` told an author *"so this one is an omission"* on
every `append_entry` to any such ledger. Found only by sweeping for the phrase rather than
repairing the surface that happened to report the problem (this is W-5's rule, applied to a
message rather than to records). Fixing the reporter alone would have left the generator
teaching the same thing to every future author.

**The fix that made the test possible:** *"This ledger defines its other entries"* is an
observable fact; *"so this one is an omission"* is an inference. Keeping the fact and
softening only the inference repaired the defect **and** left the existing test that pins
that phrase passing — the test was using it to identify which `DefinitionGap` arm fired,
which is still true. Splitting fact from inference is what made the change cheap.

**Unlooked-for bonus:** the guarding assertion prints the whole rendered message on
failure, and the mutation run's panic output showed *"before adding 1 headings"* on a
single-entry ledger — a grammar defect that had been invisible while the test was green.
**A failing assertion that dumps the rendered string is a free proofreading pass on
message text.**

**Confirming data points:**

1. This entry (2026-08-19) — `entry_without_definition`, two surfaces, `4ffd2803`.
2. Pending: a second finding whose remedy clause asserts intent the checker cannot observe.

**Impact:** high — prevented 42 wrong edits, converted a cosmetic task into a diagnostic
fix on two surfaces, and left the real defect (citation-aware partition) filed rather than
hidden behind a silenced check.

**Promote-when:** a second diagnostic is found asserting a cause it cannot observe. At 2
datapoints, promote to CLAUDE.md as: *"A finding's remedy clause is a hypothesis about
intent. Verify it against the artifact's own stated convention before acting, and treat a
check that cannot see the harm it names as the defect."*

**Status:** validated — single datapoint, acted on and committed (`4ffd2803`).

## W-12 — A presence check cannot find a wrong-value defect — `grep -c SHA` scores an observation SHA as provenance

**Observed:** 2026-08-19, working `doctor`'s `terminal_status_with_caveat` findings after
archiving the `entry_without_definition` bug.

**Pattern:** After repairing a record-keeping defect in one file, sweep the corpus — and
choose the sweep predicate by **what it must distinguish**, not by what it must find. The
natural predicate for "does this record have a fix anchor?" is *does a SHA appear in it*.
That predicate is satisfied by the defect. Prefer a predicate that separates the good case
from the bad one: here, *does a `patch-id` appear*, since a patch-id is only ever written
deliberately as provenance, while a hex string has three other reasons to be in a bug file
(environment, sibling fix, quoted commit).

**Counterfactual:** The triggering record said its outstanding item was a *master-side SHA
after cherry-pick*. Two things were wrong with that, and the stated one was the lesser: the
practice had been retired, **and no fix SHA was ever written into the file at all**. Fixing
only that file would have left every other live terminal record unanchored.

**Corrected 2026-08-19, by the check this entry led to.** The figures above were themselves
produced by a presence check — `grep -c patch-id` — which scored two records as anchored on
prose mentions, one of them a bug file whose SUBJECT is patch-ids. Measured by the shipped
`terminal_status_without_fix_anchor` against the structured `- **SHA:**` form: **9** live
terminal records lack an anchor, and **8 of the 9** carry commit-like hashes with none
declared as the fix. (Its first run said *seven* — a parity bug in the check's own hash
heuristic, fixed in `fea7101e`. Three instruments, three answers, each one measuring
something adjacent to the question. See [[W-13]].) The misleading shape is the majority, not a sub-case — which inverts
what the finding's wording has to do. An entry warning against presence-checks, measured
with one, twice.

The cost is asymmetric and measured. Recovering an anchor is cheap while a distinctive
identifier exists — `1e7722a0` came back in one `git log -S 'anchor_indent'` — and the
guide's measurement of the fallback, subject-keyword search, is **2–153 ambiguous
candidates**. `experiments` is rebased after every ship, so every unanchored record is one
ship away from that fallback.

**Confirming data points:**

1. W-5 (this log) — three bug files shared a defect; the repair belonged to the instruction
   surface, found only by sweeping.
2. W-9 (this log) — retiring a rule orphaned three records; found by sweeping for text that
   forward-referenced the retired step.
3. This entry — one instance repaired, seven more found, and **the three worst were the ones
   a presence check would have marked healthy**. That third clause is what this entry adds:
   W-5 and W-9 are about remembering to sweep; this is about the sweep being able to see
   the defect at all.

**Impact:** med — no runtime effect. It decides whether a fix can be found later, which is
the whole purpose of the record.

**Promote-when:** a fourth instance where the obvious presence-predicate scores the defect
as healthy. At that point promote to CLAUDE.md next to the SHA + patch-id rule, phrased as:
*a sweep predicate must distinguish the good case from the bad one — if the defect satisfies
the predicate, the sweep reports a clean corpus.*

**Filed and shipped the same day**, `375225cc`:
`docs/issues/archive/2026-08-19-terminal-bug-file-with-no-recoverable-fix-anchor.md`
(`53e35aaefb9f7c71`). `terminal_status_without_fix_anchor` is report-only — recovering a fix
SHA is research, and a wrong anchor is worse than an absent one. Five mutations applied,
zero survivors.

**Status:** validated — three datapoints in this log, the third contributing a distinct
mechanism. Awaiting the promotion criterion.

## W-13 — A fixture can pass a mutation of the very property it was written to pin — only applying the mutation shows it

**Observed:** 2026-08-19, fixing a parity bug in `commit_like_hashes`
(`src/librarian/tools/doctor.rs`) an hour after shipping it in `375225cc`.

**Pattern:** When one fix carries two separable behaviours, mutate them **separately**, and
check which fixture each mutation kills. A fixture written for behaviour B can pass B's
mutation because some incidental property of the fixture masks it — and it will look like
coverage right up until the mutation is actually applied.

**Counterfactual:** The fix had two parts: skip fenced blocks, and pair backticks per line
instead of across the whole file. One fixture was written for both, built from the real
failing document. **M6a** (disable fence detection) killed it. **M6b** (pair globally over
the non-fenced text) did **not** — it passed, because once fences are removed that fixture's
remaining backticks happen to be *even*, so global and per-line pairing agree on it.

So the per-line half — the defence against a stray backtick in **prose**, where no fence can
help — had zero discriminating coverage while appearing fully covered. A second fixture
(`commit_like_hashes_confines_a_stray_prose_backtick_to_its_own_line`) exists only because
M6b was run rather than reasoned about; under the mutant it returns `[]` instead of the hash.

This is a step beyond the CLAUDE.md mutation rule as written. That rule answers *"do the
tests catch this?"*. This answers *"does the test I wrote FOR this property actually
discriminate it?"* — and a fixture drawn from a real failing document is exactly the kind
most likely to carry incidental properties that mask one of its own assertions, because it
was never designed, only copied.

**Confirming data points:**

1. This entry — M6b passed the fixture written to pin its property; caught only by applying it.
2. Pending: any future case where a mutation of a named behaviour leaves its own fixture green.

**Impact:** med — no shipped defect, because the mutation was applied. The counterfactual is
a silent coverage hole in a heuristic whose failure mode is a plausible number and no error,
which is precisely how the parity bug reached `master`-bound code in the first place.

**Promote-when:** a second instance. Then promote to CLAUDE.md as a clause on the existing
mutation rule: *when a change carries two separable behaviours, mutate each separately and
name which fixture died — a mutation that kills nothing has found a coverage hole, not a
redundant test.*

**Status:** validated — one datapoint, with the second fixture as its artifact.

## F-7 — A fired `Promote-when` is a zombie win, and nothing queries for one

**Observed:** 2026-08-19, answering the operator's question *"is this a systemic failure?"*
after a seventh instance of [[W-4]]'s mechanism in two days.

**When:** Checking the *installation* surface rather than re-reading the error log — which
is what turned an impression into two verified gaps.

**Expected:** a validated entry whose promotion criterion has fired reaches the surface that
changes behaviour.

**Got — two gaps, both measured at the moment of the question:**

1. **W-4's criterion fired and was never harvested.** It reads *"at 3, promote to
   `CLAUDE.md`"*; it had 4 datapoints and `Status: validated`. Probe across all three
   profiles: `grep -ci 'calibrat|instrument|measurement|sampl'` returned **0, 0, 0**. It had
   reached nothing, while the failure recurred three more times that evening.
2. **The one lesson that WAS promoted reached one profile of three.**
   `.claude/CLAUDE.md` was 115 lines and carried the Mutation-apply block;
   `.claude-sdd/CLAUDE.md` and `.claude-kat/CLAUDE.md` were 97 lines without it, and
   **byte-identical to each other** — so this was one un-propagated write, not gradual drift.

The second is the sharper one. **This session ran on `.claude-sdd`**, the profile missing
the block, and applied mutation discipline all evening only because codescout's
project-instructions path happens to resolve to `/home/marius/.claude/CLAUDE.md`, injecting
the other profile's copy as a second file. On a repo without that coincidence, the discipline
behind every verified result of this session is simply absent — and nothing would have said so.

**Probable cause:** fix-then-forget, which this project has already measured on two other
surfaces and not on this one. `get_guide("tracker-conventions")` records **39 of 57** entries
in one ledger with fired criteria unharvested over three months; CLAUDE.md's verify-open
cadence records a **75%** zombie-open rate for bug entries. Capture is excellent — session
logs, promote-when criteria, bug files, three `doctor` checks shipped today. Installation has
no gate at all: a criterion that fires emits no event, matches no query, and trips no CI
signal. `Status:` is the only field that would make it harvestable, and nothing reads it for
this purpose.

**Severity:** high — this is the mechanism by which every *other* lesson silently fails to
take effect, including the ones that would have caught the seven instances in W-4. A capture
system with no installation step converts effort into archaeology.

**Status:** mitigated — W-4 promoted to all three profiles as *"Measurement — Never State a
Count Your Instrument Did Not Measure"*, the Mutation-apply block synced to the two that
lacked it, and all three files are now byte-identical (`md5 08f0ef6cb534`, 157 lines). The
general gap is unfixed: nothing stops the next fired criterion going the same way.

**Correction, same evening — the title overstates it, and the truth is worse.** Tracing where
the Mutation-apply rule came from turned up
`eduplanner-ui docs/trackers/archive/calendar-insight-panel-session-log-2026-08-18.md`, which
carries a **`Promotion status`** section: every `W-N` checked at archive time against the
target surface itself and marked *already promoted* (text quoted verbatim), *UNFIRED*, or
*FIRED but not yet applied*. Its `W-4` is logged as fired-and-unapplied with the exact text
to promote. **The audit exists and it worked.** "Nothing queries for one" was asserted
without checking the sibling repo — R-3's law, committed while filing a bug about unchecked
claims.

So the defect is narrower and more recursive: **the audit convention itself was never
promoted.** `promotion status` appears in **0 of 9** codescout session logs and **0 times**
in `docs/templates/session-log.md` — the generator they are all copied from, which does
mention `Promote-when` twice. Nine trackers collect criteria with no audit step because the
template never had one. That is [[W-5]] verbatim: fix the generator, the records were obeying
it.

And the failure that actually bit was not a missed firing. The audit caught the firing; the
*application* went to one profile of three, because the audit concluded "not found in the
user's global CLAUDE.md" — **singular** — and nothing re-checked. The remaining gap is
naming every instance of a multi-instance target, not detecting the fired state.

**Fix idea / Pointer:** the generator fix is **done** — `docs/templates/session-log.md` now
carries a `Promotion status` section and a `promotion-due` win status, so the fired state is
representable and queryable rather than only narratable. What remains is a `doctor` check for
fired-but-unharvested criteria, on the pattern
that worked three times today — `terminal_status_with_caveat` made a stated caveat queryable,
`archived_fix_sha_unresolvable` a broken anchor, `terminal_status_without_fix_anchor` a
missing one. Filed as `docs/issues/archive/2026-08-19-no-check-detects-a-fired-unharvested-promote-when.md`.
A profile-divergence check is the cheaper half and may not need `doctor` at all: three files
that should be byte-identical have an md5.

## F-8 — The `git add -A` prohibition existed, was measured, and still did not reach the moment of committing

**Observed:** 2026-08-20, during compaction prep — checking whether the concurrent-session
hazard warranted an entry, and finding the rule already written.

**When:** Twelve commits across one long bug-closing stream, on a checkout shared with at
least two other agent sessions (two commits landed at 22:31 between mine at 22:14 and 22:49;
four untracked bug files appeared later; the catalog reported five linked worktrees).

**Expected — what I did:** `git add -A` for the first four commits. Noticed the hazard
mid-session on spotting commits I had not made, and switched to
`git add <explicit paths> && git commit`.

**Got — what the repo already said**, in `docs/RELEASE.md` § *Concurrent-Work Rules*, live
and detailed:

- *"stage explicit paths, **never `git add -A`**, while another session has uncommitted work
  in the same tree"* — violated 4×.
- *"**Staging explicit paths is not enough — the index is shared too. Commit with a pathspec,
  in one command.** … Use `git commit -- <paths>` instead: a pathspec on `git commit` implies
  `--only`"* — violated 8×.

The second is the sharp one: **the mitigation I congratulated myself on switching to is
itself the documented anti-pattern, one rung up.** `git add` then `git commit` leaves a
window in which a peer's own commit sweeps the shared index.

**Cost this time: zero, and by luck.** Audited all twelve with `git show --name-only` — every
commit contains only my files. The concurrent session happened to have nothing uncommitted at
each of the four `git add -A` moments. What it looks like when the luck runs out is measured
in the rule itself (2026-08-18): seven staged files swept into a peer's commit `62533fee`
whose subject names only their own work, then dropped back into the working tree by their
next `--amend`, leaving the briefly-holding commit unreachable.

**Probable cause:** the rule lives in `docs/RELEASE.md`, which is read when *shipping*, not
when *committing*. Nothing surfaces it at `git commit` time. I grepped that file's headings
once this session and never opened the section — there was no reason to, because committing
is not shipping.

**Severity:** high — silent, misattributing, and no review catches it: the resulting diff
looks intentional, and the commit message describes one of its files.

**Status:** mitigated — behaviour corrected for the remainder of the session, zero harm
confirmed by audit. The general gap is untouched.

**Relation to [[F-7]] — this is its sharper half, and it inverts the diagnosis.** F-7 says
capture works and *installation* doesn't: lessons get written beautifully and never reach a
permanent surface. F-8 is the opposite case and the worse one — the lesson **was** on a
permanent surface, precise, measured, carrying a worked example with SHAs, and it still did
not reach the point of use. So promotion is necessary and **not sufficient**. What is missing
is not a place to write the rule but a **trigger at the moment of the act**, which is what a
hook is for and what a document structurally cannot be.

**Fix idea / Pointer:** codescout-companion already gates `Bash`; a PreToolUse warning on
`git add -A` / bare `git add .` when `git worktree list` shows more than one tree fires at
exactly the right moment. Deliberately **not** filed as a bug — one datapoint, and the
appetite for building checks was spent at n=1 on the promotion audit
(`docs/issues/archive/2026-08-19-no-check-detects-a-fired-unharvested-promote-when.md`).
Recorded here so a second instance has something to join.

## F-9 — Three promoted rules are in force in zero of three profiles, and the session that promoted them is the one observer that cannot see it

**Valid:** dated 2026-08-20

A count of a live state — three promoted rules in force in zero of three profiles — taken
the day the entry was observed. Both halves decay: a version bump or a cache refresh moves
the zero, and the profile count is per-machine. The *lesson* (the promoting session is the
one observer that structurally cannot see the gap) is durable and is why the entry is
cited, but the entry's headline is the measurement, and the class follows the headline.
Declared 2026-09-01.

**Observed:** 2026-08-20, `/codescout-companion:reconnaissance` invoked during compaction
prep. Reading the loaded skill surfaced its own § *Every promotion audits the promoted set*,
which governs edits to that file — a section I had not read before editing it an hour earlier.

**When:** After `claude-plugins:23a11c3` added three Phase 1 bullets promoted from codescout
ledgers (`R-89` build-vs-process freshness, `R-49` re-scout your own bug file, `W-36`
no-default-trait-impl).

**Expected:** the promoted rules are live — the skill body loaded in this very turn contains
all three.

**Got — measured across all three profile caches:**

| copy | Phase 1 bullets | contains the new rule | bytes |
|---|---:|---:|---:|
| `.claude` cache 1.16.10 | 31 | **0** | 31568 |
| `.claude-sdd` cache 1.16.10 | 31 | **0** | 31568 |
| `.claude-kat` cache 1.16.10 | 31 | **0** | 31568 |
| repo source | **34** | **1** | 33823 |

All three profiles are cache-installed at `1.16.10` (each correctly under its own profile
root — the 2026-05-16 cross-profile drift is not present). The commit did not bump
`plugin.json`, so the cache key never changed and nothing re-synced.

**Probable cause:** exactly the mechanism already filed in
`docs/issues/archive/2026-08-17-plugin-content-edit-without-a-version-bump-never-reaches-any-profile.md`:
*"the plugin cache is keyed by the version in `plugin.json` … the change is committed,
reviewed, and in force nowhere — with no error, no warning, and nothing in `git status` to
suggest it."* A second open bug describing the fix, unread at the moment it applied.

**Why the available evidence pointed the wrong way — the part worth keeping.** The skill body
loaded this turn *does* contain the three bullets, because `/reload-plugins` resolved the
skill from the **repo source** (`Base directory: /home/marius/work/claude/claude-plugins/…`)
rather than from a cache. So the session that shipped the change is the **least
representative possible observer** of whether it shipped: it is the only one reading the
write-side copy. Confirming evidence was directly in view and was evidence about the wrong
artifact.

**Severity:** high — silent, and it defeats the promotion entirely. Three lessons with 2–4
datapoints each reach nobody, while every surface says they were promoted.

**Status:** fixed-verified 2026-08-20. The `1.16.11` bump landed from a *concurrent* session
while this one was compacting (`claude-plugins:23ca288`, checklist refresh `dbc8982`); the
re-promotion below then shipped in `1.16.12` (`claude-plugins:a5df5bd` + its release commit,
`NO_PUSH=1`, local only, via `scripts/release.sh`).

**Measured at the served bytes** — the probe R-89's rewritten wording now demands:

| copy | three-axis text | stale text | bytes | install record |
|---|---:|---:|---:|---|
| `.claude` cache 1.16.12 | 1 | 0 | 34497 | `1.16.12`, own profile ✅ |
| `.claude-sdd` cache 1.16.12 | 1 | 0 | 34497 | `1.16.12`, own profile ✅ |
| `.claude-kat` cache 1.16.12 | 1 | 0 | 34497 | `1.16.12`, own profile ✅ |
| repo source | 1 | 0 | 34497 | — |

**Four plausible greens sit above the one measurement that counts**, and that is the
transferable part of this entry. The *commit* says the version bumped — silent about caches.
The *install record* says `1.16.12` — and `bump-cache.sh`'s own header warns that record can
name a directory nobody created. The *directory listing* says `1.16.12/` exists — silent
about contents. `release.sh` prints `✓ … 1.16.12` — its own claim about its own work. Only
the served file's byte count plus phrase match is evidence about the artifact CC will load.
Each of the four reads green in the broken world; three of them read green in the world this
entry was filed about.

**Two smaller gaps found in the same pass**, both conventions the ledger already follows:

1. **No SHA pinned.** Prior promotions record `promoted to SKILL.md (claude-plugins:f842848,
   2026-05-28)`; mine name the file and the verbatim text but not `claude-plugins:23a11c3`
   (`grep -c` → 0). **Closed 2026-08-20** — [[R-89]], [[R-49]] and `bug-fix-session-log:W-36`
   now each carry the commit *and* the version that shipped it, which is the pair that
   actually answers "is it live?"; the SHA alone would not have.
2. **The promoted-set audit was not run or recorded.** The skill requires it *before* adding,
   and `R-93` is the precedent that did it properly, pinning `claude-plugins:c889e83`
   (1.16.4 → 1.16.5).

**And the audit, now run, has a verdict on [[R-89]] itself — rubric row 2, *Outgrown*.**
R-89's promoted wording is *"build freshness and process freshness are two separate facts."*
This incident is a **third** axis — **cache freshness** — on the same law: the artifact was
committed (repo fresh), and no process or build was involved at all. Per the skill's own
rule, *"a recurrence of an already-promoted law is a defect in the promoted text, not a new
entry"*, so the remedy is to re-promote R-89's evolved form covering all three axes rather
than to file this as an unrelated lesson. That re-promotion was itself blocked on the version
bump above — which is the same defect, one turn later.

**Re-promotion shipped** 2026-08-20 in `claude-plugins:a5df5bd` (`1.16.12`). R-89's bullet
now opens: *"Freshness is a property of the copy that SERVES you, and it breaks on three
independent axes — build, process, and distribution. `mtime` answers none of them."* Both
measured datapoints are carried in it (the 88-minute stale process, and this incident), and
it closes on the generalisation the audit produced: **probe the copy the consumer actually
loads; every upstream proxy for it reads green in the broken world.** No fourth bullet was
added — per the skill's own rule, the recurrence was a defect in the promoted text.

## W-14 — `ps | head -N` is not "the newest N", and the truncation rule caught it live on the rule's own subject

**Observed:** 2026-08-20, a `rebuilt/recon` pass immediately after `cargo rb` + `/mcp`, running
the three-axis freshness probe that [[R-89]] had been re-promoted with hours earlier.

**Pattern.** A truncated listing is a floor, never a count — and for `ps` specifically,
`head -N` does not return the newest N. `ps` emits in process-table order, so truncating it
returns N *arbitrary* processes. Sorting was the missing step, not a larger N.

**Counterfactual, and it is concrete.** The first probe ran
`ps -eo pid,lstart,etimes,cmd | grep '[c]odescout' | head -5`. Newest row shown: **01:27:46**.
Binary built **02:25:39**. On that evidence the correct-looking report was *"the serving
process predates the build by 58 minutes"* — which is R-89's exact documented failure, and
would have had the user restart an already-fresh server and distrust a green rebuild.

Re-run without truncation, sorted: **PID 2250592, started 02:38:33, 46 seconds old** — the
`/mcp` reconnect, postdating the build by 13 minutes. Axis 2 was green the whole time. The
truncated list did not merely under-report; it inverted the verdict.

**Confirming data points:**

1. This entry. The catch was made *by* the rule, on the rule's own subject: R-89's process
   axis, probed one hour after R-89 was re-promoted to cover three axes.
2. Same pass, a second predicate defect with no consequence: `grep -c 'codescout start'`
   returned **25**, `pgrep -f 'codescout start --debug'` returned **24**. The extra was an
   `mcpo`-hosted `codescout start --diagnostic` from a *different repo*. Naming what the
   predicate literally matches would have caught it before the number was written down.
3. `docs/trackers/tracker-hygiene-log.md`'s trust table, earlier the same day: `grep -A 14`
   cut the table at D5, so D9/D10 rows were appended that already existed lower down — and
   the real D10 sat at `batch | 2`, which the duplicate visually regressed to `individual | 0`.
   Same rule, same day, three instances, only the third of which was caught before a commit.

**Impact:** high — a wrong freshness verdict is acted on. It sends the user to restart a
healthy process and, worse, teaches distrust of a build that was in fact correct.

**Promote-when:** already promoted — this is a *use* of the Measurement rule (`~/.claude*/CLAUDE.md`
§ Measurement, clause 2) rather than a new lesson. What is new and worth carrying is the
`ps`-specific corollary: **`head` on an unsorted producer selects arbitrarily, so a
"newest/oldest" claim needs an explicit sort, not a bigger N.** Promote that corollary into
the reconnaissance SKILL.md if a second tool shows the same shape (`ls` without `-t`,
`git log` without `--date-order`, a paginated API without an order-by).

**Promoted-to:** not yet — corollary only, held at 1 datapoint.

**Status:** validated — single datapoint, but decisive: the counterfactual is a verdict
inversion, not a missed detail, and the sibling instances in *Confirming data points* show the
parent rule recurring three times in one day.

**Recon outcome, for the record.** All three freshness axes green, verified rather than
assumed:

| Axis | Probe | Result |
|---|---|---|
| build | 4 literals from `4c7608ee` absent at its parent commit, sought in the running binary | present, 1 each |
| process | newest server start vs binary mtime | 02:38:33 vs 02:25:39 — process is 13 min newer |
| distribution | install record + served bytes in all three profile caches | `1.16.14`, 37817 bytes, identical to source |

`~/.cargo/bin/codescout` resolves to `target/release/codescout` in the main checkout, as memory
`gotchas` § MCP Binary Symlink states.

**One anomaly, investigated and closed.** 24 live `codescout start --debug` servers, oldest
16.5 days, looked like a leak. Already filed:
`docs/issues/archive/2026-07-28-mcp-servers-outlive-their-clients.md`, `wontfix`. Rather than
re-file or re-litigate, ran **the file's own stated re-open test** — the discipline [[R-95]]
was promoted for 20 minutes earlier, applied to the first deferral rationale encountered
afterwards. Both arms held: **0 servers with a dead or reparented parent** (23 of 24 rooted in
a live `claude`; the 24th's parent is this session's own codescout server, a legitimate
child), and total RSS **2846 MiB / 128120 MiB = 2.22%**, against the decision's 0.9% baseline.

Worth noting as the honest outcome: **re-costing a deferral does not always overturn it.** This
`wontfix` was measured when written, named the evidence that would falsify it, and survived
that test seven weeks later. R-95's other nine rationales were falsified because none of them
had ever been executed — not because deferrals are generally wrong. The RSS trend (0.9% →
2.22%, 2.5× in seven weeks) is recorded here since nothing else will notice it.

## W-15 — A substring join on `input_json` reads a value as a key — 8 of 8 "instances" were searches for a symbol named `project_id`

**Valid:** dated 2026-09-02

**Observed:** 2026-09-02, full-surface review, joining the live `tools/list` dump against `usage.db` to check whether the activation banner's `project:` instruction had misdirected any `symbols` call.

**Pattern.** A `LIKE '%"project"%'` on `input_json` returned **8** `symbols` calls and read, for about a minute, as eight field instances of a silent-drop defect — a number that would have gone straight into a bug file's Symptom section with a query beside it to make it look derived. Reading the eight rows: every one was `{"name":"project_id",…}` or `{"query":"project_id"}` — searches *for a symbol called* `project_id`, not a `project` param. Real count: **0**. A second join the same hour did the same thing in the other direction: `LIKE '%state_at%'` over `artifact` calls matched **48** rows, 47 of them tracker *bodies* mentioning `workspace_state_at`; the real `state_at` call count was **1**.

Both are the shape `scripts/probe_tool_surface.py`'s docstring already warns about — a query that returns a plausible number rather than an error — but at a level the probe's traps do not name: **a substring match on serialised JSON cannot tell a key from a value.** The remedy that worked was not a cleverer pattern (`'%"project":%'` still matches a *value* of `"project":` inside a body string); it was reading the rows before publishing the count, which cost one `SELECT … LIMIT 10`. Where the join is load-bearing, `json_extract(input_json, '$.project')` is the key-level form and is what the next such query should use.

**Counterfactual.** Without the row read, the `activation-banner` bug file would have opened with *"8 field instances in 30 days"* — a false severity, in a file whose whole point is that a surface says one thing and the tool does another.

**Confirming data points:** the two joins above, same session, opposite directions (false positives on a param key; false positives on an action value).

**Impact:** medium — no wrong number shipped, but it was one paste away, and the query looked like evidence.

**Promote-when:** a third usage.db join is caught reading a value as a key. Then add `json_extract` as the prescribed form to `scripts/probe_tool_surface.py`'s docstring traps and to `docs/PROBES.md`'s usage.db row.

**Status:** open — two datapoints, awaiting the third.

## Template for new entries

<!-- Appends land above this line. Use:
     artifact(action="append_entry", id="<this artifact id>", id_prefix="F"|"W",
              title="...", body="...", anchor_heading="## Template for new entries")
     The server writes a def_re-conformant `## <ID> — <title>` heading. Add the
     matching Index / Wins Index row in the same session. -->
