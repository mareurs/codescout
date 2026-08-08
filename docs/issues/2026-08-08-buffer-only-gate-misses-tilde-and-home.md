---
kind: bug
status: fixed
tags:
- security
- run_command
- output-buffer
- windows
closed: 2026-08-08
opened: 2026-08-08
owner: marius
related: []
severity: high
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

Landed on `experiments` (master-side SHA still to be recorded after cherry-pick).

**The heuristic existed twice, and the two copies had already diverged.** That was not in
the original report and is the reason a one-site fix would have left the bug half-open:

| copy | splitter | `../` | sigils |
|---|---|---|---|
| closure in `OutputBuffer::resolve_refs` | `shell_words` (quote-aware) | **no** | no |
| `OutputBuffer::is_buffer_only` | `split_whitespace` (quote-blind) | yes | no |

Both now call a single `is_path_like` in `src/tools/output_buffer.rs`, and
`is_buffer_only` splits with `shell_words` like its twin. The rule gains the shell
expansion sigils: a word beginning `~`, `$` or `*` is path-like.

**Sigils count only at the START of a word.** `contains` was tried first and the control
test rejected it: `awk '{print $0}' @cmd_x` (the `$0` is single-quoted, so the shell never
expands it) and `grep '.*ERROR' @cmd_x` (a regex, not a glob) both became
non-buffer-only. Those are the documented `@ref` pipelines this classifier exists to
serve, so `contains` would have made the buffer workflow demand an acknowledgement on
ordinary use. `shell_words` has already stripped quotes by that point, so quoting cannot
be consulted — leading-sigil is the available proxy.

Residual, accepted and documented in the code: `rm -rf x$HOME` and `--out=$HOME/x` are
still not flagged. Neither is a regression (the first was never caught; the second is
excluded by the pre-existing flag-sigil rule), and both are contrived.
## Tests added

Three, in `src/tools/output_buffer.rs`:

- `is_buffer_only_false_for_shell_expanded_paths` — the bypass itself. Five commands that
  are destructive or source-reading against a real path *once the shell expands them*
  (`rm -rf ~/scratch @cmd_x`, `cat @cmd_x $HOME/.ssh/id_rsa`, `rm -rf $PROJECT_ROOT @cmd_x`,
  `cat @cmd_x *.rs`, `grep pattern @cmd_x ../src/main.rs`). A regression here reopens the
  gate skip, not merely a heuristic.
- `is_buffer_only_still_true_for_genuine_ref_only_commands` — the control, and it earned
  its place by failing: it caught the `contains` over-block described above. Eight
  commands including `awk '{print $0}'`, `grep '.*ERROR'`, `grep 'a*b'`, `sed -n '1,20p'`.
- `is_buffer_only_splits_quote_aware` — pins the divergence that unification closed: a
  quoted argument must not hide the real path after it.

Gate: `cargo fmt`; `cargo clippy --all-targets -- -D warnings` clean; `cargo test` 3562
passed / 0 failed / 44 ignored.
## Workarounds

Spell the path absolutely (`/home/you`) rather than `~` — that is already classified
correctly and takes the normal gate. The gate is also reachable deliberately via
`acknowledge_risk`.

## Resume

Fixed and green. Remaining: confirm CI on `experiments` at the commit containing this
change, then archive via `artifact(action="move", …)` — never a bare `git mv`. Label the
SHA `experiments`; the master-side SHA still needs recording after cherry-pick.

The sibling bug (`docs/issues/2026-08-08-security-layer-tokenizes-unlike-the-shell.md`) is
only **mitigated** — six `split_whitespace` helpers in `path_security.rs` are still on the
old string model. Do not close it on this fix.
## References

- `src/tools/output_buffer.rs:618` — the heuristic
- `src/tools/run_command/inner.rs:288`, `:306` — the two gates it skips
- `docs/issues/2026-08-08-security-layer-tokenizes-unlike-the-shell.md` — sibling defect
  in the same layer: the gate's tokenizer does not match the executing shell's
- PR #10 review, 2026-08-08
