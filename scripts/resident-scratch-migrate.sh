#!/usr/bin/env bash
# resident-scratch-migrate.sh — move closed experiment directories out of the repository tree.
#
#   scripts/resident-scratch-migrate.sh            DRY RUN: print what would move and what would not
#   scripts/resident-scratch-migrate.sh --apply    do it
#
#   --from <dir>          the in-tree scratch directory (default <main checkout>/.codescout/resident-private)
#   --idle-minutes <N>    a directory with a file changed in the last N minutes is live (default 60)
#   --only <name>         move only this directory; repeatable
#   --force-referenced    also move directories a memory names by path (see below)
#   --retarget            also rewrite absolute links between moved directories (see below)
#
# WHY. rust-analyzer's file watcher walks every directory under the workspace root and follows
# symlinks (bug 35bcf086deeb7cbe). Scratch outside the root is never walked. The destination is
# the one scripts/resident-scratch.sh prints. Rules: docs/conventions/resident-scratch-location.md.
#
# WHAT IT LEAVES ALONE, and says so:
#   * a directory with a file changed within --idle-minutes: an experiment may be writing to it.
#   * a directory named by path in `.codescout/memories/**` (`resident-private/<name>`). Those
#     memories are instructions that sessions follow, and some name sealed directories. Moving the
#     directory would break the path they give. Edit the memory first, then use --force-referenced.
#   * a destination that already exists. Nothing is overwritten.
#
# WHAT IT CHANGES, for each directory it moves:
#   * `mv` into the destination. A pointer FILE `<from>/<name>.moved` holds the new path. It is a
#     file, not a link: notify follows links, so a link at the old path would bring the walk back.
#   * with --retarget ONLY: every symlink inside the moved directory whose absolute target is inside
#     the old scratch tree and names a directory that moved is pointed at the new location. This is
#     opt-in because a seal can cover link targets: `closure.py` in the sealed experiment
#     directories records `os.readlink`, so rewriting a link changes the sealed bytes. Without the
#     flag those links keep their old absolute target and dangle.
#
# WHAT IT CANNOT FIX: a link in a directory that STAYS, pointing into one that moved, now dangles.
# A dangling link is not a directory link, so the watcher skips it, but whatever read through it
# no longer can. The script counts these and prints them.
# Link targets containing a tab or newline are not handled.
set -euo pipefail

fail() {
    echo "resident-scratch-migrate: $*" >&2
    exit 2
}

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
apply=0
retarget=0
idle=60
from=""
force_referenced=0
only=()
while [ "$#" -gt 0 ]; do
    case $1 in
    --apply) apply=1 ;;
    --retarget) retarget=1 ;;
    --force-referenced) force_referenced=1 ;;
    --from) from=${2:?--from needs a directory}; shift ;;
    --idle-minutes) idle=${2:?--idle-minutes needs a number}; shift ;;
    --only) only+=("${2:?--only needs a name}"); shift ;;
    -h | --help) sed -n '2,45p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) fail "unknown argument '$1'" ;;
    esac
    shift
done
[[ $idle =~ ^[0-9]+$ ]] || fail "--idle-minutes is a whole number: '$idle'"

common=$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null) || fail "not inside a git checkout"
case "$common" in
*/.git) main=${common%/.git} ;;
*) main=$common ;;
esac
[ -n "$from" ] || from=$main/.codescout/resident-private
[ -d "$from" ] || { echo "nothing to migrate: $from does not exist"; exit 0; }
from=$(realpath "$from")

dest_root=$(RESIDENT_SCRATCH_QUIET=1 bash "$here/resident-scratch.sh") || fail "no destination"
memories=$main/.codescout/memories

declare -A moving=()
declare -A why=()
for d in "$from"/*/; do
    [ -d "$d" ] && [ ! -L "${d%/}" ] || continue
    name=$(basename "$d")
    if [ "${#only[@]}" -gt 0 ]; then
        wanted=0
        for o in "${only[@]}"; do [ "$o" = "$name" ] && wanted=1; done
        [ "$wanted" = 1 ] || continue
    fi
    if [ -n "$(find "$d" -mmin "-$idle" -print -quit 2>/dev/null)" ]; then
        why[$name]="live: a file changed in the last $idle minutes"
    elif [ -e "$dest_root/$name" ]; then
        why[$name]="exists: $dest_root/$name"
    elif [ "$force_referenced" = 0 ] && [ -d "$memories" ] &&
        grep -rqF -- "resident-private/$name" "$memories" 2>/dev/null; then
        why[$name]="referenced: a memory names resident-private/$name"
    else
        moving[$name]=1
    fi
done

# Links inside a directory that moves, naming another directory that moves: they keep their old
# absolute target unless --retarget is given. Links in directories that stay, pointing into one
# that moves, will dangle.
inside=0
dangling=0
while IFS=$'\t' read -r link target; do
    [ -n "$link" ] || continue
    rel=${link#"$from"/}
    top=${rel%%/*}
    if [ -n "${moving[$top]:-}" ]; then
        case "$target" in
        "$from"/*)
            first=${target#"$from"/}
            first=${first%%/*}
            [ -z "${moving[$first]:-}" ] || inside=$((inside + 1))
            ;;
        esac
        continue
    fi
    case "$target" in
    "$from"/*)
        rest=${target#"$from"/}
        first=${rest%%/*}
        if [ -n "${moving[$first]:-}" ]; then
            dangling=$((dangling + 1))
            [ "$dangling" -gt 3 ] || echo "  would dangle: $link -> $target"
        fi
        ;;
    esac
done < <(find "$from" -type l -printf '%p\t%l\n' 2>/dev/null)

echo "scratch tree: $from"
echo "destination:  $dest_root"
echo "to move:      ${#moving[@]}"
for name in "${!moving[@]}"; do echo "  move  $name"; done | sort
echo "to leave:     ${#why[@]}"
for name in "${!why[@]}"; do echo "  stay  $name — ${why[$name]}"; done | sort
echo "links left dangling in directories that stay: $dangling (the first 3 are listed above)"
if [ "$retarget" = 1 ]; then
    echo "links inside moved directories that name another moved directory: $inside (will be rewritten)"
else
    echo "links inside moved directories that name another moved directory: $inside (left as they are; --retarget rewrites them, which changes the bytes of a sealed directory)"
fi

if [ "$apply" = 0 ]; then
    echo "dry run: nothing was changed. Pass --apply to move."
    exit 0
fi

retargeted=0
for name in "${!moving[@]}"; do
    mv -- "$from/$name" "$dest_root/$name"
    printf '%s\n' "$dest_root/$name" >"$from/$name.moved"
done
for name in "${!moving[@]}"; do
    [ "$retarget" = 1 ] || break
    while IFS=$'\t' read -r link target; do
        [ -n "$link" ] || continue
        case "$target" in
        "$from"/*)
            rest=${target#"$from"/}
            first=${rest%%/*}
            if [ -n "${moving[$first]:-}" ]; then
                ln -sfn -- "$dest_root/$rest" "$link"
                retargeted=$((retargeted + 1))
            fi
            ;;
        esac
    done < <(find "$dest_root/$name" -type l -printf '%p\t%l\n')
done
echo "moved ${#moving[@]} directories; retargeted $retargeted links"
