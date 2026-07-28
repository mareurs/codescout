---
id: null
kind: bug
status: fixed
title: null
owners: []
tags:
- librarian
- constitution-tracker
topic: null
time_scope: null
closed: '2026-07-06'
opened: '2026-07-06'
owner: marius
related: []
severity: medium
---

# BUG: A malformed glob in a constitution rule's `paths` silently disables that rule

## Summary
`find_matching_rules` (`src/librarian/tools/constitution_check.rs`) drops
any `paths` glob pattern that fails to parse, and treats a build failure
or non-match as "rule doesn't apply" rather than surfacing a diagnostic.
For a tracker type whose whole purpose is "rules the agent must follow no
matter what," a typo'd glob causes the rule to be silently unenforced —
the wrong failure direction for a must-follow system.

## Symptom (Effect)
Given a constitution rule with `"paths": ["**soler/**"]` (typo: `soler`
instead of `solver`), or any pattern `globset::Glob::new` rejects, the rule
never matches any path and is never surfaced by
`find_matching_rules`/the PreToolUse hook — with no error, warning, or log
line anywhere. The rule is effectively dead, indistinguishable from a rule
that correctly doesn't apply to the current path.

## Reproduction
```rust
// In a constitution-tagged tracker's params.rules:
// {"id": "C-1", "paths": ["[invalid"], "title": "T", "rule": "R", "status": "active"}
find_matching_rules(&cat, "src/anything.kt")
// -> Ok(vec![]) — no error, no indication C-1's glob was invalid.
```

## Environment
codescout, `src/librarian/tools/constitution_check.rs`, introduced in
`docs/superpowers/plans/2026-07-06-constitution-tracker-archetype-and-cli.md`
Task 2 (commits 06946ae3, cda47a44).

## Root cause
```rust
for p in pattern_strs {
    if let Some(s) = p.as_str() {
        if let Ok(g) = globset::Glob::new(s) {
            builder.add(g);
        }
        // else: silently dropped
    }
}
let is_match = builder
    .build()
    .map(|set| set.is_match(target))
    .unwrap_or(false); // build() failure also silently -> false
```
Both the per-glob parse failure and the whole-`GlobSetBuilder::build()`
failure degrade to "no match," which is indistinguishable from a
legitimately non-matching path. There's also no glob-syntax validation at
authoring time — the `constitution` archetype's `params_schema_example`
only requires `paths` to be an array of strings, not that each string is a
syntactically valid glob.

## Evidence
Reviewer's transcript (final whole-branch review, 2026-07-06, model opus),
Minor finding #3:
> "A malformed glob in a constitution rule silently disables that rule ...
> It's consistent with the documented 'degrade to no-injection' stance for
> *whole-query* failure, but a single bad glob shouldn't silently void one
> rule."

## Hypotheses tried
1. **Hypothesis:** this is intentional, matching the CLI's documented
   "degrade to no-injection on any failure" contract.
   **Test:** re-read `docs/superpowers/specs/2026-07-06-constitution-tracker-design.md`'s
   error-handling section.
   **Verdict:** rejected — that contract is about the whole CLI query
   failing (e.g. no catalog, no augmentation), not about one malformed
   rule among many silently going dark while its siblings still enforce.
   The spec doesn't address per-rule glob validity at all; this is a gap,
   not a documented design choice.

## Fix

Implemented approach 1 (validate at write time). `src/librarian/catalog/augmentation.rs`
gained `validate_rule_globs(entry_collection, params)`, called immediately after the
existing `schema_validate::validate_against_stored(...)` check in both `append_entry`
and `merge_params_dry` — the same two pre-persist chokepoints proven by
`append_entry_rejects_schema_violation_without_writing`. When `entry_collection == "rules"`,
it walks `params.rules[].paths[]` and returns a `RecoverableError` naming the offending
pattern + rule id on the first `globset::Glob::new(s).is_err()`. JSON Schema can't itself
validate glob syntax, so this runs as a sibling check, not a schema addition.

`find_matching_rules`'s existing silent-skip in `constitution_check.rs` was intentionally
left unchanged — it remains as defense-in-depth for rules written before this guard
existed (or via any path that bypasses `append_entry`/`merge_params`).
## Tests added

`src/librarian/catalog/augmentation.rs` tests module:
`append_entry_rejects_malformed_glob_without_writing`, `append_entry_accepts_valid_glob`,
`append_entry_ignores_glob_check_for_other_collections`,
`merge_params_rejects_malformed_glob_without_writing` — cover both write chokepoints,
the valid-glob non-regression case, and scoping to `rules` only.

`src/librarian/tools/constitution_check.rs` tests module:
`malformed_glob_in_one_rule_does_not_panic_or_match` — defense-in-depth regression
confirming pre-existing bad-glob data (written before this fix) still degrades safely
at query time and doesn't suppress sibling rules.
## Workarounds
Authors of constitution trackers should double-check glob syntax manually;
no tooling currently catches a typo.

## Resume
Pick between the two Fix approaches above (likely needs a short design
note, not a full brainstorm) before implementing. If approach 1 is chosen,
it composes with the existing `params_schema` validation path already used
by `append_entry`'s schema-violation-before-write test
(`append_entry_rejects_schema_violation_without_writing`,
`src/librarian/catalog/augmentation.rs`) — JSON Schema alone can't validate
glob syntax, so this would need a custom validator, not just a schema
addition.

## References
- `src/librarian/tools/constitution_check.rs`
- `docs/superpowers/specs/2026-07-06-constitution-tracker-design.md`
