---
id: ece908f37854e557
kind: bug
status: fixed
title: post_process strips the project root from file CONTENT and collapses root-valued fields to ""
owners:
- marius
tags:
- post-process
- path-display
- output-fidelity
- data-loss
- memory-corruption
- design-needed
closed: 2026-08-09
opened: 2026-08-09
related:
- docs/issues/archive/2026-05-21-run-command-strips-project-root-from-path-literals.md
- docs/issues/archive/2026-07-18-tree-strip-bare-root-not-stripped.md
- docs/issues/archive/2026-07-09-residual-workspace-pin-gaps-post-edit-code-fix.md
severity: high
---

# BUG: post_process strips the project root from file CONTENT and collapses root-valued fields to ""

> **Reading this file through codescout's own tools will corrupt it.** The absolute
> paths quoted below are displayed stripped by `read_markdown` / `read_file` / `grep` —
> that is the bug documenting itself. Verify any literal here with
> `run_command` (exempt from stripping), e.g. `grep -n "home/marius" <this file>`.

## Summary
`server::post_process` strips the `<project_root>` prefix from **all** text in a
tool result, not just from path fields. Two consequences: (1) a path literal inside
**file content** is rewritten, so an edit keyed on what `read_file`/`grep` displayed
fails as "not found" — and the error's own "Nearest content" hint is filtered through
the same transform, so both strings look identical; (2) a value that *equals* the bare
root collapses to the **empty string**, which has been silently corrupting
`workspace(activate).project_root` and the librarian's `scope.abs_path` in real sessions.

This is the residual explicitly deferred by
`docs/issues/archive/2026-05-21-run-command-strips-project-root-from-path-literals.md`
(§ Resume: *"`read_file` raw content has the same latent exposure … not addressed here
because this bug was scoped to `run_command`"*). It was recorded only in an archived
file's Resume line — a surface nothing re-reads — and went unactioned for 80 days.

## Symptom (Effect)

### S1 — file content rewritten (the reported case)
A source line containing the project root as a quoted literal is displayed with the
root removed. Probe file written and read back on 2026-08-09:

```
on disk (run_command, exempt):
  REPO = "/home/marius/work/claude/codescout/.worktrees/single-stage"
  OTHER = "/home/marius/work/claude/codescout"
  113 bytes

displayed by read_file AND grep:
  REPO = ".worktrees/single-stage"
  OTHER = ""
```

An `edit_file` keyed on the displayed text fails:

```
{"ok": false,
 "error": "old_string not found in .codescout/tmp/strip-probe.txt",
 "hint": "No exact or whitespace-normalized match. Copy the actual bytes shown (or from read_file) and retry."}
```

The hint directs the caller back to `read_file` — the tool that produced the wrong bytes.

### S2 — root-valued fields collapse to "" (not previously reported, worse)
`workspace(action="activate")`, this session:

```
{"status": "ok", "project": "codescout", "project_root": "", "read_only": false, ...}
```

`artifact(action="find")`, this session:

```
"scope": {"applied": "repo", "abs_path": "", "git_root": "", "umbrella": "codescout-ecosystem"}
```

`memory(action="read", topic="claude-code-mcp-env")` renders line 43 of
`.codescout/memories/claude-code-mcp-env.md` as:

```
| `CLAUDE_PROJECT_DIR` | `` | **v2.1.139** | authoritative launch-project hint ... |
```

On disk that cell holds `/home/marius/work/claude/codescout`. An agent reading the
memory concludes the variable carries no value.

## Reproduction
```
git rev-parse HEAD   # c5f434dfe7fe0af7e021db581f52ab1d046ad709 (experiments)
```
1. `run_command`: `printf 'A = "%s/sub"\nB = "%s"\n' "$(pwd)" "$(pwd)" > .codescout/tmp/probe.txt`
2. `read_file(".codescout/tmp/probe.txt")` → `A = "sub"` and `B = ""`.
3. `edit_file(path=..., old_string='A = "sub"', new_string='A = "x"')` → `old_string not found`.
4. `workspace(action="activate", path="<this repo>")` → `"project_root": ""`.

## Environment
- Project codescout, branch `experiments`, HEAD `c5f434df`.
- MCP transport: codescout MCP server (release binary); client: Claude Code.
- Reported from a *different* repo (`mirela/backend-kotlin`), so this is not
  codescout-specific — it is the transform, wherever it is active.

## Root cause
Mechanism confirmed in source and measured at runtime.

`src/server.rs:527` `post_process()` builds `root_prefix = "<project_root>/"` and calls
`strip_project_root_from_result` (`src/server.rs:1662`), a blanket textual strip over
every `RawContent::Text` block. `run_command` is the only exemption
(`should_strip = tool_name != "run_command"`).

The per-occurrence decision is `strip_prefix_from_text` (`src/server.rs:1702`). It
accepts a match when the preceding character is **not** one of `/ . - _` or
alphanumeric. A quoted path literal in source code is preceded by `"` — a value
boundary — so **the heuristic admits exactly the case its own doc comment claims it
excludes** (*"avoids stripping the prefix when it appears embedded inside longer
strings such as code literals or comments"*). The guard only rejects mid-path matches
like `/opt/home/marius/…`.

Two doc comments are therefore false as written:
- `src/server.rs:1702` — the "code literals are protected" claim above.
- `src/server.rs:1662` — *"values like `"project_root": "/abs/path"` … use a bare
  absolute path without a trailing slash, so they do not match `root_prefix` and pass
  through unchanged."* True when written; **repealed** by the bare-root branch added
  for `tree` (`docs/issues/archive/2026-07-18-tree-strip-bare-root-not-stripped.md`),
  which made bare roots matchable and turned every root-valued field into `""`.

*Measured 2026-08-09* (probe above, plus a 51-transcript sweep — see Evidence).
The mechanism is both read from source and observed at runtime.

## Evidence

### E1 — transcript sweep (measured 2026-08-09)
Corpus: 51 Claude Code transcripts for this project across `~/.claude`,
`~/.claude-sdd`, `~/.claude-kat` — 117,717 JSONL lines, 16,083 tool results,
21,732,941 bytes of tool output. Scripts:
`<session scratchpad>/measure_strip.py`, `measure_strip2.py`, `inspect_empty.py`.

| Measure | Value |
|---|---|
| Yield **ceiling** of the strip (assumes every relative path was stripped from an absolute original) | 4.79% of pre-strip bytes |
| Root-valued fields rendered `""` | **136**, across **12 distinct sessions** (6.4% of such fields: 136 empty vs 2,000 filled) |
| — of those, `artifact` `scope.abs_path` | 115 |
| — of those, `workspace` `project_root` | 21 |
| Absolute roots that survived in codescout tool output | 136 (120 genuine; 16 are companion-hook IL4 denials that never pass through `post_process`) |
| — genuine leaks defeated by the JSON-escape blind spot | **102 of 120 (85%)** — `tree` 56/56, `grep` 40/58, `symbols` 6 |

### E2 — the strip also fails to fire (under-strip)
In a buffered result the content block is serialized JSON, so a newline inside a
string value is the two literal characters `\` `n`. `strip_prefix_from_text`'s
lookbehind sees `n`, classifies it as a path character, and skips the match.
Observed live this session in a `symbols` overflow envelope:

```
"summary": "23 matches in 3 files\n\n/home/marius/work/claude/codescout/src/tools/core/tests.rs (1)"
```

So the transform corrupts content it must not touch **and** silently declines to strip
real path fields — failing in both directions from the same root cause: it runs on
rendered text, after the structure that would disambiguate it is gone.

### E3 — the diagnostic shares its victim's transform
`not_found_msg` (`src/tools/edit_file/mod.rs:196`) formats
`"old_string not found in {path}. Nearest content at lines {s}-{e}:\n{text}"` where
`text` is **raw file content** from `nearest_window_hint` (`src/tools/edit_file/mod.rs:157`).
That error returns through `post_process` and is stripped identically to the
`read_file` output it exists to disambiguate, so the two strings render the same and
the mismatch is unfalsifiable from inside the session. Original reporter resolved it
only with `run_command` + `od -c`.

### E4 — a third prompt surface is stale
`get_guide("progressive-disclosure")` § *Path-relative annotation* states the
annotation rides on *"[e]very non-`run_command` tool response"* (it is now novelty-gated
to once per activation, `src/server.rs:558`), and recommends verifying against catalog
state by *"reading the buffer directly (`read_file(@tool_xxx, json_path=...)`)"* — a
path `strip_project_root_from_result`'s own doc comment confirms is stripped too. Only
`run_command` escapes.

## Hypotheses tried
1. **Hypothesis:** the boundary heuristic already protects quoted literals, so only
   exotic content is at risk.
   **Test:** wrote the probe in S1 and read it back through `read_file` and `grep`.
   **Verdict:** rejected — `"` is a boundary character, so quoted literals are the
   *most* exposed shape, not a protected one. **Evidence:** S1.
2. **Hypothesis:** the empty `abs_path` fields are genuinely-empty values, unrelated
   to stripping.
   **Test:** `inspect_empty.py` printed surrounding context for every occurrence.
   **Verdict:** rejected — every one is a field whose value *is* the root
   (`scope.abs_path`, `scope.git_root`, `project_root`); sibling item paths in the same
   response stripped correctly to `docs/trackers/…`. **Evidence:** E1, S2.
3. **Hypothesis:** the JSON-escape under-strip is a rare curiosity.
   **Test:** classified all surviving absolute roots by preceding character across the
   corpus. **Verdict:** confirmed as the dominant leak — 85% of genuine leaks.
   **Evidence:** E1, E2.
4. **Hypothesis (open):** the 12 `symbols` leaks classified `start-of-text` should have
   stripped (`pos == 0` satisfies the left-boundary test). **Verdict:** unexplained;
   not investigated. Candidate causes: no active project at the time, or a different
   root pinned. Low volume; does not change the design.

## Fix

Implemented across commits `1a30e91e`..`358113ff` on `experiments`, per the accepted
design (`docs/superpowers/specs/2026-08-09-field-aware-path-strip-design.md`, now
`status: active`):

- `1a30e91e` — added `src/tools/core/path_strip.rs`: `PATH_KEYS` (allowlist of
  path-valued fields) and `ROOT_KEYS` (root-valued fields that stay absolute), plus
  `strip_paths_in_value`, a pure recursive walker over `serde_json::Value`.
- `91757be4` — hardened the new module's tests, restored a prefix-contract assertion.
- `cfd2c44e` — wired `strip_paths_in_value` into `Tool::call_content`
  (`src/tools/core/types.rs:564`), strictly before `exceeds_inline_limit` / the
  `@tool_*` buffer payload / `format_compact`, so every downstream consumer sees
  already-relative values.
- `8ddce9bf` — asserted the buffered `@tool_*` payload is itself stripped, not just
  the inline response.
- `93565509` — reduced `post_process` to the once-per-activation banner only; deleted
  its text-rewriting responsibility (and, with it, `strip_project_root_from_result`
  and `strip_prefix_from_text` — the two stale doc comments named in Root Cause and
  in E4 are gone with their functions).
- `7da8d1e2` — fixed a double-banner regression (the novelty gate was reset after
  `call_tool_inner`'s own `post_process` call rather than before it). Net effect: the
  `activate_project` response itself — not the next ordinary call — carries the
  banner, exactly once per activation.
- `74156e99` — added the corpus gate (`no_absolute_project_paths_in_rendered_output`)
  that fails CI when a tool's absolute project path reaches rendered output outside
  `run_command`/errors.
- `358113ff` — gave that corpus gate a per-case liveness guard, so a case that stops
  exercising the tool (error envelope, empty output, wrong branch) fails loudly
  instead of the absence check passing silently.

`run_command` needs no special case any more: its `stdout` key is simply absent from
`PATH_KEYS`, so raw shell bytes are left verbatim by the allowlist itself rather than
by a tool-name branch. Errors are never stripped (unaffected — they never routed
through the deleted functions' text path in a way the allowlist now touches).

## Tests added

- `src/tools/core/path_strip.rs` (unit): `relativizes_a_path_key`,
  `leaves_file_content_untouched`, `never_produces_an_empty_string`,
  `root_keys_stay_absolute`,
  `scope_block_abs_path_survives_while_item_abs_path_relativizes`,
  `relativizes_an_array_of_paths`, `recurses_into_nested_objects_and_arrays`,
  `unknown_key_keeps_its_absolute_path`, `empty_prefix_is_a_no_op`,
  `a_path_outside_the_root_is_untouched`,
  `a_sibling_directory_sharing_the_root_as_a_prefix_is_untouched`,
  `a_root_key_prunes_recursion_beneath_it`,
  `a_path_key_whose_value_contains_the_root_mid_string_is_untouched`.
- `src/tools/core/tests.rs` (through `call_content`):
  `call_content_relativizes_path_keys_but_not_content`,
  `call_content_buffered_summary_is_built_from_the_stripped_value`.
- `src/server.rs` (regression coverage for `post_process` + the banner):
  `post_process_annotates_against_the_pinned_root_without_mutating_text`,
  `responses_emit_paths_relative_annotation_once_per_activation`,
  `activation_and_the_next_two_calls_carry_the_banner_exactly_once`,
  `edit_failure_hint_reproduces_the_files_real_bytes` (closes E3 — the
  not-found diagnostic no longer shares its victim's transform, because there is no
  transform left to share), `read_file_and_grep_show_a_path_literal_in_content_verbatim`
  (closes S1, the originally reported case).
- `src/server.rs` (CI gate): `no_absolute_project_paths_in_rendered_output` (the
  corpus gate), hardened with a per-case liveness guard in `358113ff`.

`cargo test` on `experiments` at `358113ff`: 3596 passed / 0 failed / 44 ignored.

## Workarounds
- Treat any path shown by a non-`run_command` codescout tool as **possibly** stripped;
  a string that reads as relative may be absolute on disk, and an empty string may be
  the project root.
- To see true bytes, use `run_command` (exempt): `grep -n PATTERN <file>`, `od -c`,
  `wc -c`. A byte-count mismatch against the displayed text is the tell.
- When an edit fails "not found" on a file that contains the project root, re-read the
  region through `run_command` and key the edit on those bytes.
- Do not trust `project_root` / `scope.abs_path` / `git_root` when they come back
  empty — read the root from `run_command "pwd"` instead.

## Resume

N/A — fixed and verified. Implemented across `1a30e91e..358113ff` (commit-by-commit
breakdown in `## Fix` above); documentation on all three stale prompt/doc surfaces
corrected in `2aecc0bf`. Nothing left to resume here — the only remaining step is
archiving this file, which per this project's bug-tracking discipline happens after
the whole-branch review, not before.

## References
- Strip sites: `src/server.rs:527` (`post_process`), `src/server.rs:1662`
  (`strip_project_root_from_result`), `src/server.rs:1702` (`strip_prefix_from_text`).
- Chokepoint for the fix: `src/tools/core/types.rs:546` (`Tool::call_content`).
- Diagnostic that shares the transform: `src/tools/edit_file/mod.rs:196`.
- Origin of the deferred residual:
  `docs/issues/archive/2026-05-21-run-command-strips-project-root-from-path-literals.md`.
- Bare-root branch that repealed the `project_root` guarantee:
  `docs/issues/archive/2026-07-18-tree-strip-bare-root-not-stripped.md`.
- Pin-mismatch class this fix makes unrepresentable:
  `docs/issues/archive/2026-07-09-residual-workspace-pin-gaps-post-edit-code-fix.md`.
- Design: `docs/superpowers/specs/2026-08-09-field-aware-path-strip-design.md`.
