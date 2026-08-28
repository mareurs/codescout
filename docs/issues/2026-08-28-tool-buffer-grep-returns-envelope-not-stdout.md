---
id: '2d546e0f7b8fcc0c'
kind: bug
status: open
title: 'BUG: grepping a @tool_* buffer that holds a run_command result returns the JSON envelope, and each re-read re-wraps — the stdout is unreachable'
owners:
- marius
tags:
- run_command
- output_buffer
- progressive-disclosure
---

# BUG: grepping a `@tool_*` buffer returns the envelope, not the stdout

## Summary

Chaining `grep` across buffer handles can reach a state where every read returns
the `run_command` **JSON envelope** (`{exit_code, output_id, ...}`) instead of the
stdout it describes, and each follow-up read produces another envelope one level
down. The payload becomes unreachable through the buffer API; the only escape is
to re-run the command redirecting to a real file.

**Not isolated.** The sequence below was observed live and is recorded verbatim.
Three attempts to reduce it to a minimal case did **not** reproduce. Filed as a
quirk with evidence rather than a diagnosed defect — read the Hypotheses section
before trusting any mechanism.

## Symptom (Effect)

Observed 2026-08-28, working the doctor report. `@tool_4795f92d` is a
`librarian(action="doctor")` result (136367 bytes).

```
run_command("grep -c 'work/claude/codescout' @tool_4795f92d; echo ---;
             grep -o '\"path\": \"[^\"]*\"' @tool_4795f92d")
→ @tool_479696b5   "✓ exit 0 · 127 lines"   buffered_bytes: 10273

run_command("grep -v 'work/claude/codescout' @tool_479696b5")
→ @tool_47969f3f   "✓ exit 0  (query @cmd_47969f3d)"   buffered_bytes: 11492

read_file("@tool_47969f3f")
→ 6 lines:  { "type": "generic", "exit_code": 0, "output_id": "@cmd_47969f3d", ...
             [4 of 6 lines shown]

read_file("@cmd_47969f3d")
→ 4 lines:  { "exit_code": 0,   [2 of 4 lines shown]

read_file("@tool_479696b5", start_line=1, end_line=30)
→ 4 lines:  { "exit_code": 0,   Buffer: @file_4796c316

read_file("@file_4796c316", start_line=1, end_line=25)
→ 4 lines:  { "exit_code": 0,   Buffer: @file_4796cc53
```

Six reads, four distinct handle kinds (`@tool_`, `@cmd_`, `@file_`), zero lines of
the grep output. Escaped by re-running with `> /tmp/doctor-paths.txt` and reading
the file.

## Reproduction

**Not reproduced.** Attempts that did NOT trigger it:

1. `seq 1 4000 | sed 's/^/line /' > /tmp/big.txt; cat` → `@cmd_*`; `grep 'line'`
   on it returned stdout inline, capped at 100 lines with a correct paging hint.
2. 50 lines × 300 chars (15400 bytes, over the 10000-byte buffer threshold but
   under the 100-line cap) → grep returned stdout inline with `stdout_shown: 31`,
   `stdout_total: 50`.
3. `grep -o '"detail": "[^"]*"' @tool_4795f92d` — same source buffer as the live
   case — returned stdout inline, `stdout_shown: 31`, `stdout_total: 125`.

So a `@cmd_*` buffer greps correctly, and the *same* `@tool_*` buffer greps
correctly with a different pattern. The live case differs in two ways not yet
separated: it used a **compound command** (`grep -c ...; echo ---; grep -o ...`),
and its result landed in a `@tool_*` rather than a `@cmd_*` handle.

## Environment

Branch `experiments` @ `14aab5ff`, linux, codescout 0.15.0, MCP over stdio,
release build from `cargo rb` at 11:55 local.

## Root cause

**Unknown — see Hypotheses tried.** The leading hypothesis, *inferred from the
handle kinds in the transcript and not measured*:

`run_command` results are stored under two handle kinds with **different payload
semantics** — `@cmd_*` appears to hold raw stdout, `@tool_*` appears to hold the
serialized JSON envelope. A tool reading a `@tool_*` handle therefore gets the
envelope; grepping it greps the envelope; and since that grep is itself a
`run_command`, its result is another envelope. If correct, the defect is not the
nesting but the **ambiguity**: nothing in the returned handle tells the caller
which payload kind it holds, so there is no way to know a read will yield stdout
until it does not.

Do not act on this without measuring it. `src/tools/output_buffer.rs` is the place
to look; it changed by +212 lines in the 437-commit range that landed today, so the
behaviour may be newer than the transcript suggests.

## Hypotheses tried

1. **Hypothesis:** any grep of a buffer whose result exceeds the 10000-byte
   threshold nests.
   **Test:** repro attempt 2 — 15400 bytes, 50 lines.
   **Verdict:** rejected — returned stdout inline.

2. **Hypothesis:** it is specific to `@tool_*` source buffers.
   **Test:** repro attempt 3 — grepped the same `@tool_4795f92d` with a different
   pattern.
   **Verdict:** rejected as *sufficient* — that grep returned stdout normally. May
   still be necessary-but-not-sufficient.

3. **Hypothesis:** the compound command (`;`-separated, mixing `grep -c` with
   `grep -o`) is the trigger.
   **Test:** not yet run.
   **Verdict:** deferred — this is the untested difference and the next thing to try.

## Fix

None. Do not fix before reproducing — the mechanism above is inferred from handle
names, and the repo's own record has three cases this month where a bug file's
prescribed fix was wrong in *direction*
(`docs/trackers/bug-ledger-resume-2026-08-28.md` § Method notes).

## Tests added

None. A regression test is premature without a reproduction.

## Workarounds

Redirect to a real file and read that:

```
run_command("grep -o '\"path\": \"[^\"]*\"' @tool_xxxx > /tmp/out.txt; wc -l /tmp/out.txt")
read_file("/tmp/out.txt")
```

This also sidesteps Iron Law 3 cleanly, since the redirect is not a pipe to a
trimmer.

## Resume

Run hypothesis 3: issue a compound `run_command` (`grep -c X @tool_ref; echo ---;
grep -o Y @tool_ref`) against a large `@tool_*` buffer and check whether the result
handle is `@tool_*` or `@cmd_*`, and whether reading it yields stdout or an
envelope. If it reproduces, read `src/tools/output_buffer.rs` for where the handle
kind is chosen and whether the stored payload differs by kind.

## References

- `get_guide("progressive-disclosure")` § The @ref buffer
- `docs/trackers/tool-usage-patterns.md` T-27 — *grep -n on a @tool_\* buffer numbers the buffer, not the source* (a neighbouring quirk in the same surface)
- Found during the 2026-08-28 cross-machine catalog repair; see `docs/conventions/cross-machine-catalog-resume.md`

