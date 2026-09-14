---
kind: bug
status: taken
tags:
- cluster/unclassified
claimed_at: 2026-09-14
claimed_by: f3c594ce-c424-40d3-a603-9693cfef3f63
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

**ESTABLISHED 2026-09-14 at `27f3627a`, by reading the three functions that own the state
machine.** It is the third of the three candidates this section originally listed — a result
computed before the edit and delivered after — and not an LSP or `cargo` cache. There are **two**
independent defects, and the second is the one the § Fix directions do not reach.

### 1. The trigger that would clear the notice is DROPPED, and dropped silently

`on_source_write` (`src/agent/build_check.rs:469-512`) gates on `may_start`:

```rust
if !st.may_start(Instant::now(), env.debounce) {
    return;                       // no state change, no pending flag, no record
}
```

and `may_start` (`:196-199`) is `!matches!(self.state, Running) && debounce_elapsed(...)`.

So **any write landing while a check is in flight is discarded entirely.** `SessionBuildState`
(`:172-179`) is `{ edits, state, last_started }` — there is no `pending`/`dirty` field, so a
suppressed trigger leaves nothing to replay. The window is not the 1500 ms debounce
(`DEBOUNCE_DEFAULT`, `:78`) but the **check's own duration**, documented in this module's header
as a measured ~7 s `--all-targets` incremental — roughly 4.7x the debounce.

**The write most likely to be discarded is the one that FIXES the tree.** Break at T0 and the
check starts at T0; the repair lands somewhere inside T0+1.5s..T0+7s, which is exactly the
interval `may_start` refuses. Nothing after that burst writes `.rs`, so nothing re-checks.

### 2. The in-flight result then overwrites state AFTER the repair

The spawned task closes over `edits` cloned at spawn time and ends:

```rust
let outcome = run_once(&root, &edits, &env).await;
if let Ok(mut st) = slot.lock() { st.state = outcome; }   // unconditional
```

So the failure is not merely *retained* across the repair — it is **written down several seconds
after the repair landed**, by a check that began before it, over a file list that predates it.
That is why the gate could run green minutes later and change nothing.

### 3. Delivery cannot detect any of this, by signature

`take_notice(state: &mut BuildCheckState)` (`:528`) receives no instant, no tree state, no edit
generation — and `BuildCheckState::Done { my_break, delivered }` (`:149-153`) stores none. The
reader is structurally incapable of telling a 2-second-old result from a 40-minute-old one. This
is `OB-20`'s shape one layer down: the notice ARRIVES correctly and says nothing answerable,
because the party rendering it holds no input that could falsify it.

### Why that changes § Fix

"Carry the result's instant" addresses **3** and neither **1** nor **2**. A correctly-stamped
notice about a tree repaired forty minutes ago is still a notice about a tree repaired forty
minutes ago; the reader now knows it is old, which is not the same as knowing it is false. The
defect that has to be closed is the dropped trigger — a `pending` flag re-armed on suppression
and drained when a check completes, so the repair's own write re-checks.

**Not yet reproduced by observation.** The chain above is three unconditional code paths plus
one branch; the branch (`may_start` false while `Running`) is the only part a test needs, and
that test is owed with the fix. A live reproduction requires reddening a checkout shared with
five live sessions, which is `9ca234d453d61233` and was declined.
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

- `docs/issues/archive/2026-09-14-audit-doc-refs-omits-the-one-ref-kind-every-archive-breaks.md` — the
  change being edited when this fired
- `CLAUDE.md` § *Bug Tracking* — misleading errors from codescout's own MCP tools are in scope
