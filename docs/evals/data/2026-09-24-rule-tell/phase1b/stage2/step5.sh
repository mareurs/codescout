#!/usr/bin/env bash
# Phase-1b Stage 2, Step 5: the gate, once per checkpoint, on its own final menu and on the common menu
# (step5_gate.py), then the registered readings (--summary). Registered order: B, N, NC at each seed.
# step5_gate.py refuses a checkpoint whose result exists, so a relaunch cannot re-run a gate.
set -u
REPO=/home/marius/work/claude/codescout
P=$REPO/docs/evals/data/2026-09-24-rule-tell/phase1b
PY=/home/marius/work/claude/jevk5/.venv/bin/python
OUT=/home/marius/work/claude/rule-tell-runs/phase1b-s2
RES=$P/stage2
GATE=$RES/gate

cd "$P" || exit 2
git -C "$REPO" rev-parse HEAD > "$OUT/step5-commit.txt"
sha256sum step5_gate.py "$REPO/scripts/phase1-span-selector.py" "$RES/common.json" >> "$OUT/step5-commit.txt"
fail=0
for seed in 20260935 20260937 20260940; do
  for arm in b n nc; do
    name=$arm-$seed
    "$PY" -u step5_gate.py --step4 "$RES/step4-$name.json" --scored "$OUT/scored-$name.json" \
      --common "$RES/common.json" --out-dir "$GATE" >> "$OUT/step5.log" 2>> "$OUT/step5-stderr.log"
    rc=$?; echo "gate $name exit $rc" >> "$OUT/step5.log"
    [ $rc = 0 ] || fail=1
  done
done
if [ $fail = 0 ]; then
  "$PY" step5_gate.py --summary "$GATE" --out "$GATE/summary.json" >> "$OUT/step5.log" 2>> "$OUT/step5-stderr.log"
  echo "summary exit $?" >> "$OUT/step5.log"
fi
echo "STEP5_DONE fail=$fail" >> "$OUT/step5.log"
