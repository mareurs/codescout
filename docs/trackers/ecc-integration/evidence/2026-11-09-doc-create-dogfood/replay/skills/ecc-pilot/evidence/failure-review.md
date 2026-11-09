# Silent-failure review application — 2026-11-09

## Setup

A fresh-context tester read the adapted skill and inspected the actual fixture through codescout, with the fixture README as authoritative contract. Review scope was read-only. No source modifications, runtime behavior probes or mutation writes were authorized or performed.

## Confirmed finding

`required_sync`, tests/fixtures/behavior.py:20-24, catches any Exception and returns an empty list. The README contract requires fetch failures to propagate so the caller can abort. Returning [] makes that failure indistinguishable from successful empty data. This is a statically confirmed fixture contract violation, not an observed real transaction failure. Recommended correction: remove the catch or re-raise.

## Permitted fallback

`optional_preview`, tests/fixtures/behavior.py:13-17, catches TimeoutError and returns []. The README explicitly permits this offline preview behavior; other exceptions propagate through the inspected function. No defect was confirmed there.

## Evidence and limits

This section records the initial fixture before commit_sync was added; the corrected native execution below supersedes its caller-tracing limit.

The tester inspected the complete function bodies, fixture contract, references and caller graph. Reference queries found only definitions; graph queries returned no caller edges, which does not exclude dynamic callers. Package tests do not establish fixture behavior. No fixture tests or mutation probes were executed. Proposed probes remained explicitly unexecuted.

The tester recovered from a missing cross-project memory topic, an overlarge documentation selection and initial LSP warming. This is one observed skill application, not a comparative reliability measurement.

## Native Codex execution

The first real Codex CLI 0.167.0 execution installed the native plugin in a disposable CODEX_HOME and invoked `$ecc-codescout:ecc-silent-failure-review`. It confirmed required_sync and accepted optional_preview, but could not trace callers: the scratch project had languages=[] and did not advertise references/call_graph. Controlled same-binary probes established the missing language configuration as the cause.

The corrected runner declares Python in the fixture and starts codescout with --project. Its successful both-mode execution is recorded in [native-report.json](native-report.json); raw transcripts are /tmp/ecc-native-corrected-20261109-3/review.jsonl and review.md. Actual completed references output identifies `items = required_sync(fetch)` at behavior.py:28. Actual call_graph output identifies the LSP edge `commit_sync → required_sync`. The agent inspected both bodies, confirmed the exception-to-empty-to-commit violation and accepted optional_preview's documented timeout fallback.

This remains static confirmation of the planted fixture defect. The native reviewer executed no behavior tests or mutations and made no source edits or bug records. It distinguished inspected navigation assertions from runtime propagation coverage.

The same native run also persisted a librarian spec draft; see [captured native draft](native-retry-observed.md). Two earlier persistence attempts exposed shared-catalog inheritance and Codex MCP environment filtering. The final runner isolates state and explicitly supplies it to the MCP child. Temporary authenticated profiles were private and removed after each run; routine tests copy no auth and make no model calls. This is a bounded smoke check, not a benchmark or proof of complete workflow parity.
