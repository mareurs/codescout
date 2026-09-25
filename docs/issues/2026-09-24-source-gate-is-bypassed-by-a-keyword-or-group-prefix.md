---
status: fixed
opened: 2026-09-24
closed: 2026-09-24
severity: low
owner: marius
related: [docs/issues/2026-09-24-source-gate-refusal-discards-the-clauses-it-did-not-block.md]
tags: [cluster/guard-narrower-than-its-name]
kind: bug
---

# BUG: the source-file gate is bypassed when the reader follows a shell keyword or an opening group

## Summary
`check_source_file_access` matches a run's FIRST token against the content readers. A run that
begins with a compound-command keyword (`do`, `then`, `else`) or a grouping token (`(`, `{`)
has that as its first token, so `cat` behind it is never examined and the file is read.

## Symptom (Effect)
All three ran on the live binary, and the `@cmd_*` buffer holds the file's content:
```
for f in a; do cat src/ast/mod.rs; done | wc -l   → exit 0, "210", unfiltered_output_lines: 210
( cat src/ast/mod.rs ) | wc -l                    → same
if true; then cat src/ast/mod.rs; fi | wc -l      → same
{ cat src/ast/mod.rs; } | wc -l                   → same
FOO=1 cat src/ast/mod.rs | wc -l                  → same   (assignment prefix: the IL-3 sibling's shape)
env cat src/ast/mod.rs | wc -l                    → same   (wrapper)
```
Control: `cat src/ast/mod.rs` alone is refused with `shell access to source files is blocked`.

## Reproduction
Live binary at `506924f2`; `run_command` any line above.

## Root cause
`check_source_file_access` (`src/util/path_security.rs:1686`) splits runs on `&& || ; \n` and
stages on `|`, then tests `shell_tokens(seg).first()` against `SOURCE_ACCESS_COMMANDS`. The
keyword or `(` is the first token. measured 2026-09-24: the three commands above.
Found while writing the rerun tests for the sibling bug in `related:`: a `for … do cat …`
fixture was expected to block and did not.

## Fix

One rule set for both gates, because both had the same false premise ("`tokens[0]` is the
command"). Upstream had meanwhile fixed the IL-3 half in `1fb66cf6` with `producer_index`
(assignments, `env nice timeout nohup time command`). This fix EXTENDS that function rather than
adding a second one: the keywords `do then else elif !`, standalone `(`/`{`, the `stdbuf`
wrapper, and value-taking options (`env -u/-C/-S`, `stdbuf -o L`, long forms).
`executed_command` is a thin wrapper that slices at `producer_index` and also strips grouping
glued to a word (`(cat`), which an index cannot express. Used by IL-3's `is_unbounded_lhs`, the
source gate's head check, and its remedy picker (which chose the `cat` remedy for a `do sed`
clause). `command` stays a wrapper as upstream decided, so `command -v cargo | head` is
over-refused — stated at the site.

Fix: `ce41048f` on branch `fix/lessons-friction` · patch-id `066083d1ea88e38ed7083576925b833c14bf39d5`.
### Review follow-up (2026-09-25, independent Opus review)

- **`if`, `while`, `until` were missing** from the keyword set (it had `elif` but not `if`), so
  `if cat src/main.rs; then …` still read source and `if cargo test | tail; then …` still masked
  cargo's exit status. Every fixture had put the reader after `then`. Added, with tests
  `source_gate_sees_through_a_leading_if_while_or_until` and
  `il3_sees_through_conditionals_one_word_groups_and_every_valued_option`.
- **A one-word group `(pytest)`** left the head as `pytest)`; `executed_command` now trims a
  trailing `)`/`}` from the head.
- **`env -S`'s value is the command**, so it is no longer skipped as a value; GNU `time -f/-o`
  now are.
- **The redundant standalone `(`/`{` skip** in `producer_index` was removed: a mutation run
  showed it could never fire behind `executed_command`, its only caller.

Review fix: `c4043285` on branch `fix/lessons-friction` · patch-id `07184a2f8dfc2f054db97537536b619489cc4e95`.

## Tests added

`source_gate_sees_through_keyword_group_and_assignment_prefixes` (10 shapes),
`source_gate_prefix_stripping_does_not_invent_readers` (the over-block direction),
`source_gate_remedy_is_chosen_from_the_executed_command`. Mutation results in the commit
message.
## Workarounds
N/A — this is the gate failing open.

## Resume

N/A once committed. Tag through the catalog after merge.
## References
- IL-3 env-prefix sibling, fixed independently upstream in `1fb66cf6` (`producer_index`):
  `docs/issues/archive/2026-09-24-il3-unbounded-pipe-block-is-bypassed-by-a-leading-env-assignment.md`.
  This branch was rebased onto it and extended that ONE helper (keywords, grouping, `stdbuf`,
  valued options) rather than keeping a second one, so both gates share a single rule set.
  (same "first token is the command" assumption, in the pipe gate).
