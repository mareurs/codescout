---
id: dbc617677d44cf8a
kind: adr
status: proposed
title: ADR-2026-10-07 — One measure of the delivered response
owners:
- marius
tags:
- design-principle
- progressive-disclosure
- tool-contract
topic: tool-contracts
---

# ADR-2026-10-07 — One measure of the delivered response

## Status

Proposed. Written 2026-10-07 after the byte-bound sibling sweep
(`docs/issues/archive/2026-10-06-sibling-sweep-the-byte-bound-defect-recurred-across-tools.md`),
whose Resume section asks for this. Nothing here is built yet.

## Context

The inline limit is one predicate, `exceeds_inline_limit` in `src/tools/core/types.rs`. The
sweep found the same defect in nine tools and arms across six review rounds: a size measured in
one unit or on part of the response, a different thing returned. Each round found new siblings,
so the cure was never in any one fix.

What the code looks like today (counted 2026-10-07, tests excluded):

- **31 production call sites outside `core` decide inline-or-buffered**: 15 in `read_file.rs`, 7 in
  `read_markdown.rs`, 3 in `command_summary.rs`, 2 in `run_command/output.rs`, and one each in
  `memory/mod.rs`, `file_summary/file_summary.rs`, `librarian/adapter.rs` and `legibility/mod.rs`.
  Each is one of three shapes: `exceeds_inline_limit(&candidate.to_string())` (the sound one),
  `exceeds_inline_limit(&raw_body)` (the defect shape, now kept only as a proven-equivalent
  pre-check in six places), or `exceeds_inline_limit_len(n)` with a length the caller computed.
- **The room formula exists twice.** `buffer_page_room` (`read_file.rs:569`) and
  `inline_stdout_room` (`run_command/output.rs:763`) are both
  `INLINE_MAX_RESPONSE_LEN - skeleton.to_string().len()` (the second also subtracts one key's cost).
- **A third measure disagrees with them.** `delivered_len` (`core/types.rs:146`) is the only one
  that knows the cut record is stripped before delivery. The two room functions measure the record
  as if it were delivered.
- **Three tools independently built the same carrier for keys that must be counted by the gate:**
  `LateKeys` (`run_command/output.rs:668`), the `extra` slice (`memory/mod.rs:658`) and the `noted`
  closure (`read_file.rs:287`). Each was a fix for a key added after the gate had decided.
- **The one-handle backstop covers only `output_id` envelopes** (`clip_prebuffered_envelope` returns
  early without a string `output_id`). `read_file`, `read_markdown` and `memory` return `file_id`, so
  for them the contract rests entirely on each arm sizing itself correctly.
- **A source-scanning gate already exists**: `tests/result_caps.rs` walks the tracked `src/` files
  (`tracked_src_files`), parses `// cap-class:` annotations, and scans for truncation sites
  (`TruncSite`). A new rule can reuse that harness.

The defect had two roots, and they need different cures:

1. **Unit.** A body measured in raw bytes is returned escaped inside a larger object.
2. **Late keys.** A key is added after the gate decided (`buffer_truncated`, `jobs`, `timeout_hint`,
   the refresh line, `resolved_from`, `write_target`).

## Decision

Build it in three phases, each shippable alone. Phase A is the decision; B and C are recorded so
they are not rediscovered, and each needs its own go.

**Phase A: one module owns "the response as delivered".** A new `src/tools/core/response_fit.rs`
(a name this ADR proposes; it does not exist yet) holds:

- `delivered_len(&Value)` (moved from `types.rs`, unchanged);
- `response_fits(&Value) -> bool`: the serialized, as-delivered response is within the limit;
- `response_room(widest: &Value) -> usize`: the limit less `delivered_len(widest)`, which replaces
  both existing room functions and gives them the cut-record rule they lack;
- `body_alone_overflows(raw: &str) -> bool`: true only when the raw body ALONE is over the limit.
  Escaping never shortens text, so `true` proves the response is over. It answers one direction
  only, so it cannot be used as a "fits" gate; this is the asymmetry that replaces the six raw
  pre-checks.

The text predicate `exceeds_inline_limit(&str)` becomes private to `core` (`call_content` and the
backstop use it on a string they have already serialized). Code outside `core` can no longer write
a raw-body gate: the call does not compile. The 31 sites migrate mechanically.

`exceeds_inline_limit_len(n)` stays callable from `run_command` and `command_summary`, which
compute a length from parts. Each such site (4 today) carries a `// inline-gate: <what unit n is>`
annotation, and a source check beside `tests/result_caps.rs` fails a production use without one.

**Phase B (not decided here): extend the backstop to `file_id` envelopes**, so a mis-sized arm still
cannot mint a second handle. Open question: clipping a page's `content` makes its `shown_lines` and
`next` claim lines the caller did not see. It would be marked, not silent, but it is data the caller
must notice is missing.

**Phase C (not decided here): one carrier for keys the gate must count**, replacing `LateKeys`,
`extra` and `noted`. Three concretes exist, so the abstraction is justified; it is also the cure for
root 2, which Phase A does not touch.

## Alternatives considered

- **Keep reviewing.** Rejected: six rounds each found new siblings; a review proves the arms it
  probed, not the next one added.
- **A lint alone, no visibility change.** Rejected as the first move: a text scan cannot tell a
  serialized argument from a raw one, so it needs an annotation escape hatch for every legitimate
  use, and a program-of-annotations is the parser-over-a-namespace shape
  (`docs/conventions/parsers-over-a-namespace.md`). Visibility is a compile error with no
  false positives. The lint is kept only for the `_len` form, where it has four sites.
- **One `inline_or(candidate, fallback)` combinator.** Rejected: it is
  `if response_fits(&c) { c } else { fallback() }`; each fallback differs (`file_id`, pagination,
  summary), so it saves nothing and hides the branch.
- **Phase B alone.** Rejected as the first move: it closes the second-handle contract for
  `file_id` tools but leaves every unit error as a silent clip. It is the backstop, not the fix.
- **Make every tool return a typed `Draft` and move the gate into `call_content`.** Rejected for
  now: it changes the `Tool` trait for ~30 tools to solve root 2, which Phase C solves with three
  existing concretes and no trait change.

## Consequences

- Now easier: a new tool cannot write a raw-body gate; the response-size formula exists once, so the
  cut-record rule cannot drift between sites; a reviewer has one name to search for.
- Now harder: `response_fits` takes a `Value`, so a gate must build its candidate first (the six
  pre-checks, now `body_alone_overflows`, keep the cost off huge bodies). A deliberate bypass
  remains possible: `response_fits(&json!(content))` measures the body alone and brings the old
  defect back. Types cannot stop it; review and a mutation test on each migrated arm must.
- Not addressed by Phase A: late keys (root 2). Until Phase C, a tool that adds a key after its gate
  is still wrong, and nothing but a probe through the real tool finds it.

## Change scenarios absorbed

- A new tool or arm adds its own inline gate: it must use `response_fits` or `response_room`, so it
  measures the delivered response.
- The cut-record key changes, or another key is stripped before delivery: one function changes.
- The limit changes (`MAX_INLINE_TOKENS`): already one constant; the module keeps it one.

## Revisit when

Another late-key bug appears (then do Phase C), or a `file_id` tool ships a second handle (then do
Phase B), or the `Tool` trait is reworked for another reason (then reconsider the `Draft` design).

## Evidence

Every count and line number above was read from the code on 2026-10-07: the 31 sites from a
`grep` of production files (tests excluded), the two room functions and `delivered_len` by reading
their bodies, the backstop's `output_id` gate by reading `clip_prebuffered_envelope`, and the three
carriers by grep. Not measured: how many of the 31 sites already pass a `Value` straight to the
predicate, so the exact size of the migration. Confidence: high on Phase A's boundary and
visibility wall, medium on the `_len` annotation check, low on Phase B (the cost is the clipped
`shown_lines`), medium on Phase C.
