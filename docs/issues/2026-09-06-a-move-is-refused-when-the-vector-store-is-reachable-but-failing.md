---
id: ef21d299867be70f
kind: bug
status: open
title: 'BUG: doc(move) is refused outright when the vector store constructs and then fails, though its own fallback comment names that exact case'
tags:
- cluster/guard-narrower-than-its-name
opened: 2026-09-06
owner: marius
related: []
severity: high
---

# BUG: `doc(move)` is refused when the vector store is reachable-but-failing

## Summary

`mv` re-files an artifact's chunk vectors onto its new id and propagates any
error with `?`. A vector backend that **constructs and then fails at call time**
— an unreachable Qdrant under `--features server-stack` — therefore refuses the
whole move, catalog write included. The adjacent `None` arm exists precisely to
tolerate this and its own comment names the case: *"No backend configured
(unreachable Qdrant, or a lean build)"*. It catches construction-time absence
only.

## Symptom (Effect)

CI, `Test (server-stack — the build cargo rb ships)`, red since 2026-09-02:

```
move call must succeed for its guide bytes to count — got: [Annotated { raw:
Text(RawTextContent { text: "list_collections(artifact)", meta: None }),
annotations: None }]

failures:
    server::guide_hint_tests::a_p50_session_stays_under_the_committed_emission_byte_ceiling
test result: FAILED. 5195 passed; 1 failed
```

The failing assertion belongs to a **byte-ceiling** test. It is not about
`move` at all — `move` is the sixth shape in its p50 fixture, and the fixture
asserts each call succeeds because guide injection only fires on the success
path. So the reported failure names a test whose subject is unrelated to the
defect, which is most of why this sat for four days.

## Reproduction

Needs neither `server-stack` nor a Qdrant daemon — inject the failure:

```
cargo test --workspace --lib a_failing_vector_store_does_not_refuse_the_move
```

Pre-fix:

```
a move must not be refused because the vector store is down — the catalog half
is the half that matters, and `reindex` heals the vectors: list_collections(artifact)
```

To see the CI form instead: `cargo test --features server-stack` with no Qdrant
reachable.

## Environment

Linux, `experiments`. **Only reachable with `server-stack` compiled in**, which
is why no local gate saw it — see § *Why the gate could not see this*.

## Root cause

`src/librarian/tools/mv.rs`, pre-fix:

```rust
let vectors_refiled = {
    if new_id != a.id {
        match ctx.artifact_store.as_ref() {
            Some(store) => Some(store.refile(&a.id, &new_id).await?),
            // No backend configured (unreachable Qdrant, or a lean build).
            None => None,
        }
    } else { None }
};
```

`QdrantArtifactStore::refile` fans out over `artifact_collections(prefix)`,
which calls `list_collections()` under
`.context("list_collections(artifact)")` (`src/retrieval/artifact.rs:230-235`).
With no reachable Qdrant that errors, the `?` propagates, and the tool returns
an error after the catalog rows have already been written.

**The requirement was already written down, in the trait this call implements:**

```rust
// src/librarian/artifact_store.rs — `refile`
/// `mv` calls this on every id-changing move, including for artifacts that were
/// never embedded (a lean build, an unreachable Qdrant, a file added since the
/// last reindex). A `refile` that failed there would turn a working archive into
/// a refused one.
```

That is the defect, stated as the thing to avoid, in the file that defines the
method. Two paths reach one operational fact — *the vector backend is
unavailable* — and only the construction-time path had a fallback.

*Measured 2026-09-06:* born-red run above, and CI runs `34029223220`,
`33984541537`, `33948850656` all failing this one test. *Not measured:* whether
any real session ever hit it — locally Qdrant is up, so `doc(move)` reports
`vectors_refiled: 28` and succeeds. The exposure is real but unobserved outside CI.

## Why the gate could not see this

`Cargo.toml`: `default = ["remote-embed", "http", "librarian"]` — **`server-stack`
is not in it.** `cargo test --workspace` never compiles `QdrantArtifactStore`, so
the whole branch is absent from the documented gate. Meanwhile
`.cargo/config.toml` defines `rb = "build --release --features
server-stack,local-embed"`, and the CI job is named *"the build `cargo rb`
ships"*.

So the four-command gate in `CLAUDE.md` — which mentions `server-stack` **zero
times** — certifies a feature set the shipped binary does not use. Filed
separately; the two are siblings, not duplicates. This bug is the *defect*, that
one is the *blindness*.

## Hypotheses tried

1. **Hypothesis:** my commits broke it (`1aab76c5` archived two bug files, which
   calls `doc(move)`).
   **Test:** compare failing job sets and pass counts across three runs.
   **Verdict:** **rejected.** Identical failure set at `49dbf80f` and
   `4d283f54`, both predating the work; `5173 → 5195 passed` is exactly the
   +22 tests added. Same test, same assertion string, three runs.

2. **Hypothesis:** the byte-ceiling test is itself flaky or environment-bound.
   **Verdict:** **rejected.** It fails deterministically, and its `move` call
   fails for a reason that has nothing to do with byte ceilings. The test is
   working correctly; it is a downstream victim.

## Fix

`src/librarian/tools/mv.rs` — catch instead of propagate, and report the
degradation:

```rust
Some(store) => match store.refile(&a.id, &new_id).await {
    Ok(n)  => (Some(n), None),
    Err(e) => (None, Some(format!("{e:#}"))),
},
None => (None, None),
```

plus a `vectors_refile_error` field beside `vectors_refiled`. **The pair is the
point.** `refiled: null` alone was already ambiguous after this change:
`error: null` means no backend was configured to ask, `error: <text>` means one
was asked and failed — the vectors are still filed under the dead id and a
`reindex` is owed. Omitting the field on success would leave a reader inferring
that difference from an absence, which is the shape the `null`-vs-`0`
distinction in this file already exists to avoid.

SHA: *(pending — this commit)*
patch-id: *(pending — this commit)*

## Tests added

`a_failing_vector_store_does_not_refuse_the_move` (`src/librarian/tools/mv.rs`).

Injects an `UnreachableStore` whose every method bails with the verbatim CI
string, so it needs **neither the `server-stack` feature nor a live daemon** and
runs in the lean lane — which is the point, given the defect's whole history is
that it was only reachable in a lane nobody runs locally.

Asserts four things, because `moved: true` alone would pass a fix that dropped
the vectors: the move succeeds, the id re-keys, `vectors_refiled` is **null**
(not `0`, which would claim the artifact had no vectors — a different and
unverified fact), and `vectors_refile_error` names the failure. The last is the
one that keeps a silent strand from reading as a clean move.

The sibling `move_refiles_chunk_vectors_onto_the_new_id` still passes, which is
what establishes the fix did not simply stop re-filing.

## Workarounds

Bring the vector backend up, or run a build without `server-stack` (the
sqlite-vec lite path), before archiving anything.

## Resume

N/A — fixed here. If reopening, start at the `vectors_refiled` binding in
`src/librarian/tools/mv.rs` and check whether any *other* `?` in that function
propagates a vector-store error; only the `refile` call was audited.

## References

- `docs/issues/archive/2026-09-04-artifact-vector-delete-has-no-production-caller-so-every-archive-strands-its-vectors.md`
  — the sibling that gave `refile` a caller. This is its failure mode.
- `src/librarian/artifact_store.rs` § `refile` — the doc comment that forbade
  this in advance.

### Cluster adjudication

`cluster/guard-narrower-than-its-name` (`IC-14`) on the remedy test: the fix
**widens an existing fallback** to cover a second entrance, rather than adding a
disambiguator or wiring something unwired. The name is literal here — the arm's
own comment says *"unreachable Qdrant"* and the arm cannot catch an unreachable
Qdrant, only an unconstructed one.

Two rivals considered. `cluster/doc-contradicted-by-code` (`IC-11`) fits the
symptom — `refile`'s doc forbids exactly what the caller does — but that is how
the defect was *recognised*, not what it is; the doc is right and the code is
wrong, and fixing the code discharges it. `cluster/repro-env-diverges-from-gate-env`
describes why nobody saw it for four days, which is the sibling bug's subject,
not this one's.

