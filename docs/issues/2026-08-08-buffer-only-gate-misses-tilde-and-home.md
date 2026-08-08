---
status: open
opened: 2026-08-08
closed:
severity: high
owner: marius
related: []
tags: [security, run_command, output-buffer, windows]
kind: bug
---

# BUG: `is_buffer_only` treats `~` and `$HOME` as non-paths, so a command carrying a buffer ref skips the dangerous-command gate entirely

## Summary

`OutputBuffer::resolve_refs` classifies a command as *buffer-only* when no argument looks
like a path, and `run_command` skips both the dangerous-command gate and the source-file
block for buffer-only commands. The path heuristic only recognises `/`-bearing words, so
`~`, `$HOME` and `*` are invisible to it — while the shell expands all three. A command
that contains a `@cmd_*` ref and reaches the filesystem exclusively through `~` is
therefore executed with no safety check.

Pre-existing on Unix. PR #10 makes it live on Windows too, by replacing `cmd.exe` (where
`~` is a literal character) with Git Bash.

## Symptom (Effect)

```
run_command("cat @cmd_abcd1234 && rm -rf ~")
```

Words after ref substitution: `cat`, `<temp path>`, `&&`, `rm`, `-rf`, `~`. The temp path
short-circuits as one of our own; none of the rest starts with `/` or `./` or contains
`/`. So `is_buffer_only == true`, `is_dangerous_command` is never called, no `pending_ack`
speed bump is emitted, and bash expands `~` to the user's home directory.

## Reproduction

Not executed — the payload is destructive by construction. Traced at the bytes instead;
see § Root cause for the two line references, both of which are plain conditionals.
A non-destructive equivalent that shows the classification: any command pairing a
`@cmd_*` ref with a `~`-only argument reports no `pending_ack` where the same command
spelled with an absolute path does.

## Environment

codescout 0.15.0, `experiments` and `fix/windows-paths-and-doctor` alike. Unix: always.
Windows: since WIN-32 (`b142a514`, Git Bash replaces `cmd.exe`).

## Root cause

Two conditions compose into a hole:

- `src/tools/output_buffer.rs:618-631` — a word counts as path-like only if
  `word.starts_with('/')`, `word.starts_with("./")`, or
  `!word.starts_with('-') && word.contains('/')`. `~`, `$HOME` and `*` satisfy none.
- `src/tools/run_command/inner.rs:288` — `if !buffer_only && !acknowledge_risk { ... }`
  wraps the `is_dangerous_command` call, and `:306` wraps `check_source_file_access` the
  same way. Buffer-only means *both* are skipped, not softened.

The heuristic answers "does this word look like a path to a reader", when the question
that matters is "will the shell turn this word into a path". Those diverge exactly at
shell expansion — tilde, parameter, and glob.

**measured 2026-08-08:** read at
`src/tools/output_buffer.rs:618` and `src/tools/run_command/inner.rs:288` on
`refs/pull/10/merge`; both are unconditional plain `if`s with no other guard in between.
Independently reported by the platform/security reviewer in the PR #10 review.

## Evidence

### The gate is otherwise sound — the hole is specifically the buffer-only skip

`is_dangerous_command` matches substrings over the raw command, so `$(...)` and backticks
do **not** hide a dangerous verb from it. The problem is not evasion of the matcher; it is
that the matcher is never consulted.

### Why `~` and not just any bare word

A bare word like `foo` is harmless — no expansion makes it a path. `~` and `$HOME` are
different in kind: the shell resolves them to absolute paths before the program sees them.
That is the same reasoning `shell_path_str` already applies elsewhere in this module.

## Hypotheses tried

1. **Hypothesis:** the `&&` splits the command, and only the first segment is classified.
   **Test:** read `resolve_refs` — `shell_words(&result)` tokenizes the whole string and
   `.any()` runs over every word, with no segment splitting. **Verdict:** rejected; the
   whole command is one classification.
2. **Hypothesis:** `store_dangerous`/`pending_ack` still fires later in the pipeline.
   **Test:** followed `inner.rs` past step 2 — steps 2.5 and 3 carry the same
   `!buffer_only` guard. **Verdict:** rejected.

## Fix

Teach the heuristic the words the shell will expand into paths. In
`src/tools/output_buffer.rs:618`, before the separator checks:

```rust
// Expanded by the shell into a path, though they carry no separator.
if word == "~" || word.starts_with("~/") || word.starts_with('$') || word.contains('*') {
    return true;
}
```

Deliberately NOT applied in PR #10. It changes the behaviour of a security gate — in the
conservative direction (more commands checked, never fewer), but it will make some
currently-buffer-only commands take the `pending_ack` round trip, and that belongs in a
change whose reviewers are looking at the gate rather than at Windows path handling.
Note `^foo$` and similar regex anchors do NOT start with `$`, so the common
`grep "^foo$" @cmd_x` idiom is unaffected — worth a test either way.

## Tests added

None yet. The fix needs three: `~` alone, `$HOME`, and a glob each flip
`is_buffer_only` to false; plus a negative for `grep "^foo$" @cmd_abc1234` staying
buffer-only, which is the regression the fix could plausibly cause.

## Workarounds

Spell the path absolutely (`/home/you`) rather than `~` — that is already classified
correctly and takes the normal gate. The gate is also reachable deliberately via
`acknowledge_risk`.

## Resume

Apply the § Fix hunk at `src/tools/output_buffer.rs:618`, then write the four tests named
in § Tests added BEFORE running the suite, so the `^foo$` regression is observable rather
than discovered. Then re-check `OutputBuffer::is_buffer_only` (`:649`, the static
test-only twin) for the same gap — it duplicates the heuristic and would otherwise drift
from the live computation at `:618`.

## References

- `src/tools/output_buffer.rs:618` — the heuristic
- `src/tools/run_command/inner.rs:288`, `:306` — the two gates it skips
- `docs/issues/2026-08-08-security-layer-tokenizes-unlike-the-shell.md` — sibling defect
  in the same layer: the gate's tokenizer does not match the executing shell's
- PR #10 review, 2026-08-08
