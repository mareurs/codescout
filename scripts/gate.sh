#!/usr/bin/env bash
# Run the light Python lane and the Rust lanes (clippy, then for each of the lean and default test lanes a
# nextest run and a doctest run) in a `target/` LEASED for this run, so the
# lane cannot be corrupted by — or corrupt — another session sharing this checkout.
#
# WHY THIS EXISTS, and why ordering the lanes correctly is not enough.
# `docs/issues/2026-09-14-the-gate-ordering-guarantee-is-false-under-concurrency.md`
#
#   Measured 2026-09-14, with the control that makes it a measurement: cargo holds
#   `target/debug/.cargo-lock` through the BUILD phase (holder pid observed on four
#   consecutive samples) and RELEASES IT BEFORE RUNNING TESTS (no holder, sampled with
#   three `cli_doc` processes alive). So a peer's `--no-default-features` build can
#   replace `target/debug/codescout` inside YOUR OWN lane's run phase, however either
#   session orders its lanes. A peer following the gate perfectly still writes a
#   librarian-less binary to the shared path once, mid-sequence. Their compliance cannot
#   help you and neither can yours — which is what makes "provided both lanes actually
#   run" a condition every party satisfies while the guarantee fails.
#
# WHAT THIS DOES NOT TOUCH, deliberately: `cargo rb`. That builds `--release`, and
# `~/.cargo/bin/codescout` is a symlink to `<repo>/target/release/codescout`. Isolating
# the release profile too would point that symlink at a path nothing rebuilds, so the
# live MCP binary would go permanently stale for every session on every profile. The
# gate lanes are isolated; the release loop stays shared and keeps working.
#
# THE POOL, and why a lease rather than a directory per session. The race above lasts one
# RUN, so a slot is held for one run and reused by the next, whichever session starts it:
# keying on the session id left 323G in 17 trees on 2026-09-24 (bug 1da62c896d649aa6).
# The lease lives in scripts/slot-pool.sh, which also bounds the pool on every lease:
# a slot past CODESCOUT_SLOT_CEILING_MB is emptied, and free slots numbered
# CODESCOUT_GATE_POOL_KEEP or higher are removed (bug 503fa887ab0ec144), and a free slot
# whose lessee stamp is older than CODESCOUT_SLOT_IDLE_DAYS (default 3, 0 = off) is removed
# (bug e7442516ad9fa057). Why the lock is
# held without `flock -o`, and why only the lock may authorize a removal, is written
# there. This script prints the slot's size and the pool's total when it finishes.
#
# THE PATH IT PRINTS IS LEASED FOR THIS RUN ONLY. Reusing it by hand for a targeted
# `cargo test` writes into a slot another run may hold; scripts/with-slot.sh is the leased
# way to do that, and the line below says so where the path appears (bug
# 097aa5ca2222a91d).
#
# NOT MANDATORY, and that is a real limitation rather than modesty: a session that types
# the four commands directly still shares `target/`, so this is a mechanism for whoever
# runs it and a policy for everyone else. The gate sentence in CLAUDE.md remains the
# canonical statement of WHAT runs, in what order, and is pinned byte-for-byte by
# `claude_md_gate_lists_its_four_commands_in_the_load_bearing_order`.

set -u

CHECK_ONLY=0
CONTEXT=""
EXCLUSIVE=""
while [ "$#" -gt 0 ]; do
    arg="$1"; shift
    case "$arg" in
        --check) [ "$CHECK_ONLY" -eq 0 ] || { echo "gate.sh: repeated --check" >&2; exit 2; }; CHECK_ONLY=1 ;;
        --context|--exclusive-checkout)
            [ "$#" -gt 0 ] && [ -n "$1" ] || { echo "gate.sh: $arg requires an absolute path" >&2; exit 2; }
            case "$1" in /*) ;; *) echo "gate.sh: $arg requires an absolute path" >&2; exit 2 ;; esac
            if [ "$arg" = "--context" ]; then
                [ -z "$CONTEXT" ] || exit 2; CONTEXT="$1"
            else
                [ -z "$EXCLUSIVE" ] || exit 2; EXCLUSIVE="$1"
            fi
            shift ;;
        *) echo "gate.sh: unknown argument $arg" >&2; exit 2 ;;
    esac
done
[ "$CHECK_ONLY" -eq 0 ] || [ -z "$EXCLUSIVE" ] || { echo "gate.sh: conflicting formatter modes" >&2; exit 2; }
ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || { echo "gate.sh: not inside a Git checkout" >&2; exit 2; }
cd "$ROOT" || exit 2
ROOT=$(pwd -P)
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
# nextest runs both test lanes (docs/conventions/gate-ordering.md, "Why the test lanes run nextest").
# The check comes before the formatter context, the lease and every lane, and there is NO fallback to
# `cargo test`: a gate that silently ran libtest on one machine and nextest on another would be two
# different gates under one name. `cargo nextest --version` is a subcommand lookup, not a lane.
cargo nextest --version >/dev/null 2>&1 || {
    echo "gate.sh: cargo-nextest is not installed, and the gate's test lanes need it (no fallback to libtest)." >&2
    echo "gate.sh: install it with: cargo install cargo-nextest --locked" >&2
    exit 2
}
if [ -n "$EXCLUSIVE" ]; then
    python3 -c 'import pathlib,sys; p=pathlib.Path(sys.argv[1]); raise SystemExit(0 if p.is_absolute() and p.resolve(strict=True)==pathlib.Path(sys.argv[2]) else 2)' "$EXCLUSIVE" "$ROOT" || {
        echo "gate.sh: exclusive declaration must match the actual Git root" >&2; exit 2;
    }
fi

# The light Python lane (started below) runs in the background beside the Rust lanes. An exit before
# it is joined must not leave it behind, so the EXIT trap stops it: `pkill -P` takes its children by
# parent pid, never by a pattern that could match a peer's process.
PY_PID=""
PY_DIR=""
GATE_TMP=""
gate_stop_python() {
    if [ -n "$PY_PID" ]; then pkill -P "$PY_PID" 2>/dev/null; kill "$PY_PID" 2>/dev/null; fi
    if [ -n "$PY_DIR" ]; then rm -rf -- "$PY_DIR"; fi
    return 0
}
# The private test TMPDIR (made below) goes whatever the exit: green, red or a signal. The pattern guard
# means a bad value of GATE_TMP can never make this remove something that is not the gate's own directory.
gate_remove_tmp() {
    case "$GATE_TMP" in */codescout-gate-tmp.*) rm -rf -- "$GATE_TMP" ;; esac
    return 0
}
CONTEXT_SNAPSHOT=""
trap 'rm -f -- "${CONTEXT_SNAPSHOT:-}"; gate_stop_python; gate_remove_tmp' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
CONTEXT_BLOCKED=0
FMT_ARGS=()
if [ "$CHECK_ONLY" -eq 1 ]; then
    FMT_ARGS=(--check)
else
    CONTEXT_ARGS=()
    [ -z "$CONTEXT" ] || CONTEXT_ARGS=(--context "$CONTEXT")
    RESOLVE_RC=0
    RESOLUTION=$(python3 "$SCRIPT_DIR/session_context.py" --workspace "$ROOT" "${CONTEXT_ARGS[@]}") || RESOLVE_RC=$?
    STATE=$(printf '%s' "$RESOLUTION" | python3 -c 'import json,sys; print(json.load(sys.stdin)["state"])') || STATE=unsupported
    if [ "$RESOLVE_RC" -eq 0 ] && [ "$STATE" = available ]; then
        CONTEXT_SNAPSHOT=$(mktemp /tmp/codescout-gate-context.XXXXXX) || exit 2
        printf '%s' "$RESOLUTION" | python3 -c 'import json,sys; json.dump(json.load(sys.stdin)["context"],sys.stdout)' > "$CONTEXT_SNAPSHOT" || exit 2
        export CODESCOUT_CONTEXT_FILE="$CONTEXT_SNAPSHOT"
    elif [ -n "$EXCLUSIVE" ] && [ "$STATE" = missing ]; then
        : # The formatter will report operator-declared exclusive authorization.
    else
        CONTEXT_BLOCKED=1
        echo "gate.sh: formatting context blocked ($STATE); remaining verification lanes will run." >&2
        printf '%s\n' "$RESOLUTION" >&2
    fi
    [ -z "$EXCLUSIVE" ] || FMT_ARGS=(--exclusive-checkout "$EXCLUSIVE")
fi

# The pool is outside the repo on purpose: nothing there needs gitignoring, a peer's
# `git clean` cannot reach it, and no tool that walks the worktree will scan it.
. "$(dirname "${BASH_SOURCE[0]}")/slot-pool.sh" || exit 2
# GATE_TIER is this script's own state: only lease_gate_target sets it. With a preset CARGO_TARGET_DIR
# that function never runs, and a GATE_TIER=ram brought in by the environment would turn on tier
# branches whose GATE_POOL, RAM_DEV and report function nothing has set.
GATE_TIER=
if [ -z "${CARGO_TARGET_DIR:-}" ]; then
    lease_gate_target gate.sh || exit 2
fi
mkdir -p "$CARGO_TARGET_DIR" || exit 2

echo "gate.sh: CARGO_TARGET_DIR=$CARGO_TARGET_DIR"
# The RAM tier is opt-in and machine-local; a disk run prints nothing about it. The guard is
# `GATE_TIER`, which only the real lease_gate_target sets: the Python light lane's fixture replaces
# slot-pool.sh and presets CARGO_TARGET_DIR, so no ram_* function exists there (tests/test_python_runner.py).
if [ "${GATE_TIER:-}" = ram ]; then
    echo "gate.sh: tier RAM (${RAM_DEV:-?}): leased from $GATE_POOL (CODESCOUT_GATE_RAM_POOL): a zram mount with MemAvailable above the floor and the pool budget covered"
fi
[ -n "${SLOT_FD:-}" ] && echo "gate.sh: leased for THIS run only. For a targeted run, lease your own: scripts/with-slot.sh cargo test --lib -- <filter>"
echo

# The ONNX path (`local-embed`) is opt-in and is in NO test lane here. The clippy lane below
# compiles it (`--features local-embed`), so it cannot rot unnoticed, but neither nextest lane
# builds it, so none of its tests run. That includes the two weight tests in
# `crates/codescout-embed/src/local.rs` (`from_dir_produces_a_stable_384d_vector`,
# `from_dir_matches_the_hub_path_for_the_same_model`): the only tests that catch a
# correctly-shaped but silently WRONG vector (wrong tokenizer or pooling). They demand real
# AllMiniLM-L6-v2 weights on disk via `CODESCOUT_TEST_ONNX_DIR` and PANIC rather than skip when
# it is unset, deliberately, so they cannot go quietly missing. A green gate here is not
# evidence about that property. Their lane is CI's `local-embed` matrix config, which seeds the
# weights and runs them with `--features local-embed --no-default-features`; read that job, not
# this script, before trusting a change to the local ONNX path.
#
# History: from 2026-09-17 to 2026-11-08 `local-embed` was in Cargo's `default`, so the default
# nextest lane compiled those two tests and this script exported CODESCOUT_SKIP_ONNX_TESTS=1 to
# keep the gate from redding on every machine without weights. They then PASSED while skipped:
# a green line asserting nothing. With `local-embed` out of `default` there is nothing here to
# opt out of, the export is gone, and the limit is printed at the end of the run instead.

# Test temp files on tmpfs. Measured 2026-11-05 (memory research/gate-tmpdir-tmpfs-20261105), nextest phase of
# the default lane: faster with TMPDIR on tmpfs in 3 of 3 pairs (4 s at low load, 27 s and 31 s at high load; n=3,
# load not controlled) and more stable, and the tests used at most 38 MB there. Only the two nextest commands
# get it: clippy, the doctest steps and the Python lane were not measured. It is used only when the base is a
# tmpfs with CODESCOUT_GATE_TMPFS_MIN_MB free (default 1024); CODESCOUT_GATE_TMPFS_BASE moves the base, and
# `off` or an empty value switches it off.
GATE_TMP=""
gate_tmp_base="${CODESCOUT_GATE_TMPFS_BASE-/tmp}"
gate_tmp_min="${CODESCOUT_GATE_TMPFS_MIN_MB:-1024}"
if [ -n "$gate_tmp_base" ] && [ "$gate_tmp_base" != off ] && [ -d "$gate_tmp_base" ] \
        && [[ "$gate_tmp_min" =~ ^[0-9]{1,12}$ ]] \
        && [ "$(findmnt -no FSTYPE -T "$gate_tmp_base" 2>/dev/null | head -n 1)" = tmpfs ] \
        && [ "$(df -Pm -- "$gate_tmp_base" 2>/dev/null | awk 'NR == 2 { print $4; exit }')" -ge "$gate_tmp_min" ] 2>/dev/null; then
    # A gate killed with SIGKILL cannot run its EXIT trap. Its directory is old by the time a later gate
    # starts, so that gate removes the gate's own stale ones, by name, owner and age. A fresh one may belong
    # to a gate another session is running now, and a directory with another name is not this script's.
    find "$gate_tmp_base" -maxdepth 1 -type d -name 'codescout-gate-tmp.*' -user "$(id -u)" -mmin +720 \
        -exec rm -rf -- {} + 2>/dev/null
    GATE_TMP=$(mktemp -d "$gate_tmp_base/codescout-gate-tmp.XXXXXX") || GATE_TMP=""
fi
GATE_TMP_ENV=()
if [ -n "$GATE_TMP" ]; then
    GATE_TMP_ENV=(env "TMPDIR=$GATE_TMP")
    echo "gate.sh: test temp files in $GATE_TMP (tmpfs), removed when the gate exits"
fi

# The light Python lane needs neither the leased target dir nor a built binary (tests/test_python_runner.py
# pins that by making the fixture lane wait for a Cargo lane), so it runs in the background beside the
# Rust lanes and is joined after the default lane. The default Cargo lane therefore still finishes last
# among the Cargo lanes, and a red Python lane still lets every Cargo lane run. Its output is held in a
# file and printed whole once it is joined, so it cannot interleave with a Cargo lane's.
GATE_T0=$SECONDS
# Wall seconds per lane, printed in the summary below: the gate's duration was recorded
# nowhere (bug e1515917e4215c93), and duration is the dimension that grows with the tests.
PY_DIR=$(mktemp -d "${TMPDIR:-/tmp}/codescout-gate-python.XXXXXX") || exit 2
(
    PY_START=$SECONDS
    python3 ./scripts/python-tests.py --lane light >"$PY_DIR/out" 2>&1
    echo "$? $((SECONDS - PY_START))" >"$PY_DIR/done"
) &
PY_PID=$!
T_PREV=$SECONDS

# `;` throughout, never `&&` — see the heredoc above. The default lane does two jobs,
# reporting AND rebuilding, and only the first should ever be short-circuited.
if [ "$CONTEXT_BLOCKED" -eq 1 ]; then
    FMT=2
    FMT_STATE=context-blocked
else
    ./scripts/fmt-mine.sh "${FMT_ARGS[@]}"
    FMT=$?
    if [ "$FMT" -eq 0 ]; then FMT_STATE=passed
    elif [ "$FMT" -eq 1 ] && [ "$CHECK_ONLY" -eq 1 ]; then FMT_STATE=differences
    elif [ "$FMT" -eq 1 ]; then FMT_STATE=ownership-refused
    else FMT_STATE=tool-error
    fi
fi
T_FMT=$((SECONDS - T_PREV)); T_PREV=$SECONDS
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
CLIPPY=$?
# The lean set (`--no-default-features`) is linted in the same lane. The librarian is off there, so an
# item that only `src/librarian/` uses is dead code, and `-D warnings` makes that an error. Until
# 2026-11-09 no clippy command anywhere ran this set, and it was red on `ReadDenyList`
# (docs/issues/archive/2026-11-09-no-lane-runs-clippy-on-the-lean-feature-set-and-it-is-red-on-readdenylist.md).
# The lane is red when EITHER command is, and `;` and `||` only, never `&&`: both always run.
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
CLIPPY_LEAN=$?
[ "$CLIPPY" -ne 0 ] || CLIPPY=$CLIPPY_LEAN
T_CLIPPY=$((SECONDS - T_PREV)); T_PREV=$SECONDS
${GATE_TMP_ENV[@]+"${GATE_TMP_ENV[@]}"} cargo nextest run --workspace --no-default-features --no-fail-fast
LEAN=$?
# nextest does not run doctests, so each test lane has a second command. A lane is red when EITHER is:
# the nextest exit wins when it is non-zero, else the doctest exit is the lane's. `;` and `||` only,
# never `&&` (see the heredoc above): the doctest step must run even when the nextest step is red.
cargo test --workspace --doc --no-default-features
LEAN_DOC=$?
[ "$LEAN" -ne 0 ] || LEAN=$LEAN_DOC
T_LEAN=$((SECONDS - T_PREV)); T_PREV=$SECONDS
${GATE_TMP_ENV[@]+"${GATE_TMP_ENV[@]}"} cargo nextest run --workspace --no-fail-fast
DEFAULT=$?
cargo test --workspace --doc
DEFAULT_DOC=$?
[ "$DEFAULT" -ne 0 ] || DEFAULT=$DEFAULT_DOC
T_DEFAULT=$((SECONDS - T_PREV))
# Join the Python lane: the verdict needs its exit code. A lane that left no result is red, not green.
wait "$PY_PID" 2>/dev/null
PY_PID=""
echo
echo "gate.sh: Python light lane output (it ran beside the Rust lanes):"
cat "$PY_DIR/out" 2>/dev/null
if ! read -r PYTHON_LIGHT T_PYTHON <"$PY_DIR/done" 2>/dev/null; then
    PYTHON_LIGHT=1; T_PYTHON=0
    echo "gate.sh: the Python light lane left no result; counting it red" >&2
fi
T_TOTAL=$((SECONDS - GATE_T0))

echo
echo "gate.sh: tree size $(du -sh "$CARGO_TARGET_DIR" 2>/dev/null | cut -f1) at $CARGO_TARGET_DIR"
if [ -n "${SLOT_FD:-}" ]; then
    # After a RAM lease the pool is the RAM pool; the disk text stays as it was.
    if [ "${GATE_TIER:-}" = ram ]; then POOL_LABEL="RAM pool total"; else POOL_LABEL="pool total"; fi
    echo "gate.sh: $POOL_LABEL $(du -sh "$GATE_POOL" 2>/dev/null | cut -f1) at $GATE_POOL"
fi
# The RAM figures are how the revisit trigger (a ratio worse than 1/2.2) becomes visible.
if [ "${GATE_TIER:-}" = ram ]; then ram_tier_report; fi
echo "GATE SECONDS -> TOTAL=$T_TOTAL PYTHON=$T_PYTHON FMT=$T_FMT CLIPPY=$T_CLIPPY LEAN=$T_LEAN DEFAULT=$T_DEFAULT"
echo "gate.sh: PYTHON ran beside the Rust lanes, so TOTAL is not the sum of the lane times"
echo "PYTHON LIGHT EXIT -> PYTHON_LIGHT=$PYTHON_LIGHT"
echo "GATE EXITS -> FMT=$FMT CLIPPY=$CLIPPY LEAN=$LEAN DEFAULT=$DEFAULT FMT_STATE=$FMT_STATE"
# A red lane in a RAM slot may be the tier, not the code. The gate does not read lane output, so this
# is printed on every red RAM run, with the figures a reader needs to judge it.
if [ "${GATE_TIER:-}" = ram ] && { [ "$PYTHON_LIGHT" -ne 0 ] || [ "$FMT" -ne 0 ] || [ "$CLIPPY" -ne 0 ] || [ "$LEAN" -ne 0 ] || [ "$DEFAULT" -ne 0 ]; }; then
    echo "gate.sh: this red run built in the RAM tier (${RAM_DEV:-?}; $(ram_tier_figures)): a 'No space left on device' or I/O error in a lane's output above means the RAM tier, not the code (CODESCOUT_GATE_RAM_POOL, CODESCOUT_GATE_RAM_GROWTH_MB, CODESCOUT_GATE_RAM_COLD_MB)"
fi

# What a green run here does NOT cover. Printed every time because this output is what a
# session reads when it decides "green". Pinned by gate_script_prints_what_green_does_not_cover
# (src/prompts/mod.rs), which reads these echo lines.
echo "gate.sh: green here does not cover:"
echo "  - the three heavy Python suites: install requirements-python-heavy.txt and run python3 scripts/python-tests.py --lane heavy (CI job python-heavy)"
echo "  - server-stack, which cargo rb ships and no lane compiles: read CI job test-server-stack (kept alive by every_declared_feature_has_a_lane_or_a_reason)"
echo "  - librarian code in LEAN: the librarian is off there, so only DEFAULT runs its tests"
echo "  - the local ONNX path (local-embed): clippy compiles it but no test lane here runs it, the two weight tests included: CI's local-embed job runs them"

if [ "$FMT" -ne 0 ]; then
    echo "gate.sh: formatting outcome $FMT_STATE (exit $FMT); read its diagnostics above. Other lanes still ran." >&2
fi

# Exit non-zero if ANY lane failed. The five codes above are the real report — this
# status exists so a caller that checks `$?` is not told everything passed, which is
# exactly what a trailing `echo` does to a `;`-chained sequence.
[ "$PYTHON_LIGHT" -eq 0 ] && [ "$FMT" -eq 0 ] && [ "$CLIPPY" -eq 0 ] && [ "$LEAN" -eq 0 ] && [ "$DEFAULT" -eq 0 ]
