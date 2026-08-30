---
status: open
opened: 2026-08-30
closed:
severity: medium
owner: marius
related: []
tags: [retrieval, error-handling, classifier]
kind: bug
---

# Root's sparse status errors match no classifier arm, and the comment says they do

`classify_search_error` (`src/tools/semantic/semantic_search.rs:59`) routes embedder
failures away from the Qdrant-oriented fallback. Its sparse leg does not reach it.

**The producer** (`src/retrieval/embedder.rs`, inside `embed_one_batch`) emits:

```
embed_batch sparse status 500 (inputs=3, nonempty=3): <body>
```

and its two context strings are `embed_batch sparse send` / `embed_batch sparse json`.

**The consumer** matches `err_str.contains("embed sparse")`.

`"embed_batch sparse status …"` does **not** contain `"embed sparse"` — `embed` is
followed by `_`, not a space. Verified rather than reasoned:

```
$ echo 'embed_batch sparse status 500 (inputs=3, nonempty=3): boom' | grep -c 'embed sparse'
0
```

So every sparse HTTP failure falls through to the generic bucket — *"Stack reachable
but query failed. Check `./scripts/retrieval-stack.sh ps` and qdrant logs (`docker
logs codescout-qdrant`)"* — which sends the operator to Qdrant for an embedder fault.
That is the precise misrouting class the function's doc comment exists to prevent.

**The comment asserts the opposite**, which is why this survived. In the arm added for
`dense openai status` it reads:

> *(The sparse path was fine: "embed sparse status" contains "embed sparse", which the
> embedder arm already matches.)*

The wording it quotes — `embed sparse status` — is not the wording the code emits. A
reader checking the claim against the comment finds agreement; only checking it
against the producer finds the gap.

Worse, the sparse message interpolates the server's **response body**, so a body
containing `not found` or `Collection` reaches the collection arm (which precedes the
embedder arm) and is reported as a missing Qdrant collection. Same body-hijack hazard
the `dense openai status` arm was hoisted above the collection bucket to avoid.

## Fix sketch

Match the producer, not the remembered wording. Either widen the arm to
`contains("sparse status")`, or — better, and consistent with `ET-5`'s remedy for the
connect marker — give the sparse producer a published marker constant that the
classifier imports, so the two cannot drift again. Then correct the parenthetical.

A test in the shape of `the_crates_own_connect_error_routes_where_roots_does` would
have caught it: feed a real sparse error string through `classify_search_error` and
assert the bucket, rather than asserting the arm's literal.

## Provenance

Noticed 2026-08-30 while planning T6
(`resume-embedding-transport-stages-1-3:ET-10`), when auditing which error-string
contracts the dense-leg swap would break. Not caused by that work and not fixed by it
— the dense swap touches the dense arm only.
