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
Open. Proposed direction, untested, and the three mechanisms do not share a remedy:
1. `&`: make a lone `&` a run separator, taking care that `&&`, `>&`, `&>`, `2>&1` and `|&` are not split.
2. `sudo`, `xargs`: add to the wrapper list with their value-taking options (`sudo -u/-g/-C/-h/-p/-r/-t/-U`,
   `xargs -I/-n/-P/-d/-E/-L/-s/-a`). Because `producer_index` is also IL-3's head classifier this changes
   IL-3 too, which is probably right (`sudo cargo test | tail` reads as head `sudo`) but must be measured.
   Limit to state: in `find . | xargs cat` the paths arrive on stdin and are invisible at parse time.
3. `$(...)`, backticks, `-c`, `eval`: recursing into substitution bodies is feasible; `bash -c`/`eval` take
   arbitrary code and cannot be closed by a parser. The honest remedy for those is the one `IC-14` already
   names: state the limit at the refusal site and in "Known limits", as the docstring already does for
   `cat $FILE`.
Whatever is decided, the "Known limits" docstring on `check_source_file_access` should list what stays open.

## Tests added
N/A — open, no fix written. A fix needs a fixture per mechanism and an over-block direction (`2>&1`,
`echo "a & cat x.rs"` quoted, `sudo ls src/`), per the two-direction pairing of `source_gate_prefix_stripping_does_not_invent_readers` in #29.

## Workarounds
None needed by callers; the gate is failing open. Agents should keep using `read_file`/`symbols`.

## Resume
Decide per mechanism whether to close or document it. Before writing a fix, re-run the table above on the
then-current binary: #29 and #28 both edit this function.

## References
- PR #29 (merged head classification for IL-3 and this gate); its independent review listed the first four.
- `src/util/path_security.rs`: `check_source_file_access`, `split_outside_quotes`, `producer_index`.
