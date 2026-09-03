---
kind: bug
status: fixed
tags:
- cluster/accepted-parameter-silently-dropped
closed: 2026-09-04
opened: 2026-09-03
owner: marius
related: []
severity: high
---

# BUG: per-project vector collections follow the server's cwd, so a `workspace=`-scoped reindex files another project's vectors under the active project and reports success

## Summary
`6f032dbd` gave each project its own Qdrant collection, named from `current_project`. That
value is resolved **once, when the tool context is built**, from the MCP server's cwd — so
the per-call `workspace=` parameter cannot reach it. The catalog honours `workspace=`; the
vector store does not. A `reindex(workspace=<other project>, reembed=true)` therefore embeds
that project's artifacts into the **active** project's collection, reports
`vectorless: 0, embed_error_count: 0`, and leaves the other project's semantic search
returning nothing relevant — with no error at any point.

## Symptom (Effect)

Three `reindex(workspace="/home/marius/work/claude/prompt-engineering", reembed=true)` calls,
each reporting success server-side:

```
"embedded": 1, "embed_error_count": 0, "vectorless": 0,
"vectorless_note": "every indexed artifact has a vector"
```

And yet no collection for that project exists:

```
$ curl -s localhost:6333/collections
  artifacts                                     5394 points   (the superseded artifact-grain one)
  artifact_chunks_codescout_dc6a871595179329   29046 points
  code_chunks / memories / bench_coderank_code_chunks

$ python3 -c "import hashlib; print(hashlib.sha256(b'/home/marius/work/claude/prompt-engineering').hexdigest()[:16])"
f6504bfff91be097          # artifact_chunks_prompt_engineering_f6504bfff91be097 — ABSENT
```

Reading back, a query quoting a document's own title near-verbatim does not return it:

```
doc(action="find", semantic="the arm's prompt asserts a bug that its own fixture does not
    contain, so verifying the premise scores as a failure",
    workspace="…/prompt-engineering", include_archived=true)
→ 17 hits, none of them that document
  hints.unresolved = 846
  "846 vector(s) the store returned resolved to no chunk row and were discarded before ranking"
```

## Reproduction

```bash
# with the MCP server's cwd in project A, and B a different registered project
librarian(action="reindex", workspace="<B>", reembed=true)     # reports success
curl -s localhost:6333/collections                              # no collection for B
doc(action="find", semantic="<text verbatim from a B document>", workspace="<B>")
                                                                # does not return it
```

Observed 2026-09-03 at `6f032dbd`, server cwd `/home/marius/work/claude/codescout`,
target `/home/marius/work/claude/prompt-engineering`, Qdrant backend.

## Environment

- Backend: Qdrant (the default). The sqlite-vec escape hatch is not affected the same way —
  it keys on `chunk_id` in the catalog the call is already scoped to.
- Only reachable when the server's project and the call's `workspace=` differ, which is
  exactly the cross-project pattern `CLAUDE.md` documents and the `workspace=` param exists for.

## Root cause

**CORRECTED 2026-09-03, hours after filing and before any fix was written.** The first
version of this section said the store captures a project path at
`build_tool_context_with` time and holds it for the process lifetime, so a per-call
`workspace=` "cannot reach it". That is **wrong**, and it would have sent the next reader to
the wrong file. `QdrantArtifactStore` takes a `project_id` on **every** call —
`upsert(project_id, …)` and `knn(Some(pid), …)` both route through `collection_for(pid)`.
The store is not the defect.

**The caller never derives one.** `src/librarian/tools/reindex.rs:369-375`:

```rust
let project_id = crate::librarian::tools::containing_root(&root_paths, abs_root)
    .map(|p| p.to_string_lossy().into_owned())
    .unwrap_or_default();
```

`root_paths` is `ctx.workspace.roots` — the registry's `[[roots]]`. Both projects on this
host are `[[umbrella]]` **members**, which is a different list, so `containing_root` returns
`None` for **both** and `project_id` is `""`:

```
/home/marius/work/claude/codescout           containing root: NONE -> project_id = ""
/home/marius/work/claude/prompt-engineering  containing root: NONE -> project_id = ""
```

`collection_for("")` then falls back to `default_collection`, which IS derived from the
active project. So every reindex lands in the active project's collection regardless of its
target — codescout is correct **by accident**, because it happens to be the active one, and
every other target is silently misfiled.

**The fallback was correct when it was written and is not any more.** Its doc comment says
an empty `project_id` means "unscoped KNN (the catalog scoped filter still narrows
results)" — true when one shared collection was filtered by a `project_id` payload. Once the
name *selects the collection*, the same empty value stops meaning "unscoped" and starts
meaning "the active project's", which is a different claim that happens to be true only
when target == active. The comment now describes semantics the code no longer has.

Measured 2026-09-03: collection census; `sha256` of both project paths; the roots-vs-members
resolution above computed against the live `~/.config/librarian/workspace.toml`;
`build_tool_context_with` (`src/librarian/mod.rs:78-233`), `QdrantArtifactStore`
(`src/librarian/artifact_store.rs:189-344`) and the derivation site
(`src/librarian/tools/reindex.rs:369-375`) all read.

**Not established:** that prompt-engineering's points are physically inside
`artifact_chunks_codescout_…`. Its payload carries only `chunk_id`, `artifact_id` and an
empty `project_id`, so a scroll cannot attribute a point to a project. The reachable
consequence — success reported, document unretrievable — is observed.
## Evidence

### `reembed` converges rather than repairing, which is what sent me looking

`unresolved` across three consecutive `reembed=true` runs: **429 → 846 → 844**.

Two hypotheses fit the first pair and predict differently — additive (+417 → 1263) and
multiplicative (×1.97 → 1692). **Both were falsified by the third run.** The population is
stable, consistent with deterministic (UUID-v5-shaped) point ids making re-embedding
idempotent: the same chunks are rewritten to the same point ids, in the same wrong place.

Recorded because the n=2 reading — *"the prescribed remedy amplifies the condition"* — was
one message from being published and is wrong.

### The remedy the hint names cannot fix this

`unresolved_hint` prescribes `librarian(action="reindex", reembed=true)`. That is the call
whose collection is misderived, so following it re-files the vectors in the same wrong
collection and reports success again.


### Corroborated from the other side, 2026-09-04

Measured independently by another session, on the **codescout** catalog rather than the
prompt-engineering one — so a different instrument, a different corpus, and a different
direction of approach:

```
artifact_chunks_codescout_dc6a871595179329   points_count      = 29,077
artifact_chunk rows under .../codescout                        = 28,659
delta                                                          =   +418
```

418 points in codescout's collection resolve to **no chunk row in codescout's catalog** —
the same population my `unresolved: 846` counts from the far end, and exactly what the
misfiling predicts: foreign targets writing in. Two catalogs, two instruments, agreeing in
the direction the mechanism requires.

**Stated as its author scoped it:** 418 is a *lower bound* on misfiled points, not an exact
count — a codescout artifact deleted since its vectors were written would also leave an
orphan, and those were not separated. It carries more weight than a raw delta would, though,
because 27,762 vectors were re-embedded earlier the same night: any surviving orphan has
outlived a full rewrite.

**One case the planned GC will not cover, and it comes from this number.** Those 418 orphans
have a `project_id` payload naming a path that *still exists*, so a GC keyed on "the recorded
path is gone from disk" — the design chosen to protect de-registered projects — cannot reap
them. They are identifiable only by the chunk-row join, which is catalog-local and therefore
only checkable from the catalog that owns the collection. Path-existence reaps abandoned
*projects*; it does not reap misfiled *points* inside a live one. Both are wanted; only the
first is planned.
## Verified fixed, 2026-09-04

Against the **deployed** binary, not a scratch build: `/home/marius/.cargo/bin/codescout` →
`target/release/codescout`, built 00:53:39, after `99558134` became an ancestor of HEAD.
Provenance established positively rather than by mtime — `strings` finds the fix's own error
text (`artifact vector write/read with an EMPTY project_id`) in the shipped binary, with a
negative control string returning 0 so the probe is known to discriminate.

| # | check | before | after |
|---|---|---|---|
| 1 | `artifact_chunks_prompt_engineering_f6504bfff91be097` exists | **absent** | **2100 points** |
| 2 | PE semantic find returns the document, `unresolved` falls | 846 unresolved | rank **#1** @ 0.176 (next 0.480); **no `unresolved` key at all** |
| 3 | codescout's collection intact and not renamed | 29154 | **29154**, delta **+0** |
| 4 | `scope="umbrella"` fan-out survives | — | 50 hits from PE |

The reindex that produced row 1 reported `vectorless: 0`, `embed_error_count: 0` — **byte-for
byte what it reported while misfiling**. That report is not evidence and was not read as any;
the collection list is. Row 3 is the guard that makes row 1 mean something: PE's 2100 vectors
appearing *without* codescout's count moving is what distinguishes "filed correctly" from
"filed twice".

A fifth check, unplanned and stronger than the four: the payload now **discriminates**.
Sampling 200 points of each collection returns `project_id: ''` on 200/200 of codescout's
legacy rows and `'/home/marius/work/claude/prompt-engineering'` on 200/200 of the rows the
fixed path just wrote. The empty string is the bug's own fingerprint, still in the data.

### Residue — the fix is not retroactive

The open question the *Resume* section could not settle is now answered by joining every
`artifact_id` in codescout's collection against the catalog. Full scan, not a sample:

| owning project | points | distinct artifacts | points/artifact |
|---|---:|---:|---:|
| codescout | 28,970 (99.37%) | 1,460 | 19.8 |
| **prompt-engineering** | **87 (0.30%)** | **87** | **1.0** |
| not in catalog | 97 (0.33%) | 6 | 16.2 |

So **yes** — prompt-engineering's artifacts are physically in codescout's collection, as
suspected. The ratio dates them: 1.0 point per artifact is **artifact grain**, the default
before `63fae4ea` flipped it, so these were misfiled under the old routing *and* the old grain
and nothing has rewritten them since. A codescout reindex will not remove them either — those
87 artifacts still exist in the shared catalog, merely under a different project, so the
`orphans_removed` path does not consider them orphans.

Practical cost is small and one-directional: a codescout project-scoped find filters them out
as out-of-scope, which is exactly the `unresolved: 3` seen in the control run — the mirror of
PE's 846, at 1/280th the size. Recorded, not swept: deleting points from a live collection is
a separate change with its own blast radius, and 0.30% does not justify bundling it into a
fix that is otherwise clean.
## Hypotheses tried

1. **Hypothesis:** this is the already-filed grain mixture,
   `docs/issues/2026-09-03-editing-an-artifact-removes-it-from-qdrant-backed-semantic-search.md`.
   **Test:** read that file; check whether the symptom survives its fix at `6f032dbd`.
   **Verdict:** rejected as the same defect — that one is the pre-fix `artifacts` collection
   holding two id grains, and `6f032dbd` fixes it (embed errors went 45 → 0 on the same call
   in the same session). This is a new defect **introduced by** that fix. Related, not
   duplicate.
2. **Hypothesis:** `reembed=true` never completes, because every call returned
   `Tool 'librarian' timed out after 60s`.
   **Verdict:** rejected as the explanation — the work completes server-side regardless of
   the timeout: `vectorless` went 114 → 0 across a call that reported failure. That mismatch
   is a **separate, already-filed defect**, not part of this one:
   `docs/issues/2026-09-03-librarian-is-an-embedding-loop-omitted-from-the-embedding-loop-exemption.md`
   (`1067c6c157f0d9ee`), whose Summary states it exactly — *"the work continues server-side;
   only the caller's view of it is destroyed"*. Rediscovered here independently; not re-filed.

## Fix

Fixed in `99558134` — *fix(librarian): file artifact vectors under the project being indexed*.
patch-id `f742cf305a160e67292efdd21e737d08edeac064` (recorded at fix time; the SHA dies at the
next `experiments` rebase, the patch-id does not).

Three changes, write side and read side and the guard:

- **`src/librarian/tools/reindex.rs`** — the write side now derives `project_id` from
  `abs_root` itself, the target being indexed, rather than looking the target up in
  `ctx.workspace.roots` and falling through `.unwrap_or_default()` to `""`. The registry
  lookup was the whole defect: both projects here are `[[umbrella]]` **members**, never
  `[[roots]]` entries, so `containing_root` returned `None` for both and codescout was right
  only by accident.
- **`src/librarian/tools/find.rs`** — the read side scopes to the call's own project rather
  than the process's `current_project`, and passes `None` only when the scope is genuinely
  wider than one project, so the cross-project fan-out `6f032dbd` protects still works.
- **`src/librarian/artifact_store.rs`** — `collection_for` returns `Result` and bails on an
  empty `project_id`; the `default_collection` fallback field is **deleted** rather than left
  unused, so there is no second path back to the old behaviour. This is the half the record
  above asked for: a collection named from `sha256("")` is indistinguishable from a working
  one at every observation the caller can make.

Note the fix is **not retroactive** — see *Residue* below.
## Tests added

`embedded_vectors_are_filed_under_the_target_not_the_workspace_registry`
(`src/librarian/tools/reindex.rs`). It asserts `store.project_ids() == vec![proj_path]` — the
collection the write **landed in** — and deliberately registers **no root containing the
target**, which is the condition that produced the bug. A test asserting `embed_error_count
== 0` passes on the broken code, which is why that is not the assertion.

The guard direction matters: this is an *existence* assertion over a one-element vector, so
it is not monotone under widening — a second, wrong collection appearing fails it.
## Workarounds
Run the reindex from a server whose cwd is the project being indexed. On the CLI path
(`reindex_cli`, `src/librarian/mod.rs:472`) the prefix is derived separately and may not
share the defect — untested.

## Resume

Nothing owed on the fix itself. Two follow-ups, both recorded rather than pending on this
file:

1. **Residue cleanup** (see below) — 87 misfiled points survive in codescout's collection.
2. **Eval-arm collection cleanup** — the harness teardown + GC backstop, plan step 4. Its
   design constraint is now `bug-fix-session-log:W-103`: legacy vectors carry `project_id:
   ''`, so a GC that reads "path not on disk" as the orphan test destroys codescout's entire
   index. Empty must mean *legacy, never touch*.
## References
- `src/librarian/mod.rs:78-233` — `build_tool_context_with`
- `src/librarian/artifact_store.rs:191-207` — `QdrantArtifactStore`
- `6f032dbd` — the commit that introduced per-project collections, and the fix for the grain mixture
- `docs/issues/2026-09-03-editing-an-artifact-removes-it-from-qdrant-backed-semantic-search.md` — the defect `6f032dbd` fixes; related, not duplicate
