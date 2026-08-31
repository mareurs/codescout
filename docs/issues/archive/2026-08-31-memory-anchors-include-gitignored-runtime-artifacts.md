---
kind: bug
status: fixed
tags:
- memory
- anchors
- staleness
- false-positive
- always-alarming
- cross-machine
closed: 2026-08-31
opened: 2026-08-31
owner: marius
related: []
severity: low
unverified: The code fix is covered by six tests and a five-mutation matrix, but the live shape of memory(action="refresh_anchors")'s new dropped_machine_local report has not been observed against a rebuilt server — this session's is git_sha 179ba3d7, pre-fix. The data repair IS verified live (workspace status re-reads the sidecar per call). Discharge by rebuilding, reconnecting, and calling refresh_anchors on a topic whose sidecar still carries a gitignored anchor; note that after this commit no such sidecar exists in-repo, so the trigger has to be manufactured or found in a sibling repo.
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

### Mechanism, corrected at fix time

The reproduction disagreed with the emphasis above, and the difference decides the fix.
Measured 2026-08-31 while fixing, on this machine:

- `.codescout/write.lock` last changed **2026-04-17**; `.codescout/embeddings/project.db`
  **2026-05-13** (the retrieval backend moved to remote Qdrant, so the local db is dead
  weight). Neither is churning here. The "rewritten on every index build" reading is what a
  reader would optimise for, and it is the half that is currently inert.
- What actually fires is second-order effect (2), and it is stronger than "meaningless":
  **`.codescout/project.toml`'s recorded hash was identical in all five sidecars and matched
  no file on this machine** (recorded `34e422b9…`, actual `1c616aaf…`, mtime three days
  before the re-anchor that supposedly refreshed it). The file is per-machine config —
  `extra_write_roots`, `shell_command_mode`, the embeddings URL.

So the live defect is a **cross-machine oscillation with no fixed point**, not churn. A
tracked sidecar records a hash of a gitignored file; machine A refreshes, commits A's hash, B
pulls and is permanently stale, and B refreshing flips A. That is worse than churn in kind,
not just degree: churn is self-healing on refresh, whereas here **the repair action is what
creates the next defect**, for someone not present to see it. Two machines cannot both be
fresh.

The corollary for the fix: the filter cannot be a churn-detector (a `*.db`/`*.lock`
denylist), because the worst instance is a small, stable, hand-edited TOML.
## Reproduction

1. `memory(action="refresh_anchors", topic="architecture")` → `"ok"`.
2. `workspace(action="status")` → `architecture` is absent from `stale`.
3. Perform any write, or run `index(action="build")`.
4. `workspace(action="status")` → `architecture` is stale again, reason naming
   `.codescout/write.lock` and/or `.codescout/embeddings/project.db`.

No knowledge changed at any step.

## Root cause (confirmed)

The anchor extractor collects path-like tokens from memory prose without filtering
(`path_re` in `src/memory/anchors.rs` admits `.codescout/[\w/._-]+\.\w+` deliberately, and
`seed_anchors` filtered only on `is_file()`). The paths above appear in these memories
legitimately — `architecture` describes the `.codescout/` layout, and naming `write.lock` is
the correct thing for prose about write locking to do. So the defect is not the prose; it is
that **being mentioned** and **being an evidence anchor** are conflated.

**Three write sites, not one** — and this is the part a fix could plausibly have missed.
Only `seed_anchors` creates anchors from prose; the other two carry existing ones forward:

| site | role | reached by a `seed_anchors`-only fix? |
|---|---|---|
| `seed_anchors` | creates anchors from prose | yes |
| `merge_anchors` | re-hashes and keeps an existing sidecar's paths | **no** |
| `refresh_hashes` | re-hashes the stored list, never re-seeds | **no** |

`memory(action="refresh_anchors")` calls `refresh_hashes` (`src/tools/memory/mod.rs:1142`),
which is what wrote the bad hashes in the first place: a faithful refresh of an anchor that
should not exist. Every affected memory already had a sidecar, so all five reach the two
unfiltered sites and none reach the filtered one. A fix landing only on `seed_anchors` would
have compiled, passed, and changed nothing observable.
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

## Fix

Option (1) from above, applied at all three write sites, with (3)'s reporting narrowed to the
one surface where it is not decoration.

**The rule.** A path `.gitignore` declares machine-local is never recorded in an anchor
sidecar. Keyed on `.gitignore` rather than a file-type denylist because the ignore file is the
project's own declaration of what does not travel — it states the invariant instead of
enumerating today's instances of it, and the corrected mechanism above rules the denylist out
anyway. Most of `.codescout/` stays anchorable (`memories/*.md`, `system-prompt.md`,
`librarian.toml` are all tracked); the discriminator is `.gitignore`, not the directory name.

- `src/util/gitignore.rs` (new) — `build_root_gitignore` + `is_machine_local`. The builder is
  lifted verbatim from `audit_doc_refs`' private copy, which now delegates to it, so there is
  one answer to "does this repo declare this path generated-or-local" rather than two. Its
  failure policy is unchanged and deliberate: any failure returns `None`, disabling the
  check rather than failing the call.
- `is_machine_local` uses `matched_path_or_any_parents`, **not** `matched`. `.gitignore:28`
  is `.codescout/embeddings/` — a *directory* rule — and `matched` tests only the path handed
  to it, so it answers "not ignored" for `project.db` inside it. That single call is the
  difference between catching 12 of the 12 bad anchors and catching 10.
- `src/memory/anchors.rs` — `machine_local()` carrying the *policy* rationale (the util
  carries the mechanism), applied in `seed_anchors`, `merge_anchors` and `refresh_hashes`.
- `refresh_hashes` now returns `Result<Vec<String>>` — the paths it dropped. `refresh_anchors`
  stays `json!("ok")` when that is empty and reports `dropped_machine_local` + a hint when it
  is not: silent when there is nothing to say, per
  `docs/adrs/2026-08-27-negative-results-name-their-scope.md`.

**Not filtered at comparison time**, following the bug's own `## Resume`. `check_path_staleness`
still evaluates whatever the sidecar holds, so a not-yet-repaired sidecar keeps producing its
false alarm — which is the thing that prompts the refresh that both repairs it and explains
why. Filtering there instead would hide the junk while leaving it in the file.

**Data repair — 12 anchors across 5 sidecars, deletion-only.** Deliberately *not* done by
running `refresh_anchors`, even though the fixed code would produce the same anchor list:
`refresh_hashes` also re-hashes the survivors, and `CLAUDE.md` had genuinely changed under
several of these memories. A full refresh would have silently asserted "reviewed against the
new CLAUDE.md" and destroyed exactly the real signal this bug says was being drowned. The
survivors' hashes and formatting are untouched (`git diff --stat`: 46 deletions, 0
insertions).

**Verified live before the rebuild**, because the repair is data and `check_path_staleness`
re-reads the sidecar per call — so the pre-fix server shows it:

| topic | before | after |
|---|---|---|
| `project-overview` | stale — `.codescout/project.toml` | **fresh** |
| `architecture` | 2 of 27 — `project.toml` + `CLAUDE.md` | 1 of **23** — `CLAUDE.md` only |
| `conventions` | 3 of 24 — `project.toml`, `libraries.json`, `CLAUDE.md` | 1 of **21** — `CLAUDE.md` only |
| `domain-glossary` | 2 of 16 | 2 of **14** |
| `gotchas` | stale — `.codescout/project.toml` | stale — `src/memory/anchors.rs` |

Anchor totals fell by exactly 4+3+2+2+1 = 12, and every surviving alarm names a file that
really changed — `CLAUDE.md` from `e0f98fcf`, `src/memory/anchors.rs` and
`src/tools/memory/mod.rs` from this fix. The always-alarming direction is closed without
closing the alarm.
**Fixed at `4bafaa81` on `experiments`**, patch-id
`3a7c54155b9f6f41953ed3398ae38931f20787da`. The SHA orphans when `experiments` is rebased;
the patch-id is a content hash of the diff and survives both rebase and cherry-pick.
## Tests added

Six, in `src/memory/anchors.rs`. The `## Resume` note this section replaces was right that
"the memory is fresh" is monotone under an empty anchor list, so every assertion is on the
anchor list's **contents**.

| test | what it pins |
|---|---|
| `seed_anchors_skips_a_path_gitignore_declares_machine_local` | the rule, at the seed site |
| `seed_anchors_still_records_a_dotcodescout_path_git_tracks` | the opposite direction |
| `a_gitignored_directory_rule_covers_a_file_inside_it` | `matched_path_or_any_parents` vs `matched` |
| `anchors_are_unfiltered_when_the_project_has_no_gitignore` | the degradation path |
| `merge_anchors_drops_a_gitignored_path_an_existing_sidecar_carries` | the merge site |
| `refresh_hashes_drops_gitignored_anchors_and_names_them` | the refresh site, and its report |

Two of the six exist only to fail in the **widening** direction. Four of them are absence
assertions, which a fix that refused to anchor anything under `.codescout/` — or anything at
all — satisfies completely; the pair above is what makes the cluster non-vacuous rather than
merely large.

**Mutation matrix**, run one mutation per guarded *site* rather than one per feature, because
a single kill would prove one line and imply two:

| mutation | tests killed |
|---|---|
| `matched_path_or_any_parents` → `matched` | 1 — the directory-rule test, alone |
| `is_machine_local` → always `true` | 10 — both twins, plus 6 pre-existing tests |
| disable the `seed_anchors` filter | 2 — seed + directory-rule |
| disable the `merge_anchors` filter | 1 — the merge test, alone |
| disable the `refresh_hashes` filter | 1 — the refresh test, alone |

Each site's test fails under its own mutation and under no other — so the three are
independently guarded, not covered once and assumed thrice. (The first RED run was a compile
error from the `refresh_hashes` signature change, which proves nothing; this matrix is what
stands in for it.)
