---
kind: bug
status: fixed
tags:
- iron-law
- il3
- run-command
- path-security
- bypass
closed: 2026-08-17
opened: 2026-08-17
owner: marius
related:
- docs/issues/archive/2026-08-17-heredoc-carve-out-defeated-by-a-pipe-in-the-body.md
severity: medium
---

# BUG: the source gate splits on `&&`, `||`, `;` and `|` but never on a newline, so a source read on the second line of a multi-line command is never seen

## Summary

`check_source_file_access` derives a segment's command from that segment's **first
token**, and the segment splitter breaks only on `&&`, `||`, `;` and `|`. A newline is
not a separator, so in a multi-line command only the first line's command is ever
considered. `echo hi\ncat src/main.rs` reads project source and is allowed.

Found while writing regression tests for
`docs/issues/archive/2026-08-17-heredoc-carve-out-defeated-by-a-pipe-in-the-body.md` — the first
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

**Fixed** — the steps below reproduce against `3ecb8730` or earlier. On `308014b5` and
later, step 2 blocks.

Branch `experiments`.

1. `cargo test --lib source_file_access_splits_on_newlines` — this test asserted the
   permissive behaviour on purpose until the gap was closed; it now asserts the fix.
2. `run_command("echo hi\ncat src/main.rs")` — refused after the fix, allowed before.
3. The `&&` control: `run_command("echo hi && cat src/main.rs")` was refused throughout,
   which is what isolates the separator as the only variable.
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
separately by `source_file_access_splits_on_newlines`.

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

**Fixed on `experiments`, commit `308014b5`** — promoted by **fast-forward**, so this is
also the master-side SHA and there is no second one to record. (`git rev-list
--left-right --count experiments...worktree-source-gate-newline-split` → `0 1` before the
move: `experiments` was deliberately left un-diverged this time, unlike the previous fix,
which had to be cherry-picked and re-cited.)

One line per gate, both as planned:

```rust
// check_source_file_access
let segments = split_outside_quotes(&stripped, &["&&", "||", ";", "|", "\n"]);

// pipeline_segments
split_outside_quotes(command, &["&&", "||", ";", "\n"])
```

**The sibling gate was the find worth having.** This file's plan said the IL-3 pipe gate
"should be reviewed for the same gap in the same pass". Reviewing it by test rather than by
reading showed the gap is there too — and with the opposite sign. In the source gate the
omission lets a read go unexamined; in `pipeline_segments` it is a false **negative**: with
a multi-line command collapsed into one segment, the pipe's real left-hand side is not the
segment's first token, so

```
echo hi
cargo test | grep FAILED
```

escaped IL-3 completely. A piped `cargo test` — the exact case the law exists for — was one
newline away from bypassing it.

**Both checks named in the plan came back clean, and neither needed code.**

1. *Heredoc interaction* — `strip_heredoc_bodies` runs before the split, so a body's
   newlines are already gone; the opener line survives and now separates correctly. Both
   heredoc tests stayed green.
2. *Quoted newlines* — `split_outside_quotes` carries quote state character by character
   across the whole string, so a newline inside `"…"` never split, and its escape branch
   consumes a backslash-newline continuation. That is what made the fix one line rather
   than a parser change, so it is now pinned **at the splitter** rather than inferred from
   gate behaviour.
## Tests added

Six, in `src/util/path_security.rs`. Two watched RED first — one per gate:

- `source_file_access_splits_on_newlines` — renamed from
  `source_file_access_does_not_split_on_newlines`, the tripwire the previous fix left
  asserting the permissive behaviour deliberately. Flipping it was the RED step, which is
  exactly what the tripwire was for.
- `il3_detects_a_piped_unbounded_command_on_a_later_line` — RED, and the discovery: the
  pipe gate had the same hole.

Four guards, green before and after, each covering a way a careless fix breaks:

- `source_file_access_does_not_split_a_newline_inside_a_quoted_argument`
- `il3_does_not_split_a_newline_inside_a_quoted_argument`
- `source_file_access_blocks_a_read_across_a_line_continuation` — a backslash-newline
  continuation is one command, and this one does read source.
- `split_outside_quotes_newline` and `split_outside_quotes_newline_inside_double_quotes` —
  unit tests at the splitter itself, asserting the property the one-line fix depends on
  instead of trusting it.

The whole pre-existing IL-3 and heredoc family (94 tests across the two filter groups)
stayed green, including `source_file_access_blocks_cat_rs_file_after_heredoc_segment` and
`il3_still_blocks_a_real_pipe_on_the_line_that_opens_a_heredoc`.

Gate: `cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean, `cargo test`
**4075 passed, 0 failed, 45 ignored** (4069 + 6).
## Workarounds

None needed from a caller's perspective — this gate being permissive costs the caller
nothing. For anyone relying on the gate as an enforcement boundary rather than a nudge:
it does not currently cover multi-line commands, and `acknowledge_risk` is not required
to get past it.

## Resume

N/A — closed. Fixed on `experiments` at **`308014b5`**, promoted by fast-forward, so no
pending-master-SHA line: `master` moves onto this exact commit and `308014b5` already *is*
the master SHA.

Wire verification still owed: not live in any running MCP server until `cargo rb` + `/mcp`.
The probes are `run_command("echo hi\ncat src/main.rs")` — must be refused — and
`run_command("git commit -m \"one\ncat src/main.rs\"")` — must run, proving the
quoted-newline guard holds outside the test harness too. Archive after that.

One stale reference deliberately left: the archived sibling
`docs/issues/archive/2026-08-17-heredoc-carve-out-defeated-by-a-pipe-in-the-body.md` still
names the pre-rename test. Archived files are historical snapshots — rewriting one to match
a later rename would falsify the record it exists to preserve.
## References

- `docs/issues/archive/2026-08-17-heredoc-carve-out-defeated-by-a-pipe-in-the-body.md` — the
  sibling defect in the same function, fixed in the same branch; this gap was surfaced
  by its tests.
- `docs/trackers/codescout-usage-frictions.md` — U-45 (the sibling, caller's side).
- `src/util/path_security.rs` — `check_source_file_access`, `split_outside_quotes`,
  `shell_tokens`.
