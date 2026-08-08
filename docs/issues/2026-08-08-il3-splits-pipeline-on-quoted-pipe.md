---
id: b86d81f12983f566
kind: bug
status: open
title: 'BUG: IL3 splits a pipeline on a bare `|`, so a quoted pipe inside an argument reports a violation on a command that has no pipe'
tags:
- security
- run_command
- il3
- tokenizer
closed: null
opened: 2026-08-08
owner: marius
related: []
severity: medium
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

Not implemented. Plan: replace `segment.split('|')` with
`split_outside_quotes(segment, &["|"])`, matching the sibling. Two things to settle first,
neither obvious:

1. **`||` cannot reach this site** — `pipeline_segments` consumed it upstream — but that is
   an invariant held by a *caller*, so pin it with a test rather than a comment, or pass
   `&["||", "|"]` and let the ordering rule handle it locally.
2. **`split_outside_quotes` returns owned `String`s**, while `il3_offending_lead` returns
   `Option<&str>` borrowed from `segment`. Same signature change `extract_grep_pattern`
   took; the caller does `.trim().to_string()` immediately, so the borrow buys nothing.

Direction of the behaviour change: strictly fewer IL3 blocks. A quoted `|` stops
fabricating stages; nothing new starts being blocked. That is the opposite of the
`check_source_file_access` conversion, so it needs a control asserting that genuine
violations still fire — `cargo test | head -50` must stay blocked.

## Tests added

None — not fixed. When fixed, the discriminating test is the reproduction above
(`git log --grep='fix|head foo'` → `None`) plus a control that `cargo test | head -50`
still yields a violation.

## Workarounds

Re-invoke with the returned `@ack_*` handle, or avoid `|` inside quoted arguments to
`git`/`cargo`/`rg` — e.g. use two `--grep` flags instead of an alternation.

## Resume

Edit `il3_offending_lead` in `src/util/path_security.rs`: swap `segment.split('|')` for
`split_outside_quotes(segment, &["|"])`, change the return type to `Option<String>`, and
drop the now-unneeded borrow in `detect_il3_violation` (it already calls
`.trim().to_string()`). Then add the two tests named under *Tests added*. Settle the `||`
question first — read `pipeline_segments` and decide whether to rely on it or re-handle
`||` locally.

## References

- `src/util/path_security.rs` — `il3_offending_lead` (the naive split),
  `check_source_file_access` (the quote-aware sibling), `shell_tokens` (the fallback that
  makes both fragments answer confidently)
- `docs/issues/2026-08-08-security-layer-tokenizes-unlike-the-shell.md` — the parent bug;
  its six named helpers are fixed, and this is the site that outlived the list
- `docs/issues/archive/2026-08-08-buffer-only-gate-misses-tilde-and-home.md` — the fourth
  model, fixed

