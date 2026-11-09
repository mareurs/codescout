---
id: '2595ef98f1f655ad'
kind: spec
status: draft
title: Native Codex retry_timeout observed-behavior evidence
tags:
- ecc
- codex
- native-evidence
topic: ecc-codex-codescout
---

# retry_timeout — bounded observed-behavior draft

Captured from the successful real Codex CLI run on 2026-11-09, with native spec artifact 13bb7dccffca1e3d in a disposable catalog. Links below are rebased to the retained fixture snapshot; the temporary workspace has been removed. This import is a separate draft artifact. Machine-readable execution evidence: [native-report.json](native-report.json).

## Scope and provenance

- Workspace: `/tmp/ecc-native-workflow-mts04zqg/project`.
- Observed commit: unavailable. Read-only `git rev-parse HEAD` and `git status --short` through codescout each returned exit 128, "not a git repository". Dirty state cannot be established; this draft describes the inspected working-tree bytes, not a verified commit snapshot. The server's own git SHA/dirty status is not repository provenance.
- Boundary: `behavior.py::retry_timeout`, lines 4–10, for a callable `fetch` and ordinary positive integer `attempts`. Contract evidence: [README.md](../tests/fixtures/README.md), heading "Skill application fixture", especially its `retry_timeout` paragraph.
- Inspected: complete `retry_timeout` enforcement body, `behavior.py` symbol overview, module preamble lines 1–3, and the README heading. No repository `AGENTS.md` was found.
- Navigation: `references(symbol="retry_timeout", path="behavior.py")` returned only the definition at line 4. `call_graph` in both directions, depth 3, returned zero caller edges and one LSP callee edge to built-in `range`. There were no discovered callers to inspect. The callback invocation `fetch()` is visible in the body but not resolved by that graph; these results do not establish the absence of external or dynamic callers.
- Deferred: callback implementations and their side effects, external callers, built-in implementation bodies, and `optional_preview`, `required_sync`, and `commit_sync` bodies. This is not a whole-module specification.
- Tests: workspace searches for `**/test*.py` and `**/*test*` returned no files. README mentions `test_navigation.py` and says it checks tool advertisement/reference/caller navigation rather than runtime error propagation; that file and its assertions were unavailable here. This is documentation evidence only, not inspected test evidence. No behavior tests or function executions were performed.
- Existing librarian spec search and context lookup returned no matching artifacts. No source was edited.

## Observed requirements

### Requirement: Timeout failures consume a bounded number of total attempts

- **Behavior:** WHEN `fetch` raises `TimeoutError` before the final permitted call, THEN the next loop iteration calls `fetch()` again. For a positive integer `attempts=N`, there are at most N total calls, including the initial call, and at most N−1 retries. The default N=3 therefore permits three total attempts and two retries; N=1 permits one attempt and no retries.
- **Enforcement:** [behavior.py](../tests/fixtures/behavior.py), `retry_timeout`, lines 4–10: `range(attempts)` visits indices 0 through N−1; the handler re-raises only when `attempt == attempts - 1`.
- **Evidence state:** code-inspected; runtime test evidence unavailable. No test assertion anchor or execution result is available.
- **Contract status:** observed implementation, supported by README's explicit "up to attempts total calls" contract. The parameter does not count additional retries.
- **Unknowns:** accepted inputs beyond ordinary positive integers remain unresolved; see Open questions.

### Requirement: The first successful callback return ends the operation

- **Behavior:** WHEN any permitted `fetch()` invocation returns normally, THEN `retry_timeout` immediately returns that value and makes no further calls. There is no truthiness or emptiness check, so a normally returned `None` or empty value also ends the operation.
- **Enforcement:** [behavior.py](../tests/fixtures/behavior.py), `retry_timeout`, line 7: `return fetch()` inside the loop.
- **Evidence state:** code-inspected; runtime test evidence unavailable. No behavior tests were run.
- **Contract status:** observed implementation, supported by README's requirement to return the first successful value.

### Requirement: The final TimeoutError escapes

- **Behavior:** WHEN all N permitted calls raise `TimeoutError` for positive integer N, THEN the handler on the Nth call re-raises that exception. No further attempt or fallback value follows.
- **Enforcement:** [behavior.py](../tests/fixtures/behavior.py), `retry_timeout`, lines 8–10: the final-index check and bare `raise`.
- **Evidence state:** code-inspected; runtime test evidence unavailable. No behavior tests were run.
- **Contract status:** observed implementation, supported by README's statement that the final TimeoutError propagates.

### Requirement: Exceptions outside TimeoutError handling escape immediately

- **Behavior:** WHEN a `fetch()` invocation raises an exception not caught by `except TimeoutError`, THEN it propagates immediately and no later loop iteration calls `fetch`. The handler also catches subclasses of `TimeoutError`.
- **Enforcement:** [behavior.py](../tests/fixtures/behavior.py), `retry_timeout`, lines 6–10: the only exception handler is `except TimeoutError`; there is no broader handler.
- **Evidence state:** code-inspected; runtime test evidence unavailable. No behavior tests were run.
- **Contract status:** observed implementation, supported by README's requirement that other exceptions propagate immediately.

## Open questions

- Non-positive attempts: README expressly promises no behavior for these values. Static inspection of the current `range(attempts)` loop indicates zero or negative integers produce no callback calls and fall through to implicit `None`; this is an out-of-contract observation, not a promised requirement. Should such inputs be rejected, accepted, or given another meaning?
- Input types: README requests a positive "attempts value" without a type policy. The function has no explicit validation and delegates iteration setup to `range`; intended handling of floats, strings, `None`, booleans, and custom integer-like objects is unspecified. No runtime probes were performed. In particular, custom objects may interact differently with `range(attempts)` and the separate `attempts - 1` expression, so the positive-integer counting claims should not be generalized to them.
- Callback semantics: the function invokes `fetch` synchronously with no arguments. Accepted callable types, non-callable inputs, asynchronous callbacks, and side-effect/idempotency expectations are unspecified by README; callback implementations were not available for inspection.
- Runtime confirmation: no inspected assertion or executed check establishes the behavior above. README's description of navigation tests does not supply runtime coverage. Dedicated behavioral test assertions and results remain unavailable.
