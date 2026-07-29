---
id: '56640fb8a33f2bb0'
kind: bug
status: open
title: IL3 pipe gate scans heredoc content, so a commit message that describes a pipe is blocked
tags:
- run_command
- tooling
- false-positive
- progressive-disclosure
topic: run_command guards
---

# BUG: IL3 pipe gate scans heredoc content, so a commit message describing a pipe is blocked

## Summary

`run_command`'s IL3 guard rejects an unbounded command piped to a log-trimmer. It decides
by pattern-matching the command **string**, and that string includes the body of a
heredoc. A `git commit -F -` whose message *describes* a piped command is therefore
refused, even though the pipe appears only inside quoted document text that the shell
never interprets.

Low severity, entirely recoverable, and worth recording because of what it blocks: writing
down a piping mistake in the commit that fixes it.

## Symptom (Effect)

```
run_command("git commit -q -F - <<'EOF'
… five distinct errors this session — a liveness filter whose output was then
reused as the population, pgrep | tail -1 picking an orphan, a SHA census
matching frontmatter id: values, ls | wc -l counting .anchors.toml sidecars …
EOF")
```

→

```
IL3 violation — piped `git commit -q -F - <<'EOF' … ` to a log-trimmer. BLOCKED.
```

The `git` at the head of the command supplies the "unbounded LHS", and `| tail -1` inside
the message supplies the "trimmer RHS". Neither is a pipe the shell would execute: the
heredoc is `<<'EOF'`, single-quoted, so not even parameter expansion applies to it.

## Reproduction

```
git rev-parse HEAD    # 0f6815f8, branch experiments
```

1. `run_command` any `<<'EOF'`-heredoc command whose first word is an unbounded producer
   (`git`, `cargo`, …).
2. Put the two characters `| ` followed by `tail`, `head`, `grep`, or `sort` anywhere in
   the heredoc body.
3. The call is refused with an IL3 violation naming a pipe that does not exist.

## Environment

Linux, codescout `experiments` @ `0f6815f8`, `run_command` via the MCP server built at
14:08. Also echoed by the companion plugin's advisory `PreToolUse` hook, which prints the
same diagnosis — so the two surfaces agree, and both are looking at the same string.

## Root cause

Not read. The refusal text is emitted server-side (the companion hook explicitly calls
itself "an advisory echo, not the enforcer"), so the check lives in `run_command`'s guard
path and is evidently lexical over the whole command string rather than shell-aware.

Making it fully correct means parsing shell quoting, which is a poor trade for this. The
tractable version is narrower: ignore anything between a heredoc delimiter and its
terminator. That is a single well-delimited region, and it is where free text lands.

## Evidence

The same command committed successfully once the two literal pipe characters were replaced
with prose — `pgrep piped to "tail -1"`, and `an ls count that included .anchors.toml
sidecars`. Nothing else changed: same producer at the head, same heredoc form, same
length. So the trigger is the pipe glyphs in the document body and nothing else.

## Hypotheses tried

1. **Hypothesis:** the heredoc itself is the problem — the guard rejects `<<` outright.
   **Test:** the identical command with the pipes reworded committed fine.
   **Verdict:** rejected. The heredoc form is accepted; only its content matters.

## Fix

Not implemented. Options:

1. **Skip heredoc bodies.** When the command contains `<<'DELIM'` or `<<DELIM`, exclude
   the region up to the terminator line before applying the IL3 match. Narrow, and it
   covers the case that actually bites, since free text is what arrives by heredoc.
2. **Require the pipe to be shell-effective** — outside quotes and heredocs. Strictly more
   correct and considerably more work; a small quote-state scan, the same shape as the
   literal scanner in `src/util/text.rs`.
3. **Leave it.** The workaround is one rewording and the guard's false-negative direction
   (letting a real pipe through) is worse than this false positive. If nothing else, the
   error text could name the heredoc as a likely cause.

Option 1 is the proportionate fix. Option 3 is defensible; if taken, do the error-message
part of it, because the current message points at a pipe the reader cannot find.

## Tests added

None — no fix. A regression test wants a heredoc command carrying `| tail -1` in its body
and asserting the guard **permits** it, paired with the existing negative case so the two
pin the boundary together.

## Workarounds

Reword the pipe out of the document text: `pgrep piped to "tail -1"` rather than the
glyph. Or write the message to a file first and pass `git commit -F <file>` — though the
`create_file`/`edit_file` route is the more codescout-idiomatic way to get text onto disk
than another heredoc.

## Resume

Find the IL3 check in `run_command`'s guard path (the server emits the "IL3 violation —
piped `…` to a log-trimmer" string; grep that literal) and see whether the matched region
is the raw command or an already-tokenised form. If raw, option 1 is a few lines.

## References

- `docs/PROGRESSIVE_DISCLOSURE.md` — the IL3 rule this guard enforces, and why it exists
- `src/util/text.rs` — `literal_continuation_mask`, a scanner of the shape option 2 needs

