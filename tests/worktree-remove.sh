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
# real process, so they cannot depend on what else runs on the machine, and none of them
# can signal a real process. The stop cases start real processes inside their own temp
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
#     can match. The one case that signals a fixture pid uses a pid that is dead (reaped),
#     so only a recycled pid could reach a stranger.
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

# dead_pid: print the pid of a process that has exited and been reaped.
dead_pid() {
    sleep 0 &
    local p=$!
    wait "$p"
    echo "$p"
}

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
    # With no holder the script removed the worktree; register it again for the next half.
    if [ ! -e "$WT" ]; then
        G "$REPO" worktree add -q "$WT" wt-sibling >/dev/null 2>&1 \
            || { echo "setup failed: worktree add again" >&2; exit 1; }
    fi
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

# run_with_fake_parent CWD_OF_PARENT ARG_FOR_PARENT_CMDLINE: run the script as a NON-exec child
# of a `bash -c` whose own pid is entered in the fixture proc with the given cwd and a
# command line ending in the given argument, so the script's $PPID is a known fixture pid.
# The script itself runs from the case dir, outside the worktree. Optional 3rd argument: script
# flags (word-split); optional 4th: the parent's argv[0] (default bash). The parent leaves
# $C/parent-done when the script returned and it was still running. Sets RC, OUT, ERR, PPID_FAKE.
run_with_fake_parent() {
    rm -f "$C/parent-done"
    ( cd "$C" && F="$PROCD" PCWD="$1" PARG="$2" SFLAGS="${3:-}" PARGV0="${4:-bash}" WT="$WT" S="$SRC" OUTDIR="$C" \
        WORKTREE_REMOVE_PROC="$PROCD" bash -c '
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
    spawn "$WT" rust-analyzer sleep 300 || return
    local p=$SP
    run_real "$WT"
    eq "exit 3" 3 "$RC"
    contains "the holder is listed" "${p}${TAB}rust-analyzer${TAB}" "$OUT"
    eq "the holder is still alive" yes "$(yesno alive "$p")"
    eq "the worktree is untouched" present "$(present "$WT")"
}

case_stop_ends_a_stoppable_holder_and_removes() {
    echo "stop_ends_a_stoppable_holder_and_removes"
    new_case stop
    spawn "$WT" rust-analyzer sleep 300 || return
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
    spawn "$WT" rust-analyzer sleep 300 || return
    local ra=$SP
    spawn "$WT" sleep sleep 300 || return
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
    mkfifo "$C/fifo" || return
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
    spawn "$C/muxdir" codescout bash mux --cwd "$WT" || return
    local pm=$SP
    spawn "$WT" cargo bash "$C/cargo-stub" || return
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

case_a_holder_that_ignores_sigterm_is_a_survivor() {
    echo "a_holder_that_ignores_sigterm_is_a_survivor"
    new_case ignore
    mkfifo "$C/fifo" || return
    # `read -t` rather than `sleep 1` in the loop: a `sleep` child would inherit the worktree as
    # its cwd and be listed as a second holder, of kind other, which is a blocker.
    spawn "$WT" cargo bash -c 'trap "" TERM; : >"$1"; while :; do read -t 1 x <>"$2"; done' \
        cargo "$C/ready" "$C/fifo" || return
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
    eq "the worktree is not removed" present "$(present "$WT")"
}

case_untracked_files_make_git_refuse() {
    echo "untracked_files_make_git_refuse"
    new_case untracked
    echo data >"$WT/notes.txt"
    run "$WT"
    eq "exit 6" 6 "$RC"
    contains "the status listing is printed" "?? notes.txt" "$ERR"
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

case_a_parent_whose_cwd_is_inside_is_a_holder
case_the_parent_that_is_stoppable_by_kind_is_a_blocker
case_no_holders_removes_cleanly
case_a_detached_head_worktree_is_removed
case_holders_without_stop_signal_nothing
case_stop_ends_a_stoppable_holder_and_removes
case_stop_with_a_blocker_leaves_the_stoppable_alive
case_a_codescout_server_under_target_stays_a_blocker
case_stop_signals_the_mux_before_the_other_holders
case_a_holder_that_ignores_sigterm_is_a_survivor
case_a_holder_that_exits_before_the_signal_is_not_an_error
case_untracked_files_make_git_refuse
case_a_leftover_directory_is_reported_not_deleted

echo
echo "passed=$PASS failed=$FAIL"
[ "$FAIL" -eq 0 ]
