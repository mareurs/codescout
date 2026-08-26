---
id: '7579b32b1cd2362f'
kind: bug
status: open
title: Guide topics are atomic nodes in a graph nobody modelled — 63% of the corpus auto-injected in one session, and three guides already cite sections the API cannot serve
tags:
- guides
- prompt-surface
- get_guide
- progressive-disclosure
- proposal
---

## Symptom

`get_guide(topic)` serves a whole topic body or nothing. Topics are the only unit
of addressing, so a session that needs one section of a guide receives all of it,
and a guide that wants to point at part of a sibling can only point at the whole.

Measured on the corpus, 2026-08-27:

```
error-handling.md                  1,857
project-activation-bootstrap.md    2,594
symbol-navigation.md               3,145
untrusted-content.md               5,317
progressive-disclosure.md          5,669
librarian-runtime.md               9,774
workspace-state.md                10,355
iron-laws-detail.md               11,238
librarian.md                      20,545
tracker-conventions.md            34,333   <- 33% of the corpus, one topic
                                 -------
                                 104,827
```

**In a single working session** (claude-plugins, `f6ae2d77`, 2026-08-26/27) the
auto-inject path delivered `project-activation-bootstrap`, `tracker-conventions`,
`librarian`, `symbol-navigation` and `progressive-disclosure` — **66,286 bytes,
63% of the entire guide corpus** — and `project-activation-bootstrap` fired a
second time after an MCP reconnect. The session consumed perhaps five sections of
it. `tracker-conventions` alone (34 KB) auto-injects on the first `artifact` call
of any session, whatever that call was for.

## The graph already exists — in prose, unresolvable

Measured the same day: **18 `get_guide("...")` citations across 7 of the 10
guides.** They are edges. Nothing reads them.

```
librarian            -> tracker-conventions (x4), librarian-runtime
iron-laws-detail     -> workspace-state (x2), progressive-disclosure, error-handling
workspace-state      -> progressive-disclosure (x2), error-handling
librarian-runtime    -> librarian, tracker-conventions
tracker-conventions  -> librarian, librarian-runtime
progressive-disclosure -> error-handling
symbol-navigation    -> progressive-disclosure
```

**Three of the eighteen already cite a SECTION** — a granularity the tool cannot
serve, written by authors who evidently wanted it:

- `librarian.md:139` — `get_guide("tracker-conventions")` § *One entry format, never two*
- `tracker-conventions.md:604` — `get_guide("librarian-runtime")` § *Trackers as cross-session behavior*
- `workspace-state.md:81` — `get_guide("progressive-disclosure")` § *Path-relative annotation*

A reader following one of those gets the whole sibling and has to find the section
by hand. That is the defect in its cheapest observable form: **the documentation's
own cross-references are more precise than its retrieval API.**

## Counterexample — the same delivery rule, the opposite outcome

Added 2026-08-27 from a concurrent session (`77c6f4ae`) in this checkout, which
volunteered the case that cuts against the framing above. **It is the arm the
measurement was missing, and it changes the claim.**

That session took **six** auto-injected guides on the same rule —
`project-activation-bootstrap`, `progressive-disclosure`, `tracker-conventions`,
`symbol-navigation`, `librarian`, `workspace-state`, each on the first call
touching its topic, none requested. Delivery reproduced exactly.

But `tracker-conventions` **earned its 34,333 bytes there.** Archiving two bug
files, the session used the status vocabulary, the archive trigger, the
SHA-plus-patch-id rule, the citation-sweep grep, the `## PREFIX-N — title`
definition rule, and the write-the-index-row-after rule — six or seven sections,
and at least two *changed what it did* rather than confirming it. The
`--include`-list-is-a-hypothesis warning is why it re-ran a citation sweep with
`include_hidden=true` after a clean zero; the definition rule is why it checked a
heading was `## W-71 — ` rather than merely present.

**Why this matters more than either number.** A single high-delivery /
low-utilisation measurement has no resolving power between two hypotheses:
*the guide is too big*, and *the guide is delivered without regard to whether this
session needs it*. Both predict 63% delivered and five sections used. Only a
second arm separates them — and the second arm shows near-full utilisation under
the identical delivery rule. So the size is not the defect. **The absence of
targeting is.**

One corpus, one delivery rule, two sessions, opposite outcomes. That is an
argument for **addressing**, and specifically an argument against shrinking as a
standalone remedy — see the risk now recorded under (b).

(Method note: this is `claude-plugins:W-4`'s shape — a measurement that returns
the same value under every hypothesis reads exactly like a measurement that
settled something. The original text stated the delivered-vs-used limit honestly
but still led with the byte count, which is the interpretation that limit does not
support.)

## Root cause

Bodies are `include_str!`'d and dispatched by a hardcoded match on topic name:

- `src/prompts/mod.rs:503-513` — one arm per topic in `topic_body`
- `src/server.rs:1486` — `librarian.md` embedded a second time for `static_doc_sources`
- `src/prompts/mod.rs:1629` — the test `guide_topics_have_bodies` enforces the arm

So the topic name is simultaneously the file name, the cache key, the API surface,
and the only addressable unit. Adding a topic means editing Rust and rebuilding;
splitting one means renaming the API. Consequences worth stating plainly:

- **Granularity is frozen at whatever the file happens to contain.** `tracker-conventions`
  is really about six topics (bug files, tracker frontmatter, ledger declaration,
  entry ids, citations, compaction/archival) that grew into one file.
- **`R-89` applies twice.** Guides are fixed at build time and again at process
  start, so a long-lived MCP session serves a stale body after any rebuild.
- **Auto-inject is all-or-nothing per topic**, and it is the dominant delivery
  path — the 66 KB above arrived unbidden, not through explicit `get_guide` calls.

## Proposal — model the guides as a graph

This mirrors work happening on the `claude-plugins` side for buddy specialists,
where the same shape was found: a real schema, a latent edge set, and composition
expressed only in prose. That design settled on **primary + advisors** — one node
owns the voice and output contract, others contribute subordinate sections via a
projection rule — deliberately generalising the one composition primitive that
already worked (`_<lens>.md` addenda) rather than inventing merge semantics.

**That design explicitly scoped guides OUT as reference-only**, on the grounds
that they are compiled into this binary and served atomically. The buddy graph can
route *to* a guide; it cannot slice one. This issue is the codescout half, and it
is a prerequisite for the guide corpus ever participating in that composition.

Three directions, in increasing cost. They are not exclusive — (a) is a
prerequisite for the others and worth doing alone.

### (a) Make sections addressable — no restructuring

Add an optional `section` parameter: `get_guide(topic, section="Entry ids")`,
resolving against the body's headings the way `read_markdown` already does for
files. Nothing moves; the match arm stays; the three section-qualified citations
above start resolving. Cheapest real progress, and it converts the prose `§` into
something a caller can act on.

Open question: whether the auto-inject path can pick a section, which requires a
trigger to say what it is *about*, not merely that it fired.

### (b) Split the oversized topics

`tracker-conventions` (34 KB) and `librarian` (20 KB) are 52% of the corpus
between them. Splitting them into the topics they already contain shrinks the
atom without changing the mechanism.

**Do not do this before (a), and possibly not at all.** The counterexample above
is a session that used six or seven sections of `tracker-conventions` in one
sitting. Auto-inject fires on *the first call that touches a topic* — so after a
split, that session receives whichever fragment its first `artifact()` call maps
to, and must then know that five more exist and request them by name. **Splitting
without addressing does not reduce cost; it moves the cost onto the caller and
converts a silent over-delivery into a silent under-delivery**, which is the worse
failure because nothing in the transcript shows what was missing.

The precedent is already in the corpus and it is not encouraging.
`librarian.md:407` ends by routing to `get_guide("librarian-runtime")` — an
operational reference split out by hand, explicitly to keep the parent lean. That
split already happened, and what it produced was **an edge nothing reads**: a
reader who needs both now needs two calls and has to know to make the second. That
is the outcome (b) generalises, absent (c).

Remaining costs if it is done anyway: more match arms, and **every existing
citation of the old topic name breaks** — including shipped prompt surface and
downstream repos' skills. Sequence it after (a) so section addressing serves as
the compatibility shim.
### (c) Declare the edges

Give each guide frontmatter naming its `requires` / `see-also` / `supersedes`
edges, and have the 18 prose citations derive from that rather than restate it.
Enables: "pull this topic and its prerequisites", a lint for dangling topic
references, and a graph a router could traverse. This is the piece that makes the
corpus composable rather than merely sliceable.

## Explicitly NOT proposed

- **Moving bodies out of the binary.** Considered and rejected for now on the
  claude-plugins side: `include_str!` is what makes a guide always present with no
  install step, and that property is worth more than rebuild-free editing. The
  staleness it causes is `R-89`'s problem, not this one's.
- **A resolver that assembles guides into one payload.** The buddy design chose
  no-resolver on YAGNI grounds — load the pieces, let the model compose. Same
  reasoning applies here until something proves it insufficient.

## Not yet done

The 63% figure measures what was **delivered**, not what was **used** — stated as
a limit when this was filed, and now partly answered from the other direction by
the counterexample above, which supplies a high-utilisation arm under the same
delivery rule. Together they establish that utilisation *varies by session*, which
is precisely the case for targeting.

What is still unmeasured is the **distribution**: two sessions is two points, and
they were selected by being the two that happened to be talking to each other, not
by any sampling rule. Neither is evidence about the typical session. The probe
worth running before (b) or (c) is which sections of `tracker-conventions` are
cited back or acted on across a real sample — and note that the two arms here
would both survive a bad sampling design, so the sample is the thing to get right.

"`tracker-conventions` is really six topics" remains an authoring judgement, not a
measurement. It is the kind of premise that reads as settled because it is stated
by someone who knows the file, and it deserves re-costing before anyone builds on
it.
