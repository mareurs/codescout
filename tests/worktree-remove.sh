#!/usr/bin/env bash
#
# Discrimination suite for `scripts/worktree-remove.sh`'s holder scan and its refusals.
#
# WHY THIS EXISTS
# ---------------
# A linked worktree that served rust-analyzer keeps a `codescout mux ... rust-analyzer`
# process and its `cargo check` child. After `git worktree remove` deregisters the
# worktree those processes rewrite `target/`, git fails with "Directory not empty", and a
# directory is left behind (bug 3dde1548c3e5c310). The script lists those holders so an
# operator can stop them first. The risk it must not hide: a mux is shared across sessions,
# so classifying the wrong process as stoppable, or missing a process that is not
# stoppable, leads to killing something a peer session is working with.
#
# HOW IT IS DRIVEN
# ----------------
# The script reads `WORKTREE_REMOVE_PROC` (default `/proc`). Each case points it at a
# fixture tree: `<fixture>/<pid>/cwd` (a symlink to a real directory) and
# `<fixture>/<pid>/cmdline` (NUL-separated arguments). No real process is started, so the
# cases cannot depend on what else runs on the machine, and none of them can signal a real
# process. The script under test is run as a child (`bash <script>`), the same way an
# operator runs it, so the exit codes asserted here are the ones an operator sees.
#
# THE CASES THAT MATTER are the ones that separate a holder from a lookalike
# (`a_sibling_prefix_is_not_a_holder`, `whole_component_matching`) and a stoppable process
# from a blocker (`a_codescout_server_is_not_a_mux`, `kind_comes_from_argv0_not_comm`).
# A scan that listed everything, or classified everything as stoppable, would pass the
# cases that only look for one positive row.
#
# WHAT IT DOES NOT COVER
# ----------------------
#   * Real processes and real signals: the script's own pid and its parents are only
#     proved absent from a REAL /proc scan (`the_script_does_not_list_itself`); that a
#     blocker is never signalled, and the stop, remove and verify sequence, are covered
#     with real processes in the cases added with those steps.
#   * What a real kernel puts in /proc for a process owned by another user: an unreadable
#     entry is modelled here by a missing `cmdline` file, which is not the same failure.
#   * Peers: nothing here can say whether a listed mux has a live client.
#
# Every repository and worktree this suite makes lives under its own `mktemp -d`
# directory, git is always run as `git -C <dir>`, and GIT_DIR / GIT_WORK_TREE /
# GIT_INDEX_FILE are unset, so no case can act on the checkout this suite lives in.
#
# Usage:
#   tests/worktree-remove.sh        # non-zero exit on any failure
#
# WORKTREE_REMOVE_SCRIPT may name a copy of the script to run instead of
# scripts/worktree-remove.sh; the mutation checks in docs/PROBES.md use it so the real
# file is never edited.

set -uo pipefail
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="${WORKTREE_REMOVE_SCRIPT:-$ROOT/scripts/worktree-remove.sh}"
PASS=0
FAIL=0
TAB=$'\t'

T="$(mktemp -d "${TMPDIR:-/tmp}/worktree-remove-test.XXXXXX")" || exit 1
T="$(cd "$T" && pwd -P)"
case "$T" in /?*) ;; *) echo "refusing: bad temp dir [$T]" >&2; exit 1 ;; esac
trap 'rm -rf "$T"' EXIT
# A temp dir that sits inside some other repository must not turn a plain directory into
# part of it: git may not look above the suite's own directory.
export GIT_CEILING_DIRECTORIES="$T"

ok()   { echo "  PASS  $1"; PASS=$((PASS + 1)); }
bad()  { echo "  FAIL  $1"; echo "          want=[$2] got=[$3]"; FAIL=$((FAIL + 1)); }
eq()   { [ "$2" = "$3" ] && ok "$1" || bad "$1" "$2" "$3"; }
contains() {
    case "$3" in *"$2"*) ok "$1" ;; *) bad "$1" "contains [$2]" "$3" ;; esac
}
lacks() {
    case "$3" in *"$2"*) bad "$1" "lacks [$2]" "$3" ;; *) ok "$1" ;; esac
}

# git against an explicit directory, with an identity and no hooks or signing of its own.
G() {
    local d=$1
    shift
    git -C "$d" -c user.name=suite -c user.email=suite@example.invalid \
        -c commit.gpgsign=false -c core.hooksPath=/dev/null "$@"
}

# new_case NAME [WORKTREE_DIR_NAME]: a repo with one commit and a linked worktree, plus an
# empty fixture /proc. Sets C (case dir), REPO (main checkout), WT (linked worktree), PROCD.
new_case() {
    C="$T/$1"
    PROCD="$C/proc"
    REPO="$C/repo"
    WT="$C/${2:-wt}"
    mkdir -p "$PROCD"
    git init -q -b main "$REPO" >/dev/null 2>&1 || { echo "setup failed: git init" >&2; exit 1; }
    G "$REPO" commit -q --allow-empty -m init || { echo "setup failed: commit" >&2; exit 1; }
    G "$REPO" worktree add -q -b "wt-$1" "$WT" >/dev/null 2>&1 \
        || { echo "setup failed: worktree add" >&2; exit 1; }
}

# fake PID CWD ARGV...: one fixture process. Its `comm` is argv[0]'s basename cut to 15
# characters, as the kernel does, unless FAKE_COMM overrides it.
fake() {
    local pid=$1 cwd=$2 base comm
    shift 2
    mkdir -p "$PROCD/$pid"
    ln -s "$cwd" "$PROCD/$pid/cwd"
    printf '%s\0' "$@" > "$PROCD/$pid/cmdline"
    base="${1##*/}"
    comm="${FAKE_COMM:-${base:0:15}}"
    printf '%s\n' "$comm" > "$PROCD/$pid/comm"
}

# run ARGS...: run the script from RUN_CWD (default the case dir) against the fixture proc
# (or PROC_OVERRIDE). Sets RC, OUT (stdout), ERR (stderr).
run() {
    ( cd "${RUN_CWD:-$C}" && WORKTREE_REMOVE_PROC="${PROC_OVERRIDE:-$PROCD}" bash "$SRC" "$@" ) \
        >"$C/out" 2>"$C/err"
    RC=$?
    OUT="$(<"$C/out")"
    ERR="$(<"$C/err")"
}

kinds() { printf '%s\n' "$OUT" | cut -f2 | paste -sd, -; }
pids()  { printf '%s\n' "$OUT" | cut -f1 | paste -sd, -; }

case_lists_every_holder_kind() {
    echo "lists_every_holder_kind"
    new_case kinds
    # Pids are scrambled against the expected order, so a listing that is merely sorted by
    # pid cannot pass.
    fake 91001 "$WT" vim notes.txt
    fake 91002 "$C" "$WT/target/debug/build/x/build-script-build"
    fake 91003 "$C" cargo check --manifest-path "$WT/Cargo.toml"
    fake 91004 "$WT" rust-analyzer
    fake 91005 "$WT" codescout mux --cwd "$WT"
    run "$WT"
    eq "exit 3" 3 "$RC"
    eq "kinds, mux first and blocker last" "mux,rust-analyzer,cargo,build-output,other" "$(kinds)"
    eq "first row is PID TAB KIND TAB CMDLINE" \
        "91005${TAB}mux${TAB}codescout mux --cwd $WT" "$(printf '%s\n' "$OUT" | sed -n 1p)"
    eq "last row is the blocker" "91001${TAB}other${TAB}vim notes.txt" \
        "$(printf '%s\n' "$OUT" | sed -n '$p')"
    contains "the occupancy warning goes to stderr" occupant "$ERR"
    lacks "the warning is not on stdout" occupant "$OUT"
}

case_a_codescout_server_is_not_a_mux() {
    echo "a_codescout_server_is_not_a_mux"
    new_case server
    fake 91001 "$WT" codescout start --cwd "$WT"
    run "$WT"
    eq "exit 3" 3 "$RC"
    eq "a server is other, never mux" other "$(kinds)"
}

case_a_sibling_prefix_is_not_a_holder() {
    echo "a_sibling_prefix_is_not_a_holder"
    new_case sibling
    mkdir "$WT-other" "${WT}2"
    fake 91001 "$WT-other" vim
    fake 91002 "${WT}2" vim
    fake 91003 "$C" vim "$WT-other/notes.txt"
    run "$WT"
    eq "exit 0: lookalikes are not holders" 0 "$RC"
    eq "no rows" "" "$OUT"
    contains "says so" "no holders under $WT" "$ERR"
    fake 91004 "$WT" vim
    run "$WT"
    eq "the real holder is still found beside the lookalikes" 91004 "$(pids)"
}

case_a_path_with_a_space_resolves() {
    echo "a_path_with_a_space_resolves"
    new_case space "my wt"
    fake 91001 "$WT" codescout mux --cwd "$WT"
    run "$WT"
    eq "exit 3" 3 "$RC"
    eq "the row is listed" mux "$(kinds)"
    contains "the full path is in the row" "$WT" "$OUT"
}

case_relative_trailing_slash_and_symlink_spellings_resolve() {
    echo "relative_trailing_slash_and_symlink_spellings_resolve"
    new_case spellings
    fake 91001 "$WT" rust-analyzer
    run "$WT"
    local want="$OUT"
    eq "baseline exit" 3 "$RC"
    run ./wt/
    eq "./wt/ exit" 3 "$RC"
    eq "./wt/ rows" "$want" "$OUT"
    ln -s "$WT" "$C/link"
    run link
    eq "symlink exit" 3 "$RC"
    eq "symlink rows" "$want" "$OUT"
    run "$C/link/"
    eq "symlink with slash exit" 3 "$RC"
    eq "symlink with slash rows" "$want" "$OUT"
}

case_refusals() {
    echo "refuses_an_unregistered_path / refuses_the_main_checkout / refuses_when_the_callers_cwd_is_inside"
    new_case refuse
    fake 91001 "$WT" vim
    mkdir "$C/plain" "$REPO/sub"

    run "$C/plain"
    eq "unregistered, outside any repo: exit 2" 2 "$RC"
    contains "names the reason" "not a registered worktree" "$ERR"
    eq "nothing listed" "" "$OUT"

    run "$REPO/sub"
    eq "a subdirectory of the main checkout: exit 2" 2 "$RC"
    contains "names the reason" "not a registered worktree" "$ERR"

    run "$WT/not-there"
    eq "a directory inside the worktree that is not its root: exit 2" 2 "$RC"

    run "$REPO"
    eq "the main checkout: exit 2" 2 "$RC"
    contains "names the reason" "main checkout" "$ERR"
    eq "nothing listed" "" "$OUT"

    RUN_CWD="$WT" run "$WT"
    eq "caller's cwd is the worktree: exit 2" 2 "$RC"
    contains "names the reason" "current directory" "$ERR"
    eq "nothing listed" "" "$OUT"

    mkdir "$WT/sub"
    RUN_CWD="$WT/sub" run "$WT"
    eq "caller's cwd is below the worktree: exit 2" 2 "$RC"
    contains "names the reason" "current directory" "$ERR"
}

case_refuses_without_proc() {
    echo "refuses_without_proc"
    new_case noproc
    PROC_OVERRIDE="$C/nowhere" run "$WT"
    eq "exit 2" 2 "$RC"
    contains "the message names /proc" "/proc" "$ERR"
    eq "nothing listed" "" "$OUT"
}

case_stop_with_a_blocker_refuses_before_any_signal() {
    echo "stop_with_a_blocker_refuses_before_any_signal"
    new_case blocker
    fake 91001 "$WT" codescout mux --cwd "$WT"
    fake 91002 "$WT" vim notes.txt
    run --stop "$WT"
    eq "exit 4" 4 "$RC"
    contains "the blocker row is printed" "91002${TAB}other${TAB}vim notes.txt" "$OUT"
    contains "stderr says nothing was signalled" "nothing was signalled" "$ERR"
}

case_kind_comes_from_argv0_not_comm() {
    echo "kind_comes_from_argv0_not_comm"
    new_case comm
    FAKE_COMM=rust-analyzer fake 91001 "$WT" vim notes.txt
    FAKE_COMM=vim fake 91002 "$WT" rust-analyzer
    FAKE_COMM=codescout fake 91003 "$WT" /usr/bin/env codescout mux
    run "$WT"
    eq "exit 3" 3 "$RC"
    eq "by argv[0]: a vim whose comm says rust-analyzer is other, and the reverse" \
        "91002${TAB}rust-analyzer${TAB}rust-analyzer" "$(printf '%s\n' "$OUT" | sed -n 1p)"
    eq "the other two are blockers" "rust-analyzer,other,other" "$(kinds)"
}

case_whole_component_matching() {
    echo "whole_component_matching"
    new_case component
    # cwd elsewhere in every case: only the command line can match.
    fake 92001 "$C" tool "$WT"
    fake 92002 "$C" tool --cwd "$WT"
    fake 92003 "$C" tool "--cwd=$WT"
    fake 92004 "$C" tool "--manifest-path=$WT/Cargo.toml"
    fake 92005 "$C" tool "$WT/"
    fake 92101 "$C" tool "$WT-other"
    fake 92102 "$C" tool "${WT}2/x"
    fake 92103 "$C" tool "/x$WT"
    fake 92104 "$C" tool "${WT}.bak/Cargo.toml"
    run "$WT"
    eq "exit 3" 3 "$RC"
    eq "exactly the whole-component spellings match" "92001,92002,92003,92004,92005" "$(pids)"
}

case_unreadable_and_non_pid_entries_are_skipped() {
    echo "unreadable_and_non_pid_entries_are_skipped"
    new_case skips
    mkdir -p "$PROCD/self" "$PROCD/sys" "$PROCD/92001"
    ln -s "$WT" "$PROCD/92001/cwd"                       # no cmdline file: unreadable
    mkdir -p "$PROCD/92002"
    printf '%s\0' vim > "$PROCD/92002/cmdline"           # no cwd: cannot place it
    mkdir -p "$PROCD/92003"
    ln -s "$C/gone" "$PROCD/92003/cwd"                   # dangling cwd
    printf '%s\0' vim > "$PROCD/92003/cmdline"
    fake 92004 "$WT" vim
    run "$WT"
    eq "exit 3" 3 "$RC"
    eq "only the readable holder" 92004 "$(pids)"
}

case_no_holders_exits_zero() {
    echo "no_holders_exits_zero"
    new_case none
    run "$WT"
    eq "exit 0" 0 "$RC"
    eq "no rows" "" "$OUT"
    contains "says so, with the canonical path" "no holders under $WT" "$ERR"
}

case_bad_arguments_refuse() {
    echo "bad_arguments_refuse"
    new_case args
    run
    eq "no path: exit 2" 2 "$RC"
    run --wait abc "$WT"
    eq "non-numeric --wait: exit 2" 2 "$RC"
    run --no-such-flag "$WT"
    eq "unknown option: exit 2" 2 "$RC"
    run "$WT" "$REPO"
    eq "two paths: exit 2" 2 "$RC"
    run --wait 5 "$WT"
    eq "a valid --wait is accepted" 0 "$RC"
}

case_the_script_does_not_list_itself() {
    echo "the_script_does_not_list_itself"
    new_case self
    # The real /proc, a real script process whose command line holds the worktree path, and
    # its own subshells. None of them may appear as a holder of the worktree they were
    # asked about.
    PROC_OVERRIDE=/proc run "$WT"
    eq "exit 0 on a real /proc" 0 "$RC"
    eq "no rows" "" "$OUT"
    contains "says so" "no holders under $WT" "$ERR"
}

case_lists_every_holder_kind
case_a_codescout_server_is_not_a_mux
case_a_sibling_prefix_is_not_a_holder
case_a_path_with_a_space_resolves
case_relative_trailing_slash_and_symlink_spellings_resolve
case_refusals
case_refuses_without_proc
case_stop_with_a_blocker_refuses_before_any_signal
case_kind_comes_from_argv0_not_comm
case_whole_component_matching
case_unreadable_and_non_pid_entries_are_skipped
case_no_holders_exits_zero
case_bad_arguments_refuse
case_the_script_does_not_list_itself

echo
echo "passed=$PASS failed=$FAIL"
[ "$FAIL" -eq 0 ]
