---
id: a703b36da995dab3
kind: bug
status: open
title: 'BUG: the source-file gate is bypassed by a background `&`, a `sudo`/`xargs` wrapper, and a command substitution'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
opened: 2026-09-30
severity: low
unverified: 'sudo cat was NOT run (it means privilege escalation): its bypass is inferred from the head-token rule under which xargs cat was measured to pass. Whether sudo/xargs also mask an unbounded pipe in IL-3 (producer_index is shared) is likewise unmeasured.'
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
sudo cat build.rs                        -> NOT RUN. Inferred from the same head-token rule as xargs.
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

**Mechanism 1 (`&`) is FIXED; mechanisms 2 and 3 are not, on purpose. This bug stays open.**

**Fixed:** `5d39e0d887ba7232e641448c3ca1a861253c3525` (patch-id `44c0e2fb0fab7b8c5914826b74812bd7fee3a82d`, `git show <sha> | git patch-id --stable`). A lone `&` is now a STAGE separator (`SOURCE_GATE_STAGE_SEPARATORS`), guarded by `is_background_ampersand` so `2>&1`, `>&2`, `&>`, `<&3`, `|&` and `&&` stay whole. It is a stage separator and deliberately not a run separator: a backgrounded `cd` runs in a subshell, so as a run boundary it would resolve a later relative path against the `cd` target, read it as outside the project, and ALLOW the read, a bypass created by the fix (`source_file_access_does_not_let_a_backgrounded_cd_move_the_shell`). Five mutations (guard always true, each neighbour check dropped, `&` out of the stage list, `&` in the run list) were all killed. The gate's "Known limits" docstring now names what stays open.

**Mechanism 2 is WIDER than this file said, and is not fixed here.** This file says `producer_index` skips a closed wrapper list and `sudo`/`xargs` are missing from it. Measured 2026-09-30 on the live binary (each `| wc -l`): the gate does not consult `producer_index` at all. It classifies a stage by its raw first token (`shell_tokens(seg).next()`). So EVERY wrapper `producer_index` knows is a bypass, not only `sudo`/`xargs`: `env cat build.rs`, `FOO=1 cat build.rs` and `time cat build.rs` each returned 107 lines. Not run, but the same code path: `nohup`, `nice`, `timeout`, `command`, `sudo`. The same first-token rule also causes a FALSE POSITIVE: `cat=1 ls src/main.rs` reads as a `cat` (the regex is `\bcat\b` over the raw token). It was not fixed because PR #29 (OPEN, `fix/lessons-friction`, +2060/-127, touches `path_security.rs`) adds `executed_command`, one head rule shared by this gate and IL-3 that skips keywords, groups and wrappers including `stdbuf`; a second implementation here would fork it and conflict with it. #29 does NOT add `sudo`/`xargs` and does NOT make `&` a separator, so both remain open after it lands.

**Mechanism 3 (`$(...)`, backticks, `bash -c`, `eval`) is documented, not fixable by a head rule**, and now says so in the docstring.

### Follow-up, once #29 lands

Add `sudo` (value options `-u -g -C -h -p -r -t -U -D -R -T`) and `xargs` (`-I -n -P -d -E -L -s -a -J`) to `executed_command`, then re-run this table on the then-current binary. Because the head classifier is shared with IL-3, `sudo cargo test | tail` and `xargs cargo test | tail` will newly read as unbounded pipes: probably right, but measure it. Test table (each BLOCK case needs its ALLOW partner): BLOCK `env cat src/main.rs`, `FOO=1 cat src/main.rs`, `time cat src/main.rs`, `nohup cat src/main.rs`, `nice -n 5 cat src/main.rs`, `timeout 5 cat src/main.rs`, `command cat src/main.rs`, `sudo cat src/main.rs`, `sudo -u root cat src/main.rs`, `xargs cat src/main.rs`, `xargs -n 1 cat src/main.rs`; ALLOW (assert `None`) `sudo ls src/main.rs`, `xargs ls src/main.rs`, `env FOO=1 wc -l src/main.rs`, `FOO=1 ls src/main.rs`, `time ls src/main.rs`, `cat=1 ls src/main.rs` (the last is red on the old code for the opposite reason).

## Tests added

Landed with `5d39e0d8`, in `src/util/path_security.rs`: `a_lone_ampersand_splits_but_redirections_and_quoted_ones_do_not` (the splitter, asserted directly because through the gate a wrong split of a benign command usually blocks nothing, so a gate-level test cannot tell a correct split from a broken one), `source_file_access_blocks_a_read_after_a_background_ampersand`, `source_file_access_does_not_let_a_backgrounded_cd_move_the_shell`, and the over-block partner `source_file_access_allows_a_quoted_ampersand_before_a_reader_word`. All were run RED before the fix. Wrapper tests were written, run RED (they fail on this code, which is how mechanism 2 was measured), and then REMOVED from the tree because they cannot pass until #29 lands; their table is in the Fix section above.

## Workarounds
None needed by callers; the gate is failing open. Agents should keep using `read_file`/`symbols`.

## Resume

Open for mechanism 2 (wrappers, `sudo`/`xargs`) and the IL-3 side of `&` (`detect_il3_violation` has its own separator handling; a lone `&` was not checked there). Sequence after PR #29 merges.

## References
- PR #29 (merged head classification for IL-3 and this gate); its independent review listed the first four.
- `src/util/path_security.rs`: `check_source_file_access`, `split_outside_quotes`, `producer_index`.
