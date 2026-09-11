---
kind: bug
status: fixed
tags:
- cluster/guard-narrower-than-its-name
- run_command
- il3
- shell-gate
claimed_at: 2026-09-11
claimed_by: ec98641c-d5ba-456d-8e4a-10e24d52da16
closed: 2026-09-11
opened: 2026-09-01
owner: marius
related: []
severity: low
---

# BUG: the source-file shell gate evaluates the whole command string, so one offending clause refuses every unrelated clause beside it — and the refusal does not say which one

## Summary

`run_command`'s shell gates refuse the entire command when **any** part of it offends. For a
`;`- or `&&`-separated compound command — N independent commands, not a pipeline — that
discards N−1 clauses that were never in scope. **Both** gates share this scope mismatch
(verified by probe: source-file and IL-3-pipe), but they differ in the half that matters for
repair: the pipe gate **names the offending clause** in its message, and the source-file gate
does not — so the caller re-derives which token tripped it and re-runs. Fired four times in
one session on measurement commands.

## Symptom (Effect)

Probe B, run 2026-09-01 to isolate this (probe A is the control):

```
run_command("echo probe-A-clean; wc -l Cargo.toml")
→ exit_code 0
  probe-A-clean
  265 Cargo.toml

run_command("echo probe-B-mixed; wc -l Cargo.toml; grep -c fn src/main.rs")
→ {
    "ok": false,
    "error": "shell access to source files is blocked",
    "hint": "use symbols(name='fn') for declarations, references(symbol='fn') for direct
             callers, call_graph(symbol='fn', direction='callers') for transitive blast
             radius. Re-run with acknowledge_risk: true if you need raw shell grep."
  }
```

`echo` and `wc -l Cargo.toml` are byte-identical between the two calls and succeed in A.
In B they produce nothing. Nothing in the response identifies `grep -c fn src/main.rs` as
the cause.

## Reproduction

At `72484f8d5817e4675191d84caaaad869abf78f71`, from the codescout project root — the two
calls above. The control is load-bearing: it proves the clauses are individually
permitted, so the refusal is a property of the *string*, not of any clause.

**Real-world shape from the same session** (the reason this was noticed): a single
measurement command chaining `git log … | wc -l` several times plus one
`grep -c "fn " tests/issue_clusters.rs`. Every git measurement was thrown away for the one
grep, and the whole command had to be re-issued without it.

## Environment

- codescout `experiments` @ `72484f8d5817e4675191d84caaaad869abf78f71`
- Claude Code, `~/.claude-sdd` profile, MCP stdio
- Both `run_command` and native `Bash` permitted in this profile (the shell-mode eval is
  live — see CLAUDE.md § *Companion Plugin*), so this is about `run_command`'s own gate

## Root cause

> **Source read 2026-09-02 — this section's own caveat is discharged, and the fix is cheaper
> than it assumed.** The text below says *"the gate source has not been read yet, and this
> file's claim is therefore about observed behaviour only."* It has now been read, and the
> behavioural inference was right about the *what* and wrong about the *cost*.
>
> | gate | call site | decomposes? |
> |---|---|---|
> | `detect_il3_violation` | `src/tools/run_command/mod.rs:211` | **yes** — `strip_heredoc_bodies`, then `pipeline_segments` |
> | `is_dangerous_command` | `src/tools/run_command/inner.rs:298` | no — whole string |
> | `check_source_file_access` | `src/tools/run_command/inner.rs:315` | no — whole string |
>
> **§ *Fix*'s "one splitter, not two" requirement is already satisfied — the splitter exists.**
> `pipeline_segments` (`src/util/path_security.rs:1111-1123`) splits on `&&`, `||`, `;` and
> newline, quote-safe via `split_outside_quotes`, which tracks quote state across line breaks;
> `strip_heredoc_bodies` is at `:911`. **All three gates live in the same module as both
> helpers**, so the repair is two more call sites for a private function already beside them —
> no new parser, no plumbing, and nothing to keep in sync.
>
> This also refines § *Root cause*'s "both gates evaluate a per-command property over a
> per-string scope": IL-3 **does** decompose and **does** name its offending segment. What it
> shares with the source gate is only the all-or-nothing *refusal*, which § *Fix* argues is
> correct and should stay. So the live defect is narrower than the section states — two gates
> that do not decompose, one of which also does not name.
>
> Prescribed fix and its rationale: `docs/trackers/run-command-pipeline.md` § *Rulings* **R7**,
> which rules this bug and unbuilt design surface #6 together as one rule — *a gate's predicate
> is per-command; evaluate it per-command, refuse the whole call, and name the offender.*
>
> Two further firings on 2026-09-02, both on this session's own commands, both compound
> `;`-lists where one clause named a source file and the rest were unrelated. Running total
> across the two sessions: **six**.

`inferred from the gate's own emitted condition text — the gate source has not been read
yet, and this file's claim is therefore about observed behaviour only.`

The gate's self-description, returned alongside the refusal:

```
IL-3 source condition: refused when a CONTENT reader (cat/head/tail/sed/awk/less/more/grep)
names a source file INSIDE this project. `wc` is allowed — it returns a count, not content.
A path outside the project root is allowed, because `symbols`/`read_file` resolve against
the active project and cannot serve it. `acknowledge_risk: true` bypasses.
```

"names a source file" is evaluated over the submitted string. A `;` list is not decomposed
into its constituent commands first, so the predicate is existential over the whole string
and the verdict applies to all of it.

**This is NOT confined to the source-file rule — corrected 2026-09-01, same session, by
probe C.** This file first asserted that the IL-3 pipe limiter was "correctly
all-or-nothing — a pipeline is one command" and told the fixer to leave it alone. That was
wrong, and the error was mine: it conflated the *predicate's* scope (a pipeline, correctly)
with the *evaluation's* scope (the whole submitted string, incorrectly).

```
run_command("echo pipe-scope-probe-ran; find . -name '*.toml' | head -2")
→ REFUSED. `echo pipe-scope-probe-ran` produced NOTHING.
```

The pipe is in the second clause only; the first is unconditionally permitted and did not
run. So **both** gates evaluate a per-command property over a per-string scope, and the fix
below applies to both.

What the pipe gate already does right, and the source gate does not, is the **other** half:
its message names the offending clause verbatim — ``piped `find . -name '*.toml' | head -2`
to a log-trimmer`` — while the source gate emits only `"shell access to source files is
blocked"`. That asymmetry is the model for the message fix: the pipe gate proves the
offending clause is already in hand at refusal time.

Earlier framing, kept because it is still the right decomposition of the two rules: the pipe
rule reads the left side of a pipe; the source-file rule asks whether a content reader names
a project source file. The 2026-09-01 review's SR-13 item 3 filed both under "IL-3
granularity", which is why this file initially exonerated one of them. Of four gate firings
that session, every one was a correct *predicate* verdict and every one had the wrong
*scope*.

## Evidence

### The control isolates the string as the scope

See *Symptom*. Probe A and probe B share their first two clauses verbatim; A runs, B does
not. No clause-level property can explain that.

### The refusal does not name the clause

The `error` is a fixed string (`"shell access to source files is blocked"`) and the `hint`
is derived from the offending *symbol* (`symbols(name='fn')` — it clearly parsed `fn` out
of the grep) but not from the offending *clause*. So the information needed to name it is
already in hand at refusal time.

That is the interesting half: `docs/adrs/2026-08-27-negative-results-name-their-scope.md`
requires a negative result to name the scope it examined. A refusal is a negative result
about permission, and this one names the rule while withholding the position.

### Incidental reproduction — the clause that mattered was not the clause that offended

Logged 2026-09-01, unplanned, while doing unrelated work (locating a memory file). Submitted:

```
find ~/.local/share/codescout ~/.local/share/librarian .codescout -name 'gotchas*' 2>/dev/null | head
echo "---"
ls -l .codescout/memories/ 2>/dev/null | head -30
```

as one `run_command` string. Clause 1 is a genuine IL-3 violation (bare `find` → `head`). The
gate refused **the whole submission**, so clause 2 (`echo`) and clause 3 (`ls … | head`, whose
LHS is bounded and explicitly allowed) never ran — and the response gave no indication that two
of three clauses were innocent.

This is worth more than the three designed probes already in this file, for one reason: **it was
not a probe.** The three above were constructed by someone who already believed the defect
existed, so each could be dismissed as artificial. This one arrived as ordinary cost during
unrelated work, which is the population the fix is actually for. It also shows the failure is
not rare enough to need contriving — one session hit it inside ten minutes without trying.

Note the aggravating detail for the fix's error-message half: clause 3 uses `ls` as its LHS,
which the gate's own refusal text lists among the permitted bounded producers. So the refusal
quoted a rule that *permits* one of the clauses it was refusing. Any fix that evaluates
per-clause must name the offending clause, or a caller reading the rule text will conclude the
gate is broken rather than that a sibling clause tripped it.
## Hypotheses tried

1. **Hypothesis:** the whole compound command is refused because of one clause.
   **Test:** probes A and B above.
   **Verdict:** confirmed at the boundary.
2. **Hypothesis:** this is the same defect as IL-3's pipe limiter over-firing.
   **Test:** compared the two refusal messages and their stated conditions; re-read the
   `git grep … | head` refusal from earlier in the session.
   **Verdict:** rejected — **then OVERTURNED by probe C the same session.** The original
   rejection reasoned from the messages rather than probing, and concluded the pipe rule
   was correct because its *predicate* is. Probe C
   (`echo pipe-scope-probe-ran; find . -name '*.toml' | head -2`) showed the `echo` never
   ran, so the pipe rule has the identical scope defect. The two rules remain distinct
   predicates; they share one bug. **Recorded rather than deleted** — reasoning from an
   error message about a gate's scope is the mistake, and probing took one call.
3. **Hypothesis:** `grep -c` should be exempt because it collapses to a count.
   **Test:** the IL-3 pipe rule explicitly exempts collapsing stages (`wc`, `grep -c`);
   the source rule does not mention `-c`.
   **Verdict:** deferred, and probably should stay rejected — `grep -c PATTERN file` still
   reads the file's content, and a count over a source file leaks structure. Not the fix
   to reach for. Recorded so a later session does not re-derive it as attractive.

## Fix

**Re-scouted before fixing, per this file's own root-cause correction, and found
the scope narrower than the title implies.** `check_source_file_access`
(`src/util/path_security.rs:1694`) already decomposes into `;`/`&&`/`||`/`\n`-
separated "runs", each further split on `|` into "stages", with `cd` tracked
across runs — this must have landed via one of the several archived heredoc/
newline/cd-tracking bugs referenced in its own comments (all dated
2026-08-17), none of which were filed as *this* bug. So the gate no longer
refuses on an unrelated clause; it correctly finds the one specific segment
that reads project source. What was still true, verified live before fixing:
the returned hint never named that segment, so a caller facing a 3-clause
command had to guess which one tripped it.

**Fixed:** `check_source_file_access` now prepends
`offending clause: \`<segment>\` (N other clause(s) in this command were/was
not run).` to its hint, mirroring `detect_il3_violation`'s existing pattern
(which already named its offender). The sibling count is computed over the
WHOLE command (not just the segments scanned before the first match, which
the detection loop deliberately short-circuits past via `break 'runs`) and is
omitted entirely for a single-clause command, where naming it would be noise.

**Not fixed, and deliberately left as a residual rather than folded in:**
`is_dangerous_command` (`src/util/path_security.rs:806`) still evaluates its
regex patterns over the WHOLE command string, not per-segment. It is a
different design from `check_source_file_access` (substring/regex matching
for known-dangerous patterns, including inside a `<<` heredoc body via its
own separate `locate()`/`heredoc_body_note` path) rather than token-based
command analysis, so decomposing it is a materially different change with its
own tradeoffs — not a copy-paste of this fix. Left for its own bug file if
someone wants to take it, per this repo's "don't force a fit" rule; not filed
here since it would need its own reproduction and design discussion.

Fix SHA: `6acb3da0f16f32b7be8c483005c9244ff05b97b8` (experiments)
Patch-id: `1cc6b7b72b23b8d429e52d757201893fdeb14d2d`
## Tests added

`util::path_security::tests::source_file_access_names_the_offending_clause_in_a_compound_command`
and `..._omits_the_sibling_note_for_a_single_clause`
(`src/util/path_security.rs`) — the first reproduces this file's own
reproduction (an unrelated clause on either side of the real offender) and
asserts both the named clause and the correct sibling count; the second
guards against the note appearing where there is nothing to distinguish the
clause from.
## Workarounds

Issue source-file reads as their own call, and keep compound measurement commands free of
content readers aimed at source paths. `wc` on a source file is allowed (it returns a
count, not content), so `wc -l src/main.rs` inside a compound command is fine — it is
`cat`/`head`/`tail`/`sed`/`awk`/`grep` that trip the gate. For genuine raw access,
`acknowledge_risk: true`.

## Resume

N/A for the fix in this file — done. If `is_dangerous_command`'s lack of
per-segment evaluation turns out to matter in practice (a dangerous pattern
in one clause suppressing unrelated safe clauses, or a false match inside an
inert quoted string in another clause), file it as its own bug rather than
reopening this one — the mechanism and the fix shape both differ.
## References

- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — a negative result names its
  scope; this refusal does not name its position
- CLAUDE.md § *Parsers Over a Namespace* — the four-shell-gate heredoc tell, and why the
  splitter should be shared rather than duplicated
- `docs/trackers/2026-09-01-fable-system-review.md` SR-13 item 3 — the original form, which
  conflated this with the pipe limiter; corrected here
