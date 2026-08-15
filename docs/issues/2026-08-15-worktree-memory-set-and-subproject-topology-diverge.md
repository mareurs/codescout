---
id: '403e3fad0356f171'
kind: bug
status: open
title: 'BUG: a worktree activation serves that commit''s memories and auto-detects sub-projects, so memory set and topology silently diverge from the main checkout'
owners:
- marius
tags:
- worktree
- memory
- workspace-toml
- sub-projects
- silent-divergence
- gitignore
---

## Summary

Split out of
`docs/issues/archive/2026-08-13-enter-worktree-desyncs-codescout-and-strands-semantic-search.md`
(half 3) on 2026-08-15, per that file's own § Resume. Untouched by the
worktree-semantic-search work, which fixed a different half.

Activating a linked worktree changes **which memories exist** and **what the
sub-project topology looks like**, silently and in opposite directions:

| | main checkout | worktree activation |
|---|---|---|
| memory topics | 21 | **11** |
| sub-projects | 2 | **9** (every `tests/fixtures/*`) |

Measured 2026-08-13.

## Root cause

Two different mechanisms, one shared consequence.

**Memories shrink because `.codescout/memories/` is git-tracked.** A worktree is
checked out at some commit, so it serves *that commit's* memory set. A memory
written after that commit does not exist there. This is not corruption — it is
git working exactly as specified — but it means "read the project's memories"
returns a different answer depending on which tree is active, with nothing saying
so.

**Sub-projects multiply because `.codescout/workspace.toml` is gitignored**
(`.gitignore:28`). It is absent in the worktree, so sub-project discovery falls
back to auto-detection, which finds every `tests/fixtures/*` and calls each one a
sub-project.

The second is the more dangerous of the two, and CLAUDE.md already names the
class: a mis-rooted `workspace.toml` *"silently redirects every per-project memory
write into the wrong repo with no review able to catch it."* A worktree reaches
the same failure by **absence** rather than by mis-rooting — the file is not
wrong, it simply is not there, and the fallback is confident.

## Symptom (Effect)

- `memory(action="list")` in a worktree omits topics that exist on main, so an
  agent concludes a fact was never recorded and re-derives or re-writes it.
- A per-project memory write in a worktree can land against an auto-detected
  fixture "project" rather than the real one — the redirect CLAUDE.md warns about,
  arrived at from the other direction.
- Nothing in either surface reports that the topology it is describing was
  inferred rather than configured.

## Fix

Not designed. The decision is what a worktree activation *should* mean, and it is
genuinely open:

1. **Follow the main checkout for both.** Resolve `.codescout/` against the main
   worktree's root, so memories and topology are shared. Matches how the librarian
   catalog already treats a worktree (overlay onto main, fork on first write) and
   is the most consistent with the shipped model. Costs: a worktree can no longer
   hold memories about its own in-flight work.
2. **Keep per-worktree, but say so.** Leave the divergence and report it — the
   memory list names the commit it came from, and sub-project discovery marks
   itself `inferred` when no `workspace.toml` was found. Cheapest, and closes the
   *silent* half without deciding the semantic question.
3. **Copy `workspace.toml` into new worktrees.** Fixes topology only, leaves
   memories diverging, and adds a file-sync obligation nothing else in the design
   has.

**Option 2 is the recommended first move regardless of where 1 vs 3 lands** — the
divergence being invisible is the part that bites, and marking an inferred
topology as inferred is useful even if the topology is later shared. Sequence it
first; it does not foreclose either of the others.

The librarian's worktree overlay (see `get_guide("librarian")` § Worktree overlay)
is the precedent to read before deciding: it already answered this question for
artifacts, and answered it as "overlay onto main, fork on first write".

## Tests added

None. When implemented, the discriminating check is a worktree whose HEAD predates
a memory write: the memory must either be visible (option 1) or its absence
explained (option 2). Silently missing is the current behaviour and must fail.

## Workarounds

Activate the main checkout for memory work. There is no workaround for the
sub-project auto-detection short of creating `.codescout/workspace.toml` inside
the worktree by hand.

## Resume

Open, and independent of the read-side guard gap filed alongside it. This one
needs a **decision** before any code: options 1 and 3 are mutually exclusive, and
option 2 is compatible with either.

Do not treat the two halves as one bug. Memories diverge because a file IS tracked;
topology diverges because a different file is NOT. They share a symptom and nothing
else.

