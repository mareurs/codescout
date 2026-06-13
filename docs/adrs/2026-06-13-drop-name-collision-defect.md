# ADR-2026-06-13 — Drop the `name_collision` Legibility Defect

## Status

Accepted — implemented on `experiments` 2026-06-13 (commit `919dbe5c`). The
user-facing remediation for the underlying friction shipped first as the
resolver-hint fix `c21ad73b`. Supersedes the `name_collision` half of the
legibility engine introduced in Phase 2a.

## Context

The legibility engine (`src/legibility/mod.rs`) ranks refactor candidates from
two lanes: an **index lane** (structural defects parsed from the tree-sitter
AST) and a **recorder lane** (observed `usage.db` friction). The index lane
emitted three structural defects: `over_budget_body`, `name_collision`,
`un_mappable_file`. A candidate needs a structural defect to enter the backlog.

`name_collision` flagged a `name_path` that resolved to more than one symbol
within a file — its stated purpose: *"the ambiguity that hard-fails
`edit_code`."* Dogfooding the engine on codescout produced a 19-row
`name_collision` cluster (e.g. `LspClient/hover`, `SensitiveString/fmt`,
`BookMetadata`). Two refactors (`b946171d`, `2b35f2a1`) "fixed" some by
relocating trait impls to their own files. Investigating whether that was the
right fix exposed three facts that invalidate the defect:

1. **`edit_code` does not hard-fail on these.** It resolves a colliding sibling
   by the qualified form `impl Trait for Type/method`. Verified live:
   `find_unique_symbol_by_name_path("SensitiveString/fmt")` is ambiguous, but
   `"impl fmt::Display for SensitiveString/fmt"` resolves to exactly one symbol.
   The capability always existed; only the ambiguity error's *hint* misdirected
   (fixed in `c21ad73b`). So the relocations were unnecessary — the body could
   have been edited in place via the qualified form.

2. **The disambiguator is per-language; the AST this scanner reads discards it.**
   The collision dimension differs by language:
   - **Rust** — the trait/impl context (`impl Debug for S` vs `impl Display for S`).
   - **TypeScript** — the declaration kind (`interface BookMetadata` +
     `namespace BookMetadata`), which is **declaration merging — idiomatic, not a
     defect at all.**
   - **Java/Kotlin** — the parameter-type signature (overloading: `foo(int)` vs
     `foo(String)`).
   The tree-sitter parser collapses all of these (`src/ast/parser.rs:236` keys an
   impl's prefix on the *type only*, dropping the trait), so the AST cannot tell
   an addressable collision from an unaddressable one — and for TS cannot tell a
   defect from idiomatic code.

3. **The LSP already disambiguates per-language for free**, and `edit_code` rides
   on it. So the one cross-language source of truth that resolves these is the
   LSP — which the batch scan deliberately avoids using (tree-sitter is chosen to
   stay fast and stateless; pulling in language servers imports cold-start cost
   and statefulness).

The other two defects are genuinely language-agnostic: `over_budget_body`
(token count) and `un_mappable_file` (overview size) need zero language
semantics. `name_collision` is the **outlier** — the only defect requiring
per-language semantic judgment the AST cannot supply.

## Decision

**Remove `name_collision` from the legibility engine.** The engine emits only
language-agnostic, AST-measurable defects: `over_budget_body` and
`un_mappable_file`.

The underlying friction (a caller using the bare, ambiguous `name_path`) is
real but is **not a code-refactor problem** — its remediation is the qualified
`impl Trait for Type/method` form, surfaced by the ambiguity error's improved
hint (`c21ad73b`). The recorder lane's `ambiguous_name_path` friction family is
retained as `usage.db` analytics but no longer feeds a backlog candidate.

This is *language-agnostic by subtraction*: rather than teach the scanner each
language's disambiguator (impossible cheaply, and wrong for TS), the engine
sheds the one signal it cannot measure honestly across languages.

## Consequences

### Now easier

- The engine is honest across every language by construction — it only measures
  bytes and lines, which mean the same thing in Rust, TS, Python, Java, Kotlin.
- No more backlog rows tempting a reader to relocate idiomatic code (trait impls,
  TS namespace merges) to satisfy a per-file AST heuristic.

### Now harder / lost

- The bare-`name_path` ambiguity no longer has a backlog surface. Mitigation: the
  improved hint makes it self-correcting at the point of failure, and the
  `ambiguous_name_path` recorder family still records it for analytics.

### Change scenarios absorbed

- A new language is indexed → it inherits correct behavior with no per-language
  collision logic to write or get wrong.
- A TypeScript file uses declaration merging → never flagged as a defect.

### Migration

The 19 live `name_collision` rows auto-close on the next scan (absent from the
index lane). They closed because the **detector was retired, not because the code
was refactored** — recorded in the backlog verdicts; their before→after deltas
are not meaningful and render as "structural".

### Revisit-when

- The legibility scan ever moves to an LSP-backed index lane (accepting the
  cost) → per-language disambiguation becomes available for free, and a
  *demoted* collision nudge could return as a non-ranking signal. Not worth the
  statefulness today.

**Confidence: high** that `name_collision` is mis-placed in an AST-based,
language-agnostic scanner; the three concretes (Rust trait, TS merge, Java
overload) and the live qualified-form resolution are the grounding.

## Alternatives considered

1. **Wall A — preserve the disambiguator in the AST `name_path`** (e.g.
   `impl Display for SensitiveString/fmt`). Rejected — it is *N per-language
   parser changes*, each needing language expertise, some producing awkward
   paths (Java signatures), and for TS declaration merging there is no defect to
   preserve. Not language-agnostic; the opposite.

2. **Feed the detector the LSP-qualified view instead of the AST.** Rejected for
   now — the LSP disambiguates per-language for free, but standing up language
   servers in a batch sweep imports cold-start (kotlin 30–60s) and statefulness,
   defeating the index lane's reason to exist. This is the `revisit-when`.

3. **Detector heuristic — down-rank "same name_path in distinct impl blocks".**
   Rejected — still needs the per-language structure the AST dropped, so it
   re-derives the same information loss with a fragile proxy.

4. **Keep flagging but demote to a non-ranking nudge.** Considered as the lighter
   move; rejected in favor of full removal because the signal's correctness is
   per-language and the hint already remediates at the point of failure. A demote
   can return cheaply under alternative 2 if ever warranted.

## Related

- `c21ad73b` — resolver-hint fix (the user-facing replacement): the ambiguity
  error now points at the qualified `impl Trait for Type/method` form it lists.
- `919dbe5c` — this removal (engine + tests + guard
  `index_lane_does_not_flag_name_collisions`).
- `b946171d`, `2b35f2a1` — the two trait-impl relocations, now known to have been
  unnecessary; kept (idiomatic, harmless), not reverted.
- Code: `src/legibility/mod.rs` (`Defect`, `index_lane`),
  `src/ast/parser.rs:236` (the impl-prefix collapse that drops the trait),
  `src/symbol/query.rs:610` (`find_unique_symbol_by_name_path` — the resolver
  that already disambiguates the qualified form).
- Concretes: `src/config/sensitive.rs` (Rust `Debug`/`Display` `fmt`),
  `tests/fixtures/typescript-library/src/extensions/advanced.ts` (`BookMetadata`
  interface+namespace merge).
