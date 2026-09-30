---
id: a703b36da995dab3
kind: bug
status: taken
title: 'BUG: the source-file gate is bypassed by a background `&`, a `sudo`/`xargs` wrapper, and a command substitution'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
claimed_at: 2026-09-30
claimed_by: 0b05903e-39c7-4f8a-be01-b85cfd9502a3
opened: 2026-09-30
severity: low
unverified: '`sudo cat` was reported refused by the author of 4d392858 on a rebuilt binary; THIS session did not run sudo (privilege escalation) and has not reproduced it. The table has NOT been re-measured on a binary containing the `|&` / lone-`&` IL-3 fix (8614aebf): no build contains it yet. Mechanism 3 and `find -exec` / `parallel` are open by design.'
---

## Summary
`check_source_file_access` refuses a content reader (`cat head tail sed awk less more`) that names a
source file inside the project. It decides by classifying each pipeline segment by its HEAD token, after
splitting on `&& || ; \n` and `|`. Four spellings reach the file without the head being a reader, and
three more share their mechanism. The gate's "Known limits" docstring lists variable expansion and
heredocs and none of these.

## Symptom (Effect)
Measured 2026-09-30 on the live binary, `run_command` on `build.rs` (107 lines), each suffixed
`| wc -l` so the result is one number. The control is refused, everything else runs:
```
cat build.rs | wc -l                     -> refused: "shell access to source files is blocked"   (control)
echo b & cat build.rs | wc -l            -> exit 0, "b" then 107
xargs cat build.rs </dev/null | wc -l    -> exit 0, 107
echo $(cat build.rs) | wc -l             -> exit 0, 1   (the file was read inside the substitution)
echo `cat build.rs` | wc -l              -> exit 0, 1
bash -c 'cat build.rs' | wc -l           -> exit 0, 107
eval cat build.rs | wc -l                -> exit 0, 107
sudo cat build.rs                        -> not run at first; measured later the same day: `sudo -n cat build.rs` printed the file (see "Wrapper half" under Fix)
```
The independent review of #29 reported `&`, `sudo`, `xargs` and `$(...)`. Backticks, `bash -c` and `eval`
were not reported: they put the reader inside an argument exactly as `$(...)` does, and were found while
measuring it.

## Reproduction
`run_command` any line above against a project that contains the named file. The code was read at
`origin/experiments` and again with PR #29 applied: #29 edits neither the separator list nor the wrapper
list in a way that reaches these (it adds `if/while/until/do/then/else`, `!`, grouping and `stdbuf`).

## Environment
Live binary `codescout 0.15.0` (`target/release/codescout`, rebuilt from the working tree at an unrecorded
commit), tree at `2b809856`, `src/util/path_security.rs` unmodified in the working tree.

## Root cause
Three separate narrowings, one shared premise: *the command that reads the file is the first word of a
segment the splitter found.*

1. **Separator gap.** `split_outside_quotes` is called with `&&`, `||`, `;`, `\n` for runs and `|` for
   stages. A single `&` (background) is in neither list, so `echo b & cat build.rs` is ONE segment whose head
   is `echo`. Not a one-character fix: `2>&1`, `>&`, `&>` and `|&` contain the character and must stay whole.
2. **Wrapper gap.** `producer_index` skips a closed list of prefixes (assignments, `env`, `nice`,
   `timeout`, `nohup`, `time`, `command`, and with #29 `stdbuf` and compound keywords). `sudo` and `xargs`
   are not on it, so the head reads `sudo`/`xargs`, which is not a reader.
3. **Nesting gap.** `$( ... )`, backticks, `bash -c '...'` and `eval ...` put the reader INSIDE an argument
   of the head command. No head-token rule sees it, however long the wrapper list grows.

## Evidence
The measured table above. The `IC-14` reading: the refusal text says "shell access to source files is
blocked", which is the claim every later reader reasons with; the implementation covers the segments whose
head is a reader.

## Hypotheses tried
- *Is it only the four reported?* No: measured `bash -c`, `eval` and backticks pass identically (above).
- *Does `acknowledge_risk` make this moot?* It makes it a steering gap rather than a security boundary:
  the gate routes agents to `read_file`/`symbols` (cheaper, index-aware), and an agent that wants raw
  access can already pass `acknowledge_risk: true`. That is why severity is `low`. It is still the gate
  reporting a block it does not enforce.

## Fix

**Mechanisms 1 (`&`) and 2 (wrappers) are FIXED; a fifth variant, `|&`, is open; mechanism 3 is documented. This bug stays open.** Updated 2026-09-30 after #29 merged; the paragraph on mechanism 2 below is what was believed before it did.

**Fixed:** `5d39e0d887ba7232e641448c3ca1a861253c3525` (patch-id `44c0e2fb0fab7b8c5914826b74812bd7fee3a82d`, `git show <sha> | git patch-id --stable`). A lone `&` is now a STAGE separator (`SOURCE_GATE_STAGE_SEPARATORS`), guarded by `is_background_ampersand` so `2>&1`, `>&2`, `&>`, `<&3`, `|&` and `&&` stay whole. It is a stage separator and deliberately not a run separator: a backgrounded `cd` runs in a subshell, so as a run boundary it would resolve a later relative path against the `cd` target, read it as outside the project, and ALLOW the read, a bypass created by the fix (`source_file_access_does_not_let_a_backgrounded_cd_move_the_shell`). Five mutations (guard always true, each neighbour check dropped, `&` out of the stage list, `&` in the run list) were all killed. The gate's "Known limits" docstring now names what stays open.

**Mechanism 2 is WIDER than this file said, and was not fixed in `5d39e0d8` (see "Wrapper half" below for what closed it).** This file says `producer_index` skips a closed wrapper list and `sudo`/`xargs` are missing from it. Measured 2026-09-30 on the live binary (each `| wc -l`): the gate does not consult `producer_index` at all. It classifies a stage by its raw first token (`shell_tokens(seg).next()`). So EVERY wrapper `producer_index` knows is a bypass, not only `sudo`/`xargs`: `env cat build.rs`, `FOO=1 cat build.rs` and `time cat build.rs` each returned 107 lines. Not run, but the same code path: `nohup`, `nice`, `timeout`, `command`, `sudo`. The same first-token rule also causes a FALSE POSITIVE: `cat=1 ls src/main.rs` reads as a `cat` (the regex is `\bcat\b` over the raw token). It was not fixed because PR #29 (OPEN, `fix/lessons-friction`, +2060/-127, touches `path_security.rs`) adds `executed_command`, one head rule shared by this gate and IL-3 that skips keywords, groups and wrappers including `stdbuf`; a second implementation here would fork it and conflict with it. #29 did not add `sudo`/`xargs` and did not make `&` a separator: `&` was closed by `5d39e0d8` and `sudo`/`xargs` by `4d392858`.

**Mechanism 3 (`$(...)`, backticks, `bash -c`, `eval`) is documented, not fixable by a head rule**, and now says so in the docstring.

### Wrapper half — measured, then closed (2026-09-30)

Measured on the rebuilt live binary after #29 merged and `5d39e0d8` was built, each command run once against `build.rs`:

| command | result |
|---|---|
| `echo b & cat build.rs` | refused (`5d39e0d8`); the refusal names the clause: "offending clause: `cat build.rs` (1 other clause … not run)" |
| `cd /tmp & cat build.rs` | refused: a backgrounded `cd` moves nothing |
| `env cat build.rs`, `FOO=1 cat build.rs`, `time cat build.rs`, `nohup cat build.rs` | refused (#29's `executed_command`) |
| `echo build.rs \| xargs cat` | **printed the file** |
| `sudo -n cat build.rs` | **printed the file** (passwordless sudo here; it ran, read-only) |
| `ls src/util/path_security.rs`, `echo ok && echo also-ok` | allowed (over-block controls) |

**Fixed by `4d39285807fa0792056accc088d310ad29e42000`** (patch-id `3ecd695f53366c9a9be4e056ca6815a29d4b65c6`, `git show <sha> | git patch-id --stable`):

- `sudo` joined `producer_index`, with its valued options (`-u -g -C -D -h -p -R -r -t -T -U` and the long forms), so `sudo -u root cat x` reads `cat` and not `root`. `producer_index` is shared with IL-3, so `sudo cargo test | tail` now reads as an unbounded pipe too: the intended direction, pinned by `il3_sees_through_sudo`.
- `xargs` is handled in the SOURCE GATE only (`executed_reader_command`), deliberately not in `producer_index`. Its reader gets its PATHS on stdin, so the segment names no file and a path-based verdict finds nothing to refuse: slicing at `xargs` alone would have fixed nothing. A reader run by `xargs` is refused whatever the segment names, as an unresolvable `$VAR` path is, and the refusal says why. This is an OVER-refusal by design (`ls docs | xargs cat` is refused too); `acknowledge_risk: true` is the exit. Chosen over accepting the limit because the realistic spellings (`find … | xargs cat`, `git ls-files | xargs cat`) all read project source.

**Verification.** `util::path_security` 251 passed; the wrapper tests were RED first (4 failed). Mutated once per site (12): 10 KILLED (the `sudo` arm; `sudo`'s valued options, killed by BOTH the block case and its allow-partner; `stdin_fed` set; `xargs`'s valued options, re-run after later edits; the gate's `stdin_fed` check; the stdin note; the remedy chosen from `xargs`'s command; the nested-`xargs` loop; the option-stripping call); 2 SURVIVED as semantically inert (the `--` and lone `-` branches of `strip_xargs_options`: the only input they change is a command NAMED with a leading `-`) and were deleted. Gate on `66a631ea` plus this diff in an isolated worktree: FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0. The shared-tree gate was red only in a peer's uncommitted file.

**Not covered.** The allow-partners for `xargs` (`xargs echo cat`, `xargs wc`, `xargs grep`) were green before the fix and have no mutation of their own: nothing here proves they would go red if the head check became "any token is a reader". `$(…)`, backticks, `bash -c`, `eval` stay documented limits (mechanism 3). `find … -exec cat {} +`, `parallel cat`, `doas`, `ionice` are inferred bypasses, unmeasured.

**A fifth variant, found by codescout-d7 after this file was first written: `|&`.** Splitting `a |& b` on `|` leaves the stage `& b`, headed by `&`, so `echo x |& cat build.rs \| wc -l` ran in both the source gate and IL-3. The test `a_lone_ampersand_splits_but_redirections_and_quoted_ones_do_not` in `5d39e0d8` asserted `split("a |& b") == ["a", "& b"]` with a comment calling that correct, so it PINNED the bypass as intended; the fix (`8614aebf`, patch-id `72c450e8519e3836168a6c1700dcb1e76de49623`, gated as `162ec65e` in an isolated worktree; it was `81a95660` / `179ce63b…` before it was rebased onto `4d392858` and before it changed that assertion, as it must; unpushed) changes that behaviour and must change that assertion with it.

### Spellings still open, MEASURED on the rebuilt binary (2026-09-30, after `4d392858`, before the `|&` fix)

Measured by the author of `4d392858` on the rebuilt live binary and then **reproduced independently by codescout-d7 on the same binary** (each command run once against `build.rs`, 107 lines, suffixed `| wc -l`; control `cat build.rs | wc -l` refused). Every row below returned the file:

| command | what it is |
|---|---|
| `echo x \|& cat build.rs` | the `|&` variant (fixed on a branch, not yet in any build) |
| `ionice cat build.rs` | a wrapper nobody listed: the head is `ionice` |
| `doas cat build.rs`, `doas -u root cat build.rs`; IL-3: `doas -n cargo --version \| tail -1` | another unlisted wrapper; `doas` is installed here (setuid, no `/etc/doas.conf`) and refused to run anything itself, so `wc -l` printed 0: a gate bypass, not a disclosure |
| `find build.rs -maxdepth 0 -exec cat {} +` | the reader is an argument of `find` |
| `parallel cat ::: build.rs` | the reader is an argument of `parallel` (installed here) |
| `bash -c 'cat build.rs'`, `echo $(cat build.rs)` | mechanism 3, documented |

Refused, reproduced: `echo build.rs \| xargs cat` (note says the paths arrive on stdin) and the plain control. Reported by the other author and NOT reproduced here: `sudo -n cat`, `sudo -n -u root cat`, `env cat`, `echo b & cat`, and IL-3's `sudo -n cargo --version | tail -1`. `ionice`, `find -exec`, `parallel` and `doas` therefore moved from "inferred" to **measured bypasses** (`doas` was reported by the author of `4d392858` and reproduced here). They are not closable by adding to a wrapper list one name at a time (`ionice` could be, `find -exec` and `parallel` cannot): they are mechanism 3 in another spelling, and the honest remedy is the same: name them in the docstring's Known limits.

## Tests added

Landed with `5d39e0d8` (the `&` half): `a_lone_ampersand_splits_but_redirections_and_quoted_ones_do_not`, `source_file_access_blocks_a_read_after_a_background_ampersand`, `source_file_access_does_not_let_a_backgrounded_cd_move_the_shell` and the over-block partner `source_file_access_allows_a_quoted_ampersand_before_a_reader_word`, all run RED first.

Landed with `4d392858` (`sudo`/`xargs`), all run RED first (4 failed): `source_file_access_blocks_a_read_run_through_sudo`, `source_file_access_allows_a_sudo_command_that_is_not_a_reader` (the partner: `-u cat` names a user), `source_file_access_blocks_a_reader_fed_by_xargs` (including `xargs cat src/main.rs`, the spelling first measured, and a nested `xargs xargs cat`), `source_file_access_allows_xargs_that_does_not_run_a_reader`, `a_reader_fed_by_xargs_is_refused_with_the_reason` (asserts the note itself, because a two-stage pipeline would already carry the word `xargs` in the clause note), `source_gate_remedy_for_xargs_is_chosen_from_the_command_it_runs` and `il3_sees_through_sudo`.

The env/`FOO=`/`time`/`nohup` cases are #29's tests (`executed_command`), not repeated here.

## Workarounds
None needed by callers; the gate is failing open. Agents should keep using `read_file`/`symbols`.

## Resume

Open, in order:

1. **`|&`** in the source gate and IL-3, plus IL-3's own lone `&` (`echo x & rg -c zzz f | tail -1` ran; `rg … & echo b | tail -1` was refused, a false positive): codescout-d7: committed locally as `8614aebf` (gated as `162ec65e`: FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0; rebased onto `4d392858`, the `a |& b` assertion now uses the production constant), not pushed.
2. **Mechanism 3** (`$(…)`, backticks, `bash -c`, `eval`): documented, not closable by a head rule.
3. **`find -exec`, `parallel`, `ionice`, `doas`**: MEASURED bypasses, see the table above. `ionice` and `doas` are one-line wrapper-list entries like `sudo`; `find -exec` and `parallel` are not closable name by name, and the remedy for those is the Known-limits docstring.
4. **Re-run the table above on a binary built from a tree containing `4d392858` and the `|&` fix.** Whoever closes this file owns that measurement; the author of `4d392858` has not made it.

## References
- PR #29 (merged head classification for IL-3 and this gate); its independent review listed the first four.
- `src/util/path_security.rs`: `check_source_file_access`, `split_outside_quotes`, `producer_index`.
