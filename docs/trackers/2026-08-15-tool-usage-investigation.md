---
id: '572d7f2a76026bc2'
kind: tracker
status: active
title: Tool-Usage Investigation — 2026-08-15 (TU-N)
owners:
- marius
tags:
- investigation
- usage-db
- tool-surface
- agent-guidance
- baselines
topic: tool-usage-investigation
---

> **Prefix:** `TU-N` — a finding from this investigation. Work-stream-scoped, defined here, not a
> project-wide namespace (`docs/TAXONOMY.md` § Work-stream-specific prefixes). Cite as
> `TU-3 (2026-08-15 tool-usage investigation)`.

## What this is

A one-pass empirical audit of codescout's **tool surface** — not of any feature — driven by a
question from the human: *"when reading a long doc we get a handle, then the agent tries to call it
and doesn't know how. I think there is a gap somewhere."*

Method: merge every `.codescout/usage.db` on the machine into one corpus and look for **consecutive
calls that resolve in different ways** — the signature of a tool whose failure does not teach the
caller what to do next.

It found four filable defects, one already-fixed near-miss, one blind spot in the measurement
itself, and — importantly — two guards that look alarming and are healthy. That last category is
why the investigation is worth recording: **the naive metric (error count) ranks a healthy guard
above a broken one.**

## Corpus

    53,916 tool calls · 460 sessions · 13 projects · 2026-05-03 .. 2026-08-15

Overall error rate by month — improving:

| Month | Calls | Errors | Rate | Sessions |
|---|---:|---:|---:|---:|
| 2026-05 | 31,743 | 1,679 | 5.3% | 360 |
| 2026-06 | 844 | 80 | 9.5% | 11 |
| 2026-07 | 7,351 | 431 | 5.9% | 38 |
| 2026-08 | 13,978 | 561 | **4.0%** | 51 |

## Findings index

| ID | Finding | Disposition |
|---|---|---|
| TU-1 | The overflow hint's own recommended recovery needs `[*]`, which `json_path` rejects | **filed** — `docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md` (`a7da09c6`) |
| TU-2 | IL1's always-loaded text grants line-range reads without the overlap condition | **filed** — `docs/issues/2026-08-15-il1-always-loaded-text-omits-the-overlap-condition.md` (`32984694`, corrected `9746a5f0`) |
| TU-3 | A write-scope denial does not name `approve_write` | **fixed** 2026-08-16 (`fe7732e2`) — `docs/issues/archive/2026-08-15-write-scope-denial-does-not-name-approve-write.md`. Title was wrong: it *did* name `approve_write`, as `approve_write('<dir>')` — a placeholder, in a positional form the tool does not accept |
| TU-4 | Conditionally-required params are advertised as optional | **fixed** 2026-08-16 (`1a54b5a6`, extended by `6ba720bc`) — `docs/issues/archive/2026-08-15-conditionally-required-params-advertised-optional.md`. Class A only; B and C deliberately deferred |
| TU-5 | 31% of errors carry no `err_family` — the ranking's own blind spot | **fixed** — taxonomy extended + `BACKFILL_VERSION` bumped. **Headline corrected: 31% was a lifetime figure; live-DB rate was 19.8%, now 2.9%.** See § History |
| TU-6 | `ast_extent_fail`'s hint blamed syntax errors on files that parse | **already fixed** `cafa4b37`, same day — near-miss, see § Method caveats |
| TU-7 | Two high-volume guards are healthy and must not be "fixed" | **no action** — negative result, recorded so it is not re-litigated |
| TU-8 | Routing is 45% of all errors — and is broadly working | **no action** — negative result, see § Tool sweep |
| TU-9 | `artifact_event` carries TU-4's defect in a third tool | **fixed** 2026-08-16 (`6ba720bc`) — folded into TU-4's fix as this entry asked. Nine required fields across seven kinds, not the three that happened to fail |
| TU-10 | Overflow pressure is concentrated, and compounds TU-1 | **open**, not filed |
| TU-11 | Reading the arguments overturned conclusions in **both** directions | **method** — see § Trace pass; corrections folded into TU-1 and TU-2 |

## TU-7 — the negative result that shapes everything else

Ranking error families by count puts `il3_pipe_to_trimmer` (262) near the top. It is **healthy**:

| Family | Hits | Per affected session | Same-tool recovery | Immediate repeat |
|---|---:|---:|---:|---:|
| `il3_pipe_to_trimmer` | 262 | 4.0 | **85%** | **3%** |
| `il1_read_overlaps_symbol` | 416 | 4.7 | 35% | 14% |
| `edit_stale_match` | 141 | 1.8 | 21% (125/141 recover across 7 tools) | 2% |
| `write_scope_denied` | 42 | 1.8 | 2% | **26%** |

`il3_pipe_to_trimmer` fires often, and the agent re-runs the command bare and succeeds 85% of the
time — fire, correct, done. `edit_stale_match` looks bad on same-tool recovery but is the *correct*
failure mode for optimistic-concurrency editing: re-read, then retry, which is what 125 of its 141
recoveries do.

**So frequency is not the signal — recovery is.** The two measures that separate a teaching guard
from a blocking one:

- **same-tool recovery** — did the caller fix the call, or abandon the tool?
- **immediate repeat** — did the same error fire again straight after?

`write_scope_denied` has the corpus's **highest** repeat rate (26%) on only 42 hits, and
`il3_pipe_to_trimmer` the lowest (3%) on 262. The difference is that the latter's message states a
concrete corrective action. That single comparison is the investigation's main analytic result.

## TU-5 — the blind spot in the measurement itself

**855 of 2,751 errors (31%) have `err_family IS NULL`**, so every ranking above describes 69% of the
error population. The unclassified set is not noise — it has structure, and TU-4 was found inside it
(the conditionally-required-param family was invisible to `err_family` entirely).

Other shapes visible there, unfiled: `edit_file is blocked for source code files
(debug_enforce_symbol_tools is enabled)` (26), `file has no frontmatter block` (14), `path segment
'X' not found` on json_path key misses (14), `read_markdown only supports .md files` (12),
`'…' is a librarian-managed artifact` (7).

**Recommendation:** extend `normalize_err_family` (`src/usage/db.rs:159-267`) to cover the
unclassified head, then re-run this investigation. Until then, any claim of the form "the top N
error families are …" is a claim about the classified subset only.

## Baselines — for re-measurement

Numbers that constitute acceptance tests. Re-run on a **single project** so the confound in
§ Method caveats cannot recur.

| What | Baseline | Source |
|---|---|---|
| `il1` reach / intensity (codescout, Aug) | 29% of sessions / 5.8 per affected session | TU-2 |
| `il1` rate (codescout, Jul → Aug) | 23.1 → 6.4 per 1,000 calls | TU-2 |
| `write_scope_denied` immediate repeat | 26% | TU-3 |
| `json_path` rejections: share that are `[*]` | 73% (22 of 30) | TU-1 |
| Class-A conditional-param errors | 14 of 34 live schema errors (41%) | TU-4 |
| Schema errors per 1,000 calls | 1.76 → 2.04 → 1.36 (May/Jul/Aug) | TU-4 |
| Errors carrying no `err_family` | 31% | TU-5 |

## Method — how to repeat this

Merge every project's usage.db into one queryable corpus (schemas were verified identical across
projects first):

    sqlite3 merged.db "CREATE TABLE tool_calls (id INTEGER, tool_name TEXT, called_at TEXT,
      latency_ms INT, outcome TEXT, overflowed INT, error_msg TEXT, codescout_sha TEXT,
      project_sha TEXT, session_id TEXT, input_json TEXT, output_json TEXT, cc_session_id TEXT,
      friction_target TEXT, overflow_tokens INT, err_family TEXT, project_root TEXT, proj TEXT);"

    for db in ~/work/claude/*/.codescout/usage.db; do
      p=$(basename $(dirname $(dirname $db)))
      sqlite3 merged.db "ATTACH '$db' AS s; INSERT INTO tool_calls
        SELECT id,tool_name,called_at,latency_ms,outcome,overflowed,error_msg,codescout_sha,
               project_sha,session_id,input_json,output_json,cc_session_id,friction_target,
               overflow_tokens,err_family,project_root,'$p' FROM s.tool_calls; DETACH s;"
    done

The core query — sequence each error against what the agent did next, partitioned by session:

    WITH seq AS (
      SELECT rowid r, session_id s, tool_name t, err_family f, outcome o,
             LEAD(tool_name,1) OVER w n1, LEAD(outcome,1) OVER w o1,
             LEAD(err_family,1) OVER w f1
      FROM tool_calls WINDOW w AS (PARTITION BY session_id ORDER BY id))
    SELECT f, count(*) hits, count(DISTINCT s) sessions,
           round(1.0*count(*)/count(DISTINCT s),1) per_sess,
           round(100.0*sum(CASE WHEN f1=f THEN 1 ELSE 0 END)/count(*),0) immediate_repeat,
           round(100.0*sum(CASE WHEN o1='success' AND n1=t THEN 1 ELSE 0 END)/count(*),0) same_tool_recovered
    FROM seq WHERE f IS NOT NULL GROUP BY f ORDER BY hits DESC;

Retention is 30 days and is enforced **on write**, so idle projects keep older rows — which is why
this corpus reaches back to May while `codescout`'s own window is recent.

## Method caveats — traps hit during this pass

Each of these produced a wrong intermediate conclusion that was caught before it shipped. They are
the reusable part of this document.

1. **Month-over-month is project-confounded.** Each month's corpus is dominated by a *different*
   repo (May: `code-explorer.old`, 30,556 calls; Jul/Aug: `codescout`). Only July→August is
   like-for-like. A May→July "spike" is not a real trend. **Always group by project as well as month.**
2. **Lifetime averages hide trajectories.** TU-2 was filed with "4.7 per session" — a lifetime mean
   that both understated current intensity (5.8) and hid that August is worse than May. Split
   **reach** (share of sessions affected) from **intensity** (hits per affected session); they move
   independently.
3. **Count what is still live.** 90 lifetime schema errors reduce to 34 once already-fixed causes
   are removed — the inverted-filter rows are all 2026-05-03, before the 2026-07-10 fix closed them.
   Date every class against the commit that fixed it before counting it.
4. **Read the emitting site before filing.** TU-6 looked like a clean find and was fixed the same
   day by a concurrent session (`cafa4b37`). Reading `edit_code.rs` first is what caught it — the
   fix's own comment records the identical reasoning.
5. **A `git log -S` miss can be a search artifact.** The pickaxe string spanned a line continuation
   in the source, returning empty and reading as "uncommitted". A contiguous substring found the
   commit at once.
6. **Tracing logs are not the protocol.** `.codescout/diagnostic-*.log` carry `tracing` output, not
   raw JSON-RPC, so zero hits for `progressToken` say nothing about what the client sent. The only
   `_meta` matches there are `body_meta` inside response bodies.
7. **`input_json` is `--debug`-gated** (`src/usage/mod.rs:85-89`), so the *arguments* of a call are
   not recorded in normal sessions. Every question of the form "what did the agent actually ask
   for?" is unanswerable from this corpus. That gap is CAP-1 in
   `docs/trackers/capability-proposals.md`.

## Tool sweep — all tools, 2026-07 onward

A second pass, **tool-complete rather than family-complete**, run to sidestep TU-5: it depends on
`error_msg` and `tool_name`, not on `err_family`, so the unclassified 31% is included.

Window: **21,329 calls · 89 sessions · 8 projects · 992 errors.**

| Tool | Calls | Err% | Overflow% | Avg ms | Max ms |
|---|---:|---:|---:|---:|---:|
| `run_command` | 7,098 | 4.7 | 9.9 | 7,108 | 1,500,016 |
| `symbols` | 2,925 | 0.4 | 7.8 | 396 | 45,840 |
| `read_file` | 2,598 | **11.9** | 1.5 | 0 | 45 |
| `grep` | 2,262 | 0.3 | 3.0 | 23 | 9,230 |
| `artifact` | 1,250 | 2.6 | 7.7 | 13 | 4,579 |
| `edit_code` | 1,153 | 6.9 | 0 | 64 | 9,403 |
| `edit_file` | 1,113 | 8.8 | 0 | 9 | 83 |
| `read_markdown` | 985 | 5.5 | 2.4 | 0 | 10 |
| `edit_markdown` | 527 | 5.7 | 0 | 1 | 13 |
| `workspace` | 244 | 0.4 | 0 | 1,872 | 13,433 |
| `librarian` | 139 | 0 | **40.3** | 1,831 | 16,530 |
| `semantic_search` | 172 | 0 | **20.9** | 1,050 | 2,215 |

### TU-8 — routing is 45% of all errors, and is broadly working

**444 of 992 errors (44.8%)** in the window are the gate re-routing a wrong-tool call:

| Route | n |
|---|---:|
| `run_command` → bare + `@cmd_*` buffer (IL3 pipe) | 221 |
| `run_command` → symbols/grep (shell on source) | 110 |
| `edit_file` → `edit_code` (structural) | 53 |
| `read_markdown` → `artifact` (librarian-managed) | 23 |
| `edit_file` → `edit_markdown` | 17 |
| `read_file` → `read_markdown` | 10 |
| `read_markdown` → `read_file` (non-md) | 7 |

**Compliance — and a measurement trap worth copying.** Measured on the *immediately* next call,
compliance looks poor: `edit_file`→`edit_code` 45%, `read_file`→`read_markdown` 40%. Widening to
**within three calls** changes the picture completely:

| Route | n | next call | within 3 |
|---|---:|---:|---:|
| `edit_file` → `edit_code` | 53 | 45% | **74%** |
| `read_markdown` → `artifact` | 23 | 57% | **74%** |
| `edit_file` → `edit_markdown` | 17 | 59% | **76%** |
| `read_file` → `read_markdown` | 10 | 40% | **70%** |

The strict measure understates compliance by 25–30 points, because an agent told "use `edit_code`"
reasonably **reads the symbol first**. Adjacent-call analysis systematically penalises any guard
whose correct recovery involves a lookup. Always widen the window before calling a routing guard
ineffective.

**Verdict: no action.** Routing dominates error *volume* while working ~75% of the time. Like TU-7,
this is recorded so a future pass does not mistake the volume for a defect. The residual ~25% is
roughly 14 cases at n=53 — not worth chasing ahead of TU-5.

### TU-9 — `artifact_event` carries TU-4's defect, in a third tool

`artifact_event` shows a 50% error rate in the window (3 of 6 calls). All three are the **same class
as TU-4** — a payload field required only for a particular event `kind`:

    note.text required
    external_signal.source_id required
    external_signal.summary required

TU-4 was filed against `edit_code.body` and `artifact.patch` and recommended sweeping every
`action`-dispatched tool rather than fixing only the two measured. This is that recommendation
vindicated **by a pass that did not look for it** — `artifact_event` never surfaced in the
family-based analysis because these errors carry no `err_family`. Add it to TU-4's fix.


**Resolved 2026-08-16 (`6ba720bc`), and the count was larger than measured.** The three
fields above are the three that happened to fail; `validate_payload` enforces **nine
required fields across seven kinds** (`note`→text; `status_change`→to; `field_patch`→field
+to; `superseded_by`→target_artifact_id; `external_signal`→source_id+summary;
`intent`→hypothesis; `verdict`→outcome — `reviewed` requires none). The `payload` schema
now renders that list from a table beside the validator.

This entry's own framing is worth keeping as a method note: it called itself "the
recommendation vindicated by a pass that did not look for it". That is exactly right, and
the reason generalises — **these errors carry no `err_family`**, so the family-based
analysis that found `edit_code.body` and `artifact.patch` was structurally incapable of
seeing them. A ranking by error family cannot surface a defect whose errors are
unclassified; it took a per-tool sweep. Kin to R-91 in
`docs/trackers/reconnaissance-patterns.md` — a probe that cannot observe the thing the
claim is about.

`validate_payload` was deliberately left unrewritten rather than made table-driven:
`field_patch.to` accepts any JSON value while the other eight require strings, and
collapsing that distinction would have been a silent behaviour change. The table and the
validator are held together by a test that *executes* the validator instead.
### TU-10 — overflow pressure is concentrated, and compounds TU-1

| Tool | Overflow rate | Avg tokens | Max tokens |
|---|---:|---:|---:|
| `librarian` | **40.3%** | 9,474 | 45,088 |
| `semantic_search` | 20.9% | 2,843 | 5,017 |
| `run_command` | 9.9% | 7,526 | 44,600 |
| `artifact` | 7.7% | 5,739 | 26,127 |
| `grep` | 3.0% | **84,928** | **4,427,639** |

Two distinct shapes. `librarian` overflows **two calls in five** and is also the largest by average
payload — progressive disclosure is the normal path there, not the exception. `grep` is the
opposite: it rarely overflows, but when it does the buffer is enormous — one call produced
**4.4 million tokens**.

**The compounding matters more than either number.** The tools that most often hand back a handle
(`librarian`, `artifact`, `semantic_search`) are exactly the ones whose payload is a **list of
records** — and TU-1 is that `json_path` cannot project a field across a list (`[*]` is rejected,
73% of all rejections). So the highest-overflow tools feed the weakest recovery path. Fixing TU-1
is worth more than its own 30-occurrence count suggests.

## Trace pass — reading what the agent actually asked for

Everything above is sequence-shaped: *which tool errored, which tool came next, did it succeed*.
None of it shows **intent**. Prompted by the concern that statistics-from-above give the wrong
impression, a third pass read `input_json` — which turns out to be populated on **51,164 of 53,916
rows (95%)**, because debug capture has been on. (§ Method caveats #7 said this was unavailable; the
mechanism is real but the data exists on this machine. Corrected.)

It overturned conclusions in **both** directions, which is why it is recorded as its own finding.

### TU-11a — statistics OVERSTATED recovery (TU-1)

Every `[*]` rejection was scored a successful recovery by the sequence view. The arguments show what
those recoveries actually were:

| The agent asked for | "Recovery" scored as success | What it really cost |
|---|---|---|
| `$.items[*].abs_path` | `read_file(lines 92–485)` | 393 lines of raw JSON for one field |
| `$.entries[*].id` | `$.entries[4].id` | one element per call, O(n) |
| `$.results[*].rel_path` | re-ran `artifact(find)` narrower | abandoned the buffer, re-queried upstream |
| `$.results[*].['id','rel_path','title']` | `grep` regex on the buffer | left the tool surface |

**Not one recovery got what was asked for cheaply.** A green `outcome` column concealed a degraded
workaround every time.

It also revealed a requirement TU-1 understated: two of six used **multi-field selection**
(`['id','title','rel_path','status','tags']`), not just a wildcard. Agents want *projection*, not
only `[*]`.

### TU-11b — statistics UNDERSTATED the guard (TU-2)

The opposite error. `il1`'s "35% same-tool recovery" counted only a retry of `read_file`. Traced
sequences show the *correct* recovery is usually a different tool:

    ERR  read_file("src/embed/ast_chunker.rs", 2076, 2189)
      N1 symbols(name="tests/split_file_rust_populates_metadata_headers", include_body=true)  ok
      N2 symbols(name="tests/inner_method_signature_skips_doc_comments", include_body=true)   ok

The agent wanted two test bodies and got them by name. That is the guard working, scored as failure.

But the same pass found a **sharper defect the statistics missed entirely**. Bucketing the refused
ranges by shape: of 244 refusals, **84 are file-head reads** (`start_line` ≤ 5) and **69 of those
extend no further than line 60** — canonical imports reads (`1–20`, `1–30`, `1–60`), refused because a
`mod` or struct begins inside them. And `symbols` **cannot** answer them: it is a definition
projection that does not return imports (`iron-laws-detail.md:12-16`). For 28% of refusals the
recommended recovery is structurally incapable, and `force=true` — the only thing that works — is
offered second.

TU-2 is therefore not "a guard that fires too often" but **one guard serving two populations**:
healthy for symbol-body reads, structurally wrong for the non-definition minority. Its Fix section
was rewritten around that split.

### The rule

**Sequence shape is a screen, not a verdict.** It is good at ranking where to look and systematically
wrong about what it found:

- a `success` in the next row can be a **degraded workaround** (TU-11a) — it says the call returned,
  never that the agent got what it wanted;
- a `non-recovery` can be **correct cross-tool behaviour** (TU-11b), and same-tool metrics penalise
  exactly the guards whose right answer is a different tool.

Before filing or closing anything from aggregate counts, read the arguments of ten instances. Both
corrections here came from fewer than a dozen rows, and neither was visible from any amount of
aggregation.

## Still unpulled

> **CLOSED 2026-08-16 — all four items measured.** The list below is kept as written (this is a
> snapshot); results are in § History → *2026-08-16*. Headlines: TU-5's head is classified
> (unclassified 19.8% → 2.1%); latency is `run_command` alone and the missing primitive is *await*,
> not background; overflow is concentrated on two disagreeing axes (rate: `librarian` 39.7%;
> tokens: `grep` 68%); `friction_target` has no rankable head and should stay a `legibility_scan`
> input.

- **TU-5's unclassified head** — extend `normalize_err_family`, then re-rank. Highest value next step.
- **Latency / timeout profile** — never examined; `latency_ms` and the 2-minute client backgrounding
  threshold (CAP-3) interact and neither was measured here.
- **Overflow pressure** — `overflow_tokens` and per-tool overflow rates were only glanced at
  (`run_command` 706, `symbols` 233, `grep` 179).
- **`friction_target`** — populated only on friction rows; never aggregated.

## History

### 2026-08-15 — opened

Investigation run and findings filed in one pass. TU-1 through TU-4 filed as bugs; TU-5 and the
unpulled threads left open; TU-6 closed as already-fixed; TU-7 recorded as a negative result.

### 2026-08-15 — tool-complete sweep added (TU-8 .. TU-10)

Second pass over the same corpus, restricted to **2026-07 onward** and keyed on `tool_name` +
`error_msg` rather than `err_family`, so TU-5's unclassified 31% is inside the window rather than
outside it. That change of key is what surfaced TU-9: `artifact_event`'s conditionally-required
params carry no `err_family` and were invisible to the first pass.

Two of the three new findings are **negative results** (TU-8 routing, and TU-7 before it). That is
not a shortage of defects — it is the sweep doing its job. Roughly 45% of the error volume in this
window is a guard correctly re-routing a wrong-tool call, and the temptation to read that volume as
breakage is precisely what these entries exist to block.

One method caveat was added from this pass and is worth applying elsewhere: **adjacent-call
compliance systematically understates any guard whose correct recovery involves a lookup first.**
Measured on the next call alone, `edit_file`→`edit_code` compliance is 45%; within three calls it is
74%. The first number would have justified a redesign that the second shows is unnecessary.

### 2026-08-15 — TU-5 closed, and its own headline corrected

**The 31% was wrong in the direction this document warns about.** § Method caveats 1–2 say
lifetime aggregates are project-confounded and hide trajectories — and TU-5's headline was a
lifetime aggregate. Splitting the corpus by `PRAGMA user_version` separates two populations that
must not be added together:

| Population | DBs | Unclassified | Why |
|---|---|---|---|
| **Frozen** | `code-explorer.old`, `headroom`, `hermes-agent`, `topictracker`, `opencode` | **660 (77%)** | `user_version=0`, last written May/Jun. `backfill_legacy_rows` only runs *on open*, and nothing opens them again — they never received the iron-law taxonomy and never will. |
| **Live** | `codescout`, `prompt-engineering`, `claude-plugins`, `pi`, `researcher`, `whatsapp` | **197** | `user_version=2`, actively written. |

660 + 197 = 857 exactly. So the actionable rate was **197/997 = 19.8%**, not 31% — the lifetime
figure measured dead history, not the current surface. **Rank on live DBs only.**

**The load-bearing repair was not the arms — it was the version bump.** `backfill_legacy_rows` is
gated on `PRAGMA user_version >= BACKFILL_VERSION`, so extending `normalize_err_family` without
bumping the constant leaves every already-backfilled DB's history NULL and tags only future rows.
That coupling is now pinned by `backfill_reruns_when_the_taxonomy_version_advances`, which seeds a
DB at the *previous* version and asserts a new family appears on reopen. `BACKFILL_VERSION` 2 → 3.

**13 families added**, sized against the live population, covering 168/197 (85%):
`missing_required_param` (38) · `json_path_key_miss` (27) · `librarian_managed_artifact` (25) ·
`target_already_exists` (17) · `heading_not_found` widened to artifact's `body_edits` (14) ·
`path_not_found` (13) · `unknown_enum_value` (8) · `ambiguous_heading` (7) · `buffer_ref_expired` (6) ·
`invalid_regex` (5) · `ambiguous_old_string` (4) · `destructive_replace_blocked` (3) ·
`edit_would_break_syntax` (3) · `edit_markdown_wrong_ext` (1).

Two splits were deliberate, because the *repair* differs and a merged family would make the ranking
undecidable:

- `old_string **not found**` (→ `edit_stale_match`, re-read the file) vs `old_string **found N
  times**` (→ `ambiguous_old_string`, add context).
- `unsupported json_path` (bad syntax) vs `path segment not found` (valid syntax, wrong key for
  *this* buffer's shape).

The residual 29 are genuine one-offs; a family per message would break the low-cardinality contract
the function documents.

**Post-fix ranking (live, 997 errors).** Routing guards still dominate — TU-8's negative result
holds, and strengthens: `il1` 245 · `il3_pipe` 223 · `il3_shell` 111 · `il2` 53. The reordering that
matters is below them: **`missing_required_param` (38 hits / 20 sessions) is now the largest
non-routing family**, which is the priority evidence TU-4 and TU-9's filed bug previously lacked.
`json_path_key_miss` (27 / 17 sessions) is new and **compounds TU-1** — it is the overflow-recovery
hint failing at the next step, where the agent guesses a key (`$.summary` most often) against a
buffer shape that has none.

**Not added, deliberately:** `edit_file is blocked for source code files (debug_enforce_symbol_tools
is enabled)` — 26 lifetime hits but **0 on live DBs**. The message text changed; an arm for it would
be dead code on arrival. Same for `file has no frontmatter block` (14 lifetime, 0 live).

**Still open from § Still unpulled:** latency/timeout profile, per-tool overflow rates,
`friction_target` aggregation. Unchanged by this pass.

#### Verified end-to-end, then a second gap found in the fix itself

After `cargo rb` + `/mcp`, codescout's own DB re-backfilled to `user_version=3`: **184 → 26
unclassified of 923 errors (2.8%)**, against a predicted 2.9%. All 13 families populate on real
rows. The other live DBs remain at v2 until their projects are next activated.

Reading the *new* residual surfaced a defect class the first pass had seen only once and not
named: **the taxonomy was written tool-by-tool, so wherever two tools share a failure mode, only
the first-written tool got an arm.** Three instances, not one:

| Has an arm | Twin with none | Shared failure |
|---|---|---|
| `read_markdown` wrong-ext | `edit_markdown` | non-`.md` path (fixed in v3) |
| `grep` “buffer reference not found” | `run_command` “background job ref not found” | expired `@ref` — *identical hint text* |
| `read_markdown` invalid-line-range | `read_file` | range past EOF / inverted |

v4 closes the latter two: **7 more rows, residual 26 → 19 (2.1%)**. Takes effect on the next
release build. When adding any arm, check the twin tool first — this is now 3-for-3.

#### Correction — what the version test does NOT guarantee

The v3 entry above claimed `backfill_reruns_when_the_taxonomy_version_advances` “fails if someone
adds an arm and forgets the bump.” **That is false, and the weakness survived a rewrite of the
test.** Seeding at `BACKFILL_VERSION - 1` guarantees the backfill runs, so the probe family fills
whether or not the constant was bumped for the new arms. The test proves the *mechanism* re-runs on
advance; it cannot detect a taxonomy that grew without advancing.

No unit test can, while the gate is a hand-maintained integer — the coupling is between the
classifier's *content* and a number no code derives from it. The sound fix is to stop maintaining
it: gate the backfill on a **fingerprint of the emittable family set** rather than an integer, so
adding an arm changes the fingerprint and triggers re-classification with no human step at all.
Until then the bump is a convention held by the comment on `BACKFILL_VERSION`, not by the suite —
and it should be treated as such.

### 2026-08-16 — § Still unpulled closed: latency, overflow, friction_target

Corpus rebuilt after the v4 backfill: 54,250 calls / 464 sessions, of which **21,638 sit on live
DBs** (`user_version >= 2`). All figures below are live-only, per the rule established above.

#### Latency — a negative result, then the real finding underneath it

**Latency is one tool.** Of 1,455 calls over 10s, **1,425 are `run_command`**; all 31 calls over
120s are. Every other tool is effectively fast (`symbols` p-max 45s across 2,933 calls, one call
over 30s; nothing else reaches 30s). Overall avg 2.5s, max **25 minutes**.

So CAP-3 is not a general async problem — it is a `run_command` problem, and `run_command`
**already has `run_in_background`**. The interesting question is therefore not adoption but what
the blocking calls are actually doing:

| >10s `run_command`, args present | Calls | Blocked |
|---|---|---|
| Total | 1,424 | 46,779s (≈13h) |
| · `cargo` build/test/clippy | 1,287 (90%) | 32,699s (≈9.1h) |
| · hand-rolled wait (`sleep`/`seq` loop, `gh run watch`, `@bg_` poll) | 56 (4%) | 10,584s (≈2.9h) |

The extreme tail inverts that ratio. In the **>60s** band, hand-rolled waits are **37% of calls but
64% of blocked time** (35 calls, 10,075s of 15,707s). The longest single call — 25 minutes — is a
`gh run watch` on CI.

**The smoking gun: 18 calls / 2,371s were spent polling a `@bg_` buffer for a job the agent had
already backgrounded.** A `for i in $(seq 1 60); do grep ... @bg_00000011; sleep 5; done` in the
foreground.

**Correction to a reading made during this pass.** `run_in_background` shows 140 uses, *all* in the
`<10s` band, which first reads as "used backwards — never on the slow calls." That is wrong: a
backgrounded call returns immediately **by design**, so `<10s` is exactly where it belongs. The
absence of blocking there is the feature working.

The gap is the other half of the pair: **backgrounding a job creates a need to wait for it, and
there is no await primitive** — so the wait becomes a *second*, foreground, polling call. That, not
"background execution," is what CAP-3 should specify. Evidence appended to CAP-3.

#### Overflow — concentrated on two different axes, which do not agree

1,275 of 21,638 calls overflow (**5.9%**), buffering **8.45M tokens**. TU-10 called overflow
"concentrated"; measured properly, it is concentrated **twice, differently**, and ranking by either
axis alone misleads:

**By rate** — `librarian` **39.7%** (58/146), `semantic_search` **20.9%**, `run_command` 9.9%,
`artifact` 7.9%, `symbols` 7.8%.

The librarian figure is not `context` (1 call, 0 overflow — hypothesis rejected). It is three
actions whose *normal* output exceeds the budget: `link_scan` **8/8 = 100%**, `tracker_design`
**6/6 = 100%**, `audit_doc_refs` **37/50 = 74%**. `tracker_design` is the sharp one — it exists to
teach the caller before they create a tracker, and it has **never once been delivered inline**.

**By tokens** — `grep` alone accounts for **5.78M of 8.45M (68%)** on a 3.0% overflow rate, driven
by a single call that buffered **4,427,639 tokens**: a pattern over `*.json` with `limit: 40`.
`limit` bounds *lines*, and a minified JSON file is one line — so a 40-line cap admitted 4.4M
tokens. Both filed.

#### friction_target — aggregating it is the wrong use

962 rows carry the field across **483 distinct targets** — a mean of 2. There is no head to rank:
the largest is `src/tools/markdown/tests.rs` at 31. The key space is also **heterogeneous** — it
mixes file paths (`src/librarian/tools/doctor.rs`) with bare symbol names (`call`, `tests`,
`sync_worktree`, `stream_index`), so a naive `GROUP BY` groups two different kinds of thing.

This is a negative result in the TU-7 sense: the field is doing its job as a per-row pointer for
`legibility_scan`, which ranks by observed cost against the symbol index. It is not a standalone
ranking surface and should not be turned into one.

§ Still unpulled is now empty.


### 2026-08-16 — the fingerprint gate shipped; TU-5's residual convention is gone

The § *Correction* entry above ended by naming the sound fix and admitting the project did not
have it: *"gate the backfill on a **fingerprint of the emittable family set** rather than an
integer … Until then the bump is a convention held by the comment on `BACKFILL_VERSION`, not
by the suite."* That is now built (BL-4,
`docs/issues/archive/2026-08-16-backfill-gate-not-derived-from-the-taxonomy.md`).

`const BACKFILL_VERSION` is deleted. `PRAGMA user_version` now stores an FNV-1a fingerprint
over `const ERR_FAMILIES: &[&str]` — the 38 families the classifier can emit, which are now
*enumerable* for the first time. The gate compares for equality rather than `>=`, so any DB
carrying an older marker (including the sequential v0–v4 values every real `usage.db` holds)
re-classifies once on open and is then stamped.

**The entry above was right that no unit test could catch this while the gate was an integer.
The missing piece was not a better test but an enumerable family set** — once the list exists,
a guard that reads the file's own source pins it to the classifier in both directions.
Mutation-verified: adding an arm without listing its family fails that guard by name.

Two consequences for anything re-running this investigation's method:

- **"Is this DB current?"** is now `user_version == err_family_fingerprint()`, a derived
  predicate. The § Method rule about filtering to live DBs before aggregating still holds, but
  it no longer depends on trusting that someone bumped a constant.
- **`err_family IS NULL` is no longer ambiguous for any DB that has been opened.** An open
  converges it, so `NULL` means *the classifier has no arm for this*, not *this row predates
  the arm*. The overloading that made TU-5's headline wrong by 11 points survives only in the
  frozen corpora — which this tracker already excludes from ranking.
