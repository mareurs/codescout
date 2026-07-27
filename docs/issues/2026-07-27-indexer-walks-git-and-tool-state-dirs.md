---
status: open
opened: 2026-07-27
closed:
severity: medium
owner: marius
related:
  - docs/issues/2026-07-27-ast-chunker-no-minimum-chunk-size.md
tags: [indexer, retrieval, ignore, gitignore]
kind: bug
---

# BUG: indexer walks `.git/`, `.codescout/`, `.claude/`, `.serena/` — tool state is indexed as project content

## Summary

`stream_index` sets `.hidden(false)` on the `ignore::WalkBuilder` so that tracked
dotfiles get indexed, with the comment "gitignore handles exclusions". It does
not: `.git/` is never listed in `.gitignore` (git has no reason to ignore its own
directory), and neither are codescout's / other agents' own state directories.
The result is that agent scratch files and codescout's own memories are embedded
into the code corpus and returned by `semantic_search` as if they were project
source.

## Symptom (Effect)

Measured against the live `code_chunks` collection for `project_id=backend-kotlin`
(2026-07-27, mid-index):

```
── top-level dirs ────────────
   28815  ktor-server
   12984  docs
     907  eduplanner-mcp
     377  .claude          ← agent commands/skills
     360  .codescout       ← codescout's OWN memories and plans
      79  .serena          ← another agent's state
      60  .git             ← git internals
```

876 chunks, ~2% of the corpus. What is under `.git/`:

```
chunks under .git/ : 60  distinct files: 9
  11  .git/sdd/task-2-report.md
  10  .git/sdd/task-1-report.md
   7  .git/sdd/task-4-report.md
   7  .git/sdd/task-3-report.md
   7  .git/sdd/task-2-brief.md
   6  .git/sdd/task-1-brief.md
   5  .git/sdd/task-3-brief.md
   4  .git/sdd/progress.md
   3  .git/sdd/task-4-brief.md
```

And under `.codescout/` — codescout indexing its own memory store into the
project's searchable code corpus:

```
  30  .codescout/projects/ktor-server/memories/development-commands.md
  27  .codescout/memories/gotchas.md
  23  .codescout/tracker-backfill-plan.md
  20  .codescout/projects/eduplanner-mcp/memories/architecture.md
```

No genuinely gitignored build output is affected — `build/`, `.gradle/`,
`node_modules/`, `target/`, `dist/`, `out/` all correctly return **zero** chunks.
The `.gitignore` handling works; the problem is confined to paths `.gitignore`
never mentions.

## Reproduction

Index any project that has a `.codescout/`, `.claude/`, or `.git/`-resident
scratch directory containing files with an indexable extension, then:

```
curl -s -X POST http://127.0.0.1:6333/collections/code_chunks/points/scroll \
  -H 'Content-Type: application/json' \
  -d '{"limit":2000,"with_payload":{"include":["file_path"]},"with_vector":false,
       "filter":{"must":[{"key":"project_id","match":{"value":"<project>"}}]}}'
```

Aggregate `file_path` by first path segment; dot-directories appear.

## Environment

- codescout binary built 2026-07-25 19:27 (`target/release/codescout`)
- Project: `/home/marius/work/mirela/backend-kotlin`
- Collection `code_chunks`, qdrant v1.17.0

## Root cause

`src/retrieval/sync.rs:113-119`:

```rust
for entry in ignore::WalkBuilder::new(root)
    .hidden(false) // index tracked dotfiles; gitignore handles exclusions
    .filter_entry(move |e| {
        let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
        !ignore_matcher.matched(e.path(), is_dir).is_ignore()
    })
```

`.hidden(false)` disables the `ignore` crate's hidden-entry filter. That filter is
what normally keeps `.git/` out of a walk — it is skipped for being hidden, not
for being gitignored. Turning it off to reach tracked dotfiles (`.github/`,
`.cargo/config.toml`) also opens `.git/` and every agent state directory.

The `filter_entry` closure is the intended escape hatch, but it consults only
`build_ignore_matcher(root, ignore_patterns)` — i.e. user-supplied patterns. There
is no built-in denylist.

## Evidence

See Symptom. The distinguishing observation is that gitignored build directories
are absent while dot-directories are present — which isolates the failure to the
hidden-filter change rather than to gitignore parsing.

## Hypotheses tried

1. **Hypothesis:** the indexer ignores `.gitignore` entirely (user's initial
   suspicion — "maybe it indexes .gitignored files").
   **Test:** counted chunks whose path contains any of `build/`, `.gradle/`,
   `node_modules/`, `target/`, `dist/`, `out/`, `.idea/`, `generated/`, `bin/`,
   `vendor/`, `coverage/`, `.cache/`.
   **Verdict:** rejected. All zero. `.gitignore` is honoured.

## Fix

Add a built-in denylist to the `filter_entry` closure, independent of user
patterns. At minimum `.git`; strongly suggested also `.codescout`, and
configurable coverage for other agent state dirs (`.claude`, `.serena`,
`.buddy`). `.git` should be unconditional — there is no case for indexing it.

Sketch:

```rust
const ALWAYS_SKIP_DIRS: &[&str] = &[".git", ".codescout"];
// in filter_entry, before the matcher:
if is_dir && e.file_name().to_str().is_some_and(|n| ALWAYS_SKIP_DIRS.contains(&n)) {
    return false;
}
```

Whether `.claude` / `.serena` should be skipped by default or left to
`ignore_patterns` is a judgement call — they can contain genuinely useful
project documentation (skills, command definitions), unlike `.git`.

Note this is cosmetic relative to total index cost (~2% of chunks); the
re-embed-duration problem is tracked separately in
`docs/issues/2026-07-27-ast-chunker-no-minimum-chunk-size.md`.

## Tests added

None yet. A regression test should assert that a fixture tree containing
`.git/foo.md` and `.codescout/memories/bar.md` produces zero chunks for those
paths.

## Workarounds

Add the directories to the project's `ignore_patterns` (`.codescout/project.toml`)
so `build_ignore_matcher` excludes them.

## Resume

Add `ALWAYS_SKIP_DIRS` to `src/retrieval/sync.rs:113` and a fixture test in the
`sync.rs` test module. Then re-index one project and re-run the aggregation
under "Reproduction" to confirm dot-directories are gone. Decide separately
whether `.claude` stays indexed.

## References

- `src/retrieval/sync.rs:113-119` — the walker and its `.hidden(false)`
- `src/embed/mod.rs` — `build_ignore_matcher`
- `docs/issues/2026-07-27-ast-chunker-no-minimum-chunk-size.md` — the actual
  driver of index duration
