---
status: open
opened: 2026-08-26
severity: high
owner: marius
related:
  - docs/issues/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md
tags: [test-isolation, memory, qdrant, pollution]
kind: bug
unverified: 'which test lane writes the points is NOT established — only that they exist, are fixture content, and are keyed by tempdir project_id'
---

# BUG: 91.5% of the live `memories` collection is test-fixture data

## Summary

The Qdrant `memories` collection on this host holds **2142 points**, of which
**1959 (91.5%) sit under 1909 distinct tempdir `project_id`s** (`.tmpcfUc6n`,
`.tmpBv3QkP`, …) and carry obvious fixture content (`hello`, `test-topic`,
`References \`src/a.rs\``, `We chose Rust for performance.`). Real projects
account for 183 points across 25 project_ids. Something in the test suite is
constructing a Qdrant-backed memory store against the live stack instead of
`InMemorySemanticMemoryStore`, and every run leaks a fresh tempdir project.

## Symptom (Effect)

Measured 2026-08-26 against `http://localhost:6333`:

```
total points          : 2142
distinct project_ids  : 1934
  tmp-named (fixtures): 1909 projects, 1959 points
  real-named          :   25 projects,  183 points
```

Sample fixture payloads:

```
 proj=.tmpcfUc6n  bucket=structured  title=architecture/decisions
     content: We chose Rust for performance.
 proj=.tmpBv3QkP  bucket=structured  title=test-topic
     content: References `src/a.rs`.
 proj=.tmpH8IdGd  bucket=structured  title=test/key
     content: hello
```

Two of the real-named points are themselves debris rather than memories:
`zz-probe-delete-me` (96 B) and a note titled *"Auto mode kept on — hard-deny
hooks neutralize the Bash-first nudge"*, neither of which has a file on disk.

## Reproduction

```
curl -s http://localhost:6333/collections/memories \
  | python3 -c 'import sys,json; print(json.load(sys.stdin)["result"]["points_count"])'
```

Then scroll with `with_payload: ["project_id"]` and count keys matching `^\.tmp`.
The full audit script used here is in this session's scratchpad
(`mem_audit.txt` holds its output); it is ~20 lines of paginated
`/points/scroll`.

## Environment

- Linux, branch `experiments`, HEAD `20d5d43f`
- Qdrant at `localhost:6333`, collection `memories`
- The retrieval stack is the `server-stack` default (Qdrant, not sqlite-vec)

## Root cause

**Not established — this is the part to scout first.** What is established:

- The points are fixture content, so a test wrote them.
- `project_id` is a tempdir basename, so the writing test built a project whose
  root was a `tempfile` directory — the standard fixture shape in this repo.
- The unit tests in `src/tools/memory/tests.rs` are NOT the culprit for the ones
  inspected: they use `InMemorySemanticMemoryStore`
  (`crate::memory::semantic_store::test_support`), e.g.
  `cross_embed_memory_stores_under_pinned_project_not_session_default`
  (`src/tools/memory/tests.rs:334`).

So the suspect is a lane that resolves a *real* store from ambient config — the
same shape as the CI failure recorded in `W-56`, where one test resolved an
embedder from ambient config and went red on every OS for a week. Check the
feature-gated e2e lanes (`F-62` names all five as having silently stopped
compiling) and any fixture that builds a store without an `EnvGuard` override.

`docs/conventions/test-env-isolation.md` is the convention this violates.

## Evidence

### Real-named projects, by point count

```
    29  MRV-poc            17  eduplanner-ui       7  extended-crawler
    21  backend-kotlin      10  advisory-proposal…  7  prompt-engineering
    18  codescout            9  code-explorer       7  ie-pal-engine
                             8  workspace-mcp       7  manger-agent
```

25 real project_ids, 183 points. Every other point in the collection is fixture
data.

## Hypotheses tried

1. **Hypothesis:** these are stale points from human sessions activating
   temporary worktrees.
   **Test:** read the payload content of six sampled tmp-named points.
   **Verdict:** rejected. Content is `hello` / `test-topic` /
   `References \`src/a.rs\`` / `We chose Rust for performance.` — fixture
   strings, not prose anyone wrote as a memory. 1909 distinct tempdirs also
   matches repeated suite runs, not human activation.

## Fix

*Not yet implemented.* Two parts, and the order matters:

1. **Stop the writes.** Find the lane and force it onto
   `InMemorySemanticMemoryStore` or an `EnvGuard`-isolated collection. Until
   this lands, cleaning is pointless — the next `cargo test` refills it.
2. **Then clean.** A filtered delete on `project_id` matching `^\.tmp` is safe
   *given* part 1; without it, a developer who cleans and then runs the suite is
   back where they started. Also drop the two debris points named above.

**Do not add a "prune tmp projects" maintenance command as the fix.** That
converts a test-isolation bug into a recurring chore, and the chore is what
makes the bug invisible.

## Tests added

None yet. The regression test is an assertion that the memory-write path under
test never resolves a store from ambient config — which is the same guard
`W-56`'s CI failure argues for, so the two should share it.

## Workarounds

Nothing is broken for the user today: `recall` filters by `project_id`, so
fixture points under tempdir ids cannot surface in a real project's results.
The cost is collection bloat (11.7× the real data) and the risk that any future
path which queries the collection *unfiltered* inherits 1959 pieces of garbage.
That makes this a latent correctness bug rather than an active one — hence
`severity: high` with no user-visible symptom yet.

## Resume

Scout part 1 before anything else: `grep` for constructions of a Qdrant-backed
`SemanticMemoryStore` outside `test_support`, and check the feature-gated e2e
lanes named in `bug-fix-session-log:F-62`. Do **not** clean the collection
first — a cleaned collection that refills on the next suite run destroys the
evidence that identifies the lane.

## References

- `docs/conventions/test-env-isolation.md` — the convention violated
- `bug-fix-session-log:W-56` — a test resolving an embedder from ambient config,
  red on every OS for a week; likely the same class
- `bug-fix-session-log:F-62` — the five feature-gated e2e lanes that stopped
  compiling invisibly
- `src/tools/memory/tests.rs:334` — the correct pattern
  (`InMemorySemanticMemoryStore`)
