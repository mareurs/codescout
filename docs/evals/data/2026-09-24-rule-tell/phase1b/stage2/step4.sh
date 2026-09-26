#!/usr/bin/env bash
# Phase-1b Stage 2, Step 4 for all nine checkpoints (B from Stage 1, N and NC from lanes.sh), in the
# registered order: score_run.py (one cell set: the same admission and counterexample files for every
# arm; parity-checked against each run's fold-logits.json), then step4.py per run, then --common.
# Waits for lanes.sh and refuses unless all six training runs exited 0.
set -u
REPO=/home/marius/work/claude/codescout
D=$REPO/docs/evals/data/2026-09-24-rule-tell
P=$D/phase1b
PY=/home/marius/work/claude/jevk5/.venv/bin/python
S1=/home/marius/work/claude/rule-tell-runs/phase1b-s1
OUT=/home/marius/work/claude/rule-tell-runs/phase1b-s2
RES=$P/stage2
ADM=$P/audit/admission.json
CX=$P/audit/counterexamples.jsonl
declare -A PIN=(
  [20260935]=dd89ea46236173d2fba66616c868aa5a46db4edf5575a262f79a5b1b2f710c05
  [20260937]=5fafbcc61569163f9284ce0b5a90d70f35a9e335aaf7b864c44399e60bf387a3
  [20260940]=e8690c86cff4fb1607f63c28040f860f487ee21d3348b82dee46c50fff00442f
)

until grep -q 'LANES_EXIT=' "$OUT/lanes.out" 2>/dev/null; do sleep 60; done
ok=$(grep -c ' exit 0$' "$OUT/lanes.log")
if [ "$ok" != 6 ]; then echo "refused: $ok of 6 training runs exited 0" | tee -a "$OUT/step4.log" >&2; exit 2; fi
if [ -e "$RES" ]; then echo "refused: $RES exists" | tee -a "$OUT/step4.log" >&2; exit 2; fi
mkdir -p "$RES"

cd "$P" || exit 2
fail=0
for seed in 20260935 20260937 20260940; do
  for spec in "b|$S1/s1-r1-$seed|--expect-sha256 ${PIN[$seed]}" "n|$OUT/s2-n-$seed|" "nc|$OUT/s2-nc-$seed|"; do
    IFS='|' read -r arm run extra <<< "$spec"
    name=$arm-$seed
    # shellcheck disable=SC2086
    "$PY" -u score_run.py --run-dir "$run" --cross "$ADM" --extra-rows "$CX" $extra \
      --out "$OUT/scored-$name.json" >> "$OUT/step4.log" 2>> "$OUT/step4-stderr.log"
    rc=$?; echo "score $name exit $rc" >> "$OUT/step4.log"
    [ $rc = 0 ] || { fail=1; continue; }
    "$PY" step4.py --scored "$OUT/scored-$name.json" --out "$RES/step4-$name.json" >> "$OUT/step4.log" 2>> "$OUT/step4-stderr.log"
    rc=$?; echo "step4 $name exit $rc" >> "$OUT/step4.log"
    [ $rc = 0 ] || fail=1
  done
done
if [ $fail = 0 ]; then
  "$PY" step4.py --common "$RES"/step4-*.json --out "$RES/common.json" >> "$OUT/step4.log" 2>> "$OUT/step4-stderr.log"
  echo "common exit $?" >> "$OUT/step4.log"
fi
echo "STEP4_DONE fail=$fail" >> "$OUT/step4.log"
