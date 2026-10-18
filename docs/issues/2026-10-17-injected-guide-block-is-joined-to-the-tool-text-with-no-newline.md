---
kind: bug
status: fixed
title: 'BUG: the injected guide block follows a tool''s last line with no newline, so line-oriented readers of a reply lose that line'
tags:
- cluster/unclassified
opened: 2026-10-17
owner: marius
related:
- docs/issues/2026-10-17-symbols-include-body-on-kotlin-lsp-results-is-shifted-up-one-line.md
severity: low
closed: 2026-10-18
---

# BUG: the injected guide block follows a tool's last line with no newline, so line-oriented readers of a reply lose that line

## Summary

codescout appends a `get_guide` body to a tool reply as a SEPARATE MCP content block that starts with `<!-- auto-injected get_guide(...)`. Clients that join the blocks without a separator put the marker on the same line as the tool's last line. A reader that strips the guide by line (`sed '/auto-injected/,$d'`, `grep -v`, `head`) then drops that last line: one body line, one overview row. This produced two false bug reports on 2026-10-17 (`symbols(include_body)` "shifted up one line", an overview "missing" its last member).

## Class fit

No class on the roster was walked one by one, so the sanctioned `cluster/unclassified` is used. What the case shows: two parts of one reply (the tool's text and an advisory) are separate to the server and merged by the reader, and the merge destroys content at the seam without any sign that it did.

## Symptom (Effect)

Plain `mcp call symbols ... include_body=true` output, measured 2026-10-17 (`grep -o` around the marker): `6:          }<!-- auto-injected`. Claude Code's own tool results show the same join, `}<!-- auto-injected get_guide(...)`, so it is not specific to the `mcp` CLI.

## Reproduction

```
mcp call symbols -p '{"name": "Catalog/add", "path": "src/main/kotlin/library/services/Catalog.kt", "include_body": true}' <alias> | sed '/auto-injected/,$d'
```
prints the body without its closing `}`. The same call with `-f json` and `content[0].text` is complete.

## Environment

codescout at branch worktree-agent-af85b780928eaa450, `mcp` CLI at ~/.local/bin/mcp.

## Root cause

Confirmed at the MCP layer: the guide is emitted as its own content block (`src/tools/guide_emit.rs`, around lines 110-122, per the 2026-10-17 review), so the server is not joining anything. The seam is made by the consuming client. measured 2026-10-17: `repro5.sh` plain output, and the reviewer's observation of Claude Code tool results.

The defect on the server's side is that it relied on that join having a separator: no appended block began with a newline, so every one of them (not only the guide) was exposed to whatever a client does. Re-measured 2026-10-18 before the fix, on the wire with a build of this tree: `      }<!-- auto-injected get_guide('project-activation-bootstrap') ...`.

## Evidence

The plain-output byte dump above; `repro.sh` (line-oriented strip) reproduces the false reports, `repro4.sh` (JSON) does not.

## Hypotheses tried

None needed.

## Fix

Fixed 2026-10-18 on `fix/small-items-2026-10-18` at `1f673f644`, patch-id `c7f84f3c6ad3395c43a3117a3128421f9a62f18f` (not yet on `experiments`).

Every block codescout appends BEHIND a tool's answer is now built through one constructor, `trailing_block` (`src/tools/core/guide_emit.rs`), which puts a newline at the head of the text (idempotent: a block that already opens with one is left alone). The sites that were glued, each measured red first (the seam check named the block and the line it followed):

1. the whole-topic guide block (`guide_block`, also the session opener);
2. the oversize pointer (`whole_topic_blocks`);
3. the preamble fallback (`guide_blocks_for`);
4. the section block (`section_block`, which also serves the pre-call gate's held reply);
5. the operator-rule block (`engines/emitters.rs`);
6. the workspace-notice block that rides beside a result with no object to hold it (`Tool::call_content`, `src/tools/core/types.rs`);
7. the displaced-default notice appended to a refused write (`call_tool_inner`, `src/server.rs`);
8. the refusal-predicate block appended to a refused call (`call_tool_inner`).

Not changed, because already clean (and now pinned): the paths banner, the project-status block and the build notice in `post_process` all open with `\n` already.

Why a newline at the head of the block and not a separator in the client: the server cannot control how a client joins blocks, and a leading newline makes the marker start its own line for every line-oriented reader whatever joins them. The seam holds for all three primary shapes (compact text, JSON, buffered `@cmd_*`/`@tool_*` envelope), because the seam belongs to the appended block, not to the primary.

Measured on the wire, same call before and after (`symbols name=guide_block include_body=true`, `mcp call`, project = this worktree): before, `      }<!-- auto-injected get_guide('project-activation-bootstrap') ...` and `sed "/get_guide('project-activation-bootstrap')/,\$d"` kept `(out, shape)` as the last line (the function's closing `}` was lost); after, the marker is on line 148 by itself and the same `sed` keeps `      }`.

Byte pins moved, not loosened: `a_non_declaring_topic_is_byte_identical_to_today` (expected text gains the leading `\n`, the rest still pinned byte for byte) and `operator_rules_delivers_a_matching_rule_once_then_dedups` (strips the leading `\n` before splitting the wrapper from the imperative). The emission byte ceiling (`a_p50_session_stays_under_the_committed_emission_byte_ceiling`) and `MAX_AUTO_INJECT_GUIDE_BYTES` were not touched; the whole `cargo test --lib` stayed green with them as they were. The auto-inject bound is measured on the block INCLUDING the new byte, so it is one byte tighter on the body, which no shipped guide is near (largest whole guide ~14.7 KB of 16 KiB).

## Tests added

`seam_defects` (test instrument, `guide_emit.rs`): names every block that would begin mid-line when the blocks are laid end to end. It is itself tested (`the_seam_check_names_a_glued_block_and_passes_a_separated_one`), and `all_text`/`dispatch_text`, which join with `\n`, are not used for the seam.

- `src/tools/core/guide_emit.rs`: `a_whole_topic_guide_starts_on_its_own_line`, `an_oversize_pointer_starts_on_its_own_line`, `a_preamble_fallback_starts_on_its_own_line`, `a_served_section_starts_on_its_own_line`, `trailing_block_adds_exactly_one_leading_newline`.
- `src/engines/emitters.rs`: `an_operator_rule_block_starts_on_its_own_line`.
- `src/tools/core/tests.rs`: `a_notice_block_beside_a_non_object_result_starts_on_its_own_line`.
- `src/server.rs`: `an_injected_block_never_starts_mid_line_whatever_shape_the_primary_has` (text, JSON and buffered primaries through the real dispatch), `post_process_appended_blocks_each_start_on_their_own_line` (banner, status block, build notice), `the_displaced_notice_on_a_refused_write_starts_on_its_own_line`; seam assertions added to `the_first_destructive_call_is_held_and_serves_its_guide_before_the_side_effect` (held reply) and `a_refusal_carries_the_gate_condition_once_per_family`.

Red first: 10 of these failed before the fix (`block 1 starts mid-line: "<!-- auto-injected get_guide('librarian') § Filt" follows "result\": 1\n}"` and the like); `post_process_appended_blocks_each_start_on_their_own_line` passed before the fix, as expected from reading the three sites, so it is a pin, not a catch.

Mutation, one per site, each reverted: replacing `trailing_block(..)` by `Content::text(..)` at each of sites 1 to 8 was killed (1: 2 tests, 2: 1, 3: 1, 4: 3, 5: 1, 6: 1, 7: 1, 8: 1); dropping the newline inside `trailing_block`, making it always prefix, and prefixing a space were killed; removing the leading `\n` from the paths banner, the status block and the build notice was killed (the build notice only after a second case was added: behind the status block it is masked, because that block ends with a newline). 14 of 14 killed.

## Not covered

- The seam is a property of the blocks the SERVER emits. Whether a given client lays blocks end to end, or with its own separator, is the client's choice and is not tested here.
- Blocks built by a tool's own `call_content` override (only `onboarding` overrides it, and it returns one block) are outside `trailing_block`. A new appended block that does not go through `trailing_block` is caught only if it reaches a path one of the tests above exercises.

## Resume

N/A: fixed. Archive once this lands on `experiments`.

## Workarounds

Cut the guide with parameter expansion, `${RESULT%%<!-- auto-injected*}` (what `tests/mcp-smoke-kotlin.sh` does), or read `-f json` `content[0].text`. After the fix the expansion is no longer needed against a current binary, and it still works against an older one that glues.

## References

- `docs/issues/2026-10-17-symbols-include-body-on-kotlin-lsp-results-is-shifted-up-one-line.md`
- `tests/mcp-smoke-kotlin.sh` (`call`)
