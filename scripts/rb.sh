#!/usr/bin/env bash
# Run `cargo rb` only when this checkout actually contains what origin has — and refuse
# to guess when it cannot find out.
#
# WHY THIS EXISTS
# `docs/trackers/embedder-stack-ops-session-log.md` § F-6.
#
#   Measured 2026-09-15. PR #20 merged to `origin/experiments` at 13:22; the operator ran
#   `cargo rb` at 14:08 and `/mcp` to pick up the fix. THE BINARY DID NOT CONTAIN IT —
#   local HEAD was 5 commits behind origin, so cargo compiled a tree three minutes short
#   of the reconciling push and exited 0.
#
#   EVERY AVAILABLE SIGNAL SAID THE REBUILD WAS CURRENT. `cargo rb` exited 0. The binary's
#   mtime updated. `~/.cargo/bin/codescout` resolved through the symlink. `--version`
#   returned 0.15.0 — unchanged by the merge, so it discriminated nothing. `doc --help`
#   succeeded, which proves the librarian is compiled in and not which schema it carries.
#   Nothing anywhere reported "you built a tree that does not contain what you merged."
#
#   Cost: one wasted rebuild plus a reconnect, and a window in which the operator believed
#   a WAL-backup fix was live when it was not.
#
# WHY A WRAPPER AND NOT A BETTER HABIT
# `cargo rb` is an alias in `.cargo/config.toml`, and a cargo alias cannot run a
# precondition — there is no hook to hang this on. Same shape as `scripts/gate.sh`
# wrapping the four gate commands, and the same argument: CLAUDE.md § Observer Blindness
# position 3 asks that the correct path END IN A SAFE STATE, so compliance leaves nothing
# armed. "Remember to check whether you are behind" is the policy that already failed.
#
# WHY IT FETCHES, WHICH IS THE WHOLE DESIGN
# `git rev-list --count HEAD..@{upstream}` reads `origin/<branch>`, a LOCAL ref that is
# only as fresh as your last fetch. Run without fetching it returns 0 on a checkout that
# is badly behind — an instrument correctly reading a stale tree and reporting success,
# which is F-6's own defect reproduced inside its own guard. So this script fetches
# first, and when the fetch FAILS it refuses to answer rather than passing the stale
# reading through. A zero from a ref nobody refreshed is "not looked at", never
# "not behind" — `docs/adrs/2026-08-27-negative-results-name-their-scope.md`.
#
# WHAT IT DOES NOT CHECK, said plainly so the green is not read as wider than it is:
# whether the working tree carries uncommitted Rust, yours or a peer's. Being level with
# origin does not make the tree equal to origin, and on this checkout it usually is not.
# This answers exactly one question — "does my HEAD contain what origin has?" — and F-6's
# standing instruction is unchanged for everything else: after any rebuild you are relying
# on, ASK THE BINARY WHAT IT DOES, NOT WHAT IT IS.

set -u

# THE BUILD COMMAND, overridable ONLY so this script's own suite can assert the ALLOW
# path. A guard exercised only on its refusals is indistinguishable from one that refuses
# everything, and the allow path here ends in a multi-minute `--release` build that no
# test can run. This is the seam that makes a positive control possible; it is not a
# feature, and nothing in the repo sets it outside `tests/rb-guard.sh`.
: "${CODESCOUT_RB_BUILD_CMD:=cargo rb}"
build_and_exit() {
    # shellcheck disable=SC2086  # deliberate word-splitting: the override is a command line
    exec $CODESCOUT_RB_BUILD_CMD "$@"
}

# Not a git checkout: nothing to be behind. Build and get out of the way — a guard that
# refuses where its question is meaningless is just an outage.
if ! git rev-parse --git-dir >/dev/null 2>&1; then
    build_and_exit "$@"
fi

branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo HEAD)

# Detached HEAD or no configured upstream. NOT refused: both are legitimate states (a
# bisect, a probe worktree, a fresh clone of a branch nobody tracks), and this script has
# no business blocking them. But it says so, because silence here is indistinguishable
# from a clean check that passed — which is the class the script exists to fix.
if ! upstream=$(git rev-parse --abbrev-ref --symbolic-full-name '@{upstream}' 2>/dev/null); then
    cat >&2 <<EOF
rb.sh: no upstream configured for '$branch', so "am I behind origin?" has no answer here.
  Building anyway — this is a normal state for a detached HEAD, a probe worktree, or a
  branch nobody tracks, and refusing would block work over a question that does not apply.
  Said out loud because an unremarked pass reads exactly like a check that succeeded.

EOF
    build_and_exit "$@"
fi

# THE FETCH IS THE CHECK. Without it the count below is computed against whatever
# `origin/$branch` happened to be at your last fetch, and returns 0 on a checkout that is
# badly behind. Read the header if this looks like belt-and-braces: the stale-ref reading
# is F-6's exact defect, one layer up.
if ! git fetch --quiet origin "$branch" 2>/dev/null; then
    cat >&2 <<EOF
rb.sh: REFUSING — the fetch of origin/$branch failed, so this script cannot tell whether
  your HEAD contains what origin has.

  It is refusing rather than proceeding, and the distinction is the point: without a
  fetch, \`git rev-list --count HEAD..@{upstream}\` reads a local ref frozen at your last
  fetch and returns 0 for a checkout that is arbitrarily far behind. That zero would mean
  NOT LOOKED AT and would read as NOT BEHIND — which is the exact failure this script
  exists to prevent, so producing it here would be worse than having no script.

  If you are offline or origin is unreachable and you want the build anyway:

      CODESCOUT_RB_ACK=behind ./scripts/rb.sh

  That records a decision instead of hiding a question. After it, verify the binary
  BEHAVIOURALLY rather than trusting the build — mtime and exit status are uninformative
  in both directions (F-6's table):

      LIBRARIAN_DB=\$(mktemp -u) codescout doc find --kind tracker
EOF
    [ "${CODESCOUT_RB_ACK:-}" = "behind" ] || exit 2
    echo "rb.sh: CODESCOUT_RB_ACK=behind set — building on an unverified tree." >&2
    build_and_exit "$@"
fi

behind=$(git rev-list --count "HEAD..$upstream" 2>/dev/null || echo unknown)

if [ "$behind" = "unknown" ]; then
    echo "rb.sh: REFUSING — could not count commits between HEAD and $upstream." >&2
    echo "  Set CODESCOUT_RB_ACK=behind to build anyway." >&2
    [ "${CODESCOUT_RB_ACK:-}" = "behind" ] || exit 2
elif [ "$behind" -ne 0 ]; then
    cat >&2 <<EOF
rb.sh: REFUSING — HEAD is $behind commit(s) behind $upstream, so \`cargo rb\` would
  compile a tree that does not contain what origin has, exit 0, update the binary's
  mtime, and ship it. That is F-6 exactly, and it cost a wasted rebuild plus a window in
  which a fix was believed live and was not.

  What origin has and you do not:

$(git log --format='      %h  %s' "HEAD..$upstream" | head -10)

  THE REMEDY IS NOT AUTOMATIC, and that is deliberate on a shared checkout. Moving HEAD
  here moves it for EVERY session in this working tree, several of which may hold
  uncommitted work on it — so this script will not run a pull for you. Pick one:

    * You own the tree state right now:   git pull --rebase
      Check first who else is live:       ./scripts/peer-sessions.sh
    * You are deliberately building an older tree (a bisect, reproducing a defect):
                                          CODESCOUT_RB_ACK=behind ./scripts/rb.sh

  The second is not a bypass to reach for when the first is inconvenient. It records
  that you decided to build a known-stale tree — which is a real and legitimate thing to
  do, and a different act from not having looked.
EOF
    if [ "${CODESCOUT_RB_ACK:-}" != "behind" ]; then
        exit 1
    fi
    echo >&2
    echo "rb.sh: CODESCOUT_RB_ACK=behind set — building $behind commit(s) behind origin, deliberately." >&2
fi

build_and_exit "$@"
