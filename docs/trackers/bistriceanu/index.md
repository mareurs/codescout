---
id: f370d00812848704
kind: tracker
status: active
title: External bug reports from Bistriceanu (B-N) — codescout defects found on an outside install
tags:
- external-report
- codescout-defects
- progressive-disclosure
- librarian
- prompt-surfaces
topic: external-bug-reports
---

An external codescout user — a friend of the maintainer, running codescout on macOS
against his own projects — has been reporting defects since he installed it on
2026-04-15. This tracker is the durable, **scrubbed** record of what he found.

It is deliberately *not* a session log. Every observation comes from his machine and
his checkout, so each entry carries a verification status against **our** `experiments`
tree rather than inheriting his confidence. Two of thirteen are confirmed here; most
are untested; two look environment-specific. Treating the whole list as a bug backlog
would be wrong — treating it as a *lead* list is the point.

**Prefix:** `B-N` — work-stream-scoped per `docs/TAXONOMY.md` § *Work-stream-specific
prefixes*, not a project-wide taxonomy slot. Entries that survive verification get
promoted out of here: a bug file in `docs/issues/` when reproducible, `U-N` when it is
a usage friction, `H-N` when it is a hook proposal, `T-N` when it is tool-selection.

## Provenance, and why `raw/` is gitignored

Source: `WhatsApp Chat with Bistriceanu.zip`, exported 2026-08-15, spanning
2023-10-28 → 2026-08-10 (235 messages; the technical span starts 2026-04-15). It
carried four attachments: a full Claude Code session transcript, an implementation
plan for his own project, his `Full_Read_Fidelity_design.md`, and four screenshots.

The export sits in `raw/` and is **gitignored** — see the rule and its rationale at
`.gitignore:81`. This is the single declared exception to the "`docs/trackers/` is
privacy-scrubbed by construction" claim made by the `/scratch/` rule directly above
it, and that claim was amended in the same commit so it does not read as false.

The reason is not squeamishness. codescout is a **public** repo, and the payload
carries three things that are not ours to publish: a two-year personal chat with a
named third party; the reporter's proprietary trading-system plan plus a session
transcript quoting his source tree; and a security assessment of a *third* party's
platform that names an unremediated tenant-isolation weakness, its file paths, and a
deploy poller that reports stalled rollouts as green. None of that is needed to act on
anything below. **Do not un-ignore `raw/`, and do not quote machine paths, client
names, or the third-party findings into any tracked file.**

His design doc is the one attachment that is purely about codescout, so it is
committed beside this file as [`full-read-fidelity-design.md`](full-read-fidelity-design.md),
verbatim but for home-path rewriting, with a provenance banner warning that its line
numbers belong to `experiments @ d7988aca` and not to this checkout.

## Index

Verification pass run 2026-08-15 at `821f9d0d`. **Seven filed, one closed as already
fixed, five still unverified.** Filing an unverified outside report as a bug file was
deliberately avoided — the ledger is for defects we have seen, and B-4 is exactly why.

| ID | Finding | Surface | Status here | Task |
|---|---|---|---|---|
| B-1 | `force=true` is silently discarded on whole-file reads | `read_file` | **reproduced** | [`…-read-file-force-ignored-on-full-reads`](../../issues/2026-08-15-read-file-force-ignored-on-full-reads.md) |
| B-2 | Buffered full-read summary carries no incompleteness signal | `read_file` | **fixed** `16a6b561` | [`…-read-file-buffered-summary-has-no-incompleteness-signal`](../../issues/archive/2026-08-15-read-file-buffered-summary-has-no-incompleteness-signal.md) |
| B-3 | `truncate_compact` cuts from the tail — where the overflow line lives | core/types | **fixed** `bb2a9625` | [`…-truncate-compact-tail-cut-destroys-overflow-signal`](../../issues/archive/2026-08-15-truncate-compact-tail-cut-destroys-overflow-signal.md) |
| B-4 | `format_semantic_search` discards its own `truncated` flag | `semantic_search` | **already fixed** — no task | — |
| B-5 | `grep` prints a self-refuting `Showing 400 of 400` | `grep` | **reproduced** | [`…-grep-showing-n-of-n-when-collection-hit-cap`](../../issues/2026-08-15-grep-showing-n-of-n-when-collection-hit-cap.md) |
| B-6 | Read-only metadata blocked on source paths | `run_command` | **reproduced**, his mechanism wrong | [`…-read-only-metadata-commands-blocked-on-source-paths`](../../issues/2026-08-15-read-only-metadata-commands-blocked-on-source-paths.md) |
| B-7 | Librarian catalog silently empty — `count: 0`, `exit_code: 0`, no error | librarian | not reproduced here | — (needs his `abs_path: ""` explained) |
| B-8 | Stale `~/.cargo/bin/codescout` wins PATH and no-ops two hooks | install | not reproduced; memory drift real | — |
| B-9 | `get_guide("iron-laws-detail")` asserts a `cat` that the gate refuses | guides | **FIXED `43fac6c8` + regression test; archived** | [`…-iron-laws-detail-guide-claims-cat-on-source-is-allowed`](../../issues/archive/2026-08-15-iron-laws-detail-guide-claims-cat-on-source-is-allowed.md) |
| B-10 | `server_instructions` reached the model truncated mid-word | prompt surface | **reproduced in this repo's own sessions** | [`…-server-instructions-truncated-before-reaching-the-model`](../../issues/2026-08-15-server-instructions-truncated-before-reaching-the-model.md) |
| B-11 | IL3 hook predicate ~75% false-positive | companion hook | unverified | — |
| B-12 | Fresh subagents reach for native Bash `grep` before the MCP tool | dispatch | observed, 2/4 | — (candidate `T-N`) |
| B-13 | Companion hook fails closed when the MCP server is not connected | companion hook | unverified | — |

---
---

## B-1 — `force=true` is silently discarded on whole-file reads

**Reported:** 2026-08-10, and written up as D1 in his design doc.

**Mechanism.** `read_file` passes `force` only to `read_with_line_range`. The
whole-file path never receives it and buffers unconditionally.

**Status here: reproduced on `experiments`, 2026-08-15.** `read_full_file`
(`src/tools/read_file.rs:643-752`) takes `(path, text, resolved, input, source_tag,
ctx)` — no `force` parameter exists to discard. Its `if exceeds_inline_limit(text)`
branch returns early, *before* `OutputGuard::from_input(input)` is reached at the
bottom of the function, so neither `force` nor `detail_level` can influence it. For a
source file over 10 KB there is no parameter of any kind that returns content inline.

**Nuance he raised himself, and it is fair:** the schema documents `force` as "skip
source-symbol hint and read the raw line range", so whole-file arguably was never in
scope. That makes it a design gap rather than a coding error — but the user-visible
effect is identical, and it is the effect that matters: an agent that passes
`force=true` believes it overrode the default and did not.

**Next:** decide whether `force` should mean force on this path, or whether the
schema should say plainly that it is range-only. Either resolves it; silence does not.

## B-2 — the buffered full-read summary carries no incompleteness signal

**Reported:** 2026-08-10, D2 in his design doc. He called this the defect that
"produced a wrong deliverable" — the one codescout failure he did not attribute to
himself.

**Mechanism.** The buffered branch returns `{type, line_count, symbols}` plus
`file_id`. No `complete: false`, no shown-vs-total, no hint. The renderer prints
`line_count` as a bare header — `"1505 lines"` — which reads as a property of the
answer rather than a warning that 1,505 lines were withheld.

**Status here: reproduced.** In `read_full_file`, the overflow branch sets only
`result["file_id"]` (plus markdown `coverage`). Thirteen lines below, the
*under-budget* branch builds a full `OverflowInfo { shown, total, hint }` with a
tailored hint. The same function does it right for the milder case and wrong for the
worse one — which is what makes this a local inconsistency rather than a design
philosophy, and what makes it cheap to fix.

**His key enabling discovery (unverified here):** `format_read_file_summary` already
renders a `hint` field when one is present, so the fix needs population at the source
only — no renderer change.

**Next:** highest-value item on the list. Populate `complete: false` + counts + hint
in the overflow branch.

## B-3 — `truncate_compact` cuts from the tail, where the overflow line lives

**Reported:** 2026-08-10. He rated this *"far worse than I told you — and it's
systemic"*, one root cause behind four surfaces.

**Status here: REPRODUCED on the `symbols` surface, 2026-08-15.** Filed as
[`docs/issues/archive/2026-08-15-truncate-compact-tail-cut-destroys-overflow-signal.md`](../../issues/archive/2026-08-15-truncate-compact-tail-cut-destroys-overflow-signal.md).

**Fixed 2026-08-16 (`bb2a9625`), and the report was right about more than it claimed.**
Nine call sites across five surfaces tail-appended the overflow line; the cutter itself
was left alone, since a tail cut is correct for prose and the defect was producer-side
ordering. Three further signals turned out to be riding the same cut, none of them in the
report: `grep`'s completeness warning, `tree`'s depth-cap note, and `symbols`' `[lsp
warming]` marker. B-2 (`16a6b561`) depended on this landing first — its new hint would
otherwise have been eaten by the very defect B-3 describes.

Both halves verified from source:

- **The cut is from the tail.** `truncate_compact` (`src/tools/core/types.rs:322-336`)
  returns `&text[..nl_pos]` and appends `… (truncated)`.
- **The signal is at the tail.** `references(truncate_compact)` returns exactly **one**
  production caller — `src/tools/core/types.rs:641`, the `call_content` overflow path,
  which cuts `self.format_compact(&val)` output verbatim. And
  `src/tools/symbol/display.rs:225-229` appends the overflow note *and* a warning after
  the rendered rows, so both are the first casualties.

**The finding that reframes it — the fix already exists on one surface.**
`format_semantic_search` (`src/tools/semantic/semantic_search.rs:783-897`) carries a
comment naming this exact mechanism and builds its state fields *ahead* of the rows to
survive the cut, with a regression test
(`format_semantic_search_keeps_state_fields_above_the_truncation_cap`). So this is not
an unknown bug — it is a **known bug whose remedy was never generalised**. Every other
`format_compact` is unaudited against it.

That also settles the layer question: the cutter is fine, the producers' ordering is
not. Fixing `truncate_compact` would be the wrong fix.

**Not reproduced here:** his runtime 149-vs-273 count discrepancy on `symbols`.
## B-4 — `format_semantic_search` discards its own `truncated` flag — ALREADY FIXED

**Reported:** 2026-08-10, in his list of four unfiled bugs.

**Status here: does not reproduce — the code already handles it. No task filed.**

`search_response` (`src/tools/semantic/semantic_search.rs:751-776`) emits both
`truncated` and, when set, `truncated_hint`. `format_semantic_search` (`:783-897`)
renders `truncated_hint` as a `Note:` line — and deliberately places it *above* the
result rows so it survives the B-3 tail cut. Four regression tests cover it, including
`search_response_flags_truncated_only_at_the_limit` and
`format_semantic_search_keeps_state_fields_above_the_truncation_cap`.

**Why this entry earns its place even though it is closed.** It is the reason the other
twelve entries were verified before filing rather than filed on report. Had his list
been transcribed straight into `docs/issues/`, this would have become an open bug
against code that already carries the fix *and the explanatory comment* — and the next
person to work it would have burned a session discovering that.

It also raises a real question for B-7/B-8/B-11/B-13: his checkout may lag ours. Verify
before filing, always.
## B-5 — `grep` prints a self-refuting "Showing 400 of 400"

**Reported:** 2026-08-10, in his list of four unfiled bugs.

**Status here: REPRODUCED from source, 2026-08-15.** Filed as
[`docs/issues/2026-08-15-grep-showing-n-of-n-when-collection-hit-cap.md`](../../issues/2026-08-15-grep-showing-n-of-n-when-collection-hit-cap.md).

The mechanism is a conflation of two different truncations. `src/tools/grep.rs:324`:

```rust
let (visible, total, files) = cap_grouped(matches, budget);
let truncated = hit_cap || total > visible.len();
```

`hit_cap` (set at `:208`, `:254`, `:273`, during the walk) means **collection** stopped
early; `total > visible.len()` means **display** was capped. But `cap_grouped`
(`src/tools/file_group.rs:50-109`) computes `total = items.len()` and returns the items
unchanged when `budget >= total` — so in the collection-capped case `total` *is* the
cap and `visible.len() == total`, producing `Showing N of N`.

The sharper point is not the contradictory string: after `hit_cap` the true total is
**unknown**, because the walk stopped. Reporting `N of N` claims completeness for a
number nobody has.

The identical expression is repeated in `grep_in_buffer` at `:775` — any fix must cover
both.
## B-6 — read-only metadata is blocked on source paths (his mechanism was wrong)

**Reported:** D3 in his design doc, flagged by him as minor.

**Status here: behaviour REPRODUCED, diagnosis REFUTED, 2026-08-15.** Filed as
[`docs/issues/2026-08-15-read-only-metadata-commands-blocked-on-source-paths.md`](../../issues/2026-08-15-read-only-metadata-commands-blocked-on-source-paths.md).

He concluded `wc` was "in the block list" and left open whether that was deliberate.
**There is no command list.** `src/tools/run_command/inner.rs:305-315` calls
`check_source_file_access(resolved_command)` — a *path* predicate. Any command naming a
source file is refused regardless of which binary it invokes or how bounded its output
is. Confirmed live this session: `ls -1 <source paths> && sed -n '322p' <source>` was
refused, and both `ls` and `sed` sit on the guide's own bounded-LHS allowlist.

**What led him wrong is B-9** — the guide asserts a bounded-file carve-out that does not
exist. So the two entries are one story: a wrong guide produced a wrong mechanism in an
otherwise careful investigation, and the wrong mechanism reached his design doc.

The residual question is a design decision, not a defect: carve out read-only metadata
(`wc`/`ls`/`stat`), or keep the blanket block and fix the guide. **The guide fix is
required either way.**
## B-7 — the librarian catalog was silently empty

**Reported:** 2026-08-10. He filed it on his side as F-40 (high) and rated it above
his entire D1/D2 fix in severity.

**What he saw.** `artifact(find, kind="tracker")` → `count: 0` against 57 files in
`docs/trackers/`; a `bug-fix-session-log` path filter → 0; `librarian(action="doctor")`
→ `violations: [], total: 0, hidden_rows: 0` — empty, *not* drifted. Every scope block
returned `abs_path: ""` and `git_root: ""`, matching `project_root: ""` from
`workspace(activate)`. The semantic index was healthy (`up_to_date`, 1,350 files,
39,014 chunks), so this was the librarian catalog specifically, not indexing generally.

**Why he ranked it so high, and he is right about the shape:** the failure is
`count: 0` with `exit_code: 0` and no error. Against 233 files in `docs/issues/`,
*"catalog has no rows"* and *"no open bugs"* are the same response. Every triage pass
reads a healthy-looking empty ledger. He caught it only because he scouted the query
path before filing into it — had he filed first, the bug files would have been
correct, correctly located, and undiscoverable.

**Status here: not reproduced.** Our catalog answers normally — the same query shape
returned 3 open/investigating bugs this session with a populated `scope` block
(`abs_path` and `git_root` both set). So this is environment- or setup-specific on his
install, and the empty `abs_path`/`git_root` is the thread to pull. He never
root-caused it; he ran `reindex`, which *disproved his own first hypothesis* — the
catalog stayed empty while every query worked.

**Next:** the durable lesson survives even if his instance was misconfigured — a
read path that degrades to a confident zero with no affordance is more dangerous than
truncation, which at least hands back an `@ref` saying "there's more here". He noted
this is the same class as a prior finding of his that `artifact(get)` silently
truncates large trackers, and set a promote-when at a third datapoint.

## B-8 — a stale `~/.cargo/bin/codescout` won PATH and no-opped two hooks

**Reported:** 2026-08-10. On his machine `~/.cargo/bin/codescout` was **not a
symlink** — a real file, ~4 months stale, winning PATH. `codescout constitution-check`
returned *unrecognized subcommand* while `target/release` ran it fine, so
`constitution-guard.mjs` and `goal-stop-hook.mjs` were fail-open no-ops with no signal.

**Status: not reproduced here — but the doc drift is real.** Our memory `gotchas`
documents this path *as a symlink* (CLAUDE.md points at it as "the binary symlink
gotcha"). On his install it was not one. Whether by divergent install method or by
drift, the memory states as fact something an outside install contradicts.

**Next:** the hooks failing **open and silently** is the transferable defect. A guard
that cannot run should say so, not pass.

## B-9 — `get_guide("iron-laws-detail")` asserts a `cat` the gate refuses

**Reported:** 2026-08-10, found by one of his subagents. The guide states
`cat src/foo.rs` is allowed on bounded files; he cited `path_security.rs:825` as
flatly refusing it.

**Status here: REPRODUCED, 2026-08-15 — confirmed by execution, not by reading.**
The live `get_guide("iron-laws-detail")` § *Iron Law 3* still contains:

> **Read-mode for source code is blocked.** `cat src/foo.rs` is allowed on bounded
> files but the broader "shell on source" pattern is intercepted with a hint to
> route through `symbols`.

That sentence contradicts itself inside its own bolded lead — the heading says
blocked, the body says allowed. And behaviour sides with the heading: a
`run_command` of `ls -1 <source paths> … && sed -n '322p' src/tools/core/types.rs`
was refused this session with `shell access to source files is blocked`, hinting at
`read_file` / `symbols`. `sed` is on the guide's own bounded-LHS allowlist one
paragraph earlier, so bounded-ness is not what decides it — the source-path
predicate is, exactly as he said.

**Why this one matters more than its size suggests.** He made the point himself and
it is the sharpest observation in the report: this is the exact gate he got wrong in
B-6, and he never read the guide — but **had he read it, it would have confirmed his
error.** A guide that is wrong is worse than a guide that is missing, because it
converts a checkable doubt into a false certainty. This is the same failure shape as
B-2 and B-3 one layer up: a surface answering confidently about the thing it got wrong.

**Next:** fix the sentence — the bounded-file carve-out does not exist for source
paths. Cheap, and it retires half of B-6's ambiguity: the question left there is only
whether `wc` *should* be carved out, not whether it currently is.
## B-10 — `server_instructions` reached the model truncated mid-word

**Reported:** 2026-08-10. His session's `server_instructions` ended
`- "iron-… [truncated]`; he never received the `iron-laws-detail` or
`symbol-navigation` pointers, nor the Project Status block, and measured the slice at
2,203 bytes against a real ~2 KB client cut.

**Status here: REPRODUCED — in this repo's own sessions, on Linux.** Filed as
[`docs/issues/2026-08-15-server-instructions-truncated-before-reaching-the-model.md`](../../issues/2026-08-15-server-instructions-truncated-before-reaching-the-model.md).

The session that wrote this tracker received the surface cut at the identical point:

```
- "workspace-state"         — activate, home/foreign, reset
- "iron-… [truncated]
```

Same cut point, different OS, different checkout, different machine. This is not his
misconfiguration — it is the shipped default. `MAX_INSTRUCTIONS_CHARS = 2200`
(`src/prompts/mod.rs:1193`), whose own comment says the cap "gives ~200 bytes" of
headroom against the channel limit; the cap is set **at** the cliff, not below it.

**Why it outranks its severity band.** The truncated tail is the `get_guide` topic
list — the mechanism by which a model discovers deeper guidance exists. Losing it is
silent and self-concealing: you cannot call a topic you were never told about. It also
interacts with B-9 — most sessions never reach the pointer to the guide that, when
reached, misleads.

His own line is the exact summary: *the instruction telling the model not to trust
truncated output was itself truncated before it arrived.*
## B-11 — the IL3 hook predicate is ~75% false-positive

**Reported:** 2026-08-10, logged on his side as U-34 and explicitly claimed as his own
rather than codescout's. The IL3 advisory (unbounded-pipe warning) fires on commands
that are in fact bounded. **Status: unverified here.** Note this session tripped the
same advisory on `ls … | head -40`, which is bounded-LHS and permitted — one datapoint
consistent with his rate.

## B-12 — fresh subagents reach for native Bash `grep` before the MCP tool

**Observed:** 2026-08-10, across his five blind needle-searcher agents. Two of four
opened with native Bash `grep` on source and were hard-denied before pivoting to
`mcp__codescout__grep`.

His read is worth keeping: *"that's a substrate signal, not a memory one"* — the agents
had no prior session to have learned from, so this is about what the briefing and the
tool surface make salient at spawn time, not about forgetting. Same Iron-Law class he
had logged twice for himself.

**Next:** maps onto Iron Law 6 (brief your subagents). Candidate `T-N` promotion.

## B-13 — the companion hook fails closed when the MCP server is not connected

**Reported:** 2026-07-14. A hook blocked every shell command and redirected him to
`run_command`, while the codescout MCP server was not connected that session — so its
tools had never loaded and there was no working way to run anything. Deadlock.

He resolved it in 28 minutes by reconnecting codescout. **The root cause was never
addressed:** the hook assumes codescout is available, and when it is not, the fallback
is nothing at all. Being the *first* thing he hit after a build, on his own, makes it
a first-run-experience defect rather than a curiosity.

---

## Agent behavior — analyzed separately

The codescout defects above are half the material. The other half — why the agent
behaved the way it did across his sessions (assert-at-hypothesis, wrong-environment
verification, protective framing, structure-as-rigor, session amnesia), why his venting
didn't fix it and his 08-10 protocol did, and what to promote into our surfaces — is
analyzed in **[`agent-behavior-analysis.md`](agent-behavior-analysis.md)** (AB-1…AB-7),
from the transcript, the four screenshots, and the agent's own post-mortems. Key
cross-repo result: his three-month launchd misdiagnosis is an independent confirmation
of R-86 (recorded as an `external_signal` event on `reconnaissance-patterns`), and the
one intervention with a measured positive effect — claims must carry CERTAIN/UNCERTAIN
plus justification — is the promotion candidate for our guide surfaces.
## Measurements worth keeping (not defects)

These came out of his 2026-08-10 session and are useful independent of any fix.

**The needle harness — his own theory, refuted.** He proposed testing "you are bad at
finding a needle in a haystack": one randomly-drawn needle (external `/dev/urandom`,
not chosen), five blind searchers, byte-identical prompts, answer key sealed before
dispatch, scoring pre-registered so an off-by-one counted as a miss.

| Metric | Result |
|---|---|
| Exact hits (name + value + `path:line`) | **5 / 5** |
| Line-number drift | 0 — every one said `:17` |
| Calibration | 5/5 said CERTAIN, 5/5 were right |
| Mean tool calls | 5.8, ~30s each |
| Double-verified the line independently | 5 / 5 |

The method deserves copying: he pre-registered exclusions before drawing, disclosed a
confound noticed *after* the draw and **kept the draw anyway** — re-rolling after
seeing the needle being exactly the post-hoc selection the experiment existed to catch.

**The intervention that worked.** He told the searchers to *"end with CERTAIN or
UNCERTAIN, and why"*. All five double-verified through independent means, unprompted.
His argument for why this beats a hook: it attaches the check to the **act of
asserting** — the one moment the failure is guaranteed to be present — rather than
depending on someone first suspecting something is wrong.

**Correction-trigger census, same session, 10 corrections:**

| Trigger | Count |
|---|---|
| A user prompt | 4 |
| Running something | 6 |
| **Unaided self-review** | **0** |

Zero. His conclusion: re-reading your own text produces no new signal, because it was
generated from the same belief that would have to be the thing under suspicion.
Revision needs a discrepancy, and a discrepancy needs new information.

**Why recon structurally could not have caught it.** It audits the seam you nominate,
and a wrong belief you do not know is wrong never gets nominated. Its own timing rule
disqualifies late invocation as post-mortem. Directly relevant to `R-N` in
`reconnaissance-patterns.md` — this is an outside datapoint for a limit we log
internally.

**Overflow rates from his `usage.db`** (6,621 calls, his own project): 4.9% overall —
`symbols` 11.8%, `run_command` 6.5%, `grep` 4.3%, `read_file` **0.8%**. Useful
calibration: the path with the worst *consequence* (B-2, `read_file` full-read) is the
rarest, and the most frequent (`symbols`) returns a legitimately navigational summary.

## What he judged was *not* codescout's fault

Recorded because it bounds the list above, and because he volunteered it unprompted
after a session spent hunting codescout defects:

> "One codescout defect produced a wrong deliverable. One degraded my inputs. The rest
> of my errors were failures to look, and codescout was not in the way."

He also corrected his own standing instruction, which asserted that codescout "replaces
any tool result over 2,500 tokens with a summary and formats that summary to look
complete." True for exactly one path — `read_file`'s full-read branch, i.e. B-2 — and
false for the others he checked: `tree` announces its cap explicitly, `run_command`
carries shown/total, and `read_file`'s own under-budget branch carries a hint. His note
on the cost of that over-generalization is worth keeping: **it teaches distrust of
tools that are behaving correctly, which burns calls on redundant verification.**

Separately, and predating the technical work: from 2026-08-03 he reported that Claude
writes worse code than Codex, loses the thread as complexity grows, misfires at
planning without heavy steering, and buries correct answers in structure until he has
to extract them over several turns. Model-level, not codescout, and logged here only
so the tracker reflects what he actually said rather than the convenient subset.

## Next steps

Updated 2026-08-16 — three fixes shipped, closing the loop this tracker opened.

**Shipped:**

1. **B-9 fixed** — `43fac6c8`, regression test added, bug file archived (see B-9 entry above).
2. **The maintainer's "Conclude Last" iron rule amended** on all three local CC profiles
   (`~/.claude`, `~/.claude-kat`, `~/.claude-sdd` CLAUDE.md) with the clause the eval
   proved closes its loophole — see `agent-behavior-analysis.md` § Tier 1b.
3. **codescout's own `project-activation-bootstrap` guide** now leads Phase 2 with an
   unconditional ALWAYS-VERIFY imperative — `5917e37e`. This is hard-injected once per
   session on first `workspace(activate)`, in its own uncapped content block (confirmed
   at `src/tools/core/types.rs` `guide_block()` — no truncation, unlike the B-10
   `server_instructions` slice). The exact shipped string was re-tested as its own eval
   arm before commit (100% plausibility-verified, n=35) — re-N=0 discipline, not
   assumed transfer from the ablation wording.

   **Deployment status — committed and compiled, not yet observed live.** Corrected
   2026-08-16: the first report of this fix called it shipped, which overstated it.
   Guide bodies are `include_str!`'d (`src/prompts/mod.rs:160`), so the text a session
   receives is fixed at build time and frozen again at process start. Verified since:
   the string *is* in the current binary (`grep -c "Do not hypothesise"
   target/release/codescout` → `1`), but the session that shipped it runs a server
   started 09:28, before the 10:56 build that first compiled it — and that session's
   own auto-injected bootstrap guide arrived without the paragraph. `/mcp` reconnect
   makes it live; re-probe the injected guide text after reconnecting. Logged as the
   fourth instance of R-89 in `docs/trackers/reconnaissance-patterns.md`, which fired
   that entry's Promote-when.

All three trace to the reporter's own two sentences — measured the best-performing
verification-guidance artifact in an 11-arm eval built to answer exactly this
question. Findings + the full ablation are in `agent-behavior-analysis.md`; a
scrubbed writeup for the reporter is published as a private Claude artifact —
private until shared, and access-controlled by account rather than by secrecy of
the link, so recording it here is safe even in this public repo:
https://claude.ai/code/artifact/72bb553a-56da-479e-ad19-e451edc8c43f
(also saved as `~/Downloads/Always-Verify.pdf` on the maintainer's machine, for
sending directly).

**Still open, by leverage:**

1. **B-3** — audit every `format_compact` for tail-placed state fields (9 call sites
   enumerated in its bug file). The remedy is already written and tested in
   `format_semantic_search`; this is generalising a known fix.
2. **B-2** after B-3, so its new hint is head-placed and does not get eaten.
3. **B-5**, **B-10**, then **B-1**/**B-6**, which are decisions more than code.
4. **Verify the remaining five** — B-7, B-8, B-11, B-12, B-13. None are filed, on
   purpose — B-4 was the precedent (reported in good faith, already fixed in our
   tree; filing on report would have opened a bug against code that already carries
   the fix).
5. **Do not chase B-7 as a bug here** until his empty `abs_path`/`git_root` is
   explained; the *silent-zero shape* is the real, transferable finding regardless.
6. **Promote what generalises.** B-12 is a candidate `T-N`; B-11/B-13 belong to the
   companion plugin, not this repo.

**Not yet committed to git** (deliberately — awaiting explicit go-ahead, per the
privacy decision in § Provenance): this whole `docs/trackers/bistriceanu/` folder,
the six still-open bug files above, and the `.gitignore` rule that protects `raw/`.
All are on disk and safe; nothing here depends on git history to survive.

**Close the loop with the reporter.** B-4 is fixed, B-6 rested on a wrong mechanism
(now corrected), B-9 is fixed, and B-10 reproduces here too — his instance was never
the outlier he assumed. The published writeup covers all four plus the eval.
## Template for new entries

```
## B-N — <title>

**Reported:** <date>, <how>.
**Mechanism:** <what he observed, in his terms>
**Status here:** reproduced | unverified | not reproduced — <evidence, with our path:line>
**Next:** <the single check or decision that moves it>
```
