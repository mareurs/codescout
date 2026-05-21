---
status: fixed
opened: 2026-05-21
closed: 2026-05-21
severity: medium
owner: marius
related: []
tags: [run_command, post-process, path-display, output-fidelity, design-needed]
kind: bug
---

# BUG: run_command output strips project-root prefix, corrupting path-literal output (symlink targets, readlink, realpath)

## Summary
`server::post_process` blanket-strips the `<project_root>/` prefix from **all**
tool output content, including raw `run_command` stdout. For commands whose
output *is* a path literal — `readlink`, `ls -l <symlink>`, `realpath`, `pwd`,
`find` — this silently rewrites an **absolute** path into something that reads
as **relative**, changing its meaning. The disambiguation footer that would
warn the reader is capped at 3 emissions per session, so after three strips the
rewrite is completely silent. This caused a real misdiagnosis this session: an
absolute symlink target was read as relative, triggering a phantom "broken
symlink" investigation.

## Symptom (Effect)
`readlink ~/.cargo/bin/codescout` (a symlink whose target is the absolute path
`/home/marius/work/claude/code-explorer/target/release/codescout`) returned via
`run_command`:

```
target/release/codescout
```

`ls -l ~/.cargo/bin/codescout` showed `-> target/release/codescout` while the
reported link size was **63 bytes** — exactly the length of the *absolute*
target. The size/display mismatch is the tell: the stored target is absolute,
the displayed target was stripped to relative.

## Reproduction
```
git rev-parse HEAD   # 26618957ba1a9341bd61b421fc99b526b3c577aa
```
1. Create an absolute symlink under the project root:
   `ln -sf "$(pwd)/target/release/codescout" ~/.cargo/bin/codescout`
2. `run_command("readlink ~/.cargo/bin/codescout")`
3. Observe: output is `target/release/codescout` (project-root prefix stripped),
   not the absolute path the symlink actually stores.
4. Run any path-emitting command ≥4 times in one session; after the 3rd strip
   the `[codescout] paths are relative to …` footer stops appearing — the
   rewrite is then silent.

## Environment
- Project: code-explorer, branch `experiments`, HEAD `26618957`.
- MCP transport: codescout MCP server (release binary).
- Client: Claude Code.

## Root cause
Mechanism confirmed in source (not hypothesis):

`src/server.rs:334` `post_process()` computes
`root_prefix = "<project_root>/"` and calls
`strip_project_root_from_result(call_result, &root_prefix)` — a blanket textual
strip of that prefix across the entire result content. It is applied to every
tool, including `run_command`, whose content is raw, byte-faithful shell
stdout.

For most output the strip is a harmless token-saving cosmetic (the long
repeated prefix on `file:` fields). But for output that is *itself* a path
literal, the strip is semantically lossy: an absolute path
`/home/.../code-explorer/target/release/codescout` becomes
`target/release/codescout`, indistinguishable from a genuinely relative path.

Compounding: the "low hint" footer at `src/server.rs:350-358` that says
`[codescout] paths are relative to <root>` is gated by
`PATH_NOTE_MAX = 3` (`self.path_note_count`). After 3 emissions per session the
note is suppressed, so the lossy strip continues *without any marker*. A reader
several commands deep sees stripped paths with no disambiguation cue.

## Evidence
### E1 — size vs. display mismatch
`ls -l` reported the symlink at 63 bytes;
`len("/home/marius/work/claude/code-explorer/target/release/codescout") == 63`
while `len("target/release/codescout") == 24`. The 63-byte size proves the
stored target is absolute; the 24-char display proves it was stripped.

### E2 — strip site
`src/server.rs:334-362`:
```rust
let (mut call_result, stripped) = strip_project_root_from_result(call_result, &root_prefix);
// ...
const PATH_NOTE_MAX: usize = 3;
const PATH_NOTE_TOOLS: &[&str] = &["read_file", "run_command"];
if stripped && PATH_NOTE_TOOLS.contains(&tool_name) { /* emit footer, capped at 3 */ }
```
`run_command` is explicitly in `PATH_NOTE_TOOLS`, confirming raw shell output is
subject to the strip; the cap makes the warning disappear after 3 uses.

## Hypotheses tried
1. **Hypothesis:** The symlink was created relative / is broken.
   **Test:** counted bytes — 63 == absolute path length; re-created with an
   explicit absolute target and size stayed 63. **Verdict:** rejected — symlink
   was absolute all along; the display lied. **Evidence:** E1.
2. **Hypothesis:** `ls`/`readlink` themselves print relative.
   **Test:** located the strip in `post_process`. **Verdict:** confirmed — the
   codescout output post-processor strips the prefix, not the shell tools.
   **Evidence:** E2.

## Fix

Implemented option 1: `run_command` is now exempt from project-root
stripping. In `src/server.rs` `post_process`, the strip is gated
`should_strip = tool_name != "run_command"` — `run_command` stdout passes
through verbatim, while codescout's own structured tools (`read_file`, `tree`,
error messages, etc.) still get the redundant prefix stripped. Removed
`run_command` from `PATH_NOTE_TOOLS` (the disambiguation footer is now
unreachable for it and would be dead code). Updated the `post_process` doc
comment to record the exemption.

Options 2 (always-on marker) and 3 (strip only structured path fields) were
considered and not taken: for raw shell output, byte-faithfulness is simpler
and more correct than any marker scheme, and the token cost of not stripping
shell output is small.
## Tests added

`run_command_output_keeps_absolute_project_paths` in `src/server.rs` (tests
module) — runs `echo '<project_root>/some/nested/path'` through
`call_tool_inner` and asserts the absolute path survives verbatim in the
output. Fails (path stripped to `some/nested/path`) without the fix. The
existing `call_tool_strips_project_root_from_output` test (which uses `tree`)
still passes, confirming structured-tool stripping is unchanged.
## Workarounds
- Treat any path in `run_command` output as *possibly* stripped of the project
  root — a leading-slash-absent path may actually be absolute under the repo.
- Cross-check path-literal commands with a length/size signal (as in E1) or run
  them from a cwd outside the project root.
- For symlink verification specifically, compare `stat`/`ls -l` byte size
  against expected absolute vs relative target lengths.

## Resume

N/A — fixed and verified (build + clippy + tests green). Residual to consider
separately: `read_file` raw content has the same latent exposure (a path
literal at a value boundary in file content would be stripped); not addressed
here because this bug was scoped to `run_command`.
## References
- Strip site: `src/server.rs:334` (`post_process`), `src/server.rs:350-358`
  (footer cap `PATH_NOTE_MAX`).
- Path-relativization helper pattern: `src/fs/mod.rs:148`.
- Discovered during the artifact_event payload-schema fix session
  (`docs/issues/2026-05-21-artifact-event-create-payload-rejected.md`).
