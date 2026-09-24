# rtk (rtk-ai/rtk) evaluated for codescout — 2026-09-24

**Verdict: do not adopt rtk as a wrapper around codescout's `run_command`.** Measured gain is
under 1% of codescout tool output, and it defeats two codescout guards. The one idea worth
porting has been ported natively (see the end). Team report:
`docs/research/2026-09-24-rtk-evaluation.pdf` (generator `…-report.py` beside it).

## What rtk is (v0.49.0, 2026-09-11, Apache-2.0, Rust)
A CLI output filter. `rtk init -g` installs a Claude Code PreToolUse hook that rewrites **Bash**
commands (`git status` → `rtk git status`). The hook ignores MCP tools, and native Bash is
denied on all three profiles since 2026-09-20, so **the hook never fires in this setup**. The
only way in would be a custom hook that calls `rtk rewrite "$cmd"` and rewrites the input to
`mcp__codescout__run_command`. Useful subcommands: `rewrite`, `pipe -f <filter>` (filters
stdin, so stored outputs can be replayed offline), `discover`, `recall <hash>`.

## Measurements (usage.db, 2026-08-25 → 2026-09-24, 206 run_command sessions)
- run_command returned 30.5 MB, which is 20% of all codescout tool output (154 MB).
- `rtk rewrite` coverage: 60.9% of calls (14,401 of 23,641) and 63.3% of bytes would be rewritten.
- Offline `rtk pipe` replay of INLINE, single-segment outputs: 1.94 MB → 0.97 MB (−50%). That
  is ~242K tokens/month, **0.63% of all codescout tool output** and ~1.2K tokens per session.
  cargo-test −88% (n=319) is reliable, because live `rtk cargo test` behaves the same. The
  git-log −73% and git-diff −30% figures are UPPER BOUNDS: live `rtk git log --format=…` passes
  output through almost untouched (−1%), and pipe mode ignores the user's format flags.
- Buffered outputs (>10 KB) are already capped by codescout's summary at ~≤3 KB. rtk can make
  them WORSE: `git log -20` gives a codescout summary of ~2.5 KB with the full content kept in
  @cmd, but rtk's 7.4 KB falls under the 10 KB threshold and is returned inline in full, and it is lossy.
- `rtk discover` (Bash era, 248 sessions): claimed ~1.6M tokens "saveable". The path it
  measures is gone now that Bash is denied.
- **Replay-filter trap:** a filter that excludes commands containing `&` also drops every
  `2>&1` run, the most common form here. It cut the inline cargo-test population 4× (342 vs
  1,513) and under-estimated the native port's saving ~2.7×.

## Fidelity and guard breakage (verified live)
- `rtk cargo test <no-match>` prints `cargo test: 0 passed, 45 filtered out`. There is no
  libtest `test result:` line, so codescout's **`empty_test_selection_diagnostic` stays
  silent** (it fires on the raw form). rtk also drops compiler warnings.
- **IL-3 bypass:** `rtk cargo test 2>&1 | tail -2` is not blocked because the left-hand side
  reads as `rtk`, and run_command reported exit 0 on a failing suite, the exact masked-exit
  hazard IL-3 exists to prevent.
- Lossy by default. `git log` keeps 3 body lines, then `[+N lines omitted]`, and truncates
  subjects to ~110 chars; this repo's commit bodies are load-bearing. `git diff` caps each hunk
  (`702 additions truncated`) and truncates the tail file. `grep` strips indentation and
  truncates lines, which breaks copying a match into an edit `old_string`, and hides files
  behind `rtk recall`. `clippy` drops the lint name and the `help:` fix.
- rtk's `recall` store is a second buffer system. The @cmd buffer would then hold FILTERED
  output, which breaks the lossless-buffer contract of progressive disclosure.
- Fine: exit codes pass through; `git status` becomes a porcelain-like form (−48% on the long form, 0% on `--short`).

## Ported natively (2026-09-24, session 3b4fae98)
- `src/tools/libtest_compact.rs` + `run_command/output.rs::compacted_test_response`: an inline
  (<10 KB) libtest run is compacted by EXCLUSION (drop cargo progress, empty-target blocks,
  blanks, the RUST_BACKTRACE note; group passing names per target block; keep everything else
  verbatim). Raw streams stay in an `@cmd_*` buffer, and the trailer names `read_file` calls for
  both streams. Diagnostics are computed from RAW output first, so none are silenced.
  Replayed on 1,513 recorded inline cargo-test responses: 347 compacted, 1.80 MB → 0.58 MB
  (−68% on those), 1.22 MB saved. Gates: test command, not a buffer query, libtest summary
  present, ≥1 KB, ≥30% saving.
- `summarize_stderr` (buffered test/build envelopes) drops cargo progress before taking its
  20-line tail, and its remedy now names the real `<output_id>.err` handle. The old text,
  "stderr is NOT in the buffer", was stale after `.err` shipped. Bug `dd14e009ae2a3417` (archived),
  fixed in `176015f2`, patch-id `c6389996c860499ac888b6fe09d74468fca8b012`.
