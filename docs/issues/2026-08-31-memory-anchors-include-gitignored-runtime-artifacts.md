---
status: open
opened: 2026-08-31
closed:
severity: low
owner: marius
related: []
tags: [memory, anchors, staleness, false-positive, always-alarming, cross-machine]
kind: bug
---

# BUG: memory anchors include gitignored runtime artifacts, so five memories are stale by construction

## Summary

`memory_staleness` compares a content hash per anchored path. The anchor list is derived
from paths a memory's prose mentions, and it does not exclude **gitignored runtime
artifacts**. Five memories are consequently anchored to files that change without any
knowledge changing — including a lock file and a database — so they return to `stale`
within minutes of any refresh, permanently.

The signal is not merely noisy. It fails in the **always-alarming** direction, which is the
direction that trains a reader to ignore it. A memory whose content is genuinely wrong looks
identical to one anchored to `write.lock`.

## Symptom (Effect)

Measured 2026-08-31 at `765bba6a`, after refreshing anchors on every affected memory:

```
5 of 134 anchors are gitignored or absent:

  architecture      .codescout/embeddings/project.db  .codescout/project.toml
                    .codescout/workspace.toml         .codescout/write.lock
  conventions       .codescout/libraries.json         .codescout/project.toml
                    .codescout/write.lock
  domain-glossary   .codescout/project.toml           .codescout/write.lock
  gotchas           .codescout/embeddings/project.db  .codescout/project.toml
  project-overview  .codescout/project.toml
```

`.codescout/write.lock` is a **lock file**; `.codescout/embeddings/project.db` is a
**database rewritten on every index build**. Neither can stay hash-stable, so
`architecture`, `conventions`, `domain-glossary` and `gotchas` are stale by construction.

Two second-order effects observed in the same session:

1. **The real signal was drowned.** `architecture` reported "16 of 27 anchored files
   changed" and the genuine finding inside it — `src/tools/` had been regrouped into
   subdirectories, leaving **12 of 17** module-map paths unresolvable — was
   indistinguishable from lock-file churn. It had survived a whole reorganisation.
2. **Cross-machine, the comparison is meaningless.** These paths are gitignored
   (`.gitignore:11` for `project.toml`), but the memories themselves are tracked in git —
   42 of them. So a memory travels to another machine while its anchor does not, and the
   freshness verdict there is computed against a file that is absent or unrelated.

## Reproduction

1. `memory(action="refresh_anchors", topic="architecture")` → `"ok"`.
2. `workspace(action="status")` → `architecture` is absent from `stale`.
3. Perform any write, or run `index(action="build")`.
4. `workspace(action="status")` → `architecture` is stale again, reason naming
   `.codescout/write.lock` and/or `.codescout/embeddings/project.db`.

No knowledge changed at any step.

## Root cause (hypothesis — not yet confirmed in code)

The anchor extractor collects path-like tokens from memory prose without filtering. The
paths above appear in these memories legitimately — `architecture` describes the
`.codescout/` layout, and naming `write.lock` is the correct thing for prose about write
locking to do. So the defect is not the prose; it is that **being mentioned** and **being an
evidence anchor** are conflated.

## Fix options

1. **Exclude gitignored paths at anchor-selection time.** Cheapest, and matches the
   invariant that a tracked memory's anchors should be tracked too. Risk: a legitimately
   gitignored-but-stable anchor would silently stop being watched.
2. **Exclude by kind** — lock files, `*.db`, `*.sqlite`, anything under an
   `embeddings/`-style cache dir. Narrower, and directly targets the always-changing cases,
   but it is a denylist and will need extending.
3. **Report them in a separate bucket** (`anchors_untracked`) rather than as staleness, so
   the count stays visible without contaminating the verdict.

Recommend (1) with (3) as the reporting shape: the rule "a git-tracked memory's anchors must
be git-tracked" is checkable, states an invariant rather than a file-type list, and keeps the
information rather than discarding it.

## Consequence for now

The four affected memories were content-verified by hand this session and their anchors
refreshed. They will read `stale` again shortly, and that will not mean anything. Anyone
triaging memory staleness should check the `reason`'s `changed_files` list before believing
it, and discount the five paths above.

## Resume

Find the anchor extractor — start from `memory(action="refresh_anchors")`'s handler under
`src/tools/memory/` and the staleness computation that `workspace(action="status")` reads
(`memory_staleness` in its response). The filter belongs at anchor-selection time, not at
comparison time, so that refreshing actually settles the memory. A regression test wants a
fixture memory whose prose names a gitignored path, asserting the path does not enter the
anchors list — and note that asserting "the memory is fresh" is monotone under the anchor
list being empty, so assert on the anchor list's contents, not on the verdict.
