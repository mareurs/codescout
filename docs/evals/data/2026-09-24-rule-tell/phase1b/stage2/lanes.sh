#!/usr/bin/env bash
# Phase-1b Stage 2, Step 3: arms N (cross-rule term) and NC (N plus counterexamples),
# recipe s1-r1, seeds 20260935/37/40. Two lanes on one GPU, as Stage 1 ran its two recipes.
# Registered in docs/evals/phase1b-local-classifier-preregistration.md, § Stage 2.
set -u
REPO=/home/marius/work/claude/codescout
D=$REPO/docs/evals/data/2026-09-24-rule-tell
PY=/home/marius/work/claude/jevk5/.venv/bin/python
OUT=/home/marius/work/claude/rule-tell-runs/phase1b-s2
ADM=$D/phase1b/audit/admission.json
CX=$D/phase1b/audit/counterexamples.jsonl
SEEDS="20260935 20260937 20260940"

# train_arm appends to log.jsonl and reuses the directory, so a relaunch would mix two runs' evidence.
for arm in n nc; do for seed in $SEEDS; do
  if [ -e "$OUT/s2-$arm-$seed" ]; then echo "refused: $OUT/s2-$arm-$seed exists" >&2; exit 2; fi
done; done
git -C "$REPO" rev-parse HEAD > "$OUT/commit.txt"
sha256sum "$D/stage3/train_arm.py" "$ADM" "$CX" >> "$OUT/commit.txt"

cd "$D/stage3" || exit 2
lane() {
  arm=$1; shift
  for seed in $SEEDS; do
    run=$OUT/s2-$arm-$seed
    mkdir -p "$run"
    "$PY" -u train_arm.py --arm qwen --recipe s1-r1 --seed "$seed" --cross "$ADM" "$@" --out "$run" \
      > "$run/stdout.log" 2> "$run/stderr.log"
    echo "train $arm $seed exit $?" >> "$OUT/lanes.log"
  done
  echo "lane $arm done" >> "$OUT/lanes.log"
}
lane n &
lane nc --extra-rows "$CX" &
wait
echo "all lanes done" >> "$OUT/lanes.log"
