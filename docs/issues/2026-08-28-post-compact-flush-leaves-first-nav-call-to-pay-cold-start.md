---
id: caa8bc1df0e8c0d8
kind: bug
status: mitigated
title: 'BUG: workspace(post_compact) flushes LSP without prewarming, so the next navigation call pays cold start and can blow the 60s tool timeout — while its own hint promises no disruption'
owners:
- marius
tags:
- lsp
- cold-start
- post-compact
- references
- doc-vs-behavior
- timeout
closed: 2026-08-31
unverified: 'The MESSAGING is fixed and verified live in all three profiles; the MECHANISM is untouched. (a) the prewarm is deliberately unshipped — its prescribed form is a no-op for this bug''s own Rust reproduction (PREWARM_LANGUAGES is JVM-only) and the workspace-keyed mux makes the cold window far narrower than this record assumed, so the flush still does not prewarm and a genuinely cold single-session workspace still pays. Also NO REGRESSION GUARD: nothing asserts the hook text, so the sentence can regress silently — which is why this is not archived. The original 60s timeout has still never been reproduced with the mux confirmed down; that measurement remains owed.'
---

## Summary

`workspace(post_compact=true)` flushes every LSP client and returns a hint saying
*"Clients restart automatically on the next navigation call. LSP clients restart
lazily — **no disruption to the session**."*

The restart is lazy, so the **first navigation call after the flush pays the whole
cold start** — on this 1697-file Rust crate that exceeded the 60s tool timeout and
the call died with no result. "No disruption" is the part that is wrong.

`workspace(action="activate")` does **not** have this problem: its documented step 3
prewarms LSP for the project's languages in the background. The flush path skips
that. The asymmetry is the finding.

## Symptom (Effect)

2026-08-28, first turn of a post-compaction session. Call sequence, in order:

1. `workspace(action="status", post_compact=true)` → `{"flushed": true, …}`, fast.
2. …six intervening non-LSP calls (`run_command`, `grep`, `read_file`)…
3. `references(symbol="impl MemoryStore/write", path="src/memory/mod.rs")`
   → `Tool 'references' timed out after 60s.`

No partial result, no "still indexing" signal — the tool timed out and the caller
is left to guess whether the symbol, the path, or the server was at fault. I
guessed the symbol name, and was wrong.

## Reproduction

Not yet reproduced deliberately. The natural setup is:

```
workspace(action="status", post_compact=true)      # flush, no prewarm
references(symbol=<any resolvable symbol>, path=<any file in a large crate>)
```

Timing is the whole variable — a small crate, or a rust-analyzer that another
session already warmed, will not show it. See *Resume* for what to capture.

### Performed 2026-08-30 — and it did NOT reproduce, for a reason the record did not know

Ran the protocol this file's *Resume* section prescribes, from a session that had
called `workspace(post_compact=true)` as its first action and then made **zero** LSP
calls for 18 minutes (`symbols` and `grep` are AST/ripgrep, not LSP, so the window
stayed open).

| step | result |
|---|---|
| `ps -C rust-analyzer` before | PID 1112731, alive **17m02s** |
| `references("guard_stale_binary", "src/retrieval/sync.rs")` | returned immediately, 6 hits |
| `ps -C rust-analyzer` after | **same PID**, 17m19s — no new process spawned |

**There is a mux, and it changes the mechanism.** `rust-analyzer` does not run under
the codescout server that uses it. It runs under
`codescout mux --socket /run/user/1000/codescout-rust-mux-<workspace-hash>.sock
--idle-timeout 180 -- rust-analyzer`, a separate process keyed by **workspace**, not by
session. The mux serving my call (PID 1112714) is parented by server **801705 — a
different session's**. Mine is 803849.

So `shutdown_all()` drains *this server's* client map (`src/lsp/manager.rs:1306-1324`)
and the language server itself survives untouched.

**The Environment note above has its confound backwards.** It records *"Two other
rust-analyzer instances and eight `codescout start` processes were live on the host, so
lock/CPU contention is a confound not excluded"* — reading the peers as a source of
**contention**. Through the mux they are the opposite: concurrent sessions are what keep
the language server **warm**. The cold window only opens when the mux is genuinely down,
which needs a single-session workspace idle past 180s.

That does not refute the original timeout — it was observed. It relocates the trigger
from "any flush" to "a flush with no live mux", which is a much narrower and much more
testable claim.
## Environment

Branch `experiments` @ `894a5e26`, linux, codescout 0.15.0, release build,
rust-analyzer 1.97.1. Two other rust-analyzer instances and eight `codescout start`
processes were live on the host, so lock/CPU contention is a confound not excluded.

## Root cause

**Not established.** What IS established is that name resolution is not involved,
which is worth recording because it was my hypothesis and it looked well-supported.

The `references` seed path at `src/tools/symbol/references.rs:280-286` falls back to
`resolve_binding_by_position` when a name resolves to no document symbol, and that
fallback validates *every candidate occurrence* with `goto_definition`. Since
`symbols(name_path="impl MemoryStore/write")` returns **0 matches**, and `write`
occurs many times in that file, N LSP round-trips looked like a clean explanation.

**Refuted by probe** — see Evidence. The suspect name is fast once warm.

## Evidence

Three probes, same session, warm rust-analyzer, build idle (no lock contention):

| # | call | result |
|---|---|---|
| A | `references("MemoryStore/write", …)` — the *correct* name | **0 references**, fast, with the false-zero warning |
| B | `references("impl MemoryStore/write", …)` — the *timed-out* name | **34 references in 7 files**, fast |
| A′ | probe A again, minutes later | **34 references in 7 files** |

A → A′ is the same query returning 0 then 34, which isolates the variable to index
warmth. B being fast refutes the name-resolution hypothesis outright: the name I
blamed resolves fine.

Probe A's zero was **correctly guarded** — it carried *"the reference index may
still be warming after a reindex. Re-run, or corroborate with grep /
call_graph"*. That guard works. The timeout has no equivalent.

## Hypotheses tried

1. **Hypothesis:** the unresolvable name sent `references` into the
   per-occurrence `goto_definition` fallback, burning the timeout.
   **Test:** probe B, the same name, warm.
   **Verdict:** **rejected** — returns 34 refs immediately.

2. **Hypothesis:** OOM killed the call.
   **Test:** `journalctl -k` for the window. The only hits are an NVIDIA GPU
   `NV_ERR_NO_MEMORY` and an `OOM killer disabled`/`enabled` pair one second
   apart — the suspend/resume signature, not a kill.
   **Verdict:** rejected. (A `dmesg`-based count said `1` and was a false
   positive: `dmesg` is unreadable here, so the `||` fallback grepped a
   different source. Recorded because the wrong probe returned a *number*.)

3. **Hypothesis:** this is the zombie
   `docs/issues/2026-08-27-references-symbol-not-found-while-lsp-warms.md`
   recurring.
   **Test:** that file's re-open trigger is `references` returning
   **`symbol not found`** for a symbol `symbols(name=…)` resolves.
   **Verdict:** rejected — no `symbol not found` was ever returned. A timeout
   and a guarded zero are both explicitly outside that trigger, and that file
   says *"Do NOT re-file the refuted mechanism."* This is a separate bug.

## Fix

Not started. In preference order:

- **a. Prewarm after the flush.** `workspace(activate)` already does this
  (background, non-blocking, documented as step 3). `post_compact` flushes and
  stops. Making the flush path do what the activate path already does removes the
  cold window rather than documenting it.
- **b. Make the hint honest.** *"no disruption to the session"* is the sentence
  that cost the investigation. If (a) is not done, it should say the next
  navigation call may pay a cold start and can exceed the tool timeout.
- **c. Report, don't just die.** A timeout that returns "server still indexing,
  re-run" is a different experience from one that returns nothing. Compare probe
  A's false-zero, which is guarded and self-describing.

(a) and (b) are not exclusive; (a) alone would make (b) unnecessary.

### Correction 2026-08-30 — option (a) as written is a no-op for this bug's own reproduction

Verified at `src/lsp/mod.rs:23-25`:

```rust
/// Languages whose LSP servers are pre-warmed on project activation.
/// Hardcoded to JVM languages — Kotlin in particular takes 30–60 s to start.
const PREWARM_LANGUAGES: &[&str] = &["java", "kotlin"];
```

`prewarm_lsp_background` filters on that list. So "make the flush path do what the
activate path already does" would prewarm **Java and Kotlin only** — and the failure
reported here is **Rust**, on a 1697-file crate, against rust-analyzer. Shipping (a)
literally changes nothing observable about the reported symptom while closing the bug:
the `bug-fix-session-log:W-66` shape, a fix that changes nothing and a live defect
marked done.

Any real version of (a) has to decide something (a) never states: **which** languages
the flush path prewarms. Widening `PREWARM_LANGUAGES` is not free — it changes
`workspace(activate)` for every project and starts rust-analyzer eagerly on activation.
The conservative alternative is to prewarm exactly the languages that were **drained**
by this flush: it can never start something that was not already running, needs no
global policy change, and matches the intent ("restore what I just tore down"). That
needs `shutdown_all` to report what it drained — it currently returns `()`, across a
trait with 9 impls.

### Correction — option (b) targets a sentence that is not in this repo

The title and Summary say the tool's *"own hint promises no disruption"*. It does not.
The hint is:

> `LSP position caches cleared. Clients restart automatically on the next navigation
> call (symbol_at, references).`

which promises nothing about cost. The sentence *"LSP clients restart lazily — no
disruption to the session"* comes from the **SessionStart injection**, and
`grep -rn "no disruption"` finds it in exactly one executable place:
`claude-plugins/codescout-companion/hooks/session-start.mjs:339` — **a different repo**.
It is echoed in `docs/manual/src/concepts/post-compact-cache-flush.md:19` and `:33`.

So (b) is a cross-repo change plus a manual fix, and the Summary here merges two
surfaces into one quotation. Worth keeping straight, because the misleading half is the
one this repo cannot fix by itself.
### Shipped 2026-08-30 — (b) in-repo only, and (a) deliberately deferred

**Done, in this repo.** The `post_compact` hint no longer describes only the mechanism.
It was:

> `LSP position caches cleared. Clients restart automatically on the next navigation
> call (symbol_at, references).`

True about what happens, silent about who pays. It now names the cost, the condition
that removes the cost, and the remedy — the condition being load-bearing, because the
mux means an unconditional "expect a cold start" would be as wrong as the old silence.
The manual's two stale passages are fixed in the same commit, and the section quoting
the hook's output is left **verbatim** with a warning attached rather than silently
corrected: a manual that quotes a hook has to match the hook.

**Deferred, with reasons rather than by omission — (a), the prewarm.** Its prescribed
form is a no-op for the reproduction in this very file (`PREWARM_LANGUAGES` is JVM-only;
the repro is Rust). Any real version has to choose between widening that constant —
which changes `workspace(activate)` for every project and starts rust-analyzer eagerly —
and teaching `shutdown_all` to report what it drained, a signature change across a trait
with 9 implementations. Both are large next to a window the mux has now shown to be
narrow: single-session workspace, idle past 180s. Worth doing when someone reproduces
the timeout *with the mux confirmed down*, which is the measurement this file still
lacks and the `## Resume` protocol should now capture.

**Outstanding, and it is the half this file calls the worse one.** The sentence that
actually asserts something false — *"LSP clients restart lazily — no disruption to the
session"* — is at `claude-plugins/codescout-companion/hooks/session-start.mjs:339`, in a
different repository, and is still emitted at every post-compaction SessionStart. This
repo cannot fix it. The exact replacement it needs is the hint's own wording: name the
cost, name that a concurrent session in the same workspace may absorb it, name `re-run`
as the remedy. Update `docs/manual/src/concepts/post-compact-cache-flush.md`'s quoted
block in the same commit that lands there — the manual carries a marker saying so.

### Shipped 2026-08-31 — the cross-repo sentence, at source but not yet served

**Done.** The sentence this file calls the worse half is fixed where it lives:
`claude-plugins:02ac8f3`, patch-id `798f7d6fb9fc9f3cd1f36e123660a563ab69a377`. It now carries
the runtime hint's own wording — the cost, the condition that removes it (a concurrent session
in the same workspace holding the mux warm), and `re-run` as the remedy. Verified it had exactly
one executable home before editing: a repo-wide grep returned a single hit. `node --check` passes.

The manual's quoted block moved in the same landing, as its marker required — `2c730ebd`.

**Reproduced first, and it took no setup.** The false sentence was in this session's own
SessionStart context, received verbatim at a post-compaction start on 2026-08-31, while the
`workspace(post_compact=true)` call in the same turn returned the *corrected* in-repo hint. Both
halves of the split this file documents, observable in one turn.

**A finding the update surfaced, worth more than the edit.** The manual's block carried a warning
asserting it was quoted **verbatim** from the hook — and two of its three lines had silently
drifted anyway. The hook emits `POST-COMPACT: Context was just compacted.` and
`workspace(post_compact=true)`; the block showed `codescout PostCompact: context was compacted.`
and the `action: status` form, and the section's closing sentence repeated that second error. It
was caught by reading what a live session *actually received*, not by re-reading the block. **A
quotation that asserts its own fidelity does not check it, and the assertion is what stops a
reader looking** — the same shape as this bug's original defect one level up. The replacement note
tells the next editor to re-read the emitting file rather than trust the marker.

**Still open, and this is the whole reason.** The plugin cache is version-keyed —
`<profile>/plugins/cache/sdd-misc-plugins/codescout-companion/<version>/hooks/` — and all three
profiles hold `1.19.7` and `1.19.8`, serving 1.19.8. **The commit changes nothing any session
loads.** A bump plus `scripts/bump-cache.sh codescout-companion <version>` (which seeds all three
profiles, and refuses to run if `plugin.json` has not been bumped first) is what makes it real.
That work is owned by the claude-plugins session and is deliberately not duplicated here.

Close this bug only after probing the **served** copy — the cached file's own bytes, or a fresh
post-compaction session's injected text. Commit, install record and directory listing all read
green in the broken world; `reconnaissance-patterns:R-89`.

### Verified live 2026-08-31 — the served copy, not the source

The bump landed in the other repo (`claude-plugins:30f8fd8` → 1.19.9, checklist refreshed at
`fe39f85`) and the fix is now **actually being served**. Probed rather than inferred, because
every upstream proxy for this reads green in the broken world:

| check | result |
|---|---|
| `md5sum` of source + all three profile caches | **identical** — `cc63702c25a7218d432784ff135415b8` |
| old sentence in the served copy | **0** occurrences |
| new wording in the served copy | **1** occurrence |
| `installed_plugins.json` × 3 | `version=1.19.9`, each `installPath` under its **own** profile |

The last row is not ceremony: this machine has previously carried a `~/.claude-kat` install
record whose `installPath` pointed into `~/.claude`'s cache, so "the cache has 1.19.9" and "this
profile loads 1.19.9" are separate claims.

**Checksums alone would have been the trap.** Four copies agreeing with each other is consistent
with all four being stale — that is exactly how `R-89`'s distribution instance passed review. The
content check against the *claim* (0 old / 1 new) is the half that discriminates; the checksum
only adds that the three profiles do not disagree with one another.

### Why this is `mitigated` and not `fixed`

What shipped is **honest messaging about a cost**, not the removal of the cost. The flush still
does not prewarm; a genuinely cold single-session workspace, idle past the mux's 180s timeout,
still pays the language-server start on its first navigation call. That is (a), deliberately
deferred with reasons recorded above rather than by omission.

**Not archived, and the reason is a gap rather than a formality.** The archive trigger is a
verified fix *plus a regression test*, and nothing asserts the hook's text — in either repo. The
sentence can regress silently, and the only thing that would notice is a human reading a
post-compaction banner. A guard asserting `session-start.mjs` does not contain "no disruption"
would be cheap and belongs in the `claude-plugins` repo.

The measurement this file has owed since it was opened is still owed: the original 60s timeout has
never been reproduced **with the mux confirmed down**.
## Tests added

`post_compact_hint_prices_the_flush_rather_than_only_describing_the_mechanism`
(`src/tools/config/tests.rs`). Written RED first and confirmed RED — it failed against
the old hint, printing it verbatim in the panic message.

It asserts two things a revert to the old text fails: that the hint names the **remedy**
(`re-run`), and that it names the **shared-server caveat** (`another session`). The
second is why the test does not simply demand the word "cold": the mux makes the cost
conditional, so a hint promising an unconditional cold start would be a different wrong
answer, and a test that accepted it would be worse than none.

The original note above — *"a timing assertion would be flaky; the testable claim is
structural"* — was right and is what this follows. Nothing here asserts a latency.

The pre-existing `post_compact_flushes_lsp_clients_and_returns_flushed` asserts
`result["hint"].is_string()`, which is satisfied by any string including the misleading
one. It is left alone: it covers response *shape*, and the new test covers content.
## Workarounds

**Re-run the call.** The second attempt succeeds. If a navigation call is the
first thing you do after `workspace(post_compact=true)`, expect to pay for it
once.

Corroborate a suspicious zero with `grep` or
`call_graph(direction="callers")` before believing it — `references`' own warning
says so, and in probe A it was right.

## Resume

To pin the mechanism, capture **in one turn, before anything warms**:

1. `ps -o pid,etime -C rust-analyzer` immediately after `post_compact` — process
   age is what separates a genuine cold start from contention. The 2026-08-27
   probes on the sibling zombie were 19s old and did NOT reproduce, so age is the
   discriminator that file already relies on.
2. The `references` call, timed.
3. The same call again, timed.

Then read `workspace`'s `post_compact` branch and confirm whether it issues the
prewarm that the `activate` branch does. If it does not, (a) is a small change.

## References

- `docs/issues/2026-08-27-references-symbol-not-found-while-lsp-warms.md` — **zombie**, adjacent but NOT this: its trigger is `symbol not found`, and its cold-start mechanism is refuted
- `docs/issues/archive/2026-06-09-references-false-zero-stale-graph.md` — mitigated; the guarded false-zero probe A hit, working as designed
- `docs/issues/archive/2026-04-24-find-symbol-cold-start-hang.md` — fixed; the same 60s cold-start shape on `find_symbol`
- `docs/issues/archive/2026-07-10-lsp-shutdown-all-holds-clients-lock-across-await.md` — fixed; `post_compact` stalling navigation via a lock held during *shutdown*, which is the other half of this path
- `get_guide("workspace-state")` § *What `activate_project` does*, step 3 — the prewarm the flush path lacks
