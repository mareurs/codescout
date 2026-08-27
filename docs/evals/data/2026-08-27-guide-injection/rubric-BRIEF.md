# BRIEF — measuring codescout guide auto-injection in Claude Code transcripts

You are ONE of 10 agents running the SAME procedure on DIFFERENT transcripts.
Your numbers get aggregated with nine others. **Deviating from this rubric silently
corrupts the aggregate.** If something here cannot be done on your file, say
`UNMEASURABLE` with a reason — never substitute `0`.

---

## 0. Context you are NOT expected to rediscover

codescout is an MCP server (Rust) at `/home/marius/work/claude/codescout`. It ships
10 "guide" documents compiled into the binary (`src/prompts/guides/*.md`). Two
delivery paths:

- **pull** — the agent calls `get_guide(topic)` explicitly.
- **push (dominant)** — `Tool::relevant_guide_topic()` fires on a tool call and
  `Tool::call_content` appends the **entire guide body** to that tool's response as
  a second content block. Once per topic per session (deduped by a per-session
  ledger shared between a parent session and its subagents).

There is **no byte budget on the push path**: a 34 KB guide is appended whole.

The open question — filed as `docs/issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md` —
is that delivery has been measured but **use never has**. That is your job.

### Guide corpus (exact bytes on disk, 2026-08-27)

| topic | bytes |
|---|---|
| tracker-conventions | 34333 |
| librarian | 20545 |
| iron-laws-detail | 11238 |
| workspace-state | 10355 |
| librarian-runtime | 9774 |
| progressive-disclosure | 5669 |
| untrusted-content | 5317 |
| symbol-navigation | 3145 |
| project-activation-bootstrap | 2594 |
| error-handling | 1857 |
| **total** | **104827** |

Per-section byte tables (fence-aware): `../scratchpad/guide_sections.json` — full path
`/tmp/claude-1000/-home-marius-work-claude-codescout/2e058ff8-36e7-457d-96da-e5fd34d33a31/scratchpad/guide_sections.json`.
Guide source text: `/home/marius/work/claude/codescout/src/prompts/guides/<topic>.md`.

---

## 1. Your input

Your dispatch message names `AGENT_ID`, `TRANSCRIPT` (absolute path), and
**calibration values** (`EXPECTED_INJECTIONS`, `EXPECTED_TOPICS`) measured by the
controller with a verified instrument.

**The transcripts are 0.75-5 MB. NEVER read one into context.** Write Python and
run it via `run_command`. Read only targeted line slices for quoting evidence.

---

## 2. Parsing contract (verified by the controller — do not re-derive)

Each line is one JSON object.

- `d["type"]` is one of `user | assistant | attachment | system | last-prompt | mode |
  permission-mode | atis-latch | ai-title`
- `d["message"]["model"]` exists on `assistant` lines (e.g. `claude-opus-5`).
- `d["timestamp"]`, `d["uuid"]`, `d["parentUuid"]` order the conversation.
- `d["message"]["content"]` is a **string OR a list of blocks**. Handle both.
- Block types you care about: `text`, `tool_use` (on assistant lines, has `id`,
  `name`, `input`), `tool_result` (on **user** lines, has `tool_use_id`, `content`).

**Guide injections land on `type:"user"` lines, inside a `tool_result` block.**

Markers:

```
opening:  <!-- auto-injected get_guide('TOPIC') — first call this session that triggers the topic. Do NOT re-call get_guide for this topic. -->
closing:  <!-- end auto-injected get_guide('TOPIC') -->
```

### Three traps that have already produced wrong numbers

1. **Every injection contains BOTH markers.** Counting occurrences of
   `auto-injected get_guide(` double-counts. Count the **opening** form only (it is
   followed by ` — first call`; the closing form is preceded by `end `).
2. **`{topic}` and `{}` are false positives** — they are the literal Rust format
   string from `src/tools/core/types.rs`, appearing in transcripts where somebody
   read that source. Accept only the 10 real topic names in the table above.
3. **A marker in assistant `text` or in a user's typed message is a DISCUSSION
   mention, not an injection.** Only a marker inside a `tool_result` block counts.
   Report discussion mentions separately as `discussion_mentions`.

### Pairing an injection to its trigger

The `tool_result` carries `tool_use_id`. Find the `tool_use` block with that `id` on
an earlier `assistant` line — that gives you the **triggering tool name and input**.
If pairing fails, record `trigger_tool: "UNPAIRED"` — do not guess.

---

## 3. FIRST ACTION — calibrate (mandatory gate)

Run your parser and compare against `EXPECTED_INJECTIONS` / `EXPECTED_TOPICS`.

- **Match** then proceed, and report `calibration: "PASS"`.
- **Mismatch** then **STOP measuring.** Report `calibration: "FAIL"` with your count,
  the expected count, and your best diagnosis. A silently-diverging parser is worse
  than no data. Do not "adjust" the expected value until it matches.

---

## 4. What to measure

### M1 — HOW MUCH

- `injections`: count, and per-topic counts.
- `injected_bytes`: sum of the guide byte sizes (table above) over all injections,
  counting **repeats** (same topic injected twice = twice the bytes).
- `repeat_injections`: injections of a topic already injected earlier in this
  transcript. Note the gap in assistant turns and, if visible, the apparent cause
  (MCP reconnect, `/clear`, project switch).
- `session_total_chars`: total characters of all `text` and `tool_result` content
  in the transcript (your denominator).
- `injected_share`: `injected_bytes / session_total_chars`.
- `assistant_turns`: count of `assistant` lines.

### M2 — RIGHT TIME

For **each** injection record:

- `turn_index`: how many `assistant` lines precede it, and `turn_pct` of total.
- `first_opportunity_turn`: the FIRST assistant turn (anywhere in the transcript)
  containing a tool call of the class that guide governs — see the class table in
  section 5. `null` if none.
- `timing_verdict`, exactly one of:
  - `LATE` — `first_opportunity_turn < turn_index` (the session already did the
    governed thing before the guide arrived).
  - `TIMELY` — an opportunity exists at or after `turn_index`.
  - `NEVER_NEEDED` — no opportunity anywhere in the transcript. The guide arrived
    and the class it governs never occurred.
- `opportunities_after`: count of governed-class calls after the injection.

### M3 — WAS IT USED

Classify each injection into exactly one `use_class`, highest that applies:

- **`U3_CITED`** — later assistant *text* names the topic, names `get_guide`, or
  states a rule attributing it to the guide.
- **`U2_PRESCRIBED_CALL`** — a later tool call matches a call shape the guide
  prescribes (section 5) **and** that shape does not appear anywhere BEFORE the
  injection. The before-check is what separates *use* from *the agent already did it*.
- **`U1_ECHO`** — later assistant text contains a distinctive **6-or-more
  consecutive words** from the guide body that appear nowhere in the transcript
  before the injection.
- **`U0_UNUSED`** — none of the above.

Also, independently (a flag, not a class):

- **`contradicted`: true** — a later action violates an explicit rule of the
  injected guide (e.g. `tracker-conventions` injected, then a bare `git mv` on a
  tracker; `progressive-disclosure` injected, then the same tool re-run instead of
  reading its `@ref` buffer). **This is the most decision-relevant negative — look
  for it deliberately.**

**Every non-`U0` classification REQUIRES evidence**: `{"line": N, "quote": "<=25 words"}`.
No evidence means it is `U0`. You may not classify from impression.

### M4 — HOW MUCH OF IT WAS USED

For each injection classified U1/U2/U3, map each piece of evidence to the guide
`##` section it comes from (use `guide_sections.json`).

- `sections_touched`: list of section names.
- `bytes_touched`: summed bytes of those sections.
- `utilisation`: `bytes_touched / topic_bytes`.

For `U0_UNUSED`, `utilisation` is `0.0`.

---

## 5. Governed class + prescribed call shapes (per topic)

Used for BOTH `first_opportunity_turn` (M2) and `U2` (M3).

| topic | governed class (opportunity) | prescribed shapes (U2) |
|---|---|---|
| `progressive-disclosure` | any tool response containing `output_id` / `@tool_` / `@cmd_` / `@file_` | reading an `@ref` (`read_file("@tool_...")`, `grep ... @cmd_...`), `json_path=` |
| `librarian` | any `artifact` / `librarian` / `artifact_*` call | `filter={field:{op:val}}` leaf form, `entry_filter=`, `append_entry` / `update_entry`, `artifact(action="move")`, `scope=` |
| `tracker-conventions` | any call touching `docs/issues/` or `docs/trackers/` | `append_entry` with `anchor_heading`+`title`+`body`, `filter={"status":{"in":[...]}}`, `artifact(action="move")` to archive, `entry_prefix`, `**Valid:**` / `**Rests on:**` written into a body |
| `symbol-navigation` | any `symbols` / `references` / `call_graph` / `symbol_at` call | `name_path=`, `include_body=true`, `references(...)` or `call_graph(direction=...)` instead of grepping for callers |
| `workspace-state` | any `workspace` call, or any call carrying `workspace=` | `workspace(action="activate")` restoring home before turn end, per-call `workspace=` pin |
| `project-activation-bootstrap` | session start (always an opportunity; `first_opportunity_turn` = 0) | `memory(action="list"/"read")`, `artifact(find, kind="bug", ...)` early, `semantic_search` for a concept, `references` for callers |
| `error-handling`, `iron-laws-detail`, `untrusted-content`, `librarian-runtime` | (never yet observed to auto-inject; if you see one, flag it prominently) | — |

---

## 6. Output

Write **strict JSON** to
`/tmp/claude-1000/-home-marius-work-claude-codescout/2e058ff8-36e7-457d-96da-e5fd34d33a31/scratchpad/results/<AGENT_ID>.json`,
then reply with a 200-word-max summary (the JSON is the deliverable; your prose is
not aggregated).

```json
{
  "agent_id": "OPUS-3",
  "transcript": "/abs/path.jsonl",
  "calibration": "PASS",
  "calibration_detail": "found N injections vs expected M",
  "model": "claude-opus-5",
  "project": "<the -home-... dir name>",
  "assistant_turns": 0,
  "session_total_chars": 0,
  "injections": 0,
  "per_topic": {},
  "injected_bytes": 0,
  "injected_share": 0.0,
  "repeat_injections": 0,
  "discussion_mentions": 0,
  "per_injection": [
    {
      "topic": "librarian",
      "line": 0, "turn_index": 0, "turn_pct": 0.0,
      "trigger_tool": "artifact", "trigger_summary": "action=find",
      "topic_bytes": 20545,
      "timing_verdict": "TIMELY",
      "first_opportunity_turn": 0,
      "opportunities_after": 0,
      "use_class": "U0_UNUSED",
      "evidence": [{"line": 0, "quote": ""}],
      "sections_touched": [],
      "bytes_touched": 0,
      "utilisation": 0.0,
      "contradicted": false,
      "contradiction_evidence": null
    }
  ],
  "notes": "anything that made a number unreliable; UNMEASURABLE items"
}
```

---

## 7. Rules of evidence (non-negotiable)

1. **Line numbers on every claim.** A classification without `{line, quote}` is `U0`.
2. **`UNMEASURABLE` is not `0`.** Truncated content, unpaired `tool_use_id`, a
   transcript that ends mid-turn: say so in `notes`.
3. **Do not read the transcript into context.** Python plus targeted slices only.
4. **Do not infer use from plausibility.** "The agent used `append_entry` later, so
   it probably read the guide" is `U2` ONLY if `append_entry` appears nowhere before
   the injection. Check.
5. **Report the awkward result.** If your transcript shows 0% utilisation across
   every injection, that IS the finding. Do not hunt for a nicer number.
6. You have codescout MCP tools. Guide topics already triggered in the controller's
   session: `project-activation-bootstrap`, `symbol-navigation`, `librarian`,
   `progressive-disclosure`, `tracker-conventions`. Use `run_command` for Python and
   `read_file` with `start_line`/`end_line` for quoting.
