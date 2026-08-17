---
status: open
opened: 2026-08-17
closed:
severity: medium
owner: marius
related: ['docs/issues/2026-08-17-heredoc-carve-out-defeated-by-a-pipe-in-the-body.md']
tags: [iron-law, il3, run-command, path-security, bypass]
kind: bug
---

# BUG: the source gate splits on `&&`, `||`, `;` and `|` but never on a newline, so a source read on the second line of a multi-line command is never seen

## Summary

`check_source_file_access` derives a segment's command from that segment's **first
token**, and the segment splitter breaks only on `&&`, `||`, `;` and `|`. A newline is
not a separator, so in a multi-line command only the first line's command is ever
considered. `echo hi\ncat src/main.rs` reads project source and is allowed.

Found while writing regression tests for
`docs/issues/2026-08-17-heredoc-carve-out-defeated-by-a-pipe-in-the-body.md` — the first
draft of a here-string test failed for this reason rather than the one it was written
for, which is what surfaced the gap.

## Symptom (Effect)

Measured 2026-08-17 at `6c174deb`, via the unit helper `check_source_file_access_at_root`:

```
check_source_file_access_at_root("echo hi\ncat src/main.rs")   → None      (allowed)
check_source_file_access_at_root("echo hi && cat src/main.rs") → Some(hint) (blocked)
```

Same read, same file. The verdict turns on whether the separator is a newline or `&&`.

## Reproduction

Branch `worktree-il3-gate-and-find-lift` (from `experiments` at `6c174deb`).

1. `cargo test --lib source_file_access_does_not_split_on_newlines` — passes, and that
   test asserts the permissive behavior deliberately.
2. Read its body: it documents this gap and points here.
3. To see it live: `run_command("echo hi\ncat src/main.rs")` from a session with the
   gate armed.

## Environment

codescout MCP server, `run_command`, IL-3 source gate.
`src/util/path_security.rs`. Rust.

## Root cause

Measured 2026-08-17 by the pair above; mechanism read at `6c174deb`.

`check_source_file_access` (`src/util/path_security.rs`) splits with:

```rust
let segments = split_outside_quotes(&stripped, &["&&", "||", ";", "|"]);
```

then, per segment:

```rust
let first_token = shell_tokens(seg).into_iter().next().unwrap_or_default();
if !cmd_re.is_match(&first_token) {
    return false;
}
```

For `"echo hi\ncat src/main.rs"` there is one segment. Its first token is `echo`, which
is not a blocked reader, so the segment is dismissed before the extension half runs —
and `cat src/main.rs`, sitting on line two of the same segment, is never treated as a
command.

The first-token rule is itself correct and deliberate: it was added to stop false
positives from quoted arguments that merely contain command names
(`git commit -m "… output_buffer.rs"`). The gap is that a newline is as much a command
separator in shell as `;` is, and the splitter does not know it.

## Evidence

### The `&&` control

Swapping the newline for `&&` blocks the identical read, which isolates the separator as
the only variable and rules out the extension matcher, the project-membership check, and
the heredoc path.

### Where it was surfaced

`source_file_access_here_string_does_not_swallow_a_following_read` was first written as
`"cargo test <<< word\ncat src/main.rs"` and expected to block. It did not. The
diagnosis was that no `|` meant no split, so the segment's command was `cargo` — nothing
to do with here-strings. The test was rewritten with a pipe, and this gap was pinned
separately by `source_file_access_does_not_split_on_newlines`.

## Hypotheses tried

1. **Hypothesis:** the here-string `<<` was causing the segment to be skipped.
   **Test:** removed the here-string, kept the newline — still allowed.
   **Verdict:** rejected. The `<<` skip was a *separate* real defect (fixed in the
   sibling bug), but it is not what made this case pass.
   **Evidence link:** *Where it was surfaced*.

2. **Hypothesis:** the extension matcher misses `src/main.rs` in a multi-line string.
   **Test:** the `&&` control, which keeps the string multi-token but changes only the
   separator; it blocks.
   **Verdict:** rejected — the extension half never runs, because the first-token check
   dismisses the segment first.
   **Evidence link:** *The `&&` control*.

## Fix

Not yet implemented. Add `"\n"` to the separator list:

```rust
let segments = split_outside_quotes(&stripped, &["&&", "||", ";", "|", "\n"]);
```

Ordering does not matter for `\n` (it shares no prefix with the others). Two things to
check before shipping it, both of which are why this is filed rather than fixed inline:

1. **Heredoc interaction.** `strip_heredoc_bodies` already removes bodies before the
   split, so a body's newlines are gone. But the *opener* line survives, so splitting on
   newlines will now separate `git commit -F - <<'MSG'` from whatever follows the closed
   heredoc. That is correct, and
   `source_file_access_allows_cat_heredoc_with_source_ext_in_content` plus
   `source_file_access_allows_a_commit_message_quoting_a_pipe_alternation` should both
   still pass — confirm rather than assume.
2. **False-positive risk.** A newline inside a quoted argument must not split. That is
   what `split_outside_quotes` is for, but it has never been exercised on `\n`; add a
   case for a multi-line quoted string containing a source filename.

The IL-3 pipe gate (`detect_il3_violation`) should be reviewed for the same gap in the
same pass.

## Tests added

`source_file_access_does_not_split_on_newlines` (`src/util/path_security.rs`) — asserts
**today's permissive behavior** on purpose, with a comment saying so and pointing here.
Closing the gap will break it, which is the intent: it is a tripwire, not an
endorsement.

Still needed when the fix lands:
- flip that test to expect `Some`;
- `source_file_access_does_not_split_on_a_newline_inside_a_quoted_argument` — the
  false-positive guard, which is the one a careless fix breaks;
- re-run the two heredoc tests named in *Fix* §1 as the interaction check.

## Workarounds

None needed from a caller's perspective — this gate being permissive costs the caller
nothing. For anyone relying on the gate as an enforcement boundary rather than a nudge:
it does not currently cover multi-line commands, and `acknowledge_risk` is not required
to get past it.

## Resume

Apply the one-line separator change above, flip
`source_file_access_does_not_split_on_newlines` to expect `Some`, and add the
quoted-newline guard. Then check `detect_il3_violation` for the same splitter gap — if it
shares the separator list, the fix is one place; if it has its own, both need it.

## References

- `docs/issues/2026-08-17-heredoc-carve-out-defeated-by-a-pipe-in-the-body.md` — the
  sibling defect in the same function, fixed in the same branch; this gap was surfaced
  by its tests.
- `docs/trackers/codescout-usage-frictions.md` — U-45 (the sibling, caller's side).
- `src/util/path_security.rs` — `check_source_file_access`, `split_outside_quotes`,
  `shell_tokens`.
