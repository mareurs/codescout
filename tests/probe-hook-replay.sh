#!/usr/bin/env bash
# tests/probe-hook-replay.sh — scripts/probe-hook-replay.py, against a synthetic history.
#
# WHAT THE PROBE IS FOR
#
# Changing a commit hook raises one question before anything else: would the new rule have refused
# real past commits that the old rule accepted? (`docs/conventions/changing-an-enforcement-gate.md`
# step 3.) Answering it by re-deriving "when would this gate have fired" is the mistake that
# overcounted 29 as 55 on 2026-10-16. This probe replays by EXECUTION instead: for each historical
# commit it builds a throwaway repo whose HEAD is the commit's parent and whose index is the commit's
# tree, runs the candidate hook and a baseline in it, and compares their exit codes. The hook is its
# own firing oracle.
#
# THE TOY HOOKS. `base.py` refuses a staged `docs/` file that is empty; `cand.py` refuses one that
# contains TODO and accepts an empty one. So the two disagree in BOTH directions on purpose, and
# every cell of the 2x2 below is hand-derived from the history, not computed by the probe:
#
#                         candidate accepts   candidate refuses
#     baseline accepts    c1 (ok), c4 (fine)   c2 (TODO)         <- the false-refusal population
#     baseline refuses    c3 (empty)           —
#
# `dump.py` prints exactly what a hook can see — `git diff --cached --name-status` — and refuses, so
# a case can compare the reconstructed index against `git show --name-status` of the real commit.
# That is the FIDELITY case, and it is the one that matters most: a probe whose throwaway repo is not
# the repo the hook really saw reports a clean-looking number about a different question.
set -uo pipefail

SELF_ROOT=$(cd "$(dirname "$0")/.." && pwd)
PROBE="$SELF_ROOT/scripts/probe-hook-replay.py"
PASS=0
FAIL=0

eq() { # eq <label> <actual> <expected>
    if [ "$2" = "$3" ]; then
        PASS=$((PASS + 1))
    else
        FAIL=$((FAIL + 1)); echo "FAIL: $1"; echo "  actual:   $2"; echo "  expected: $3"
    fi
}
has() { # has <label> <haystack> <needle>
    if printf '%s' "$2" | grep -qF -- "$3"; then
        PASS=$((PASS + 1))
    else
        FAIL=$((FAIL + 1)); echo "FAIL: $1"; echo "  expected to find: $3"; printf '  in: %s\n' "$2" | head -6
    fi
}
# jq-free JSON reads: the probe's --json output through python, one expression per call.
js() { python3 -I -c "import json,sys; d=json.load(sys.stdin); print($1)"; }

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

R="$WORK/repo"
mkdir -p "$R/docs" "$R/scripts"
git -C "$R" init -q
git -C "$R" config user.email t@t; git -C "$R" config user.name t

cat > "$R/scripts/base.py" <<'PY'
import subprocess, sys
names = subprocess.run(["git", "diff", "--cached", "--name-only", "--diff-filter=AM"],
                       capture_output=True, text=True).stdout.split()
bad = [n for n in names if n.startswith("docs/")
       and subprocess.run(["git", "show", ":" + n], capture_output=True, text=True).stdout == ""]
if bad:
    print("empty:", bad, file=sys.stderr); sys.exit(1)
PY
cat > "$R/scripts/cand.py" <<'PY'
import subprocess, sys
names = subprocess.run(["git", "diff", "--cached", "--name-only", "--diff-filter=AM"],
                       capture_output=True, text=True).stdout.split()
bad = [n for n in names if n.startswith("docs/")
       and "TODO" in subprocess.run(["git", "show", ":" + n], capture_output=True, text=True).stdout]
if bad:
    print("todo:", bad, file=sys.stderr); sys.exit(1)
PY
cat > "$R/scripts/dump.py" <<'PY'
import subprocess, sys
sys.stderr.write(subprocess.run(["git", "diff", "--cached", "--name-status"],
                                capture_output=True, text=True).stdout)
sys.exit(1)
PY

# Distinct, increasing dates so `--since` has something to cut on. Every commit is made WITHOUT
# running any hook: the history is being replayed, not enforced.
commit() { # commit <date> <message>
    GIT_AUTHOR_DATE="$1T12:00:00" GIT_COMMITTER_DATE="$1T12:00:00" git -C "$R" commit -q -m "$2"
}
git -C "$R" add -A;                                                   commit 2026-01-01 "c0 root: hooks only"
printf 'ok\n' > "$R/docs/a.md";           git -C "$R" add -A;         commit 2026-02-01 "c1 ok"
printf 'TODO later\n' > "$R/docs/b.md";  git -C "$R" add -A;         commit 2026-03-01 "c2 todo"
: > "$R/docs/c.md";                       git -C "$R" add -A;         commit 2026-04-01 "c3 empty"
printf 'fine\n' > "$R/docs/a.md";         git -C "$R" add -A;         commit 2026-05-01 "c4 fine"
git -C "$R" rm -q docs/a.md;                                          commit 2026-06-01 "c5 delete a"
printf 'x\n' > "$R/scripts/other.txt";    git -C "$R" add -A;         commit 2026-07-01 "c6 outside docs"

replay() { python3 "$PROBE" --repo "$R" --paths docs --json "$@" 2>"$WORK/err"; }

# --- 1. The 2x2, from the hook's own verdicts --------------------------------------------------
OUT=$(replay --script "$R/scripts/cand.py" --baseline-script "$R/scripts/base.py"); RC=$?
eq "1 exits 0 when a replay ran" "$RC" "0"
eq "1 replays the five commits that touch docs/, not the root and not c6" \
    "$(printf '%s' "$OUT" | js "d['replayed']")" "5"
eq "1 baseline-accepts/candidate-accepts" "$(printf '%s' "$OUT" | js "d['pairs']['0,0']")" "3"
eq "1 baseline-accepts/candidate-REFUSES (the false-refusal population)" \
    "$(printf '%s' "$OUT" | js "d['pairs']['0,1']")" "1"
eq "1 baseline-refuses/candidate-accepts (a loosening)" "$(printf '%s' "$OUT" | js "d['pairs']['1,0']")" "1"
eq "1 both refuse" "$(printf '%s' "$OUT" | js "d['pairs']['1,1']")" "0"
eq "1 names the commit the candidate newly refuses" \
    "$(printf '%s' "$OUT" | js "[e['subject'] for e in d['events'] if e['base_rc']==0 and e['cand_rc']!=0]")" "['c2 todo']"

# --- 2. FIDELITY: the index the hook saw is the commit's own staged change ----------------------
OUT=$(replay --script "$R/scripts/dump.py" --baseline-script "$R/scripts/dump.py")
for c in c1 c2 c3 c4 c5; do
    sha=$(git -C "$R" log --format=%H --grep="^$c " -1)
    want=$(git -C "$R" show --name-status --format= "$sha")
    got=$(printf '%s' "$OUT" | js "[e['cand_stderr_head'] for e in d['events'] if e['subject'].startswith('$c ')][0].rstrip()")
    eq "2 $c: the replayed index is exactly the commit's name-status" "$got" "$want"
done

# --- 3. --paths limits the population ----------------------------------------------------------
OUT=$(python3 "$PROBE" --repo "$R" --paths scripts --json --script "$R/scripts/cand.py" --baseline-script "$R/scripts/base.py" 2>/dev/null)
eq "3 --paths scripts replays the commits that touch scripts/ (c6; c0 is the root)" \
    "$(printf '%s' "$OUT" | js "d['replayed']")" "1"

# --- 4. The default baseline is the SAME path at a revision, not a second script ---------------
# The usual call: edit the hook in a worktree, replay it against what is committed. Commit base.py's
# logic as scripts/hook.py, put cand.py's logic in the working tree, and ask for no baseline script.
cp "$R/scripts/base.py" "$R/scripts/hook.py"; git -C "$R" add scripts/hook.py; commit 2026-08-01 "c7 add hook"
cp "$R/scripts/cand.py" "$R/scripts/hook.py"
OUT=$(replay --script "$R/scripts/hook.py")
eq "4 baseline defaults to HEAD:scripts/hook.py (the committed base logic)" \
    "$(printf '%s' "$OUT" | js "d['pairs']['0,1']")" "1"
eq "4 and the loosening is seen the same way" "$(printf '%s' "$OUT" | js "d['pairs']['1,0']")" "1"
git -C "$R" checkout -q -- scripts/hook.py

# --- 5. A replay of nothing must not look like a clean replay ----------------------------------
# Every reading here is an EXISTENCE report over a selection: a probe that matched no commits prints a
# clean-looking zero, which is indistinguishable from a healthy history (docs/PROBES.md).
# The window is 2027-01-01 and NOT a far-future year: measured 2026-10-16, git 2.56.0,
# `git log --since=2999-01-01` returns the NEWEST commit (as does 2100-01-01), while 2027-01-01 returns
# none. An "empty window" built from 2999 silently replays everything and the case would pass or
# fail for a reason that has nothing to do with the probe.
OUT=$(replay --script "$R/scripts/cand.py" --baseline-script "$R/scripts/base.py" --since 2027-01-01); RC=$?
eq "5 replaying zero commits exits non-zero (3, which is neither python's 'no such file' nor argparse's usage error, both 2)" "$RC" "3"
has "5 and says why" "$(cat "$WORK/err")" "replayed 0 commits"

# --- 6. A baseline that refuses almost everything means the reconstruction is wrong ------------
OUT=$(replay --script "$R/scripts/cand.py" --baseline-script "$R/scripts/dump.py")
has "6 warns when the baseline refuses nearly every replayed commit" "$(cat "$WORK/err")" "baseline refused"

# --- 7. --jobs is a speed-up and nothing else ----------------------------------------------------
# Each commit is replayed in its own throwaway repo, so they are independent and can run in parallel. The
# cost is the hook under test (measured 2026-10-16: 6 s per run on the real ledger; building the repo is
# 0.09 s), so a serial replay of a long window takes hours. The result must be IDENTICAL to the serial
# one, including order, or a number printed with --jobs is a number about a different question.
SERIAL=$(replay --script "$R/scripts/cand.py" --baseline-script "$R/scripts/base.py" --jobs 1 | js "[(e['subject'], e['base_rc'], e['cand_rc']) for e in d['events']]")
PARALLEL=$(replay --script "$R/scripts/cand.py" --baseline-script "$R/scripts/base.py" --jobs 4 | js "[(e['subject'], e['base_rc'], e['cand_rc']) for e in d['events']]")
eq "7 --jobs 4 gives exactly the serial events, in the same order" "$PARALLEL" "$SERIAL"
has "7 and the serial reference is not empty" "$SERIAL" "c2 todo"

echo
echo "probe-hook-replay: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
