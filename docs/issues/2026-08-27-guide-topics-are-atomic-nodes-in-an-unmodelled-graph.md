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
atom without changing the mechanism. Costs: more match arms, and **every existing
citation of the old topic name breaks** — including the ones in shipped prompt
surface and in downstream repos' skills. Sequence it after (a) so section
addressing can serve as the compatibility shim.

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

No measurement of how often a session needs only part of a guide versus all of it.
The 66 KB / 63% figure above measures what was *delivered*, not what was *used* —
those are different claims and only the first is established. A probe worth running
before (b): instrument which sections of `tracker-conventions` are actually cited
back or acted on across a sample of sessions. Splitting on the assumption that the
file is six topics is an authoring judgement, not a measurement, and it is the sort
of premise that deserves re-costing before anyone builds on it.

**Valid:** dated 2026-08-27

