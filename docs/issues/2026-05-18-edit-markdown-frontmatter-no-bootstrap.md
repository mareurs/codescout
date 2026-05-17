---
status: fixed
opened: 2026-05-18
closed: 2026-05-18
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


Landed in commit `4011ed2a` on `experiments`. Implementation:
`apply_frontmatter_mutation` in `src/tools/markdown/edit_markdown.rs:299`
now branches on `extract_frontmatter().is_none()`:

- **`set:` non-empty:** synthesize a fresh block via `apply_ops(&[], set,
  delete)`, prepend `---\n<block>\n---\n` to the original content. A
  separator newline is added when the body does not already start with
  one (so `# Heading` gets blank-line-separated, but `\nbody` does not
  get double-blanked).
- **`delete:`-only, `set:` empty:** return the original content unchanged
  (idempotent — nothing to delete from a non-existent block).
- **Existing block:** unchanged — `apply_ops` + `splice_back` flow.

The old hard-error path is removed entirely.
## Tests added


In `src/tools/markdown/tests.rs`:

- `frontmatter_set_bootstraps_block_on_file_without_frontmatter` — bootstrap on body-only file
- `frontmatter_bootstrap_does_not_double_blank_when_body_already_blank_first` — leading-blank body is preserved verbatim
- `frontmatter_bootstrap_on_empty_file_produces_block_only` — empty file becomes a block-only file
- `frontmatter_delete_only_on_file_without_frontmatter_is_noop` — `delete:`-only on no-block file returns original

Plus removed the superseded test
`frontmatter_on_file_without_frontmatter_errors_with_hint` which pinned
the old wrong-behavior contract.

9/9 frontmatter tests pass; release build clean; clippy `-D warnings` clean.
## Workarounds

Use `edit_file(insert="prepend", new_string="---\n<keys>\n---\n\n")` to
synthesize the block once, then `edit_markdown(frontmatter:{set:...})`
works on subsequent edits. Verified in the same session — the 14-file
backfill commit `136f4c48` uses exactly this pattern.

## Resume


Shipped on `experiments` as `4011ed2a`. Standard Ship Sequence next:
cherry-pick to `master`, then `git mv` this file to `docs/issues/archive/`.
Live verification: restart MCP (`/mcp`), then run `edit_markdown` with
`frontmatter:{set:{...}}` on a body-only markdown file; expect a
synthesized block at the head.
## References

- Discovered 2026-05-18 while backfilling `docs/trackers/*.md` frontmatter
  for librarian indexing (commit `136f4c48`).
- Workaround pattern lives in that commit.
