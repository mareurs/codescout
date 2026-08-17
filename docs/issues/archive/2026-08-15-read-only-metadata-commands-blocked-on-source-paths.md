---
kind: bug
status: fixed
tags:
- run_command
- path-security
- friction
- external-report
closed: 2026-08-17
opened: 2026-08-15
owner: marius
related: []
severity: low
---

# BUG: read-only metadata commands (`wc`, `ls`, `stat`) are blocked on source paths with no carve-out

## Summary

`run_command("wc -lc src/foo.rs")` is refused by the source-path gate. `wc`, `ls` and
`stat` return pure metadata and cannot disclose more than `symbols` already does, so
the block buys no safety while nudging the agent toward *guessing* file size rather
than measuring it. Filed as a decision — carve out read-only metadata, or declare the
blanket block intentional and say so in the guide.

## Symptom (Effect)

```
run_command("wc -lc src/…")
→ shell access to source files is blocked
```

Reproduced this session with a different command in the same family:

```
run_command("ls -1 src/tools/read_file.rs … && sed -n '322p' src/tools/core/types.rs")
→ shell access to source files is blocked
   hint: use read_file(path, start_line, end_line), symbols(path),
   symbols(name=..., include_body=true), or grep(regex) instead.
   Re-run with acknowledge_risk: true if you need raw shell access.
```

## Reproduction

Any `run_command` naming a source path, at `821f9d0d`. The `acknowledge_risk: true`
override works, so this is friction, not a wall.

## Environment

Reported on macOS against `experiments @ d7988aca`; reproduced on Linux at `821f9d0d`.

## Root cause

**Working as coded — the reporter's diagnosis was wrong, and that is itself part of the
finding.** He concluded `wc` was "in the block list". There is no command list.
`src/tools/run_command/inner.rs:305-315` calls
`crate::util::path_security::check_source_file_access(resolved_command)`, a **path**
predicate: any resolved command naming a source file is refused regardless of which
binary it invokes or how bounded its output is.

He was led to the command-list model by the guide, which claims a bounded-file
exception that does not exist — filed separately as
`docs/issues/archive/2026-08-15-iron-laws-detail-guide-claims-cat-on-source-is-allowed.md`.

*Measured 2026-08-15: the `ls`/`sed` command above was executed against the live server
and returned the quoted error.*

**Correction, measured 2026-08-17 at `021c130d`: this file's own symptom was
misattributed, and the "no per-command carve-out" claim was never true.**

`check_source_file_access` is a **two-part** predicate: the segment's *first token* must
be in `SOURCE_ACCESS_COMMANDS` **and** the segment must name a source extension. There is
a command list, and it is half the gate. The quoted repro is two segments joined by `&&`
— the block came from **`sed`**, not from `ls`. `ls`, `stat`, `du` and `file` were never
on the list at all.

Of the three commands in this bug's title, only `wc` was ever blocked, and it came off the
list on 2026-08-16 (GF-3, `docs/trackers/2026-08-16-iron-law-gate-firing-audit.md`) with
the reasoning this bug argued for: it returns a measurement *of* the content, and
codescout ships no tool that returns a line count, so the refusal named an alternative
that does not exist.

Verdicts on `src/util/path_security.rs`, one call each:

| command | verdict |
|---|---|
| `wc -l` | allowed |
| `ls -la` | allowed |
| `stat -c` | allowed |
| `du -h` | allowed |
| `file` | allowed |
| `cat` (control) | **refused** |

The control refusing is what makes the five greens mean something rather than reading as
a disabled gate.

## Evidence

### The cost is a wrong mental model, not a blocked task

The override exists and works. What the gate actually cost here was an outside user
spending part of an investigation on a "block list" that does not exist, and writing a
design section (his D3) against the wrong mechanism.

### The nudge-to-guess concern

Both the reporter and the gate's own hint route the agent to `symbols` / `read_file`.
Neither answers "how many bytes is this file", so the practical effect is an agent that
estimates size instead of measuring it — the exact habit
`docs/issues/archive/2026-08-15-read-file-buffered-summary-has-no-incompleteness-signal.md`
shows going wrong.

## Hypotheses tried

1. **Hypothesis:** `wc` is specifically enumerated in a blocklist (the reporter's
   model). **Test:** read `src/tools/run_command/inner.rs` Step 2.5 and look for a
   command allowlist/blocklist. **Verdict:** rejected — the predicate is
   `check_source_file_access(resolved_command)`, path-based, with no per-command branch.

## Fix

Both outcomes landed, a day apart and by different routes.

**A — carve out read-only metadata: already shipped, 2026-08-16.** `wc` was removed from
`SOURCE_ACCESS_COMMANDS` under GF-3. `ls`, `stat`, `du` and `file` needed no carve-out;
they were never on the list. Nothing in this bug was actioned to make that happen, and
nothing closed this file when it did — the fix arrived from a gate-firing audit that was
not reading this queue.

**B — the guide: `90c5aea1` (experiments).** `src/prompts/guides/iron-laws-detail.md` had
over-corrected from B-9's phantom carve-out into the opposite falsehood, *"by path, not by
command … there is no per-command carve-out"*, and listed `wc` and `ls` as refusing. It
now states the two-part predicate, names all eight blocked content readers, and documents
the metadata commands as usable with the content-vs-measurement-of-content line the
constant's own doc comment already carried.

The part that stops a third round: `SOURCE_ACCESS_COMMANDS` became a `&[&str]` with the
regex built from it, and the guide's list is now **derived from that constant by test**.
The two copies drifted for a day when `wc` was removed because no test read both.
## Tests added

`src/prompts/mod.rs`, both in `redesign_invariants`:

- `iron_laws_detail_gate_names_every_blocked_command` — iterates
  `SOURCE_ACCESS_COMMANDS` and asserts the guide names each one, so an edit to the gate
  fails the build until the guide follows. Its complement asserts `wc`/`ls`/`stat`/`du`/
  `file` are absent from the constant, because the guide now documents them as usable and
  that half needs a guard too.
- `iron_laws_detail_never_regrows_the_bounded_file_carve_out` — renamed from
  `iron_laws_detail_gate_claim_matches_path_predicate`, which pinned the false sentence.
  Keeps B-9's `"allowed on bounded files"` guard verbatim (0/10 unaided survival is why it
  exists) and adds a refusal of `"by path, not by command"`.

Red observed before the fix, each for its own reason: *"iron-laws-detail never names
`head`"* and the path-only assertion. `162` `path_security` tests were green after the
constant refactor and before the guide was touched, which is what establishes the refactor
as behaviour-neutral rather than as part of the fix.
## Workarounds

`run_command(..., acknowledge_risk=true)` — the reporter confirmed this executes `wc`
successfully. Or read `line_count` from a `read_file` summary, which reports it without
shell access.

## Resume

N/A — fixed and archived.

One loose thread, deliberately not pulled: `docs/issues/archive/` holds two files citing
the old test name `iron_laws_detail_gate_claim_matches_path_predicate`. Left alone per
`get_guide("tracker-conventions")` — archives are historical snapshots, and that name was
correct when they were written.
## References

- `docs/trackers/bistriceanu/index.md` § B-6
- `docs/trackers/bistriceanu/full-read-fidelity-design.md` § D3 — the reporter's writeup, against the wrong mechanism
