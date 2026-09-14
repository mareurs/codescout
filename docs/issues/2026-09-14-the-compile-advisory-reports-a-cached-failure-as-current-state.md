---
kind: bug
status: open
tags:
- cluster/unclassified
closed: null
opened: 2026-09-14
owner: marius
related: []
severity: medium
---

# BUG: the compile advisory reports a cached failure as current state

## Summary

A `doc(action="update")` response carried an advisory asserting *"your uncommitted edit does
not compile"* with 33 errors, roughly four minutes after the full four-command gate had run
green on that same tree. The errors were real — at an earlier instant. They were presented,
with no timestamp and no hedge, as a fact about the tree right now, and the advisory extends
the claim to other sessions: *"peers' gates see this break too."*

## Symptom (Effect)

Appended to an unrelated `doc(action="update")` result:

```
[codescout] your uncommitted edit does not compile, and this checkout is shared with other live sessions.
  src/librarian/tools/audit_doc_refs/mod.rs:549  missing field `live_artifact_ids` in initializer of `ResolveCtx<'_>`
  src/librarian/tools/audit_doc_refs/resolver.rs:104  non-exhaustive patterns: `RefKind::ArtifactId` not covered
  src/librarian/tools/audit_doc_refs/mod.rs:1085  non-exhaustive patterns: `Verdict::ArtifactMissing` not covered
  … showing 3 of 33
This is a statement about your own working tree, not a request — peers' gates see this break too.
```

Checked at the bytes in the same minute:

```
$ cargo check --workspace --all-targets ; echo $?
0
```

All three named errors were from a window earlier in the session, between adding two enum
variants and adding their match arms. That window closed before the gate ran.

## Reproduction

```
git rev-parse HEAD          # d311762a at time of filing
```

1. Add a variant to an enum with exhaustive `match` sites elsewhere in the crate. The tree
   does not compile.
2. Add the arms. Run `cargo check --workspace --all-targets` — exits 0.
3. Run the four-command gate — all four exit 0.
4. Call any `doc(action=...)` write. The advisory arrives naming step 1's errors.

## Environment

codescout `experiments`, MCP stdio, shared checkout with four live sessions, `target/`
contended by three concurrent `cargo test --workspace` runs.

## Root cause

Unknown — not investigated. The shape is a cached or in-flight compile result surfaced without
a freshness check, but which layer holds the staleness is unestablished: it could be an LSP
diagnostic cache, a memoised `cargo check`, or a result computed before the edit and delivered
after. **Inferred from the symptom, not measured** — recorded this way deliberately rather
than picking the plausible one, per this repo's own rule that an unmeasured mechanism is a
hypothesis wearing a conclusion's clothes.

What IS established: the advisory's content was true earlier in the session and false when
shown, and nothing in its text distinguishes those.

## Evidence

Three independent readings within the same minute, two of them authoritative:

| instrument | verdict |
|---|---|
| the advisory | 33 errors, tree does not compile |
| `cargo check --workspace --all-targets` | exit 0 |
| `grep -c ArtifactId` over the three named files | 6 / 4 / 3 — every symbol present |

The four-command gate had reported `FMT 0 / CLIPPY 0 / LEAN 0 / DEFAULT 0` minutes earlier,
with 15 named tests from the change listed `ok` in the default lane.

## Hypotheses tried

1. **Hypothesis** — a peer reverted or clobbered the edit on the shared checkout.
   **Test** — grep the three named symbols in the three named files.
   **Verdict** — rejected. All present; `cargo check` exits 0.

## Fix

None yet. Two directions, and the second is cheap enough to do regardless:

- Re-check before emitting, or carry the result's instant so a reader can see it is stale.
- **Hedge the sentence.** The cost here is not the staleness, it is the confidence. The text
  asserts a present-tense fact and then escalates to a claim about *other sessions* — which is
  the part that makes it expensive on a shared checkout, because "you are breaking everyone's
  build" is exactly the claim a careful reader stops to act on. A reader who believed it would
  have gone looking for a regression that does not exist, or worse, reverted a green change.

## Tests added

None — no fix yet.

## Workarounds

**Run `cargo check --workspace --all-targets` before believing it.** That is the whole
workaround and it costs seconds against a warm `target/`. The advisory is an advisory; it is
not an instrument, and on this evidence it is not current.

## Resume

Establish which layer holds the stale result before designing anything: instrument the advisory
to log the instant its diagnostics were computed, then reproduce with the steps above and
compare that instant against the edit's mtime. The fix differs completely depending on whether
the result is cached, memoised, or merely late.

## References

- `docs/issues/2026-09-14-audit-doc-refs-omits-the-one-ref-kind-every-archive-breaks.md` — the
  change being edited when this fired
- `CLAUDE.md` § *Bug Tracking* — misleading errors from codescout's own MCP tools are in scope
