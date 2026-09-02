#!/usr/bin/env bash
#
# Discrimination matrix for the shared-checkout git hooks.
#
# WHY THIS EXISTS
# ---------------
# `scripts/pre-commit-foreign-index.sh` and `scripts/post-index-change-stage-log.sh`
# guard against one session's commit capturing another's staged work. The guard failed
# in production within two hours of being installed
# (docs/issues/archive/2026-09-01-foreign-index-guard-passed-a-peers-staged-deletion.md),
# and nothing in the repo would have caught a recurrence: there is no shellcheck, no CI
# step for `scripts/`, and the sibling hook `pre-commit-unreviewed-content.sh` has no
# test at all. Its precedent was a hand-run matrix pasted into a commit message, which
# is evidence about one instant and cannot fail a later build.
#
# Every case below is a DISCRIMINATION: each asserts the hook is silent where it must be
# silent AND loud where it must be loud. A suite checking only the loud direction passes
# against a hook that refuses everything; one checking only silence passes against a hook
# that was deleted.
#
# Usage:
#   tests/hooks-discrimination.sh          # all suites; non-zero exit on any failure
#
# Runs entirely inside throwaway repos under $TMPDIR. It never touches this checkout,
# which matters because several sessions share it and a stray `git reset` here would
# destroy their uncommitted work.

set -uo pipefail

SRC="$(cd "$(dirname "$0")/../scripts" && pwd)"
REAL_REPO="$(cd "$(dirname "$0")/.." && pwd)"
PASS=0
FAIL=0

ok() { echo "  PASS  $1"; PASS=$((PASS + 1)); }
no() {
    echo "  FAIL  $1"
    [ -n "${2:-}" ] && echo "        $2"
    FAIL=$((FAIL + 1))
}
eq() { [ "$2" = "$3" ] && ok "$1" || no "$1" "want '$3' got '$2'"; }
has() { printf '%s' "$2" | grep -qF "$3" && ok "$1" || no "$1" "missing: $3"; }

# REFUSE TO RUN ANYWHERE BUT A THROWAWAY. Defence in depth, and not theoretical.
#
# This suite runs `git add -A` and `git commit`. On 2026-09-01 its first version defined
# `new_repo() { cd "$(mktemp -d)"; ...; }` and called it as `T="$(new_repo)"` — command
# substitution runs a SUBSHELL, so the `cd` never reached the parent and every one of
# those commands executed against the real shared checkout. A junk commit landed on
# `experiments`, and a `git reset --hard` in the same suite ran in a tree four other
# sessions were working in.
#
# The `cd` bug is fixed below. This check exists because fixing a bug is not the same as
# making its class impossible: any future edit that reintroduces a subshell gets a
# refusal instead of a live repo. `git reset --hard` is also gone from the suite outright
# — a plain `git reset` empties the index without touching the working tree, which is all
# any case here ever needed, so the destructive form has no reason to appear in a test.
assert_throwaway() {
    _at="$(pwd -P)"
    case "$_at" in
        "$REAL_REPO" | "$REAL_REPO"/*)
            echo "REFUSING: this suite is running inside the real checkout:" >&2
            echo "    $_at" >&2
            echo "It runs destructive git commands and must only run in a throwaway." >&2
            exit 1
            ;;
    esac
    [ -d "$_at/.git" ] || {
        echo "REFUSING: no .git at $_at — not running git commands here." >&2
        exit 1
    }
}

# Sets $T and cds into it. Call as `new_repo`, NEVER as `T="$(new_repo)"` — that is the
# subshell that caused the incident described above.
new_repo() {
    T="$(mktemp -d)"
    cd "$T" || exit 1
    git init -q .
    assert_throwaway
    git config user.email t@t
    git config user.name t
    mkdir -p .git/hooks
    cat > .git/hooks/post-index-change <<SHIM
#!/usr/bin/env bash
exec "$SRC/post-index-change-stage-log.sh" "\$@"
SHIM
    chmod +x .git/hooks/post-index-change
}

# The guard as a bare (index) commit sees it. GIT_INDEX_FILE must be UNSET rather than
# empty: an empty value makes git read a nonexistent index and report every dst blob as
# zeros, so nothing matches the log and the guard reads everything as ours — silence for
# entirely the wrong reason.
guard() {
    env -u GIT_INDEX_FILE CLAUDE_CODE_SESSION_ID="$1" \
        bash "$SRC/pre-commit-foreign-index.sh" 2>&1
    echo "EXIT=$?"
}
owner_of() { awk -F'\t' -v p="$1" '$3 == p { print $1; exit }' .git/session-stage-log; }

A="aaaaaaaa-0000-0000-0000-aaaaaaaaaaaa"
B="bbbbbbbb-1111-1111-1111-bbbbbbbbbbbb"

# ---------------------------------------------------------------- 1. the index guard
echo "== index guard"
new_repo
echo base > a.txt
echo base > b.txt
git add -A > /dev/null 2>&1
git commit -qm base

echo mine > a.txt
CLAUDE_CODE_SESSION_ID="$A" git add a.txt
eq "records the staging session" "$(owner_of a.txt)" "$A"
has "own paths only -> silent" "$(guard "$A")" "EXIT=0"

echo theirs > b.txt
CLAUDE_CODE_SESSION_ID="$B" git add b.txt
out="$(guard "$A")"
has "foreign path -> refuse" "$out" "EXIT=1"
has "names the foreign path" "$out" "b.txt"
has "offers the pathspec remedy for mine" "$out" "git commit -- a.txt"

# A pathspec commit gets a temp index named next-index-<pid>.lock and ignores the shared
# index entirely, so it cannot capture and must not be refused.
has "pathspec commit -> silent" \
    "$(CLAUDE_CODE_SESSION_ID="$A" GIT_INDEX_FILE=".git/next-index-1.lock" \
        bash "$SRC/pre-commit-foreign-index.sh" 2>&1; echo "EXIT=$?")" "EXIT=0"

git reset -q
has "nothing staged -> silent" "$(guard "$A")" "EXIT=0"

echo x > a.txt
CLAUDE_CODE_SESSION_ID="$B" git add a.txt
has "no session id -> silent" \
    "$(env -u GIT_INDEX_FILE -u CLAUDE_CODE_SESSION_ID \
        bash "$SRC/pre-commit-foreign-index.sh" 2>&1; echo "EXIT=$?")" "EXIT=0"
rm -rf "$T"

# ------------------------------------------------ 2. the stager wins, not the observer
# The production failure. `post-index-change` fires on EVERY index write including
# `git status`, so "first observer wins" let a passer-by claim a peer's staged batch.
echo "== stager wins"
new_repo
echo base > s1.txt
echo gone > del.txt
git add -A > /dev/null 2>&1
git commit -qm base

echo mine > s1.txt
CLAUDE_CODE_SESSION_ID="$A" git add s1.txt
CLAUDE_CODE_SESSION_ID="$A" git rm -q --cached del.txt
eq "a staged DELETION is recorded" "$(owner_of del.txt)" "$A"

CLAUDE_CODE_SESSION_ID="$B" git status --short > /dev/null
eq "peer's git status does not steal an edit" "$(owner_of s1.txt)" "$A"
eq "peer's git status does not steal a deletion" "$(owner_of del.txt)" "$A"

rm -f .git/session-stage-log
CLAUDE_CODE_SESSION_ID="$B" git status --short > /dev/null
eq "cold log + peer status -> unknown, not the passer-by" "$(owner_of s1.txt)" "-"
out="$(guard "$B")"
has "unknown reads as foreign -> refuse" "$out" "EXIT=1"
has "refusal names the staged deletion" "$out" "del.txt"

echo more > s1.txt
CLAUDE_CODE_SESSION_ID="$A" git add s1.txt
eq "a real add still claims normally" "$(owner_of s1.txt)" "$A"

# Invocation form must not change attribution. `staging_op` classifies the tokens after
# `git` in /proc/$PPID/cmdline, and git's global flags come in two shapes: `--git-dir=X`
# is ONE token, but `-C <path>` and `--git-dir <path>` put the value in its own argv slot.
# A parser that skips the flag and then classifies the next token reads a PATH as the
# subcommand. Every add below is the same operation by the same session as the one above.
# `git -C` matters most because it is the form a session reaches for in a multi-worktree
# checkout. It is NOT mandated for staging: an earlier version of this comment said the
# companion's worktree guard requires it, which was wrong and was retracted at the hook
# source (F-90) while this copy was missed. Probed directly 2026-09-01 against
# git-worktree-guard.mjs: it triggers only on the commit family
# (commit/push/reset --hard/rebase/merge/checkout -b), so a bare `git add` passes. The
# parse bug these cases guard is real regardless of the guard's scope.
echo formC > formC.txt
CLAUDE_CODE_SESSION_ID="$A" git -C "$PWD" add formC.txt
eq "-C <path> add is a staging op" "$(owner_of formC.txt)" "$A"

echo formJ > formJ.txt
CLAUDE_CODE_SESSION_ID="$A" git --git-dir="$PWD/.git" --work-tree="$PWD" add formJ.txt
eq "--git-dir=X joined add is a staging op" "$(owner_of formJ.txt)" "$A"

echo formS > formS.txt
CLAUDE_CODE_SESSION_ID="$A" git --git-dir "$PWD/.git" --work-tree "$PWD" add formS.txt
eq "--git-dir X separate add is a staging op" "$(owner_of formS.txt)" "$A"

echo formK > formK.txt
CLAUDE_CODE_SESSION_ID="$A" git -c user.name=t -C "$PWD" add formK.txt
eq "-c k=v then -C <path> add is a staging op" "$(owner_of formK.txt)" "$A"

# The other direction, and it is what stops "skip the value too" being over-applied: a
# NON-staging verb wearing the same flags must still fail to claim. A fix that swallows
# one token after every flag, or that returns 0 whenever it cannot classify, passes the
# four cases above and fails this one.
rm -f .git/session-stage-log
CLAUDE_CODE_SESSION_ID="$B" git -C "$PWD" status --short > /dev/null
eq "-C <path> status is NOT a staging op" "$(owner_of formC.txt)" "-"

rm -rf "$T"

# ------------------------------------- 2b. a cold log claims only what argv NAMED
# The cross-claim. `:135` used to be `[ -n "$owner" ] || owner="$claimant"`, so a pair with
# no surviving row went to whoever caused the CURRENT write -- one session staging one file
# became the recorded owner of every staged path, its peers' included, and the guard then
# saw nothing foreign and passed silently.
#
# The trigger needs no `rm -f`: ONE hook invocation that does not complete is enough, and an
# inherited CODESCOUT_STAGE_LOG_RUNNING is the cheapest way to reach it -- which is what the
# suppressed `git add` below simulates. Measured 2026-09-01.
echo "== cold log claims only what argv named"
new_repo
echo seed > seed.txt
git add -A > /dev/null 2>&1
git commit -qm base

# A stages with its hook suppressed, so the log never learns about peer.txt.
echo peer > peer.txt
CODESCOUT_STAGE_LOG_RUNNING=1 CLAUDE_CODE_SESSION_ID="$A" git add peer.txt
# B then stages ITS OWN file, normally. B named mine.txt and nothing else.
echo mine > mine.txt
CLAUDE_CODE_SESSION_ID="$B" git add mine.txt

eq "cold log: B claims the path B named" "$(owner_of mine.txt)" "$B"
eq "cold log: B does NOT claim A's staged path" "$(owner_of peer.txt)" "-"
out="$(guard "$B")"
has "an unowned peer path still refuses" "$out" "EXIT=1"
has "refusal names the unowned path" "$out" "peer.txt"

# A blanket form names no path, so it claims nothing -- including its own. This is the
# intended degradation, not a regression: `git add -A` followed by a BARE commit is exactly
# the capture this guard exists for, so making it loud is the point. The pathspec commit
# remedy is unaffected, because it never reads the shared index at all.
rm -f .git/session-stage-log
echo extra > extra.txt
CLAUDE_CODE_SESSION_ID="$B" git add -A
eq "git add -A names no path, so it claims nothing" "$(owner_of extra.txt)" "-"
has "and a bare commit after -A is refused" "$(guard "$B")" "EXIT=1"
rm -rf "$T"

# -------------------------------------------------------------- 3. owner resolution
echo "== owner resolution"
new_repo
LIVE="${CLAUDE_CODE_SESSION_ID:-}"
if [ -n "$LIVE" ]; then
    echo base > live.txt
    echo base > dead.txt
    git add -A > /dev/null 2>&1
    git commit -qm base
    echo x > live.txt
    echo x > dead.txt
    git add -A > /dev/null 2>&1
    git diff --cached --raw > raw.tmp
    : > .git/session-stage-log
    while IFS=$'\t' read -r blob path; do
        case "$path" in
            live.txt) o="$LIVE" ;;
            *) o="99999999-dead-dead-dead-999999999999" ;;
        esac
        printf '%s\t%s\t%s\n' "$o" "$blob" "$path" >> .git/session-stage-log
    done < <(awk -F'\t' '{ split($1, a, " "); print a[4] "\t" $2 }' raw.tmp)
    rm -f raw.tmp
    out="$(guard "00000000-1111-2222-3333-444444444444")"
    has "a live owner resolves" "$out" "LIVE — "
    has "a live owner gets an address" "$out" "cc-socks/"
    has "an absent owner is NOT LIVE" "$out" "NOT LIVE"
    has "and points at its transcript" "$out" ".jsonl"
else
    echo "  SKIP  owner resolution (no CLAUDE_CODE_SESSION_ID to resolve against)"
fi
rm -rf "$T"

# ---------------------------------- 4. the neighbouring hook still covers its own axis
# foreign-index covers CROSS-path capture; unreviewed-content covers INTRA-path, the
# working tree moving under a pathspec commit after you staged. Neither covers the
# other, so a change to one must never be read as covering both.
echo "== intra-path axis (pre-commit-unreviewed-content.sh)"
new_repo
echo base > f.txt
git add f.txt > /dev/null 2>&1
git commit -qm base
echo mine > f.txt
git add f.txt
echo "mine + THEIR LINE" > f.txt
cp .git/index .git/next-index-9.lock
GIT_INDEX_FILE=".git/next-index-9.lock" git add f.txt
out="$(GIT_INDEX_FILE=".git/next-index-9.lock" \
    bash "$SRC/pre-commit-unreviewed-content.sh" 2>&1; echo "EXIT=$?")"
has "working tree moved after staging -> refuse" "$out" "EXIT=1"
has "names the file" "$out" "f.txt"
rm -rf "$T"

# ------------------------------------------ 6. `git apply --cached` names paths in the PATCH
# docs/issues/archive/2026-09-01-git-apply-cached-stages-but-records-no-owner.md
#
# `apply` sits in staging_op()'s verb list, so the write is eligible to claim -- but
# argv_paths() emits the POSITIONAL, which for `apply` is the PATCH FILE and never a staged
# path. names_path() cannot match it, so every `apply --cached` records `-`, and
# pre-commit-foreign-index then refuses the stager's own commit while naming an owner who
# does not exist. That disables the one tool able to split a file holding two sessions'
# edits -- the documented remedy for the capture bug this whole suite exists after.
#
# The fix must not spend names_path()'s strictness to buy this. A patch's own `+++ b/<p>`
# and `--- a/<p>` headers are the same KIND of thing argv is for `add`: the set of paths
# this invocation intends to stage. Deriving from the index instead would drop the
# restriction altogether and re-open the false-claim failure it exists to prevent.
echo
echo "== apply --cached claims what the PATCH names"

new_repo
printf 'a\nb\n' > f.txt
git add f.txt > /dev/null 2>&1
git commit -qm base > /dev/null 2>&1
printf 'a\nX\nb\n' > f.txt
git diff f.txt > p1.patch
git checkout -- f.txt
CLAUDE_CODE_SESSION_ID="$A" git apply --cached p1.patch
eq "apply --cached claims the patch's path" "$(owner_of f.txt)" "$A"

# A path the patch does NOT name must stay unclaimed, though it sits in the very same
# `git diff --cached --raw` output the hook iterates. This is precisely what names_path()
# buys and the fix must leave it intact -- mutate the fix to claim every diffed path and
# this is the assertion that dies.
new_repo
printf 'a\n' > mine.txt
printf 'a\n' > theirs.txt
git add mine.txt theirs.txt > /dev/null 2>&1
git commit -qm base > /dev/null 2>&1
printf 'peer edit\n' > theirs.txt
CLAUDE_CODE_SESSION_ID="$B" git add theirs.txt
printf 'b\n' > mine.txt
git diff mine.txt > only-mine.patch
git checkout -- mine.txt
CLAUDE_CODE_SESSION_ID="$A" git apply --cached only-mine.patch
eq "the patch's own path goes to the applier" "$(owner_of mine.txt)" "$A"
eq "a co-staged path the patch never named stays with its stager" "$(owner_of theirs.txt)" "$B"

# New-file patch: the pre-image is /dev/null, so the path appears only on the +++ side.
new_repo
printf 'a\n' > base.txt
git add base.txt > /dev/null 2>&1
git commit -qm base > /dev/null 2>&1
printf 'brand new\n' > added.txt
git add -N added.txt > /dev/null 2>&1
git diff added.txt > new.patch
git rm -q --cached added.txt > /dev/null 2>&1
rm -f added.txt
CLAUDE_CODE_SESSION_ID="$A" git apply --cached new.patch
eq "a new-file patch (--- /dev/null) is claimed" "$(owner_of added.txt)" "$A"

# Deletion patch: the post-image is /dev/null, so the path appears only on the --- side.
new_repo
printf 'a\n' > doomed.txt
git add doomed.txt > /dev/null 2>&1
git commit -qm base > /dev/null 2>&1
git rm -q doomed.txt > /dev/null 2>&1
# The `--` is load-bearing: after `git rm` the path is gone from the working tree, so
# `git diff --cached doomed.txt` is ambiguous, errors, and writes an EMPTY patch. `apply`
# then fails with "No valid patches in input", nothing is staged, and the assertion below
# fails against a case that never ran -- which reads exactly like a defect in patch_paths.
git diff --cached -- doomed.txt > del.patch
git reset -q --hard > /dev/null 2>&1
CLAUDE_CODE_SESSION_ID="$A" git apply --cached del.patch
eq "a deletion patch (+++ /dev/null) is claimed" "$(owner_of doomed.txt)" "$A"

# Multi-file patch: every path it names, not merely the first.
new_repo
printf 'a\n' > m1.txt
printf 'a\n' > m2.txt
git add m1.txt m2.txt > /dev/null 2>&1
git commit -qm base > /dev/null 2>&1
printf 'b\n' > m1.txt
printf 'b\n' > m2.txt
git diff > multi.patch
git checkout -- m1.txt m2.txt
CLAUDE_CODE_SESSION_ID="$A" git apply --cached multi.patch
eq "multi-file patch claims the first path" "$(owner_of m1.txt)" "$A"
eq "multi-file patch claims the second path" "$(owner_of m2.txt)" "$A"

# -p0 changes what a +++ header means, so the header no longer names a repo-relative path.
# OVER-REFUSE rather than guess: names_path()'s asymmetry is the whole design, and a miss
# is recoverable where a false hit is not.
new_repo
printf 'a\n' > z.txt
git add z.txt > /dev/null 2>&1
git commit -qm base > /dev/null 2>&1
printf 'b\n' > z.txt
git diff --no-prefix z.txt > p0.patch
git checkout -- z.txt
CLAUDE_CODE_SESSION_ID="$A" git apply --cached -p0 p0.patch
eq "an unusual -p level over-refuses rather than guessing" "$(owner_of z.txt)" "-"

# A patch read from STDIN puts no filename in argv at all. Nothing to open, so nothing to
# claim -- and that must stay a `-` rather than becoming a claim on everything diffed.
new_repo
printf 'a\n' > s.txt
git add s.txt > /dev/null 2>&1
git commit -qm base > /dev/null 2>&1
printf 'b\n' > s.txt
git diff s.txt > s.patch
git checkout -- s.txt
CLAUDE_CODE_SESSION_ID="$A" git apply --cached < s.patch
eq "a patch on stdin over-refuses (no filename in argv)" "$(owner_of s.txt)" "-"

# ------------------------- 7. the guard stands down where git refuses its own remedy
# docs/issues/archive/2026-09-02-foreign-index-prescribes-a-remedy-git-refuses.md
#
# The refusal names exactly ONE escape — `git commit -- <path>` — and git rejects that
# form outright during a sequencer stop ("cannot do a partial commit during a
# cherry-pick"), while the bare form being refused is the only one it will accept. A
# guard may refuse; it may not refuse and then name a route git will reject, because the
# caller's only remaining move is `--no-verify`.
#
# Keyed on CHERRY_PICK_HEAD / MERGE_HEAD and NOT on "a rebase is running". Measured
# 2026-09-02: a rebase stopped with rebase-merge/ present and CHERRY_PICK_HEAD absent
# commits by pathspec fine, so the wider test would stand the guard down in a state where
# the prescribed remedy still works.
#
# EVERY case below asserts the path is FOREIGN before calling the guard. Without that the
# hook can exit 0 for entirely the wrong reason — an unmatched log key reads as "all mine"
# and passes silently, which is the false green that cost a probe upstream.
echo "== sequencer stand-down"

# Ownership lookup that survives a linked worktree, where `.git` is a file, not a dir.
owner_at() { awk -F'\t' -v p="$1" '$3 == p { print $1; exit }' \
    "$(git rev-parse --git-dir)/session-stage-log" 2>/dev/null; }

# Build master/topic tips that conflict on one path, then leave the repo on master.
conflicting_tips() {
    echo base > a.txt
    git add a.txt > /dev/null 2>&1
    git commit -qm base
    git branch -q topic
    echo master > a.txt && git commit -qam master
    git checkout -q topic
    echo topic > a.txt && git commit -qam topic
    git checkout -q master
}

new_repo
conflicting_tips
git cherry-pick topic > /dev/null 2>&1
echo resolved > a.txt
CLAUDE_CODE_SESSION_ID="$B" git add a.txt
eq "cherry-pick fixture stages a FOREIGN path" "$(owner_at a.txt)" "$B"
has "cherry-pick stop -> stand down" "$(guard "$A")" "EXIT=0"
rm -rf "$T"

# The control, and the only case that fails if the stand-down is written unconditionally
# — which is the cheapest wrong fix here. Identical fixture minus the sequencer state.
new_repo
echo base > a.txt
git add a.txt > /dev/null 2>&1
git commit -qm base
echo edited > a.txt
CLAUDE_CODE_SESSION_ID="$B" git add a.txt
eq "control fixture stages a FOREIGN path" "$(owner_at a.txt)" "$B"
has "no sequencer -> still refuses" "$(guard "$A")" "EXIT=1"
rm -rf "$T"

# MERGE_HEAD is a separate arm of the condition and needs its own case: deleting it leaves
# every cherry-pick case above green.
new_repo
conflicting_tips
git merge topic > /dev/null 2>&1
echo resolved > a.txt
CLAUDE_CODE_SESSION_ID="$B" git add a.txt
eq "merge fixture stages a FOREIGN path" "$(owner_at a.txt)" "$B"
has "merge stop -> stand down" "$(guard "$A")" "EXIT=0"
rm -rf "$T"

# The reported incident: a linked worktree, where sequencer state lives per-worktree under
# `$git_dir` rather than the common dir. A probe reading the common dir finds nothing here.
new_repo
conflicting_tips
WT="$(mktemp -d)"
git worktree add -q "$WT" topic > /dev/null 2>&1
# NOT a subshell: `eq`/`has` increment PASS/FAIL, and a subshell would discard both — the
# assertions would still print, and the suite's exit code would stop depending on them.
# Same trap this file's `new_repo` header records.
cd "$WT" || exit 1
git cherry-pick master > /dev/null 2>&1
echo resolved > a.txt
CLAUDE_CODE_SESSION_ID="$B" git add a.txt
eq "worktree fixture stages a FOREIGN path" "$(owner_at a.txt)" "$B"
has "worktree sequencer stop -> stand down" "$(guard "$A")" "EXIT=0"
cd "$T" || exit 1
git worktree remove --force "$WT" > /dev/null 2>&1
rm -rf "$WT" "$T"

# ---------------------------------------------------------------------------
# 8. THE ROUTE COLUMN — WHY `-` was recorded, not merely that it was.
#
# docs/issues/archive/2026-09-02-foreign-index-refusal-names-a-cause-no-route-produces.md: the
# guard asserted ONE cause ("staged before this guard was installed") for a state that
# four separate branches reach, and the sentence had frozen while the recorder grew two
# of them under it. Prose cannot be kept correct here by care — the enumeration proposed
# as the cheap fix was ALREADY short by one route on the day it was written (a patch
# whose headers carry no default prefix). So the recorder logs the branch it took and
# the guard reports that instead of listing candidates.
#
# ONE CASE PER BRANCH. A route is written at four sites; a kill at one says nothing
# about the other three.
route_of() { awk -F'\t' -v p="$1" '$3 == p { print $4; exit }' .git/session-stage-log; }
# The ABBREVIATED blob `git diff --raw` emits. A full 40-char sha never matches the log,
# so a legacy-row fixture built from `git hash-object` is silently re-derived rather than
# carried over — a case that passes while testing nothing. Cost this suite's author one
# wrong green.
blob_of() {
    git diff --cached --raw |
        awk -F'\t' -v p="$1" '$2 == p { split($1, x, " "); print x[4]; exit }'
}
hasnt() { printf '%s' "$2" | grep -qF "$3" && no "$1" "must NOT contain: $3" || ok "$1"; }

S_A=route-sess-A

new_repo
echo a > a.txt
CLAUDE_CODE_SESSION_ID="$S_A" git add -- a.txt
eq "explicit-path add records route=named" "$(route_of a.txt)" "named"
eq "explicit-path add still records the real owner" "$(owner_of a.txt)" "$S_A"
rm -rf "$T"

new_repo
mkdir -p sub
echo b > sub/b.txt
CLAUDE_CODE_SESSION_ID="$S_A" git add sub/
eq "directory add records route=unnamed" "$(route_of sub/b.txt)" "unnamed"
eq "directory add leaves the owner unrecorded" "$(owner_of sub/b.txt)" "-"
rm -rf "$T"

new_repo
echo a > a.txt
env -u CLAUDE_CODE_SESSION_ID git add -- a.txt
eq "add with no session id records route=id-unset" "$(route_of a.txt)" "id-unset"
rm -rf "$T"

# An unreadable or unrecognised parent is route 2. Invoking the hook from a plain shell
# reproduces it exactly: $PPID is bash, not git, so staging_op() declines.
new_repo
echo a > a.txt
CLAUDE_CODE_SESSION_ID="$S_A" git add -- a.txt
rm -f .git/session-stage-log
CLAUDE_CODE_SESSION_ID="$S_A" bash -c '.git/hooks/post-index-change'
eq "index write from an unrecognised parent records route=not-staging" \
    "$(route_of a.txt)" "not-staging"
rm -rf "$T"

# Migration. Every log written before this change holds three columns, so this is the
# case that fires FIRST in a live checkout, not an edge case.
new_repo
echo a > a.txt
CLAUDE_CODE_SESSION_ID="$S_A" git add -- a.txt
printf 'SESS-OTHER\t%s\ta.txt\n' "$(blob_of a.txt)" > .git/session-stage-log
echo b > b.txt
CLAUDE_CODE_SESSION_ID="$S_A" git add -- b.txt
eq "a legacy three-column row keeps its owner" "$(owner_of a.txt)" "SESS-OTHER"
eq "a legacy three-column row is labelled pre-route, not blank" \
    "$(route_of a.txt)" "pre-route"
rm -rf "$T"

# The two guard-text cases below are a PAIR on purpose. `has` alone is monotone under
# widening (any prose containing the phrase satisfies it) and `hasnt` alone is monotone
# under deleting the whole block. Together they pin the replacement: the true cause is
# named AND the false one is gone.
new_repo
mkdir -p sub
echo b > sub/b.txt
CLAUDE_CODE_SESSION_ID="$S_A" git add sub/
route_out="$(guard other-session)"
has "guard names the blanket form for an unnamed row" "$route_out" "blanket add"
hasnt "guard no longer asserts the pre-guard cause" \
    "$route_out" "staged before this guard was installed"
rm -rf "$T"

new_repo
echo a > a.txt
CLAUDE_CODE_SESSION_ID="$S_A" git add -- a.txt
printf -- '-\t%s\ta.txt\n' "$(blob_of a.txt)" > .git/session-stage-log
legacy_out="$(guard "$S_A")"
has "guard reports an unknown route honestly for a legacy row" \
    "$legacy_out" "route not recorded"
hasnt "guard does not invent a blanket-add cause for a legacy row" \
    "$legacy_out" "blanket add"
rm -rf "$T"

# THE INVARIANT THAT MAKES THIS SAFE TO SHIP ON A SHARED CHECKOUT, and the reason the
# route is a diagnostic field rather than an input to the decision: the refusal keys on
# the OWNER column alone. A garbage route can therefore only produce a wrong
# explanation — never a wrong refusal, and never a capture. If a later edit makes a
# route value gate the decision, these two cases are what fail.
new_repo
echo a > a.txt
CLAUDE_CODE_SESSION_ID="$S_A" git add -- a.txt
printf 'SESS-OTHER\t%s\ta.txt\tgarbage-route-value\n' "$(blob_of a.txt)" \
    > .git/session-stage-log
has "an unrecognised route value still refuses on the owner" \
    "$(guard "$S_A")" "EXIT=1"
rm -rf "$T"

new_repo
echo a > a.txt
CLAUDE_CODE_SESSION_ID="$S_A" git add -- a.txt
has "my own named paths still pass a bare commit" "$(guard "$S_A")" "EXIT=0"
rm -rf "$T"

# `unnamed` vs `pre-staged` -- the fallback must not assert a cause it did not determine.
#
# Found by codescout-0a reviewing the fix: `route="${claim_route:-unnamed}"` made `unnamed`
# the catch-all, and `unnamed` is NOT neutral -- the guard renders it "blanket add ...
# PROBABLY YOUR OWN STAGING". A pair already in the index when a later command ran reaches
# the same branch, is frequently a PEER's, and was being handed that advice. Both arms now
# key on an observable (did argv name anything at all?) instead of an inferred cause.
new_repo
echo p > p.txt
echo q > q.txt
CLAUDE_CODE_SESSION_ID="$S_A" git add -- p.txt
rm -f .git/session-stage-log
# argv names q.txt; p.txt was already staged, so its row is rebuilt without a claim.
CLAUDE_CODE_SESSION_ID="$S_A" git add -- q.txt
eq "a pre-existing staged pair is pre-staged, NOT unnamed" "$(route_of p.txt)" "pre-staged"
eq "the path argv actually named is still claimed" "$(owner_of q.txt)" "$S_A"
prestaged_out="$(guard other-session)"
has "guard explains a pre-staged row as already-staged" \
    "$prestaged_out" "already staged when a later command ran"
# THE SAFETY-CRITICAL HALF. Telling a reader an unattributable pre-existing pair is
# "probably your own staging" invites exactly the capture this pair prevents.
hasnt "guard does NOT call a pre-staged row the reader's own staging" \
    "$prestaged_out" "PROBABLY YOUR OWN STAGING"
rm -rf "$T"

# The blanket case must still reach `unnamed` -- the split has to discriminate, not just
# rename. Without this, replacing `unnamed` with `pre-staged` everywhere would pass.
new_repo
mkdir -p sub
echo b > sub/b.txt
CLAUDE_CODE_SESSION_ID="$S_A" git add sub/
eq "a blanket add still records unnamed, not pre-staged" "$(route_of sub/b.txt)" "unnamed"
has "guard still names the blanket form for it" "$(guard other-session)" "blanket add"
rm -rf "$T"

# ---------------------------------------------------------------------------
# 9. OWNERSHIP SURVIVES A TRANSIENTLY EMPTY INDEX
#
# docs/issues/archive/2026-09-02-a-transiently-empty-index-destroys-stage-log-ownership.md
#
# The log was a projection of the CURRENT staged set: the write loop truncated and
# re-emitted one row per staged pair, so any operation that transiently empties the index
# (`git stash`, a reset, a failed pre-commit's stash cycle) wrote an empty log and the
# rows were gone for good. Carry-over could not help -- it reads the file the truncate
# already discarded. The author's own paths then read as `theirs:` under owner `-`.
#
# THE STASHER IS A DIFFERENT SESSION FROM THE STAGER, and that is the load-bearing detail
# of this fixture. With A doing both, a "fix" that simply let the restoring writer claim
# every staged pair would satisfy the owner assertion while destroying the claiming rule
# (§ 2b). Splitting the sessions makes that mutation RED here: it would report B.
S_R=retain-sess-A
S_R_PEER=retain-sess-B

new_repo
echo base > base.txt && git add base.txt && git commit -qm base
echo a > a.txt
echo b > b.txt
CLAUDE_CODE_SESSION_ID="$S_R" git add -- a.txt b.txt
eq "baseline: the stager owns both paths" "$(owner_of a.txt)" "$S_R"

# The empty-index moment itself, not merely the round trip. This is the root cause: if the
# rows do not survive WHILE the index is empty, there is nothing for the restore to find.
CLAUDE_CODE_SESSION_ID="$S_R_PEER" git stash -q --include-untracked
eq "ownership survives while the index is empty" "$(owner_of a.txt)" "$S_R"

CLAUDE_CODE_SESSION_ID="$S_R_PEER" git stash pop -q
eq "ownership survives a stash/pop cycle" "$(owner_of a.txt)" "$S_R"
eq "and for the second path too" "$(owner_of b.txt)" "$S_R"
# The route must survive with the owner. A retained row that came back as `not-staging`
# would still read as unattributable to the guard even with the owner restored.
eq "the route survives with the owner" "$(route_of a.txt)" "named"
has "the stager's own bare commit is not refused" "$(guard "$S_R")" "EXIT=0"
rm -rf "$T"

# Retention must not resurrect a claim the claiming rule refused. A pair that was never
# owned stays unowned across the cycle -- otherwise retention becomes a second, quieter
# route to the cross-claim § 2b exists to prevent.
new_repo
echo base > base.txt && git add base.txt && git commit -qm base
mkdir -p sub
echo c > sub/c.txt
CLAUDE_CODE_SESSION_ID="$S_R" git add sub/
eq "baseline: a blanket add is unowned" "$(owner_of sub/c.txt)" "-"
CLAUDE_CODE_SESSION_ID="$S_R_PEER" git stash -q --include-untracked
CLAUDE_CODE_SESSION_ID="$S_R_PEER" git stash pop -q
eq "retention does not invent an owner for an unowned pair" "$(owner_of sub/c.txt)" "-"
rm -rf "$T"

# Retention is bounded. The log self-limited by tracking only staged pairs; keeping
# unstaged rows removes that limit, so the prune is what replaces it -- and the prune must
# not be "drop what is not staged", which is the bug. Cap is env-overridable SO THAT this
# case can reach it; a test that cannot reach the bound cannot assert one.
new_repo
echo base > base.txt && git add base.txt && git commit -qm base
for i in 1 2 3 4 5 6; do
    echo "$i" > "r$i.txt"
    STAGE_LOG_MAX_RETAINED=3 CLAUDE_CODE_SESSION_ID="$S_R" git add -- "r$i.txt"
    STAGE_LOG_MAX_RETAINED=3 CLAUDE_CODE_SESSION_ID="$S_R" git rm -q --cached "r$i.txt"
done
retained_rows="$(wc -l < .git/session-stage-log)"
[ "$retained_rows" -le 4 ] && [ "$retained_rows" -ge 1 ] \
    && ok "retention honours the cap ($retained_rows rows)" \
    || no "retention honours the cap" "expected 1..4 rows, got $retained_rows"
# Newest-wins, not oldest-wins. A cap that kept the OLDEST retained rows would satisfy the
# bound above while evicting exactly the row a stash/pop is about to ask for.
eq "the cap evicts the oldest, keeping the newest" "$(owner_of r6.txt)" "$S_R"
rm -rf "$T"

# ---------------------------------------------------------------------------
# 10. A TEMPORARY INDEX IS NOT THE SHARED ONE
#
# docs/issues/2026-09-02-a-refused-pathspec-commit-stamps-your-own-content-unowned.md
#
# `git commit -- <paths>` runs hooks with GIT_INDEX_FILE pointing at a temporary
# partial-commit index ($GIT_DIR/next-index-<pid>.lock) holding the pathspec content. Any
# index write inside a pre-commit hook -- which the pre-commit framework performs on every
# run -- fires the recorder, which inherits the variable. Without the guard it reads that
# temp index and writes rows about it into the DURABLE log, stamped `-` because the writer
# is not a staging command. The author is then refused on their own paths and cannot
# reclaim them, since re-adding identical content is not an index write.
#
# NOT keyed on the pre-commit framework: any index write inside any pre-commit hook during
# a pathspec commit reaches this. The hook below is a bare stash cycle for that reason.
S_T=tempidx-sess

new_repo
printf 'base\n' > t.txt
CLAUDE_CODE_SESSION_ID="$S_T" git add -- t.txt
git commit -qm base
cat > .git/hooks/pre-commit <<'PC'
#!/usr/bin/env bash
git stash push -q --keep-index -- . >/dev/null 2>&1
git stash pop -q >/dev/null 2>&1
exit 1
PC
chmod +x .git/hooks/pre-commit
printf 'base\nmine\n' > t.txt
# Cold, so there is no prior owned row for carry-over to preserve -- that is the case the
# damage needs. A pair already staged AND owned survives a refused commit either way.
rm -f .git/session-stage-log
CLAUDE_CODE_SESSION_ID="$S_T" git commit -m x -- t.txt > /dev/null 2>&1
eq "a temp partial-commit index writes no row" "$(owner_of t.txt)" ""
# THE PAIRED POSITIVE. The assertion above is an absence and therefore monotone under
# removal: a recorder deleted outright produces exactly the same silence. This is what
# distinguishes the guard from a dead hook, and it must run in the SAME repo.
CLAUDE_CODE_SESSION_ID="$S_T" git add -- t.txt
eq "and the recorder still claims on the real index" "$(owner_of t.txt)" "$S_T"
rm -rf "$T"

echo
echo "passed=$PASS failed=$FAIL"
[ "$FAIL" = "0" ]
