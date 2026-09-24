#!/usr/bin/env bash
# Score B for S0, as registered in 622459bf. Forks on ~/.claude, then every compared arm
# re-scored on the clean judge channel. Stages log to $S/e2s-*.txt.
set -u
S=/tmp/claude-1000/-home-marius-work-claude-codescout/571eb3d6-c879-43f6-b3f9-5a51e744e1af/scratchpad
REPO=/home/marius/work/claude/codescout
P=$HOME/.claude/projects/-home-marius-work-claude-codescout
SRC=$HOME/.claude-kat/projects/-home-marius-work-claude-codescout/571eb3d6-c879-43f6-b3f9-5a51e744e1af.jsonl
PREFIX=$P/p2src-571eb3d6-prefix.jsonl
cd "$REPO"
# phase2-replay.py (imported by phase2-fork.py for ARMS) imports `anthropic`; the system
# python lacks it. Import only: the fork route makes no API call.
PY=/home/marius/work/claude/prompt-engineering/.venv/bin/python
rm -f $S/e2s-score-*.txt $S/fork-e2s-dp1.jsonl $S/fork-e2s-rtd3.jsonl

# 1. the transcript prefix where --resume resolves seeds (records 0..1781 cover both cuts)
head -n 1782 "$SRC" > "$PREFIX"
echo "prefix lines: $(wc -l < "$PREFIX")"

# 2. forks, both decision points concurrently (pool 2 each)
( CLAUDE_CONFIG_DIR=$HOME/.claude $PY scripts/phase2-fork.py --transcript "$PREFIX" --cut 1780 \
    --dynamic $S/e2s-dp1.jsonl --dynamic-label e2s --workdir $S/forkwork-e2s-dp1 \
    --out $S/fork-e2s-dp1.jsonl --pool 2 > $S/e2s-dp1-fork.txt 2>&1; echo "exit=$?" >> $S/e2s-dp1-fork.txt ) &
( CLAUDE_CONFIG_DIR=$HOME/.claude $PY scripts/phase2-fork.py --transcript "$PREFIX" --cut 1493 \
    --prompt-records 1494,1495 --inject-in-prompt \
    --dynamic $S/e2s-rtd3.jsonl --dynamic-label e2s --workdir $S/forkwork-e2s-rtd3 \
    --out $S/fork-e2s-rtd3.jsonl --pool 2 > $S/e2s-rtd3-fork.txt 2>&1; echo "exit=$?" >> $S/e2s-rtd3-fork.txt ) &
wait
rm -f "$PREFIX"
echo "prefix removed: $([ -e "$PREFIX" ] && echo NO || echo yes)"
# Never score a partial fork stage: a dead fork stage once flowed straight into scoring.
for f in fork-e2s-dp1 fork-e2s-rtd3; do
  n=$(cat $S/$f.jsonl 2>/dev/null | wc -l)
  [ "$n" -eq 10 ] || { echo "REFUSING TO SCORE: $f.jsonl has $n rows, want 10"; exit 3; }
done

# 3. one clean channel for every compared arm
export JUDGE_CONFIG_DIR=$S/judge-config-main
cat $S/fork-dp1-n10.jsonl $S/fork-e2s-dp1.jsonl > $S/score-b-dp1-rows.jsonl
cat $S/fork-rtd3r.jsonl $S/fork-e2s-rtd3.jsonl > $S/score-b-rtd3-rows.jsonl
for r in "rtd8 rtd8-corrected.txt" "rtd9 rtd-9-corrected.txt" "rtd10 rtd-10-corrected.txt"; do
  set -- $r
  $PY scripts/phase2-score-dp1.py --rule $1 --request $S/dp1-request.json --corrected $S/$2 \
    --replays $S/score-b-dp1-rows.jsonl --arms 0,1b,e2s > $S/e2s-score-$1.txt 2>&1
  echo "exit=$?" >> $S/e2s-score-$1.txt
done
$PY scripts/phase2-score-dp1.py --rule rtd3r --request $S/rtd3-recorded.json \
  --corrected $S/rtd-3-corrected.txt --replays $S/score-b-rtd3-rows.jsonl --arms 0,3-1b,e2s \
  > $S/e2s-score-rtd3r.txt 2>&1
echo "exit=$?" >> $S/e2s-score-rtd3r.txt
echo PIPELINE-DONE
