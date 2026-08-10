---
specialist: architecture-snow-lion
scope: project
slug: tests-that-cannot-fail
created: 2026-08-08
updated: 2026-08-08
tags: [testing, verification, coverage, feature-flags, migration, contracts]
---

**Lesson:** A green suite is not evidence that a contract holds. Before trusting a
test as coverage, ask the separate question: **can this test fail?** In codescout,
three distinct mechanisms answer "no", and all three were found live on 2026-08-08 in
one afternoon, around one defect.

**Why:** The AST metadata header was computed for every chunk, carried nine dedicated
tests, and reached neither the embedder nor the payload — empty on 579,311 of 579,311
points in the live collection. Every test involved was passing. Three ways that
happened:

1. **Deleted with its consumer.** `embed_text_format_includes_metadata_prefix` asserted
   the embed text is `{metadata}\n{content}` *and not just content*. It lived in
   `src/embed/index.rs`, deleted in `66db4c70`. Delete a module, delete its tests — and
   the assertions that describe a contract *shared* with the replacement leave with the
   departing implementation. The producer's nine tests kept passing, because they assert
   the header is **produced**, never that it is **received**. The suite got greener as
   the feature died.
2. **Asserts a subset while its name claims the whole.**
   `payload_roundtrip_preserves_fields` checked 4 of 11 fields. `ast_header` could
   round-trip as garbage without failing anything. The name is what a reader greps for
   and trusts; the body is what runs.
3. **Compiled by no lane.** `server-stack` is declared in `Cargo.toml` but is in neither
   `default` nor any workflow, so every `#[cfg(feature = "server-stack")]` test is never
   built. A filtered-out test is indistinguishable from a test that does not exist — and
   the file *looks* covered, which is worse than an obvious hole.

**How to apply:**

- **Mutation-verify, always.** A test never seen to fail has an unknown failure mode.
  Reintroduce the defect, watch the specific test die, revert. Then check *which other
  tests stayed green* — that set is the measure of what the old suite was blind to. Here
  seven siblings passed while the guard failed, which is the whole argument in one line.
- **Assert on the CONSUMER's input, not the producer's output.** Unit tests on a producer
  cannot catch a value dropped two modules downstream. Concretely: the legacy path
  *stored* raw content while *embedding* header+content, so checking stored content would
  have "confirmed" correct behaviour in both the working and broken worlds. Only what the
  embedder received discriminates.
- **When retiring an implementation, triage its tests before deleting.** Sort into
  *implementation detail* (goes) and *shared contract* (migrate first, against the
  replacement). This is the strangler-fig failure mode: not that the new design is wrong,
  but that the old system's written-down requirements are deleted before anyone asks the
  new one to satisfy them.
- **Check the gate before trusting the gate.** Before citing a test as coverage, confirm
  it runs in a lane: `cargo test --test <file>` and look for the name in the output. A
  `#[cfg(feature = ...)]` on a test is a claim that some lane enables that feature —
  verify it (`grep -rn '<feature>' .github/workflows/`) rather than assume.
- **A name that claims more than the body asserts is a defect.** When touching such a
  test, make the name true rather than narrowing it — the name is the interface.

Related: [[cross-cutting-side-effects-at-the-chokepoint]] — same root shape, that a
value's *entry points* must be enumerated rather than assumed from the one in front of
you. There it was `references()` on callers; here it is "who consumes this?" on a field.
