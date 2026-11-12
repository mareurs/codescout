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
# `<fixture>/<pid>/cmdline` (NUL-separated arguments). The scan and refusal cases start no
# real process, so they cannot depend on what else runs on the machine. Every fixture pid is
# above the kernel's `pid_max` (`fake` refuses anything else), so a `kill` of one is always
# ESRCH and no fixture case can signal a real process; the one exception is the entry for
# the script's own parent in `run_with_fake_parent`, which the script never signals. The
# stop cases start real processes inside their own temp
# worktrees (see `spawn`) and scan the real /proc. The script under test is run as a child
# (`bash <script>`), the same way an operator runs it, so the exit codes asserted here are
# the ones an operator sees.
#
# THE CASES THAT MATTER are the ones that separate a holder from a lookalike
# (`a_sibling_prefix_is_not_a_holder`, `whole_component_matching`) and a stoppable process
# from a blocker (`a_codescout_server_is_not_a_mux`, `kind_comes_from_argv0_not_comm`).
# A scan that listed everything, or classified everything as stoppable, would pass the
# cases that only look for one positive row.
#
# WHAT IT DOES NOT COVER
# ----------------------
#   * Signals to a process another session owns: the cases that signal start their own
#     processes inside the suite's temp worktrees and scan the real /proc, where only those
#     can match; fixture pids are above `pid_max`, so they cannot be anyone's process.
#   * That the stop loop signals only pids its latest scan still lists (a pid recycled
#     inside the pause after the mux signal): no case can make a pid change owner inside
#     that window, so that guard is covered by reading the code, not by a case.
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
# Every invocation of the script runs under an outer `timeout` (RUN_TIMEOUT, default 120 s),
# so a hang in the script fails a case instead of stalling the gate.
#
# WORKTREE_REMOVE_SCRIPT may name a copy of the script to run instead of
# scripts/worktree-remove.sh, so that a mutation check (a narrowed classifier arm, a dropped
# guard) can run this suite against an edited copy and the real file is never edited.

set -uo pipefail
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="${WORKTREE_REMOVE_SCRIPT:-$ROOT/scripts/worktree-remove.sh}"
PASS=0
FAIL=0
TAB=$'\t'
# Fixture pids are above the kernel's pid_max, where `kill` is always ESRCH.
FIXTURE_PID_BASE=5000001
PID_MAX=$(cat /proc/sys/kernel/pid_max 2>/dev/null || echo 4194304)
if [ "$FIXTURE_PID_BASE" -le "$PID_MAX" ]; then
    echo "refusing: pid_max ($PID_MAX) reaches the fixture pids ($FIXTURE_PID_BASE and up)" >&2
    exit 1
fi

T="$(mktemp -d "${TMPDIR:-/tmp}/worktree-remove-test.XXXXXX")" || exit 1
T="$(cd "$T" && pwd -P)"
case "$T" in /?*) ;; *) echo "refusing: bad temp dir [$T]" >&2; exit 1 ;; esac
# Every real process a case starts is recorded in SPAWNED and killed here, with its
# children, so a failing case leaves nothing behind. SIGKILL is fine in test code.
SPAWNED=()
cleanup() {
    local p
    for p in "${SPAWNED[@]+"${SPAWNED[@]}"}"; do
        pkill -KILL -P "$p" 2>/dev/null
        kill -KILL "$p" 2>/dev/null
    done
    wait 2>/dev/null
    rm -rf "$T"
}
trap cleanup EXIT
trap 'exit 130' INT TERM HUP
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
    [ "$pid" -gt "$PID_MAX" ] || { echo "setup failed: fixture pid $pid is not above pid_max" >&2; exit 1; }
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
    ( cd "${RUN_CWD:-$C}" && WORKTREE_REMOVE_PROC="${PROC_OVERRIDE:-$PROCD}" \
        timeout "${RUN_TIMEOUT:-120}" bash "$SRC" "$@" ) >"$C/out" 2>"$C/err"
    RC=$?
    OUT="$(<"$C/out")"
    ERR="$(<"$C/err")"
}

kinds() { printf '%s\n' "$OUT" | cut -f2 | paste -sd, -; }
pids()  { printf '%s\n' "$OUT" | cut -f1 | paste -sd, -; }

# run_real ARGS...: like run, but the script scans the REAL /proc. Only for cases whose real
# processes live in this case's temp worktree, so nothing outside it can be listed.
run_real() { PROC_OVERRIDE=/proc run "$@"; }

present() { if [ -e "$1" ] || [ -L "$1" ]; then echo present; else echo absent; fi; }

# alive PID: a live process. A zombie answers `kill -0` and is not alive.
alive() {
    local s
    s=$(sed -n 's/^State:[[:space:]]*\(.\).*/\1/p' "/proc/$1/status" 2>/dev/null)
    [ -n "$s" ] && [ "$s" != Z ]
}

# yesno CMD...: print yes or no by the exit status of CMD.
yesno() { if "$@"; then echo yes; else echo no; fi; }

# wait_file PATH: wait up to 3 s for PATH to exist.
wait_file() {
    local i
    for i in $(seq 1 60); do
        [ -e "$1" ] && return 0
        sleep 0.05
    done
    return 1
}

# spawn CWD ARGV0 CMD [ARG...]: start `exec -a ARGV0 CMD ARG...` in the background from CWD,
# record it for the EXIT trap, and wait until the exec has happened (the process shows
# ARGV0 as argv[0]; before it, it is a copy of this suite). Sets SP to the pid.
spawn() {
    local cwd=$1 argv0=$2 first i
    shift 2
    ( cd "$cwd" && exec -a "$argv0" "$@" ) &
    SP=$!
    SPAWNED+=("$SP")
    for i in $(seq 1 60); do
        first=""
        IFS= read -r -d '' first <"/proc/$SP/cmdline" 2>/dev/null
        [ "$first" = "$argv0" ] && return 0
        sleep 0.05
    done
    echo "setup failed: [$argv0] did not exec" >&2
    return 1
}

# dead_pid: print a pid no process can have (above pid_max): a `kill` of it is always ESRCH.
dead_pid() {
    echo "$FIXTURE_PID_BASE"
}

case_lists_every_holder_kind() {
    echo "lists_every_holder_kind"
    new_case kinds
    # Pids are scrambled against the expected order, so a listing that is merely sorted by
    # pid cannot pass.
    fake 5091001 "$WT" vim notes.txt
    fake 5091002 "$C" "$WT/target/debug/build/x/build-script-build"
    fake 5091003 "$C" cargo check --manifest-path "$WT/Cargo.toml"
    fake 5091004 "$WT" rust-analyzer
    fake 5091005 "$WT" codescout mux --cwd "$WT"
    # The other stoppable kinds: rust-analyzer is a prefix (its proc-macro server is another
    # binary), rustc and sccache are matched by name. Their pids are placed so that rank, not
    # pid, decides the order: sccache (rank 5) has a lower pid than rustc (rank 4).
    fake 5091006 "$WT" rust-analyzer-proc-macro-srv
    fake 5091007 "$C" sccache rustc --edition 2021 "$WT/src/lib.rs"
    fake 5091008 "$C" rustc --edition 2021 "$WT/src/lib.rs"
    run "$WT"
    eq "exit 3" 3 "$RC"
    eq "kinds, mux first and blocker last" \
        "mux,rust-analyzer,rust-analyzer,cargo,rustc,sccache,build-output,other" "$(kinds)"
    eq "order is by kind rank, then pid" \
        "5091005,5091004,5091006,5091003,5091008,5091007,5091002,5091001" "$(pids)"
    eq "first row is PID TAB KIND TAB CMDLINE" \
        "5091005${TAB}mux${TAB}codescout mux --cwd $WT" "$(printf '%s\n' "$OUT" | sed -n 1p)"
    eq "last row is the blocker" "5091001${TAB}other${TAB}vim notes.txt" \
        "$(printf '%s\n' "$OUT" | sed -n '$p')"
    contains "the occupancy warning goes to stderr" occupant "$ERR"
    lacks "the warning is not on stdout" occupant "$OUT"
}

case_a_codescout_server_is_not_a_mux() {
    echo "a_codescout_server_is_not_a_mux"
    new_case server
    fake 5091001 "$WT" codescout start --cwd "$WT"
    run "$WT"
    eq "exit 3" 3 "$RC"
    eq "a server is other, never mux" other "$(kinds)"
}

case_a_sibling_prefix_is_not_a_holder() {
    echo "a_sibling_prefix_is_not_a_holder"
    new_case sibling
    mkdir "$WT-other" "${WT}2"
    fake 5091001 "$WT-other" vim
    fake 5091002 "${WT}2" vim
    fake 5091003 "$C" vim "$WT-other/notes.txt"
    run "$WT"
    eq "exit 0: lookalikes are not holders" 0 "$RC"
    eq "no rows" "" "$OUT"
    contains "says so" "no holders under $WT" "$ERR"
    # With no holder the script removed the worktree; register it again for the next half.
    if [ ! -e "$WT" ]; then
        G "$REPO" worktree add -q "$WT" wt-sibling >/dev/null 2>&1 \
            || { echo "setup failed: worktree add again" >&2; exit 1; }
    fi
    fake 5091004 "$WT" vim
    run "$WT"
    eq "the real holder is still found beside the lookalikes" 5091004 "$(pids)"
}

case_a_path_with_a_space_resolves() {
    echo "a_path_with_a_space_resolves"
    new_case space "my wt"
    fake 5091001 "$WT" codescout mux --cwd "$WT"
    run "$WT"
    eq "exit 3" 3 "$RC"
    eq "the row is listed" mux "$(kinds)"
    contains "the full path is in the row" "$WT" "$OUT"
}

case_relative_trailing_slash_and_symlink_spellings_resolve() {
    echo "relative_trailing_slash_and_symlink_spellings_resolve"
    new_case spellings
    fake 5091001 "$WT" rust-analyzer
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
    fake 5091001 "$WT" vim
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
    fake 5091001 "$WT" codescout mux --cwd "$WT"
    fake 5091002 "$WT" vim notes.txt
    run --stop "$WT"
    eq "exit 4" 4 "$RC"
    contains "the blocker row is printed" "5091002${TAB}other${TAB}vim notes.txt" "$OUT"
    contains "stderr says nothing was signalled" "nothing was signalled" "$ERR"
    lacks "no signal line" "worktree-remove: signalled " "$ERR"
    lacks "no failed-signal line" "worktree-remove: could not signal " "$ERR"
}

case_kind_comes_from_argv0_not_comm() {
    echo "kind_comes_from_argv0_not_comm"
    new_case comm
    FAKE_COMM=rust-analyzer fake 5091001 "$WT" vim notes.txt
    FAKE_COMM=vim fake 5091002 "$WT" rust-analyzer
    FAKE_COMM=codescout fake 5091003 "$WT" /usr/bin/env codescout mux
    run "$WT"
    eq "exit 3" 3 "$RC"
    eq "by argv[0]: a vim whose comm says rust-analyzer is other, and the reverse" \
        "5091002${TAB}rust-analyzer${TAB}rust-analyzer" "$(printf '%s\n' "$OUT" | sed -n 1p)"
    eq "the other two are blockers" "rust-analyzer,other,other" "$(kinds)"
}

case_whole_component_matching() {
    echo "whole_component_matching"
    new_case component
    # cwd elsewhere in every case: only the command line can match.
    fake 5092001 "$C" tool "$WT"
    fake 5092002 "$C" tool --cwd "$WT"
    fake 5092003 "$C" tool "--cwd=$WT"
    fake 5092004 "$C" tool "--manifest-path=$WT/Cargo.toml"
    fake 5092005 "$C" tool "$WT/"
    fake 5092101 "$C" tool "$WT-other"
    fake 5092102 "$C" tool "${WT}2/x"
    fake 5092103 "$C" tool "/x$WT"
    fake 5092104 "$C" tool "${WT}.bak/Cargo.toml"
    run "$WT"
    eq "exit 3" 3 "$RC"
    eq "exactly the whole-component spellings match" "5092001,5092002,5092003,5092004,5092005" "$(pids)"
}

case_unreadable_and_non_pid_entries_are_skipped() {
    echo "unreadable_and_non_pid_entries_are_skipped"
    new_case skips
    mkdir -p "$PROCD/self" "$PROCD/sys" "$PROCD/5092001"
    ln -s "$WT" "$PROCD/5092001/cwd"                       # no cmdline file: unreadable
    mkdir -p "$PROCD/5092002"
    printf '%s\0' vim > "$PROCD/5092002/cmdline"           # no cwd: cannot place it
    mkdir -p "$PROCD/5092003"
    ln -s "$C/gone" "$PROCD/5092003/cwd"                   # dangling cwd
    printf '%s\0' vim > "$PROCD/5092003/cmdline"
    fake 5092004 "$WT" vim
    run "$WT"
    eq "exit 3" 3 "$RC"
    eq "only the readable holder" 5092004 "$(pids)"
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
    new_case args_zero
    run --wait 0 "$WT"
    eq "--wait 0 is accepted" 0 "$RC"
    new_case args_octal
    # Bash reads 08 and 09 as bad octal (an abort with exit 1) and 010 as 8: leading zeros
    # are refused up front, for the spaced and the `=` spelling alike.
    run --wait 08 "$WT"
    eq "--wait 08: exit 2" 2 "$RC"
    contains "--wait 08 names the reason" "leading zeros" "$ERR"
    run --wait 09 "$WT"
    eq "--wait 09: exit 2" 2 "$RC"
    run --wait 010 "$WT"
    eq "--wait 010: exit 2, not eight seconds" 2 "$RC"
    run --wait=010 "$WT"
    eq "--wait=010: exit 2" 2 "$RC"
    eq "the worktree is untouched" present "$(present "$WT")"
    run -h
    eq "-h: exit 0" 0 "$RC"
    contains "-h prints the usage" "usage: worktree-remove.sh" "$ERR"
    run --help "$WT"
    eq "--help: exit 0" 0 "$RC"
    eq "--help removes nothing" present "$(present "$WT")"
}

# run_with_fake_parent CWD_OF_PARENT ARG_FOR_PARENT_CMDLINE: run the script as a NON-exec child
# of a `bash -c` whose own pid is entered in the fixture proc with the given cwd and a
# command line ending in the given argument, so the script's $PPID is a known fixture pid.
# The script itself runs from the case dir, outside the worktree. Optional 3rd argument: script
# flags (word-split); optional 4th: the parent's argv[0] (default bash). The parent leaves
# $C/parent-done when the script returned and it was still running. Sets RC, OUT, ERR, PPID_FAKE.
run_with_fake_parent() {
    rm -f "$C/parent-done"
    ( cd "$C" && F="$PROCD" PCWD="$1" PARG="$2" SFLAGS="${3:-}" PARGV0="${4:-bash}" WT="$WT" S="$SRC" OUTDIR="$C" \
        WORKTREE_REMOVE_PROC="$PROCD" timeout "${RUN_TIMEOUT:-120}" bash -c '
            mkdir -p "$F/$$"
            ln -s "$PCWD" "$F/$$/cwd"
            printf "%s\0-c\0%s\0" "$PARGV0" "$PARG" >"$F/$$/cmdline"
            echo "$$" >"$OUTDIR/ppid"
            bash "$S" $SFLAGS "$WT"
            rc=$?
            : >"$OUTDIR/parent-done"
            exit $rc' ) >"$C/out" 2>"$C/err"
    RC=$?
    OUT="$(<"$C/out")"
    ERR="$(<"$C/err")"
    PPID_FAKE="$(<"$C/ppid")"
}

case_a_parent_whose_cwd_is_inside_is_a_holder() {
    echo "a_parent_whose_cwd_is_inside_is_a_holder"
    new_case parent
    # A launcher sitting inside the worktree that starts the script from another directory
    # holds the worktree through its cwd. Its command line does not name the path.
    run_with_fake_parent "$WT" x
    eq "exit 3: the parent is a holder" 3 "$RC"
    eq "the row is the parent's pid, kind other" "${PPID_FAKE}${TAB}other${TAB}bash -c x" "$OUT"
    # The skip's original purpose: a parent whose cwd is elsewhere and whose command line
    # merely names the worktree (because it launched the script) is not a holder.
    new_case parent_outside
    mkdir "$C/elsewhere"
    run_with_fake_parent "$C/elsewhere" "$WT"
    eq "exit 0: a parent that only names the path is not a holder" 0 "$RC"
    eq "no rows" "" "$OUT"
    contains "says so" "no holders under $WT" "$ERR"
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
case_the_parent_that_is_stoppable_by_kind_is_a_blocker() {
    echo "a_parent_whose_cwd_is_inside_is_never_signalled"
    new_case parent_stop
    # The launcher's argv[0] is `cargo`, a stoppable kind, and its cwd is inside the worktree.
    # --stop must treat it as a blocker: exit 4, and the launcher is still running when the
    # script returns (a SIGTERM would have ended it before it could leave the marker).
    run_with_fake_parent "$WT" x --stop cargo
    eq "exit 4: the parent is a blocker, whatever its kind" 4 "$RC"
    contains "stderr says nothing was signalled" "nothing was signalled" "$ERR"
    contains "the parent is named" "$PPID_FAKE" "$ERR"
    eq "the parent survived" present "$(present "$C/parent-done")"
    eq "the worktree is untouched" present "$(present "$WT")"
}

case_no_holders_removes_cleanly() {
    echo "no_holders_removes_cleanly"
    new_case clean
    run "$WT"
    eq "exit 0" 0 "$RC"
    contains "says there were no holders" "no holders under $WT" "$ERR"
    lacks "git no longer lists it" "$WT" "$(G "$REPO" worktree list --porcelain)"
    eq "the directory is gone" absent "$(present "$WT")"
    G "$REPO" rev-parse --verify -q refs/heads/wt-clean >/dev/null
    eq "the branch still exists" 0 "$?"
}

case_a_detached_head_worktree_is_removed() {
    echo "a_detached_head_worktree_is_removed"
    new_case detached
    G "$REPO" worktree add -q --detach "$C/dwt" >/dev/null 2>&1 \
        || { echo "setup failed: detached worktree add" >&2; exit 1; }
    run "$C/dwt"
    eq "exit 0" 0 "$RC"
    lacks "git no longer lists it" "$C/dwt" "$(G "$REPO" worktree list --porcelain)"
    eq "the directory is gone" absent "$(present "$C/dwt")"
    eq "the other worktree is untouched" present "$(present "$WT")"
}

case_holders_without_stop_signal_nothing() {
    echo "holders_without_stop_signal_nothing"
    new_case nostop
    spawn "$WT" rust-analyzer sleep 300 || { bad "setup failed in ${FUNCNAME[0]}" ok failed; return; }
    local p=$SP
    run_real "$WT"
    eq "exit 3" 3 "$RC"
    contains "the holder is listed" "${p}${TAB}rust-analyzer${TAB}" "$OUT"
    eq "the holder is still alive" yes "$(yesno alive "$p")"
    eq "the worktree is untouched" present "$(present "$WT")"
    lacks "no signal line" "worktree-remove: signalled " "$ERR"
    lacks "no failed-signal line" "worktree-remove: could not signal " "$ERR"
}

case_stop_ends_a_stoppable_holder_and_removes() {
    echo "stop_ends_a_stoppable_holder_and_removes"
    new_case stop
    spawn "$WT" rust-analyzer sleep 300 || { bad "setup failed in ${FUNCNAME[0]}" ok failed; return; }
    local p=$SP
    run_real --stop --wait 5 "$WT"
    eq "exit 0" 0 "$RC"
    eq "the holder is gone" no "$(yesno alive "$p")"
    eq "the directory is gone" absent "$(present "$WT")"
    lacks "git no longer lists it" "$WT" "$(G "$REPO" worktree list --porcelain)"
}

case_stop_with_a_blocker_leaves_the_stoppable_alive() {
    echo "stop_with_a_blocker_leaves_the_stoppable_alive"
    new_case blocker_real
    spawn "$WT" rust-analyzer sleep 300 || { bad "setup failed in ${FUNCNAME[0]}" ok failed; return; }
    local ra=$SP
    spawn "$WT" sleep sleep 300 || { bad "setup failed in ${FUNCNAME[0]}" ok failed; return; }
    local pl=$SP
    run_real --stop --wait 2 "$WT"
    eq "exit 4" 4 "$RC"
    contains "the blocker is named" "$pl" "$ERR"
    eq "the rust-analyzer-named process is still alive" yes "$(yesno alive "$ra")"
    eq "the blocker is still alive" yes "$(yesno alive "$pl")"
    eq "the worktree is untouched" present "$(present "$WT")"
}

case_a_codescout_server_under_target_stays_a_blocker() {
    echo "a_codescout_server_under_target_stays_a_blocker"
    new_case server_stop
    local dp
    dp=$(dead_pid)
    fake "$dp" "$C" "$WT/target/debug/codescout" start --cwd "$WT"
    run --stop --wait 1 "$WT"
    eq "exit 4: a server is not a mux" 4 "$RC"
    eq "it is listed as other" other "$(kinds)"
}

case_stop_signals_the_mux_before_the_other_holders() {
    echo "stop_signals_the_mux_before_the_other_holders"
    new_case order
    mkfifo "$C/fifo" || { bad "setup failed in ${FUNCNAME[0]}" ok failed; return; }
    mkdir "$C/muxdir"
    export FIFO="$C/fifo" LOG="$C/order.log" MUX_READY="$C/mux.ready" CARGO_READY="$C/cargo.ready"
    # The mux takes 0.4 s to finish its SIGTERM handler, then writes `mux`. A cargo that was
    # signalled together with it would write `cargo` first.
    cat >"$C/muxdir/mux" <<'EOF'
trap 'read -t 0.4 x <>"$FIFO"; echo mux >>"$LOG"; exit 0' TERM
: >"$MUX_READY"
while :; do read -t 1 x <>"$FIFO"; done
EOF
    cat >"$C/cargo-stub" <<'EOF'
trap 'echo cargo >>"$LOG"; exit 0' TERM
: >"$CARGO_READY"
while :; do read -t 1 x <>"$FIFO"; done
EOF
    spawn "$C/muxdir" codescout bash mux --cwd "$WT" || { bad "setup failed in ${FUNCNAME[0]}" ok failed; return; }
    local pm=$SP
    spawn "$WT" cargo bash "$C/cargo-stub" || { bad "setup failed in ${FUNCNAME[0]}" ok failed; return; }
    local pc=$SP
    wait_file "$MUX_READY" && wait_file "$CARGO_READY" || { bad "stubs are ready" ready "not ready"; return; }
    run_real --stop --wait 5 "$WT"
    eq "exit 0" 0 "$RC"
    contains "both were listed, the mux first" "${pm}${TAB}mux${TAB}" "$(printf '%s\n' "$OUT" | sed -n 1p)"
    eq "the mux was signalled and finished before the cargo" "mux,cargo" "$(paste -sd, "$LOG" 2>/dev/null)"
    eq "both are gone" "no,no" "$(yesno alive "$pm"),$(yesno alive "$pc")"
    eq "the directory is gone" absent "$(present "$WT")"
    unset FIFO LOG MUX_READY CARGO_READY
}

case_stop_prints_each_signal_it_sends() {
    echo "stop_prints_each_signal_it_sends"
    new_case sigline
    mkfifo "$C/fifo" || { bad "setup failed in ${FUNCNAME[0]}" ok failed; return; }
    mkdir "$C/muxdir"
    export FIFO="$C/fifo" MUX_READY="$C/mux.ready" CARGO_READY="$C/cargo.ready"
    cat >"$C/muxdir/mux" <<'EOF'
trap 'exit 0' TERM
: >"$MUX_READY"
while :; do read -t 1 x <>"$FIFO"; done
EOF
    cat >"$C/cargo-stub" <<'EOF'
trap 'exit 0' TERM
: >"$CARGO_READY"
while :; do read -t 1 x <>"$FIFO"; done
EOF
    spawn "$C/muxdir" codescout bash mux --cwd "$WT" || { bad "setup failed in ${FUNCNAME[0]}" ok failed; return; }
    local pm=$SP
    spawn "$WT" cargo bash "$C/cargo-stub" || { bad "setup failed in ${FUNCNAME[0]}" ok failed; return; }
    local pc=$SP
    wait_file "$MUX_READY" && wait_file "$CARGO_READY" || { bad "stubs are ready" ready "not ready"; return; }
    run_real --stop --wait 5 "$WT"
    eq "exit 0" 0 "$RC"
    contains "the mux signal is printed" "worktree-remove: signalled ${pm} mux with SIGTERM" "$ERR"
    contains "the cargo signal is printed" "worktree-remove: signalled ${pc} cargo with SIGTERM" "$ERR"
    local before_mux=${ERR%%"signalled ${pm} mux"*} before_cargo=${ERR%%"signalled ${pc} cargo"*}
    eq "the mux line comes before the cargo line" yes \
        "$([ "${#before_mux}" -lt "${#before_cargo}" ] && echo yes || echo no)"
    unset FIFO MUX_READY CARGO_READY
}

case_a_holder_that_ignores_sigterm_is_a_survivor() {
    echo "a_holder_that_ignores_sigterm_is_a_survivor"
    new_case ignore
    mkfifo "$C/fifo" || { bad "setup failed in ${FUNCNAME[0]}" ok failed; return; }
    # `read -t` rather than `sleep 1` in the loop: a `sleep` child would inherit the worktree as
    # its cwd and be listed as a second holder, of kind other, which is a blocker.
    spawn "$WT" cargo bash -c 'trap "" TERM; : >"$1"; while :; do read -t 1 x <>"$2"; done' \
        cargo "$C/ready" "$C/fifo" || { bad "setup failed in ${FUNCNAME[0]}" ok failed; return; }
    local p=$SP
    wait_file "$C/ready" || { bad "the holder is ready" ready "not ready"; return; }
    run_real --stop --wait 2 "$WT"
    eq "exit 5" 5 "$RC"
    contains "the survivor is listed" "${p}${TAB}cargo${TAB}" "$ERR"
    eq "it is still alive: SIGTERM only" yes "$(yesno alive "$p")"
    eq "the worktree is not removed" present "$(present "$WT")"
    contains "git still lists it" "$WT" "$(G "$REPO" worktree list --porcelain)"
}

case_a_holder_that_exits_before_the_signal_is_not_an_error() {
    echo "a_holder_that_exits_before_the_signal_is_not_an_error"
    new_case gone
    local dp
    dp=$(dead_pid)
    # A fixture entry for a pid that no longer exists: the signal fails with ESRCH, and the
    # entry never disappears, so the rescan still lists it.
    fake "$dp" "$WT" cargo check
    run --stop --wait 1 "$WT"
    eq "exit 5: the entry is still listed, so it is a survivor" 5 "$RC"
    lacks "no shell error text" "No such process" "$ERR"
    contains "the row is listed" "${dp}${TAB}cargo${TAB}cargo check" "$ERR"
    contains "the failed signal is printed" "worktree-remove: could not signal ${dp} cargo (already gone, or not permitted)" "$ERR"
    eq "the worktree is not removed" present "$(present "$WT")"
}

case_untracked_files_make_git_refuse() {
    echo "untracked_files_make_git_refuse"
    new_case untracked
    echo data >"$WT/notes.txt"
    # An ignored file does not block git, so it must not be listed (the mutant that adds
    # --ignored back lists it).
    echo 'ignored.log' >>"$REPO/.git/info/exclude"
    echo data >"$WT/ignored.log"
    run "$WT"
    eq "exit 6" 6 "$RC"
    lacks "ignored files are not listed" "ignored.log" "$ERR"
    contains "the status listing is printed" "?? notes.txt" "$ERR"
    contains "the header names the command, without --ignored" \
        "git -C $WT status --porcelain -uall:" "$ERR"
    lacks "a short listing has no remainder line" "... and" "$ERR"
    eq "the directory is still there, not forced" present "$(present "$WT/notes.txt")"
    contains "git still lists the worktree" "$WT" "$(G "$REPO" worktree list --porcelain)"
}

case_a_leftover_directory_is_reported_not_deleted() {
    echo "a_leftover_directory_is_reported_not_deleted"
    new_case leftover
    mkdir "$C/shim"
    # A git that behaves as git, except that after `worktree remove` it recreates a directory
    # in the removed worktree, as a dying `cargo check` rewrites target/.
    cat >"$C/shim/git" <<'EOF'
#!/usr/bin/env bash
prev=""
for a in "$@"; do
    if [ "$prev" = worktree ] && [ "$a" = remove ]; then
        "$SHIM_REAL_GIT" "$@"
        rc=$?
        mkdir -p "$SHIM_WT/target"
        exit $rc
    fi
    prev=$a
done
exec "$SHIM_REAL_GIT" "$@"
EOF
    chmod +x "$C/shim/git"
    SHIM_REAL_GIT="$(command -v git)" SHIM_WT="$WT" PATH="$C/shim:$PATH" run "$WT"
    eq "exit 7" 7 "$RC"
    contains "the contents are printed" target "$ERR"
    eq "the directory is still there, not deleted" present "$(present "$WT/target")"
    lacks "git no longer lists it" "$WT" "$(G "$REPO" worktree list --porcelain)"
}

case_a_scan_that_blocks_is_bounded() {
    echo "a_scan_that_blocks_is_bounded"
    new_case blockscan
    # An entry whose cmdline is a FIFO nobody writes to: opening it for reading blocks, as a
    # read of a /proc/<pid>/cmdline blocks on a process that holds its memory-map lock. The
    # scan runs in a child that `timeout` kills together with whatever it blocked in, so no
    # blocked reader outlives the case.
    mkdir -p "$PROCD/5099001"
    mkfifo "$PROCD/5099001/cmdline" || { bad "setup failed in ${FUNCNAME[0]}" ok failed; return; }
    ln -s "$WT" "$PROCD/5099001/cwd"
    local t0=$SECONDS
    WORKTREE_REMOVE_SCAN_TIMEOUT=1 RUN_TIMEOUT=20 run "$WT"
    local took=$((SECONDS - t0))
    eq "exit 2: the script's own code, not the outer timeout's 124" 2 "$RC"
    contains "says the scan timed out" "timed out" "$ERR"
    contains "says nothing was changed" "nothing was changed" "$ERR"
    eq "bounded: done within 10 s" yes "$([ "$took" -lt 10 ] && echo yes || echo no)"
    eq "the worktree is untouched" present "$(present "$WT")"
    contains "git still lists it" "$WT" "$(G "$REPO" worktree list --porcelain)"
    WORKTREE_REMOVE_SCAN_TIMEOUT=1 RUN_TIMEOUT=20 run --stop "$WT"
    eq "with --stop too: exit 2, nothing signalled or removed" 2 "$RC"
    eq "the worktree is untouched" present "$(present "$WT")"
}

case_the_exit_6_listing_is_capped_and_omits_ignored_files() {
    echo "the_exit_6_listing_is_capped_and_omits_ignored_files"
    new_case cap
    # Ignored files never block `git worktree remove`; listing them buries the cause. The
    # ignore rule lives in the main repository's exclude file, shared by its worktrees.
    echo 'ignored.log' >>"$REPO/.git/info/exclude"
    echo data >"$WT/ignored.log"
    local i
    for i in $(seq 1 60); do echo data >"$WT/u$i.txt"; done
    run "$WT"
    eq "exit 6" 6 "$RC"
    contains "the header names the command, without --ignored" \
        "git -C $WT status --porcelain -uall:" "$ERR"
    lacks "--ignored is not used" "--ignored" "$ERR"
    lacks "the ignored file is not listed" "ignored.log" "$ERR"
    eq "fifty untracked files are listed" 50 "$(printf '%s\n' "$ERR" | grep -c '^    ?? ')"
    contains "the rest is counted" "    ... and 10 more" "$ERR"
    eq "nothing was deleted" present "$(present "$WT/ignored.log")"
}

# git_shim DIR: a `git` that behaves as git except for `worktree remove`, which SHIM_MODE changes:
#   leave   the real remove succeeds, then a live process rewrites target/ and git fails with
#           `Directory not empty` after it deregistered the worktree (the bug itself, exit 255)
#   gone    the real remove succeeds, then git exits 255 anyway; nothing is left behind
#   dotgit  git deletes the worktree's .git file and exits 255, still registered
git_shim() {
    mkdir -p "$1"
    cat >"$1/git" <<'EOF'
#!/usr/bin/env bash
prev=""
for a in "$@"; do
    if [ "$prev" = worktree ] && [ "$a" = remove ]; then
        case $SHIM_MODE in
            leave)
                "$SHIM_REAL_GIT" "$@" >/dev/null 2>&1
                mkdir -p "$SHIM_WT/target"
                echo data >"$SHIM_WT/target/rewritten"
                echo "error: failed to delete '$SHIM_WT': Directory not empty" >&2
                exit 255
                ;;
            gone)
                "$SHIM_REAL_GIT" "$@" >/dev/null 2>&1
                echo "error: failed to delete '$SHIM_WT': Directory not empty" >&2
                exit 255
                ;;
            dotgit)
                rm -f "$SHIM_WT/.git"
                echo "error: failed to delete '$SHIM_WT/.git'" >&2
                exit 255
                ;;
        esac
    fi
    prev=$a
done
exec "$SHIM_REAL_GIT" "$@"
EOF
    chmod +x "$1/git"
}

case_a_git_that_fails_after_removing_the_worktree_is_not_a_refusal() {
    echo "a_git_that_fails_after_removing_the_worktree_is_not_a_refusal"
    new_case gitfail
    # The case directory is itself a repository, as a checkout enclosing a worktree path is: with
    # the worktree's .git gone, a `git status` run inside it would report THIS repository.
    git init -q -b main "$C" >/dev/null 2>&1 || { echo "setup failed: git init enclosing" >&2; exit 1; }
    echo data >"$C/enclosing-marker.txt"
    git_shim "$C/shim"
    local real
    real="$(command -v git)"

    # The bug: git deleted the work tree and deregistered it, then failed on a rewritten target/.
    SHIM_MODE=leave SHIM_REAL_GIT="$real" SHIM_WT="$WT" PATH="$C/shim:$PATH" run "$WT"
    eq "exit 7, not 6: the directory is left behind" 7 "$RC"
    contains "git's own message is shown" "Directory not empty" "$ERR"
    contains "the leftover is listed" "  target" "$ERR"
    lacks "it is not called a refusal" "refused" "$ERR"
    lacks "no other repository's status is printed" "enclosing-marker.txt" "$ERR"
    eq "the leftover is not deleted" present "$(present "$WT/target/rewritten")"
    lacks "git no longer lists it" "$WT" "$(G "$REPO" worktree list --porcelain)"

    # Git failed after removing everything: the path is gone and unregistered, so exit 0.
    rm -rf "$WT"
    G "$REPO" worktree add -q "$WT" wt-gitfail >/dev/null 2>&1 \
        || { echo "setup failed: worktree add again" >&2; exit 1; }
    SHIM_MODE=gone SHIM_REAL_GIT="$real" SHIM_WT="$WT" PATH="$C/shim:$PATH" run "$WT"
    eq "exit 0: the path is absent and unregistered" 0 "$RC"
    contains "it says git failed after removing" "git reported a failure after removing it" "$ERR"
    eq "the directory is gone" absent "$(present "$WT")"
    lacks "git no longer lists it" "$WT" "$(G "$REPO" worktree list --porcelain)"

    # Git failed and still lists the worktree, whose .git file is gone: exit 6, and no `git status`
    # (it would climb to the enclosing repository).
    G "$REPO" worktree add -q "$WT" wt-gitfail >/dev/null 2>&1 \
        || { echo "setup failed: worktree add third time" >&2; exit 1; }
    SHIM_MODE=dotgit SHIM_REAL_GIT="$real" SHIM_WT="$WT" PATH="$C/shim:$PATH" run "$WT"
    eq "exit 6: git still lists it" 6 "$RC"
    contains "it says why there is no status" "does not exist: git status skipped" "$ERR"
    lacks "no other repository's status is printed" "enclosing-marker.txt" "$ERR"
    contains "git still lists the worktree" "$WT" "$(G "$REPO" worktree list --porcelain)"
}

case_control_characters_in_a_command_line_are_scrubbed() {
    echo "control_characters_in_a_command_line_are_scrubbed"
    new_case cntrl
    # CR would overwrite the start of the row, ESC starts a terminal control sequence: a command
    # line must not be able to forge the table an operator decides on.
    fake 5091001 "$WT" vim $'evil\rFORGED' $'\e[2Jwiped' $'tab\there'
    run "$WT"
    eq "exit 3" 3 "$RC"
    lacks "stdout has no CR" $'\r' "$OUT"
    lacks "stdout has no ESC" $'\e' "$OUT"
    contains "the text stays, the controls became spaces" "vim evil FORGED  [2Jwiped tab here" "$OUT"
    eq "still one row" 1 "$(printf '%s\n' "$OUT" | wc -l)"
    run --stop "$WT"
    eq "with --stop: exit 4" 4 "$RC"
    contains "the blocker is listed" "5091001" "$ERR"
    lacks "the blocker list has no CR" $'\r' "$ERR"
    lacks "the blocker list has no ESC" $'\e' "$ERR"
    new_case cntrl_survivor
    fake 5091001 "$WT" cargo $'check\r\e[31mred'
    run --stop --wait 1 "$WT"
    eq "a survivor: exit 5" 5 "$RC"
    contains "the survivor is listed" "5091001${TAB}cargo${TAB}cargo check  [31mred" "$ERR"
    lacks "the survivor list has no CR" $'\r' "$ERR"
    lacks "the survivor list has no ESC" $'\e' "$ERR"
}

case_a_blocker_that_appears_after_the_first_scan_exits_4_before_any_signal() {
    echo "a_blocker_that_appears_after_the_first_scan_exits_4_before_any_signal"
    new_case late_blocker
    # A stoppable fixture holder (a pid above pid_max: a signal to it is always ESRCH). Main's scan
    # sees only it. A `timeout` shim adds a blocker, a `vim` in the worktree, to the fixture just
    # before the SECOND scan, the first one of the stop loop. The script must refuse with exit 4
    # and signal nothing; one that signalled the cargo anyway would wait out --wait and end in
    # exit 5.
    fake 5091001 "$WT" cargo check
    mkdir "$C/shim"
    cat >"$C/shim/timeout" <<'EOF'
#!/usr/bin/env bash
case " $* " in
    *" --internal-scan "*)
        n=$(cat "$SHIM_COUNT" 2>/dev/null || echo 0)
        n=$((n + 1))
        echo "$n" >"$SHIM_COUNT"
        if [ "$n" -eq 2 ]; then
            mkdir -p "$SHIM_PROC/5091002"
            ln -s "$SHIM_WT" "$SHIM_PROC/5091002/cwd"
            printf '%s\0' vim >"$SHIM_PROC/5091002/cmdline"
        fi
        ;;
esac
exec "$SHIM_REAL_TIMEOUT" "$@"
EOF
    chmod +x "$C/shim/timeout"
    SHIM_COUNT="$C/count" SHIM_PROC="$PROCD" SHIM_WT="$WT" SHIM_REAL_TIMEOUT="$(command -v timeout)" \
        PATH="$C/shim:$PATH" run --stop --wait 1 "$WT"
    eq "exit 4: the blocker is found before any signal" 4 "$RC"
    contains "the late blocker is named" "5091002  vim" "$ERR"
    contains "nothing was signalled" "nothing was signalled" "$ERR"
    lacks "it did not go on to a survivor report" "still there after SIGTERM" "$ERR"
    eq "two scans only: main's and the stop loop's first" 2 "$(cat "$C/count")"
    eq "the worktree is untouched" present "$(present "$WT")"
}

case_a_parent_whose_cwd_is_inside_is_a_holder
case_a_scan_that_blocks_is_bounded
case_the_parent_that_is_stoppable_by_kind_is_a_blocker
case_no_holders_removes_cleanly
case_a_detached_head_worktree_is_removed
case_holders_without_stop_signal_nothing
case_stop_ends_a_stoppable_holder_and_removes
case_stop_with_a_blocker_leaves_the_stoppable_alive
case_a_codescout_server_under_target_stays_a_blocker
case_stop_signals_the_mux_before_the_other_holders
case_stop_prints_each_signal_it_sends
case_a_holder_that_ignores_sigterm_is_a_survivor
case_a_holder_that_exits_before_the_signal_is_not_an_error
case_untracked_files_make_git_refuse
case_a_leftover_directory_is_reported_not_deleted
case_the_exit_6_listing_is_capped_and_omits_ignored_files
case_a_git_that_fails_after_removing_the_worktree_is_not_a_refusal
case_control_characters_in_a_command_line_are_scrubbed
case_a_blocker_that_appears_after_the_first_scan_exits_4_before_any_signal

echo
echo "passed=$PASS failed=$FAIL"
[ "$FAIL" -eq 0 ]
