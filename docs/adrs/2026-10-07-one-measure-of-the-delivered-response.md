---
id: dbc617677d44cf8a
kind: adr
status: active
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

Accepted, Phase A built. Written 2026-10-07 after the byte-bound sibling sweep
(`docs/issues/archive/2026-10-06-sibling-sweep-the-byte-bound-defect-recurred-across-tools.md`),
whose Resume section asks for this. Phase A landed on `experiments` with the gate green on its
integration tip. Phases B and C are not decided; see Built for what Phase A does and does not do.

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

**Phase A: one module owns "the response as delivered".** A new `src/tools/core/response_fit.rs` holds:

- `delivered_len(&Value)`: stays in `types.rs` beside the cut record it knows about; the module uses it;
- `response_fits(&Value) -> bool`: the serialized, as-delivered response is within the limit;
- `response_room(widest: &Value) -> usize`: the limit less `delivered_len(widest)`, which replaces
  both existing room functions and gives them the cut-record rule they lack;
- `body_alone_overflows(raw: &str) -> bool`: true only when the raw body ALONE is over the limit.
  Escaping never shortens text, so `true` proves the response is over. It answers one direction
  only, so it cannot be used as a "fits" gate; this is the asymmetry that replaces the raw
  gates. Five of the six raw checks in `read_file` and `read_markdown` were pure shortcuts; the
  sixth, in `read_full_file`, is a policy gate (in exploring mode the inline arm returns the first
  page, so the check is what sends an over-limit file to a summary with a handle).

The text predicate `exceeds_inline_limit(&str)` becomes private to `core` (`call_content` and the
backstop use it on a string they have already serialized). Code outside `core` can no longer write
a raw-body gate: the call does not compile. The 29 sites outside `core` that still used it migrated; `memory` and `file_summary` had moved first.

`exceeds_inline_limit_len(n)` stays callable from `run_command` and `command_summary`, which
compute a length from parts. Each such site (2 today) carries a `// inline-gate: <what unit n is>`
annotation, and `tests/inline_gates.rs` fails a production use without one.

**Phase B (not decided here): extend the backstop to `file_id` envelopes**, so a mis-sized arm still
cannot mint a second handle. Open question: clipping a page's `content` makes its `shown_lines` and
`next` claim lines the caller did not see. It would be marked, not silent, but it is data the caller
must notice is missing.

**Phase C (decided against on 2026-10-07, after measuring): one carrier for keys the gate must
count.** The case for it was that three tools had each built the same carrier (`LateKeys`, `extra`,
`noted`). The survey (section Survey below) found they do not share an interface: one prices a
length delta and can prefix text into `stdout`, one inserts keys into the candidate and overwrites,
one is a closure applied inside the widest skeleton; and they are built at different times (after
the child exits, before the gate, per arm). Unifying them would have fixed none of the three
dangerous sites the survey found. The cure is the rule recorded under Built (every fallback arm is
measured or statically bounded), applied site by site.

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

Another late-key bug appears in a shape the survey did not cover (then re-survey; a unified
carrier needs a shared interface first), or a `file_id` tool ships a second handle (then do
Phase B), or the `Tool` trait is reworked for another reason (then reconsider the `Draft` design).

## Evidence

Every count and line number above was read from the code on 2026-10-07: the 31 sites from a
`grep` of production files (tests excluded), the two room functions and `delivered_len` by reading
their bodies, the backstop's `output_id` gate by reading `clip_prebuffered_envelope`, and the three
carriers by grep. Not measured: how many of the 31 sites already pass a `Value` straight to the
predicate, so the exact size of the migration. Confidence: high on Phase A's boundary and
visibility wall, medium on the `_len` annotation check, low on Phase B (the cost is the clipped
`shown_lines`), medium on Phase C.

## Built

Phase A, landed 2026-10-07: the module with `response_fits`, `response_room` and
`body_alone_overflows`; 31 production gates moved onto it; the text predicate private to `core` in
production (tests keep it for delivered text); `buffer_page_room` removed; `tests/inline_gates.rs`
enforcing the annotation rule and forbidding the text form outside `core`. A production call of the
text predicate outside `core` now fails to compile, and a deleted annotation fails the source
check; both were shown by mutating them. Gate on the integration tip: fmt, clippy, lean (4,168
tests) and default (6,426 tests) green.

Found while building it, and fixed: `compacted_test_value` replaced the `failures` and `stderr`
fields but kept the cut record naming them, so the compaction gate counted about 25 bytes the
caller never receives and a run in that window was summarized where it should be compacted. Three
gates had no test that failed when their measure was swapped for the raw body (the multi-heading
error, the `coverage` drop and the `siblings` drop); each now has one.

What Phase A does NOT cover, so nobody reads the wall as wider than it is:

- **Late keys (root 2).** A key added after a gate decided is still the tool's job.
- **`response_fits(&json!(content))`** measures the body alone and brings the old defect back. Types
  cannot stop it; review and a mutation per arm must.
- **A length-form function used as a pointer** is not checked by the annotation rule, and
  `run_command/output.rs` compares `raw_stdout.len() + raw_stderr.len()` to the limit directly as a
  compaction filter. The source check does not see it.
- **`over_budget_bodies`** (`legibility`) flags a symbol body only when its raw text alone is over
  the limit; an escape-heavy body just under it is not flagged although `symbols` would overflow.
- The source check cuts production from test code by test-gated item, not at the first test module:
  five files (`librarian/frontmatter.rs`, `librarian/mod.rs`, `audit_doc_refs/parser.rs`,
  `retrieval/embedder.rs`, `symbol/call_graph/mod.rs`) carry production code after an inline test
  module. `tests/result_caps.rs` scans whole files, test modules included, so it makes no such cut.

## Survey of late keys (2026-10-07)

A read-mostly survey enumerated every site in the tools that return their own handle (`read_file`
and its markdown arms, `memory`, `run_command`) and in `call_content` where a key or text is added
after the gate decided, and drove the uncounted ones through the real tool. 22 sites: 9 counted by
the gate already, 13 uncounted and harmless (they overshoot 10,003 bytes without minting a handle,
which this ADR accepts: `_guide_hint`, `_workspace_notice`, the alias prefix, `corrections`), and 3
uncounted and dangerous:

- `memory`'s `missing` list in the `file_id` fallback arm, which was never measured and is about 87
  bytes wider than the inline candidate it replaces: two handles from a 9,948-byte section name.
- `run_command`'s pending-ack and timeout shapes, which have no gate: an unbounded `jobs` list
  pushes them behind `@tool_*`, and the `@ack_*` handle the caller must act on is visible only
  inside that buffer.
- `memory`'s `extra` keys, re-applied to the fallback arm after it is built (latent, not reached).

The common failure is a fallback arm or response shape that nothing measures, not a missing
carrier. Rule: every arm that can be returned either passes `response_fits` or has a statically
bounded shape, and a test per arm says which. The survey also found that a counted key is not a
bounded one: `jobs` carries each job's full command, so the gate counted it and the response was
still over the limit.

Fixed the same day: the `memory` fallback arm is built with every key, measured, and falls back to
dropping `missing` and clipping the layout paths, with `extra` applied once before the measure; the
`run_command` shapes are measured in `RunCommand::call`, `jobs` is bounded (8 jobs, 300 escaped
bytes a command, a count and a route for the rest), and envelope keys are shed in a fixed order
when a shape is still over. Left open and recorded in the sweep record: the interactive arm, the
pending-ack `reason` echo of a custom pattern, the unbounded `buffer_truncated` count, and the
unmeasured Text render of a `memory` read.
