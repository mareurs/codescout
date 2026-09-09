---
kind: bug
status: fixed
tags:
- cluster/guard-narrower-than-its-name
closed: 2026-09-09
opened: 2026-09-09
owner: marius
related: []
severity: medium
---

# BUG: the SAFETY gate checked one LINE where its name promised the CONSTRUCT

## Summary

`every_safety_comment_precedes_an_unsafe_construct` (`tests/safety_comments.rs`, shipped
in `6ac00158`) asserted that the **first code line** after a `// SAFETY:` comment contains
`unsafe`. Its name promises a claim about the *construct*. An `unsafe` block passed as an
argument to a multi-line macro call satisfies the name and fails the predicate, so the
gate red the shared build against correct code — and told its reader to delete a live
safety justification.

## Symptom (Effect)

From a full-gate run at 03:19Z, against a peer's untracked `src/agent/build_check.rs`:

```
test every_safety_comment_precedes_an_unsafe_construct ... FAILED
  src/agent/build_check.rs:667 -> next code line is 668: assert_eq!(
  src/agent/build_check.rs:711 -> next code line is 712: assert_eq!(
```

The code it accused:

```rust
// SAFETY: live fd owned by `holder`, unlocked below before it drops.
assert_eq!(
    unsafe { libc::flock(holder.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
    0
);
```

## Reproduction

At `2438a645`, revert `justified_statement` to break after the first code line, then
`cargo test --test safety_comments`.

## Root cause

**Two defects, and the second is the expensive one.**

**The predicate is narrower than the name.** `unsafe` is an expression in Rust, so it
appears wherever an expression can — including as a macro argument on a continuation
line. "The next code line" is a proxy for "the construct that follows", and the two
diverge exactly at multi-line expressions.

**The remedy text is confidently wrong for precisely the case the predicate misses.** The
failure said *"the note is about safe code — drop the `SAFETY:` prefix and write it as an
ordinary comment."* Following that deletes a correct justification for a live
`libc::flock`. `CLAUDE.md` § *Testing Discipline* says a suite tests a guard's predicate
and never its remedy text; this is the sharper form the peer who received it named — **the
reader most likely to comply is the one whose code is fine.**

**Why the validation missed it.** The rule was measured over 16 `// SAFETY:` comments at
tree `f10eefe2`: 15 pass, 1 fail, and I published *"zero false positives"*. That corpus
contained **no macro-wrapped `unsafe`**, so no member of it could falsify the rule in the
direction the rule was wrong. § *Testing Discipline*'s first law is usually read as being
about mutation direction; this is the population half — I checked that the rule caught the
defect and never that it spared a correct comment of a shape absent from the sample. The
first file outside the corpus falsified it within the hour, and it was outside the corpus
only because it was another session's untracked WIP.

## Evidence

The gate is not a window and the fix does not make it one: the span now runs from the
first code line to where its **delimiters balance** — an extent the code defines, with no
constant to raise. Preserved in the module header so a later "simplification" back to the
one-line form is refused by a reader rather than only by a test.

## Fix

`tests/safety_comments.rs` — `justified_statement()` accumulates lines until depth returns
to 0, and `code_only()` strips `//` tails and string literals so punctuation that is data
cannot extend the span. Char literals are deliberately unhandled (a bare `'` is more often
a lifetime); that residual runs toward a **false negative**, which is the acceptable
direction for a gate that reds a shared build.

Four mutations, four kills, each applied-verified before its run:

| mutation | killed |
|---|---|
| revert to first-line-only (v1) | `the_span_stops_at_the_statement_it_justifies` **and** `every_safety_comment_precedes_an_unsafe_construct` |
| remove the `depth <= 0` balance check | `the_span_stops_at_the_statement_it_justifies` |
| bypass `code_only` | `the_span_stops_at_the_statement_it_justifies` |
| restore the original `client.rs` orphan | `every_safety_comment_precedes_an_unsafe_construct` |

The first row is the useful one: reverting to v1 now reds the **corpus** test as well,
because the corpus has since gained a member that can falsify it. At `f10eefe2` that
mutation would have been survived silently.

## Tests added

`the_span_stops_at_the_statement_it_justifies` — five cases against the real
`justified_statement`, not a re-implementation. Case 4 (`unsafe` in a *later, separate*
statement must not count) is the one that keeps the fix honest: following the statement is
a **widening** of v1, and a widening needs a case that still fails or the repair for a
false positive quietly becomes a rule that accepts everything.

## Workarounds

None needed; fixed in tree.

## Resume

N/A.

## References

- `tests/safety_comments.rs`, `src/lsp/client.rs`
- `docs/issues/archive/2026-09-09-a-safety-comment-outlived-both-its-unsafe-block-and-its-own-rationale.md`
  — the bug this gate was built for; its "zero false positives" line is retracted there
- Reported by sessionId `5399543d-22d6-4ed9-9ebb-876be459989f`, whose file it accused
