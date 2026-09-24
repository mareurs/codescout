---
kind: bug
status: fixed
tags:
- cluster/lazy-warmup-bills-the-first-caller
- references
- lsp
- cold-start
- misleading-error
closed: 2026-09-24
last_observed: 2026-09-24
opened: 2026-08-27
owner: marius
related: []
severity: low
unverified:
  __delete__: true
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

**Two facts that argue against a plain "too young" reading.** In the same window, a subagent's path-scoped `symbols` call on `src/server.rs` **succeeded** at 14:49:17.34 and an overview of `guide_ledger.rs` succeeded at 17.585. After that, every path-scoped lookup on those files failed from 17.83 to 18.96. So the failures are **not monotone in process age**. The same run's `symbols` zeros are recorded in `docs/issues/archive/2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md`, which this now very likely shares a site with: `document_symbols`' empty-on-failure return. The mechanism behind rust-analyzer's empty answer is still unknown. The next step is to make `document_symbols` say which of its three empty paths it took, rather than to guess.

Recorded by sessionId `ebf651ec-5ab7-42d9-a526-dcf9758692e1`.
## Environment
- Project: codescout (Rust, rust-analyzer), branch `experiments`
- Transport: MCP stdio, Claude Code
- Binary: `target/release/codescout` built 2026-08-27 21:02
- Last HEAD observed this session: `14aa0a08` (not re-checked at observation
  time — `shell_command_mode = "disabled"` was in effect, so no `git` available)

## Root cause

**Found and measured 2026-09-24. rust-analyzer answers `textDocument/documentSymbol` with a SUCCESSFUL `null` while it swaps its crate graph, and `LspClient::document_symbols` read that `null` as "this file has no symbols".**

This was measured at the component boundary, bypassing codescout. A scripted LSP client spoke to a cold rust-analyzer 1.97.1 on this repo using codescout's own `initialize` capabilities. It opened `src/server.rs`, `src/tools/guide_ledger.rs` and `src/lsp/client.rs`, and polled `documentSymbol` every 150 ms, logging each reply as null, `[]`, N symbols or an error, next to rust-analyzer's `$/progress` stream. The run was done twice:

| run | 32 / 10 / 20 symbols | `null` for every file | back to symbols | second `null` window |
|---|---|---|---|---|
| 1 | 0.02 s | 0.72 s, at `Fetching` end, then `Building CrateGraph`, then `Roots Scanned 0/451` | 1.40 / 1.81 s | none seen at 150 ms polling |
| 2 | 0.02 s | 0.40 s, at the same transition | 0.77 s | 1.20–1.39 s, at the next `Building CrateGraph` → `Roots Scanned` |

So the answer is correct from `didOpen`, empties for every open file at each crate-graph swap, and recovers. That is exactly the **non-monotone** pattern recorded under Reproduction § 2026-09-24 (a success at 17.34 s, then failures from 17.83 to 18.96 s). It is why a single age threshold never explained this record: the 2026-08-27 probes at 19 s landed after the swaps.

**Why nothing caught it.** `request` retries only the `-32800` / `-32801` **errors** (`is_retryable_lsp_error`). A `null` is a success, so it went straight through, and `document_symbols` had `if result.is_null() { return Ok(vec![]); }`. `References::call` (`references.rs:311-312`) resolves the name against that list, so an empty list became `symbol not found`. The bare message (no "did you mean") is the tell: the leaf search over the file's symbols also found nothing. The 1–2 ms latencies of the three `usage.db` instances under *Fix* history fit a server answering from its swap state.

**The originally filed class was right in spirit and wrong in mechanism.** It *is* a warming language server. But the symptom is an empty `Ok`, not an error, and it comes from the crate-graph swap, not from indexing time.
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

**FIXED in `e26da0b2` (2026-09-24).**

- **`LspClient::document_symbols` (`src/lsp/client.rs`) re-asks a `null`** every 200 ms within a budget: 5 s inside the cold-start window, 1 s once warm. The window is read from `in_cold_start_window()`, which is now shared with `request`'s error retry, so both use one clock. An empty **array** is still returned as an answer; only `null` is re-asked.
- **A persistent `null` is reported as "no answer"** (`RecoverableError`: *"the language server answered textDocument/documentSymbol with null for … on every attempt for N ms"*, with a hint that this is not evidence the file has no symbols). It is no longer reported as `Ok(vec![])`. So if the swap ever outlasts the budget, `references` says the server did not answer instead of `symbol not found`.

This is one site, and it covers every production caller of `document_symbols` (`references`, `call_graph`, `edit_code` rename, `fetch_validated_symbol`, path-scoped `symbols`, `list_overview`, `resolve_range_via_document_symbols`, `audit_doc_refs`).

**STANDING: the unparseable branch is untouched.** If neither response shape parses, `document_symbols` still returns `Ok(vec![])`. That branch was not observed in any recorded failure, and making it an error is a separate decision about servers codescout has not measured.

**History, kept because the rejected direction would otherwise be retried:** until the 2026-09-24 boundary probe this section said no fix was warranted, because the only filed mechanism was refuted and a fix for an unproduced path would guard nothing. That was correct at the time. The fix became warranted only once the mechanism was measured.
## Tests added

All were observed RED against the pre-fix code, and all are gate-runnable (`#[cfg(unix)]`, no `#[ignore]`). They use a scripted LSP peer on a Unix socket via `LspClient::connect`, **not** the `tests/fixtures/fake_lsp_*.py` harness, whose tests are `#[ignore]`d and so never run in the gate.

- `lsp::client::tests::document_symbols_waits_out_a_null_answer_instead_of_reporting_no_symbols`: two `null`s, then one symbol. It asserts the symbol comes back and that exactly 3 requests were served. Pre-fix: `left: []`.
- `lsp::client::tests::document_symbols_reports_a_persistent_null_as_no_answer_not_as_no_symbols`: `null` forever on a warm client. It asserts an error naming `null`, and that the request was re-asked. Pre-fix: `Ok([])`.

**Mutations** (`scripts/mutation-probe.sh`, isolated worktree): "take the null as the answer" **KILLED**; "budget exhausted → return the old `Ok(vec![])`" **KILLED**. The path-scoped `symbols` half of the same fix is recorded in `docs/issues/archive/2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md`.
## Workarounds

None needed after `e26da0b2`. On an older binary: re-run `references` once, or corroborate with `symbols(name=…)` / `grep`. The window is the rust-analyzer crate-graph swap, under a second.

## Resume

Nothing left. **The live check is owed to the next release rebuild**: after `./scripts/rb.sh` plus `/mcp`, dispatch several parallel subagents whose first calls are path-scoped `symbols` / `references`, so they land inside rust-analyzer's crate-graph swaps. There should be no `symbol not found` or bare `0 matches` for existing symbols. At worst there should be the new "answered … with null" error or `completeness_warning`, never a silent zero. The boundary probe used to find the cause is reproducible from the Root cause table's method: speak to rust-analyzer over stdio and poll `documentSymbol` during start-up.
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

## Fix provenance

- **SHA:** `e26da0b2` (on `experiments`) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `65ab673b88f82817b3c85466671a28a969c7ddd1` — content hash of the diff; survives rebase and cherry-pick.

`fix(lsp): a null documentSymbol answer is re-asked, never read as 'no symbols'; path-scoped symbols names the files it could not read`
