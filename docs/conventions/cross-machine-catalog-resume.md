---
kind: convention
status: active
title: Cross-Machine Catalog Resume
owners: [marius]
tags:
  - librarian
  - catalog
  - onboarding
  - cross-machine
---

# Cross-Machine Catalog Resume

**Run this when you pull codescout onto a machine that has not been building it,
or after any catalog loss.** It is a normal, expected process — not an incident.
A clone is *supposed* to arrive with three layers missing; this page is how you
put them back and how you tell the difference between "missing" and "broken".

First measured end-to-end 2026-08-28, pulling 437 commits onto a laptop after a
desktop work stream. Every number below came from that pass.

## Why a clone is never enough

codescout splits its state across two stores with **opposite** durability:

| Layer | Lives in | Travels with `git pull`? |
|---|---|---|
| Markdown bodies, frontmatter, `entry_prefix`, `entry_high_water_*` | the repo | **yes** |
| Semantic index (code chunks + memory vectors) | Qdrant / local store | **no** |
| `cites` edges (the link graph) | `catalog.db` | **no** |
| Artifact **augmentations** — `prompt`, `params`, `params_schema`, `render_template`, `entry_collection` | `catalog.db` | **no** |
| Event log, observations | `catalog.db` | **no** |

`~/.local/share/librarian/catalog.db` is machine-local and gitignored, and it is
**machine-global** — one DB spanning every repo on the host, not one per project.
Both facts matter below.

**Each missing layer is silent, and silent in a different way.** That is the whole
reason this page exists:

- `reindex` **preserves** augmentation keyed by id rather than regenerating it — so
  it reports success and repairs nothing.
- `artifact(get)` returns `augmentation: null` with no comment.
- A missing `cites` edge is indistinguishable from an artifact that cites nothing.
- Memory points missing from the store make `recall` return *fewer* results, never
  an error.

Nothing fails. You just quietly get less.

## The sequence

Run in this order. Later steps read state the earlier ones write.

### 1. Pull, and confirm it was a fast-forward

```
git fetch --all --prune && git status
git pull --ff-only
```

`--ff-only` is the point: if the branch diverged you want to know before any
repair runs against a tree you did not expect.

### 2. Reindex the catalog

```
librarian(action="reindex")
```

Adds rows for files the catalog has never seen. **Do this before any `artifact(find)`
query** — a `find` against a stale catalog returns a confidently wrong empty set. The
response's `unindexed_hint` on a prior call is the tell you skipped it.

*2026-08-28: 99 added, 67 updated, 23 removed, 165 embedded.*

### 3. Repair memory vectors

```
index(action="verify")     # read-only; reports memories on disk with no point
./target/release/codescout migrate-memories --in-place
```

`verify` names the gap; `migrate-memories --in-place` reads the memories from disk
**server-side** and re-embeds them. Check `skipped` in the JSON result — a non-zero
`skipped` means those rows kept old vectors and are still on the previous embedding
convention.

*2026-08-28: 21 of 23 memories were on disk with no point — invisible to `recall`,
with no error anywhere. After: 23 upserted, 0 skipped, 7 anchors attached.*

### 4. Rebuild the semantic index

```
index(action="build")      # returns immediately; runs in background
index(action="status")     # poll — file_count and chunk_count climb
```

`git_sync.behind_commits` in the `verify`/`status` output is the honest measure of
how stale it was.

*2026-08-28: 437 commits behind, 116 files missing, 23 orphans.*

### 5. Rebuild the link graph

```
librarian(action="link_scan")              # report only
librarian(action="link_scan", write=true)  # apply
librarian(action="link_scan")              # MUST now show edges_missing[0]
```

**The third call is not optional.** `link_scan` is idempotent, so a second scan
reaching a fixpoint (`edges_missing[0]`, `edges_stale[0]`) is what proves the write
landed. A scan that still finds work means it did not.

*2026-08-28: 697 of 1117 edges missing → 697 added, 9 pruned, 1540 entry edges
written, fixpoint confirmed.*

### 6. Scan for what is left

```
librarian(action="doctor")
```

Then **partition the violations three ways before touching anything** — they need
opposite responses, and the report does not separate them for you:

1. **Machine-local drift** — `augmentation_declared_but_absent`. This is your work.
2. **Pre-existing content debt** — `terminal_status_with_caveat`,
   `cited_prefix_with_no_definer`, `entry_dated_stale`,
   `entry_cited_from_outside_but_undeclared`. These travel in git and are
   *identical on every machine*. They are not caused by the move and are not
   yours to fix as part of a resume.
3. **Other repos** — `abs_path_outside_managed_roots`, and most `missing_file`.
   The catalog is machine-global, so it holds rows for every repo on the host.

**Never run `fix=prune_missing` as part of a resume.** Those rows may be perfectly
valid for a repo that is simply in a different state on this machine. Pruning is
destructive and the resume is not the moment to decide.

*2026-08-28: 125 violations — 22 machine-local, 65 content debt, ~23 other repos.
Only the 22 were in scope.*

### 7. Restore augmentations — tiered, not uniform

This is the only step with no automated path, because **augmentation is the one
artifact state with no on-disk form.** `expects_augmentation: true` in frontmatter
records *that* one should exist; nothing records *what it was*.

Precedent: `docs/issues/archive/2026-07-02-tool-usage-patterns-augmentation-lost.md`
restored this same class by reconstructing from body prose, and states plainly that
the restore is per-machine and must be re-run elsewhere.

**Do not restore every tracker to full params.** Since 2026-08-18
`get_guide("tracker-conventions")` has withdrawn the params-rendered-index pattern:
no `render_template` writes a body, and a table row defines no citable token. Restore
in two tiers:

- **Tier A — `prompt` + `params` + `entry_collection`.** Only for trackers with a
  *documented* `entry_filter` workflow. Find them by evidence, not memory:
  ```
  grep -rn 'entry_filter' docs/ CLAUDE.md --include='*.md'
  ```
  then intersect with the `augmentation_declared_but_absent` list.
- **Tier B — `prompt` only.** Everything else. Restores the `[LIVE]` standing-instruction
  block in `librarian(action="context")` and clears the doctor check.

Follow `docs/conventions/retrofitting-trackers-for-filtering.md` for the Tier-A
mechanics. Two constraints bite every time:

- `merge=false` overwrites **all seven** caller-controlled fields. Pass `prompt`,
  `params`/`params_path`, `params_schema`, `render_template` and `entry_collection`
  in the *same* call — an omitted field silently resets to `None`.
- Params ≳9 KB **cannot** be passed inline; the MCP result buffer caps the
  round-trip, so it cannot be read back to re-emit either. Write the JSON to a file
  and pass `params_path=` (read server-side). A prose-heavy 11-row table already
  measured 8.8 KB in 2026-07.

**Say so in your report when a Tier-B prompt is newly authored rather than
recovered.** It is reconstruction from the body, not restoration of the original,
and the distinction is not visible in the result.

## Verify the resume

Do not trust the steps; check the outcomes.

```
index(action="verify")                 # git_sync.status, memories.missing_count → 0
librarian(action="link_scan")          # edges_missing[0], edges_stale[0]
librarian(action="doctor")             # augmentation_declared_but_absent → 0
artifact(action="get", id="<a Tier-A tracker>", entry_filter={"status":{"eq":"open"}})
```

The last one is the real test: it is the call CLAUDE.md documents for browsing
entries, and it is the one that fails first when augmentation is missing.

## Traps measured on the 2026-08-28 pass

Both produced a **plausible number**, which is what makes them dangerous — neither
raised an error.

**Counting bare `---` to find duplicate frontmatter reports ~22× too many.**
Session logs use `---` as an entry separator: same string, different position. A
sweep for duplicated frontmatter must count a frontmatter-only key instead:

```
for f in docs/trackers/*.md; do
  n=$(awk 'NR<=60 && /^kind: /{c++} END{print c+0}' "$f")
  [ "$n" -ge 2 ] && echo "$n $f"
done
```

*Naive version: 22 hits, all false. Correct version: 1.*

**Filtering doctor output by an absolute project path matches nothing.** Response
paths are **relativized** — project-internal paths render relative
(`docs/trackers/<name>.md`) while foreign repos stay absolute. So
`grep -v '/home/.../codescout'` excludes *nothing*, which reads exactly like "this
project has no violations." Filter on the leading `/` instead: absolute means
another repo. See `get_guide("progressive-disclosure")` § Path-relative annotation.

## Related

- `get_guide("tracker-conventions")` — augmentation declaration, `entry_prefix`, entry shape
- `get_guide("librarian")` — augmentation lifecycle, `merge` semantics, `params_path`
- `docs/conventions/retrofitting-trackers-for-filtering.md` — Tier-A mechanics
- `docs/issues/archive/2026-07-02-tool-usage-patterns-augmentation-lost.md` — the precedent restore
- `docs/trackers/bug-ledger-resume-2026-08-28.md` — the handoff this process was derived from
