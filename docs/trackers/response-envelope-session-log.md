---
kind: tracker
status: active
title: Session Log — Response Envelope Redesign
tags: ["envelope", "progressive-disclosure", "context-cost"]
entry_prefix: ["F", "W"]
entry_high_water_F: 5
---

# Session Log — Response Envelope Redesign

> **Purpose:** Two-sided observation log for a multi-session work stream.
> Captures frictions (F-N) and wins (W-N) that the session producing it
> wants to preserve so future sessions inherit the lesson.
>
> **How to use:** Copy this file to `docs/trackers/<topic>-session-log.md`
> in the active project on first reconnaissance pass. Append F-N / W-N
> entries with:
>
> ```
> artifact(action="append_entry", id="<artifact id>", id_prefix="F",
>          anchor_heading="## Template for new entries",
>          title="<one-line title>", body="**Observed:** ...")
> ```
>
> One call, one write: the server allocates the next id, formats the
> heading as `## F-N — <title>` (the only shape `link_scan` accepts as a
> definition), records the ledger's high-water mark, and stamps
> `**Valid:** dated <today>` unless your body declares a class. **Then**
> add the Index / Wins Index row, using the id the call returned — the
> indexes are the eval surface, the sections are the evidence.
>
> **Do not hand-allocate ids, and do not pre-write index rows.** A max-id
> is a fact about an instant, and a peer session in the same checkout can
> take the number between your scan and your write. Pre-written rows are
> worse: the allocator counts an id claimed by an index row, so rows
> written ahead of their sections consume the ids they name — which is why
> codescout's `statement-validity-session-log` starts at `statement-validity-session-log:F-2`/`statement-validity-session-log:W-3`
> rather than `statement-validity-session-log:F-1`/`statement-validity-session-log:W-1` (see `statement-validity-session-log:F-3` there).
>
> **`edit_markdown` is not the append path**, though it works at first.
> This template ships without frontmatter, so a fresh copy is directly
> editable — but once you declare `entry_prefix` to make the ledger
> guarded (which `get_guide("tracker-conventions")` tells you to do), the
> librarian guard refuses direct edits and only `append_entry` writes.
> Reach for `edit_markdown` for the prose sections and the index tables,
> never for allocating an entry.
>
> **Lifecycle:**
> - Created at the start of a multi-session work stream.
> - Appended-to across every session that touches the work.
> - Entries with `Status: open` carry forward across sessions.
> - Promotion to permanent surfaces (CLAUDE.md, ADRs, formal bug
>   trackers) happens when the entry's `Promote-when` / `Fix idea`
>   criteria fire.
> - File archived (moved to `docs/trackers/archive/`) when the work
>   stream wraps.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-09-01 | high | architectural | fixed-verified | Change 3's premise was false — the overflow gate already exists in `call_content`, and the proposed gate would have been permanently false |
| F-2 | 2026-09-01 | high | architectural | fixed-verified | Plan's commit step used a bare `git commit`, which commits the whole index — a check-then-act guard on the wrong side of its own gap |
| F-3 | 2026-09-01 | med | architectural | fixed-verified | The fix for F-2 was invalid git syntax — `-m` after `--` is read as a pathspec |
| F-4 | 2026-09-01 | high | architectural | fixed-verified | Review package scoped by recorded BASE collected 13 peer commits — a range is a proxy for authorship and stops being one on a shared checkout |
| F-5 | 2026-09-01 | med | architectural | fixed-verified | A contended `git add` failed silently inside a `;`-joined command, leaving a stale staged copy under a green gate |
## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-<n> | YYYY-MM-DD | low/med/high | <pattern> | <what-would-have-happened> | open |

---

## Promotion status

**Audited:** <YYYY-MM-DD>, against the target surface itself — opened and read,
not recalled.

One line per `W-N` (and any `F-N` with a `Fix idea` bound for a permanent
surface). Check the **target**, not the entry: a `Promote-when` that fired is
invisible from inside the tracker, because `Status: validated` reads as healthy
either way. Record one of:

- **already promoted, no action** — quote the promoted text verbatim and name
  where it landed, so the next reader verifies instead of re-deriving.
- **UNFIRED, carried forward** — restate the criterion and the current datapoint
  count.
- **FIRED but not yet applied** — the one that leaks. Name the exact target
  surface and the exact text to add. This is an action item, not a note; set the
  entry's `Status:` to `promotion-due` so a query can find it.

> ⚠️ **Name every instance of the target, not the target's type.** This machine
> runs three Claude Code profiles (`~/.claude`, `~/.claude-sdd`,
> `~/.claude-kat`), each with its own `CLAUDE.md`. An audit that concluded
> *"not found in the user's global CLAUDE.md"* — singular — led to a promotion
> that reached one file of three on 2026-08-18. The session that found the gap
> was running on a profile **without** the rule, and applied it only because
> another profile's copy happened to be injected as project instructions. Three
> files that should be byte-identical have an md5; compare them.

> ⚠️ **For an INSTALLED artifact the target is the SERVING copy — not the repo
> source, and not the other copies.** Measured 2026-08-20: three rules promoted
> into a plugin skill were byte-identical across all three profile caches *and*
> stale against source, because the commit never bumped the version the cache is
> keyed on. Comparing the copies to each other reads **green** there — only
> comparing each copy to the claim catches it. And the session that made the edit
> is the **least representative observer**: its own reload resolved the skill from
> the repo source, so the confirming evidence sitting in front of it was evidence
> about the wrong artifact.

> ⚠️ **Anchor on a back-citation, not a verbatim quote.** A quote goes red when the
> promoted rule is legitimately reworded — a false positive produced by the
> promotion working as intended, observed 2026-08-20 when `codescout:R-89`'s bullet was
> rewritten and the tracker's stored quote had to be edited to match. The durable
> form is the promoted text citing its own entry id —
> *"(codescout:R-1 + codescout:R-7 in codescout's `docs/trackers/reconnaissance-patterns.md`.)"* — so
> verification is a `grep` for the id and survives every rewording. Keep the quote
> as a reading aid; do not make it the predicate.

Run this when the work stream wraps, **and** whenever a criterion fires
mid-stream — an audit that only happens at archive time is one that happens
after the lesson was needed. Prior art:
`eduplanner-ui:docs/trackers/archive/calendar-insight-panel-session-log-2026-08-18.md`, whose
audit correctly caught its own `calendar-insight-panel-session-log-2026-08-18:W-4` as fired-and-unapplied and named the exact
text to promote.

## Category conventions

Use a short kebab-case category to group similar frictions. Prior
sessions have used:

| Category | When to use |
|---|---|
| `codescout-tool` | Friction in a codescout MCP tool (`grep`, `read_file`, `edit_markdown`, etc.) |
| `subagent` | Subagent produced unexpected output or diverged from instructions |
| `plan-prose` | Plan document had drift vs reality (wrong file paths, fictional code, mismatched counts) |
| `architectural` | Discovered structural property of the system that the plan / docs didn't surface |
| `self-friction` | Predicted a friction that turned out to be a false alarm — recorded for transparency |
| `<language>-<library>` | Language- / library-specific footgun (`rust-serde`, `python-typing`) |
| `release-pipeline` | Deployment-time gap (release binary missing, MCP reload needed, etc.) |

Add a new category by writing it as a kebab-case string; no central registry needed.

---

## F-N entry template

Pass this block as `append_entry`'s `body` (without the `## F-N — <title>`
line — the server writes the heading from `title`). Add the matching Index
row afterwards, using the id the call returned. Do not allocate the id
yourself; see *How to use* above.

```markdown
## F-N — <one-line title>

**Observed:** <date, session task>

**When:** <what you were trying to do>

**Expected:** <what plan / docs / prior session said>

**Got:** <actual observed reality>

**Probable cause:** <one sentence>

**Workaround:** <what you did to proceed>

**Severity:** low | med | high

**Status:** open | wontfix-false-alarm | fixed-verified | mitigated | promoted-to-bug-tracker | pinned-as-eval-baseline

**Valid:** invariant | dated YYYY-MM-DD | conditional — <the event that ends it>

**Rests on:** <one durable sentence — an ADR, a decision, or the principle this
instantiates>

**Fix idea / Pointer:** <issue # in formal tracker, plan task ID, or "TBD">

---
```

## W-N entry template

Pass this block as `append_entry`'s `body`, with `id_prefix="W"` — F-N and
W-N have separate counters. A win without a **Counterfactual** is marketing
— name what would have happened without the pattern, with at least one
piece of evidence.

```markdown
## W-N — <one-line title>

**Observed:** <date, session task>

**Pattern:** <the practice that worked>

**Counterfactual:** <what would have happened without the pattern, with evidence>

**Confirming data points:** <list of session moments validating the pattern; aim for ≥2>

**Impact:** low | med | high

**Promote-when:** <criterion for graduating into permanent docs (CLAUDE.md, ADR, etc.)>

**Promoted-to:** <surface + section, one per line, line-start — omit until it lands>

**Status:** validated | promotion-due | promoted-to-permanent-docs | archived

**Valid:** invariant | dated YYYY-MM-DD | conditional — <the event that ends it>

**Rests on:** <one durable sentence — an ADR, a decision, or the principle this
instantiates>

---
```

---

## Status vocabulary

Codified so the Index column means the same thing across sessions.

### Friction statuses

| Status | Meaning |
|---|---|
| `open` | Observed, not yet resolved. Default for new entries. |
| `wontfix-false-alarm` | Initial observation was wrong; documented for transparency rather than deleted. |
| `mitigated` | Workaround in place; root cause not fully resolved. |
| `fixed-verified` | Code / process fix landed AND empirically confirmed. (`fixed` alone is too weak — verification is part of the status.) |
| `promoted-to-bug-tracker` | Moved to a formal tracker (`docs/issues/*`, `docs/TODO-*`, GitHub issue). The session log keeps the pointer; the formal tracker owns the lifecycle. |
| `pinned-as-eval-baseline` | Kept verbatim as a reference point for measuring later improvements. Do NOT close — its job is to remain comparable. |

### Win statuses

| Status | Meaning |
|---|---|
| `validated` | Pattern confirmed by ≥1 counterfactual data point. Default for entries with evidence. |
| `promotion-due` | `Promote-when` has **fired** and the text is not yet on the target surface. An action item, not a resting state. Exists because `validated` cannot distinguish "criterion not yet met" from "criterion met, nobody harvested it" — and both read as healthy, which is how a lesson sits unpromoted while the failure it describes recurs. |
| `promoted-to-permanent-docs` | Moved into CLAUDE.md, an ADR, a skill, or another permanent surface. Session log keeps the pointer — and, for a multi-instance target, names every instance it landed in. |
| `archived` | Pattern no longer load-bearing — either the underlying system changed or the discipline became automatic. |

---

## F-1 — Change 3's premise was false — the overflow gate already exists in call_content, and the proposed gate would have been permanently false

**Observed:** 2026-09-01, pre-dispatch reconnaissance for Task 3 of
`docs/superpowers/plans/2026-09-01-request-aware-response-envelopes.md`. Task 1 was
already running; Task 3 was next in the queue and had not been dispatched.

**When:** Scouting the seam before dispatch. The specific question: my gate reads
`result.get("output_id")`, but `output_id` is added by the *buffering* layer — is
`relevant_guide_topic` called with the pre- or post-buffering value?

**Expected (spec + plan):** `src/tools/markdown/read_markdown.rs:488` returns
`Some("progressive-disclosure")` unconditionally with `_result` ignored, therefore the
overflow guide ships on reads that did not overflow. Remedy: gate it inside the method on
`result.get("output_id").is_some()`.

**Got (scouted reality) — the premise is FALSE and the remedy would have regressed.**
Two independent findings, either one fatal:

1. **The gate already exists, centrally.** `src/tools/core/types.rs:1271-1279`
   special-cases this exact topic inside `call_content`:

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

   `candidates` is seeded from `relevant_guide_topic`'s return at `:1255`
   (`vec![content_topic]`, where `content_topic` came from the call at `:1195`). So an
   unconditional `Some(...)` is already filtered downstream and the guide does **not**
   ship on a non-overflowing read. There was no defect to fix.

2. **The proposed gate would have been permanently false.** `let mut val =
   self.call(input, ctx).await?` at `:851`; `relevant_guide_topic(&val)` at `:1195`;
   `"output_id": ref_id` first inserted at `:1385`, inside the *buffered* branch. So
   `output_id` reaches the raw result only for tools that pre-buffer themselves
   (`run_command` storing a `@cmd_*`). `read_markdown` does not. The gate would have
   evaluated `None` on every call, including overflowing ones — permanently disabling the
   guide for that tool.

**Probable cause:** the spec asserted a defect from reading ONE layer — the method's own
body — and never asked what the caller does with its return. This is the recon skill's
own *"a proposed fix is a claim about CURRENT STATE — verify it before designing around
it"* law, in its absence form: I asserted a capability was missing (the gate) when it was
present. The skill flags that form as the one that reaches durable records, and it had
already reached the spec, the plan, and a peer conversation.

**Workaround:** Task 3 dropped entirely; Change 3 withdrawn from the spec. Nothing to
implement — the behaviour is already correct.

**Severity:** high — would have cascaded. The implementer would have transcribed the gate,
and **both plan tests would have passed**: each asserts `relevant_guide_topic` in isolation
with a hand-built fixture that supplies `output_id` directly, so neither exercises the
`call_content` path where the key does not exist. Green suite, passing review, silent
regression. The paired-twin discipline did not help — both twins share the fixture's false
premise, which is the recon skill's *"a green result certifies the path that actually
EXECUTED"* combined with a fixture unreachable-by-construction from production.

**Status:** fixed-verified — Task 3 dropped and spec amended before any subagent ran.

**Valid:** dated 2026-09-01

True of `call_content`'s ordering at `b769277b`. Re-verify if the guide-delivery block in
`src/tools/core/types.rs` moves — codescout-17 has uncommitted work in that file.

**Rests on:** `call_content` consulting `relevant_guide_topic` on the pre-buffering `val`,
and the `progressive-disclosure` special-case in the `should` match remaining the single
place overflow-gating happens.

**Fix idea / Pointer:** Plan Task 3 (dropped), spec § Change 3 (withdrawn), controller
Ruling 5. Sibling lessons from the same afternoon, both about a verification that answers
a narrower question than the one asked: `reconnaissance-patterns:R-156` (a refusal's
remedy borrows the refusal's authority) and `reconnaissance-patterns:R-159` (a mechanism
verified *when entered* borrows the verification's authority without anyone asking whether
it is entered). This entry is the third: a method's body verified without asking what its
caller does with the return.

## F-2 — Plan's commit step used a bare `git commit`, which commits the whole index — a check-then-act guard on the wrong side of its own gap

**Observed:** 2026-09-01, mid-SDD-execution. Reported by peer session codescout-17 while
my Task 1 implementer was running and approaching its commit step.

**When:** After dispatch, before the implementer committed. Found by a peer's measurement,
not by my own scout — worth recording, because my pre-dispatch scan did not look at the
commit command at all.

**Expected (plan):** Task 1 Step 8 read `git add <path>` then `git commit -m "..."`, and
my dispatch prompt told the implementer to run `git status --short` before committing and
confirm it was staging only its own files. I believed that covered peer-capture on a
shared checkout.

**Got (reality):** It does not. A bare `git commit` commits the **entire index**, not the
paths just staged. So any file a peer stages between my implementer's `git status` check
and its `git commit` lands in my commit, under my message. **The check sat on the wrong
side of the gap it was meant to close.**

Peer measurement, same day: staging **2** paths and listing seconds later showed **6**
staged files — 4 belonging to another session, staged in the interval. This checkout has
roughly ten live agent sessions.

**Probable cause:** I wrote the guard against the *capture* mechanism (`git add -A` /
`git add .`) and not against the *window*. `git add <pathspec>` is genuinely safe; the
unsafety is entirely in the commit verb, and the two look like one precaution. The
pre-flight scan compared tasks against each other and against the files they touch — it
never treated the commit command as a seam, because it reads as boilerplate.

**Workaround (applied):** `git commit -- <paths> -m "..."`, which commits only those paths
regardless of index state, followed by `git show --stat HEAD` verified **after** the
commit. Both written into the plan's Global Constraints and Task 1 Step 8, with the peer's
2-vs-6 measurement carried as the reason rather than a bare rule. The running implementer
was messaged directly. It is also now explicit that a commit which *does* capture a peer's
file must **not** be `reset` or `amend`ed — on a shared tree the repair destroys work the
defect only mislabels.

**Severity:** high — would have cascaded, and silently. A swept-in peer file produces a
commit that passes every gate (the peer's work is usually fine), is attributed to the
wrong session, and is discovered later by someone reading history. The `git show --stat`
step is the only thing that surfaces it at the time.

**Status:** fixed-verified — plan amended and the live implementer redirected before its
commit step.

**Valid:** dated 2026-09-01

The 2-vs-6 figure is a peer's single measurement on this checkout at this session count,
not a rate. The mechanism is invariant; the frequency is not.

**Rests on:** `git commit` with no pathspec committing the whole index — a git property,
not a codescout one — and on this checkout being shared by concurrent writers.

**Fix idea / Pointer:** Plan Global Constraints + Task 1 Step 8. Generalises past SDD:
**any** dispatch instruction that says "check X, then act" on shared state has this shape,
and the fix is always to make the action itself scoped rather than to move the check
closer. Sibling: `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md`.

**Corrected 2026-09-01 — the remedy is safer, not safe, and this entry overstated it.**
Read `scripts/pre-commit-unreviewed-content.sh`'s header: *"`git commit -- <paths>` does
not commit the index. It commits the WORKING TREE at those paths. On a shared checkout
that means it also commits whatever a concurrent session wrote to the same file between
your edit and your commit."* So the pathspec form trades one capture window for a
narrower one — peer edits to **the same paths** — rather than closing capture. It is still
the right remedy for what this entry is about (a bare commit sweeping *unrelated* staged
files), and that hook exists precisely to narrow what remains. Its own header says it
*"narrows the window, it does not close it — only a per-session worktree does that."*

## F-3 — The fix for F-2 was invalid git syntax — `-m` after `--` is read as a pathspec

**Observed:** 2026-09-01, Task 1 closeout. The implementer reported it as a concern; I
verified it in a throwaway repo rather than accepting the report.

**When:** After `F-2` — the fix I wrote *for* `F-2`, and the message I sent the running
implementer, both carried the invalid form.

**Expected:** `git commit -- <paths> -m "msg"` commits only those paths.

**Got:** Exit **1**. `error: pathspec '-m' did not match any file(s) known to git` and
`error: pathspec 'test' did not match any file(s) known to git`. After `--`, git reads
**everything** as a pathspec, so `-m` and the message become filenames. Verified in a
scratch repo: `git commit -m "msg" -- foo.rs` → exit 0; `git commit -- foo.rs -m "msg"` →
exit 1.

**Probable cause:** I wrote a command I never ran. Every command in a plan is a claim
about behaviour, and the ones in a *fix* are the least tested of all — written under the
pressure of a defect just found, when the author's attention is on the mechanism they now
understand rather than on the syntax of the remedy. `F-2` was about `git commit`'s
*semantics*; nothing in that discovery prompted a check of its *grammar*.

**Workaround:** `git commit -m "msg" -- <paths>`. Plan's Global Constraints and Task 1
Step 8 both corrected, with the two exit codes recorded so the next reader does not have
to re-derive them.

**Severity:** med — the implementer hit it immediately and corrected it, so the real cost
was one implementer detour. It would have been **high** if the invalid form had appeared
somewhere that fails silently instead of at exit 1; a command that errors loudly on first
use is the good case.

**Status:** fixed-verified — corrected in the plan; the implementer's own commit
(`f3a76f81`) verified to touch exactly its two files.

**Valid:** invariant

`--` terminating option parsing is git's documented behaviour, not a version artefact.

**Rests on:** git's pathspec separator semantics.

**Fix idea / Pointer:** The transferable rule is narrower than "test your commands": **a
remedy written in the same breath as the defect it fixes inherits none of that defect's
scrutiny.** `F-2` was verified against a peer's measurement; its fix was verified against
nothing. Pairs with `reconnaissance-patterns:R-161` — the fix wore the appearance of the
finding's rigour.

## F-4 — Review package scoped by recorded BASE collected 13 peer commits — a range is a proxy for authorship and stops being one on a shared checkout

**Observed:** 2026-09-01, generating the Task 1 review package.

**When:** Between the implementer reporting DONE and dispatching the reviewer.

**Expected (skill guidance):** `superpowers:subagent-driven-development` says to run
`scripts/review-package PLAN_FILE BASE HEAD` where *"BASE is the commit you recorded
before dispatching the implementer — **never `HEAD~1`**, which silently drops all but the
last commit of a multi-commit task."*

**Got:** That produced **14 commits, 195,067 bytes**. Exactly **one** was mine. The other
13 were peer sessions' work, landed during Task 1's 30-minute run. The reviewer would have
been handed thirteen other sessions' commits as the diff to judge Task 1 by.

**Probable cause:** The skill's rule is correct on a **solo** checkout, where the only
commits between BASE and HEAD are the implementer's. On a shared checkout it silently
over-collects. Both failure modes are real and they point opposite ways — `HEAD~1`
under-collects a multi-commit task, recorded-BASE over-collects concurrent peers — so
neither is the rule. **The correct scope is neither a range endpoint but an
identity: the implementer's own commits.**

**Workaround:** codescout stamps a `Session-Id:` trailer on commits, so the population is
positively identifiable rather than inferred:

```
git log --format='%H' <base>..<head> --grep='Session-Id: <my-session-id>' | wc -l
```

Returned `1`, naming `f3a76f81`. Review package regenerated over `f3a76f81^..f3a76f81` —
1 commit, 12,645 bytes. Recorded in the plan's Global Constraints.

**Severity:** high — would have cascaded, and quietly in the expensive direction. An Opus
reviewer handed 195 KB of mostly-foreign diff either flags a pile of "findings" against
work Task 1 never touched (each one costing a fix round against an innocent commit), or
dilutes its attention across 14 commits and misses the one it was dispatched for. Neither
failure announces itself; both read as a review that ran.

**Status:** fixed-verified — scope corrected before the reviewer was dispatched.

**Valid:** dated 2026-09-01

Depends on this repo stamping `Session-Id:` trailers. A repo without them needs a
different positive identifier; the reported SHAs from the implementer's own report are the
fallback.

**Rests on:** commits carrying a session identity, so the population can be *identified*
rather than inferred from a range.

**Fix idea / Pointer:** Worth upstreaming to the SDD skill — its BASE guidance names one
failure mode (`HEAD~1` under-collecting) and is silent on its mirror image. The general
form: **a range is a proxy for authorship, and it stops being one the moment anyone else
commits.** Same shape as this project's standing rule against closing an authorship
question by elimination — the remedy is a positive identifier, and here the harness
already publishes one.

## F-5 — A contended `git add` failed silently inside a `;`-joined command, leaving a stale staged copy under a green gate

**Observed:** 2026-09-01, re-staging a bug file after editing it post-`git add`.

**When:** Immediately after amending a file that was already staged, on a checkout shared
with ~10 live agent sessions.

**Expected:** `git add -- <path>; cargo test --test issue_clusters` re-stages, then gates.

**Got:** The `git add` **failed** — `fatal: Unable to create
'.git/index.lock': File exists. Another git process seems to be running in this
repository` — a peer held the index lock. Because the compound used `;`, execution
continued. The gate then reported **16/16 green**, and the failure appeared only in
`stderr`, which is not where anyone looks after a green.

The staged copy was therefore the **pre-edit** version. Nothing in the success path said
so.

**Probable cause:** two independent things, and it needed both.

1. `git add` is a *write to shared state* and can fail transiently on a contended index.
   On a solo checkout this essentially never happens, so it reads as infallible.
2. The gate could not detect it: `every_index_count_matches_the_corpus` reads the corpus
   via `git ls-files`, and **the stale and current versions of the file carry the same
   cluster tag** — so the count was correct either way. The gate was green *and right*,
   about a different question than the one I needed answered.

**Workaround (applied):** a bounded retry on the `git add`, plus a **post-condition**
check that does not depend on the exit code:

```
git diff -- <path> | wc -l      # MUST be 0 — staged copy == worktree copy
```

Zero on the retry, `git status --short` showing `A `.

**Severity:** med — **downgraded from `high` 2026-09-01 after a peer measured the claim the `high` rested on, and it was false.**

The original justification was that a peer "would have committed the stale copy in good faith." Verified: they would not have. `scripts/pre-commit-unreviewed-content.sh` compares the blob being committed against the blob in the real index for every pathspec commit and refuses on any difference. So that path was already guarded. What remains genuinely exposed is a **bare `git commit`** (takes the index, stale bytes and all, with nothing comparing them to anything) and **pure verification** — a peer reporting "staged and uncommitted" as a health check, which is what actually happened.

Recording the downgrade rather than quietly editing the number: a severity is a claim, and this one was inflated by an unchecked assumption about a hook I had not read.

**Status:** fixed-verified — re-staged, post-condition confirmed 0.

**Valid:** dated 2026-09-01

The lock contention is a property of this checkout's session count, not of git. The
silent-continuation half is invariant for `;`-joined commands.

**Rests on:** `git add` writing `.git/index.lock`, and `;` not propagating failure.

**Fix idea / Pointer:** The rule is **verify the post-condition, not the exit code** —
and note what makes this one nastier than `F-2`. There the action was *wrong*; here the
action was *right and did not happen*. `F-2`'s remedy (scope the action) does nothing for
it. Same family as `reconnaissance-patterns:R-161`: the failed `git add` wore the
appearance of a completed one, and the green gate three lines later reinforced it.

**Corrected 2026-09-01 — two claims in the first version of this entry were wrong, both
measured by a peer and re-verified here.**

1. This entry originally said `git status` "shows the file staged, not whether the staged
   bytes are current." **False.** The second column carries exactly that: `A ` after a
   clean add, `AM` once the worktree diverges. Measured in a throwaway repo. So the signal
   **exists** and is not read — which is a different defect with a different remedy. A
   signal nobody parses is not an absent signal: the first wants a habit change (read the
   second column as a field, not the line as a presence check), the second an instrument
   change. `git diff -- <path> | wc -l` is still the better post-condition, but for
   **legibility**, not because status is silent.

2. **The transferable rule is `;` versus `&&`, not `index.lock`.** The lock is merely how
   it fired here. Any staging or writing step joined with `;` continues past a failure
   whose only trace is stderr, and a green check afterwards reads as confirmation. That is
   a two-character fix and it generalises to every write-then-verify pair; `index.lock` is
   one instance and would have sent a reader looking for a git problem.

## Template for new entries

<!-- New F-N / W-N entries land above this line. This heading is the anchor:

     artifact(action="append_entry", id="<artifact id>", id_prefix="F",
              anchor_heading="## Template for new entries",
              title="<one-line title>", body="**Observed:** ...")

     The server allocates the id, writes `## F-N — <title>` at the ledger's
     own level, records the high-water mark and stamps `**Valid:** dated
     <today>` — one write. Then add the Index / Wins Index row with the id
     it returned. Do not hand-allocate; do not pre-write the row. -->
