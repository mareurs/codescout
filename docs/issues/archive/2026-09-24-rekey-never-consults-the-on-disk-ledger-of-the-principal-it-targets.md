---
id: bebe1b228d668b40
kind: bug
status: fixed
title: 'BUG: adopt_request_conversation''s "principal not served before" branch calls rekey(), which never consults the on-disk ledger of the principal it targets'
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-09-24
opened: 2026-09-24
owner: marius
related:
- a5054d135acacbe3
severity: high
unverified: 'CLEARED 2026-09-24. Was: full ./scripts/gate.sh not observed fully green for 971ed73f -- one default-lane failure (librarian::catalog::rekey::tests::rekeying_one_prefix_leaves_the_ledgers_other_prefix_untouched) attributed by run_command''s provenance check to a peer''s in-flight edit. Re-run same day on a tree containing 971ed73f (HEAD 995c0879): FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0, GATE_EXIT=0. Also verified live end to end -- see Tests added.'
---

# BUG: `adopt_request_conversation`'s "principal not served before" branch calls `rekey()`, which never consults the on-disk ledger of the principal it targets

## Summary

`CodeScoutServer::adopt_request_conversation` (`src/server.rs:1175-1204`) restores a returning principal's guide ledger from its **in-memory** `parked_ledgers: HashMap` only. `parked_ledgers` is constructed empty on every server-process startup (`src/server.rs:203-207`) and is never itself persisted. When a principal (a subagent, or the parent across a process restart) is not found there, the code falls to `live.rekey(&target)` — which unconditionally discards the live in-memory ledger and starts a fresh, empty one, **without checking whether a rich on-disk history already exists for that exact principal.** It does: `GuideLedger::persist()` writes one JSON file per principal-key, and those files demonstrably accumulate real, multi-topic state over a session's life.

## Symptom (Effect)

A principal that has genuinely already received a set of guide topics — evidenced by a populated, correctly-named on-disk ledger file for its exact key — is treated by `adopt_request_conversation` as never having been served, and every one of those topics is redelivered to the model as if for the first time.

Observed via forensic filesystem enumeration of `~/.local/state/codescout/guide_hints/` for one real, long-lived Claude Code session (`571eb3d6-c879-43f6-b3f9-5a51e744e1af`):

- The session's own **base** ledger file, `571eb3d6-c879-43f6-b3f9-5a51e744e1af.json`, held **exactly one topic** (`project-activation-bootstrap`, stamped `2026-09-24T06:20:48Z`) despite the session being ~4 days old and 7491 transcript lines long at investigation time — consistent with having been reset to empty and re-earning only one topic since.
- **33 separate `571eb3d6-c879-43f6-b3f9-5a51e744e1af_a<16-hex>.json` files** exist for that one session id — one per distinct dispatched subagent — with mtimes spanning **2026-09-20 13:34 through 2026-09-24 09:13** (the session's entire life), sizes up to 1015 bytes (a dozen-plus accumulated topics each).
- No codescout server process currently claims session `571eb3d6-c879-43f6-b3f9-5a51e744e1af` (cross-checked against all 26 live `codescout start` processes' rendezvous slots) — the server instance(s) that served it are gone, i.e. at least one restart/handoff has occurred.

## Reproduction

```
git rev-parse HEAD          # 0fef556273c211db4fe61a34d2b8ff20a92d91bb, branch experiments
```

Minimal reproduction (not yet run as an automated test — see Tests added):

1. Start a codescout server for session `S`, with `guide_hints_dir` pointing at a real directory.
2. As principal `S_a<hash>` (a subagent), insert several topics (e.g. via `get_guide`) so its on-disk ledger file `S_a<hash>.json` accumulates real content and is persisted.
3. Kill the server process (simulating any restart: `/mcp` reconnect, crash, machine sleep/wake).
4. Start a **new** server process for the same session `S` (fresh `parked_ledgers`, empty).
5. Make a tool call asserting principal `S_a<hash>` again (the same subagent identity returning).
6. Expected: the ledger for `S_a<hash>` is restored from its on-disk file, so previously-delivered topics are NOT redelivered.
7. Actual: `adopt_request_conversation` finds nothing in the fresh process's `parked_ledgers`, falls to `rekey(&target)`, and every topic that principal already held is forgotten and redelivered on next touch.

## Environment

Linux, `experiments` @ `0fef556273c211db4fe61a34d2b8ff20a92d91bb`, codescout MCP over stdio, per-CC-process server (confirmed: 26 distinct rendezvous-slot-bearing processes observed live, no sharing).

## Root cause

`CodeScoutServer::adopt_request_conversation` (`src/server.rs:1175-1204`), case 3 ("never seen"):

```rust
match parked.remove(&target) {
    Some(restored) => { *live = restored; /* … */ }
    None => {
        live.rekey(&target);   // <-- total wipe, no disk consultation
        /* … */
    }
}
```

`GuideLedger::rekey` (`src/tools/guide_ledger.rs:265-281`) repoints `self.path` at the new key's file location but does **not** read it — its own doc comment states *"a new conversation holds nothing"* and its own test (`rekey_repoints_the_path_and_forgets_every_topic`, `src/tools/guide_ledger.rs`) asserts `is_empty()` immediately after rekeying to a key with a pre-existing on-disk file, confirming this is the *intended, tested* contract of `rekey()` **as a general-purpose method**.

That contract is correct for `rekey()`'s **other** caller, `CodeScoutServer::poll_rendezvous` (`src/server.rs:1105-1123`), which uses it specifically for a genuinely new conversation (`/clear` mints a session id that has never existed, so no file exists to lose). It is **not** correct for `adopt_request_conversation`'s case 3, which restores a principal that may have a real, populated file from this same session's earlier life — in an **earlier server process** that has since exited. `GuideLedger::load` (`src/tools/guide_ledger.rs:116-132`) — used only once, for the base/parent principal, at server construction (`src/server.rs`, ~line 511) — already implements exactly the read-if-present logic that's missing here; it is simply never called for a re-adopted non-base principal.

*inferred from `src/server.rs:1175-1204`, `src/tools/guide_ledger.rs:116-132,265-281` — not measured against a live process restart; see Reproduction.*

## Evidence

### Base ledger file for `571eb3d6-c879-43f6-b3f9-5a51e744e1af`

```
{"project-activation-bootstrap":"2026-09-24T06:20:48.466163121Z"}
```
65 bytes, mtime 2026-09-24 09:20 — one topic, for a session active since 2026-09-20.

### Subagent ledger file count for the same session id

```
$ ls ~/.local/state/codescout/guide_hints/ | grep -c '^571eb3d6-c879-43f6-b3f9-5a51e744e1af_'
33
```
mtimes span 2026-09-20 13:34 → 2026-09-24 09:13. Sample content (`_a000a99d500d01a41.json`, 469 bytes):
```
{"librarian#Artifact Model":"2026-09-20T10:31:39...","librarian#docs/trackers/ — Backing Store, Not a Docs Folder":"...","librarian#librarian(action=...) — Reference":"...","progressive-disclosure":"2026-09-20T10:31:27...","project-activation-bootstrap":"2026-09-20T10:31:08...","symbol-navigation":"...","tracker-conventions":"..."}
```

### No live process currently serving this session

```
$ ps aux | grep 'codescout start' | wc -l
26
```
26 rendezvous slot files under `~/.local/state/codescout/servers/`, none stamped `"session":"571eb3d6-c879-43f6-b3f9-5a51e744e1af"`.

## Hypotheses tried

1. **Hypothesis:** codescout's MCP server is shared across multiple concurrent Claude Code sessions, so peer sessions' hooks fight over one rendezvous slot and cause the observed repeat guide delivery.
   **Test:** enumerate `~/.local/state/codescout/servers/*.json` against `ps aux | grep codescout`.
   **Verdict:** rejected — 26 slot files map 1:1 to 26 distinct processes; none share a slot.

2. **Hypothesis:** the repeat delivery is fully explained by `poll_guide_rearm`'s parent/subagent race (Decision #8 in `docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md`).
   **Test:** read the spec and the 2026-09-14 ADR directly.
   **Verdict:** confirmed as A contributing, but already-accepted, mechanism — not this bug. That race is explicitly ruled "Acceptable" and does not explain a session's **base** ledger holding only one topic after 4 days, nor 33 independently-reset subagent files.

3. **Hypothesis (this bug):** `parked_ledgers` being in-memory-only, combined with `rekey()` never consulting disk, explains the base-ledger thinness and the subagent-file count independently of the Decision-#8 race.
   **Test:** read `adopt_request_conversation` and `rekey()` directly; cross-reference against the ADR's "Deliberately out of scope" list (no mention of restart-across-process principal restoration).
   **Verdict:** confirmed as a plausible, unaddressed mechanism. Not yet confirmed by a controlled reproduction (Resume).

## Fix

*Plan first, implementation second.*

Add a new `GuideLedger` method that mirrors `rekey()`'s path-repointing but **populates from disk when a file exists** for the target key, instead of unconditionally clearing — effectively the same logic `GuideLedger::load` already uses for the base principal at construction. Call it from `adopt_request_conversation`'s case-3 branch **only** (`src/server.rs`, the `None => { live.rekey(&target); … }` arm); leave `rekey()` itself, and `poll_rendezvous`'s use of it, untouched — that call site genuinely wants "a new conversation holds nothing" and has its own passing test asserting exactly that contract, which a shared-method change would break.

Implemented as planned: `GuideLedger::adopt` (`src/tools/guide_ledger.rs`) added as `rekey`'s disk-consulting twin, wired into `adopt_request_conversation`'s case-3 branch only (`src/server.rs`). `rekey()` itself and its other caller (`poll_rendezvous`) are untouched.

One additional, latent defect surfaced by the new regression test and fixed in the same commit: `adopt_request_conversation`'s fallback target for a parent call (`asserted=None`) was the construction-time `base_ledger_key`, frozen even after `poll_rendezvous` rekeys `live` to a new conversation earlier in the SAME call. Invisible under the old blind `rekey()` (any target landed on empty); a real regression under `adopt()` (resurrected the stale key's on-disk history right back into a ledger `poll_rendezvous` had just correctly cleared). Fixed by passing `poll_rendezvous`'s current resolution into `adopt_request_conversation` as an intermediate fallback, ahead of `base_ledger_key`.

- **SHA** — `971ed73f4d9f1ded140926de7d8b7889eb1dc161` (branch `experiments`).
- **patch-id** — `5dba2cc5ae2d34af2d23f58778ecd6f520bd79f4`.

## Tests added

`a_subagent_returning_after_a_server_restart_is_restored_from_its_on_disk_ledger` — `src/server.rs`, `guide_hint_tests` module, immediately after `a_parent_call_after_a_subagent_restores_the_parents_own_ledger`. Constructs two separate `CodeScoutServer`s against the same `guide_hints_dir` and session id (simulating a restart), serves a subagent principal on the first, drops it, and asserts the second server's response to the same principal is empty (deduped from disk) rather than a fresh redelivery. Watched RED against the pre-fix code (panicked at the exact assertion, for the expected reason — real redelivery, not a compile error); GREEN after the fix. Full `guide_ledger::` (38 tests) and `guide_hint_tests` (54 tests) suites pass, including the two pre-existing tests this change could plausibly have broken: `rekey_repoints_the_path_and_forgets_every_topic` (confirms `rekey()`'s own contract is untouched) and `a_tool_call_polls_the_rendezvous_and_re_arms` (caught the `base_ledger_key` staleness regression on first write; passes after the fallback-ordering fix).

### Live end-to-end verification, 2026-09-24

The regression test hand-builds the principal and constructs both servers in one test process; this run went through the real chain instead — companion `principal-stamp.mjs` → a real `/mcp` process restart → the real per-user ledger dir (`~/.local/state/codescout/guide_hints/`).

- **Binary under test.** `target/release/codescout` built 10:31:53 +0300, after `971ed73f` (10:22:28); serving PID 1405510 started 10:35:13 with `/proc/<pid>/exe` = that path, and the binary contains the new branch's log string (`adopted its on-disk ledger`, 1 match).
- **Probe.** One `general-purpose` subagent, principal `774ba049-…/a3ba615808d91a57d`, making identical `symbols(name="GuideLedger/adopt")` calls. Topic under test: `symbol-navigation`, chosen because the parent's ledger never held it — the companion's `SubagentStart` re-arm copies the *parent's* keys, so it cannot supply this topic and cannot confound the result.
- **Positive control (old process, PID 1405510).** The probe's second call delivered `symbol-navigation`; its ledger file recorded it at `07:40:19Z`. This shows the topic *can* fire for this principal, so an absence afterwards means something.
- **Restart.** `/mcp` → PID 2072420, started 11:16:26. Probe resumed via `SendMessage`; the **same** `…_a3ba615808d91a57d.json` file was updated, so a resumed subagent keeps its `agent_id` and this fix governs resumes.
- **Result.** The file was rewritten at 11:16:51 **by the new process**:

  ```
  {"project-activation-bootstrap":"2026-09-24T08:16:51.656637764Z",
   "symbol-navigation":"2026-09-24T07:40:19.894269007Z"}
  ```

  `persist` overwrites from memory (not read-modify-write), so the new process's in-memory ledger for this principal held `symbol-navigation` **with the old process's stamp** — only reachable by `adopt` reading the disk file. Under the pre-fix `rekey()` the same write would have been `{bootstrap}` alone.

**What is NOT the evidence.** The probe's own report of its post-restart second call ("no guide injected") is uninformative: that call returned `0 matches` for a symbol the first call had just found, and a zero-match response may not trigger the topic at all — an absence the broken world produces identically. The file content above is the discriminator, and it does not depend on that call.

## Workarounds

None known. The cost is redundant guide re-delivery (token cost, conversation noise), not data loss or incorrect answers.

## Resume

N/A — fixed (`971ed73f`), regression-tested, gate green, and verified live. Archived.

## References

- `src/server.rs:1175-1204` (`adopt_request_conversation`), `:203-207` (`parked_ledgers` field), `:1105-1123` (`poll_rendezvous`, the other `rekey()` caller)
- `src/tools/guide_ledger.rs:116-132` (`load`), `:265-281` (`rekey`)
- `docs/adrs/2026-09-14-a-subagent-is-a-principal.md` — introduces the principal/park-restore model; does not address restart-across-process restoration
- `docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md` § Decision #8 — the related-but-distinct, already-accepted race this bug is NOT
- `docs/trackers/context-injection-session-log.md` F-13 (this repo) — the session-log record of the investigation that found this
- Sibling, already-open bug: `a5054d135acacbe3` (`docs/issues/2026-08-31-post-compact-clears-the-ledger-with-no-compaction-check.md`) — a different mechanism reaching a similar symptom (over-broad ledger clearing)
