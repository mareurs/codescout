#!/usr/bin/env bash
# tests/mutation-probe.sh — cases for scripts/mutation-probe.sh
#
# WHAT THIS GUARDS
#
# `mutation-probe.sh` exists so that arming a mutation cannot publish a red to
# peers who cannot attribute it
# (`docs/issues/archive/2026-09-08-an-armed-mutation-is-a-deliberate-red-no-observer-can-distinguish.md`).
# Two of its properties are load-bearing and both failed at least once while it
# was being written — found by RUNNING it, not by reading it:
#
#   1. THE OCCURRENCE COUNT. The first version counted with `grep -F -c`, which
#      counts matching LINES. Given a two-line `--find` it treats each line as a
#      separate alternative and returns their union: a pattern occurring exactly
#      once reported **11**. It did not error — it returned a plausible integer
#      meaning something else, which is the whole defect class this script serves.
#      Case 4 is that regression and fails against any line-based counter.
#
#   2. THE WORKTREE DOES NOT CARRY UNCOMMITTED WORK. `git worktree add` checks out
#      HEAD, so an isolated probe would silently test the COMMITTED version of a
#      file you have just edited — reporting a survival for a guard that does not
#      exist there yet. Case 6 pins the copy that fixes it.
#
# Cases 7 and 8 are MUST-SURVIVE: they fail if the marker is written when nothing
# is armed, or left behind after a revert. A marker that over-reports is worse
# than none, because it teaches readers to ignore it.
set -uo pipefail

SELF_ROOT=$(cd "$(dirname "$0")/.." && pwd)
PROBE="$SELF_ROOT/scripts/mutation-probe.sh"
PASS=0
FAIL=0

has() { # has <label> <haystack> <needle>
    if printf '%s' "$2" | grep -qF -- "$3"; then
        PASS=$((PASS + 1))
    else
        FAIL=$((FAIL + 1)); echo "FAIL: $1"; echo "  expected to find: $3"; echo "  in: $2"
    fi
}
eq() { # eq <label> <actual> <expected>
    if [ "$2" = "$3" ]; then
        PASS=$((PASS + 1))
    else
        FAIL=$((FAIL + 1)); echo "FAIL: $1"; echo "  actual:   $2"; echo "  expected: $3"
    fi
}

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
SID="test-sid-0000"

# A throwaway git repo, because the probe resolves its root with `rev-parse` and
# the isolated path needs a real HEAD to create a worktree from.
newrepo() { # newrepo -> echoes the repo path
    local p="$WORK/repo$RANDOM"
    mkdir -p "$p/src"
    git -C "$p" init -q
    git -C "$p" config user.email t@t; git -C "$p" config user.name t
    printf 'fn a() {\n    guard();\n    return;\n}\nfn b() {\n    return;\n}\n' > "$p/src/lib.rs"
    git -C "$p" add -A >/dev/null; git -C "$p" commit -qm init
    echo "$p"
}

run() { # run <repo> <env-sid> <args...> -> sets OUT and RC
    local repo="$1" sid="$2"; shift 2
    OUT=$(cd "$repo" && CLAUDE_CODE_SESSION_ID="$sid" "$PROBE" "$@" 2>&1)
    RC=$?
}

# --- 1. No session id: the marker would name no author, so refuse ------------
R=$(newrepo)
run "$R" "" --file src/lib.rs --find 'guard();' --replace '' -- true
eq   "1 no sid exits 2" "$RC" "2"
has  "1 says why 'mine' has no referent" "$OUT" "CLAUDE_CODE_SESSION_ID is not set"

# --- 2. Pattern occurring zero times ----------------------------------------
R=$(newrepo)
run "$R" "$SID" --file src/lib.rs --find 'nosuchtext' --replace 'x' -- true
eq   "2 zero occurrences exits 2" "$RC" "2"
has  "2 names the count" "$OUT" "found 0"
has  "2 says a survival would be meaningless" "$OUT" "makes a survival meaningless"

# --- 3. Pattern occurring twice ---------------------------------------------
# `    return;` appears in both fns. Mutating it would change more than named.
R=$(newrepo)
run "$R" "$SID" --file src/lib.rs --find '    return;' --replace '' -- true
eq   "3 two occurrences exits 2" "$RC" "2"
has  "3 names the count" "$OUT" "found 2"

# --- 4. REGRESSION: a MULTI-LINE literal occurring exactly once is accepted --
# This is the `grep -F -c` defect. That counter reports the number of LINES
# matching either half — 3 for this pattern in this fixture — and the probe
# would refuse a perfectly good mutation. A correct literal count says 1.
R=$(newrepo)
run "$R" "$SID" --shared --file src/lib.rs --find '    guard();
    return;' --replace '    return;' -- true
eq   "4 multi-line literal occurring once is ACCEPTED" "$RC" "0"
has  "4 armed and reported" "$OUT" "ARMED"
# `-- true` runs no tests, so no verdict is available. This assertion read
# "SURVIVED" until 2026-09-14 — the suite pinned the very defect the probe exists
# to study, because a fixture that runs no tests is the zero-test case.
has  "4 renders no verdict when nothing ran" "$OUT" "INCONCLUSIVE"

# --- 5. Shared mode: file is restored, marker is gone ------------------------
# Runs `-- true`, so zero tests execute and the verdict is INCONCLUSIVE by design.
# INERT ON THE VERDICT AXIS — asserts restoration and marker cleanup only. Do not
# "repair" it by asserting SURVIVED: that is the defect case 4's comment describes,
# and this fixture establishes nothing about a verdict.
R=$(newrepo)
run "$R" "$SID" --shared --file src/lib.rs --find 'guard();' --replace '' -- true
eq   "5 shared run exits with the test command's rc" "$RC" "0"
eq   "5 the mutated line is restored" "$(grep -c 'guard();' "$R/src/lib.rs")" "1"
eq   "5 no marker is left behind" "$(ls "$R/.codescout/mutations" 2>/dev/null | wc -l)" "0"

# --- 6. Isolated mode leaves the shared tree byte-identical ------------------
# Also `-- true`, so also INCONCLUSIVE and also INERT ON THE VERDICT AXIS — see
# case 5. The `-- true` population is three (4, 5, 6) plus case 14; only 4 and 14
# assert on the verdict.
R=$(newrepo)
BEFORE=$(sha256sum "$R/src/lib.rs" | cut -d' ' -f1)
# Uncommitted edit: the worktree is created at HEAD, so without the copy the
# probe would test a version of this file that does not contain `moved()`.
printf 'fn a() {\n    moved();\n    return;\n}\nfn b() {\n    return;\n}\n' > "$R/src/lib.rs"
UNCOMMITTED=$(sha256sum "$R/src/lib.rs" | cut -d' ' -f1)
run "$R" "$SID" --file src/lib.rs --find 'moved();' --replace '' -- true
eq   "6 isolated run succeeds on UNCOMMITTED text (worktree carries the edit)" "$RC" "0"
eq   "6 shared tree is byte-identical after" "$(sha256sum "$R/src/lib.rs" | cut -d' ' -f1)" "$UNCOMMITTED"
has  "6 reports isolated mode" "$OUT" "mode=isolated"

# --- 7. MUST-SURVIVE: a refused run writes no marker ------------------------
# The refusal paths exit before arming. A marker written there would name a
# mutation that never existed, and a reader who checks once and finds it false
# stops checking.
R=$(newrepo)
run "$R" "$SID" --file src/lib.rs --find 'nosuchtext' --replace 'x' -- true
eq   "7 a refused run arms nothing" "$(ls "$R/.codescout/mutations" 2>/dev/null | wc -l)" "0"

# --- 8. MUST-SURVIVE: isolated mode writes a marker the READER ignores -------
# `attribute-red.py` reports only `mode == "shared"` markers, because an isolated
# probe cannot produce a red in the tree the reader is looking at. Asserted on
# the reader rather than on the file, since that is the behaviour that matters.
R=$(newrepo)
MARKS="$R/.codescout/mutations"; mkdir -p "$MARKS"
printf '{"session_id":"x","mode":"isolated","file":"src/lib.rs","armed_at":"now","pid":%d}\n' "$$" > "$MARKS/iso.json"
printf '{"session_id":"y","mode":"shared","file":"src/lib.rs","armed_at":"now","pid":%d}\n' "$$" > "$MARKS/shd.json"
NOTES=$(cd "$R" && python3 -c "
import sys, pathlib
import importlib.util
spec = importlib.util.spec_from_file_location('ar', '$SELF_ROOT/scripts/attribute-red.py')
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
print('\n'.join(m.mutation_notes(pathlib.Path('$R'), ['src/lib.rs'])))
")
has  "8 reader reports the SHARED marker" "$NOTES" "by y"
eq   "8 reader ignores the ISOLATED marker" "$(printf '%s' "$NOTES" | grep -c 'by x')" "0"
has  "8 wording asks for nothing (OB-23)" "$NOTES" "Nothing is asked of you"

# --- 9. MUST-SURVIVE: a marker whose process is gone is not reported ---------
# CASE 8 IS THIS CASE'S CONTROL. Case 9 asserts EMPTY output, and an erroring
# loader produces empty too — it passed vacuously once, while a path bug meant the
# module never loaded at all. Case 8 exercises the same loader and asserts on
# CONTENT, so a green there is what makes this empty attributable to the dead pid.
R=$(newrepo)
MARKS="$R/.codescout/mutations"; mkdir -p "$MARKS"
printf '{"session_id":"z","mode":"shared","file":"src/lib.rs","armed_at":"now","pid":999999}\n' > "$MARKS/dead.json"
NOTES=$(cd "$R" && python3 -c "
import pathlib, importlib.util
spec = importlib.util.spec_from_file_location('ar', '$SELF_ROOT/scripts/attribute-red.py')
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
print('\n'.join(m.mutation_notes(pathlib.Path('$R'), ['src/lib.rs'])))
")
eq   "9 a dead pid's marker is not reported" "$(printf '%s' "$NOTES" | wc -c)" "0"

# --- 10-13. A VERDICT REQUIRES THAT TESTS ACTUALLY EXECUTED -----------------
# Four states, and the two INCONCLUSIVE ones are the defect: before 2026-09-14 the
# verdict was `rc == 0 ? SURVIVED : KILLED`, which rendered a finding over an
# absence in BOTH directions — a run that executed nothing exits 0 and read as
# SURVIVED, and a mutation that failed to COMPILE exits non-zero and read as KILLED.
#
# Faked with `sh -c`, deliberately: the runner's output is the only input the verdict
# reads, so a fixture that prints the line is exercising the real predicate. It also
# keeps this suite toolchain-free, which is why its CI job needs no Rust.
#
# Mutation that must kill these: restore `[ "$rc" -eq 0 ]` as the sole discriminator.

# 10. Runner ran tests and passed -> SURVIVED (the verdict must still be reachable;
#     without this, a fix that only ever said INCONCLUSIVE would pass 11-13).
R=$(newrepo)
run "$R" "$SID" --shared --file src/lib.rs --find 'guard();' --replace '' \
    -- sh -c 'echo "running 2 tests"; echo "test result: ok. 2 passed"; exit 0'
has  "10 tests ran and passed -> SURVIVED" "$OUT" "SURVIVED"
has  "10 names how many ran" "$OUT" "2 test(s) ran"

# 11. Runner ran tests and failed -> KILLED
R=$(newrepo)
run "$R" "$SID" --shared --file src/lib.rs --find 'guard();' --replace '' \
    -- sh -c 'echo "running 2 tests"; echo "test result: FAILED. 1 passed; 1 failed"; exit 101'
has  "11 tests ran and failed -> KILLED" "$OUT" "KILLED"
has  "11 count rules out a compile failure" "$OUT" "2 test(s) ran"

# 12. Runner started, selected nothing -> INCONCLUSIVE, not SURVIVED.
#     This is the measured original: `SURVIVED (rc=0)` printed over `running 0 tests`
#     because the caller's test was uncommitted and the worktree builds at HEAD.
R=$(newrepo)
run "$R" "$SID" --shared --file src/lib.rs --find 'guard();' --replace '' \
    -- sh -c 'echo "running 0 tests"; echo "test result: ok. 0 passed"; exit 0'
has  "12 zero tests selected -> INCONCLUSIVE" "$OUT" "INCONCLUSIVE"
eq   "12 and NOT survived" "$(printf '%s' "$OUT" | grep -c 'SURVIVED')" "0"
has  "12 names the uncommitted-test cause" "$OUT" "only the mutated file is carried across"

# 13. No count line at all (a compile failure looks like this) -> INCONCLUSIVE,
#     and specifically NOT KILLED despite the non-zero exit.
R=$(newrepo)
run "$R" "$SID" --shared --file src/lib.rs --find 'guard();' --replace '' \
    -- sh -c 'echo "error: expected one of \`)\`, found \`;\`"; exit 101'
has  "13 no count line -> INCONCLUSIVE" "$OUT" "INCONCLUSIVE"
eq   "13 and NOT killed" "$(printf '%s' "$OUT" | grep -c 'KILLED')" "0"
has  "13 names the parse as a cause, not only the build" "$OUT" "has changed shape"

# --- 14. MUST-SURVIVE: the exit status is still the test command's -----------
# `docs/PROBES.md` pins this and case 5 asserts it for the passing path. The verdict
# lives on stderr and NOT in `$?` — an INCONCLUSIVE run exits 0 when its command did.
# Stated as a test so a future change to the exit contract is a deliberate one.
R=$(newrepo)
run "$R" "$SID" --shared --file src/lib.rs --find 'guard();' --replace '' \
    -- sh -c 'echo "running 0 tests"; exit 0'
eq   "14 inconclusive still exits with the command's rc" "$RC" "0"
has  "14 while saying so in the verdict" "$OUT" "INCONCLUSIVE"

echo
echo "mutation-probe: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] || exit 1
