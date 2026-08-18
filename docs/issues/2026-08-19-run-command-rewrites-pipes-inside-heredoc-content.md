---
id: d5cb0c41335b2610
kind: bug
status: open
title: 'BUG: run_command''s pipe instrumentation rewrites pipes inside heredoc content, corrupting written files'
tags:
- run-command
- shell
- output-buffer
- data-corruption
closed: ''
opened: 2026-08-19
owner: marius
related: []
severity: high
---

# BUG: run_command rewrites pipes inside heredoc content

> **Status: open.** Sibling of
> `docs/issues/2026-08-16-run-command-backticks-substituted-in-quoted-message.md`
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

## References

- [2026-08-16 run_command backticks substituted in a quoted message](2026-08-16-run-command-backticks-substituted-in-quoted-message.md) — sibling defect, same root: `run_command` transforming shell metacharacters that are content rather than command structure.
- [2026-08-19 archived fix SHAs orphan when experiments rebases](2026-08-19-archived-fix-shas-orphan-when-experiments-rebases.md) — the 53-file pass during which this surfaced.
