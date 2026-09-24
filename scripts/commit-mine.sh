#!/usr/bin/env bash
# scripts/commit-mine.sh — commit exactly the paths YOU staged, on a shared index.
#
#   usage:  scripts/commit-mine.sh -m "message" [other git-commit message options]
#
# WHY IT EXISTS
#
# On this checkout every session shares one index, and there was a state with no compliant
# commit at all: your staged change is COUPLED to a file a peer is also editing (a bug file
# and the `**Members:**` line naming it). A bare commit is refused because the index holds
# the peer's paths; a pathspec commit is refused because it takes the whole WORKING-TREE file,
# peer's unstaged hunk included; committing the bug file alone is refused because the class
# gained a member its ledger line does not name. Each guard is right, and their intersection
# was empty (docs/issues/2026-09-01-two-correct-pre-commit-guards-have-an-empty-intersection.md).
#
# HOW
#
#   1. Ask the foreign-index guard which staged paths are yours (`--classify`). This script
#      does not look owners up itself: a second copy of that lookup would be a second site of
#      one law, and this repo already paid for one such pair drifting.
#   2. Refuse a staged rename whose two halves have different owners. Committing only your
#      half would turn their move into a copy, or into a deletion.
#   3. Build a PRIVATE index: HEAD, plus the shared index's entry for each path that is yours.
#   4. `git commit` from it. Every commit hook reads GIT_INDEX_FILE, so each one judges exactly
#      this set; the stage-log recorder ignores any index but $GIT_DIR/index, so the durable
#      ownership log is untouched.
#
# The SHARED index needs no repair afterwards. An entry you own holds the blob you just
# committed, so it simply stops differing from HEAD — and a peer's next bare commit carries
# only their own paths. tests/commit-mine.sh F2 is that claim, as an assertion.
#
# WHAT IT DOES NOT SOLVE: one index entry holding two authors' hunks, i.e. a peer re-staged a
# file on top of yours. Ownership is recorded per (blob, path), so that entry is theirs and is
# left out, and any refusal that follows is correct rather than a false alarm. Splitting
# inside one entry would need hunk-level ownership, which nothing records.
#
# Exit: 0 committed | 1 nothing of yours, a split rename, or the commit was refused
#       2 usage (no session id, or an argument that re-selects paths) | 3 cannot classify
set -uo pipefail

me="${CLAUDE_CODE_SESSION_ID:-}"
if [ -z "$me" ]; then
    echo "commit-mine: no CLAUDE_CODE_SESSION_ID. Outside a Claude session there is no" >&2
    echo "  per-session ownership to split by — use plain \`git commit\`." >&2
    exit 2
fi

# Anything that makes git choose paths itself, or skip the hooks, defeats the one thing
# this does. A message that is literally one of these words is refused too: loud and
# harmless, where accepting a real one would be silent.
for arg in "$@"; do
    case "$arg" in
        --|-a|--all|-i|--include|-o|--only|-p|--patch|--interactive|--amend|-n|--no-verify)
            echo "commit-mine: \`$arg\` is refused. It re-selects paths from the working tree," >&2
            echo "  rewrites a commit that may be a peer's, or skips the hooks — each of which" >&2
            echo "  is what this helper exists to avoid. Stage what you mean with \`git add\`," >&2
            echo "  then pass only message options (-m, -F, -C, --signoff, ...)." >&2
            exit 2 ;;
    esac
done

root="$(git rev-parse --show-toplevel 2>/dev/null)" || { echo "commit-mine: not in a git repo" >&2; exit 2; }
guard="$root/scripts/pre-commit-foreign-index.sh"

# The SHARED index, whatever the caller exported: the classification is about it.
classified="$(env -u GIT_INDEX_FILE bash "$guard" --classify)"
rc=$?
if [ "$rc" -ne 0 ]; then
    echo "commit-mine: could not tell whose staged paths are whose (guard exit $rc)." >&2
    echo "  If the index holds only your own work, a plain \`git commit\` is judged by the" >&2
    echo "  same guard; this helper will not guess ownership it cannot read." >&2
    exit "$rc"
fi

mine=()
declare -A is_mine=() owner_of=()
while IFS=$'\t' read -r kind owner path; do
    [ -n "$path" ] || continue
    case "$kind" in
        mine) mine+=("$path"); is_mine["$path"]=1 ;;
        theirs) owner_of["$path"]="$owner" ;;
    esac
done <<< "$classified"

if ((${#owner_of[@]})); then
    echo "commit-mine: leaving staged — not yours:" >&2
    for p in "${!owner_of[@]}"; do
        printf '    %s    (staged by %s)\n' "$p" "${owner_of[$p]}" >&2
    done
fi

if ! ((${#mine[@]})); then
    echo "commit-mine: nothing staged is yours — nothing to commit." >&2
    exit 1
fi

# A rename is ONE change stored as two index paths. `--classify` reads with --no-renames, so
# it sees the halves separately; pair them here, and refuse if they are split across owners.
split=0
while IFS=$'\t' read -r status src dst; do
    case "$status" in R*) ;; *) continue ;; esac
    if { [ -n "${is_mine[$src]:-}" ] && [ -n "${owner_of[$dst]:-}" ]; } ||
       { [ -n "${is_mine[$dst]:-}" ] && [ -n "${owner_of[$src]:-}" ]; }; then
        printf 'commit-mine: a staged rename spans two owners: %s -> %s\n' "$src" "$dst" >&2
        split=1
    fi
done < <(env -u GIT_INDEX_FILE git diff --cached --name-status -M 2>/dev/null)
if ((split)); then
    echo "  Committing only your half would turn the move into a copy or a deletion. Ask the" >&2
    echo "  other owner (/codescout-companion:reaching-peer-sessions), or commit it as the" >&2
    echo "  joint archive it is — the foreign-index refusal names the ack for that." >&2
    exit 1
fi

git_dir_abs="$(cd "$(git rev-parse --git-dir)" && pwd -P)" || exit 1
priv="$git_dir_abs/commit-mine-index.$$"
trap 'rm -f "$priv" "$priv.lock"' EXIT

GIT_INDEX_FILE="$priv" git read-tree HEAD || exit 1
for p in "${mine[@]}"; do
    # --literal-pathspecs: a path containing `*` or `?` must name itself, not a glob.
    entry="$(env -u GIT_INDEX_FILE git --literal-pathspecs ls-files -s -- "$p")"
    if [ -z "$entry" ]; then
        GIT_INDEX_FILE="$priv" git update-index --force-remove -- "$p" || exit 1
        continue
    fi
    read -r mode blob stage _ <<< "$entry"
    if [ "$stage" != "0" ]; then
        printf 'commit-mine: %s is unmerged (stage %s); resolve it first\n' "$p" "$stage" >&2
        exit 1
    fi
    GIT_INDEX_FILE="$priv" git update-index --add --cacheinfo "$mode,$blob,$p" || exit 1
done

GIT_INDEX_FILE="$priv" git commit "$@"
rc=$?
if [ "$rc" -eq 0 ]; then
    printf 'commit-mine: committed %s path(s) of yours; left %s staged for their owners.\n' \
        "${#mine[@]}" "${#owner_of[@]}" >&2
fi
exit "$rc"
