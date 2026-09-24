#!/usr/bin/env bash
# Run ONE command inside a leased build tree from the gate's pool:
#
#   scripts/with-slot.sh cargo test --lib -- tools::symbol::
#
# WHY THIS EXISTS. A targeted test run wants a warm `target/`, and the only warm tree a
# session knows about is the one gate.sh printed. So sessions typed
# `CARGO_TARGET_DIR=<that path> cargo test …` by hand. After the pool replaced per-session
# trees, one session kept doing it and grew a 4.5G tree the pool cannot reclaim, because
# no lock proves it idle (bug 294ba0ae7ed8c7b1). With today's gate the printed path is a
# SLOT, and the same habit writes into a tree another session may be leasing. That is
# the concurrent-writer window the lease exists to close (scripts/gate.sh's header).
#
# This takes the lease exactly as gate.sh does (scripts/slot-pool.sh), then `exec`s the
# command. The lock fd survives the exec, so the lease lasts exactly as long as the
# command does, and the command's exit status is this script's.
#
# A preset CARGO_TARGET_DIR is honoured untouched, as in gate.sh: whoever set it chose.

set -u

if [ $# -eq 0 ]; then
    cat >&2 <<'EOF'
usage: scripts/with-slot.sh <command> [args...]
  Runs the command with CARGO_TARGET_DIR set to a build tree leased from the gate's pool
  (~/.cache/codescout-gate). The lease ends when the command exits.
EOF
    exit 2
fi

[ -n "${CARGO_TARGET_DIR:-}" ] && exec "$@"

. "$(dirname "${BASH_SOURCE[0]}")/slot-pool.sh" || exit 2
lease_gate_target with-slot.sh || exit 2
echo "with-slot.sh: CARGO_TARGET_DIR=$CARGO_TARGET_DIR (leased until this command exits)" >&2
exec "$@"
