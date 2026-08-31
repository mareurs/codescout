---
status: open
opened: 2026-08-31
closed:
severity: high
owner: marius
related: []
tags: [guides, section-grain-delivery, topic-routing, librarian, silent-non-delivery]
kind: bug
---

# BUG: a guide section can declare a call's shape, pass every test, and still never be delivered — because the TOPIC router picks a different topic from the result's content

## Summary

Section-grain delivery has two independent gates and only one of them is guarded. A
section declares `<!-- serves: tool.action -->` and `match_sections` honours it — but the
*topic* is chosen first, by `Tool::relevant_guide_topic`, from the **content of the
result**. If that picks a different topic, the declaring topic's sections are never
consulted.

For `librarian(doctor)` this is not an edge case: `names_tracker_path` scans `path` inside
the `violations` array, and a doctor scan of any real catalog names tracker and bug files.
So **every `serves: librarian.doctor` declaration in `librarian.md` is unreachable in
practice.**

And the withheld section is not simply missing: the content-chosen topic is delivered in
its stead, **whole**, at 26× the size of the section it displaced. Measured at runtime —
see § *Runtime confirmation* below.

## Symptom (Effect)

`librarian.md` § *doctor repairs — what each `fix=` mode does* declares
`<!-- serves: librarian.doctor -->`. Two consecutive `librarian(action="doctor")` calls
against the live catalog delivered it **zero** times. The section had never been emitted:

```
$ python3 - <<'EOF'   # this session's ledger
...
06:58:42  librarian#librarian(action=...) — Reference
08:24:56  project-activation-bootstrap
EOF
doctor repairs present: False
```

The first call delivered `project-activation-bootstrap`; the second delivered nothing.

## Reproduction

```
git rev-parse HEAD                  # 13e37b8d + working tree at filing
cargo rb && /mcp                    # ensure the running server carries the section
librarian(action="doctor")          # on a catalog whose violations name docs/trackers/
```

Read the response: no `librarian` section arrives. Then confirm the section *is*
declared and *does* match a doctor selector — it does:

```
cargo test --lib doctor_repairs_section_declares_a_shape_a_doctor_selector_matches
```

The two facts are consistent, which is the whole bug.

## Environment

codescout 0.15.0, `experiments`, MCP stdio, release binary built 2026-08-31 11:24:01,
server pids started 11:24:23 (confirmed running the new binary, not a stale one).

## Root cause

Two gates, evaluated in order, and the first one ignores declarations:

1. **Topic choice** — `LibrarianAdapter::relevant_guide_topic` (`src/librarian/adapter.rs`):
   returns `Some("tracker-conventions")` when `names_tracker_path(result)`, else
   `Some("librarian")`. `names_tracker_path` explicitly scans `path` inside a
   `doctor`-style `violations` array — deliberately, per its own doc comment.
2. **Section choice** — `GUIDE_INDEX.match_sections(topic, selector, result)`
   (`src/prompts/guide_index.rs`), which only ever sees the topic gate 1 chose.

A section declaring `librarian.doctor` therefore competes for a slot it can only win when
gate 1 happens to choose `librarian` — i.e. when a doctor scan finds **no** violations
under `docs/trackers/` or `docs/issues/`.

Measured 2026-08-31 on this repo: **128 of 138** violations named one of those paths, so
gate 1 chose `tracker-conventions`. Not inferred — the counts are from the buffered result
of the failing call.

## Evidence

### Runtime confirmation — what arrives INSTEAD, and what it costs

Filed from source reading plus an empty-ledger observation. Confirmed at runtime
2026-08-31 08:59:09Z against the release binary built 11:56 local, which postdates every
commit through `801767d7`.

The probe was clean rather than lucky. Before the call the session ledger held exactly two
keys — `librarian#librarian(action=...) — Reference` and `project-activation-bootstrap` —
so `tracker-conventions` had never been served this session and could not be suppressed as
a duplicate. A single `librarian(action="doctor")` (139 violations, live catalog) then
injected **`get_guide('tracker-conventions')` in full**.

So the defect is not only that the declared section is withheld. **A different, larger
topic is delivered in its place.**

| | bytes | lines |
|---|---|---|
| `librarian.md` § *doctor repairs* — declared for this call | 1,490 | 19 |
| `tracker-conventions.md` — actually injected | 39,106 | 707 |

A **26× overshoot into the wrong topic**, paid on the first `doctor` call of every session.

**The ledger key shape records which mode fired**, so this is checkable after the fact
without re-running anything: a section-grain delivery keys as `topic#heading`, a
whole-topic one as bare `topic`. The post-call ledger reads
`tracker-conventions:2026-08-31T08:59:09Z` — no `#`, hence whole-topic.

**The two defects compound, and the second one is structural.**
`tracker-conventions.md` contains **zero** `serves:` declarations, so `declares()` is false
for it and that topic can only ever deliver whole. Section-grain is what made `librarian`
affordable to serve; the router lands doctor on the one guide with none of it. Counted
across the guide set the same day — `for f in src/prompts/guides/*.md; do grep -c 'serves:'
$f; done` — only `librarian.md` declares any sections (13). **9 of 10 topics are
whole-topic**, so any mis-route pays its destination's full price, and closing this for
`doctor` alone leaves the shape live everywhere else.

*Derivation shipped because the first attempt was wrong.* `awk '/^### doctor
repairs/,/^### [^d]/'` does not stop at `##`-level headings and over-captured by 5×
(7,372 B). The 1,490 figure comes from `awk '/^### doctor repairs/{f=1} f&&/^#{1,3}
/&&!/^### doctor repairs/{exit} f'`, whose first and last captured lines were read back
before the number was published.

### The guard that exists does not cover this

`src/server.rs:3272` asserts every call shape routing to a *declaring* topic has a section
serving it — *"Add a `serves:` declaration, or a SECTION_WAIVERS entry."* That is the
converse property. It cannot fire here, because the shape never routes to the declaring
topic in the first place; nothing asserts that a shape declared in topic T is ever routed
to T.

### The test written to verify the move was an adjacent proposition

`doctor_repairs_section_declares_a_shape_a_doctor_selector_matches` (renamed from
`..._is_reachable_from_a_doctor_call`) passes and always did. It calls `match_sections`
directly, bypassing gate 1 — so it measured the shape, faithfully, while the modes were
undocumented anywhere a caller would see. `reconnaissance-patterns:R-136`, produced by
the same move that produced this bug file.

## Hypotheses tried

1. **Hypothesis:** the running server predates the change.
   **Test:** `stat` the binary (11:24:01) against `ps -o lstart=` for every codescout pid.
   **Verdict:** rejected — servers started 11:24:23, after the build.
2. **Hypothesis:** ledger suppression; the section had already been delivered.
   **Test:** read this session's ledger JSON.
   **Verdict:** rejected — `doctor repairs` absent; only the older *Reference* section and
   `project-activation-bootstrap` present.
3. **Hypothesis:** the declaration failed to parse (blank line under the heading).
   **Test:** read lines 304–306 of the guide; run `match_sections` in a unit test.
   **Verdict:** rejected — comment sits directly under the heading, and the shape matches.
4. **Hypothesis:** the topic router chose a different topic.
   **Test:** read `names_tracker_path`; count violations naming tracker/bug paths.
   **Verdict:** **confirmed** — 128/138.

## Fix

Not applied. The immediate damage is undone: the `fix=` per-mode text was **restored to
the tool schema**, so the modes are documented on the surface a caller actually receives.
The guide section is left in place — it is correct content, reachable via an explicit
`get_guide("librarian")` and on a clean-catalog doctor run.

Candidate fixes, none chosen:

- **Make declaration beat content.** If any section in the corpus declares the call's
  shape, route to that section's topic rather than to the content heuristic. Most correct,
  and it makes `serves:` mean what it appears to mean. Needs a tie-break when two topics
  declare the same shape.
- **Let a tool return several candidate topics** and deliver matching sections from each,
  rather than one topic winning outright.
- **Extend the `server.rs:3272` gate with its converse**: every declared shape must be
  routable to its declaring topic by at least one plausible result, or carry a waiver.
  This is the cheap one — it would have failed on the commit that introduced the section,
  which is the point.

**Correction, 2026-08-31 — the third is NOT cheap, and calling it "the cheap one" above was
wrong.** Attempted, and it cannot catch this by construction:
`every_observed_shape_of_a_declaring_topic_has_a_section` skips any topic where
`GUIDE_INDEX.declares(topic)` is false, and `tracker-conventions.md` carries **0** `serves:`
declarations — so the very topic a doctor result routes to is invisible to that gate. The
gate is scoped to declaring topics on purpose; widening it means confronting what a
whole-topic destination should owe, which is the same design question as the first two
options rather than a cheap alternative to them.

Two things did land from the attempt, and neither closes the hole:

- The gate now pairs **each probe with the topic IT routes to**, instead of
  `find_map`-ing the first `Some` (the empty probe, which yields `Some("librarian")`
  unconditionally, so coverage was only ever evaluated against `librarian`). More faithful
  to the runtime relation; guards the case where a currently-whole-topic guide later adopts
  `serves:`. It does not catch today's bug and is not claimed to.
- A probe shaped like a real doctor response (`violations[].path`) was added, because
  without one the content branch of `names_tracker_path` is unreachable from the test.

**What actually guards this now** is a targeted invariant, not a general gate:
`doctor_results_route_away_from_librarian_so_fix_modes_stay_in_the_schema`
(`src/librarian/adapter.rs`) asserts both halves together — that a doctor-shaped result
trips `names_tracker_path`, *and* that all six modes are explained in the `fix` description.
Mutation-verified: restoring the `get_guide` pointer in place of the modes fails it on
`prune_missing`. It asserts the **reachable surface** rather than the guide, because the
guide side is exactly what passed while the modes were undocumented.

The class remains *silent* in general — a declaration that never routes costs nothing at
authoring time and produces no error at run time. One shape is now pinned; the mechanism
is not.

## Tests added

`doctor_repairs_section_declares_a_shape_a_doctor_selector_matches`
(`src/prompts/guide_index.rs`) — kept and **renamed to what it proves**, with its own doc
comment naming the gap. No test yet covers the end-to-end property; writing one means
asserting that a realistic doctor result reaches a topic serving `librarian.doctor`, which
would fail today. That is the regression test for whichever fix is chosen.

## Workarounds

Do not move first-call-relevant text out of a tool schema into a section served by a
`librarian.*` shape whose results name tracker or bug paths. Verify a move by making the
real call and reading the response — not by asserting on `match_sections`, which bypasses
the gate that fails.

## Resume

Decide between "declaration beats content" and "several candidate topics" in
`Tool::relevant_guide_topic` (`src/tools/core/types.rs` calls it; `src/librarian/adapter.rs`
implements the librarian one).

**The runtime confirmation constrains that choice.** The destination here declares no
sections at all, so any fix phrased as "prefer the topic whose sections match the call"
must still say what happens when the content-chosen topic is whole-topic — today's answer
is to silently pay 39,106 bytes. With 9 of 10 topics whole-topic, that is the common case,
not the exotic one.

A commit-time gate is still wanted, but it is **not** the converse of `src/server.rs:3272`:
that gate is scoped to declaring topics and cannot see this bug (§ *Fix*). Whatever is built
has to start from the declaration side — for each declared shape, assert that some plausible
result routes to its declaring topic.

## References

- `docs/issues/2026-08-31-served-guide-sections-arrive-after-the-call-they-inform.md` —
  the sibling limitation (delivery is post-execution). Together they bound what
  section-grain delivery can be relied on for.
- `src/librarian/adapter.rs` — `relevant_guide_topic`, `names_tracker_path`.
- `src/prompts/guide_index.rs` — `declares()` (the per-topic phase switch), `match_sections`.
- `src/server.rs:3272` — the guard that covers the converse property only.
- `d94dd53d` — the commit that moved the text out; its claim of delivery was wrong.
