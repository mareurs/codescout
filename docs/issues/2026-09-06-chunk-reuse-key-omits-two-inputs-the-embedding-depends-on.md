---
id: b32c8f1ff14f66bb
kind: bug
status: mitigated
title: 'BUG: the chunk reuse key hashes `content`, but the text that gets embedded is entry token + entry title + content'
tags:
- cluster/gate-keyed-on-unobservable-event
- librarian
- embeddings
- retrieval-grain
closed: null
opened: 2026-09-06
owner: marius
severity: med
---

# BUG: the chunk reuse key hashes `content`, but the text that gets embedded is entry token + entry title + content

## Summary

`replace_chunks` (`src/librarian/catalog/chunk.rs`) decides whether a chunk's
`artifact_vec_v2` embedding may be kept by comparing `content_hash`, and its own
doc comment states the premise: *"the vector depends on the chunk's bytes, so
`content_hash` is the whole of what decides whether it can be kept."*

That premise is false. The text actually sent to the embedder is assembled one
layer up, in `embed_queue_items` (`src/librarian/indexer.rs`), and for a
**mid-entry** chunk it is not the chunk's bytes:

```rust
let text = match &r.entry_token {
    Some(tok) if !r.content.trim_start().starts_with('#') => {
        match titles.get(tok) {
            Some(title) => format!("{tok} — {title}\n\n{}", r.content),
            None => format!("{tok}\n\n{}", r.content),
        }
    }
    _ => r.content,
};
```

So the embedded text has **three** inputs — `entry_token`, that entry's **title**
as it appears in the body right now, and `content` — and the reuse key covers
exactly one of them. `content_hash` is `sha256(content)`; neither the token nor
the title is hashed.

## Symptom (Effect)

Silent, and it degrades retrieval rather than breaking it.

Rename an entry's heading — `## W-81 — old title` to `## W-81 — new title`:

- the chunk **containing** the heading has changed bytes, so it is correctly
  re-embedded;
- every **later** chunk of that same entry is byte-identical, so its vector is
  reused — and that vector was embedded as `W-81 — old title\n\n…`.

The entry is then half-findable under its new title and half-findable under a
title that no longer exists in the corpus. Nothing reports it: no error, no count
moves, and `doctor` has no check for it. The only observable is a semantic query
that should rank the entry highly and does not.

Same shape for `entry_token` itself, which is a position field: a chunk can
acquire a different enclosing token with byte-identical content.

## Root cause

The reuse decision is a **gate keyed on an event it cannot observe** — "the text
this chunk will be embedded as has changed" — so it substitutes the nearest
available proxy, `content_hash`. The proxy was accurate when the embedded text
*was* the chunk's bytes. It stopped being accurate when `embed_queue_items`
started prepending the entry's identity, and nothing connected the two: the
prepend lives in the indexer, the reuse key lives in the catalog layer, and each
is locally correct.

The layering is what hides it. `replace_chunks` cannot see what its caller will
do with the rows it returns, so it cannot key on the real thing; and
`embed_queue_items` never learns which rows kept their id.

## Evidence

Read at the bytes 2026-09-06 while fixing the ordinal-reuse defect
(`docs/issues/archive/2026-09-02-chunk-reuse-key-misses-chunks-that-only-moved-ordinal.md`,
fixed at `71077fe9`). Confirmed **by construction, not by a run** — and that is
recorded as a limitation rather than as a finding, because the file this one
supersedes made exactly that claim and it decayed without anyone noticing.

The three facts are each read from the source rather than inferred:

1. `content_hash` is a SHA-256 over `content` alone — two sites in
   `src/librarian/catalog/chunk.rs`, `build_chunks` and `build_single_chunk`.
2. `embed_queue_items` prepends `{tok} — {title}` for any chunk carrying an
   `entry_token` whose content does not itself start with `#`.
3. `titles` is rebuilt from the **current** body on every call
   (`entry_titles_by_token(body)`), so a renamed heading changes it immediately.

**2026-09-07 — the run happened, and it went the other way.** All three facts are
true; the symptom drawn from them is not. They describe the **key**, and the
symptom additionally needed a property of the **caller** — that a reused row can
skip re-embedding — which does not hold at either call site. See `## Fix`.

Fact 3 is the refutation, written down here a day before it was read as one: a
title table rebuilt from the current body cannot hand a stale title to anything.
It was recorded as background for how the stale value *arises* without asking what
consumes it. And the caution this file opened with — that the superseded file made
a by-construction claim which decayed unnoticed — named the right risk and missed
that the by-construction claim was **this file's own**, one paragraph below.
## Hypotheses tried

None yet — filed on notice, not investigated.

## Fix

**Reproduced 2026-09-07, and the reproduction falsified this file's own symptom.**
Applied option three — accept the incomplete key and say so at the refusal site —
plus the regression guard that keeps the acceptance honest.

What the run established, against `index_repo_sync`'s real shape rather than by
construction:

- `embed_queue_items` rebuilds `titles` from the **current** body on every call, so
  the text it queues for a REUSED row already carries the new title. Pinned by
  `a_reused_chunk_embeds_with_the_current_entry_title_not_the_stored_one`
  (`src/librarian/indexer.rs`), which asserts the reuse genuinely happened —
  `chunk_id` preserved for the mid-entry chunk, changed for the heading-bearing one
  — so it cannot pass by the row having been rebuilt instead. Verified to
  discriminate: hardcoding the old title in the production path reds it.
- Both production callers embed **every** item they are handed. `index_repo_sync`
  gates per ARTIFACT on the file's `sha256`, which any heading rename changes, and
  drains the queue unfiltered. `backfill_chunk_vectors` selects only artifacts with
  **no chunk rows at all** (`NOT EXISTS`), so it never reuses anything.

So no stale title can reach a stored vector today. The gap in the key is real and
**latent**, not live — which makes the fix stopping the sentence that would let
someone close it wrongly.

Applied in `src/librarian/catalog/chunk.rs`:

- The false half of the doc comment — *"the vector depends on the chunk's bytes, so
  `content_hash` is the whole of what decides whether it can be kept"* — now says it
  decides whether the **row** can be kept, and points at the limitation.
- A new *The reuse key is incomplete* section states which one of the three inputs
  the key covers, that the harmlessness is a property of the CALLER rather than of
  the key, and names the single change that would make it live: **filtering the
  embed queue against chunks that already hold a vector.** That is the obvious cost
  optimisation, which is precisely why it needed writing down.
## Tests added

None. A regression test would rename an entry's heading, leaving a later chunk of
that entry byte-identical, and assert that chunk's vector was **not** reused —
which is only expressible once the fix direction is chosen, since option three
makes reuse correct.

The fixture has one load-bearing detail: the entry must split into **more than
one** chunk, and the assertion must be about a chunk that does **not** contain the
heading. A single-chunk entry always carries its own heading, so its content hash
moves with the title and the defect is unreachable — a fixture that forgets this
passes against every version of the code.

## Workarounds

`librarian(action="reindex", reembed=true)` re-embeds regardless of content hash,
so it clears the stale vectors. It is a whole-corpus operation, so it is a repair,
not a mitigation.

## Resume

Price the three options above with the retrieval cost in front of you, and check
whether option three is already the honest answer. Whichever is chosen, the doc
comment in `replace_chunks` asserting that the vector depends on the chunk's bytes
must be corrected in the same commit — it is currently false, and it is what
stopped this being noticed for as long as it was.
