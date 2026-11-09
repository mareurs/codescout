---
name: reconnaissance
description: Use before subagent dispatch, before editing code that changes a struct, function signature, or API contract, or after a tool response contradicts the plan. Appends friction (F-N) and wins (W-N) to the project's session-log tracker.
---

# /codescout-companion:reconnaissance

A **seam** is a place where your next action depends on the current shape of code you have not read this session: a struct's fields, a function's signature, an API's response, a tool's actual output. Scout the seam before you act. When plan and reality disagree, externalize the gap as a session-log entry with a monotonic ID — IDs make lessons portable across sessions; entries without IDs don't compound.

**REQUIRED SUB-SKILL:** None. See § *Composition with other skills*.

## When to Use

- **Before delegating to a subagent.** Scout the seam yourself first — read the affected symbols, confirm the plan's code matches reality. Subagents inherit drift; the controller absorbs it.
- **Before editing code whose shape you have not verified.** If the change names a struct, function, type method, or API you haven't read this session, read it before editing.
- **After a tool response disagrees with the plan.** Empty results where N were predicted, compile errors on plan code, a wrong field name, an unexpected status — signals that plan is stale or substrate moved.

## When NOT to Use

- **Read-only Q&A that *describes behavior*.** "What does X do?" — answer via `symbols(name=..., include_body=true)`. No scout, no entry. **But asserting a specific, checkable fact is not Q&A** — "it IS BLAKE3", "the field IS named Y", "it's at line N" — especially when the assertion will be presented as a recommendation or written into a doc. Read the symbol this session before you commit the fact. (`codescout:R-19`)
- **Genuinely no-decision edits.** Whitespace, comment typos, version-string bumps that no test asserts on. When in doubt whether an edit is "mechanical," scout — one extra `grep` costs less than one missed invariant. Editing a markdown file that backs an `include_str!`'d constant is NOT mechanical: see Phase 1's `include_str!`'d-constant bullet below.
- **Already in verification phase.** Use `verification-before-completion` for commit gating.
- **Already scouted this seam in the current session.** One pass per seam — re-scouting the same struct/function is noise unless the source has changed since.
- **Underspecified refactor prompts** (`"refactor X for readability"` with no shape contact named). Ask the user whether shape changes; do not scout speculatively.

## Method — Four Phases

```
1. Scout (read actual shape)
   ↓
2. Compare (plan vs reality)
   ↓
3. If gap     → F-N entry  (friction: drift, surprise, cost)
   If win     → W-N entry  (pattern that prevented worse)
   If match   → silent resume (no entry needed)
   ↓
4. Resume task + announce one-line outcome to user
```

### Phase 1 — Scout

For each symbol, type, or contract about to be touched:

- Read the symbol body: `symbols(name=..., include_body=true)`
- Read callers if shape changes: `references(symbol, ...)` or `call_graph(symbol, direction="callers")`
- For tools / external APIs: read the actual response shape, not docs. Run the call once, inspect output.
**Seam-class case law lives in a reference file, not here.** When the seam you are
scouting is one this project has already paid for, read
`references/seam-classes.md` (17 promoted laws — stale substrates, lossy renders,
claims about current state, green-but-uninformative results, empty populations,
migration ordering, instruments that write into what they measure). Use native
`Read`; the path is guard-exempt. Do not `@`-link it — that force-loads.


**Statusline marker (recommended).** Touch `.buddy/$SID/recon-active` once at scout start so the user's statusline shows `[recon]` for 30 minutes. The badge signals scout-in-progress; the user knows not to redirect mid-scout, which prevents abort-and-restart cost:

```bash
SID="${CLAUDE_CODE_SESSION_ID:-$(cat .buddy/.current_session_id 2>/dev/null)}" && \
  [ -n "$SID" ] && mkdir -p ".buddy/$SID" && touch ".buddy/$SID/recon-active"
```

**`$CLAUDE_CODE_SESSION_ID` first — the order is load-bearing**, because the statusline
resolves the sid from the harness while `.current_session_id` is last-writer and can name a
peer. Case law: `references/seam-classes.md`; guard: `tests/test-recon-count.sh` § 7.

Skip silently if the marker dir is unavailable — the skill works without the badge.

### Phase 2 — Compare

State what the plan / docs said vs. what reality holds. **Match** → resume, no entry. **Gap** → Phase 3 (F-N). **Match, but the scout was non-trivial** (multiple files read, hidden contract surfaced, non-obvious shape) → Phase 3 (W-N): a pre-dispatch scout that prevented a subagent slip is a win even though nothing broke.

### Phase 3 — Externalize

Findings go into `docs/trackers/<topic>-session-log.md` in the active project.

**Before you cite a confirming result as evidence, name the proposition it proves — then ask whether a broken world produces the same result.** An uncontended 10/10 is the output a still-racy test also gives. A green suite is the output a never-called guard also gives. A match count measures a text, never a cause. If the check cannot *express* the failure, its result is not evidence about the failure: write down what you actually established, and leave the rest as a thing to check rather than a thing concluded.

This is a check on the sentence you are about to write, not on your choice of tool — which is why it lives here and not in Phase 1. Both recorded failures happened in sessions that had already invoked this skill, while writing up a result (`codescout:R-125`, `codescout:bug-fix-session-log:F-78`).

**Topic naming.** Pick a topic from the work stream, not the seam: `bug-fix`, `auth-refactor`, `jsonpath-impl`, `migration-2026-q2`. One topic = one work stream across sessions. If the right topic file already exists, append; if not, copy the template:

```bash
cp <codescout-repo>/docs/templates/session-log.md \
   docs/trackers/<topic>-session-log.md
```

Resolve `<codescout-repo>` from `claude mcp list` (the codescout server's source path) or ask the user. Do not hardcode a path — installations differ.

**ID + append — one call.** Do not hand-allocate the ID by grepping the tracker, and do not pre-write the Index row before the section exists — both race a peer session, and a pre-written row consumes the id it names (this is why codescout's own `statement-validity-session-log` starts at `F-2`/`W-3`). Let the server allocate and write the section in the same call:

```python
doc(action="append_entry", id="<tracker artifact id>", id_prefix="F",
         anchor_heading="## Template for new entries",
         title="<one-line title>", body="**Observed:** ...",
         index_row="| {id} | <date> | <sev> | <cat> | open | **<title>** — … |",
         index_after_line="<the table line your row follows>")
```

One write: the server allocates the next `F-N` / `W-N` id (separate counters), writes `## F-N — <title>` at the ledger's own level (`link_scan` defines on the `<ID> — <title>` shape as a heading's first inline text, at any depth), records the high-water mark, and stamps `**Valid:** dated <today>` unless the body declares a class. **`dated` takes no trailing text.** Put any qualifier on a following line — `**Valid:** dated 2026-05-18` then a blank line then the prose. An em-dash tail after `dated` is rejected outright (`is not an ISO date`), and the two branches of the grammar differ here: only `conditional — <event>` carries one. `index_row` rides the same `fs::write` with `{id}` replaced by the id just allocated, so section, high-water mark and Index row land together — **this is what "one call" means, and omitting the two parameters silently gives you the two-call form instead.**

**The Index row needs a placement anchor, and one of its failure modes is silent.** A declared `snapshot_anchor` is the safer source — a duplicate is refused by name — while a per-call `index_after_line` wins when passed but takes the **first** trimmed match, so a non-unique line (never the `|---|---|` separator) files your row under the wrong table with no error. A line matching *nothing* is the loud case: refused by name, ledger byte-identical, no id consumed. The refusals, what `entry_prefix` actually locks, and the definition-heading shape are in `references/append-entry-anchoring.md` — read it once before your first entry in an unfamiliar ledger.

`edit_file` is not the append path: a copy of `docs/templates/session-log.md` declares `entry_prefix`, so it is a ledger from the start and `edit_file` refuses it. `append_entry` is the sole **id allocator** (not the sole writer: `doc(action="update", patch={body_edits: […]})` writes sections and prose). See the reference above.

Never reuse an ID. Never skip an ID.

**Severity rubric (F-N).**

| Severity | When |
|---|---|
| `low` | Cosmetic, surfaced-but-not-blocking, future-proofing |
| `med` | Would have caused ≥1 failed tool call, compile error, or 1 subagent retry; controller could absorb |
| `high` | Would have cascaded — multiple subagent retries, wrong code merged, data loss risk, or hidden state change |

If unsure, write `med` and explain the cost in one line. Anchored severity beats free-form severity.

**Status vocabulary.** See the template's `## Status vocabulary` section — `open | mitigated | fixed-verified | wontfix-false-alarm | promoted-to-bug-tracker | pinned-as-eval-baseline` for frictions; `validated | promoted-to-permanent-docs | archived` for wins. Pick the one that matches; the template defines each.

**Count the entry.** Right after the `append_entry` call lands, bump the session counter so the statusline `[recon]` badge shows your scout output as an `F<n>/W<n>` suffix. Use the helper next to this skill (its directory is the "Base directory for this skill" path printed when the skill loaded):

```bash
python3 "<skill-dir>/recon_count.py" bump F 2>/dev/null || true   # friction
python3 "<skill-dir>/recon_count.py" bump W 2>/dev/null || true   # win
```

Best-effort — the `2>/dev/null || true` keeps a counter failure from ever breaking the turn. The counter is session-scoped (resets each CC session) and independent of the tracker's monotonic F-N/W-N IDs.

#### Worked exemplars

Two complete entries — one friction, one win — are in
`references/worked-examples.md`. Read them once, when writing your first entry of
a session; the rubric above is enough after that.

### Phase 4 — Resume + Announce

With verified context, return to the original task. Announce the scout outcome to the user in **one line**, citing the F-N / W-N ID if one was written:

- Match, no entry: `Recon: matched plan, proceeding.`
- F-N written: `Recon: gap captured as F-7 (plan cited .hint field; type has no such field). Proceeding with workaround.`
- W-N written: `Recon: scout prevented Task 2 test-shape slip; captured as W-2.`

Cite the ID in the next subagent dispatch prompt and in the commit message of any change that closes the gap.

## Stop Conditions

Reconnaissance is done when **any one** of:

- The shape question has a one-line answer cited from the code.
- The gap is captured as an F-N entry with an ID.
- The decision is made to revise the plan rather than the code (the plan owns the drift, not the substrate).

Do NOT loop. If the same seam needs scouting again later in the session, the substrate has moved — capture that as a separate F-N entry (`category: architectural` or similar) rather than re-running this flow.

## Common Mistakes

- **Scouting after dispatching.** The subagent has already started; drift now lives in two contexts. Scout BEFORE dispatch.
- **Skipping the counterfactual on W-N.** A win without a counterfactual reads as marketing. Name what would have happened without the pattern, with concrete evidence (round-trips saved, tests that would have failed, files that would have been wrongly edited).

## Composition with other skills

| Trigger | Skill | Reconnaissance role |
|---|---|---|
| Plan dispatched to subagent | `subagent-driven-development` | Scout the seam BEFORE dispatch |
| Plan code looks fictional | `writing-plans` | Externalize as F-N, revise plan inline before any subagent |
| Tool returned unexpected output | `systematic-debugging` | Capture as F-N, then debug from the captured baseline |
| About to claim work complete | `verification-before-completion` | Different timing — reconnaissance is at the seam, verification is at completion |

## The recon-patterns tracker (per project)

Promoting a recurring finding into a durable pattern, and auditing the promoted
set, is a separate workflow with its own routing rules and audit obligation. It is
in `references/patterns-tracker.md`. The four phases above do not depend on it —
read it when you are promoting or auditing, not to run recon.

## Skill maintenance

Trigger-string scoring lives in `<codescout-repo>/docs/evals/reconnaissance-trigger.md`. Re-score before any future description change. **Behavioral eval** (do triggered scouts produce useful F-N entries?) lives at `<codescout-repo>/docs/evals/reconnaissance-output.md` — 14 cases drawn from the R-N ledger's hits and misses, with the six MISS cases (`codescout:R-2`, `codescout:R-4`, `codescout:R-8`, `codescout:R-10`, `codescout:R-19`, `codescout:R-23`) as a hard regression gate. **Baseline RUN** — 2026-09-04, `--paired` n=3, 15 cases: power 1 · tautological 7 · no-effect 7, only C4 clears Δ≥0.5. **Ship gate NOT met** (C2/R-4 at −0.67; C5 has no scenario). Re-score before any change targeting scout *behavior*, measuring the **`--paired` rate delta, never a raw pass-count** (L-15).

Version history is tracked via git on this file; see `git log -- codescout-companion/skills/reconnaissance/SKILL.md`.
