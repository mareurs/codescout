---
kind: bug
status: open
tags:
- cluster/lazy-warmup-bills-the-first-caller
- references
- lsp
- cold-start
- misleading-error
- refuted-mechanism
closed: null
last_observed: 2026-09-24
opened: 2026-08-27
owner: marius
related: []
severity: low
unverified: 'Recurred 2026-09-24 (twice, at a 1-2 s old rust-analyzer, under 4 parallel subagents): see Reproduction § 2026-09-24. Derived from code: the error comes from document_symbols returning an Ok empty list (null, [], or unparseable response), not from a request error. Which of the three, and why rust-analyzer answered empty, is unknown because none is logged. The 19 s cold-start refutation stands for that age only.'
---

# BUG: `references` answers a warming LSP with `symbol not found` — a resolution error, which the false-zero guard cannot see

## Summary

`references(symbol, path)` returned a hard `symbol not found` error once, for a
symbol that plainly exists, then returned 31 references on an identical later
call. **Filed 2026-08-27 with a root cause that has since been refuted.** The
entry originally claimed this was the warming-LSP class with a symptom the
false-zero guard cannot reach; a deliberate cold-start reproduction shows the
opposite — the cold-start path produces the GUARDED false-zero, and its guard
fires correctly. The original observation had no identified mechanism until **2026-09-24, when it recurred twice** (see Reproduction § 2026-09-24). That run narrows it to `document_symbols` returning an empty `Ok` list.
## Symptom (Effect)
First call, immediately after `workspace(action="activate")` on the home project:

```
references(symbol="ToolCapabilities", path="src/tools/core/types.rs")
→ {
    "ok": false,
    "error": "symbol not found: ToolCapabilities",
    "hint": "Use symbols(path) to list symbols. Trait impl methods use format 'impl Trait for Struct/method'."
  }
```

Same arguments, later in the same session:

```
references(symbol="ToolCapabilities", path="src/tools/core/types.rs")
→ 31 references in 11 files
```

The hint is actively misleading in two ways: it suggests the caller used the
wrong name form, and it points at the trait-impl syntax — neither of which
applies to a plain top-level struct that `symbols(name=...)` resolves fine.

## Reproduction

**Not reproducible.** Two mechanisms were probed deliberately; both are refuted.

**1. Genuine LSP cold start — produces a DIFFERENT, guarded symptom.**

```
workspace(action="status", post_compact=true)      # flushes all LSP clients
references(symbol="ToolCapabilities", path="src/tools/core/types.rs")
```

Confirmed genuinely cold: `ps -o etime -C rust-analyzer` showed **ELAPSED 00:19**
immediately after, i.e. the process had just respawned for this call. Result was
not `symbol not found` but:

```
0 references

warning: LSP returned 0 references outside the definition file, but
`ToolCapabilities` appears as a whole word in 5+ other source file(s) (e.g. …) —
the reference index may still be warming after a reindex. Re-run, or corroborate
with grep / call_graph(direction='callers') before treating this symbol as unused.
```

That is the symptom of `docs/issues/archive/2026-06-09-references-false-zero-stale-graph.md`,
and its `corroborate_zero_references` guard fired **correctly** — accurate,
actionable, naming the corroborating tools.

**2. Stale position after an in-place edit — refuted.** The original failure came
moments after `types.rs` was edited (a field plus a long doc comment), so a stale
indexed line number was the leading candidate. Probed by inserting a comment line
directly above the struct to shift its position, then querying immediately:
still 31 references. Probe reverted; `types.rs` clean.

A plain `activate` does not reproduce it either — it does not cold-start an
already-warm rust-analyzer.
### 2026-09-18 — non-repro datapoint, recorded as a DENOMINATOR

`references(symbol="index_repo_sync", path="src/librarian/indexer.rs")` resolved **49 references
across 4 files on the FIRST call**, issued immediately after an `/mcp` reconnect — a freshly
started server with a cold LSP, which is the condition this record was filed against. No
`symbol not found`, no retry.

Published because a re-derivation that **confirms** is a denominator and never a catch: a record
listing only the occasions something fired makes the population look more self-correcting than it
is (`CLAUDE.md` § *Testing Discipline*).

**What this is NOT evidence for**, stated because the datapoint invites the stronger reading: the
originally-filed mechanism is already REFUTED — deliberate cold-start probes produce the *guarded*
false-zero, not this record's hard resolution error. So a single cold call cannot separate "the
defect is gone" from "the defect was never cold-start in the first place", which is the standing
position. It moves the denominator and nothing else. Stays `zombie`. Recorded by sessionId
`3aa55c01-9663-44ca-82d2-48b6b8d76d66`.
### 2026-09-24 — RECURRENCE: the re-open trigger fired twice at a 1–2 s old rust-analyzer

This recurred for the first time since filing. Two `references` calls from parallel subagents returned the bare error (no "did you mean"), for symbols that `symbols(name=…)` resolves at the same path:

| time (Z) | call | result |
|---|---|---|
| 14:49:18.197 | `references(symbol="GuideLedger/adopt", path="src/tools/guide_ledger.rs")` | `symbol not found: GuideLedger/adopt` |
| 14:49:18.432 | `references(symbol="CodeScoutServer/live_ledger", path="src/server.rs")` | `symbol not found: CodeScoutServer/live_ledger` |

These are the three captures this file's Resume asks for:

1. **Process age.** `ps -o etime,lstart -C rust-analyzer` showed one rust-analyzer, **started 14:49:16Z**, which is the same second as the first subagent call after an `/mcp` reconnect. So both failures hit a **1–2 s old** process. The 2026-08-27 probes that refuted cold start ran at **19 s**. They covered a later part of warm-up, not this one.
2. **The arguments resolve.** At 14:50:44Z the identical `references(GuideLedger/adopt, …)` returned 2 references (`src/server.rs:1284`, `src/tools/guide_ledger.rs:341`), and `symbols(name=…)` found `adopt_request_conversation` and `LedgerHandle` in `src/server.rs`.
3. **Call sequence.** Four subagents dispatched in parallel, three LSP calls each. Their first calls landed 14:49:16.3–18.3Z against the just-spawned process. There was no edit to either file.

**What is now narrowed, and how it was derived, not guessed.** `References::call` (`src/tools/symbol/references.rs:311-312`) runs `c.document_symbols(&p, &l).await?` and then `find_unique_symbol_by_name_path`. A **request error** would propagate through `?` as a different message, so these two errors came from an `Ok` list with no matching symbol. The bare message (no suggestions) means the leaf search also found nothing. `live_ledger` is a field **and** a method in `server.rs`, so any real symbol list for that file would have produced a suggestion. The list was empty. `LspClient::document_symbols` (`src/lsp/client.rs:1156-1231`) returns `Ok(vec![])` in three cases: rust-analyzer answered `null`, rust-analyzer answered `[]`, or neither response shape parsed. Which of the three fired is **not recoverable**, because none of them is logged.

**Two facts that argue against a plain "too young" reading.** In the same window, a subagent's path-scoped `symbols` call on `src/server.rs` **succeeded** at 14:49:17.34 and an overview of `guide_ledger.rs` succeeded at 17.585. After that, every path-scoped lookup on those files failed from 17.83 to 18.96. So the failures are **not monotone in process age**. The same run's `symbols` zeros are recorded in `docs/issues/2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md`, which this now very likely shares a site with: `document_symbols`' empty-on-failure return. The mechanism behind rust-analyzer's empty answer is still unknown. The next step is to make `document_symbols` say which of its three empty paths it took, rather than to guess.

Recorded by sessionId `ebf651ec-5ab7-42d9-a526-dcf9758692e1`.
## Environment
- Project: codescout (Rust, rust-analyzer), branch `experiments`
- Transport: MCP stdio, Claude Code
- Binary: `target/release/codescout` built 2026-08-27 21:02
- Last HEAD observed this session: `14aa0a08` (not re-checked at observation
  time — `shell_command_mode = "disabled"` was in effect, so no `git` available)

## Root cause

**Unknown, and the originally-filed cause is REFUTED.**

This entry first claimed: *"same root-cause class as the archived false-zero bug,
but a resolution error rather than a successful zero — so
`corroborate_zero_references`, which fires on `external_refs == 0`, cannot reach
this symptom."* The reasoning was sound and the premise was wrong. A deliberate
cold start does **not** produce a resolution error; it produces the guarded
false-zero, guard firing. So there is no evidence that a warming LSP can yield
`symbol not found` at all, and the gap this entry was filed to name may not exist.

What is established:

- The observation happened (verbatim error quoted under Symptom).
- `symbols(name="ToolCapabilities")` and `symbol_at(path, 464)` both resolved the
  same name at the same path at that moment, so the arguments were correct.
- It has not recurred across many `references` calls since, including two
  deliberate cold starts and one post-edit probe.

measured 2026-08-27: `workspace(post_compact=true)` → `references` on a fresh
(19s-old) rust-analyzer → `0 references` + completeness warning, twice; comment
insert above the struct → 31 references, no failure.
## Evidence
### Ordering of the four batches, single session
```
batch 1 (right after activate):
  symbols(name_path="impl Tool for RunCommand/availability")  → ok
  references(symbol="ToolCapabilities", …)                    → symbol not found
  semantic_search("gate a tool out of the advertised list")    → ok
  read_file(".codescout/project.toml", toml_key="security")    → ok

batch 2:
  symbols(name="ToolCapabilities")                             → Struct 464
  symbol_at("src/tools/core/types.rs", line=464)               → ok (def + hover)
  artifact(action="find", kind="bug")                          → ok

batch 3:
  references(symbol="check_tool_access", …)                    → 12 refs / 2 files
  references(symbol="Availability", …)                         → 43 refs / 10 files

batch 4:
  references(symbol="ToolCapabilities", …)                     → 31 refs / 11 files
```

### The symbol was resolvable by other means at failure time
`symbols(name="ToolCapabilities")` reported `Struct 464`, and `symbol_at` at
line 464 returned a full hover including the struct's fields. So the name and
path passed to `references` were correct.

## Hypotheses tried

1. **Hypothesis:** Wrong symbol name or `name_path` form (what the error's hint
   suggested). **Test:** `symbols(name="ToolCapabilities")`, `symbol_at(path, 464)`.
   **Verdict:** rejected — both resolve the bare name at that path.
2. **Hypothesis:** Caused by `shell_command_mode = "disabled"`, set moments
   earlier. **Test:** `references` on two other symbols with shell still disabled.
   **Verdict:** rejected — both succeeded; `references` is gated on `RequiresLsp`
   and reads no shell config.
3. **Hypothesis:** `references` cannot resolve `struct` symbols. **Test:** re-ran
   the identical call. **Verdict:** rejected — 31 references.
4. **Hypothesis:** rust-analyzer had not finished project-load, and a resolution
   failure in that window surfaces as `symbol not found`. **Test:**
   `workspace(post_compact=true)` to flush clients, then `references` as the first
   navigation call, with `ps -o etime -C rust-analyzer` confirming a 19-second-old
   process. **Verdict:** REFUTED — the cold-start window yields `0 references`
   plus the completeness warning, i.e. the guarded false-zero path, not a
   resolution error. This was the leading hypothesis and the entry's filed cause.
5. **Hypothesis:** a stale indexed position after the in-place edit to `types.rs`
   that immediately preceded the failure. **Test:** inserted a comment line above
   the struct to shift its line number, queried immediately. **Verdict:** rejected
   — 31 references; probe reverted.
## Fix
**REOPENED from `zombie` — 2026-09-24: it came back twice** (open-bug sweep, `deep-agent-workflow-observations:DWF-7`). The mitigation `corroborate_zero_references` is in place (`src/tools/symbol/references.rs:162`, test at `tests.rs:5888`), but it does not cover this path. The "symbol not found" text comes from `symbol/query.rs:866-876`, reached when `document_symbols` returns a list without the name (`references.rs:311-318`), which is a resolution error rather than a zero. `usage.db` holds two more instances after this file was opened. Both were re-checked by the collector: `git grep` at each row's `project_sha` shows the symbol existed at that path.

| row | when (UTC) | call | latency | symbol at that tree |
|---|---|---|---|---|
| 76406 | 2026-08-27 18:13 | `ToolCapabilities` @ `src/tools/core/types.rs` | 2 ms | (the original) |
| 79110 | 2026-08-31 21:45 | `GetUsageStats` @ `src/tools/usage.rs` | 1 ms | `5d405b6:src/tools/usage.rs:6: pub struct GetUsageStats;` |
| 126947 | 2026-09-15 13:17 | `backup_db` @ `src/librarian/catalog/mod.rs` | 2 ms | `b4660a9:src/librarian/catalog/mod.rs:500: fn backup_db(…)` |

All three share one shape: issued in the same parallel batch as another LSP call, and answered in 1-2 ms. That is the same shape as a same-day `symbols` false zero (`usage.db` row 137400, 1 ms, path-scoped) recorded under `523233935cc53bc4`. **Unconfirmed lead:** `LspClient::document_symbols` (`src/lsp/client.rs:1156-1231`) returns `Ok(vec![])` when the result is null, and also when neither parse succeeds, so a warming or contended server can look like a file with no symbols. Confirming that is the next step.

None, and none is warranted while the mechanism is unknown and the filed cause is
refuted. Writing a "fix" for a resolution-error path that no probe can produce
would be the empty-population defect — code that compiles, tests that pass, and
zero cases acted on.

**A positive finding worth keeping instead:** the mitigation from
`docs/issues/archive/2026-06-09-references-false-zero-stale-graph.md` is now
**validated in a live cold-start window** rather than only by unit test. Its
`corroborate_zero_references` text scan fired on a genuinely 19-second-old
rust-analyzer, correctly identified 5+ other files containing the identifier, and
named `grep` / `call_graph(direction='callers')` as corroboration. That archived
entry recorded its guard as a mitigation with the LSP barrier deferred; this is
the first end-to-end confirmation that the guard does its job.
## Tests added

None. Justified: there is no established defect left to guard. The symptom this
entry was opened for is unreproduced and its proposed mechanism is refuted; the
adjacent real behaviour (cold-start false zero) already has a guard, and that
guard is now confirmed working in production conditions.

If `symbol not found` recurs, the test to write is a `References::call` unit case
pinning the error TEXT for an unresolvable symbol — the original hint
(*"Trait impl methods use format …"*) was actively misleading for a plain
top-level struct, and that is fixable independently of whatever causes the
resolution to fail.
## Workarounds
- Re-run `references` once. It is a warming window, not a persistent state.
- Warm LSP first, or corroborate with `grep "\bSYMBOL\b"` /
  `call_graph(direction="callers")`.
- Treat `symbol not found` from `references` as "unknown, retry" rather than
  "the name is wrong" — especially early in a session, and *especially* when
  `symbols(name=...)` finds the same symbol.

## Resume

**Nothing to do unless it recurs.** Re-open trigger: `references` returns
`symbol not found` for a symbol that `symbols(name=…)` resolves at the same path.
If that happens, capture in the same turn, before anything warms:

1. `ps -o pid,etime -C rust-analyzer` — process age, to establish whether it is
   genuinely a cold-start window (the 2026-08-27 probes were 19s and did NOT
   reproduce, so a recurrence at similar age argues against cold start entirely).
2. `symbols(name="<sym>")` and `symbol_at(path, line)` — confirm the arguments
   resolve by other means, as they did originally.
3. The exact preceding call sequence in the session, since the one observation
   followed a `workspace(activate)` and an in-place edit to the same file.

Do NOT re-file the refuted mechanism. Both cold-start and stale-position are
probed and rejected — see Hypotheses tried, entries 4 and 5.
## References
- `docs/issues/archive/2026-06-09-references-false-zero-stale-graph.md` — same
  root-cause class, different symptom; its `corroborate_zero_references` guard
  cannot reach this one.
- `docs/issues/archive/2026-08-16-audit-doc-refs-calls-a-warming-lsp-offline.md`
  — precedent for correcting a warming-LSP misdiagnosis in the message.
- `docs/issues/archive/2026-05-07-symbols-empty-lsp-cold-start.md`,
  `docs/issues/archive/2026-04-24-find-symbol-cold-start-hang.md`,
  `docs/issues/archive/2026-08-21-mux-lsp-cold-starts-not-recorded.md` — the
  broader cold-start family.
- Noticed while verifying `shell_command_mode = "disabled"` end-to-end, commit
  `6058dad6` (`feat(tools): hide run_command when shell_command_mode is
  disabled`).
