#!/usr/bin/env bash
# Run the four-command gate in a `target/` LEASED for this run from a small pool, so the
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
# RUN, so a slot is held for one run and reused by the next, whichever session starts it.
# The pool therefore grows with peak concurrent gate runs, not with sessions ever started:
# keying on the session id left 323G in 17 trees on 2026-09-24 (bug 37b251b33adb37eb).
# The lock sits on an fd every child inherits, deliberately WITHOUT `flock -o`: with `-o`,
# SIGKILLing this script frees the slot while its cargo is still writing into it, which
# reopens the race; without it, a daemon started mid-run (sccache, measured) pins one slot,
# which costs disk and never correctness (bug-fix-session-log:F-173). This script prints
# the slot's size and the pool's total when it finishes.
#
# NOT MANDATORY, and that is a real limitation rather than modesty: a session that types
# the four commands directly still shares `target/`, so this is a mechanism for whoever
# runs it and a policy for everyone else. The gate sentence in CLAUDE.md remains the
# canonical statement of WHAT runs, in what order, and is pinned byte-for-byte by
# `claude_md_gate_lists_its_four_commands_in_the_load_bearing_order`.

set -u

if [ -z "${CLAUDE_CODE_SESSION_ID:-}" ]; then
    cat >&2 <<'EOF'
gate.sh: CLAUDE_CODE_SESSION_ID is unset. Step 1, `fmt-mine.sh`, needs it to tell your
files from a peer's, and outside a Claude session you are almost certainly the only writer
to this checkout, which is the case the shared `target/` is already correct for. Run the
four commands directly:

  ./scripts/fmt-mine.sh ; \
  cargo clippy --workspace --all-targets --features local-embed -- -D warnings ; \
  cargo test --workspace --no-default-features ; \
  cargo test --workspace

Chain them with `;` and read the four exit codes. Never `&&`: `cargo test` builds THEN
runs, so a failing lean lane has already overwritten target/debug/codescout by the time
anything can fail, and `&&` would then skip the default lane that repairs it.
EOF
    exit 2
fi

# Outside the repo on purpose: nothing here needs gitignoring, a peer's `git clean`
# cannot reach it, and no tool that walks the worktree will scan it.
POOL="${CODESCOUT_GATE_POOL:-$HOME/.cache/codescout-gate}"
if [ -z "${CARGO_TARGET_DIR:-}" ]; then
    mkdir -p "$POOL" || exit 2
    SLOT=0
    while :; do
        exec {SLOT_FD}>"$POOL/slot-$SLOT.lock" || exit 2
        flock -n "$SLOT_FD"; rc=$?
        [ "$rc" -eq 0 ] && break
        exec {SLOT_FD}>&-
        # Exit 1 means held; anything else (flock missing, say) would loop forever.
        [ "$rc" -eq 1 ] || { echo "gate.sh: flock failed with exit $rc" >&2; exit 2; }
        SLOT=$((SLOT + 1))
    done
    export CARGO_TARGET_DIR="$POOL/slot-$SLOT"
fi
mkdir -p "$CARGO_TARGET_DIR" || exit 2

echo "gate.sh: CARGO_TARGET_DIR=$CARGO_TARGET_DIR"
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

# `;` throughout, never `&&` — see the heredoc above. The default lane does two jobs,
# reporting AND rebuilding, and only the first should ever be short-circuited.
./scripts/fmt-mine.sh
FMT=$?
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
CLIPPY=$?
cargo test --workspace --no-default-features
LEAN=$?
cargo test --workspace
DEFAULT=$?

echo
echo "gate.sh: tree size $(du -sh "$CARGO_TARGET_DIR" 2>/dev/null | cut -f1) at $CARGO_TARGET_DIR"
[ -n "${SLOT_FD:-}" ] && echo "gate.sh: pool total $(du -sh "$POOL" 2>/dev/null | cut -f1) at $POOL"
echo "GATE EXITS -> FMT=$FMT CLIPPY=$CLIPPY LEAN=$LEAN DEFAULT=$DEFAULT"

# FMT is the one ambiguous code, and the ambiguity is routine rather than rare on a shared
# checkout — so it is named here rather than left for the reader to infer from a bare 1.
if [ "$FMT" -ne 0 ]; then
    cat >&2 <<'EOF'

gate.sh: FMT is non-zero, which on a shared checkout is AMBIGUOUS and is usually not your
failure. `fmt-mine.sh` exits non-zero in two unrelated cases:

  * your own Rust needed formatting — act on it; or
  * it REFUSED a peer's unformatted file, which is the guard working. There is no
    --force, and `cargo fmt` would rewrite their uncommitted work.

Read its output above: it names the owner's sessionId and the socket to ask them on. The
other three codes are unaffected either way.
EOF
fi

# Exit non-zero if ANY lane failed. The four codes above are the real report — this
# status exists so a caller that checks `$?` is not told everything passed, which is
# exactly what a trailing `echo` does to a `;`-chained sequence.
[ "$FMT" -eq 0 ] && [ "$CLIPPY" -eq 0 ] && [ "$LEAN" -eq 0 ] && [ "$DEFAULT" -eq 0 ]
