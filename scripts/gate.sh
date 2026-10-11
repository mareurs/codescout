#!/usr/bin/env bash
# Run the light Python lane and four Rust commands in a `target/` LEASED for this run, so the
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
# CODESCOUT_GATE_POOL_KEEP or higher are removed (bug 503fa887ab0ec144). Why the lock is
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
if [ -n "$EXCLUSIVE" ]; then
    python3 -c 'import pathlib,sys; p=pathlib.Path(sys.argv[1]); raise SystemExit(0 if p.is_absolute() and p.resolve(strict=True)==pathlib.Path(sys.argv[2]) else 2)' "$EXCLUSIVE" "$ROOT" || {
        echo "gate.sh: exclusive declaration must match the actual Git root" >&2; exit 2;
    }
fi

CONTEXT_SNAPSHOT=""
trap 'rm -f -- "${CONTEXT_SNAPSHOT:-}"' EXIT
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
if [ -z "${CARGO_TARGET_DIR:-}" ]; then
    lease_gate_target gate.sh || exit 2
fi
mkdir -p "$CARGO_TARGET_DIR" || exit 2

echo "gate.sh: CARGO_TARGET_DIR=$CARGO_TARGET_DIR"
[ -n "${SLOT_FD:-}" ] && echo "gate.sh: leased for THIS run only. For a targeted run, lease your own: scripts/with-slot.sh cargo test --lib -- <filter>"
echo

# The two ONNX weight tests in `crates/codescout-embed/src/local.rs`
# (`from_dir_produces_a_stable_384d_vector`,
# `from_dir_matches_the_hub_path_for_the_same_model`) demand real AllMiniLM-L6-v2
# weights on disk via `CODESCOUT_TEST_ONNX_DIR`, and PANIC rather than skip when it
# is unset — deliberately, so they cannot go quietly missing.
#
# They only COMPILE when `local-embed` is on, which until 2026-09-17 no lane here
# had, so the gate never met them. `local-embed` then entered Cargo's `default`
# (the default model is `local:AllMiniLML6V2Q`, so a lean default shipped a binary
# that could not construct its own default config), and the default lane below
# started building them — redding the gate on every machine without the weights,
# which is every machine here.
#
# Opting out matches what CI already does for every non-`local-embed` lane
# (.github/workflows/ci.yml, "Opt out of ONNX tests"). It is set for the WHOLE
# script rather than per-lane because the lean lane does not compile those tests
# and the clippy lane does not run any, so the narrower form would read as a
# distinction that does not exist.
#
# WHAT THIS GIVES UP, stated because it is real: those two are the only tests that
# catch a correctly-shaped but silently WRONG vector (wrong tokenizer or pooling).
# A green gate here is not evidence about that property. Their lane is CI's
# `local-embed` matrix config, which seeds the weights and runs them; read that job,
# not this script, before trusting a change to the local ONNX path. To run them here,
# set CODESCOUT_TEST_ONNX_DIR to a directory holding `onnx/model_quantized.onnx` plus
# the tokenizer files and unset this.
export CODESCOUT_SKIP_ONNX_TESTS="${CODESCOUT_SKIP_ONNX_TESTS:-1}"

# Run Python first so the default Cargo lane still finishes last, even if Python fails.
python3 ./scripts/python-tests.py --lane light
PYTHON_LIGHT=$?

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
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
CLIPPY=$?
cargo test --workspace --no-default-features
LEAN=$?
cargo test --workspace
DEFAULT=$?

echo
echo "gate.sh: tree size $(du -sh "$CARGO_TARGET_DIR" 2>/dev/null | cut -f1) at $CARGO_TARGET_DIR"
[ -n "${SLOT_FD:-}" ] && echo "gate.sh: pool total $(du -sh "$GATE_POOL" 2>/dev/null | cut -f1) at $GATE_POOL"
echo "PYTHON LIGHT EXIT -> PYTHON_LIGHT=$PYTHON_LIGHT"
echo "GATE EXITS -> FMT=$FMT CLIPPY=$CLIPPY LEAN=$LEAN DEFAULT=$DEFAULT FMT_STATE=$FMT_STATE"

# What a green run here does NOT cover. Printed every time because this output is what a
# session reads when it decides "green". Pinned by gate_script_prints_what_green_does_not_cover
# (src/prompts/mod.rs), which reads these echo lines.
echo "gate.sh: green here does not cover:"
echo "  - the three heavy Python suites: install requirements-python-heavy.txt and run python3 scripts/python-tests.py --lane heavy (CI job python-heavy)"
echo "  - server-stack, which cargo rb ships and no lane compiles: read CI job test-server-stack (kept alive by every_declared_feature_has_a_lane_or_a_reason)"
echo "  - librarian code in LEAN: the librarian is off there, so only DEFAULT runs its tests"
if [ "$CODESCOUT_SKIP_ONNX_TESTS" = 1 ]; then
    echo "  - the two local ONNX weight tests, which print 'ok' while skipped: CI's local-embed job runs them"
fi

if [ "$FMT" -ne 0 ]; then
    echo "gate.sh: formatting outcome $FMT_STATE (exit $FMT); read its diagnostics above. Other lanes still ran." >&2
fi

# Exit non-zero if ANY lane failed. The five codes above are the real report — this
# status exists so a caller that checks `$?` is not told everything passed, which is
# exactly what a trailing `echo` does to a `;`-chained sequence.
[ "$PYTHON_LIGHT" -eq 0 ] && [ "$FMT" -eq 0 ] && [ "$CLIPPY" -eq 0 ] && [ "$LEAN" -eq 0 ] && [ "$DEFAULT" -eq 0 ]
