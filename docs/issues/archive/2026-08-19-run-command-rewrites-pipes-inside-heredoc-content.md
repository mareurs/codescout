---
id: 0de2778e6adac220
kind: bug
status: fixed
title: 'BUG: run_command''s pipe instrumentation rewrites pipes inside heredoc content, corrupting written files'
tags:
- run-command
- shell
- output-buffer
- data-corruption
closed: 2026-08-19
opened: 2026-08-19
owner: marius
related: []
severity: high
---

# BUG: run_command rewrites pipes inside heredoc content

> **Status: fixed 2026-08-19.** Sibling of
> `docs/issues/archive/2026-08-16-run-command-backticks-substituted-in-quoted-message.md`
> (mitigated) — same root: `run_command` transforming shell metacharacters that are
> **content**, not command structure.

## Summary

`run_command` instruments pipelines to capture unfiltered output, inserting
`| tee '/tmp/codescout-unfiltered-XXXXXX' |` into the command. The rewrite is applied to
the command **string**, so it also fires on a `|` that appears inside a heredoc body —
text destined for a file, never executed as a pipeline.

The result is silent corruption of written content. Exit code 0, no warning.

## Symptom

Appending a documentation block containing a shell example:

```
cat >> "$f" <<EOF
- **Resolve:** \`git log --all -p | git patch-id --stable | grep $short\`
EOF
```

What landed in the file:

```
- **Resolve:** `git log --all -p | git patch-id --stable | tee '/tmp/codescout-unfiltered-hUMfFa' | grep 0951182a71b5`
```

A temp path that will not exist, written into a permanent record as an instruction.

## Why this is severity high

The corruption is **invisible at the call site**. The command succeeds, the file is
written, and the damage is only in content the author does not re-read — which is exactly
the content a heredoc is used for (docs, templates, generated config). Caught here only
because the change was rehearsed on one file and the result inspected before scaling to 53.
Unrehearsed, it would have written 53 permanent archive records each carrying a
resolution command that cannot work — manufacturing the precise defect the pass existed to
cure.

## Reproduction

1. `run_command` with a heredoc whose body contains a literal `|`.
2. Read the written file.
3. The pipe has become ` | tee '/tmp/codescout-unfiltered-…' | `.

## Workaround

Source the character from a variable so the static scan cannot see it:

```
P='|'
cat >> "$f" <<EOF
... ${P} ...
EOF
```

Better for bulk writes: build a template with a **quoted** heredoc (`<<'EOF'`, no
expansion) holding placeholders, then fill it per file with `sed`. That avoids both this
bug and backtick substitution, and is what the 53-file pass used after the first attempt
was corrupted.

## Fix ideas

1. Parse the command rather than string-rewriting it, so heredoc bodies and quoted strings
   are not candidates for instrumentation. Correct, and the largest change.
2. Detect a heredoc (`<<`) and skip instrumentation for that invocation, reporting that
   unfiltered capture was disabled.
3. At minimum, refuse rather than silently rewrite — a visible error beats corrupted
   content, given the corruption lands in files nobody re-reads.

Option 2 is cheap and removes the whole class for the common case.

## Fix

**Option 1's correctness at option 2's cost, by masking instead of stripping.**

`mask_heredoc_bodies(command)` in `src/util/path_security.rs` blanks heredoc bodies **in
place** — every body byte becomes a space, so `len()` and every index are unchanged.
`inject_tee` now asks `detect_terminal_filter` about the *masked* copy and splices into the
*original*: because masking is length-exact, the returned `pipe_pos` addresses the same
character in both, and the splice needs no offset mapping.

**Why not option 2, which this file recommended.** Skipping instrumentation whenever `<<`
appears removes the corruption but also silently disables unfiltered capture for every
heredoc command — including `cat <<'EOF' | grep x`, where the pipe is on the opener line
and is real pipeline syntax. Masking costs about fifteen more lines and loses nothing.
The deviation is recorded here rather than made silently.

**Why the existing `strip_heredoc_bodies` could not simply be reused** — and this is the
whole reason a second function exists. It *removes* body lines, which is exactly right for
IL-3, a yes/no gate that never reports a position. Removing bytes moves every later index,
so a caller that splices at a returned offset would write into the wrong place. The two
functions now share one opener regex via `heredoc_opener()`, extracted rather than copied:
this file has already paid for a rule that existed twice and diverged (`dbaeb78b`), and
two heredoc scanners disagreeing would mean the analysis and the transform reading
different commands.

Multi-byte characters are replaced by as many spaces as they occupy. A one-space-per-char
version drifts on the first non-ASCII byte in a heredoc, which is why the function carries
an internal `debug_assert_eq!` on its own length.

## Tests added

Six. Five pin `mask_heredoc_bodies` in `src/util/path_security.rs`; one is end-to-end in
`src/tools/run_command/tests.rs` — it writes a file through a heredoc whose body contains
pipes and reads the bytes back, because the damage this bug does is invisible at the call
site and only a byte comparison of written content can see it.

`masking_hides_body_pipes_from_terminal_filter_detection` asserts the **pre-fix** behaviour
as a precondition: the raw string must still fool `detect_terminal_filter`. Without it, a
later change that stopped reproducing the bug would leave the second assertion passing
vacuously.

Mutations applied and the **observed** result:

| # | Mutation | Observed |
|---|---|---|
| M1 | detect on the raw string again (revert the wiring) | end-to-end test FAILS |
| M2 | one space per char instead of `len_utf8` | offset test FAILS |
| M3 | disable heredoc-opener detection | 3 tests FAIL |

Zero survivors. **M1's failure output reproduced this bug verbatim** — the written file
came back as
`- Resolve: git log --all -p | git patch-id --stable | tee '/tmp/codescout-unfiltered-Vyjjdw' | grep abc123`,
the same shape reported in § *Symptom* down to the temp-file prefix. The regression test
reproduces the defect on demand rather than describing it.

Gate: fmt, clippy `--all-targets -D warnings`, `cargo test` 4262 passed / 45 ignored
(+6 from 4256).
## References

- [2026-08-16 run_command backticks substituted in a quoted message](archive/2026-08-16-run-command-backticks-substituted-in-quoted-message.md) — sibling defect, same root: `run_command` transforming shell metacharacters that are content rather than command structure.
- [2026-08-19 archived fix SHAs orphan when experiments rebases](2026-08-19-archived-fix-shas-orphan-when-experiments-rebases.md) — the 53-file pass during which this surfaced.

## Fix provenance

- **SHA:** `4ea33d15` (`experiments`) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `50691255eccdc8e7ebb3ce3634d3ee01a8b17a3d` — content hash of the diff; survives rebase and cherry-pick.

If the SHA stops resolving, recover the commit by patch-id. Use redirects, not pipes —
Iron Law 3 blocks an unbounded `git log -p` piped to a trimmer, and note that the pipes
below are exactly the shape this bug used to rewrite:

```
git log --all -p > /tmp/all.patch
git patch-id --stable < /tmp/all.patch > /tmp/patch-ids.txt
grep 50691255eccd /tmp/patch-ids.txt
```

Each hit is `<patch-id> <commit>`. Several hits mean the change exists on several branches
(cherry-pick) and any of them is the fix.
