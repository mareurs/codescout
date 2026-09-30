---
id: f32b7d248a833167
kind: bug
status: open
title: A labelling packet cuts tool results and tool-call arguments with no marker, so a partial value reads as the whole
tags:
- cluster/capped-result-presented-as-complete
closed: ''
opened: 2026-09-30
owner: marius
related:
- measure-packet-fixed-position-cuts-drop-the-part-the-judgement-needs
severity: high
---

# BUG: a labelling packet cuts tool results and tool-call arguments with no marker, so a partial value reads as the whole

## Summary

`scripts/measure/packet.py` builds the blinded packets the operator labels for the System 1 labelled sample.
It shortens three things silently: a tool result is cut to its last 1,500 characters, a tool call's JSON
arguments to their first 300, and the operator's message to its first 1,500. None of these cuts leaves a
marker, so the labeller cannot tell that text is missing, and a result can begin mid-line. The one cut
that IS marked is the judged message's own (`TRIM_MARKER`, in `_fit`), so the packet marks one kind of cut
in four, and an unmarked cut reads as a complete value.

## Symptom (Effect)

Reported by the sandboxed pilot walkthrough (relayed by the operator, no packet content), item 2:

```
A result shown as its tail loses its start: counts, headers and the first section of multi-part
output. In some packets the cut is unmarked and the text begins mid-line.
```

## Reproduction

Synthetic input only. Never reproduce on a real packet (see Resume).

```
cd scripts/measure && python3 - <<'EOF'
import packet as p
src = "HEADER: 3 matches\n" + "\n".join(f"line {i} " + "x"*40 for i in range(100))
t, _ = p._tail(src, 200); print(repr(t[:30]), "not shown" in t)
h, _ = p._head(src, 60);  print(repr(h[-30:]), "not shown" in h)
print(p._call("Write", {"file_path": "/x/y.md", "content": "BODY " * 200})[0][-20:])
EOF
```

## Environment

`experiments` at `ee313cce`; python3; synthetic strings, no corpus.

## Root cause

- `_result_line` (`scripts/measure/packet.py:151`): `tail, leaked = _tail(text, RESULT_TAIL_CHARS)`.
  `_tail` (`scripts/measure/packet.py:117`) returns the kept slice and a token-overlap flag, and nothing
  else.
- `_call` (`scripts/measure/packet.py:131`): `_head(_blind(json.dumps(inp, ...)), ARGS_CHARS)`, with the
  same shape: the cut JSON is wrapped in `name(...)` with no marker.
- `build_packet` (`scripts/measure/packet.py:336`): `op_text, op_leak = _head(_blind(first),
  OPERATOR_CHARS)`, the operator message (or the dispatch prompt for a hand-back), also unmarked. The
  walkthrough did not name this site, but it has the same mechanism.
- Contrast: `_fit` (`scripts/measure/packet.py:224-236`) prefixes `TRIM_MARKER` when it cuts the judged
  message. So the convention exists and three sites skip it.

measured 2026-09-30 with the reproduction above: the tail begins `'xxxx\nline 96 …'` (mid-line) with no
marker; the head ends mid-line with no marker; the `Write` call's arguments end `BOD)` with no marker, and
the `HEADER: 3 matches` line is gone.

## Evidence

### Probe output (2026-09-30)

```
TAIL starts: 'xxxx\nline 96 xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\nline 9' | marker present: False
HEAD ends: 'ne 0 xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx' | marker present: False
call: ('Write({"file_path": "/x/y.md", "content": "BODY BODY … BODY BOD)', False)
```

## Hypotheses tried

None. The mechanism was read from the code and confirmed by the probe.

## Fix

Plan (not implemented):

1. At every `_head`/`_tail` call site that cuts, add a marker that says what was dropped and how much,
   for example `[… 840 earlier characters not shown]` before a tail and `[… 1,200 more characters not
   shown]` after a head. The exact wording is a choice. Keep it distinct from `TRIM_MARKER`, which is
   about the judged message.
2. The marker counts against `PACKET_CHARS`. The cap tests must still hold.
3. The token-overlap refusal (`_overlaps`, the `leaked` flag) must keep judging the kept source range.
   The marker is not source text.
4. Coordinate with `measure-packet-fixed-position-cuts-drop-the-part-the-judgement-needs`: if head plus
   tail is ruled there, the marker sits between the two parts.

Record SHA and patch-id at fix time.

## Tests added

None yet. Owed, per site (result tail, argument head, operator or dispatch head): a cut input shows the
marker, and an uncut input shows none. Both directions are needed, because the absence assertion alone is
monotone under removal. Then one mutation per site (`./scripts/mutation-probe.sh`).

## Workarounds

None. Do not label packets rendered before the fix; labels are bound to the packet sha256, so a
re-rendered packet needs a fresh label anyway.

## Resume

Write the per-site failing tests in `tests/test_measure_packet.py` against the synthetic fixture
(`tests/measure_corpus_fixture.py`), then add the markers.

**Constraints for whichever session fixes this (shared by the six 2026-09-30 `measure-*` bugs):**

- **Deadline.** Spec `docs/superpowers/specs/2026-09-29-system1-labelled-sample-design.md` L110-111: the
  packet rule may change after the pilot and before registration (Amendment 1), never after. Land this
  before Amendment 1 is committed.
- **Frozen file.** `packet.py` is one of four files (`sampler.py`, `packet.py`, `estimate.py`, `run.py`)
  whose sha256 `run.py frame` records. Any edit invalidates the frame file, and `estimate` refuses a
  mismatch. Batch every `packet.py` fix into one wave. Afterwards the order is `frame`, then `preflight`,
  then re-render the pilot, all on the real corpus. Ask the operator who runs them.
- **Blindness.** Never open a real packet, `key.json`, labels or anything under
  `~/work/claude/measurement-corpora/`. Test only on the synthetic fixture. Only counts-only
  `run.py`/`label.py` output may be read.
- **Tests.** Write the test first. Run one mutation per guarded site. Run all `tests/test_measure_*.py`
  (768 passed, 357 subtests at `ee313cce`). Before a commit under `scripts/`, run
  `scripts/with-slot.sh cargo test --test committed_paths` (R39).
- **Siblings:** `measure-packet-fixed-position-cuts-drop-the-part-the-judgement-needs`,
  `measure-packet-blinding-patterns-miss-a-value-glued-to-a-word-character`,
  `measure-packet-placeholders-merge-distinct-ids-into-one-token`,
  `measure-packet-a-tool-only-message-reads-as-missing-content`,
  `measure-label-recall-flag-does-not-say-whether-wanting-the-outcome-counts`.

## References

- Spec: `docs/superpowers/specs/2026-09-29-system1-labelled-sample-design.md` § packet rule (L120-132).
- Plan: `docs/superpowers/plans/2026-09-29-system1-labelled-sample.md` (Task 6).
- Walkthrough handoff: `docs/trackers/2026-09-29-system1-pilot-walkthrough-handoff.md`.
- SDD ledger (git-ignored): `.superpowers/sdd/2026-09-29-system1-labelled-sample/progress.md`, PILOT
  FINDING #2.
