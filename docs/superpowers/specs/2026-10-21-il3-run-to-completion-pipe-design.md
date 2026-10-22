---
id: '915baae7e8ad70bd'
kind: spec
status: draft
title: 'IL-3 retirement: run the producer to completion, pipe the capture, return a handle to all of it'
---

# IL-3 retirement: run the producer to completion, pipe the capture, return a handle to all of it

Status: implemented. Merged 2026-10-22 as `43d3a25ee` on `experiments`. The build departs from this text in the places listed in § *Amendments during the build* at the end; where the two differ, the amendments win.

## Intent and why

Iron Law 3 refuses `run_command("cargo test | grep FAILED")` before it runs. The refusal exists because a pipe to a trimmer used to discard the full output, so the agent could not query it again. Agents now pay a second call for every filtered command, or hit the refusal and rerun. The refusal text itself is the friction.

The reason for the refusal no longer holds if the server keeps the full output. This design keeps it: the server runs the producer to the end, saves the whole stream, applies the agent's pipe to the saved stream, and returns the piped result plus a handle to all of the saved stream.

## Goals

- A pipe whose last stage is a terminal filter is never refused. The filters are the 13 in `detect_terminal_filter` (`src/tools/command_summary.rs:304`): grep, egrep, fgrep, rg, head, tail, sed, awk, cut, wc, sort, uniq, tr. Those pipes take one path. Other pipes (`xargs`, `sh`, `git apply`, `while read`) run as they do today.
- The response carries the piped result and one handle to the whole capture of `A`'s stdout.
- A capture that is incomplete says so: cut by the size limit, or cut by the wait cap.
- `head`, `sed -n 1,10p`, `grep -m1` and other early-exit consumers get a complete capture too. There is no special case for them.

## Non-goals

- Streaming output while the command runs.
- Per-stage handles (`pipeline=[...]`, see `docs/trackers/run-command-pipeline.md`). This design is the one-tap case of that tracker's ruling R3 and must not conflict with it.
- Estimating output size before the run. It is not possible in general, and the existing static classifier `is_unbounded_lhs` is the only estimate. It may feed a warning, never a refusal.

## Anti-goals (a technically working build that still fails)

- A response that looks complete when the capture is not. Today `seq 1 300000 | head -3` returns a handle of 1860 lines with no `unfiltered_truncated` flag. That is the failure to remove.
- A handle that serves unredacted credentials. The current capture path calls `redact_credentials` before it stores the text; a bug comment in `output.rs` records `env | grep PATH` leaking the whole environment through this buffer. The new handle must serve redacted text.
- Two code paths: one for early-exit trimmers and one for the rest.

## Decided (marius)

1. The producer runs to completion; the pipe is applied after.
2. The wait is capped. On timeout the server kills the producer, applies the pipe to the partial capture, and flags the result incomplete.
3. The handle keeps "a few MiB". Past that the server cuts and marks the cut.
4. The refusal is retired for foreground pipes that end in a terminal filter, including `head` and `tail`.
5. Background (`run_in_background`) pipes keep the refusal, with new text, because the background path keeps no unfiltered stream. A bounded background pipe already runs unrefused and keeps only the filtered output; that gap is out of scope here.

## My calls (not yet confirmed by marius)

- The handle points at the whole redacted capture, not at the 10 KB `@cmd_*` prefix buffer used today.
- `timeout_secs` (default 30) is the wait cap.
- The limit is 4 MiB. `seq 1 300000` is about 1.9 MiB, so it fits and makes a clean test; the builder may pick a different value in the 2–8 MiB range.
- `A | B | C` runs `B | C` on the capture.
- The static classifier stays and may add a warning for an unbounded left side; it never refuses.
- **Layout: one shell, one tap, no Rust-side two-step.** The shape is `A | tee <cap> | { B; cat >/dev/null; }`. The trailing `cat` drains the stream, so an early-exit `B` (`head`) never closes the pipe on `A`. `A` runs to the end, `B` sees the stream, and the capture is whole. This is the one-tap case of R3 and keeps one timeout and one `killpg` mechanism. The earlier text that offered a "Rust-side two-step" is withdrawn.
- **Trigger: the existing splice site.** The rewrite uses the same textual splice as `inject_tee_in` (the last unquoted `|`, found by `detect_terminal_filter`). In `cd d && cargo test | grep F`, `A` is `cargo test`. A shape the splice cannot classify runs unchanged with no handle, as today.
- **Size limit: the pipe sees the whole stream; only the handle is cut.** `cargo test 2>&1 | grep FAILED` must report every failure, not those in the first 4 MiB. The handle keeps the first 4 MiB and carries the existing truncation marker. The builder must also bound the capture file on disk (for example `head -c`), so a fast producer cannot fill the disk inside the wait cap.
- **Timeout: one total deadline for the producer, then a short grace for `B`.** On timeout the server kills the process group (SIGKILL loses `B`'s buffered output), then runs `B` once on the partial capture with a fixed 5 s grace. If the grace expires, the response carries the partial handle and no piped result, and says so. This is a bounded exception to R5's "total timeout" and must be named as one.
- **Known cost, accepted by the wait cap:** an endless producer into a trimmer (`yes | head -1`, `tail -f log | grep -m1 ready`) now ends at the cap, flagged incomplete, instead of at once.
- **Background runs (decided by marius, 2026-10-21, option 1).** Verified live on 2026-10-21: `run_command("cargo --version | head -1", run_in_background=true)` is refused by the gate. `run_command("seq 1 50000 | grep 4999", run_in_background=true)` is allowed (bounded left side) and returns `@bg_00000008` at once. Its log held 15 lines, the filtered result, and the 50000-line stream was kept nowhere. The cause is in `inner.rs`: `spawn_background_command` (Step 4.7) runs before the tee step (Step 4.5) and sends the shell's stdout straight to the log file. So a background pipe already loses the unfiltered stream today when its left side is bounded; the refusal only covers the unbounded left side. If the refusal retires for background, unbounded background pipes lose it too. My proposal: foreground only; background pipes keep the refusal, with new text. Marius accepted this as the one exception to decision 4.

## Verified facts (read in this session, 2026-10-21)

- The tee fires only when the LAST pipe stage is one of the 13 filters above. The shell is `sh -c` (`src/platform/unix.rs:252`). A timeout today kills the process group and returns only `timed_out`, `stderr`, `exit_code: null` and a hint, with no output (`inner.rs:677-711`).
- The refusal: `detect_il3_violation`, `src/util/path_security.rs:1898`, called at `src/tools/run_command/mod.rs:428`.
- The tee rewrite: `inject_tee_in`, `src/tools/run_command/inner.rs:211`. It turns `A | B` into `A | tee <tmp> | B` when `detect_terminal_filter` finds a terminal filter.
- The capture is read whole with `read_to_string`, redacted, line-counted on the whole text, then cut to `MAX_INLINE_TOKENS * 4` = 10,000 bytes before `store_truncated_kept` (`output.rs`, "Step 6.5"). `TmpfileGuard` (`inner.rs:13`) deletes the temp file when the function ends. So after the response, only the 10 KB prefix survives.
- SIGPIPE is reset to the default in `pre_exec` (`inner.rs`, near line 502) on purpose, so that `| head -N` ends the pipeline. This is why the tee capture for `head` is partial.
- Live runs: `seq 1 300000 | head -3` → `unfiltered_output_lines: 1860`, no truncation flag. `seq 1 300000 | grep 77777` → `unfiltered_output_lines: 300000`, `unfiltered_truncated: true`, `unfiltered_buffered_lines: 2222`.
- `can_append_stage` (`output.rs:642`) asks the gate whether a hint's appended stage "would be run". Retiring the gate changes this function's meaning; it needs a replacement predicate (the query is one pipeline and the stage reads all of it). Callers: `wide_line_route`, a range-hint parser near line 728, and the pager hint near line 937.
- The companion plugin hook `claude-plugins/codescout-companion/hooks/il3-deny-hook.sh` is parked and unwired (header says so). The server is the only enforcer. The hook's header and `docs/architecture/companion-plugin.md` still describe a re-promotion path and need an update.
- The IL-3 tests are many: `il3_*` in `src/util/path_security.rs` and `src/tools/run_command/tests.rs` (for example `il3_blocks_cargo_pipe_grep_via_run_command`, `il3_blocks_chained_unbounded_pipe`, `il3_blocks_unbounded_pipe_pre_exec`), plus `src/tools/run_command/observe.rs` (a test whose premise is "the IL-3 gate must refuse this") and `tests.rs:9499`.

## Not verified

- What the response looks like today when `timeout_secs` expires inside a teed pipeline.
- Behavior on Windows Git Bash (the tee code already runs there).
- Whether a capture of a few MiB is safe for the output pool; the guide says a `@file_*` handle may hold up to 64 MiB, which suggests yes.
- Whether `tail` already yields a complete capture (it should, by how pipes work).
- Any commands in the wild that rely on the producer receiving SIGPIPE (for example an endless producer into `head -1`). The wait cap is the proposed safeguard.

## Left to the builder

- The new refusal text for background pipes, and a decision on whether the gate stays as a function that only the background path calls (`detect_il3_violation`) or becomes a smaller background-only check.
- The exact shell text of the single-shell layout (see My calls), how the capture file is bounded on disk, and how the post-timeout run of `B` gets its 5 s grace.
- The exit code to report for a pipeline. Today it follows the shell. Ruling R4 in the pipeline tracker says to decide from `PIPESTATUS`. The builder must pick and state it.
- Response key names for "capture incomplete because of the wait cap". Reuse `unfiltered_truncated` and `unfiltered_buffered_lines` for the size cut.
- The replacement for `can_append_stage`.
- What to do with `detect_il3_violation`: delete, or keep the classifier only as a warning source.
- Prompt-surface updates: Iron Law 3 text in the server instructions, `src/prompts/`, the onboarding text, `docs/PROGRESSIVE_DISCOVERABILITY.md` and the companion `[cs-hint]` text. `src/prompts/README.md` says when to bump `ONBOARDING_VERSION`.
- Test plan: flip the `il3_*` refusal tests to acceptance tests. Add tests for the exact limit edge, the wait cap, redaction of the handle, and `head` completeness.


## Amendments during the build (2026-10-21 to 2026-10-22)

Every item below was ruled by the controller during a subagent-driven build, after review or measurement showed the text above wrong or incomplete. Each cites what was verified.

- **Splice point.** The tee goes at the FIRST depth-0 pipe, not the last (`seq 1 1000 | grep 7 | head -2` handed back 271 lines at the last pipe, not 1000). One function, `tee_splice_point`, decides both the splice and the foreground refusal. A command is capturable only if: its last stage is one of the 13 terminal filters; the first pipe is not nested in `( )`, `$( )`, backticks or `{ }`; the text after it is one plain pipeline (no `;`, `&&`, `||`, newline, comment, heredoc, `$(`, backtick); it is not `|&`; it holds no buffer ref; and the shell parses both the original and the spliced text (a `set -n` probe). Otherwise the command runs untouched.
- **Spliced shape.** `A | ( trap '' XFSZ; ulimit -f N 2>/dev/null; exec tee '<cap>' 2>/dev/null ) | { REST` newline `s=$?; head -c <limit> >/dev/null; exit $s; }`. The drain is bounded at the store limit (an unbounded `cat` let an endless producer fill the disk); the capture FILE is bounded by `ulimit -f` while the filter still sees the whole stream; `read_capture` reads at most the limit plus 1 MiB and decodes lossily.
- **Line count.** `unfiltered_output_lines` counts the lines of the capture file and is a LOWER BOUND whenever `unfiltered_truncated` is set, for early-exit and whole-stream filters alike.
- **Timeout.** The existing `timed_out` shape is kept; when a capture exists the response adds `capture_incomplete: true`, the `unfiltered_*` keys and either the filter's redacted `stdout`, `filter_grace_expired` (the replay ran out of its 5 s grace) or `filter_not_run: "<reason>"`; `filter_exit_code` and `filter_stderr` report a failing replay. The replay runs only a provably pure filter: every stage is a terminal filter with allowlisted flags (exact spellings; `awk` never), no `$`, backtick, `>` or `<`, and the producer side is one pipeline. The hint says a higher `timeout_secs` does not help an endless or slow producer. A timed-out filtered call can take up to `timeout_secs` + 5 s.
- **Refusal.** Foreground: refused (message starts `IL3 violation`) when the old detector fires and the capture cannot be spliced in. Background: always refused for those pipes, because a background run keeps no unfiltered stream. Bounded left sides, aggregators (`wc`, `grep -c`) and pipes whose left side is a buffer handle were never refused.
- **Exit status.** A pipe reports the FILTER's exit status, not the producer's (`cargo test 2>&1 | tail -30` reports 0 for a failing run). Stated in the guides and the tool description. Open decision: a `producer_exit_code` key.
- **Known costs.** A finite producer into `head`, `sed -n Np` or `grep -m` now runs to completion or until 4 MiB more is read (`git log | head`, `curl url | head -c 100`). A slow or endless producer into an early-exit filter now runs to `timeout_secs` and returns `timed_out` with the replayed result (`tail -f log | grep -m1 ready`, `ping host | head -3`). A fast endless producer (`yes | head -1`, the urandom idiom) still ends in about 20-120 ms.
- **Prompt surfaces.** Iron Law 3 in the server instructions was rewritten in the slice's zero-slack budget; that change did NOT go through the README's P-1..P-8 measurement protocol.
- **Not verified.** Windows, macOS and `dash` were never run (CI matrix: ubuntu, macos, windows). `ulimit -f` units differ between POSIX-mode `sh` (512 B) and non-POSIX bash (1024 B): the file bound is about 4.06 MiB or 8.1 MiB. An inherited `RLIMIT_FSIZE` below the bound makes the capture silently partial.
- **Deferred, not fixed.** The parse probe runs twice per command and synchronously; the tee/replay code lives in `command_summary.rs` and should move to its own module; `trap '' XFSZ` is unguarded on a shell that does not know the signal; an unquoted glob as a `sed` script is not refused by the replay check; the drain cut has no marker of its own. The companion plugin still injects "Never pipe unbounded run_command output" into every session (`hooks/session-start.mjs`, `hooks/subagent-guidance.mjs`), and its parked `il3-deny-hook.sh` and `skills/tracker-hygiene/SKILL.md` still state the old rule.
