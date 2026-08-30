---
status: fixed
opened: 2026-08-30
closed: 2026-08-30
severity: medium
owner: marius
related: []
tags: [retrieval, error-handling, classifier]
kind: bug
---

# The sparse leg discards its error body, and its two producers disagree on wording

**Filed with an overstated impact claim, corrected on investigation. Both halves of
the correction are kept, because the mis-filing is the more useful record.**

## What was filed

That `classify_search_error` matches `err_str.contains("embed sparse")` while the
producer emits `embed_batch sparse status …`, which does not contain it — `embed` is
followed by `_`, not a space. Verified then and still true:

```
$ echo 'embed_batch sparse status 500 (inputs=3, nonempty=3): boom' | grep -c 'embed sparse'
0
```

And that the function's own comment asserts the opposite:

> *(The sparse path was fine: "embed sparse status" contains "embed sparse", which the
> embedder arm already matches.)*

## What was wrong with it

The impact claim — *"every sparse HTTP failure falls through to the generic bucket
and sends the operator to Qdrant"* — does not hold.

`classify_search_error` is called from `src/tools/semantic/semantic_search.rs` **and
nowhere else** (28 references there; the only other occurrence in the tree is a
comment). Search embeds a single query through `EmbedderHttp::embed`, whose sparse
contexts *are* `embed sparse …` and *do* match. The mismatched producer is
`embed_one_batch` — the **indexing** path, whose errors surface through
`SyncReport.skipped` and never reach this classifier.

So there were two sparse producers, the comment was true of one and false of the
other, and the one it was false about was not routed through the classifier anyway.
Filing from a `grep` of one producer against one consumer, without asking which
producer the consumer actually sees, is what produced a real mechanism attached to an
impact that does not occur.

## The defect that was actually there

On the path the classifier *does* see, `EmbedderHttp::embed` used:

```rust
.error_for_status()
.context("embed sparse status")?
```

`error_for_status()` keeps the status code and **discards the response body**. For
this backend the body is the only place the actionable cause appears: an operator
hitting a sparse 422 saw `422 Unprocessable Entity` rather than `batch size 40 >
maximum allowed batch size 32`.

That is the *identical* defect fixed for the **dense** leg in
`docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md`,
whose explanatory comment still sits a few lines away in the same function. The fix
landed on one of the two legs. Same shape as
`docs/issues/archive/2026-08-30-crate-status-errors-hijack-the-qdrant-collection-bucket.md`
and `…-remote-embedder-panics-on-a-short-server-response.md`: a hazard handled on one
side of a pair and never on the other.

## Fix

Fixed on `experiments` in `5dfa5051`, patch-id
`189e55a8656e357ea80186eeeb372de277a1b08e`.

- The body is restored and bounded at 400 characters, with `<empty response body>`
  for a blank one — the dense side's treatment, for the dense side's reason.
- Both producers render `SPARSE_MARKER` / `SPARSE_STATUS_MARKER`; the classifier
  matches the constants. Ungated deliberately, since `classify_search_error` compiles
  in a lean build.
- The sparse arm is **hoisted above the Qdrant-collection bucket**. This is not
  optional: restoring the body puts arbitrary remote text into the message, so a
  sparse 404 reading `model not found` would otherwise be reported as a missing
  collection. Fixing the body without hoisting the arm trades one defect for another,
  which is why both are in one commit.

## Regression tests, and which one earns its keep

- `a_sparse_error_status_surfaces_the_servers_body` — the body survives.
- `a_sparse_error_status_with_no_body_says_so` — an empty body is named.
- `a_sparse_status_body_saying_not_found_is_not_reported_as_a_missing_collection` —
  the hijack. Verified to **run** under `--no-default-features` (`1 passed`), not
  filtered out to a green `0 passed`.
- `the_batch_sparse_producers_real_error_reaches_an_embedder_bucket` — drives
  `embed_batch` against a failing sparse mock and feeds the **real** error string to
  the **real** classifier.

The last one replaced a string-built test that was deleted before it was committed.
That test formatted its input from `SPARSE_MARKER` and asserted the classifier routed
it — so a producer that *stopped* rendering `SPARSE_MARKER` would not have moved it.
It would have been a regression guard for this bug that this bug could pass. The
end-to-end version fails when the producer's wording is reverted, printing the
original defect verbatim:

```
err:  embed_batch sparse status 400 Bad Request (inputs=1, nonempty=1): bad input. …
hint: Stack reachable but query failed. Check `./scripts/retrieval-stack.sh ps` and
      qdrant logs (`docker logs codescout-qdrant`).
```

— while both classifier-level tests stay **green**, which is the demonstration that
they were blind to it.

## Provenance

Noticed 2026-08-30 while auditing which error-string contracts the T6 dense-leg swap
would break, and corrected the same day when the fix was attempted and the filed
claim did not reproduce. `resume-embedding-transport-stages-1-3:ET-10`.
