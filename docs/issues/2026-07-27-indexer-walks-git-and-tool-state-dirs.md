---
kind: bug
status: fixed
tags:
- indexer
- retrieval
- ignore
- gitignore
closed: null
opened: 2026-07-27
owner: marius
related:
- docs/issues/2026-07-27-ast-chunker-no-minimum-chunk-size.md
severity: medium
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

Fixed on `experiments`, taking the sketch this section proposed and its judgement call.

`src/retrieval/sync.rs` gains a denylist consulted before the user-pattern matcher, so it
holds regardless of `.gitignore` or per-project `ignore_patterns`:

```rust
pub(crate) const ALWAYS_SKIP_DIRS: &[&str] = &[".git", ".codescout"];

pub(crate) fn is_always_skipped(name: &str, is_dir: bool) -> bool {
    is_dir && ALWAYS_SKIP_DIRS.contains(&name)
}
```

**Directory-only**, which the sketch did not specify and which matters: a *file* named
`.git` is a worktree pointer, not a state tree. Skipping by bare name would be a different
and wrong decision.

**The judgement call, decided as the entry recommended:** `.claude`, `.serena`, and
`.buddy` are deliberately absent. They can hold genuine project documentation — skills,
command definitions, prompts — so excluding them is a per-project decision belonging in
`ignore_patterns`, not a default. `.git` and `.codescout` have no such case: both are tool
state *derived from* the project, so indexing them makes the corpus self-referential and
`semantic_search` starts returning codescout's own memories as if they were source.

The `.hidden(false)` comment was also rewritten. It read "gitignore handles exclusions",
which is the belief that caused this — it now says why the denylist is load-bearing rather
than belt-and-braces.
## Tests added

One, `always_skipped_covers_git_and_codescout_state_only_as_directories` in
`src/retrieval/sync.rs`. The walk itself needs a live Qdrant, so the skip decision was
extracted into `is_always_skipped` to make it assertable on its own — the same move that
put `skip_lead_region` and `anchor_indent` outside their LSP-dependent callers.

It asserts all four quadrants rather than just the happy path: the two names skipped as
directories, the same two *not* skipped as files, the three agent dirs deliberately left
in, and that matching is whole-name rather than prefix (`.gitlab-ci` survives). A mutation
dropping the `is_dir` conjunction, or switching `contains` to a `starts_with`, fails.

Full gate: 18 binaries, 3458 passed, 0 failed, 44 ignored; clippy clean.
## Workarounds

Add the directories to the project's `ignore_patterns` (`.codescout/project.toml`)
so `build_ignore_matcher` excludes them.

## Resume

Fixed and unit-verified. Two follow-ons, neither blocking:

1. **The effect is not retroactive.** Chunks already embedded from `.git/` and
   `.codescout/` stay in the collection until those files are re-walked and reconciled;
   nothing prunes them on the strength of a new denylist. A full reindex clears them — and
   that cost is the reason to read
   `docs/issues/2026-07-27-ast-chunker-no-minimum-chunk-size.md` first, which is what makes
   a full re-embed expensive.
2. **Master-side SHA** after cherry-pick.

The entry's own framing still holds: this is ~2% of chunks, so it is a search-quality fix
rather than a cost fix. The cost problem is the chunker entry above.
## References

- `src/retrieval/sync.rs:113-119` — the walker and its `.hidden(false)`
- `src/embed/mod.rs` — `build_ignore_matcher`
- `docs/issues/2026-07-27-ast-chunker-no-minimum-chunk-size.md` — the actual
  driver of index duration
