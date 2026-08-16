---
id: '403e3fad0356f171'
kind: bug
status: mitigated
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

**Memories shrink because `.codescout/memories/` is git-tracked.** Confirmed by
`git ls-files .codescout/` — 39 tracked memory files. A worktree is checked out
at some commit, so it serves *that commit's* memory set. A memory written after
that commit does not exist there. This is not corruption — it is git working
exactly as specified — but "read the project's memories" returns a different
answer depending on which tree is active, with nothing saying so.

**Sub-projects multiply because `.codescout/workspace.toml` is gitignored**
(`.gitignore:28`; confirmed absent from `git ls-files`). It does not travel into
a worktree.

**Correction, measured 2026-08-16 — the original wording of this section was
wrong about the mechanism.** It said discovery *"falls back to auto-detection"*.
There is no fallback, because there is no second mode: `Agent::load_project_resources`
calls `discover_projects` — a manifest walk — **unconditionally**, on every
`Agent::new` and every `activate`. `workspace.toml`'s `[[project]]` entries never
replace that walk; they only annotate its results with `depends_on`.

What the missing file actually removes is `load_discover_settings`'s two return
values — `exclude_projects` and `discovery_max_depth` — which fall back to
`(3, vec![])`. This repo's `workspace.toml` is:

```toml
exclude_projects = ["fixtures"]
[workspace]
discovery_max_depth = 3
```

`discovery_max_depth = 3` **is** the default, so the single operative difference
is `exclude_projects = ["fixtures"]`. That one line is the entire 2 → 9 gap: the
walk simply stops pruning `tests/fixtures/*`.

The distinction matters for the fix. "Discovery falls back to a dumber mode"
invites replacing the mode; "a prune list went missing" points at carrying the
settings, which is a much smaller change and is what option 3 should mean.

CLAUDE.md already names the class: a mis-rooted `workspace.toml` *"silently
redirects every per-project memory write into the wrong repo with no review able
to catch it."* A worktree reaches a neighbouring failure by **absence** rather
than by mis-rooting.
## Symptom (Effect)

- `memory(action="list")` in a worktree omits topics that exist on main, so an
  agent concludes a fact was never recorded and re-derives or re-writes it.
- A per-project memory write in a worktree can land against an auto-detected
  fixture "project" rather than the real one — the redirect CLAUDE.md warns about,
  arrived at from the other direction.
- Nothing in either surface reports that the topology it is describing was
  inferred rather than configured.

## Fix

**Option 2 implemented 2026-08-16 — report the divergence.** Options 1 and 3
remain open; option 2 was always compatible with either.

A linked-worktree activation now returns a `worktree` block:

```json
"worktree": {
  "main_root": "/repo",
  "memories_are_this_checkouts": "N memory topics come from THIS worktree's commit …",
  "topology": "inferred",          // or "configured"
  "topology_hint": "No .codescout/workspace.toml here …"
}
```

and the compact summary line — which is what most callers actually read — gains
`linked worktree · memories + topology are this checkout's (topology inferred)`.
A plain checkout gets neither.

Detection is filesystem-only and shared: `is_linked_worktree` /
`worktree_main_root` moved from `librarian::current_project` to
`util::path_security`, beside `list_git_worktrees`. `librarian` is an optional
feature and `tools::config` needs the same two facts, so worktree *detection*
and worktree *enumeration* now live in one place that no feature gates.
`current_project` re-exports them; its tests are unchanged.

### Still open — the semantic question

1. **Follow the main checkout for both.** Resolve `.codescout/` against the main
   worktree's root. Matches how the librarian catalog already treats a worktree
   (overlay onto main, fork on first write). Costs: a worktree can no longer hold
   memories about its own in-flight work.
3. **Carry `workspace.toml` into new worktrees.** Now known to be narrower than
   originally written: it is not "restore a discovery mode", it is "carry
   `exclude_projects` + `discovery_max_depth`". Fixes topology only, leaves
   memories diverging, and adds a file-sync obligation nothing else has.

The librarian's worktree overlay (`get_guide("librarian")` § Worktree overlay)
remains the precedent to read before deciding: it answered this question for
artifacts as "overlay onto main, fork on first write".
## Tests added

Five. The load-bearing one is the end-to-end activation — the three formatter
tests would all pass even if the block were never emitted:

- `activating_a_linked_worktree_reports_the_divergence_it_creates` — activates a
  real linked-worktree root (`.git` as a file with a `worktrees` component) and
  asserts `main_root`, `topology: "inferred"`, and the memory-provenance string.
  **Mutation-verified**: short-circuiting the `is_linked_worktree` branch
  reproduces exactly the pre-fix response, which carries no `worktree` key at all.
- `activating_a_plain_checkout_adds_no_worktree_block` — the common case pays
  nothing.
- three `format_activate_project` tests pinning the compact line for
  configured / inferred / plain.

The discriminating check this file asked for — "a worktree whose HEAD predates a
memory write: the memory must either be visible (option 1) or its absence
explained (option 2)" — is satisfied by the explanation half. The visibility half
belongs to option 1, if it is chosen.
## Workarounds

Activate the main checkout for memory work. There is no workaround for the
sub-project auto-detection short of creating `.codescout/workspace.toml` inside
the worktree by hand.

## Resume

**Mitigated, not fixed.** The divergence is reported; whether it *should* exist
is undecided, and that decision is the remaining work. Options 1 and 3 above are
mutually exclusive; option 2 has shipped and forecloses neither.

Do not re-derive the mechanism — and in particular do not trust the original
wording of § Root cause, which is corrected in place. The topology half is a
missing `exclude_projects` list, not a missing discovery mode, and that makes
option 3 substantially cheaper than this file first implied.

Still true, and still the reason not to treat the two halves as one bug:
memories diverge because a file IS tracked; topology diverges because a
different file is NOT.
