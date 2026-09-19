#!/usr/bin/env bash
# git-safe-reset.sh — `git reset` that defaults to --soft, and refuses to run
# --mixed/--hard on a checkout other sessions share.
#
# WHY THIS EXISTS
# docs/issues/2026-09-15-git-reset-mixed-silently-unstages-every-peer-on-a-shared-checkout.md
#
#   `.git/index` is ONE file per checkout, not per session. `git reset --mixed`
#   (git's own default when no mode flag is given) rewrites that index to match
#   the target commit, discarding whatever ANY OTHER session had staged at that
#   instant — with no error, no output, and nothing for the victim to attribute
#   it to: the index carries no owner, no history and no reflog. `--hard` does
#   the same to the index AND throws away every peer's unstaged working-tree
#   edit on top. Measured 2026-09-15 in a throwaway repo with a peer's `git add`
#   in flight across the reset: `--mixed` silently dropped it; `--soft` left it
#   intact. `--soft` is not merely the safer flag here — it is also what a
#   repairing session usually wants anyway, since it leaves the change being
#   rescued staged and ready to re-commit, so it saves a step rather than
#   costing one.
#
# WHY A WRAPPER AND NOT A BETTER HABIT
# Same argument as `scripts/gate.sh` and `scripts/rb.sh`: CLAUDE.md § Observer
# Blindness position 3 asks that the correct path END IN A SAFE STATE, so
# compliance leaves nothing armed. "Remember to pass --soft" is the policy
# that had already failed once — the bug above was filed by the session that
# broke this rule while repairing something else, having just read the
# convention page that already forbade `reset` on a shared tree.
#
# WHAT THIS DOES NOT AND CANNOT DO, stated because a green run here is not
# evidence about the whole checkout:
#
#   * It cannot see who is live AT THE INSTANT of the reset, only who has left
#     a transcript trace of writing a path that is staged RIGHT NOW. The gap
#     between that check and the `git reset --mixed`/`--hard` it would gate is
#     exactly where a peer's `git add` can land unseen — which is why there is
#     no way to make --mixed/--hard SAFE here, only a way to refuse it.
#   * `scripts/file-provenance.py`'s own limits carry over unchanged: a Bash
#     write its heuristics miss is invisible, and that is reported as "cannot
#     tell", never as "nobody's".
#   * It is a mechanism for whoever reaches for it and a policy for everyone
#     else — nothing stops a session from typing bare `git reset --mixed`
#     directly and skipping this file entirely. It closes the gap only for the
#     session that runs THIS instead of the raw command.
#
# Usage:
#   scripts/git-safe-reset.sh [<commit>]           same as --soft <commit>
#   scripts/git-safe-reset.sh --soft [<commit>]    moves HEAD, keeps the index
#   scripts/git-safe-reset.sh --mixed [<commit>]   REFUSED on a shared checkout
#   scripts/git-safe-reset.sh --hard [<commit>]    REFUSED on a shared checkout
#
# Exit: 0 the --soft reset ran.
#       1 refused — --mixed/--hard would touch the shared index (and, for
#         --hard, every peer's unstaged working-tree edits too).
#       2 could not answer the question at all: no session id, or not a git
#         repository.

set -u

if [ -z "${CLAUDE_CODE_SESSION_ID:-}" ]; then
    cat >&2 <<'EOF'
git-safe-reset.sh: CLAUDE_CODE_SESSION_ID is unset, so there is no session identity
this script could use to attribute a peer's staged work if it had to refuse.

Outside a Claude session you are almost certainly the only writer to this checkout,
which is the case bare `git reset` is already correct for. Run it directly:

  git reset --soft <commit>      # move HEAD, keep the index -- usually what you want
  git reset --mixed <commit>     # also unstages everything (git's own default)
  git reset --hard <commit>      # also discards working-tree changes

EOF
    exit 2
fi

ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
    echo "git-safe-reset.sh: not inside a git repository" >&2
    exit 2
}
cd "$ROOT" || exit 2

# Overridable so the test suite can stub attribution without a live transcript
# corpus -- same escape `fmt-mine.sh` gives itself via FMT_MINE_PROVENANCE, and
# for the same reason: a suite that can only run against this machine's real
# profiles is a suite that runs nowhere else.
PROVENANCE="${GIT_SAFE_RESET_PROVENANCE:-$ROOT/scripts/file-provenance.py}"

MODE="--soft"
ARGS=()
for a in "$@"; do
    case "$a" in
    --soft | --mixed | --hard) MODE="$a" ;;
    *) ARGS+=("$a") ;;
    esac
done

if [ "$MODE" = "--soft" ]; then
    exec git reset --soft "${ARGS[@]}"
fi

# --mixed or --hard from here on. Refused unconditionally -- see the header for
# why there is no safe way to compute a window-free answer instead.
STAGED=$(git diff --cached --name-only)
COUNT=0
[ -n "$STAGED" ] && COUNT=$(printf '%s\n' "$STAGED" | grep -c .)

echo "git-safe-reset.sh: REFUSED -- $MODE rewrites .git/index, which is ONE file shared" >&2
echo "  by every session in this checkout, not one per session." >&2
echo >&2

if [ "$COUNT" -gt 0 ]; then
    echo "  Right now $COUNT file(s) are staged and would be unstaged by $MODE:" >&2
    printf '    %s\n' $STAGED >&2
    echo >&2
    echo "  Whose, so far as this checkout's transcripts can say:" >&2
    set +e
    # shellcheck disable=SC2086
    "$PROVENANCE" $STAGED 2>&1 | sed 's/^/    /' >&2
    set -e
else
    echo "  Nothing is staged right now, by this check -- but that check and the reset" >&2
    echo "  it would gate are two separate commands, and the index is shared: a peer's" >&2
    echo "  \`git add\` landing between them is exactly what this script exists to catch," >&2
    echo "  and a run that already reached this line would not see it happen." >&2
fi

if [ "$MODE" = "--hard" ]; then
    echo >&2
    echo "  --hard also discards every UNCOMMITTED working-tree edit under this root," >&2
    echo "  yours and every peer's, with no copy left behind for an untracked file." >&2
fi

cat >&2 <<EOF

  There is no --force here, on the same reasoning as scripts/fmt-mine.sh: a flag that
  wipes a peer's staged work on your say-so re-admits the exact defect this script
  exists to prevent, under a spelling that reads as deliberate. If you have checked
  who else is live (scripts/peer-sessions.sh) and decided $MODE is safe anyway, run
  git itself:

    git reset $MODE ${ARGS[*]:-<commit>}

  That is the same act, minus the false assurance that this guard sanctioned it.

  What you likely want instead is --soft, which this script runs by default with no
  mode flag: it moves HEAD and leaves the index untouched, so anything staged --
  yours or a peer's -- survives and stays ready to re-commit.
EOF

exit 1
