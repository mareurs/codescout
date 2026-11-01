#!/usr/bin/env bash
# resident-scratch.sh — create and print the OUT-OF-TREE scratch directory for one experiment.
#
#   scripts/resident-scratch.sh <name>   create <root>/<repo>/<name>, print its absolute path
#   scripts/resident-scratch.sh          create and print <root>/<repo>
#
# WHY IT EXISTS. Session scratch (run logs, patches, copies of doc trees) used to live in
# `.codescout/resident-private/` inside the repository. rust-analyzer's file watcher walks every
# directory below the workspace root, follows symlinks, and has no exclusion list, so scratch that
# holds links — some of them cycles — kept `documentSymbol` answering null (bug
# 35bcf086deeb7cbe). Scratch outside the workspace root is never walked. This script is the
# easy path to such a place: a convention that asks for care is the shape that failed.
# Rules and boundaries: docs/conventions/resident-scratch-location.md.
#
# <root> is $CODESCOUT_RESIDENT_ROOT, else ${XDG_CACHE_HOME:-$HOME/.cache}/codescout-resident.
# It is NOT under the gate pools (`codescout-gate`, `codescout-gate-ram`): `slot_tend` reclaims
# `<pool>/slot-*`, so scratch under a pool root could be deleted.
# <repo> is the basename of the MAIN checkout (the parent of the git common dir), so a linked
# worktree and the main checkout share one scratch root.
#
# Writes go through the shell. The codescout file tools refuse paths outside the project root
# unless the operator approves them or adds an `extra_write_roots` entry to .codescout/project.toml.
set -euo pipefail

fail() {
    echo "resident-scratch: $*" >&2
    exit 2
}

name=""
if [ "$#" -gt 1 ]; then
    fail "usage: resident-scratch.sh [<name>]"
fi
if [ "$#" -eq 1 ]; then
    name=$1
    # One path component: no slash, no leading dot, so `..` and hidden names cannot escape or hide.
    if ! [[ $name =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]]; then
        fail "a name is letters, digits, '.', '_' or '-', and starts with a letter or digit: '$name'"
    fi
fi

toplevel=$(git rev-parse --show-toplevel 2>/dev/null) || fail "not inside a git checkout"
common=$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null) ||
    fail "git cannot name the common directory (git 2.31 or newer is needed)"
case "$common" in
*/.git) main=${common%/.git} ;;
*) main=$common ;;
esac
repo=$(basename "$main")

root=${CODESCOUT_RESIDENT_ROOT:-${XDG_CACHE_HOME:-${HOME:?HOME is not set}/.cache}/codescout-resident}
resolved_root=$(realpath -m "$root")

# The whole point is to be outside every checkout. A root inside one defeats it silently.
for checkout in "$main" "$toplevel"; do
    resolved_checkout=$(realpath -m "$checkout")
    case "$resolved_root/" in
    "$resolved_checkout"/*) fail "the scratch root '$resolved_root' is inside the checkout '$resolved_checkout'" ;;
    esac
done

dir=$resolved_root/$repo
if [ -n "$name" ]; then
    dir=$dir/$name
fi
umask 077
mkdir -p "$dir"

# Standing nudge, at the one place every new experiment passes through.
if [ -z "${RESIDENT_SCRATCH_QUIET:-}" ] && [ -d "$main/.codescout/resident-private" ]; then
    echo "resident-scratch: note: $main/.codescout/resident-private still exists inside the repository." >&2
    echo "resident-scratch: it slows rust-analyzer; see scripts/resident-scratch-migrate.sh" >&2
fi

printf '%s\n' "$dir"
