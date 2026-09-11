---
kind: bug
status: open
tags:
- cluster/declared-not-wired
closed: null
opened: 2026-09-11
owner: marius
related: []
severity: low
---

# BUG: the `symbols` no-name refusal is unreachable — the branch that would tell a caller their name key was dropped can never execute

## Summary

`symbols`' `call()` builds a `RecoverableError` naming the keys the caller actually sent
(*"missing 'name' or 'symbol' parameter (received keys: …)"*) for the case where neither name
argument resolves. That case is decided two lines earlier by `has_name_arg`, which dispatches to
the path-overview branch instead — and the two predicates are byte-equivalent, so the refusal can
never run. A caller who sends no resolvable name gets a **directory listing**, which is how
`assert_symbols_found` in the MCP smoke scripts went green against scenery
(`docs/issues/archive/2026-09-11-mcp-smoke-scripts-call-a-parameter-and-a-tool-that-do-not-exist.md`).

## Symptom (Effect)

No observable symptom, and that is the defect: the alarm is unreachable. A call whose name key
does not resolve returns a well-formed overview response instead of

```
missing 'name' or 'symbol' parameter (received keys: …)
```

which no caller has ever seen.

## Reproduction

At `a8e8a91a`, in a scratch copy of the tree, plant a `panic!` as the first statement of the
`.ok_or_else(…)` closure in `src/tools/symbol/symbols.rs` and run the whole lean library suite:

```
$ cargo test --lib --no-default-features
test result: ok. 3480 passed; 0 failed; 7 ignored; 0 measured; 0 filtered out
```

Positive control — plant the same `panic!` two lines further down, at the top of the search branch
that `has_name_arg == true` reaches:

```
test result: FAILED. 3448 passed; 32 failed; 7 ignored; 0 measured; 0 filtered out
```

32 reds. So the suite reaches `symbols`' search branch repeatedly and reaches the refusal zero
times; without the control the first run would be indistinguishable from a suite that never calls
`symbols` at all.

## Environment

Branch `experiments` at `a8e8a91a`, `serde_json` 1.0.149. Both feature lanes — nothing here is
feature-gated.

## Root cause

In `src/tools/symbol/symbols.rs`, `call()` computes

```
let has_name_arg = input["name"].is_string() || input["symbol"].is_string();
if !has_name_arg { return list_overview(input, ctx).await; }
```

and then resolves the pattern with
`input["name"].as_str().map(...).or_else(|| input["symbol"].as_str().map(...)).ok_or_else(...)`.

`serde_json`'s `Value::is_string` is literally `self.as_str().is_some()`
(`value/mod.rs:465-467` in 1.0.149). So `has_name_arg == true` implies at least one of the two
`as_str()` calls yields `Some`, and the `ok_or_else` arm is unreachable; `has_name_arg == false`
has already returned. The two predicates are the same predicate, written twice.

This is the intended consequence of the 4→2 collapse (`8b396343`) — `param_aliases()` guarantees
`call()` can only observe `name`/`symbol`, and the tool deliberately keeps the no-name path as an
overview dispatch. What did not follow is deleting the refusal the collapse made dead, or moving
it above the dispatch so a caller who *meant* a search still gets told.

Measured 2026-09-11 by the two probe runs above; the `is_string` equivalence read from the
vendored `serde_json` source, not inferred.

## Evidence

The suite's own treatment of the surrounding behaviour is correct and deliberate:
`a_raw_alias_key_reaching_call_directly_is_not_a_name_argument` in `src/tools/symbol/tests.rs`
asserts that a raw `query`/`name_path` reaching `call()` **must** fall through to the unfiltered
overview, and carries a positive control (`name` with a pattern matching nothing returns nothing).
So no test is passing against scenery here — the dead refusal is residue, not a hidden hole in the
tests.

## Hypotheses tried

1. **Hypothesis** — a non-object `input` reaches the refusal. **Test** — `input["name"]` on a
   non-object `Value` indexes to `Value::Null`, so `has_name_arg` is false and the overview
   branch returns first. **Verdict:** rejected.
2. **Hypothesis** — some caller invokes the resolution chain without the `has_name_arg` guard.
   **Test** — the chain is local to `call()` and sits directly below the guard. **Verdict:**
   rejected.

## Fix

Not fixed. Two directions, and they differ in behaviour, so this is a decision rather than a
cleanup:

- **Delete** the `ok_or_else` arm and make the resolution infallible (`expect`-free, e.g. by
  restructuring the guard to produce the pair). Keeps today's behaviour: no name → overview.
- **Move** the refusal above the dispatch for the shape that is genuinely a mistake — a call
  carrying a name-ish key the tool does not read. That would have made the smoke-script defect
  loud instead of green, which is the argument for it; it also changes a currently-useful
  behaviour (`symbols(path=…)` alone is the documented overview call) so the predicate has to
  distinguish "no name key at all" from "a name key that did not resolve".

Per CLAUDE.md § Testing Discipline: when adding a guard, name the caller that reaches it and the
observer who acts on it. The current one has neither.

## Tests added

None. A regression test for the *deletion* direction is not writable (there is nothing to assert
about a branch that no longer exists); a test for the *move* direction is the natural artefact of
choosing it.

## Workarounds

N/A — no caller is blocked. The cost is that a wrong call returns a plausible answer.

## Resume

Decide delete-vs-move with the smoke-script defect as the motivating case
(`docs/issues/archive/2026-09-11-mcp-smoke-scripts-call-a-parameter-and-a-tool-that-do-not-exist.md`),
then re-run the probe from *Reproduction* with its control to confirm the chosen branch is
reachable or gone.

## References

- `src/tools/symbol/symbols.rs` — `Symbols::call`, `has_name_arg` and the resolution chain
- `src/tools/symbol/tests.rs` — `a_raw_alias_key_reaching_call_directly_is_not_a_name_argument`
- `docs/issues/archive/2026-09-11-mcp-smoke-scripts-call-a-parameter-and-a-tool-that-do-not-exist.md`
- `docs/superpowers/plans/2026-09-10-parameter-alias-collapse.md` § Execution Record, item 2
