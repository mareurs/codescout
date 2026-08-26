---
id: 7f18aaf9d68c0b24
kind: bug
status: fixed
title: 'BUG: the IL-3 limiter table matches whole tokens, so git''s attached-value flag forms (--porcelain=v1, --stat=200, -n5) are refused as unbounded'
tags:
- il3
- path-security
- gate
- false-positive
closed: 2026-08-26
opened: 2026-08-26
owner: marius
related: []
severity: low
---

# BUG: the IL-3 limiter table matches whole tokens, so git's attached-value flag forms (`--porcelain=v1`, `--stat=200`, `-n5`) are refused as unbounded

## Summary

`git_output_is_bounded` decides whether a piped `git` command carries an output
limiter by comparing each argument token for **exact equality** against a fixed
list. Git accepts most of those flags in an attached-value form as well
(`--porcelain=v1`, `--stat=200`, `-n5`), and those forms match nothing in the
list, so a genuinely bounded command is refused as an IL-3 violation.
`--max-count=` is the sole form handled, via a hard-coded `starts_with`.

The cost is a false refusal on legal commands, which the function's own doc
comment names as the cheaper failure direction — but this is the noise-training
kind, and the refusal message contradicts itself on one of the four cases.

## Symptom (Effect)

`git log -n5 | head -3` is refused, and the refusal text offers `git log -3` as
a correct alternative — the same limit, spelled the other way git documents:

```
IL3 violation — piped `git log -n5 | head -3` to a log-trimmer. BLOCKED.
...
`git` is unbounded ONLY without an output limiter: `git log -3`,
`git status --short`, `git show --stat` are bounded and may be piped;
`--oneline` is not a limiter (it bounds width, not line count).
```

Same shape for `--porcelain=v1`, `--porcelain=v2` and `--stat=200`.

## Reproduction

Commit `9eb0dd62` on `experiments`. Each pair differs only in how the limiter is
spelled; every command on both sides is valid git and genuinely bounded.

| Command | Gate verdict |
|---|---|
| `git status --porcelain \| head -5` | allowed |
| `git status --porcelain=v1 \| head -5` | **BLOCKED** |
| `git status --porcelain=v2 \| head -3` | **BLOCKED** |
| `git status --short \| head -5` | allowed |
| `git log --max-count 3 \| head -5` | allowed |
| `git log --max-count=3 \| head -5` | allowed (hard-coded `starts_with`) |
| `git log -n 5 --format='%h' \| head -3` | allowed |
| `git log -n5 \| head -3` | **BLOCKED** |
| `git show --stat=200 HEAD \| head -3` | **BLOCKED** |

All measured 2026-08-26 by issuing each through `run_command`.

**The blocked forms are valid, bounded git** — verified separately by running each
bare, since "blocked by the gate" is not evidence about git's own syntax:

- `git log -n5 --format='%h'` → exit 0, exactly 5 lines.
- `git show --stat=200 --format='%h' HEAD` → exit 0.
- `git status --porcelain=v1 --untracked-files=normal` → exit 0.
- `git status --porcelain=v2 --branch` → exit 0.

## Environment

codescout `0.15.0`, `experiments` @ `9eb0dd62`, Linux, MCP stdio, server pid
3691039 started 23:35:01 (post-rebuild, so the served binary is current — the
`include_str!`/process-start staleness axis is ruled out for this observation).

## Root cause

`src/util/path_security.rs:1208-1227`, `git_output_is_bounded`. The limiter test
is a `matches!` over `tok.as_str()` — **whole-token equality**:

```rust
matches!(
    tok.as_str(),
    "-n" | "--max-count"
        | "--show-current"
        | "--porcelain" | "--short" | "-s"
        | "--stat" | "--name-only" | "--name-status"
) || tok.starts_with("--max-count=")
    || (tok.len() >= 2
        && tok.starts_with('-')
        && tok[1..].chars().all(|c| c.is_ascii_digit()))
```

Two escape hatches exist and both are narrow: `starts_with("--max-count=")`
covers exactly one flag's attached form, and the digit rule covers `-3` but not
`-n5` (for `-n5`, `tok[1..]` is `"n5"`, which is not all digits).

So the defect is not a missing flag — every affected flag is already in the
table. It is that the table is consulted with an equality test while git's CLI
grammar admits `--flag=value` for long options and attached values for short
ones. Measured 2026-08-26, read from source and confirmed against the gate by
the verdict table above.

## Evidence

### The discriminator is the form, not the flag

`--porcelain` passes and `--porcelain=v1` blocks; `--stat` passes and
`--stat=200` blocks; `-n 5` passes and `-n5` blocks. In each pair the flag is
identical and present in the table, so nothing but the spelling changed.

### `--max-count` is the control

`--max-count=3` is the one attached form that passes, and it is the one with a
dedicated `starts_with`. That the exception exists at all shows the shape was
known for one flag and not generalised to its siblings.

### The message contradicts itself on `-n5`

The refusal offers `git log -3`. `-n5` and `-3` request the same bound; git
documents both. This is the U-44 failure recurring in the surviving
implementation: U-44 was the *companion hook* asserting a rule its own message
denied, and its resolution deleted the hook so that `path_security.rs` would be
the sole implementation. U-44's own "Fix idea" enumerated the carve-out as
``-N``, ``--max-count``, ``--porcelain``, ``--short``, ``--stat``, ``-n`` —
bare flags only — so the gap was carried into the specification, not introduced
after it.

## Hypotheses tried

1. **Hypothesis:** the gate takes a whole `;`-chain as the pipe's left-hand side,
   so an earlier unbounded command in the chain poisons a bounded final one.
   **Test:** ran `git status --porcelain=v1 | head -5` with no chain.
   **Verdict:** rejected — blocked identically. The chain was incidental.
2. **Hypothesis:** any `=`-valued limiter is unmatched.
   **Test:** `git log --max-count=3 | head -5`.
   **Verdict:** rejected as stated — `--max-count=` is handled. Refined to: any
   attached-value form *other than* `--max-count=`, plus the `-nN` short form.
3. **Hypothesis:** already fixed on `worktree-il3-gate-and-find-lift`, which
   carries +101/-7 on this file.
   **Test:** `git diff experiments...worktree-il3-gate-and-find-lift -- src/util/path_security.rs`,
   grepped for `porcelain|max-count|starts_with|stat=`.
   **Verdict:** rejected — the only hit is a bug-file citation in a comment. That
   branch's changes are the heredoc/newline split work, not the limiter table.
4. **Hypothesis:** already filed as U-44.
   **Test:** read `codescout-usage-frictions:U-44` in full.
   **Verdict:** rejected — different component (companion `il3-warn-hook.mjs`,
   since deleted) and different mechanism (flat alternation with no limiter check
   at all, vs. a limiter check that misses a spelling). Related, not duplicate.

## Fix

Fixed in `git_output_is_bounded` (`src/util/path_security.rs`). Two changes, and
the first subsumes rather than extends the special case:

- Compare against the **flag name** — `tok.split_once('=')` — instead of the whole
  token, so `--porcelain=v1` matches on `--porcelain`. This replaces the
  hard-coded `starts_with("--max-count=")`; a fourth special case was the
  alternative.
- Strip a leading `-n` before the digit-shorthand test, so `-n5` is not tested as
  the bare `-<digits>` form and rejected on the `n`. A bare `-n` yields an empty
  remainder, fails there, and is caught by the table.

**SHA:** `058d8683` (`experiments`)
**patch-id:** `ca87ff5592bfed0f0d5f147114e7a6f59da46d13`

### The deferral rationale recorded here was false, and was falsified within the hour

This section previously read *"Deliberately left unimplemented in this session:
`worktree-il3-gate-and-find-lift` holds +101/-7 on this exact file, unmerged since
2026-08-17, and a competing edit buys a conflict on a gate whose test suite is
load-bearing."* Every clause of that was true except the one that mattered.

The branch holds exactly **one** commit not in `experiments`, and
`git cherry -v experiments worktree-il3-gate-and-find-lift` marks it `-` — an
equivalent patch is already in `experiments`. `strip_heredoc_bodies` and
`mask_heredoc_bodies` are present in `experiments`' `path_security.rs`, citing a
different commit (`dbaeb78b`), and the bug that branch fixes
(`docs/issues/archive/2026-08-17-source-gate-does-not-split-on-newlines.md`) is
`status: fixed` and archived. `experiments` is **577** commits ahead of the branch,
so the two-dot diff is 15,597 deletions against 2,038 insertions: merging it would
have *removed* work, not added any. There was no conflict to buy.

The `+101/-7` figure was not wrong — it is the three-dot diff, i.e. the content of
that one commit measured from the merge-base. It was the *inference* from it that
was wrong: a large diff against the fork point reads as unmerged work, and says
nothing about whether the work already arrived by another route.

Recorded because it is a clean instance of R-95 (`reconnaissance-patterns`): a
deferral rationale is a claim, and the least-audited kind — a wrong root cause is
corrected the moment you fix the bug, while a wrong deferral is never revisited,
because its whole function is to stop anyone looking. The bias has a direction:
nobody drafts an estimate that makes the work sound easier, because that estimate
would not justify stopping. Here the true cost was one function and one test.
The check that broke it — `git cherry`, which compares by patch equivalence rather
than by SHA — took one command.
## Tests added

`il3_limiter_matches_gits_attached_value_spellings`
(`src/util/path_security.rs:3438`), placed beside
`il3_allows_git_status_porcelain`.

It asserts the **pair**, deliberately. Five attached forms
(`--porcelain=v1`, `--porcelain=v2`, `--stat=200`, `--max-count=3`, `-n5`) must
read as bounded; the five bare spellings they are equivalent to must still read as
bounded; and three cases must still be **refused** — a limiter-free `git log`,
`--oneline` (bounds width, not line count), and `--pretty=format:%h`.

The third group is what gives the first two any force. Every allow-assertion above
would also pass in a world where the gate stopped inspecting `git` entirely, so an
allow-only test would be green and uninformative. `--pretty=format:%h` specifically
guards the new `=` split: it proves splitting on `=` does not admit any flag that
merely carries a value.

Whole gate at the fix commit: `cargo fmt` clean, `cargo clippy --workspace
--all-targets -D warnings` clean, `cargo test --workspace --all-targets` **4577
passed / 0 failed** (up one, this test).
## Workarounds

Spell the limiter in its detached or bare form: `--porcelain` for
`--porcelain=v1`, `-n 5` or `-3` for `-n5`, `--stat` for `--stat=200`. Or run the
command bare and query the returned `@cmd_*` buffer, which is what the refusal
already recommends and costs one extra call.

## Resume

N/A — fixed and verified on `experiments`.

One caveat that is about the observer, not the fix: the unit tests prove the
predicate, but the **live** MCP gate still runs the binary this session's server
started from (pid 3691039, 23:35:01), which predates this commit. `include_str!`
and process-start freezing mean a running server cannot pick the change up — see
R-89. Confirming the refusals are gone at the tool surface needs `cargo rb` then
`/mcp`, and until then the honest claim is "committed and unit-verified", not
"live".
## References

- `src/util/path_security.rs:1208-1227` — `git_output_is_bounded`
- `src/prompts/mod.rs:569` — the surface text stating which flags are limiters;
  it lists the bare forms only, so it is accurate about the code and silent
  about the gap.
- `codescout-usage-frictions:U-44` — the same self-contradiction class in the
  companion hook, fixed by deleting the hook.
- `docs/issues/archive/2026-05-18-il3-overtriggers-bounded-lhs.md` — the original
  bounded/unbounded LHS split that put `git` wholesale on the wrong side.
- `docs/trackers/2026-08-16-iron-law-gate-firing-audit.md` — GF-1 / GF-2, the
  94-refusal measurement that motivated `git_output_is_bounded`.
