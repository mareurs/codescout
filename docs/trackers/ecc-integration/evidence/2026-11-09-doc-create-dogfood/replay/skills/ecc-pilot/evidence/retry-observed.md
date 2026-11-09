---
id: '1a6d98b75d325a46'
kind: spec
status: draft
title: retry_timeout observed-behavior draft
tags:
- skill-application
- observed-behavior
topic: retry_timeout
---

# retry_timeout observed-behavior draft

## Scope and provenance

- Workspace: `/home/marius/work/claude/claude-plugins.worktrees/ecc-codex-codescout`.
- Observed HEAD: `85720782f0efbaba3a2fc607f7a44317fc95adec` (`git rev-parse HEAD`, exit 0).
- Dirty-state limitation: `git status --short` reported untracked `ecc-codescout/` and `docs/superpowers/plans/2026-11-09-ecc-codex-codescout.md`. The fixture, its README and the exercised skill were inspected from the working tree; HEAD does not identify their bytes.
- Capability boundary: only `retry_timeout` in `ecc-codescout/tests/fixtures/behavior.py:4-10`, for a callable `fetch` and positive integer attempt budgets.
- Authoritative fixture contract: `ecc-codescout/tests/fixtures/README.md:5`. This contract is separate from implementation evidence. README line 11 explicitly supplies no automated execution evidence.
- This is a bounded observed-behavior draft from one application of the skill. It does not establish whole-module completeness or skill reliability.

## Inspected evidence and limits

- Implementation body: `retry_timeout`, `ecc-codescout/tests/fixtures/behavior.py:4-10`, read through `symbols(include_body=true)`.
- Caller navigation: `references` returned the definition only. `call_graph(direction="both", max_depth=1)` returned no caller edges and one LSP edge to built-in `range`. The injected `fetch` implementation is unknown; absence of graph edges is not proof of absence of dynamic callers.
- Test assertions inspected: `PackageTests/test_codex_only_and_self_contained`, `ecc-codescout/tests/test_package.py:18-37`, checks package/skill layout and text. These are not retry behavior assertions. The file overview also listed marketplace, provenance/license, and native install/discovery tests; their bodies were deferred.
- Exact-text discovery in `ecc-codescout/tests` found `retry_timeout` in the fixture definition and README; this search and the reference output supply no executed behavior evidence and do not prove repository-wide test absence.
- No automated tests or fixture behavior probes were executed. Commands actually executed were only `git rev-parse HEAD` and `git status --short`, both exit 0.
- Deferred: `optional_preview`, `required_sync`, concrete `fetch` implementations, other repository callers, other test bodies, timing/backoff behavior, and input types outside the stated boundary.

## Observed requirements

### Requirement: an attempt budget limits total fetch calls

- **Behavior:** WHEN `attempts=N` is a positive integer and every fetch call times out, THEN the loop invokes `fetch` N times in total. There is one initial attempt and at most N-1 retries. The default `attempts=3` permits three total calls and at most two retries.
- **Enforcement:** `retry_timeout`, `ecc-codescout/tests/fixtures/behavior.py:4-10`: `range(attempts)` drives the loop; each iteration contains one `fetch()`.
- **Evidence state:** code-inspected; behavioral test evidence unavailable; not executed.
- **Contract status:** observed implementation, independently supported by the total-call wording in `ecc-codescout/tests/fixtures/README.md:5`.
- **Unknowns:** behavior for non-positive values and unsupported types is outside this requirement.

### Requirement: the first successful fetch result ends the retry loop

- **Behavior:** WHEN an attempted `fetch()` returns successfully, THEN `retry_timeout` returns that value immediately and makes no later fetch calls. Earlier TimeoutErrors may precede that success within the budget.
- **Enforcement:** `retry_timeout`, `ecc-codescout/tests/fixtures/behavior.py:6-7`, immediately returns `fetch()` inside the loop.
- **Evidence state:** code-inspected; behavioral test evidence unavailable; not executed.
- **Contract status:** observed implementation, independently supported by `ecc-codescout/tests/fixtures/README.md:5`.
- **Unknowns:** the concrete result type and side effects of `fetch` are uninspected.

### Requirement: an exhausted timeout budget propagates its final error

- **Behavior:** WHEN every permitted attempt raises TimeoutError, THEN intermediate TimeoutErrors allow the next attempt, and the TimeoutError raised by the final attempt propagates. For `attempts=1`, the initial timeout is already final; there are no retries.
- **Enforcement:** `retry_timeout`, `ecc-codescout/tests/fixtures/behavior.py:8-10`, catches TimeoutError and uses bare `raise` when `attempt == attempts - 1`.
- **Evidence state:** code-inspected; behavioral test evidence unavailable; not executed.
- **Contract status:** observed implementation, independently supported by `ecc-codescout/tests/fixtures/README.md:5`.
- **Unknowns:** no concrete fetch error instance or traceback was exercised.

### Requirement: non-timeout failures propagate immediately

- **Behavior:** WHEN `fetch()` raises an exception outside the TimeoutError hierarchy on any attempt, THEN that exception propagates immediately and no later retry occurs.
- **Enforcement:** `retry_timeout`, `ecc-codescout/tests/fixtures/behavior.py:6-10`, has only an `except TimeoutError` handler.
- **Evidence state:** code-inspected; behavioral test evidence unavailable; not executed.
- **Contract status:** observed implementation, independently supported by `ecc-codescout/tests/fixtures/README.md:5`.
- **Unknowns:** no concrete non-timeout exception was exercised.

## Open questions

- **Non-positive attempts:** README `ecc-codescout/tests/fixtures/README.md:5` explicitly promises no behavior for these values and expects positive callers. Static inspection of `range(attempts)` in `behavior.py:5` shows that integer values 0 or below skip the loop: `fetch` is never called and the function implicitly returns None after line 10. This is observed implementation only, unexecuted, and is not promoted to a desired Requirement or Invariant.
- Accepted non-integer types, validation policy, and whether the non-positive implementation should be preserved remain unresolved.
- Dynamic fetch callees and caller-level expectations remain unknown.
- Executable regression evidence for the four requirements remains unavailable in this application; inspected packaging tests must not be reported as passing retry behavior tests.

## Application tool record

Used codescout `read_file` for skill/instruction/README sections and a memory output buffer; `get_guide` for symbol-navigation and librarian; `workspace(status)`; `memory(read)`; `symbols` for source and test bodies; `references`; `call_graph`; `grep` for exact text; `run_command` for git evidence; `librarian(context)`; and `doc(find/create)` for this artifact. Source, tests, skill and config were not edited.

An initial native shell attempt to read the generic using-superpowers skill was blocked by a PreToolUse hook. It did not read or alter source; the skill was subsequently read through codescout and explicitly exempts dispatched subagents. No required codescout capability failed. Calls were pinned to the target workspace; no activation request was made.

Ledger checked: none (no bug hunt or new bug report; bug ledger not queried).
