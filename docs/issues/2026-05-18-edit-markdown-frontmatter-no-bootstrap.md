---
status: open
opened: 2026-05-18
closed:
severity: medium
owner: marius
related: [src/tools/edit_markdown/]
tags: [edit_markdown, frontmatter, dx]
---

# BUG: `edit_markdown(frontmatter:{set})` refuses to bootstrap a frontmatter block on files that have none

## Summary

When a markdown file has no YAML frontmatter block at the start,
`edit_markdown(frontmatter={"set": {...}})` returns
`"file has no frontmatter block (must start with ---)"` instead of creating
the block. This makes the tool unable to bootstrap frontmatter retroactively
— a common case when adopting a frontmatter-driven schema like the librarian's
`kind`/`status` convention.

## Symptom (Effect)

```
{
  "ok": false,
  "error": "file has no frontmatter block (must start with `---`)",
  "hint": "frontmatter editing is only valid on files with a `---`-delimited YAML block at the start of the file."
}
```

Reproduced 13 times in a single batch on session 2026-05-18 when backfilling
`docs/trackers/*.md` with librarian frontmatter.

## Reproduction

```
# any plain markdown file with no leading ---
echo "# Hello\n\nbody" > /tmp/repro.md

# fails with the error above:
edit_markdown(path="/tmp/repro.md",
              frontmatter={"set": {"kind": "tracker", "status": "active"}})
```

Branch: `experiments`. HEAD: `136f4c48`.

## Environment

- codescout MCP server, release build.
- Linux 7.0.0-15-generic, bash.

## Root cause

`Unknown — under investigation.` Best lead: the frontmatter mutation path in
`src/tools/edit_markdown/` (or whatever module owns the YAML block parsing)
gates on `---` being the first line and bails when absent, rather than
synthesizing a new block before applying `set:`.

## Evidence

13 consecutive failures with the same error in the same response, all on
files starting with `#` (heading), no `---` block. Workaround was to fall
back to `edit_file(insert="prepend", new_string="---\n...\n---\n\n")` —
which worked uniformly.

## Hypotheses tried

1. **Hypothesis:** Pass `set: {}` plus a non-mutating no-op first.
   **Test:** N/A — would still trip the gate.
   **Verdict:** deferred.

## Fix

`Unknown.` Proposed direction: when `frontmatter:{set:...}` is non-empty
AND the file has no `---` block, synthesize one at the head of the file
with the provided keys, in canonical key order. The `delete:` array on a
no-block file is a no-op (idempotent). Mirror what `edit_file(prepend)`
achieves today, but inside the tool.

## Tests added

`N/A — bug only filed, fix not implemented yet.` Future fix should add:
- `frontmatter_set_bootstraps_block_when_absent`
- `frontmatter_delete_on_no_block_is_noop`

## Workarounds

Use `edit_file(insert="prepend", new_string="---\n<keys>\n---\n\n")` to
synthesize the block once, then `edit_markdown(frontmatter:{set:...})`
works on subsequent edits. Verified in the same session — the 14-file
backfill commit `136f4c48` uses exactly this pattern.

## Resume

Read `src/tools/edit_markdown/` (likely `mod.rs` or `frontmatter.rs`) to
find the gate. Add a `bootstrap_if_absent` branch under the `set:` arm
that writes a fresh `---...---` block before applying the merge-patch.
Add the two regression tests above. Confirm `edit_markdown(frontmatter:
{set:...})` on a body-only file now succeeds and produces a clean block
at the head. Wire the existing key-ordering / value-encoding path into
the bootstrap so the synthesized block matches what `set:` would produce
on a real file.

## References

- Discovered 2026-05-18 while backfilling `docs/trackers/*.md` frontmatter
  for librarian indexing (commit `136f4c48`).
- Workaround pattern lives in that commit.
