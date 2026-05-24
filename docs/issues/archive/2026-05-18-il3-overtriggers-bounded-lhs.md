---
status: fixed
opened: 2026-05-18
closed: 2026-05-18
severity: medium
owner: marius
related: [2026-05-18-il3-pipe-violation-subagent]
tags: [il3, run_command, friction, rule-design]
kind: bug
---

# BUG: IL3 over-triggers on bounded LHS commands, forcing buffer-dance overhead

## Summary

`detect_il3_violation` blocks all `LHS | RHS` pairs where LHS is in an
allowlist (`ls`, `cat`, `grep`, `find`, ...) and RHS is a log-trimmer
(`head`, `wc`, `sort`, `sed`, ...). The rule does not distinguish
unbounded LHS (`cargo test`, `find /`) from bounded LHS (`ls <small
dir>`, `grep <pat> <one-file>`). For bounded outputs the mandated
buffer-then-query workflow is pure overhead — typically 2× the tool
calls for outputs that fit in tens of bytes. Friction is high enough
that summoned specialists ("Pika") spend most of their whistles on this
single pattern.

## Symptom (Effect)

Every call of the shape `ls <dir> | head -N`, `grep <pat> <file> | wc -l`,
`awk ... <file> | sort -u`, etc. is hard-blocked with:

```
IL3 violation — piped `<command>` to a log-trimmer. BLOCKED.

The @cmd_* buffer system saves context tokens:
  1. run_command("<lhs>")               — full output stored as @cmd_xxx
  2. grep PATTERN @cmd_xxx                 — query the buffer at any granularity
                                              (also: tail -20 @cmd_xxx, head -50 @cmd_xxx)

Promoted from warn to deny on 2026-05-18 after 50+ slips across 3 sessions.
Rerun the command bare and query the returned @cmd_* buffer.
```

Observed in one Pika-summoned session: 4 distinct trips in ~6 turns of
statusline debugging (`head -80` on a `grep` of `metadata.json`, `wc -l`
on `grep ~/.claude/sl-probe.log`, `sort -u` on a small `awk`, `sed -E`
on a small render output). All four LHS outputs were known to be
< 100 lines by construction.

## Reproduction

```bash
# Bounded: ls of a 6-entry directory. Blocked even though output is finite.
run_command "ls /home/marius/work/claude/code-explorer/.buddy/ | head -20"

# Bounded: grep of a single 200-line file. Blocked.
run_command 'grep "session_id" ~/.claude/sl-probe.log | wc -l'
```

Git commit: `d8086be2` (current `experiments` HEAD as of 2026-05-18).
Invocation: codescout MCP server (release build), any client.

## Environment

- OS: Linux 7.0.0-15-generic
- Rust: stable (per workspace `Cargo.toml`)
- MCP transport: stdio
- Project: codescout
- Branch: `experiments`
- Relevant code: `src/util/path_security.rs:534-584` (detection),
  `src/tools/run_command/mod.rs:196` (gate site),
  `codescout-companion/hooks/il3-deny-hook.sh` (mirrored client-side
  hook)

## Root cause

`IL3_LHS` regex at `src/util/path_security.rs:554-559` treats every
allowlisted command as unbounded:

```rust
Regex::new(
    r"^\s*(cargo|npm|pnpm|yarn|python|pytest|go|mvn|gradle|git|find|ls|grep|cat|diff|du|stat|rg|fd)\s.*\|\s*(tail|head|grep|less|wc|sed|awk|cut|sort|uniq|tr|fmt)\b",
)
```

The list conflates two populations:

- **Truly unbounded:** `cargo`, `npm`, `pnpm`, `yarn`, `pytest`, `mvn`,
  `gradle`, `find` (no `-maxdepth`), `rg`/`fd` against project root.
  Output regularly exceeds 10k lines — buffering is essential.
- **Often bounded by argument shape:** `ls <one-dir>`,
  `cat <one-file>`, `grep <pat> <one-file>`, `stat`, `du <one-path>`,
  `diff <two-files>`. Output rarely exceeds 200 lines; buffer-then-
  query doubles tool-call count for zero context savings.

The escape route (`@<buffer>_` token in pre-pipe segment) only helps
when a buffer already exists — first-touch bounded probes have no
buffer to reference, so the escape doesn't apply.

## Evidence

### Session transcript (this session, 2026-05-18)

User ran statusline debugging. Tool log shows 4 IL3 blocks in ~6
turns, every one on a bounded LHS:

| Turn | Command (excerpt)                                            | LHS output bound       |
|------|--------------------------------------------------------------|------------------------|
| 1    | `grep ... metadata.json \| head -80`                         | One 80kB JSON file     |
| 2    | `ls ~/.claude-kat/.../codescout-pika/ \| head` (Pika probe)  | 3-entry directory      |
| 3    | `grep ab012c18 ~/.claude/sl-probe.log \| wc -l`              | One 200-line log       |
| 4    | `awk ... ~/.claude/sl-probe.log \| sort -u`                  | One 200-line log       |

Each requires a 2-call recovery: bare LHS to materialize the buffer,
then `<rhs> @cmd_xxx` to query.

### Predecessor bug

`docs/issues/2026-05-18-il3-pipe-violation-subagent.md` (the *previous*
IL3 bug — archived as fixed in commit `2c3badfc`) motivated promoting
the rule from warn to deny. That fix closed a real subagent-context
hole; this bug is the cost side of the same change.

## Hypotheses tried

1. **Hypothesis:** Escape via `@<buffer>_` token covers the friction
   already. **Test:** inspected `IL3_BUFFER_REF` regex
   (`path_security.rs:560-563`); requires a buffer to *already exist*
   in the pre-pipe segment. **Verdict:** rejected — first-touch
   bounded probes (the dominant case) have no prior buffer.
2. **Hypothesis:** Agents should always run bare and query the buffer.
   **Test:** the Pika summon turn — 4 of 6 commands tripped IL3 on
   bounded LHS. User feedback: *"this is a lot of friction"*.
   **Verdict:** confirmed cost; rejected as a discipline fix —
   doubling the tool-call count on every bounded probe is a poor
   trade.
3. **Hypothesis (not tried, deferred):** Run-time output cap on the
   bare command would let the rule allow the pipe and clip output
   server-side. **Verdict:** deferred — requires plumbing through
   `run_command`'s buffer ceiling and risks silently truncating
   already-bounded outputs.

## Fix

Implemented fix candidate #1 (split LHS into bounded/unbounded) on
2026-05-18. Commit SHA pending until staged.

**Server-side (`codescout`):**

- `src/util/path_security.rs:534-617` — `detect_il3_violation` rewritten.
  New `is_unbounded_lhs()` + `has_recursive_flag()` helpers. RHS log-trimmer
  detection split from LHS unbounded-shape detection. Pre-pipe buffer-ref
  escape preserved.
- `src/util/path_security.rs` (tests module) — 12 new IL3 tests:
  `il3_allows_grep_single_file_pipe_sort` (renamed from
  `il3_blocks_similar_shape_without_buffer_ref` + assertion flipped),
  `il3_blocks_grep_recursive`, `il3_blocks_grep_capital_recursive`,
  `il3_blocks_grep_long_recursive`, `il3_blocks_find_no_maxdepth`,
  `il3_allows_find_with_maxdepth`, `il3_allows_cat_pipe_grep`,
  `il3_allows_ls_pipe_head`, `il3_allows_awk_file_pipe_sort`,
  `il3_allows_sed_file_pipe_head`, `il3_blocks_rg_pipe_head`,
  `il3_blocks_fd_pipe_wc`.
- `src/tools/run_command/tests.rs` — three integration tests renamed and
  reframed around `cargo test | grep` (unbounded sentinel):
  `il3_blocks_cargo_pipe_grep_via_run_command` (was
  `piped_grep_returns_unfiltered_ref`),
  `il3_blocks_chained_unbounded_pipe` (was
  `grep_no_match_suppresses_unfiltered_ref`),
  `il3_blocks_unbounded_pipe_pre_exec` (was
  `unfiltered_truncated_when_over_threshold`).
- `src/prompts/source.md` — Iron Law 3 text softened: "NO PIPING
  UNBOUNDED `run_command` OUTPUT TO LOG-TRIMMERS" with an explicit
  bounded-LHS allow paragraph. `server_instructions.md` surface only
  (live-loaded → no `ONBOARDING_VERSION` bump).
- `src/server.rs` (`prompt_surfaces_reference_only_real_tools` test) —
  allowlist extended with `awk`, `cargo`, `cat`, `diff`, `find`, `git`,
  `gradle`, `head`, `mvn`, `npm`, `pnpm`, `pytest`, `python`, `sort`,
  `stat`, `yarn` (shell-command tokens now backticked in the Iron Law 3
  block).
- `tests/fixtures/prompt_surfaces/server_instructions.md` — snapshot
  regenerated via `UPDATE_PROMPT_SNAPSHOTS=1`.

**Client-side (`claude-plugins/codescout-companion`):**

- `hooks/il3-deny-hook.sh` — rewritten to mirror the new
  bounded/unbounded split. Cheap reject on missing RHS log-trimmer;
  buffer-ref escape preserved; per-command unbounded classification via
  `case "$HEAD" in ... esac` with `grep -r` / `find -maxdepth` special
  cases.
- `hooks/il3-deny-hook.test.sh` — 12 new tests covering bounded-allow
  and unbounded-deny cases (test count: 10 → 22, all passing).

Verification:

- `cargo fmt && cargo clippy --all-targets -- -D warnings` — clean.
- `cargo test --lib` — 2421 passed, 7 ignored, 0 failed.
- `cargo build --release` — green.
- `bash codescout-companion/hooks/il3-deny-hook.test.sh` — 22/22 passing.
- Live MCP restart required (`/mcp`) before the release binary picks
  up the change.
## Tests added

Twelve new unit tests in `src/util/path_security.rs` (tests module)
covering both the new bounded-allow paths and the unbounded-deny
guardrails. Three reframed integration tests in
`src/tools/run_command/tests.rs` keep the run_command-path coverage
on `cargo test | grep` as the canonical unbounded sentinel. Plus 12
hook tests in `codescout-companion/hooks/il3-deny-hook.test.sh`
mirroring the same matrix client-side.
## Workarounds

- **Two-call dance:** run LHS bare to get `@cmd_xxx`, then
  `<rhs> @cmd_xxx` to query. Cost: doubled tool calls per probe.
- **Redirect to file:** `cmd > /tmp/out; head -N /tmp/out` — two
  bare calls, no pipe, not blocked. Cost: leaks `/tmp/` files,
  still two calls.
- **One-shot in bash builtins:** `bash -c '...; head'` — currently
  also blocked because the LHS regex matches the inner command.

## Resume

N/A — fixed. Commit on `experiments` pending. When ready to ship, follow
the Standard Ship Sequence in `CLAUDE.md`: cherry-pick to `master`,
push, rebase `experiments`, then `git mv` this bug file into
`docs/issues/archive/`.

The companion plugin change lives in a separate repo
(`/home/marius/work/claude/claude-plugins/`) and ships independently —
remember to commit both sides, and verify the regex on the shell side
stays in sync with `src/util/path_security.rs:534-617` on any future
edit.
## References

- `src/util/path_security.rs:534-584` — `detect_il3_violation`
- `src/tools/run_command/mod.rs:196` — IL3 gate site
- `src/tools/run_command/tests.rs:2600-2650` — existing IL3 tests
- `codescout-companion/hooks/il3-deny-hook.sh` — Claude-Code-side
  mirror
- `docs/issues/archive/2026-05-18-il3-pipe-violation-subagent.md` —
  predecessor bug (fixed by `2c3badfc`)
- `docs/trackers/codescout-usage-hookify.md` (H-1 entry, to be
  authored) — Pika promotion candidate for this rule refinement
