#!/usr/bin/env bash
#
# Discrimination for `scripts/install-hooks.sh --check`'s hook POPULATION, not the
# health of any one hook (that half is `tests/pre-push-foreign-session-guard.sh`'s
# "install-hooks.sh --check refuses a STALE shim" section).
#
# WHY THIS EXISTS
# ---------------
# docs/issues/archive/2026-09-09-install-hooks-check-reports-all-ok-for-a-skipped-pre-push-guard.md:
# `--check`'s selector was its own hardcoded list of `install_shim` call sites. A branch
# whose copy of this file predates some hook's addition to that list iterates its OWN
# short list, finds every entry on it healthy, and exits 0 -- while the hooks directory
# it never looked at (`git rev-parse --git-path hooks`, the COMMON dir shared by every
# worktree and every branch checked out into one) holds a hook that branch has no idea
# exists, installed there by a different branch's `install-hooks.sh` run. A rescue path
# that is supposed to fail independently of whether anyone reads stderr at push time
# instead certified health.
#
# The fix inverts the selector: the ground truth for what a `--check` run reports on is
# the directory listing, not this script's call sites. This file's job is to prove that
# inversion actually discriminates -- silence when the directory holds only what this
# copy of the script manages, and a named, actionable report when it holds something
# this copy cannot vouch for -- and to prove it survives the EXACT mechanism the bug
# report measured: a linked worktree, sharing the common hooks dir, whose checked-out
# branch's own install-hooks.sh has no call site for a hook that is nonetheless live
# there.
#
# HERMETIC ON PURPOSE, same idiom as tests/pre-push-foreign-session-guard.sh and
# tests/hooks-discrimination.sh: every case below builds its own throwaway repo under
# $TMPDIR and copies the REAL script into it, so a mutation to install-hooks.sh is what
# this file is exercising, not a second reimplementation of it.
#
# Usage:
#   tests/install-hooks-check-population.sh     # non-zero exit on any failure

set -uo pipefail

INSTALLER="$(cd "$(dirname "$0")/../scripts" && pwd)/install-hooks.sh"
[ -x "$INSTALLER" ] || { echo "FATAL: $INSTALLER is not executable"; exit 1; }

PASS=0
FAIL=0
ok() { echo "  PASS  $1"; PASS=$((PASS + 1)); }
no() {
    echo "  FAIL  $1"
    [ -n "${2:-}" ] && echo "        $2"
    FAIL=$((FAIL + 1))
}
eq() { [ "$2" = "$3" ] && ok "$1" || no "$1" "want exit $3, got $2"; }
has() { printf '%s' "$2" | grep -qF -- "$3" && ok "$1" || no "$1" "expected to find: $3"; }
hasnt() { printf '%s' "$2" | grep -qF -- "$3" && no "$1" "should NOT contain: $3" || ok "$1"; }

# ---------------------------------------------------------------- throwaway repo builder
REPO=""
new_repo() {
    REPO="$(mktemp -d "${TMPDIR:-/tmp}/ih-check-pop-XXXXXX")"
    git -C "$REPO" init -q -b main
    git -C "$REPO" config user.email t@example.invalid
    git -C "$REPO" config user.name Test
    git -C "$REPO" config --unset core.hooksPath 2>/dev/null || true
}

hooks_dir_of() {
    local d
    d="$(git -C "$1" rev-parse --git-path hooks)"
    case "$d" in /*) printf '%s\n' "$d" ;; *) printf '%s\n' "$1/$d" ;; esac
}

# A fixture with the real install-hooks.sh and all of its known targets, seeded and
# committed. No fakebin/pre-commit stub: install-hooks.sh retired the pre-commit.com
# framework, so that stub is inert (same note as installer_fixture() in
# tests/pre-push-foreign-session-guard.sh, which this mirrors).
installer_fixture() {  # -> $REPO, seeded and committed, all known targets executable
    new_repo
    mkdir -p "$REPO/scripts"
    cp "$INSTALLER" "$REPO/scripts/install-hooks.sh"
    chmod +x "$REPO/scripts/install-hooks.sh"
    for t in pre-commit-run post-index-change-stage-log pre-push-foreign-session-guard \
             prepare-commit-msg-session-id; do
        printf '#!/usr/bin/env bash\nexit 0\n' > "$REPO/scripts/$t.sh"
        chmod +x "$REPO/scripts/$t.sh"
    done
    git -C "$REPO" add -A
    git -C "$REPO" commit -q -m "seed with scripts"
}

# run <args...> against $REPO -> sets OUT, EC
OUT=""; EC=0
check() {
    OUT="$(cd "$REPO" && bash scripts/install-hooks.sh --check 2>&1)"
    EC=$?
}

echo
echo "== control: a healthy install reports clean, and git's OWN *.sample hooks (shipped"
echo "   executable by 'git init' -- measured, not assumed) never register as unrecognized =="
installer_fixture
(cd "$REPO" && bash scripts/install-hooks.sh >/dev/null 2>&1)
check
eq   "clean checkout: --check exits 0"              "$EC" 0
hasnt "clean checkout: no UNRECOGNIZED line at all" "$OUT" "UNRECOGNIZED"
# Positive control that the exclusion is real, not incidental: confirm the samples exist
# in this fixture's own hooks dir (git ships them on every `git init`), then confirm none
# of them was reported.
HOOKS_DIR="$(hooks_dir_of "$REPO")"
if ls "$HOOKS_DIR"/*.sample >/dev/null 2>&1; then
    ok "fixture: git's *.sample hooks are present to be (wrongly) flagged"
    for s in "$HOOKS_DIR"/*.sample; do
        hasnt "sample $(basename "$s") is not reported" "$OUT" "$(basename "$s")"
    done
else
    no "fixture: git's *.sample hooks are present to be (wrongly) flagged" \
       "none found in $HOOKS_DIR -- this control cannot discriminate"
fi

echo
echo "== OBSERVED RED (pre-fix), reproduced here as the regression this section guards:"
echo "   a hook this copy of install-hooks.sh has no call site for, planted directly in"
echo "   the shared hooks dir, made the pre-fix --check print four ok/off lines and exit 0."
echo "   Fixed behaviour asserted below. =="
installer_fixture
(cd "$REPO" && bash scripts/install-hooks.sh >/dev/null 2>&1)
HOOKS_DIR="$(hooks_dir_of "$REPO")"
printf '#!/usr/bin/env bash\nexit 0\n' > "$HOOKS_DIR/post-checkout"
chmod +x "$HOOKS_DIR/post-checkout"
check
eq  "an unmanaged live hook: --check now refuses"        "$EC" 1
has "names it by hook name"                              "$OUT" "UNRECOGNIZED post-checkout"
has "names the shared directory it was found in"         "$OUT" "$HOOKS_DIR"
# THE REMEDY TEXT, asserted as SHAPE rather than as exact prose (a prose pin reds on
# every rewording and buys nothing a shape check does not). The claim under test is
# narrow: does the report still tell a reader what to DO, not merely that something is
# wrong -- the "Loudness is a property of a PATH" / "test the remedy, not only the
# predicate" convention this repo's CLAUDE.md states explicitly.
has "remedy names an inspection step"                    "$OUT" "Inspect it"
has "remedy gives a concrete command"                     "$OUT" "cat \"$HOOKS_DIR/post-checkout\""
has "remedy names the actual next action (switch branch)" "$OUT" "check out a branch"
# The population scan must not crowd out or replace the per-hook health checks --
# a known, healthy hook must still report as such alongside the unrecognized one.
has "known hooks are still individually verified"        "$OUT" "ok      pre-push      shim matches generator"
has "known hooks are still individually verified"        "$OUT" "ok      post-index-change      shim matches generator"

echo
echo "== the footer must not collapse UNRECOGNIZED into 'NOT installed' -- same law as the"
echo "   2026-09-06 STALE-vs-MISSING footer fix, applied to the third state this adds =="
hasnt "footer does not say the hook is not installed" "$OUT" "One or more hooks are NOT installed"
has   "footer names the UNRECOGNIZED state instead"   "$OUT" "UNRECOGNIZED by this copy of"
has   "footer's own remedy also names an action"      "$OUT" "check out"

echo
echo "== the exact mechanism from the bug report: a linked worktree sharing the common"
echo "   hooks dir, whose checked-out branch's install-hooks.sh predates a hook's own"
echo "   install_shim call site, must still surface that hook via the population scan =="
installer_fixture
(cd "$REPO" && bash scripts/install-hooks.sh >/dev/null 2>&1)
WT="$REPO.wt"
git -C "$REPO" worktree add -q -b oldbranch "$WT" 2>/dev/null
# Simulate "predates the hook": strip the pre-push call site from THIS worktree's own
# copy of the script, mirroring the bug's measured state (2 call sites on the triggering
# commit, neither pre-push) without needing to check out a real historical commit.
if grep -q 'install_shim pre-push scripts/pre-push-foreign-session-guard.sh' "$WT/scripts/install-hooks.sh"; then
    ok "fixture: the worktree's script starts with a pre-push call site to remove"
else
    no "fixture: the worktree's script starts with a pre-push call site to remove" \
       "grep found none -- this section cannot express the defect"
fi
sed -i '/install_shim pre-push scripts\/pre-push-foreign-session-guard\.sh/d' \
    "$WT/scripts/install-hooks.sh"
if grep -q 'install_shim pre-push' "$WT/scripts/install-hooks.sh"; then
    no "fixture: the worktree's script no longer names pre-push" \
       "the call site is still present -- sed did not match"
else
    ok "fixture: the worktree's script no longer names pre-push"
fi
git -C "$WT" add -A
git -C "$WT" commit -q -m "simulate a branch that predates the pre-push hook"
WT_OUT="$(cd "$WT" && bash scripts/install-hooks.sh --check 2>&1)"
WT_EC=$?
eq  "old-branch worktree: --check refuses"                 "$WT_EC" 1
has "old-branch worktree: names the live pre-push hook"    "$WT_OUT" "UNRECOGNIZED pre-push"
has "old-branch worktree: points at the shared common dir" "$WT_OUT" "$(hooks_dir_of "$REPO")"

echo
echo "-------------------------------------------"
echo "  $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] || exit 1
