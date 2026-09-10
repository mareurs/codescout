---
kind: bug
status: open
tags:
- cluster/unclassified
- testing-discipline
- source-scanning-guard
- monotone-assertion
closed: null
opened: 2026-09-09
owner: marius
related: []
severity: medium
unverified: 'Legitimately open, and the patch-id in the body is NOT this record''s fix anchor. `26b60af8` / `39641840397a72f257450e7d36b72e197a1c67bf` is cited in § Root cause as a WORKAROUND at one call site — doctor.rs keeps its UmbrellaPolicy rationale above the `let` rather than inside the argument list — and the body says so in the same sentence: "that is a workaround at one call site, not a fix to the guard." The guard itself still checks a fixed byte window and still reports a correct call site, so status stays `open` and `closed:` stays null. Recorded here because `non_terminal_status_with_fix_anchor` cannot distinguish a citation of a sibling''s fix from this record''s own, and its own detail names `unverified:` as the discharge for exactly this case.'
---

# BUG: a fixed-byte-window source guard both refuses a correct call site and admits a wrong one

## Summary

`every_resolve_scope_call_names_project_as_its_default` decides whether a
`resolve_scope` call names `Scope::Project` as its default by searching a
**fixed 240-byte window** after the call token. The window is a proxy for "this
call's default argument", and it fails in both directions: a correct call site
with a comment in its argument list is reported as an offender, and a call site
passing the **wrong** default passes whenever any text within 240 bytes merely
*mentions* `Scope::Project` — including a comment.

The second direction is the expensive one. It is exactly the defect the guard
was written to prevent: the guard's own doc comment records that `find`/`context`
took `Repo` while every documentation surface said "project", and nothing failed.

## Symptom (Effect)

Direction 1 — correct call site refused. A `resolve_scope` call that passes
`Scope::Project`, with a three-line explanatory comment between the call token
and that argument:

```
thread '...every_resolve_scope_call_names_project_as_its_default' panicked at
src/librarian/tools/scope.rs:678:9:
every resolve_scope call must name Scope::Project as its default -- ...
Offenders: ["librarian/tools/doctor.rs:377"]
```

Direction 2 — wrong call site admitted. The same call site with
`Scope::Repo` as its default and a comment inside the argument list reading
`// MUTATION PROBE - unlike Scope::Project, this surface wants the repo.`:

```
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5296 filtered out
```

## Reproduction

Measured 2026-09-09 against `HEAD` at the time (`670fb7bf`, four variants of one
call site in `src/librarian/tools/doctor.rs`). Each row is an observed run of
`cargo test --workspace --lib every_resolve_scope_call_names_project_as_its_default`,
not a computed offset:

| variant | offset of `Scope::Project` from the call token | verdict | correct verdict |
|---|---|---|---|
| `Scope::Project`, comment inside the argument list | 400 | **RED** | green |
| `Scope::Project`, comment hoisted above the `let` | 144 | green | green |
| `Scope::Repo`, no mention nearby | n/a | RED | red |
| `Scope::Repo` + comment mentioning `Scope::Project` | 60 | **GREEN** | red |

Rows 2 and 3 are the control pair: they establish that the guard is functional
and that the counting method works, which is what makes rows 1 and 4
measurements rather than a broken harness.

## Environment

codescout, branch `experiments`, Linux. The guard is a plain `#[test]`, no
feature gating, and runs in the default lane.

## Root cause

`src/librarian/tools/scope.rs` — the guard locates each `resolve_scope(`
occurrence, then clamps a window of `idx + 240` bytes and asks whether that
slice contains the string `Scope::Project`.

Two independent problems in one predicate:

1. **240 bytes is a proxy for "this call's arguments" and does not track the
   construct.** Comments, rustfmt wrapping, and long qualified paths all push a
   correct argument past it. The guard's own comment says the window exists to
   "widen past rustfmt's wrapping", which shows the author knew the window had
   to cover the construct and picked a constant instead of a delimiter.
2. **`contains` is monotone under adding text.** The assertion cannot
   distinguish "the argument is `Scope::Project`" from "the bytes
   `Scope::Project` appear somewhere nearby". Any comment, string literal, or
   neighbouring call that names the enum satisfies it.

The guard is deliberately a source-text scan, and the doc comment argues that
correctly: a behavioural test that calls `resolve_scope(None, .., Scope::Project)`
and asserts it returns `Project` is computed from the thing it judges. That
reasoning is sound. The defect is the *predicate*, not the decision to scan text.

The positive control (`checked >= 3`) guards against the needle silently
stopping matching, but nothing guards against the window being satisfied for
the wrong reason.

## Evidence

Direction 2 is the load-bearing one and was produced by mutating the
**production path**, not the test's inputs: the default argument was changed to
`Scope::Repo` and a comment naming `Scope::Project` was added inside the
argument list. The suite reported `ok. 1 passed`. Reverted immediately; the
committed tree carries `Scope::Project`.

Note that clippy does not backstop this: a wrong default compiles cleanly and
raises no lint.

## Hypotheses tried

1. **Hypothesis** — the window is large enough in practice and only a
   pathological comment defeats it.
   **Test** — wrote the argument list the way an ordinary explanatory comment
   would be written (three lines, no padding) and measured the offset.
   **Verdict** — rejected. 400 bytes, from prose a reviewer would ask for.

2. **Hypothesis** — the false-RED is the whole defect, since a wrong default
   would still be caught.
   **Test** — mutated the production path to `Scope::Repo` with a comment
   mentioning the literal within the window.
   **Verdict** — rejected. Green. The false-GREEN direction exists and is the
   direction the guard was built to cover.

## Fix

Not yet fixed. Direction of the fix, and the reason it is not a window bump:
raising 240 to 1000 buys direction 1 and makes direction 2 strictly worse, since
a longer window admits more incidental mentions. The two directions trade off
against each other under any constant.

Candidates, in order of preference:

- Parse the call's argument list to its closing delimiter and test the **last
  argument**, rather than a byte window over the whole neighbourhood. Strips
  comments as a side effect and removes the constant entirely.
- Failing that, strip `//` line comments from the source before scanning. Cheap,
  fixes direction 2 for the common case, and shrinks direction 1's exposure
  because comments are what usually consume the window.

Whichever lands, the guard needs a **negative** self-test: a fixture call site
with a wrong default plus a nearby mention, asserted to be reported. That case
does not exist today, which is why the false-GREEN shipped.

Workaround in tree meanwhile: `src/librarian/tools/doctor.rs` keeps its
`UmbrellaPolicy` rationale **above** the `let` rather than inside the argument
list, with a comment saying why. Committed in `26b60af8`
(patch-id `39641840397a72f257450e7d36b72e197a1c67bf`) -- that is a workaround at
one call site, not a fix to the guard.

## Tests added

None for the guard itself -- adding one is the fix, not a step toward it, and it
belongs with whichever predicate replaces the window.

The four-variant table under **Reproduction** is the reproduction, and each row
was run. It is recorded here rather than as a test because the two failing
variants require mutating a production call site.

## Workarounds

Keep explanatory prose about a `resolve_scope` call **above** the statement, not
inside the argument list. This is not discoverable from the guard's failure
message, which reports the call site as though its default were wrong -- a
reader's first move is to check the argument, find it correct, and doubt the
test rather than the window.

## Resume

Replace the window in `src/librarian/tools/scope.rs` with a delimiter-bounded
read of the call's last argument, then add the missing negative case: a call
site with `Scope::Repo` as its default and `Scope::Project` in an adjacent
comment must be reported. Run
`cargo test --workspace --lib every_resolve_scope_call_names_project_as_its_default`
against all four variants in the Reproduction table and confirm the verdict
column matches the "correct verdict" column in every row.

## References

- `src/librarian/tools/scope.rs` -- the guard and its sibling
  `scope_fallback_arm_is_not_inlined_outside_resolve_scope`, which uses an
  exact-match needle and is not affected.
- `src/librarian/tools/doctor.rs` -- the call site that surfaced it.
- `docs/trackers/issue-clusters.md` — classified `cluster/unclassified`, and the
  hatch's protocol requires naming what was checked. `IC-9`
  (`assertion-satisfiable-by-accident`) is the closest and still wrong: its claim
  turns on **environment-controlled** text, and this is project source text — that
  ledger has already withdrawn two tags matched on the looser reading *"a test
  that passes when it shouldn't"*. `IC-16` (`assertion-that-cannot-fail`) fails
  because this assertion has failing inputs; rows 1 and 3 of the table are two of
  them. `IC-14` (`guard-narrower-than-its-name`) is the exact inverse — tagging it
  there would corrupt the count its promotion reads. What this **is** is the
  second instance of the candidate class parked in `IC-14`'s own member note,
  *"a guard's trigger matches a superset of what its name claims"*: there, a
  regex `\b` made `merge\b` match `git merge-base`; here, a byte window makes
  "the default is absent" match "the default is 400 bytes away". Different
  mechanism, same shape, and both refuse valid work. Unlike that instance, this
  one **also** admits invalid work, which the candidate class as stated does not
  cover.
