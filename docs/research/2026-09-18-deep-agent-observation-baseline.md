---
id: '6a8d6b8eaea61de9'
kind: research
status: complete
title: Deep-agent observation baseline — usage and tracker evidence
tags:
- baseline
- usage
- observational
topic: deep-agent-observation
time_scope: '2026-09-04_to_2026-09-18'
---

# Deep-agent observation baseline — usage and tracker evidence

**Window and snapshot.** This is an aggregate-only, read-only measurement of
recorded MCP tool calls in UTC [2026-09-04, 2026-09-18). The script opened
usage.db through SQLite mode=ro, began one read transaction, and repeated the
frozen tool_calls.id cutoff 130770 at 2026-09-18T09:24:42Z. (The live database
had reached id 130804, but rows above the supplied cutoff were excluded.)
Every window query constrained both timestamps and id <= 130770. The in-transaction direct-SQL
calibration returned the same six headline counts shown below. Raw input,
output, command, path, session identifier, and error-message values are not
stored in this report or JSON.

Run again:

    python3 scripts/measure-deep-agent-baseline.py \
      --db .codescout/usage.db \
      --start 2026-09-04 --end 2026-09-18 --max-id 130770 \
      --output /tmp/deep-agent-baseline-recheck.json

## Aggregate baseline

The frozen window has 26,052 recorded MCP calls. Outcomes were 24,535 success,
1,499 recoverable_error, and 18 error: 1,517 non-success calls (5.82%). There
were 1,926 overflowed calls (7.39%). Latency was p50 20 ms, p95 15,085 ms, and
p99 100,033 ms. These are tool-call timing measurements, not model latency or
task duration.

The observed covered call times were 2026-09-04 00:00:52 through 2026-09-17
16:58:27 UTC; they do not cover every instant in the requested calendar window.
Scope is all matching tool_calls rows in this one database, across its five
recorded project-root values. It is neither all machine usage nor a
home-project-only measurement.

Input JSON and output JSON were each present for 25,435 calls (97.63%).
Presence is not a quality claim and does not license exporting the data; the
recorder only stores them when debug capture is enabled
(src/usage/mod.rs, UsageRecorder/write_content). All 26,052 rows had nonempty
session_id and cc_session_id. They comprise 275 distinct session_id values,
64 distinct cc_session_id values, and five distinct nonempty project roots.
Those cardinalities are coverage indicators only; they cannot identify a
person, agent, task, or principal.

Build composition was 77 distinct nonempty codescout SHA values by count,
24,280 rows marked dirty, 1,772 marked clean, and zero null dirty bits. These
are configuration/provenance composition counts, not a measure of code quality
or a comparison between builds.

Highest-volume tool families:

| tool | calls | non-success | overflowed | p50 / p95 ms |
|---|---:|---:|---:|---:|
| run_command | 8,382 | 489 | 674 | 71 / 43,055 |
| read_file | 3,813 | 358 | 75 | 0 / 5 |
| doc | 3,784 | 299 | 394 | 15 / 122 |
| grep | 3,454 | 13 | 282 | 13 / 124 |
| edit_file | 2,183 | 264 | 0 | 8 / 166 |
| symbols | 2,052 | 7 | 287 | 37 / 7,072 |
| edit_code | 866 | 62 | 4 | 93 / 570 |
| workspace | 507 | 0 | 4 | 539 / 12,239 |
| librarian | 315 | 2 | 188 | 6,391 / 43,545 |

The last seven complete UTC days had 14,822 calls versus 11,230 in the
preceding seven. Non-success rate was 5.73% versus 5.94%; overflow rate was
7.87% versus 6.76%. This is descriptive only: tool mix, projects, sessions,
and live work changed between weeks. It is not an on/off-policy comparison and
does not show a deep-agent effect.

## Sequence proxy

For rows with cc_session_id, the script ordered rows by
(cc_session_id, called_at, id). Of 25,988 adjacent comparable pairs, 68 were
two non-success calls with the same non-null error family: 0.262%. This is a
repeat-family proxy, not recovery, task success, user satisfaction, or causal
evidence. The three most common repeated-family labels had 16, 11, and 11
pairs; names are retained in the JSON only because they are normalized
instrument taxonomy, not raw error text.

The current tool_calls schema has session_id and cc_session_id but no
agent_id/is_sidechain field. It therefore cannot separate parent and child
tool activity in this measurement. A sequence grouped by cc_session_id can
blend delegated activity, matching the limitation in
memory infra/friction-measurement; this was checked against the current schema,
not assumed from memory.

## Interpretation limits

The recorder classifies tool outcomes before the server router serializes the
MCP response. classify_content_result downcasts RecoverableError and records
recoverable_error, while ordinary errors are error; overflowed successful
responses remain success with an overflow flag (src/usage/mod.rs,
classify_content_result). Thus non-success is a tool-result category, not a
semantic task failure, and overflow is output-buffer pressure, not failure.
Neither can measure a completed fix, correct judgment, downstream value, or
model quality.

usage.db records MCP calls only. Native shell activity outside the MCP tool
surface is invisible, so a zero is evidence about this instrumentation rather
than work performed. called_at is UTC; the half-open UTC window is deliberate.
The database contains raw debug fields, but this baseline never selects their
contents. Message/token accounting is absent from this report and must not be
substituted for a workflow outcome.

The direct SQL calibration, performed in the same read transaction with the
frozen id and UTC window, exactly matched: calls 26,052; non-success 1,517;
recoverable_error 1,499; overflowed 1,926; input present 25,435; output
present 25,435. Its grouped-by-tool call total was also 26,052 across 21 tool
groups, matching the projection-derived tool total. Both validation booleans
and the aggregate source query result are stored in the JSON; no text values
from debug columns are projected.

## Reuse

scripts/measure-deep-agent-baseline.py accepts explicit --db, --start, --end, --output,
and optional --max-id arguments. It rejects non-calendar UTC dates, an
inverted window, an invalid cutoff, and an output path equal to the database.
It verifies required schema columns before querying, uses a mode=ro
transaction, projects only input/output presence booleans rather than debug
content, writes aggregate JSON only, and automatically compares
last seven versus preceding seven days when the requested dates span exactly
14 calendar days. It does not backfill, migrate, write to usage.db, or query
raw text into its output.

Validation used a one-row synthetic schema-compatible database and confirmed
both source-query calibration booleans. An empty-window run returned zero
counts with null first/last coverage times; non-zero-padded dates and output
equal to the database path were refused. The final run above repeated the
frozen max-id cutoff and wrote both calibration checks as true.


The saved [aggregate JSON](2026-09-18-deep-agent-observation-baseline.json) and [measurement script](../../scripts/measure-deep-agent-baseline.py) preserve this baseline. Run the command from the repository root. A future window uses new dates and a new output file, omitting the frozen cutoff to snapshot the current database. Database retention or later row modification can prevent exact historical reproduction; this is not a retained raw database snapshot.

## Historical tracker evidence


The denominator is exactly the eight entry sections deep-read for this brief, across four artifacts. It is not a count of tracker populations, positive/negative balance, or prospective data. Artifact catalog provenance at read time was `fc6f5bb7b29b968def8b44a5ffa3f664c1edb6f0` for U, TU, context-injection and PR-review artifacts; each row below gives its own observed date. Historical narrative and later revision are deliberately separate.

| Seed | Area | Source and observed time | Pre-decision evidence / action | Observed outcome evidence | Prospective capture use | Label status |
|---|---|---|---|---|---|---|
| S1 | Context over-delivery, beneficial injection | `context-injection-session-log` `c340ee552bce5441`, `## W-3`, 2026-09-14 | Parent called the same `doc(find)` shape before/after subagent dispatch; hook stamp arm differed. | The entry records parent re-deliveries 2→0 under the stamp, repeated after two concurrent subagents; subagents still received bootstrap. A fresh `event_list` guide delivery was the positive control. | Capture trigger identity, delivered-set before/after, recipient principal, bytes, the fresh-delivery control, and whether the next call needed new context. | Historical measured win; **not** a prospective label. Its own control arm was historical rather than simultaneous. |
| S2 | Context claim refuted by runtime probe | `context-injection-session-log` `c340ee552bce5441`, `## F-4`, 2026-09-14, superseded by F-5 per body | Source reading suggested unknown injected top-level arguments would hit `deny_unknown_fields`; planned hook deployment was stopped. | A live stamped `doc(find)` and `doc(event_create)` succeeded; the asserted outage premise was false. The surviving finding was different deployment latency and an inert field before reconnect. | Capture source inference, the live probe chosen, rival predictions, deployed-version identity, and whether the controller changed course before write. | Historical disproven premise; useful `misleading/needs_probe` seed, never a “bad context” label. |
| S3 | Healthy high-volume guard, no intervention needed | `2026-08-15-tool-usage-investigation` `572d7f2a76026bc2`, `## TU-7`, dated 2026-08-15 | Error family/recovery/repeat metrics for IL3, IL1, stale-match, and scope-denied families. | IL3 had 85% same-tool recovery and 3% immediate repeat in that dated corpus; stale-match behavior was described as correct optimistic concurrency. | Record every trigger considered but declined: family, recovery sequence, immediate repeat, shown remedy, and `no_intervention` reason. | Dated negative result, not current state or a prospective success label. It is required anti-selection-bias evidence. |
| S4 | Ambiguous edit miss, successful discriminating investigation | `codescout-usage-frictions` `c43df94e69ca915f`, `### U-40`, 2026-08-17 | A multiline scoped edit miss reported `no_close`; the exact text had been read twice. Rival hypotheses were multiline unsupported vs literal escape corruption. | A scratch two-line edit with real newlines succeeded, refuting the first hypothesis; single-line original anchor then succeeded. | Capture original request bytes, miss tier, section hash, hypotheses, scratch probe spec/result, retry result, and whether a write happened. | Historical incident with a specific observed probe outcome; not a general cause label. |
| S5 | Document-audit false positive followed by deterministic repairs | `codescout-usage-frictions` `c43df94e69ca915f`, `### U-17`, 2026-05-23 | Audit candidates looked like repository paths; nearby onboarding text said `path/to/...` and `.github/...` belonged to reader setup. | Entry reports shipped placeholder filtering, relative resolver correction, and agent-doc exclusion; post-fix audit count table is historical measurement. | Capture parsed reference kind, source span, namespace evidence, candidate repair class, before/after finding key, and control findings unaffected. | Historical repaired incident; later fix is outcome evidence, not an input label. |
| S6 | Substantive adversarial investigation found gaps despite green tests | `pr-review-session-log` `fee90da3055f3e19`, `## F-4`, 2026-08-07 | A PR’s 12-case suite passed; its stated limitations named only two routes. Reviewer built a same-fixture adversarial probe. | Body records 10 of 11 exfiltration variants allowed and two benign commands blocked; it lists parser/tool-substitution mechanisms. | Capture candidate control type, threat hypotheses, fixture identity, each probe and control case, baseline suite result, observed result, and whether a repair intent was opened. | Historical review finding, status open in that draft tracker; neither a current vulnerability claim nor independently adjudicated training label. |
| S7 | Executing the control gave a verified review result | `pr-review-session-log` `fee90da3055f3e19`, `## W-3`, 2026-08-07 | Same PR #9 control was driven through its own handler with adversarial inputs plus a must-block control. | Entry reports 10 reproduced bypasses, 2 false positives and one triggering control; it explicitly contrasts this with source-only inference. | Capture harness build/run steps, controls, candidate variants, tool registration evidence, raw logs, and whether a reproduction was independently rerun. | Historical validated pattern with only one stated datapoint; not a model-performance label. |
| S8 | Cheap scope investigation before deep review | `pr-review-session-log` `fee90da3055f3e19`, `## W-1`, 2026-07-20 | Compared PR-described files to actual PR file list before hunk reading. | Entry says six unmentioned files were surfaced and led to later top finding F-2. | Capture declared scope, actual file list snapshot, diff, selected next investigation, and whether mismatch altered review plan. | Historical validated win with one datapoint and pending replication. |

There is no manufactured ordinary-success row beyond S3. W-1/W-3 are successful interventions; S3 is the only deep-read real “guard fired and routine recovery made new action unnecessary” example. The capture rule must sample no-intervention decisions and verified normal completions, otherwise the new ledgers will recreate the old report-only selection bias.


S6 and S7 describe the same incident and must remain grouped. Eight source sections are not eight independent episodes. This purposive sample supplies candidate patterns, not population rates or training labels. Canonical sources: [context delivery](../trackers/context-injection-session-log.md), [tool usage investigation](../trackers/2026-08-15-tool-usage-investigation.md), [usage frictions](../trackers/codescout-usage-frictions.md), and [PR review](../trackers/pr-review-session-log.md).

## Collection before implementation

Capture from 18 September until **2 October 2026 at 00:00 UTC**, with a collection-quality review on **25 September**. The [context ledger](../trackers/deep-agent-context-observations.md) records the first eligible context decision per coordinating session. The [workflow ledger](../trackers/deep-agent-workflow-observations.md) records the first eligible substantive multi-step task and session coverage receipts. Their full protocols and project [CLAUDE.md](../../CLAUDE.md) rules select ordinary cases regardless of outcome; later noteworthy cases are separate enrichment. Historical seeds remain separate from prospective observations.

| Gap in current evidence | Prospective record |
|---|---|
| Parent/child attribution | Observed principal and coordinating collector; unknown when unavailable |
| Context available at decision time | Frozen pre-action evidence, candidate context, selection or no added context |
| Tool success versus substantive completion | Task disposition and actual independent verification evidence |
| Success and no-intervention selection bias | First eligible routine sample, separated from enrichment |
| Missing captures | Session coverage receipt: selected, none-observed, missed or unknown; recording effort |
| Future training leakage | Preserve decision-time inputs separately from later outcomes and revisions; group related incidents |

Review completeness, missingness and recording burden before interpreting patterns. This observational collection changes agent behavior and does not estimate causal uplift. No automatic capture hook, scheduled review or model evaluation has been installed. Calendar completion alone does not authorize implementation or make the records training-ready.
