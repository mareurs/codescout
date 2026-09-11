---
kind: bug
status: fixed
tags:
- cluster/declared-not-wired
claimed_at: 2026-09-11
claimed_by: f3c594ce-c424-40d3-a603-9693cfef3f63
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

Fixed — **delete**, not move. The `ok_or_else` refusal was removed and the resolution chain made
infallible via `.expect("has_name_arg above guarantees name or symbol resolves to a string")`,
with a comment at the site deriving why: `has_name_arg` (checked two lines above) and this
resolution read the identical predicate (`serde_json::Value::is_string` is `self.as_str().is_some()`
by definition), so if `has_name_arg` is true one of the two `as_str()` calls is provably `Some`.

**Why delete over move**, weighed explicitly rather than defaulted to: the "move" alternative
existed to make a caller sending a wrong/stale name-ish key (the smoke-scripts shape) fail loudly
instead of silently getting an overview. That actual failure mode is now caught by a different,
more general mechanism already shipped this session —
`tests/mcp_smoke_scripts_reference_real_tools.rs` (`9406f3c4`), a static cross-check of every
`call <tool>` invocation against registered tool names/params. Building a NEW heuristic here for
"a name-ish key that failed to resolve" would mean inventing an unspecified predicate (which keys
count as "name-ish"?) for a case the corpus no longer has an open, motivating instance of — exactly
the kind of speculative complexity this repo's own conventions argue against. Delete removes
unreachable, untested residue and changes nothing observable.

**Verified, not assumed:** `cargo build --lib` clean (no unused-import warning for
`RecoverableError`, still used elsewhere in the file), `cargo clippy --workspace --all-targets
--features local-embed -- -D warnings` clean, and `cargo test --no-default-features --lib
tools::symbol::` reports the SAME 344-test pass count before and after — the honest confirmation
for a deletion, per this bug's own "Tests added: None" reasoning: there is nothing new to assert
about a branch that no longer exists, so the evidence is behavioral parity, not a new red/green.

**SHA:** `3863055e4e4e3eeebfa8860e848a96770c15dc19`
**patch-id:** `cd09e2934bb8b723c24cc7d7849e03acdee527fd`
## Tests added

None added — matches this bug's own reasoning under § Fix. Confirmed via the full
`tools::symbol::` suite (344 tests, unchanged pass count) and the full workspace gate
(fmt-mine, clippy, both test lanes), not a new assertion.
## Workarounds

N/A — no caller is blocked. The cost is that a wrong call returns a plausible answer.

## Resume

Done — see § Fix. Nothing left to resume.
## References

- `src/tools/symbol/symbols.rs` — `Symbols::call`, `has_name_arg` and the resolution chain
- `src/tools/symbol/tests.rs` — `a_raw_alias_key_reaching_call_directly_is_not_a_name_argument`
- `docs/issues/archive/2026-09-11-mcp-smoke-scripts-call-a-parameter-and-a-tool-that-do-not-exist.md`
- `docs/superpowers/plans/2026-09-10-parameter-alias-collapse.md` § Execution Record, item 2
