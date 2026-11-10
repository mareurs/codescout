---
name: ecc-silent-failure-review
description: Use when reviewing code for swallowed errors, misleading success results, dangerous fallbacks, missing error propagation, or failures that callers cannot distinguish from valid empty data.
license: MIT
---

# Review silent failures with codescout

Adapted from ECC silent-failure-hunter; source revision and license are recorded
in the plugin's `upstream.json` and `LICENSE`.

A suspicious catch or fallback is a search lead. Confirm a defect by tracing a
failure to an observable consequence that contradicts the relevant contract.

## Establish review scope

Read the target repository's instructions and requested diff, symbol or boundary.
A review-only request authorizes inspection and reporting, not code edits or
mutation writes. Use the existing codescout MCP connection; if a required tool
is missing, report that limitation without silently switching to native tools.

Identify the previous active root using `workspace(action="status")`. Activate
only if necessary and restore the previous root afterward. Pin subsequent
codescout calls with `workspace=<absolute target>`.

## Trace the failure

1. Locate known code with `symbols(name=..., include_body=true)`; start with
   `symbols(path=...)` for an overview. Exact strings use `grep`; concepts use
   `semantic_search` when the target index is available.
2. Inspect catch-all handlers, errors converted to empty/default values,
   log-and-continue paths, lost error context, missing timeouts, and incomplete
   rollback. Read enclosing branches rather than judging a matched line.
3. Use `references` for use sites and `call_graph` for the relevant chain.
   Establish what the caller observes and how it distinguishes success from
   failure. Read contracts through `doc(find/get)` or heading-addressed
   `read_file` for ordinary docs.
4. Inspect tests and their assertions. A passing test for another consumer does
   not establish this call site's error behavior. Commands use `run_command`;
   query buffered output and read the actual exit status.
5. An intentional optional/offline fallback can satisfy its contract. Without
   contract or caller evidence, report an open question instead of asserting a
   defect, absent logging, user impact, or a mandatory propagation rule.

## Report shape

Lead with **Confirmed findings**, ordered by demonstrated impact. Each finding has:

- **Location:** path, symbol and relevant lines.
- **Failure path:** triggering failure → handler → observable caller result.
- **Expected behavior:** authoritative contract or caller requirement, with source.
- **Evidence:** code inspected; tests inspected/not-run or actual command/result.
- **Impact/severity:** concrete affected behavior and why that severity follows.
- **Recommendation:** smallest contract-preserving correction.

Then give **Open questions** and **Coverage limits**: missing callers, unavailable
dependencies, uninspected branches and tests not run. A finding count counts
confirmed findings only. Say "none confirmed in the inspected scope" when applicable.

## Mutation verification when authorized

For a review-only task, list useful candidate probes as unexecuted. If the user
also authorized mutation testing in an isolated writable target:

- Establish a passing baseline and inspect pre-existing edits.
- Select a discriminating mutation at the actual call site: suppress propagation,
  substitute an empty success result, drop error context, or bypass rollback.
- Apply one mutation using `edit_code`, run the relevant suite through
  `run_command`, and record the observed killed/survived/inconclusive result.
- Restore exactly that candidate before the next probe, preserving pre-existing
  work, then verify restoration. Observe a survivor before writing its regression
  test. Report candidate details and commands alongside surviving/total counts.

A proposed mutation, static coverage argument or unrelated failing suite is not
an observed kill. If an isolated writable target is unavailable, report probes
as unexecuted.

Persist findings only as requested or required by the target repository's bug
workflow, using `doc` for managed artifacts. Durable lessons stay in codescout
memory or trackers.
