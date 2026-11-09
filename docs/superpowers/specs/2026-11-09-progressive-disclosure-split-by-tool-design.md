# progressive-disclosure split by overflowing tool (design)

**Status:** approved by the operator 2026-11-09; built in `281ff16b3` (2026-11-10). Step 2 of the pilot in bug `7579b32b1cd2362f` (`docs/issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md`, § Pilot, step 2). Measured result: grep 3,703 B, run_command 8,730 B, doc preamble 643 B wrapped; 5.1 MB saved over the census. The byte targets in Goals below were estimates and are superseded by those numbers. Step 1, the trim of the pipe-capture bullet, is done (`f34fbc8b3`).

## Intent

`progressive-disclosure` is 45% of all guide bytes delivered in the 30-day census: 1,250 whole deliveries, 13.7 MB, median 10 KB (917 to subagents, 333 to parents). Each delivery ships the whole 13.3 KB body whichever tool overflowed. Seventy percent of subagent deliveries follow a `run_command` overflow and 20% a `grep` overflow. The parts that concern only `run_command` (the `.err` suffix, background jobs, 3.8 KB together) go to every other overflow too. Layer-1 numbers say the `.err` subsection is followed by its use in 2% of cases.

The aim is to deliver, on an overflow, the sections written for the overflowing tool. Scenario estimate from the issue: about 7.4 MB of 13.7 MB over the census, under two assumed section sizes.

## Goals

- An overflow from `grep`, `read_file`, `tree`, `semantic_search`, `symbols`, `references` or `call_graph` delivers about 2.9 KB (the two core sections) instead of 13.3 KB.
- An overflow from `run_command` delivers the core sections plus the `.err` and background-job sections, about 7 KB.
- An overflow from `doc` or `librarian` delivers the preamble, which carries a one-paragraph minimum.
- An explicit `get_guide("progressive-disclosure")` still returns the whole topic, every section included.
- No behaviour changes for any other topic.

## Non-goals and anti-goals

- No new kind of `serves:` shape, and no change to the `overflowing` gate in `emit_guide_sections`.
- No change to the text of the sections that become pull-only (Buffer internals, Path-relative annotation, Anti-patterns). They move or stay; their wording does not change.
- Anti-goal: a section that is declared but never reachable, or a shape a gate cannot account for. Gate 2 (`every_observed_shape_of_a_declaring_topic_has_a_section`) must pass without a shape waiver. Gate 5 (`every_section_of_a_declaring_topic_is_reachable`, `guide_index.rs:1939`) also applies once the topic declares anything: each pull-only part needs a `SECTION_WAIVERS` row (`src/prompts/mod.rs`) with a reason over 40 characters. That is the mechanism built for pull-only parts, so the design uses it (see Decided).
- Anti-goal: touching `workspace-state.md:87`, `src/server.rs:317` or `docs/conventions/cross-machine-catalog-resume.md:462`. They cite `progressive-disclosure` § Path-relative annotation, and that heading must stay.

## Verified facts the design rests on

- A declared shape is `tool`, an optional `action`, an optional `path~` taken from the result (`src/prompts/guide_index.rs`, `Shape::matches`). It cannot see call arguments. The finest split is by overflowing tool.
- Eight tools route to this topic: `run_command`, `grep`, `read_file`, `tree`, `semantic_search` always, and `symbols`, `references`, `call_graph` only when the result has `overflow` or `output_id`. Librarian tools route to it on overflow through the adapter. The emitter then gates the topic on `ctx.overflowing`.
- For a declaring topic, `guide_blocks_for` delivers the sections matching the call's shape (plus their `requires:` closure) that this ledger has not yet received, keyed `topic#heading`. If no section matches the shape it delivers `topic#<preamble>`: the text before the first heading plus a pointer to the full topic.
- `no_two_topics_declare_an_overlapping_shape` forbids this topic from declaring `doc.*` or `librarian.*`, which `librarian` and `tracker-conventions` declare.
- `every_observed_shape_of_a_declaring_topic_has_a_section` judges every census shape whose tool routes to the topic on its probes: here `run_command`, `grep`, `read_file`, `tree`, `semantic_search`.
- Gate 5 `every_section_of_a_declaring_topic_is_reachable` requires every section of a declaring topic to have a `serves:`, to be named by another section's `requires:`, or to have a `SECTION_WAIVERS` row. A `requires:` would deliver the section, which defeats pull-only.
- `MAX_DECLARED_SECTION_BYTES` is 2,500 and `declared_sections_are_within_the_size_cap` enforces it on declared sections.
- `a_topic_without_declarations_reports_false` (`guide_index.rs`) uses this topic as its example of a non-declaring topic.

## Approaches

- **A. Declare sections, serve by overflowing tool (chosen).** The topic gains `serves:` lines. Uses the mechanism as built; every guard stays.
- **B. A new "any overflow" shape kind.** Rejected: a second implementation of that shape kind does not exist, so it is an abstraction with one user.
- **C. A short variant body per tool, like the subagent bootstrap pointer.** Rejected: duplicates content that then drifts, and the bootstrap pointer worked only because another channel (the hook) carried the content.
- **D. Do nothing further.** After the trim the topic is still 13.3 KB on every overflow.

## Decided

New layout of `src/prompts/guides/progressive-disclosure.md`. Each declared section is at most 2,500 B.

| Part | Target bytes | Served on |
|---|---|---|
| Preamble (before the first heading) | at most 450 | fallback for any overflow no section serves (`doc`, `librarian`) |
| `## Output budgets` (unchanged) | 901 | core tool list |
| `## Reading a buffer` (the envelope fields, the query forms, the `[*]` grammar, the paragraph on the envelope's `hint`; moved out of today's "The @ref buffer") | about 2,000 | core tool list |
| `## Buffer internals` (reading a buffer never makes another; paging; the pool; truncation; handle kinds; the rest of today's "The @ref buffer") | about 2,200 | pull-only, no `serves:` |
| `### Reading the stderr of a buffered command: the .err suffix` (unchanged) | 989 | `run_command` |
| `## Background jobs: start, end the turn, read after the wake` (first part of today's section) | about 1,600 | `run_command` |
| `## Background jobs: when no wake arrives` (rest of today's section: no-wake cases, the server-ended case, waiting on a foreign process) | about 1,250 | `run_command` |
| `## Path-relative annotation` (unchanged) | 2,068 | pull-only |
| `## Anti-patterns` (unchanged after the trim) | about 2,100 | pull-only |
| `## Related` | 84 | pull-only |

- **Waivers (my call, from the builder check):** the four pull-only parts (`## Buffer internals`, `## Path-relative annotation`, `## Anti-patterns`, `## Related`) each get a `SECTION_WAIVERS` row for `progressive-disclosure` with a reason that says why the part is pull-only (reachable through `get_guide`, not tied to one tool's overflow). The builder may fold `## Related` (one line) into the preamble or the end of `## Buffer internals` to need one row fewer.
- **Core tool list** is `run_command`, `grep`, `read_file`, `tree`, `semantic_search`, `symbols`, `references`, `call_graph`. The five that Gate 2 judges are all in it.
- The preamble is rewritten to carry the minimum a reader of an overflow needs in one paragraph: a result over the inline budget is stored server-side; the response is an envelope with an `output_id` handle (`@tool_*`, `@cmd_*`, `@file_*`, `@bg_*`); read it with `read_file("@tool_xyz", json_path=...)`, `grep PATTERN @cmd_abc` or line ranges; do not re-run the tool.
- Text that moves keeps its wording. The sentence that `src/tools/core/tests.rs` cites ("or in strings the clip stops cutting at 1,000 raw bytes each") stays verbatim in `## Buffer internals`.
- `a_topic_without_declarations_reports_false` switches to a topic that stays non-declaring (`error-handling`, to be confirmed non-declaring by the builder).
- The two existing pins (`progressive_disclosure_guide_states_the_foreground_pipe_capture`, `..._states_the_background_job_recipe`) read the whole body and must still pass; the background recipe needles must all land in the two background sections together.

## Tests that prove it

- First overflow from `grep` delivers the two core sections and nothing from `.err` or the background sections; a second `grep` overflow in the same ledger delivers nothing.
- First overflow from `run_command` delivers the core, `.err` and both background sections; a later overflow from `grep` then delivers nothing new.
- An overflow result from `doc` delivers the preamble and the `Preamble` shape, not a section.
- A subagent's ledger (`caller_is_subagent`) behaves the same as a parent's.
- A byte budget: wrapped delivery for a `grep` overflow at most 3.6 KB; for a `run_command` overflow at most 8 KB. Pinned like `the_subagent_opener_block_is_a_fraction_of_the_whole_one`.
- `get_guide("progressive-disclosure")` returns a body containing every heading in the table.
- Gate 2, Gate 5 (with the waiver rows), the collision test and the size-cap test pass.

## Consequences and costs

- Now easier: a section can be served to a new tool by one `serves:` edit; the size cap stops re-growth per section.
- Now harder: an agent whose overflow came from `doc` or `grep` no longer receives the background-job recipe or the buffer internals unless it fetches the guide. Whether that changes behaviour is not measured: no control group exists for this topic.
- One-time double delivery: a session whose persisted ledger already holds the bare `progressive-disclosure` key from before this change receives the core sections once.
- An explicit `get_guide` stamps only the bare topic key, so a later overflow can still auto-deliver sections the agent just read. The same already holds for `librarian`.
- Absence of the pipe-capture bullet and the Path-relative text from overflow deliveries: that text reached overflow readers only because the whole body shipped. The tool descriptions and `iron-laws-detail` carry the pipe rule.

## Change scenarios absorbed

- A new tool whose result can overflow: add its name to the core `serves:` list. Without it the tool gets the preamble, and Gate 2 fails if the tool's probe routes here.
- The background-job text grows past the cap: the size-cap test fails and the text is split again.
- A pull-only section proves useful after delivery: give it a `serves:` line, no structural change.

## Revisit when

- A re-run of the census after the change shows `progressive-disclosure` deliveries per session or bytes per delivery not falling as estimated.
- Tool-call errors on buffer reads rise (a `json_path` on a `@cmd_*` handle, a `.err` on a `@bg_*` handle, a re-run instead of a buffer query) against the 30 days before the change.
- A fourth declaring topic needs a shape that collides with the core tool list.

**Confidence:** medium. The mechanism and the guards are read and understood; the byte estimate rests on two assumed section sizes that the build replaces with measured ones; the effect on agent behaviour is unmeasured.

## Left to the builder

- The exact wording of the preamble, within 450 B and the content above.
- Where today's "The @ref buffer" is cut between `## Reading a buffer` and `## Buffer internals`, and where the background-job section is cut, so each declared part is at most 2,500 B and every pinned needle still appears in the body.
- Whether `error-handling` is the right replacement exemplar in `a_topic_without_declarations_reports_false`.
- The names of the new tests, and the exact byte budgets once the sections are measured.
