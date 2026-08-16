---
id: '409ba8bb92cb559c'
kind: bug
status: fixed
title: 'BUG: IL3 splits a pipeline on a bare `|`, so a quoted pipe inside an argument reports a violation on a command that has no pipe'
tags:
- security
- run_command
- il3
- tokenizer
closed: 2026-08-14
opened: 2026-08-08
owner: marius
related: []
severity: high
---

# BUG: IL3 splits a pipeline on a bare `|`, so a quoted pipe inside an argument reports a violation on a command that has no pipe

## Summary

`il3_offending_lead` splits a command segment with `segment.split('|')`, which is
quote-blind. A `|` inside a quoted argument therefore manufactures pipeline stages that
the shell will never create, and if the fabricated stage's first word happens to name a
trimmer, IL3 blocks a command containing no pipe at all. The seventh divergent string
model in a layer whose other six were just unified.

## Symptom (Effect)

```
run_command("git log --grep='fix|head foo' --oneline -3")
```

```
IL3 violation — piped `git log --grep='fix|head foo' --oneline -3` to a log-trimmer. BLOCKED.

The @cmd_* buffer system saves context tokens:
  1. run_command("git log --grep='fix")               — full output stored as @cmd_xxx
```

Two things are wrong. The command has no pipe — `|` sits inside a single-quoted argument,
and `git log` receives `fix|head foo` as one `--grep` pattern. And the remediation the
error offers, `git log --grep='fix`, is not a runnable command: it carries an unterminated
quote, so following the hint produces a shell parse error.

## Reproduction

```bash
git rev-parse HEAD          # 2b1c9ec6 or later on experiments
```

Through the MCP surface:

```
run_command("git log --grep='fix|head foo' --oneline -3")
```

Blocked. `run_command("git log --grep='fix-head foo' --oneline -3")` — same command with
the `|` removed — runs.

## Environment

Linux, `experiments`, codescout 0.15.0. Platform-independent: the split is plain Rust
string handling, not shell behaviour.

## Root cause

`il3_offending_lead` (`src/util/path_security.rs`) does its own pipeline split:

```rust
let mut stages = segment.split('|');
let pre_pipe = stages.next().unwrap_or("");
```

`measured 2026-08-08:` the reproduction above, run through the live MCP server on
`2b1c9ec6` — blocked, with the truncated hint quoted verbatim in *Symptom*.

The caller already does the quote-aware thing one level up: `detect_il3_violation` splits
on `;` / `&&` / `||` via `pipeline_segments`, and the sibling `check_source_file_access`
splits on all four operators via `split_outside_quotes`. So the file contains a
quote-aware pipeline splitter already; this site does not use it.

For `git log --grep='fix|head foo'` the naive split yields two stages:

| stage | what happens |
|---|---|
| `git log --grep='fix` | `is_unbounded_lhs` → tokenize fails (unclosed quote) → falls back to `split_whitespace` → head `git` → **unbounded** |
| `head foo'` | `stage_trims` → tokenize fails → falls back → head `head` → **trimmer** |

Unbounded LHS piped to a trimmer → violation. Both halves reach their verdict through
`shell_tokens`' fallback path, which is correct in isolation — the fallback exists so an
unclosed quote never *skips* a check — but here the unbalanced fragments are manufactured
by the caller, not supplied by the user.

**Not a regression.** `split('|')` predates the 2026-08-08 tokenizer work; the
`stage_trims` / `is_unbounded_lhs` conversions changed how each fragment is read, not how
the fragments are produced. Verify before assuming otherwise:
`git log -S "segment.split('|')" -- src/util/path_security.rs`.

## Evidence

### The quote-aware splitter is already in the file

`check_source_file_access` calls
`split_outside_quotes(command, &["&&", "||", ";", "|"])`, with a comment explaining that
`&&` / `||` must be consumed before `|` so `||` is not mis-split. `il3_offending_lead`
is called per-segment *after* `pipeline_segments` has consumed `;` / `&&` / `||`, so it
needs only the `|` case.

### Why the false positive needs a trimmer name after the quoted pipe

`echo 'a|head'` does NOT trip it: the fabricated stage is `head'`, and with the tokenize
fallback the token keeps its apostrophe, so it matches no trimmer name. The failing shape
needs whitespace after the fabricated head — `'fix|head foo'` — so that `split_whitespace`
yields a clean `head`. That narrowness is why it went unnoticed.

## Hypotheses tried

1. **Hypothesis:** the 2026-08-08 `shell_tokens` conversion introduced this.
   **Test:** read `il3_offending_lead`; its `split('|')` is untouched by that commit
   (`dbaeb78b` and the follow-up both leave it alone).
   **Verdict:** rejected — pre-existing.

## Fix

Fixed 2026-08-14 on `experiments`. Both splitters in `src/util/path_security.rs` are
now quote-aware:

- `il3_offending_lead`: `segment.split('|')` → `split_outside_quotes(segment, &["|"])`,
  return type `Option<&str>` → `Option<String>`.
- `pipeline_segments`: hand-rolled quote-blind byte scan →
  `split_outside_quotes(command, &["&&", "||", ";"])`, return `Vec<&str>` → `Vec<String>`.
- `detect_il3_violation`: adapted to owned segments (it already did
  `.trim().to_string()`, so the borrow bought nothing — as this file predicted).

### This bug's premise was wrong, and the error hid a bypass

*Root cause* said:

> The caller already does the quote-aware thing one level up: `detect_il3_violation`
> splits on `;` / `&&` / `||` via `pipeline_segments` …

`pipeline_segments` was **not** quote-aware. It scanned raw bytes with zero quote
tracking — read it and there is no `in_single` / `in_double` state anywhere. Only
`check_source_file_access`'s use of `split_outside_quotes` was quote-aware. So the
same defect existed in two functions, and this file recorded one of them as the
solution to the other.

That mattered, because the second instance is not a false positive. **It is a
bypass.** Measured 2026-08-14 through the live MCP server, before any change:

```
run_command("git log --oneline -50 --grep='a;b' | head -3")
  -> {"exit_code": 0}
```

An unbounded producer piped to a trimmer — a textbook IL3 violation — **allowed**.
The quoted `;` split the command into `git log --oneline -50 --grep='a` and
`b' | head -3`. The second segment's pre-pipe lead is the fragment `b'`, which is not
an unbounded command, so the check passed and the pipe went through.

Any quoted separator does it: `;`, `&&`, `||`. Prefix a real pipe with one and the
enforcer stops seeing the pipe.

### What made it visible

The companion hook and the server gate **disagreed**. Running the repro, the
advisory hook printed `IL3 warning — piped … to a log-trimmer` while the server —
the actual enforcer — returned `exit_code: 0`. The hook uses a separate
implementation that happens to handle the quoted `;` correctly. Without that
disagreement there was no signal at all: a bypass produces a *successful command*,
which looks exactly like correct behaviour.

### Severity raised low→high

Filed `medium` as a false positive. A false positive costs a retry and prints a
confusing hint. A false negative costs the guarantee the control exists to provide,
and is silent by construction.

### The `||` question, settled

*Fix* asked whether to rely on `pipeline_segments` consuming `||` or re-handle it
locally. **Rely on it** — but the answer only became correct once `pipeline_segments`
was itself fixed:

- an **unquoted** `||` is consumed upstream, so it never reaches `il3_offending_lead`;
- a **quoted** `||` reaches it, and `split_outside_quotes(segment, &["|"])` correctly
  leaves it alone because it is inside quotes.

Passing `&["||", "|"]` would have been *wrong*, not merely redundant: `||` is a
logical-or, so splitting on it would make the RHS look like a pipe stage and turn
`cargo build || head -3 log.txt` into a violation. Pinned by test rather than comment,
since the invariant is held by a caller.

### Cross-repo half — not fixed here

The companion's IL3 implementation has the **false positive** (it warned on
`git log --grep='fix|head foo'`) though not the bypass. It needs the same
quote-awareness. Deliberately not touched: `claude-plugins` has another session
mid-release, and companion ships from a version-keyed cache, so a source edit is
inert until `1.16.4 → 1.16.5` plus a reinstall across all three profiles. That bump
is the operator's call and this fix should ride with it.
## Tests added

Four, in `src/util/path_security.rs`:

- **`il3_allows_a_quoted_pipe_inside_an_argument`** — the filed false positive, in
  single and double quotes, **plus** the control that a quoted pipe followed by a real
  one still blocks. Without that third assertion the fix could have been a blanket
  exemption and the test would not have noticed.
- **`il3_does_not_treat_the_rhs_of_a_logical_or_as_a_pipe_stage`** — pins the caller-held
  invariant that makes `&["|"]` sound.
- **`il3_still_blocks_when_a_quoted_separator_precedes_a_real_pipe`** — the bypass, for
  all three separators.
- **`il3_control_plain_violations_still_fire_after_quote_awareness`** — six plain
  violations. The change strictly reduces fabricated stages, so the risk it introduces
  is under-blocking; this is the arm that catches over-correction.

**Verified the bypass test can fail.** Reverted `pipeline_segments` to a quote-blind
`split(';')` and re-ran:

```
il3_still_blocks_when_a_quoted_separator_precedes_a_real_pipe ... FAILED
  a quoted `;` must not hide a real pipe from the check
```

The control test kept passing under that mutation, which is the point of having both:
one arm catches under-blocking, the other over-blocking, and a mutation that trips only
one of them is correctly diagnosed.

Gate: **3714 passed / 0 failed / 44 ignored** (3710 + these 4, reconciling exactly),
`clippy --all-targets -D warnings` clean.

Two doc comments that cited the naive split as a *live* hazard were corrected in the
same change — `shell_tokens` in the same file (it claimed the tokenizer fallback is
hit "on every call" *because* of the naive split) and `posix_tokenize` in
`src/platform/mod.rs` (which listed `il3_offending_lead` among the things that do not
agree with the shell). Both would have been false the moment this landed, and the
latter carries an explicit warning that it has already been wrong twice in opposite
directions.

### Live-surface verification, 2026-08-14 (post `cargo rb` + `/mcp` reconnect)

Both arms confirmed against the running MCP server, not just the unit suite:

| Command | Before | After |
|---|---|---|
| `git log --grep='fix\|head foo' --oneline -3` | BLOCKED (false positive) | **ran, `EXIT=0`** |
| `git log --oneline -50 --grep='a;b' \| head -3` | ran, `exit_code: 0` (**bypass**) | **BLOCKED** |

The remediation hint is now runnable too, which was the second half of the original
complaint. It prints `run_command("git log --oneline -50 --grep='a;b'")` — balanced quotes
— where the pre-fix hint printed `git log --grep='fix`, an unterminated quote that would
have produced a shell parse error if followed.

**The companion hook still emits the false positive.** Observed in the same run: the
server allowed the quoted-pipe command while the advisory hook warned about it. That is
the expected split — companion ships from a version-keyed cache at 1.16.4 and a source
edit there is inert until the bump. It also re-demonstrates the property that made the
bypass findable at all: when the two implementations disagree, the disagreement is the
signal.
## Workarounds

Re-invoke with the returned `@ack_*` handle, or avoid `|` inside quoted arguments to
`git`/`cargo`/`rg` — e.g. use two `--grep` flags instead of an alternation.

## Resume

N/A on the codescout side — fixed and verified, including the bypass this file's
premise denied existed.

**One thing remains, cross-repo:** the companion's IL3 implementation still has the
false positive. It rides with the `codescout-companion 1.16.4 → 1.16.5` bump and the
three-profile reinstall, which is the operator's call — a source edit there is inert
until the cache is refreshed. Do not re-open this file for it; it is a claude-plugins
change.
## References

- `src/util/path_security.rs` — `il3_offending_lead` (the naive split),
  `check_source_file_access` (the quote-aware sibling), `shell_tokens` (the fallback that
  makes both fragments answer confidently)
- `docs/issues/archive/2026-08-08-security-layer-tokenizes-unlike-the-shell.md` — the parent bug;
  its six named helpers are fixed, and this is the site that outlived the list
- `docs/issues/archive/2026-08-08-buffer-only-gate-misses-tilde-and-home.md` — the fourth
  model, fixed
