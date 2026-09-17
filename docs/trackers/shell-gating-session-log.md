---
kind: tracker
status: active
title: "Session Log — Shell Gating"
owners: []
tags:
  - shell
  - run_command
entry_prefix:
  - F
  - W
entry_high_water_F: 4
entry_high_water_W: 3
---

# Session Log — Shell Gating (`shell_command_mode` / `run_command` exposure)

> **Purpose:** Two-sided observation log for a multi-session work stream.
> Captures frictions (F-N) and wins (W-N) that the session producing it
> wants to preserve so future sessions inherit the lesson.
>
> **How to use:** Copy this file to `docs/trackers/<topic>-session-log.md`
> in the active project on first reconnaissance pass. Append F-N / W-N
> entries with:
>
> ```
> doc(action="append_entry", id="<artifact id>", id_prefix="F",
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
> **`edit_file` is not the append path**, though it works at first.
> This template ships without frontmatter, so a fresh copy is directly
> editable — but once you declare `entry_prefix` to make the ledger
> guarded (which `get_guide("tracker-conventions")` tells you to do), the
> librarian guard refuses direct edits and only `append_entry` writes.
> Reach for `edit_file` for the prose sections and the index tables,
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
| F-1 | 2026-08-27 | med | companion-hooks | fixed-verified | CLAUDE.md asserts native `Bash` is hard-denied; a positive control shows it is not |
| F-2 | 2026-09-16 | med | companion-hooks | promoted-to-bug-tracker | A gate's refusal echoed the input it actually received; I reconstructed it from memory instead |
| F-3 | 2026-09-17 | low | tracker-navigation | mitigated | A zero from the wrong FILE read as a zero from the corpus; only the commit gate could tell them apart |
| F-4 | 2026-09-17 | low | workspace-state | open | Activation dies with the MCP process; reads warn and writes refuse, and the hint names a worktree first |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-08-27 | high | Grep the config struct for the capability before designing the knob a request implies is missing | Would have re-added the `shell_enabled` switch this repo deleted as redundant, given two knobs for one behaviour, and missed the real gap — `run_command` still advertised in `list_tools` | validated |
| W-2 | 2026-09-16 | med | Pair every confirming read with a member that must read NEGATIVE — and re-pick the control when the old one gains the property | Would have reported "correctly scoped to `refs/heads/experiments`" from a reading an `~ALL`-scoped ruleset produces identically — a true-sounding claim to the user about a setting that would govern every branch | validated |
| W-3 | 2026-09-17 | med | Write the test first so an immediate PASS is itself a signal — a fixture that cannot reach the failing value looks exactly like coverage | Would have shipped a test monotone under the absence of the fix: green pre-fix, green post-fix, credited with pinning the token-vs-deletion choice it could not detect | validated |

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

## F-1 — CLAUDE.md asserts native `Bash` is hard-denied; a positive control shows it is not

**Observed:** 2026-08-27, while reporting the consequences of setting
`security.shell_command_mode = "disabled"` in this project.

**When:** Immediately after the config flip took effect and
`mcp__codescout__run_command` returned `No such tool available`. I needed to tell
the user what capability they had just lost.

**Expected (CLAUDE.md § Companion Plugin):** "native `Read`/`Grep`/`Glob`/`Edit`/`Write`
on source files and all native `Bash` are hard-denied — use codescout's MCP
tools". Read as an absolute, so with `run_command` gone I concluded there was no
shell at all, and reported that the project's mandated pre-commit gate
(`cargo fmt` / `cargo clippy` / `cargo test`) could no longer be run.

**Got (scouted reality):** Native `Bash` is not denied in this session. The user
said so ("I block run_command but I opened bash"), and a positive control
confirmed it: `cat src/main.rs` — which matches `pre-tool-guard.mjs`'s OWN
most-specific block branch at line 165, `/^cat .*\.(rs|ts|...)/` — executed and
returned file contents. Only a PostToolUse advisory fired
(`[cs-hint] Use read_file or find_symbol`). No PreToolUse deny.

Two further facts make "hard-denied" wrong even as a description of the design:

- `pre-tool-guard.mjs:61` sets `BREAKER_THRESHOLD = 3` and stands the guard down
  after 3 consecutive unanswered redirects, explicitly letting the call through
  ("this is advisory context, NOT an auto-approval"). A guard with a documented
  stand-down path is not a hard deny.
- This profile's `~/.claude-sdd/settings.json` carries `permissions.allow` entries
  for `Bash(cat:*)`, `Bash(head:*)`, `Bash(tail:*)`, `Bash(ls:*)`, `Bash(find:*)`,
  `Bash(echo:*)`, `Bash(pwd:*)`, `Bash(which:*)` — a per-profile surface CLAUDE.md
  does not mention.

Why the deny did not fire here is NOT established; the breaker explanation is
ruled out (codescout tools answered between every Bash call, so strikes reset).
Left unresolved deliberately rather than guessed at.

**Probable cause:** CLAUDE.md states a runtime absolute for a mechanism that is
(a) configurable per profile, (b) has a built-in stand-down, and (c) lives in a
separate repo (`../claude-plugins/codescout-companion/`) that drifts
independently. The doc describes intent; a reader uses it to predict runtime.

**Workaround:** Treat the hook inventory in CLAUDE.md as design intent, not as a
prediction. Before asserting a tool is unavailable, call it once — the answer
costs one turn and is authoritative.

**Severity:** med — produced confidently wrong user-facing guidance twice in one
session, including a written recommendation that shell work would need the user
to run `! cargo test` by hand, and a claim that the mandated gate was
unrunnable. The user corrected it in one message; had they believed it, the
plausible cost is abandoning a config change they wanted, or hand-running a
4700-test gate that I could run myself.

**Status:** fixed-verified — the CLAUDE.md sentence was rewritten 2026-08-27 (see
Fix idea below; landed same day). The replacement states what was measured, dates
it, names the profile, and tells the reader to call the tool rather than infer
availability from the file. Gate green after the edit:
`claude_md_contains_no_deprecated_tool_names` + all three `prompt_surfaces`
snapshots + `prompt_surfaces_reference_only_real_tools` = 5 passed / 0 failed,
and `audit_doc_refs` on CLAUDE.md exits 0.

**Residual, deliberately not folded into this entry's status:** *why* the
PreToolUse deny does not fire is still unestablished. That is a separate question
from the friction filed here — the friction was a doc asserting a falsehood, and
the doc no longer asserts it. The scope of the correction also widened on
re-probing: native `Read` and `Edit` reach source files unblocked too (the `Edit`
probe used a deliberately non-matching `old_string`, so it returned "String to
replace not found" rather than a deny, and modified nothing), and `Grep`/`Glob`
turn out to be absent from the tool list entirely rather than denied — a third
mechanism the original sentence collapsed into "hard-denied" along with the other
two.

**Valid:** dated 2026-08-27

True of this profile (`~/.claude-sdd`) and this checkout at commit `c37c7c98`. The
companion plugin lives in a sibling repo and its hook behaviour can change
without any commit here; re-run the `cat src/main.rs` positive control rather
than trusting this entry.

**Rests on:** the positive control, not on the hook source — `pre-tool-guard.mjs`
intends to deny (line 177, `enforce("This call is blocked...")`), so a
read-the-code scout would have concluded the opposite of what the probe showed.

**Fix idea / Pointer:** Hedge the CLAUDE.md sentence — "native Bash is *normally*
redirected to `run_command` by a PreToolUse guard that can stand down
(`BREAKER_THRESHOLD`) and can be relaxed per profile; verify with one call rather
than assuming". Sentence lives in `CLAUDE.md` § Companion Plugin: codescout-companion.

## W-1 — Scout found the requested feature already shipped — prevented re-adding a deliberately deleted switch

**Observed:** 2026-08-27, on the request "lets add a configuration where we can
disable run_command entirely".

**Pattern:** A feature request asserts the feature is absent. That is a claim
about current state — the same class as *"just pin X"* / *"enable X"* in this
skill's Phase 1. Grep the config struct and its consumers for the capability
BEFORE designing the knob, and read what the codebase says about knobs that were
previously removed.

Concretely: `grep("shell_command_mode")` → `SecuritySection` already carried it,
`run_command/inner.rs:349-367` already refused every call on `"disabled"`, and
`run_command/tests.rs` already pinned it with
`shell_command_mode_disabled_blocks_run_command`. It also worked machine-wide via
`GlobalSecuritySection` → `to_toml_value()` → `load_with_global_base`.

**Counterfactual:** Without the scout I would have added a new boolean —
`shell_enabled` or `run_command_enabled` — because that is what the request's
wording implies. Four concrete costs, all avoided:

1. **Re-adding a deliberately deleted switch.** `run_command/tests.rs:1673`
   records "the former shell_enabled master switch was removed as redundant", and
   `src/tools/config/mod.rs:1037` records it being dropped from the activation
   response. The new knob would have re-litigated a settled decision, with
   nothing in the diff to reveal that.
2. **Two knobs for one behaviour**, with no defined precedence between
   `shell_enabled = true` and `shell_command_mode = "disabled"`.
3. **Missing the actual gap.** The real deficit was exposure, not enforcement —
   `run_command` stayed advertised in `list_tools` because `Availability` had no
   shell variant. A new boolean would have satisfied the literal request and left
   the agent still paying the tool's description and schema, still reaching for it.
4. **The trap was live, not hypothetical.** This project's own
   `.codescout/project.toml` still carried a dead `shell_enabled = true` three
   lines from `shell_command_mode`. A scout that read the config FILE rather than
   the config STRUCT would have found "the switch" and wired to a key nothing
   parses. (Removed this session; `grep` confirmed zero `src/` readers first.)

The scout turned a redundant-knob implementation into `Availability::RequiresShell`
+ `ToolCapabilities.shell_enabled` (commit `6058dad6`), which inherited the
existing `notifications/tools/list_changed` path for free.

**Confirming data points:**
1. This session — request premise "we don't have this" was false; the knob had
   shipped, was tested, and worked at two config layers.
2. `shell-gating-session-log:F-1` is the same law's **miss** in its prohibition form, in the
   same session: CLAUDE.md's "native Bash is hard-denied" was also an unverified
   claim about current state, and that one I acted on rather than checking.

**Impact:** high — prevented shipping a config field that the codebase had
already removed once, and redirected the work to the actual gap. The two
datapoints being a hit and a miss of one law, hours apart, is the useful signal.

**Promote-when:** Already promoted — this is the Phase 1 bullet "A proposed fix —
and equally a prohibition — is a claim about CURRENT STATE." Per the skill's
*Every promotion audits the promoted set*, treat this pair as an **audit** rather
than a new law: the text is correct and was reached in the request direction and
not reached in the prohibition direction. That is failure mode 3, **Unreachable**
(placement, not wording) — the law was fetched when the recon skill was invoked
on an implementation request, and not fetched when a CLAUDE.md sentence was taken
at face value mid-report. Candidate remedy is the session-opening surface, which
needs a base arm before it earns a slot.

**Status:** validated — single-session, but the counterfactual is documented in
the codebase itself (two comments recording the prior removal), not inferred.

**Valid:** dated 2026-08-27

The counterfactual rests on comments at `run_command/tests.rs:1673` and
`src/tools/config/mod.rs:1037`; re-verify those still record the removal if
either file is rewritten.

**Rests on:** `shell_command_mode` remaining the single source of truth for shell
gating. If a second shell knob is ever added, this win's reasoning inverts.

## F-2 — A gate's refusal echoed the input it actually received; I reconstructed it from memory instead

**Valid:** dated 2026-09-16

**Observed:** `git-worktree-guard.mjs` refused a read-only `grep` as a "Worktree-ambiguous git
mutation". I formed a mechanism hypothesis — `segments()` splits quote-naively, so
`grep -rn -E "git push|peter-evans"` breaks at the `|` and the left fragment ends in `git push`,
satisfying `TRIGGER`'s `(\s|$)` via `$` — and probed it by driving the hook on its real stdin
contract (`{tool_name, tool_input.command, cwd}`) as a subprocess.

**The probe falsified the story for my own call.** Version `1.20.11` *allows* the command I
believed I had sent, and so do all twelve cached versions `1.20.0` → `1.20.11`. My command began
`cd /home/marius/work/claude/codescout && …`, and `CD_TO_PATH` exempts every later segment — the
guard should never have fired.

**The answer was in the refusal from the first second.** Its `Command:` field began
`echo "=== workflows that push ==="`, with the leading `cd <path> && ` I had typed **absent**. The
harness lifts a leading `cd` into a real working-directory change and removes it from the string
the hook is handed, so the guard's exemption was keyed on a token that no longer existed.

**Two defects, composed; neither alone refuses this command.** The quote-naive split is real and
newly filed (`docs/issues/2026-09-16-worktree-guard-reads-a-quoted-regex-alternation-as-a-bare-git-verb.md`,
`7018ce840e36b942`, `cluster/addressing-without-an-escape-hatch`). The `cd` strip is already filed
and open (`docs/issues/2026-09-13-worktree-guards-cd-chain-remedy-is-stripped-before-the-guard-sees-it.md`,
`cluster/gate-keyed-on-unobservable-event`).

**This is the SECOND instance of the same reader failure, in the same guard.** The `2026-09-13`
file records its own author doing it: *"I spent three calls and one wrong diagnosis … before
reading the `Command:` field that had the answer in it from the first refusal."* Two independent
sessions, same guard, same evidence on screen, same miss. That it recurs against an author who
**documented** the miss is the § *Observer Blindness* result — knowing the class prevented nothing.

**Cost:** one falsified hypothesis and a twelve-version sweep run to answer a question the echo had
already answered. No wrong code shipped, and the sweep was independently useful — it establishes the
alternation defect has never worked rather than being a regression.

**Lesson:** when a gate echoes its input, **that echo is the input**. A probe reconstructed from
your memory of a command tests your memory, not the command — and it returns a clean, confident,
wrong answer, because the reconstruction is well-formed. The tell here was specific and available:
a probe that *contradicts an observed refusal* means the two runs had different inputs, not that
the guard is nondeterministic.

**Rests on:** the `Command:` field being `input.tool_input.command` verbatim; `installPath` in all
three profiles' `installed_plugins.json` naming `1.20.11` (no cross-profile drift); a 6-case probe
with both a positive control (`git push origin experiments` → BLOCKED) and a negative
(`ls -la docs/` → allowed), so the method demonstrably discriminates.

## W-2 — A control that must read NEGATIVE is what separates a scoped rule from a global one

**Valid:** dated 2026-09-16

**Pattern:** Verifying two GitHub rulesets (branch protection on `experiments`, then `master`),
the confirming reading — `GET /rules/branches/experiments` returns the `pull_request` rule — is
**the same reading an `~ALL`-scoped ruleset would produce**. It cannot distinguish *scoped to this
branch* from *applies to every branch*. So each positive reading was paired with a branch that must
read empty: `master` → `[]` while only `experiments` was ruled, and then — once `master` gained its
own ruleset and could no longer serve — `refactoring` → `[]`.

**Counterfactual:** without the negative member I would have reported *"the condition is correctly
scoped to `refs/heads/experiments`"* on evidence that a repo-wide rule satisfies identically. The
failure mode here is not a failed tool call that announces itself — it is a **true-sounding
sentence told to the user** about the blast radius of a setting that would silently govern every
branch in the repo, including `master` and every feature branch, at a moment when the user was
explicitly asking who could push where.

**The re-pick is half the win.** `master` was the control for the first ruleset; protecting it
*consumed* the control. Reusing it would have left a check that reads green because both sides now
have the property — a tautology wearing the shape of a verification. A control is only a control
while it is a member that **cannot** have the property being asserted.

**Second application, same session:** the guard probe carried a positive control
(`git push origin experiments` → BLOCKED) and a negative (`ls -la docs/` → allowed). That pairing
is the only reason a uniform result across twelve plugin versions was readable as a measurement
rather than as a harness that blocks everything or nothing.

**This is a DENOMINATOR, not a catch.** It confirms the law already promoted in the reconnaissance
skill's `references/seam-classes.md` — *"Run a positive control before trusting any of them — one
per state you believe the instrument can report"* — rather than extending it. Recorded because
`CLAUDE.md` § *Testing Discipline* asks that a re-derivation which **confirms** be published as a
confirmation, so the population does not read as self-correcting.

**Rests on:** `GET /repos/{owner}/{repo}/rules/branches/{branch}` returning `[]` for an unruled
branch, observed twice (on `master` pre-protection, on `refactoring` post-); and
`current_user_can_bypass: "always"` read back from the ruleset row rather than inferred from the
create response that set it.

## F-3 — A zero from the wrong FILE read as a zero from the corpus; only the commit gate could tell them apart

**Valid:** dated 2026-09-17

**Observed:** Filing the guard bug, I needed `IC-6`'s `**Members:**` field. I grepped the
roster — `docs/trackers/issue-clusters.md` — for `^\*\*Slug:\*\* \`cluster/addressing-without-an-escape-hatch\``
and got **0 matches**. I read that zero as "this ledger does not carry a per-class Slug
block" and moved on, tagging the bug and committing. The **pre-commit gate** caught it, and
its refusal text says the thing I should have:

> *"Grepping your slug there returns 0, and that zero means WRONG FILE, not `no such class`
> — while a generic `cluster/` grep returns dozens, which is what makes the wrong file look
> like the right one."*

Since the per-class split the field lives in `docs/trackers/issue-clusters/IC-6-addressing-without-an-escape-hatch.md`.
The roster retains a `**Members:**` for exactly ONE slug (`cluster/unclassified`), so the
corpus contains just enough of the shape I was looking for to make the miss invisible.

**Why the zero was convincing, and it was not carelessness.** A bare `cluster/` grep in that
file returns dozens of hits, so every signal said *right file, this class just has no Slug
block*. The instrument answered in its own terms, completely, without complaint — the
`seam-classes.md` Shape case exactly: a query that answers *"what is in this file?"* never
answers *"where does this field live?"*.

**What makes it worth an entry rather than a shrug.** I had **read that law this session**,
in this skill's own `references/seam-classes.md`, roughly an hour earlier. Knowing the class
prevented nothing — § *Observer Blindness*'s result, arriving inside the session that had
just cited it. And the law was **widened by a peer the same morning** (`2b7c70d`, `ea6eb36`,
09:42 and 09:44 +0300) with the expectation-priming half: *"a zero that confirms what you
were just told is the one to distrust — the more reliable the source, the worse it is."* My
copy predated that edit, though the pre-existing Shape case already covered my miss, so the
widening is not what I lacked.

**Cost:** one refused commit, no wrong data — the gate held. Cheap here *because* a
mechanism caught it; there is nothing in the reading path that would have.

**Lesson, and it is about the mechanism rather than the care:** when a grep for a FIELD
returns zero in a file that is clearly about the right SUBJECT, the next question is "does
this field live here?", not "does this value exist?". The corpus made the wrong file look
right; only the gate distinguished them.

**Rests on:** the gate's own refusal text naming the per-class path; `2b7c70d` / `ea6eb36`
timestamps read from `git log --date=iso`; the served `SKILL.md` verified byte-identical to
the repo and unchanged since 2026-09-15, so only `references/seam-classes.md` was stale in
my context.

## F-4 — Activation dies with the MCP process; reads warn and writes refuse, and the hint names a worktree first

**Valid:** dated 2026-09-17

**Observed:** Workspace activation is scoped to the **MCP server process**, and a `/mcp`
reconnect starts a fresh one. Nothing announces the loss. The asymmetry is the finding:

| path | behaviour when unactivated, with worktrees present |
|---|---|
| reads (`read_file`, `grep`, `doc(get)`) | **succeed**, with a `⚠` workspace notice prepended |
| writes (`doc(create)`, `append_entry`, `edit_file`) | **hard refuse**: *"Write blocked: git worktrees detected but workspace(action='activate') has not been called"* |

So the warning rides along on ~20 successful calls and is easy to read as ambient noise,
and enforcement arrives only at the first write — typically mid-task, with the payload
already composed. Observed twice this session: once in a fresh session that had simply
never activated, and once **after a `/mcp` reconnect silently dropped a prior activation**.
The third time I pre-empted it by activating before the first write, which is the only
reason there were two and not three.

**The remedy text names the wrong target first.** The refusal hint reads:

> `Call workspace(action='activate', path="…/codescout.worktrees/check-codescout-integration")`
> `to select the write target (or use "…/codescout" for the main repo).`

The **first-named** option is a worktree; the main repo is the parenthetical. This checkout
carries nine worktrees, eight of them short-lived `mutation-*` probe trees, and the session
that hits this is overwhelmingly working in the main repo. A reader following the hint
literally pins the wrong tree and their writes land somewhere they did not mean — quietly,
because activation then succeeds. This is § *Testing Discipline*'s remedy-text law: the
predicate is right (writes really are ambiguous here), and nothing tests where the refusal
sends you. The first worktree in the enumeration appears to be whichever the walker yields
first, so the hint is not even naming a stable target.

**Cost:** two refused writes, both recoverable, no data lost. Low per occurrence — but it
recurs on every reconnect, and the reconnect is exactly when a session is least primed to
expect it, having just watched the server come back healthy.

**Why it is filed rather than absorbed:** the fix shape is cheap and belongs in the
mechanism, not in anyone's memory — either carry activation across a reconnect, or name the
main repo first in the hint and the worktrees as the alternative. "Remember to re-activate
after `/mcp`" is a policy the model must notice, which `skill-frictions:SKF-22` already
records as not a mechanism.

**Rests on:** the two refusal payloads quoted verbatim from this session; the read path
observed succeeding with the `⚠` notice on many calls in between; the worktree count read
from the refusal's own enumeration.

## W-3 — A test that passes on its first run is a fixture that cannot reach the failing value

**Valid:** dated 2026-09-17

**Pattern:** Fixing the quoted-alternation defect, I added a fourth test pinning a
*consequence* of the chosen implementation. `stripQuoted` replaces each quoted span with an
inert TOKEN rather than deleting it, because `git -C "<path>" commit` and `cd "<path>"` need
the path to survive as a single `\S+` word. That is a design choice a later "simplification"
could undo, so it wanted a test. I wrote:

```
cd '$MAIN/.worktrees/feature' && git commit -m x     → expect allowed
```

It **passed immediately**, which this skill and the TDD skill both read the same way: you
are testing existing behaviour. The fixture path had no space, so `\S+` already matched the
whole quoted token, quotes and all — the case passed *before* the fix and therefore said
nothing about it. Replacing the fixture with a genuinely spaced path (`'/tmp/some dir/with
space'`) produced the RED, and the suite went 40/2 → 40/3.

**The counterfactual is not "no test" — it is a test that looks like coverage.** Work out
what the vacuous version could actually detect:

| world | vacuous fixture (no space) | corrected fixture (space) |
|---|---|---|
| pre-fix | **passes** — proves nothing | **reds** ✓ |
| tokenise (shipped) | passes | passes |
| deletion refactor | reds ✓ | reds ✓ |

So the vacuous fixture was **monotone under the absence of the fix**: the one world it could
not distinguish is the world the test exists for. It would still have caught a later
deletion-refactor, and that is exactly what makes it dangerous rather than merely useless —
it works often enough to be trusted, and its green gets credited to a property it never
tested. § *Testing Discipline*'s "demand an observed RED, never an assertion's existence",
met by a fixture that could not reach the failing value.

**What actually caught it was the ORDER, not attention.** Written after the fix — the normal
way, and the way it would have been written had I not been running red-first — it passes on
the first run and nothing distinguishes that pass from a correct one. The pass *is* the
signal, and only if you were expecting a failure.

**Second-order note worth keeping:** the pre-existing suite had three tests asserting *"a
quoted mention must not DISARM the guard"* and none asserting *"must not TRIGGER it"* — one
direction of one mention problem, by authors who demonstrably knew the problem existed. The
bug I was fixing lived in the untested direction. Monotone-direction blindness at the level
of a whole suite, not a single assertion.

**Rests on:** the observed 40/2 → 40/3 → 43/0 progression; the table above derived by
reasoning about `CD_TO_PATH`'s `\S+` against each implementation, with the pre-fix and
post-fix rows both observed rather than argued.

## Template for new entries

<!-- New F-N / W-N entries land above this line. This heading is the anchor:

     doc(action="append_entry", id="<artifact id>", id_prefix="F",
              anchor_heading="## Template for new entries",
              title="<one-line title>", body="**Observed:** ...")

     The server allocates the id, writes `## F-N — <title>` at the ledger's
     own level, records the high-water mark and stamps `**Valid:** dated
     <today>` — one write. Then add the Index / Wins Index row with the id
     it returned. Do not hand-allocate; do not pre-write the row. -->
