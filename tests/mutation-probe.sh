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
BGPIDS=()
# Kill every background pid by ITS pid before removing anything: a process that outlives
# the suite can fill a shared tmpfs (bug-fix-session-log:F-175).
trap 'for p in "${BGPIDS[@]:-}"; do [ -n "$p" ] && kill -9 "$p" 2>/dev/null; done; rm -rf "$WORK"' EXIT
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
# The remedy text changed with the behaviour, and that is the point: the old wording
# named "your test is uncommitted" as a cause, which cases 19-20 have now made
# impossible. A predicate fix that left the remedy naming a retired cause would send
# the reader to commit a test that is already carried.
has  "12 names a cause that is still REAL" "$OUT" "names a HELPER or a module"
has  "12 and retires the one the fix removed" "$OUT" "is NOT a cause here"

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

# --- 15-18. `--strict` REMAPS THE NO-VERDICT CASE AND NOTHING ELSE ----------
# The flag exists because the verdict line does not always ARRIVE. Measured
# 2026-09-14: through codescout's `run_command`, a `cargo test` wrapped by this
# script classifies `type: "test"`, and that envelope carried no stderr field —
# so `{"exit_code": 0, "passed": 0}` was the whole result, byte-identical to a
# SURVIVED mutant. The exit code is the only channel that survives every renderer.
#
# Cases 16-18 are the NON-VACUITY PAIR for 15: a flag that returned 3 for
# everything would satisfy 15 alone, and would be strictly worse than no flag —
# it would convert two real verdicts into "proved nothing". Each asserts that a
# verdict the caller ACTS ON keeps its own status under the same flag.
#
# Mutation that must kill these, both run 2026-09-14 against an isolated copy of the
# script and the suite (never the shared tree — the fixture resolves PROBE to the repo's
# own script, so mutating it in place ships a broken probe to every concurrent session):
#   drop the `[ "$inconclusive" = "1" ]` conjunct -> 17, 18 red.
#   drop the `[ "$STRICT" = "1" ]`      conjunct -> 4, 5, 6, 14 red.
# The second is worth reading rather than counting: cases 4-6 run `-- true`, which emits
# no count line and is therefore INCONCLUSIVE, so the default path's protection turns out
# to be guarded at four sites rather than the one this comment first claimed. That is the
# measurement that makes "opt-in" a property of the code and not of the flag's name.

# 15. INCONCLUSIVE + --strict -> 3. Case 14 is this case's control: same runner
#     output, no flag, and it must still exit with the command's own status.
R=$(newrepo)
run "$R" "$SID" --shared --strict --file src/lib.rs --find 'guard();' --replace '' \
    -- sh -c 'echo "running 0 tests"; exit 0'
eq   "15 strict remaps inconclusive to 3" "$RC" "3"
has  "15 while still naming the verdict" "$OUT" "INCONCLUSIVE"
has  "15 and saying what it remapped from" "$OUT" "exiting 3 rather than 0"

# 16. No count line + --strict -> 3 as well. The OTHER inconclusive branch; one
#     conjunct guards both, and a fix touching only one would pass 15.
R=$(newrepo)
run "$R" "$SID" --shared --strict --file src/lib.rs --find 'guard();' --replace '' \
    -- sh -c 'echo "error: expected one of \`)\`, found \`;\`"; exit 101'
eq   "16 strict remaps the no-count-line branch too" "$RC" "3"
has  "16 and still calls it inconclusive" "$OUT" "INCONCLUSIVE"

# 17. SURVIVED + --strict -> still 0. A real finding, not an absence.
R=$(newrepo)
run "$R" "$SID" --shared --strict --file src/lib.rs --find 'guard();' --replace '' \
    -- sh -c 'echo "running 2 tests"; echo "test result: ok. 2 passed"; exit 0'
eq   "17 strict leaves SURVIVED at the command's rc" "$RC" "0"
has  "17 and it is still a verdict" "$OUT" "SURVIVED"

# 18. KILLED + --strict -> still the command's rc, not 3.
R=$(newrepo)
run "$R" "$SID" --shared --strict --file src/lib.rs --find 'guard();' --replace '' \
    -- sh -c 'echo "running 2 tests"; echo "test result: FAILED. 1 passed; 1 failed"; exit 101'
eq   "18 strict leaves KILLED at the command's rc" "$RC" "101"
has  "18 and it is still a verdict" "$OUT" "KILLED"

# --- 19-20. A MULTI-FILE uncommitted change reaches the worktree ------------
# Case 6 pins that the MUTATED file's uncommitted content is carried. These pin the
# REST of the change. A slice that alters a type and its call sites is the ordinary
# shape of real work, and carrying only `--file` left the worktree not compiling:
# the run paid a full cold build and ended INCONCLUSIVE, having reported the fact
# ("N other .rs file(s) are dirty") without its consequence.
#
# The test command runs INSIDE the worktree, so `cat` is a DIRECT observation of what
# was carried. Inferring it from a build outcome would conflate "not carried" with
# "carried and still broken", which is the distinction under test.
R=$(newrepo)
printf 'pub fn helper() -> u8 {\n    7\n}\n' > "$R/src/other.rs"
git -C "$R" add -A >/dev/null; git -C "$R" commit -qm other
printf 'fn a() {\n    guard();\n    return;\n}\n// EDIT-IN-MUTATED-FILE\n' > "$R/src/lib.rs"
printf 'pub fn helper() -> u8 {\n    7\n}\n// EDIT-IN-SIBLING-FILE\n' > "$R/src/other.rs"
run "$R" "$SID" --file src/lib.rs --find 'guard();' --replace '' \
    -- sh -c 'cat src/lib.rs src/other.rs; echo "running 1 test"; echo "test result: ok. 1 passed"'
has  "19 the mutated file's own edit is carried (case 6, observed directly)" "$OUT" "EDIT-IN-MUTATED-FILE"
has  "19 a SIBLING dirty file's edit is carried too" "$OUT" "EDIT-IN-SIBLING-FILE"

# 20. An UNTRACKED file is part of the change too — a new module is the commonest
#     way a slice adds a call site, and it has no HEAD version to fall back to, so
#     omitting it is a compile error rather than a stale build.
R=$(newrepo)
printf 'fn a() {\n    guard();\n    return;\n}\n' > "$R/src/lib.rs"
printf '// EDIT-IN-UNTRACKED-FILE\n' > "$R/src/newmod.rs"
run "$R" "$SID" --file src/lib.rs --find 'guard();' --replace '' \
    -- sh -c 'cat src/newmod.rs 2>/dev/null; echo "running 1 test"; echo "test result: ok. 1 passed"'
has  "20 an untracked new file is carried" "$OUT" "EDIT-IN-UNTRACKED-FILE"

# --- 21. THE REFUSAL NAMES THE NON-CARGO RUNNER AMONG ITS CAUSES ------------
# The `ran_lines == 0` branch is CORRECT here and must stay a refusal: inferring a
# count from an unrecognised format is the `absence rendered as a value` that cases
# 10-13 exist to forbid. What was wrong was the next sentence a reader acts on.
#
# For a shell suite the three causes it named were inapplicable (nothing compiles),
# false (it IS a test runner, and it ran 155 tests) and misleading (a cargo format
# "changed shape" that never applied to the run). Measured 2026-09-16: six mutations
# of a Python file against `bash tests/file-provenance.sh` all read INCONCLUSIVE
# while four were decisive kills and two decisive survivals, and the whole table was
# read by hand off the suite's own `passed=N failed=M` line
# (`docs/issues/archive/2026-09-16-mutation-probe-renders-no-verdict-for-a-non-cargo-runner.md`).
#
# This asserts ARRIVAL, not that the advice is correct — the distinction CLAUDE.md
# § Testing Discipline draws: a suite tests a guard's PREDICATE and never its REMEDY
# TEXT, so the half that sends a reader somewhere useless is untested by construction
# and no mutation reaches it.
#
# THE NEEDLE IS A PHRASE UNIQUE TO THE CLAUSE, AND THE OBVIOUS BETTER IDEA WAS
# MEASURED VACUOUS. An entity needle — case-insensitively, "does this message still
# say cargo at all" — is what CLAUDE.md § Testing Discipline prescribes for a remedy
# text, because it reds on deletion and survives rewording. Here it does not: the
# remedy paragraph two lines below the clause says "On the NON-CARGO cause", so
# deleting the clause entirely leaves `cargo` in the output and the assertion green.
# Measured 2026-09-16 with the probe itself — `--find` the clause, `--replace` a
# cargo-free string, `-- bash tests/mutation-probe.sh`: 50 passed, 0 failed, SURVIVED.
# That is the scope law of § Testing Discipline exactly: an assertion computed over a
# POPULATION (the whole message) cannot verify a claim about a MEMBER (one clause),
# and re-reading it returns a true sentence either way.
#
# So "not CARGO" it is — unique to the clause, reds on its deletion, and WILL red on a
# rewording that keeps the advice. That cost is accepted rather than unnoticed: the
# message line carries a pointer back to this case, so a rewriter is told where to
# look instead of finding a red they read as a regression. "summary line" is the same
# kind of needle for the remedy half, where the entity has no one-word name.
#
# Neither can tell you the advice is CORRECT — only that the case is still addressed.
#
# The fixture is the reported shape rather than case 13's compile-error shape: a
# runner that exits NON-ZERO with its own decisive summary. Case 13 pins that a
# non-zero exit with no count line is not a kill; this pins that the reader is told
# why, and is not sent to hunt a compile error that cannot exist.
#
# Mutation that must kill this: delete the non-cargo clause from the `ran_lines == 0`
# message in scripts/mutation-probe.sh.
R=$(newrepo)
run "$R" "$SID" --shared --file src/lib.rs --find 'guard();' --replace '' \
    -- sh -c 'echo "passed=154 failed=1"; exit 1'
has  "21 a non-cargo runner -> INCONCLUSIVE" "$OUT" "INCONCLUSIVE"
has  "21 names the non-cargo runner as a cause" "$OUT" "not CARGO"
has  "21 and says where the verdict IS readable" "$OUT" "summary line"
eq   "21 and still renders no verdict" "$(printf '%s' "$OUT" | grep -cE 'KILLED|SURVIVED')" "0"

# --- 22-25. THE ISOLATED WORKTREE IS LEASED PER RUN, not keyed on the session --------
# Keying the tree on the session id left one 3.5-7.5G worktree per session that ever
# ran the probe, never removed: 72G across 18 on 2026-09-24
# (docs/issues/2026-09-24-mutation-probe-worktrees-are-never-reclaimed.md). It also
# handed two CONCURRENT runs from one session (parallel subagents share the id) the
# same tree, where each run's reset reverts the other's mutation.
tree_of() { printf '%s\n' "$1" | sed -n 's/.*ARMED .* tree=//p' | head -1; }
probe_trees() { git -C "$1" worktree list | grep -cF "$1.worktrees/"; }
hold_cmd() { # hold_cmd <tag> -> a test command that parks until $WORK/hold-<tag> is removed
    printf 'echo $$ > %q; : > %q; for i in $(seq 1 300); do [ -e %q ] || break; sleep 0.1; done' \
        "$WORK/cmd-$1.pid" "$WORK/started-$1" "$WORK/hold-$1"
}
wait_for() { for _ in $(seq 1 150); do [ -e "$1" ] && return 0; sleep 0.1; done; return 1; }

# 22. A later session reuses the tree a finished session freed.
R=$(newrepo)
run "$R" sid-a --file src/lib.rs --find 'guard();' --replace '' -- true
TA=$(tree_of "$OUT")
run "$R" sid-b --file src/lib.rs --find 'guard();' --replace '' -- true
TB=$(tree_of "$OUT")
eq "22 the first run armed an isolated tree" "$([ -n "$TA" ] && echo armed || echo none)" "armed"
eq "22 a second session reuses the freed tree" "$TB" "$TA"
eq "22 two sessions leave ONE probe worktree" "$(probe_trees "$R")" "1"

# 23. Two concurrent runs from the SAME session never share a tree.
R=$(newrepo); : > "$WORK/hold-c"
( cd "$R" && CLAUDE_CODE_SESSION_ID=sid-c exec "$PROBE" --file src/lib.rs --find 'guard();' \
    --replace '' -- bash -c "$(hold_cmd c)" ) > "$WORK/out-c" 2>&1 & BGPIDS+=("$!")
if wait_for "$WORK/started-c"; then
    BGPIDS+=("$(cat "$WORK/cmd-c.pid")")
    run "$R" sid-c --file src/lib.rs --find 'guard();' --replace '' -- true
    TC=$(tree_of "$(cat "$WORK/out-c")"); TD=$(tree_of "$OUT")
    eq "23 a concurrent same-session run gets its OWN tree" \
        "$([ -n "$TD" ] && [ "$TD" != "$TC" ] && echo distinct || echo "shared-or-refused")" "distinct"
else
    FAIL=$((FAIL + 1)); echo "FAIL: 23 holder never started: $(cat "$WORK/out-c")"
fi
rm -f "$WORK/hold-c"; wait 2>/dev/null

# 24. SIGKILLing the probe while its command runs does NOT free the tree: the command
# still holds the lease, so a same-session run started now must get another tree.
R=$(newrepo); : > "$WORK/hold-e"
( cd "$R" && CLAUDE_CODE_SESSION_ID=sid-e exec "$PROBE" --file src/lib.rs --find 'guard();' \
    --replace '' -- bash -c "$(hold_cmd e)" ) > "$WORK/out-e" 2>&1 & PE=$!; BGPIDS+=("$PE")
if wait_for "$WORK/started-e"; then
    W=$(cat "$WORK/cmd-e.pid"); BGPIDS+=("$W")
    # Precondition: the pid about to be killed IS the probe (its marker records $$).
    MPID=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["pid"])' \
        "$R/.codescout/mutations/sid-e.json" 2>/dev/null)
    eq "24 the killed pid is the probe itself" "$MPID" "$PE"
    kill -9 "$PE"; wait "$PE" 2>/dev/null
    eq "24 the command outlives its SIGKILLed probe" "$(kill -0 "$W" 2>/dev/null && echo alive || echo dead)" "alive"
    run "$R" sid-e --file src/lib.rs --find 'guard();' --replace '' -- true
    TE=$(tree_of "$(cat "$WORK/out-e")"); TF=$(tree_of "$OUT")
    eq "24 the orphaned command keeps its tree leased" \
        "$([ -n "$TF" ] && [ "$TF" != "$TE" ] && echo distinct || echo "shared-or-refused")" "distinct"
else
    FAIL=$((FAIL + 1)); echo "FAIL: 24 holder never started: $(cat "$WORK/out-e")"
fi
rm -f "$WORK/hold-e"

# 25. A flock that cannot run stops the probe before any worktree exists, and never
# spins. `timeout` bounds a mutant that would loop (bug-fix-session-log:F-175).
R=$(newrepo); mkdir -p "$WORK/noflock"
printf '#!/usr/bin/env bash\nexit 127\n' > "$WORK/noflock/flock"; chmod +x "$WORK/noflock/flock"
OUT=$(cd "$R" && PATH="$WORK/noflock:$PATH" CLAUDE_CODE_SESSION_ID=sid-f timeout 20 "$PROBE" \
    --file src/lib.rs --find 'guard();' --replace '' -- true 2>&1); RC=$?
eq "25 the probe exits 2" "$RC" "2"
eq "25 no probe worktree was created" "$(probe_trees "$R")" "0"

# 26. An isolated run leaves nothing in TMPDIR. `WTPATCH` (the carried working-tree
# patch) was never removed, so every run leaked one file: often a copy of peers'
# uncommitted diffs.
R=$(newrepo); mkdir -p "$WORK/tmp26"
OUT=$(cd "$R" && TMPDIR="$WORK/tmp26" CLAUDE_CODE_SESSION_ID=sid-g "$PROBE" \
    --file src/lib.rs --find 'guard();' --replace '' -- true 2>&1); RC=$?
eq "26 the run armed" "$(tree_of "$OUT" | grep -c .)" "1"
eq "26 TMPDIR is empty after the run" "$(ls -A "$WORK/tmp26" | wc -l | tr -d ' ')" "0"

echo
echo "mutation-probe: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] || exit 1
