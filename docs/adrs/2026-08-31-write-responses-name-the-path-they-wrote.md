---
id: '437223096ca67296'
kind: adr
status: draft
title: ADR-2026-08-31 — Write responses name the path they wrote
owners:
- marius
tags:
- tool-contracts
- no-echo
- operator-rules
- response-shape
topic: tool-contracts
---

## Context

codescout's write tools answer `json!("ok")` — the **no-echo convention**, named in
`CLAUDE.md` and in the `conventions` memory. Its purpose is that a write must not echo
the *content* it wrote: `call_content` refuses to clone its own input for exactly this
reason, because `edit_file` and `create_file` carry whole file bodies in it.

The convention was never absolute. Two exceptions already shipped:

- `memory/mod.rs:844` returns `{"status":"ok","warnings":[…]}` when a best-effort side
  effect failed, on the grounds that the caller cannot otherwise know.
- `annotate_write_root` promotes a bare `"ok"` to `{"status":"ok","wrote_to":<root>}` for
  every unpinned write **in a repo that has linked git worktrees**, so a call that resolved
  against the session default is distinguishable from one that landed where the caller
  meant. *(Scope corrected 2026-10-16: this originally read "for every unpinned write".
  See the Amendment below.)*

`OP-4` declares `**Serves:** edit_file(path~/.claude), create_file(path~/.claude)`. Its
predicate runs through `names_path_containing`, which scans the **response**. So the rule
was structurally dead: the only path-shaped field a write response carried was
`wrote_to`, which names the project root — for a write to `~/.claude/settings.json` that
is the codescout checkout, and matches no `~/.claude` needle.
`docs/issues/archive/2026-08-28-op-4-path-predicate-can-never-fire.md` recorded it, with mutations
showing that widening the scan to `wrote_to` still does not fire while a response carrying
a real `abs_path` does.

## Decision

**A write response names the path it wrote**, via `annotate_write_path` in
`call_content` — a sibling to `annotate_write_root`, not a fold into it.

- Keyed by absoluteness: `abs_path` for an absolute path, `rel_path` otherwise.
  `names_path_containing` scans both, and a relative path filed under `abs_path` would be
  a lie the matcher happens not to notice.
- Gated on `is_write` and the existing annotation exemption list — **not** on
  `workspace_override`. That gate exists on `annotate_write_root` because a pinned call
  already named the checkout it meant; which *file* it wrote is a fact about the call
  either way.
- Never overwrites a path the tool set itself.
- Captured from `&input` before `self.call` consumes it, cloning one string rather than
  the input.

**This is a narrow widening of no-echo, not its repeal.** A path is bounded, is already
half-present as `wrote_to` *where a repo has linked worktrees* (in a single-checkout repo it
is not; see the Amendment), and tells the caller where the write landed. The content
prohibition is untouched.

## The alternative, and why measurement rejected it

The other route was to match the predicate against the call **input** instead of the
response — leaving no-echo entirely intact. It was rejected on blast radius, measured
rather than argued:

| | this decision | input-threading |
|---|---|---|
| touches | `call_content` only | `route()` → `Shape::matches` → predicate |
| shared with | nothing | **the shipped guide section-grain feature**, `guide_index.rs:194` |
| tool `call` returns changed | none | none |
| `json!("ok")` assertions broken | **0 of 44** | 0 |

`Shape::matches` is the same matcher the guide section-grain feature uses, and its
Phases 2–3 are open as `GG-N`. Threading an input through it widens a signature two
subsystems depend on while one of them is mid-flight.

Note the row that inverted the original framing: this was first described as "bend a
convention" versus "bend a signature", implying the signature change was the contained
one. Measured, it is the reverse.

## Confidence

High on the mechanism, and it is asserted end-to-end rather than by construction:
`op_4_routes_on_a_write_response_the_pipeline_produced` feeds `route()` a response that
came back out of `call_content` past the path-stripper and both annotations, with a
negative control for a path outside `~/.claude`.

The pin that was supposed to detect this fix landing **could not**, and that is the
strongest evidence for asserting on produced values rather than fabricated ones:
`op_4s_path_predicate_cannot_fire_against_a_write_response_today` asserted against a
hand-written response bound to a variable named `observed`, and so could see neither the
bug nor its repair. It is removed, per its own instruction.

## Revisit-when

- A write tool needs to answer with a path it did **not** take from `input["path"]` —
  the capture is a single well-known key, not a general mechanism. **Revisited 2026-10-17:
  that has now happened, once; see the Amendment below for what holds.**
- `names_path_containing` grows beyond `abs_path`/`rel_path`/`items[]`/`violations[].path`,
  at which point the absoluteness keying should be re-checked against what it scans.
  (Grew 2026-10-17 by `old_abs_path`/`new_abs_path`, a `doc(move)` result.)
- The no-echo convention is revisited as a whole. This ADR widens it by one bounded field
  and does not license a second widening by analogy.

## Amendment 2026-10-16 — `wrote_to` was never universal; for a single checkout this ADR is the first shape change

- **Status:** accepted correction to a shipped decision. **Trigger:**
  `docs/issues/archive/2026-10-16-write-path-adr-says-every-unpinned-write-carries-wrote-to.md`.

The Context said `annotate_write_root` promotes a bare `"ok"` "for every unpinned write",
and the Decision called the path "already half-present as `wrote_to`". Both overstated the
scope. `annotate_write_root` returns at once when
`crate::util::path_security::list_git_worktrees(root)` is empty
(`src/tools/core/types.rs`, the function's first statement), and its doc comment says the
single-checkout case is untouched. That gate predates this ADR (added in `22e60ee9`,
2026-08-27). So `wrote_to` exists only in a repo with linked worktrees.

What that changes:

- **In a repo with linked worktrees** the ADR's account stands as written: a write already
  answered `{"status":"ok","wrote_to":…}`, and `annotate_write_path` added `abs_path` or
  `rel_path` to that object.
- **In a single-checkout repo** a write answered the bare string `"ok"` before this ADR.
  `annotate_write_path` is there the **first** change of the write response shape: it
  promotes `"ok"` to `{"status":"ok","rel_path":…}` (or `abs_path`) for every write whose
  tool is not exempt, pinned or not. In the common case the no-echo convention therefore
  moved by one bounded field *and* by one response shape (string to object), not by one
  field on a response that was already an object.
- **The decision is unchanged.** The content prohibition is untouched and the path is still
  bounded. The measurement table above is not re-derived here.

A reader sizing the blast radius of a future widening of no-echo should start from this:
the baseline for most repos is a bare string, and a second annotation there is a shape
change, not a field addition.

## Amendment 2026-10-17 — the `doc` tool names the row it wrote, taking the path from the catalog

- **Status:** accepted extension of the decision. **Trigger:**
  `docs/issues/2026-10-16-tracker-conventions-write-sections-are-declared-but-unreachable.md`.
- **What happened.** `annotate_write_path` captures `input["path"]`, and the `doc` tool has no
  such input: it names its target by `id` (`src_id` for `link`). Four `doc` writes —
  `append_entry`, `update_entry`, `augment`, `link` — therefore answered `{id, …}` or a bare
  `"ok"` that named no file, so the guide router, which reads the RESULT, could never pick the
  guide section written for a write to a tracker. This is the Revisit-when's first condition,
  and it is a second mechanism, not a widening of the first: `name_the_target`
  (`src/librarian/tools/artifact.rs`), in the `doc` dispatcher, resolves the id against the
  catalog and puts `abs_path` on the result. A bare `"ok"` is promoted to `{"status":"ok",
  "abs_path":…}`, the same promotion `annotate_write_path` already makes.
- **What now holds.**
  1. **The path named is the row the write LANDED ON**, not the row the caller's id points at.
     In a worktree session a write to a main-checkout artifact lands on its overlay shadow
     (`resolve_write_target`), so the id the caller passed and the file that changed differ.
     `name_the_target` asks `worktree::peek_write_target`, which computes the shadow without
     forking anything and shares `shadow_coordinates` with the forking function so the two
     cannot disagree. The first cut read `args["id"]` and named the main-checkout file, which
     the write never touched (a reviewer's probe; pinned by
     `a_write_names_the_row_it_landed_on_in_a_worktree_session_and_the_artifact_otherwise`).
  2. **Scope is the four writes above and `move`** (which already answered
     `old_abs_path`/`new_abs_path`, now scanned). It is **not** `update`: naming a status
     change delayed librarian's anti-wipe sections (measured; see the issue), so every
     `update` keeps answering without a path. Any further `doc` action named this way needs
     the same measurement of what its routing displaces.
  3. **Input-threading stays rejected** for the reason in the table above: it widens
     `Shape::matches`, which the operator rules share.
  4. The path is bounded (one string, project-relative after the stripper) and carries no
     content, so the no-echo prohibition is untouched. Measured cost, in compact JSON: an object
     result gains `14 + len(path)` B and a promoted bare `"ok"` gains `25 + len(path)` B over `"ok"`:
     +50 B and +61 B for the probe path, and +59 B and +70 B at the median `docs/trackers/` path
     (45 characters, n = 92).
  5. **A `link` names the row whose FILE it wrote.** A plain edge writes only a catalog row on its
     source, so the source is named, through the overlay. A `supersedes` link writes the
     DESTINATION's frontmatter, so the destination is named, as the caller passed it:
     `link::call` hands `dst_id` to `write_field_to_frontmatter` unresolved, so from a worktree
     session it writes the MAIN checkout's file (filed:
     `docs/issues/2026-10-17-a-supersedes-link-from-a-worktree-session-writes-the-main-checkouts-destination-file.md`),
     and the result names what was written, not what should have been. Pinned per rel kind, in
     and out of a worktree session. **Superseded by Amendment 2026-10-24 below for the
     `supersedes` destination in a worktree session**; the rest of this item holds.
  6. The row is looked up AFTER the write, which takes the catalog lock a second time. A
     `merge_worktree` or abandon landing in that window makes the result name the main row's
     path or none. Cosmetic: the answer is advisory and routing does not depend on which it
     names.

## Amendment 2026-10-24 — a `supersedes` destination goes through the overlay, and is named through it

- **Status:** accepted; corrects item 5 of the 2026-10-17 amendment. **Trigger:**
  `docs/issues/2026-10-17-a-supersedes-link-from-a-worktree-session-writes-the-main-checkouts-destination-file.md`.
- **What now holds.** `link::call` resolves a `supersedes` link's destination with
  `resolve_write_target` before writing it, as it already did the source. From a worktree session
  the destination is forked into its overlay shadow and `status: superseded` is written to the
  SHADOW copy at the worktree path; the main checkout's file is untouched until `merge_worktree`.
  So the result now names the shadow path (`name_the_target` resolves `dst_id` through
  `peek_write_target` like every other write; the `through_overlay` flag it needed while the
  destination was named "as passed" is gone). Outside a worktree session nothing changes: the
  destination's own path is named. A plain link is unchanged (source, through the overlay). The
  link edge, the `superseded_by` event payload and `superseded.id` in the result still carry the
  `dst_id` the caller passed. Pinned by
  `link::tests::supersedes_from_a_worktree_session_writes_the_shadow_and_not_the_main_file` and,
  for the naming, `a_link_names_the_row_whose_file_it_wrote_for_each_rel_in_and_out_of_a_worktree`.

## Sites (initial)

- `src/tools/core/types.rs` — `annotate_write_path`, and its capture in `call_content`
- `src/util/librarian_response.rs` — `names_path_containing`, the consumer
- `src/operator_rules/route.rs` — `op_4s_predicate_is_itself_sound_given_a_path_bearing_response`
- `src/tools/core/tests.rs` — `op_4_routes_on_a_write_response_the_pipeline_produced`
- `docs/issues/archive/2026-08-28-op-4-path-predicate-can-never-fire.md`
