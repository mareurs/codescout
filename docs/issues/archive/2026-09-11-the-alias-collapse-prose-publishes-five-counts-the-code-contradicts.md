---
kind: bug
status: fixed
tags:
- cluster/doc-contradicted-by-code
closed: 2026-09-11
opened: 2026-09-11
owner: marius
related: []
severity: low
---

# BUG: the alias-collapse's own prose publishes five counts the shipped code contradicts, including the one the run named as its worst defect

## Summary

The parameter-alias collapse identified its worst defect as *"all three render paths"* when there
were four, and wrote *"a count published as a scope is not a scope that was verified"* into a gate
comment. The governing ADR still says **three**, in the Consequences paragraph **and** in the
Revisit-when trigger. Four further counts in the same work stream are also wrong against the code
they describe, one of them falsified by a single live tool call. None is load-bearing for a gate,
which is exactly why all five survived.

## Symptom (Effect)

Five claims, each false against `a8e8a91a`:

1. `docs/adrs/2026-07-10-repair-and-continue-input-handling.md` — *"it must be threaded through
   **all three** of `call_content`'s render paths"*, and in Revisit-when, *"it means one of the
   three sites regressed"*. The plan's own Execution Record opens with
   *"'All three render paths' was wrong — there are four."*
2. Same file — *"five of the eight alias-declaring tools (`read_file`, `grep`, `references`,
   `symbol_at`, `call_graph` — every alias-declaring tool whose `output_form()` is
   `OutputForm::Text`; only `create_file`, `edit_file` and `edit_code` are `OutputForm::Json`)"*.
   Derived below: **six of ten**, and both omissions are tools this work stream added.
3. Same file, *"Scope of amendment 2 — path family only. Deliberately excludes: `symbols`'
   `name`/`query` and `name_path`/`symbol` … Revisit if a `usage.db` split on **those four
   names** …"*. Commit `8b396343` collapsed exactly those four names. The exclusion and its
   revisit condition describe a surface that no longer exists.
4. `src/server.rs` — `EXPECTED_ALIAS_PAIR_COUNTS_BY_TOOL`'s doc comment ends
   *"4\*3 + 2\*5 + 1\*5 + 1\*4 + 2\*2 = 37."* and `EXPECTED_ALIAS_PAIRS`' doc comment says
   *"the same population `EXPECTED_ALIAS_PAIR_COUNTS_BY_TOOL` counts (37 pairs, 10 tools)"*. The
   terms are right and the result is not: the expression evaluates to **35**, the per-tool table
   sums to **35**, and `EXPECTED_ALIAS_PAIRS` holds **35** rows.
5. `src/tools/core/types.rs` — `merge_param_corrections`' doc comment: *"Verified zero today, not
   assumed zero: ten tools implement `param_aliases()` (… `doc`, `symbols`) and **none writes its
   own `corrections`**; `corrections` is written only by `find.rs` and `update.rs`, **both under
   `doc`**."* The two halves contradict each other inside one sentence, and the live tool agrees
   with the second: `doc` both declares aliases and writes its own `corrections`. The consequence
   is a second false claim downstream, in `call_content`'s small-output branch — *"The `None` arm
   is the live one on every ordinary alias-repair call …; the other two have no in-tree production
   caller today"* — which tells the next reader the object arm is unreached, and is why nobody
   would think to test it.

## Reproduction

At `a8e8a91a`:

```
$ awk '/const EXPECTED_ALIAS_PAIRS/,/^    \];/' src/server.rs | grep -c '^        ("'
35
```

For claim 2, enumerate the population from the code rather than the prose — ten tools override
`param_aliases()` with a non-empty map (`create_file`, `edit_file`, `grep`, `read_file`,
`edit_code`, `references`, `symbol_at`, `call_graph`, `symbols`, and `doc` via
`src/librarian/adapter.rs`), and every `output_form()` override returns `OutputForm::Text`:

```
$ grep -rn "fn output_form" src/ --include=*.rs | grep -v "core/types.rs"
```

returns `grep`, `read_file`, `symbol_at`, `references`, `symbols`, `call_graph` (plus `tree`,
`library`, `memory`, which declare no aliases). `OutputForm::Json` is the trait default, so the
Json half is `create_file`, `edit_file`, `edit_code`, `doc`.

For claim 5, one live MCP call against the running server —
`doc(action="find", kind="tracker", query="alias", rel_path="docs/trackers", limit=2)` — returns

```
"corrections": {
  "filter": ["top-level `rel_path` lifted into the filter: ..."],
  "hint": "`rel_path` is a create-time param; on find it was read as a filter clause. ...",
  "param_aliases": {
    "params": [{"received": "query", "canonical": "semantic", "conflicted": false, "superseded_by": null}],
    "hint": "'query' is not a parameter of doc \u2014 corrected to 'semantic'. Use 'semantic' next time."
  }
}
```

i.e. `merge_param_corrections`' **object arm** firing in production. No data is lost — the nested
design is exactly what makes it safe — but the claim used to justify trusting the key is false.

## Environment

Branch `experiments` at `a8e8a91a`. Documentation and doc comments only; no runtime behaviour.

## Root cause

Not a mechanism in code — a mechanism in review. Each of the five claims is **decoration**: no
assertion reads any of them. `every_declared_alias_pair_normalizes_to_its_own_canonical` compares
`EXPECTED_ALIAS_PAIRS.len()` against the sum of the per-tool table, both derived from the arrays,
so the prose `37` can be any integer and the gate stays green. The ADR has no gate at all.

Claim 1 is the sharper case: the number moved because the code moved (a fourth render path was
added by `295a928e`), and claim 3 the same way (`8b396343` collapsed a surface the ADR had
excluded). The document and the code have independent rates of change and nothing couples them,
which is this class's signature — no authoring error to find.

Measured 2026-09-11: counts re-derived from `src/server.rs`, `src/librarian/adapter.rs` and each
tool's `output_form()` at `a8e8a91a`, not transcribed from any report.

## Evidence

The plan's Execution Record already states claim 1's correction and names the residue, and the
gate `the_dispatch_boundary_normalizes_and_announces_for_real_tool_calls` carries a whole
paragraph on it ending *"A count published as a scope is not a scope that was verified, so this
gate states its own scope rather than leaving a reader to infer it."* That paragraph was added to
a **test**; the ADR the plan names as *"the binding authority. Read the ADR amendment first."* was
not amended.

## Hypotheses tried

1. **Hypothesis** — 37 is the count under some other feature lane. **Test** — the lean lane has
   *fewer* rows (`doc` is `#[cfg(feature = "librarian")]`, so 33, filtered by
   `alias_table_row_is_registered_here`); nothing yields 37. **Verdict:** rejected.
2. **Hypothesis** — the ADR's "eight tools" predates `doc` and `symbols` and is simply historical
   prose. **Test** — it is written in the present tense inside the Round-3 implementation note,
   and its parenthetical enumerates the live `output_form()` split as fact. **Verdict:** rejected;
   it reads as a current claim.

## Fix

**Applied, 2026-09-11, commit `6d5dba0f709298c8ffdc8717001b1e4549b8a290` (patch-id
`1d17d3cc62c07985ee5a4b2e9a0e2c35b2f24f82`).** All six text edits, no behaviour change. Every
number was re-derived from the tree rather than copied out of this file — and two of the five
claims needed a shape, not a replacement digit:

- **ADR Consequences** — three → **four** render paths, and now ENUMERATED (buffered envelope,
  `OutputForm::Text` compact render, pretty-JSON value, error path) rather than totalled, with the
  derivation named: every consumer of `param_corrections` in `src/tools/core/types.rs`. The
  pretty-JSON entry says explicitly that it is ONE site serving TWO emitters, because that is where
  a reader re-deriving from branch arms gets five instead of four. Plus the shape the flat count
  hid: path 4 holds **three outcomes across two shapes**, and only the shape count bounds the
  advisory's addresses.
- **ADR Revisit-when** — the load-bearing one. Not "three → four": a fixed list is exactly what
  misled twice. It now opens *"do not read this trigger's own count as the search space"*, cites
  both times the trigger fired at a site the then-current count did not contain (`295a928e` at
  three, `4629a95b` at four), and instructs re-derivation from two named surfaces first.
- **ADR Round-3 note** — five-of-eight → **six of ten**, with both halves derived in place: the ten
  `param_aliases()` overrides, and the rule that `OutputForm::Json` is the trait default while every
  `output_form()` override in the tree returns `Text`, so the split is exactly "does it override".
  Names `tree`/`library`/`memory` as the three Text overrides that declare no aliases, since an
  `output_form()` grep alone over-counts that side.
- **ADR amendment-2 scope** — dated `OVERTAKEN 2026-09-11 by 8b396343` and kept, per the brief and
  per CLAUDE.md's test: a reader would act wrongly without it, because it is the only thing
  explaining why a decision recorded there looks contradicted by shipped code. It also records that
  the collapse was funded by a finding the exclusion's own ambiguity-cost argument never
  contemplated — the transferable lesson for the two exclusions still standing.
- **`src/server.rs`** — `= 37.` → `= 35.`, `(37 pairs, 10 tools)` → `(35 pairs, 10 tools)`. Origin
  found: `8b396343`'s own commit message reported its change as *"35 -> 37"* when the pre-collapse
  population was **33 over nine tools**, and both numbers were transcribed. The comment now states
  why it survived — `every_declared_alias_pair_normalizes_to_its_own_canonical` compares the two
  ARRAYS and never the prose, so the sentence can hold any integer and the gate stays green.
- **`src/tools/core/types.rs`** — `merge_param_corrections`' self-contradicting sentence replaced
  with the derived fact (`doc` is the one tool that both declares aliases and writes its own
  `corrections`, so the object arm always had a live production caller), and `call_content`'s
  small-output comment, which told the next reader the opposite, corrected. That comment now names
  the consequence as a **live gap**: the object arm has no test because the false claim left nobody
  a reason to write one.

**On history:** the superseded counts are deleted, not annotated — `git log` holds them. The one
exception is the `8b396343` exclusion, dated and kept for the reason above.
## Tests added

None, and the reason is unchanged: a gate pinning a number written in prose reds on every
rewording, which this repo deliberately avoids. The affordable half already exists for
`EXPECTED_ALIAS_PAIRS` (its length asserted against the per-tool table) and simply does not read
the comment — which the comment now says out loud, so the next reader is not left to infer that
the prose is covered.

One gap this investigation surfaced and did NOT close, recorded so it is not lost:
`merge_param_corrections`' **object arm** has no test, despite firing in production on every
`doc(action="find")` call that carries both an alias and a lifted top-level param. It was believed
unreached because of claim 5, which is why no one wrote one. Named at the site.
## Workarounds

Derive any of these counts from the code before citing it. The commands are in *Reproduction*.

## Resume

Apply the five text edits listed under *Fix*, then re-run
`cargo test --lib every_declared_alias_pair_normalizes_to_its_own_canonical` to confirm nothing
depended on the prose.

## References

- `docs/adrs/2026-07-10-repair-and-continue-input-handling.md` § Amendment 2026-09-10
- `docs/superpowers/plans/2026-09-10-parameter-alias-collapse.md` § Execution Record
- `src/server.rs` — `EXPECTED_ALIAS_PAIR_COUNTS_BY_TOOL`, `EXPECTED_ALIAS_PAIRS`
- `src/librarian/adapter.rs` — `doc`'s alias map, the tenth tool
