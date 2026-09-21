---
id: '48565be8ec0e15af'
kind: research
status: active
title: Codex — controlled telemetry validation and seven-day instrument alignment
tags:
- codex
- telemetry
- controlled-probe
---

# Codex — controlled telemetry validation and seven-day alignment

**Valid:** dated 2026-09-21

## Result and boundary

The installed a832ae89 dirty binary successfully migrated a legacy-shaped temporary database and recorded emitted/referenced handles with debug both off and on. The ongoing Codex MCP process still runs b53a3ffc clean. The real project's database still lacked the two columns at the end of these probes. No peer process was restarted, no production schema was manually changed, and no rebuild was needed.

The linkage records mentions, not delivery. A successful grep using handle A as its search pattern against a small ordinary file records the same read_output_ids=[A] as a successful partial read of A. Thus even filtering for outcome=success does not turn this column into a delivery receipt.

## Writer provenance

[Writer evidence](2026-09-21-codex-telemetry-writer.json) records the ancestor MCP process of a command run through this session: PID 3016183, executable path marked deleted, build b53a3ffc, dirty=false. The process executable's own version command supplied this identity. Deleted here means the running inode differs from the file now installed at that path; it does not mean the process stopped.

Separately, the first latest-three-row inspection showed build 5909c464 and process session ed33876f-6aab-4cb8-986f-a3ca92e2db02. Those rows are not evidence that this Codex process runs that build. Shared database activity must not be attributed to whichever session is asking.

The disk binary reports a832ae89, dirty=true. Final isolated runs at 08:49 UTC used that binary with SHA-256 c203fba3d10f8ecb9c3c1f2371ec83ad1f7de8b8d2019d85548e9ca60baf0709:

| debug | PID | Recorded process session |
|---|---:|---|
| off | 1709919 | 663050e3-ed40-4a19-95c9-e0068eb24674 |
| on | 1711141 | d5f0b883-4c8a-4d00-8631-79e10d423170 |

Both processes were terminated by the harness after capture. PID is historical provenance, not an address to signal later. A dirty binary is not identified by its git SHA alone; the checksum identifies the tested file, not the exact source diff used to build it.

## Controlled experiment

[Harness](2026-09-21-codex-telemetry-live-probe.py) and [captured evidence](2026-09-21-codex-telemetry-live-probe.json). The harness is a machine-local research script, not a supported product command. It starts stdio MCP in temporary roots, initializes the protocol, and calls actual tools. Each fixture begins with the then-current main database's tool_calls schema and one synthetic legacy row; no real call records are copied. Replaying after the main schema changes requires preserving that legacy precondition if migration is the property under test.

Each of two final runs executed seven calls. The producers print synthetic 400-line A/B outputs. Assertions inspect actual returned markers, persisted linkage, tool outcomes and debug payload presence. They do not merely trust isError=false, since recoverable errors can use that flag too.

| Temporary row | Control | Observed result |
|---|---|---|
| 1 | Legacy seed | Both new columns remain NULL after migration; no historical backfill demonstrated |
| 2, 3 | Produce distinct A and B overflow buffers | Distinct emitted_output_id values persist |
| 4 | Intervening ordinary small-file read | No handle reference recorded |
| 5 | Read B instead of A | Only B linked; returned B0000–B0002, no A0000 |
| 6 | Mention A only as grep pattern in small.txt | A linked, outcome success, response exactly 0 matches; A content not delivered |
| 7 | Read A lines 1–3 | A linked; A0000–A0002 returned; A0399 absent |
| 8 | Delayed read A lines 398–400 | A linked; A0397–A0399 returned; A0000 absent |

All seven final tool calls in each run recorded success. With debug off, both payload columns were NULL on every new call; linkage still populated. With debug on, both payloads were present on every new call. This verifies the new unconditional linkage path separately from debug recording.

Only six of A's 400 lines were requested/delivered in the final read controls. Neither a full-buffer retrieval nor semantic sufficiency was claimed. The result JSON omits producer envelopes and duplicate database payloads, but preserves returned control text, input arguments, row identity, linkage, payload-presence flags, process identity and binary checksum.

### Initial harness mistake and correction

The first trial attempted json_path on a command buffer, which supports raw text rather than JSON extraction. The tool correctly returned recoverable_error; the linkage still recorded the named handle. The harness was corrected to line slices and then rerun with assertions. The final two runs above are the asserted results, not the failed first trial relabeled as success. This also demonstrates why a recorded mention alone cannot prove a successful read.

## Definitions before numeric alignment

The peer's current predicate functions were inspected and imported directly from [the existing probe](../../scripts/probe-predicate-candidates.py). No alternative copy of its zero detector was substituted. Alignment data and exact matching row pairs are saved in [the comparison JSON](2026-09-21-codex-telemetry-alignment.json), including the instrument file hash.

Frozen cohort: UTC [2026-09-14 05:23:49, 2026-09-21 05:23:49), id <= 133453. Retrieved population: 9748 rows. This is the Codex seven-day cohort, not a new run over the Kat 27-day window. The two windows' denominators remain separate.

- **Path-read selector used for this comparison:** canonical read_file tool; path/file_path/file alias accepted; nonempty string target; the peer's REF_PREFIXES exclusion. This selector yields 917 reads, matching the saved Codex baseline. It is an operational selector, not proof every target names a normal filesystem file.
- **Argument equality:** parsed JSON serialized with sorted keys, not original byte order. Compare prior matching request in timestamp/id sequence. The peer uses session_id + target; the Codex reconstruction adds project_root. Both yield the identical 33 prior/current row pairs on this cohort. Their grouping definitions still differ and need not agree on other data.
- **Response equality:** compare recorded serialized output_json for those pairs; 12 equal, 21 different, reproducing the saved split. This does not identify source changes or redundant work.
- **Zero selector:** first content block's text starts with literal 0 matches. Explicit controls return true for 0 matches and false for 10 matches, 20 matches, 100 matches. This prefix detector's contract is narrower than an arbitrary prose search; its behavior for other malformed output strings is not established here.
- **Scope warning selector:** within those zero rows, first text block contains the exact standard warning prefix stored in the JSON. It yields 47 rows. This is a standard-warning predicate, not a general semantic detector of every possible scope limitation.

| Quantity on the shared frozen cohort | Result |
|---|---:|
| Path reads selected | 917 |
| Identical-argument repeat pairs, either grouping | 33 |
| Equal / different serialized responses | 12 / 21 |
| Grep calls / grep calls with output | 953 / 943 |
| Exact-prefix zero responses | 64 |
| Standard scope-warning zero responses | 47 |
| Naive substring zero responses | 121 |

The naive substring predicate adds 57 rows on this cohort. No older percentage was transplanted. Equal final totals alone would not prove interchangeable instruments; pair equality and explicit detector controls are the stronger checks performed here. The original scratch instrument was not retained as a standalone script, so this is a verified reconstruction of its declared predicates plus reproduction of its saved results, not a byte-for-byte rerun of that original instrument.

## Implications for the peer's consumer design

The [Kat division of labour](../trackers/2026-09-21-kat-telemetry-findings-for-codex-sync.md) was read. Codex supplies these empirical checks; the peer owns the annotation attachment/consumer investigation. Numeric results here are offered for peer review, not declared jointly approved.

1. Reuse the shipped mention-linkage fields. Their observed semantics should remain named, not resolved or delivered. Even success + matching handle can be mention-only.
2. Delivery instrumentation needs a resolver/served-content event if its consumer needs to distinguish row 6 from row 7. It needs segment/revision information if it needs to distinguish partial content from complete required coverage.
3. Agent sufficiency reports remain separate evidence, with unknown supported. The consumer must state what action it takes on the report; no schema change was made in this validation.
4. Rollout is still required for existing processes. Starting an isolated new writer proves the feature can run; it does not update an already-running MCP process or populate old rows. Timestamp after migration alone also cannot establish instrumentation coverage if old and new writers coexist.

No production code was changed. Validation consisted of live MCP controls and read-only frozen-cohort checks; it was not a full Rust gate or a task-outcome evaluation.
