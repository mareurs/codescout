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
hasnt() { printf '%s' "$2" | grep -qF "$3" && no "$1" "must NOT contain: $3" || ok "$1"; }

# Assert the stage log is THERE, before a case that reads it after deliberately removing
# it. Nothing here recreates that file directly: `post-index-change` fires on index
# WRITES, and `git status` rewrites the index only when it has stat information to
# refresh -- so the recreation is a side effect of an event no case can observe or force.
# When it does not fire, every downstream read is about an absent file, and the failures
# describe the SYMPTOM instead of the unmet PRECONDITION.
#
# Measured 2026-09-18: 4 failures in 45 clean runs (~9%), always the same three cases and
# always together, because they are ONE absent file observed at three points. Read at face
# value they said "your change broke three stage-log cases", which was false, and the
# natural next action was to go debug a working change
# (docs/issues/2026-09-16-three-hooks-discrimination-cases-failed-once-and-did-not-reproduce.md).
#
# This does NOT make the suite deterministic -- the hook still fires or does not. It makes
# the first failure name the reason. It deliberately does not SKIP the cases below: a skip
# shrinks the reported case count on exactly the runs where something went wrong, which is
# a capped result presented as complete.
log_recreated() {
    [ -f .git/session-stage-log ] && ok "$1" || no "$1" \
        "post-index-change did not fire on the preceding git command, so .git/session-stage-log was never recreated. The cases below read an absent file; their failures are downstream of this one, not independent."
}

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
# An ABSENT log must not read like a log with no matching row. Both answered '' until
# 2026-09-18: `awk` fatalled to stderr while the function returned the same empty string a
# legitimate "no row for this path" produces, so the assertion could not tell them apart
# and reported `want '-' got ''`. The sentinel makes the two distinguishable AT THE
# ASSERTION, which is the only place a reader looks.
owner_of() {
    [ -f .git/session-stage-log ] || { printf 'NO-LOG'; return; }
    awk -F'\t' -v p="$1" '$3 == p { print $1; exit }' .git/session-stage-log
}

# Same absent-vs-empty guard as `owner_of`, for the same reason. § 7's cases reach this
# after a `git add`, which writes the index unconditionally, so the non-firing path that
# hit § 2b is not known to reach here: this guards the SHAPE, and is not evidence of a
# second observed failure.
route_of() {
    [ -f .git/session-stage-log ] || { printf 'NO-LOG'; return; }
    awk -F'\t' -v p="$1" '$3 == p { print $4; exit }' .git/session-stage-log
}
# The ABBREVIATED blob `git diff --raw` emits. A full 40-char sha never matches the log,
# so a legacy-row fixture built from `git hash-object` is silently re-derived rather than
# carried over — a case that passes while testing nothing. Cost this suite's author one
# wrong green.
blob_of() {
    git diff --cached --raw |
        awk -F'\t' -v p="$1" '$2 == p { split($1, x, " "); print x[4]; exit }'
}
# A usage.db with only the columns post-index-change-stage-log.sh's `foreign_writer` reads.
# The recorder takes the path from CODESCOUT_USAGE_DB, so no case here touches the real one.
mkdb() {
    rm -f "$1"
    sqlite3 "$1" "CREATE TABLE tool_calls (id INTEGER PRIMARY KEY AUTOINCREMENT, tool_name TEXT, called_at TEXT, outcome TEXT, cc_session_id TEXT, input_json TEXT);"
}
# wrote <db> <sid> <tool> <input_json> [outcome] [called_at]. `called_at` defaults to now,
# in the `YYYY-MM-DD HH:MM:SS.mmm` form the real table uses. No single quote may appear in
# the arguments.
wrote() {
    sqlite3 "$1" "INSERT INTO tool_calls(tool_name, called_at, outcome, cc_session_id, input_json) VALUES ('$3', COALESCE(NULLIF('${6:-}', ''), strftime('%Y-%m-%d %H:%M:%f', 'now')), '${5:-success}', '$2', '$4');"
}

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

# An EMPTY temp index commits nothing, so there is nothing to capture and the guard must be
# silent. RENAMED 2026-09-14: this was "pathspec commit -> silent", which read as a claim
# about pathspec commits in general and was cited as one. It is not — it says nothing about
# a pathspec commit that NAMES a contested path, which is the case below.
has "empty pathspec index -> silent" \
    "$(CLAUDE_CODE_SESSION_ID="$A" GIT_INDEX_FILE=".git/next-index-1.lock" \
        bash "$SRC/pre-commit-foreign-index.sh" 2>&1; echo "EXIT=$?")" "EXIT=0"

# A pathspec commit DOES capture. It commits the named path's WORKING-TREE content, which
# is a peer's whenever a peer is editing that file. The guard exited 0 here until
# 2026-09-14 on the premise that it could not, and the case above is why that went
# unnoticed: an empty index passes whether the guard works or is deleted.
# docs/issues/archive/2026-09-02-a-pathspec-commit-does-capture-staged-content-and-both-guards-stand-down.md
cp .git/index .git/next-index-2.lock
pout="$(CLAUDE_CODE_SESSION_ID="$A" GIT_INDEX_FILE=".git/next-index-2.lock" \
    bash "$SRC/pre-commit-foreign-index.sh" 2>&1; echo "EXIT=$?")"
has "pathspec capturing a peer's path -> refuse" "$pout" "EXIT=1"
has "pathspec refusal names the captured path" "$pout" "b.txt"
# The REMEDY, not only the predicate. The bare form's remedy IS "commit by pathspec";
# printing that to someone whose pathspec commit just failed routes them back into the
# failure, and no assertion about who is refused would catch it.
has "pathspec refusal does not prescribe the refused form" "$pout" "cannot narrow further"
has "pathspec refusal warns against discarding their work" "$pout" "destroys it"

# DISCRIMINATION: the guard must not refuse every pathspec commit. A temp index holding
# only the committer's own path stays silent. Without this, the four assertions above pass
# against a guard that refuses unconditionally.
cp .git/index .git/next-index-3.lock
GIT_INDEX_FILE=".git/next-index-3.lock" git reset -q -- b.txt
has "pathspec naming only my own path -> silent" \
    "$(CLAUDE_CODE_SESSION_ID="$A" GIT_INDEX_FILE=".git/next-index-3.lock" \
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
log_recreated "precondition: peer status recreated the stage log"
eq "cold log + peer status -> unknown, not the passer-by" "$(owner_of s1.txt)" "-"
out="$(guard "$B")"
has "unknown reads as foreign -> refuse" "$out" "EXIT=1"
has "refusal names the staged deletion" "$out" "del.txt"

echo more > s1.txt
CLAUDE_CODE_SESSION_ID="$A" git add s1.txt
eq "a real add still claims normally" "$(owner_of s1.txt)" "$A"

# A staged RENAME must attribute its DESTINATION, not just its source.
#
# `git diff --cached --raw` collapses a rename into ONE row carrying TWO paths when
# detection is on -- git's default since 2.9 -- and the recorder's awk reads `$2`, the
# SOURCE. The pair recorded was then (destination blob, SOURCE path): a pair present
# nowhere in the index, while the destination path got no row at all. The guard looks up
# (blob, path), found no owner for the destination, and fell through to `mine`.
#
# ARCHIVING A BUG FILE IS A RENAME, and it is the commonest one in this repo, so the guard
# was blind at every archive move's destination. It captured one on 2026-09-08.
# docs/issues/archive/2026-09-08-the-stage-log-records-a-renames-source-path-and-drops-its-destination.md
#
# `git mv` and a filesystem move plus `git add -- <old> <new>` produce an IDENTICAL index,
# and the recorder reads the index -- the real capture came via the latter.
echo renameme > r1.txt
CLAUDE_CODE_SESSION_ID="$A" git add r1.txt
CLAUDE_CODE_SESSION_ID="$A" git commit -q -m seed
mkdir -p arch
CLAUDE_CODE_SESSION_ID="$A" git mv r1.txt arch/r1.txt
eq "a staged rename attributes its DESTINATION" "$(owner_of arch/r1.txt)" "$A"
eq "a staged rename attributes its source deletion" "$(owner_of r1.txt)" "$A"

# CONTROL, and it is not decoration: the first assertion above also passes against a
# recorder that emits a row for every field of every raw line. That change would write a
# garbage pair from a two-field line's empty `$3`, so assert the log stays clean for a path
# nobody staged. Without this, "emit more rows" is a passing fix.
eq "a path nobody staged has no row" "$(owner_of never-staged.txt)" ""

# And the LOUD direction. A peer committing over the rename must be refused, naming the
# DESTINATION -- the whole point of recording it. Silence here is the production failure.
out="$(guard "$B")"
has "peer is refused over a staged rename" "$out" "EXIT=1"
has "refusal names the rename DESTINATION" "$out" "arch/r1.txt"

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

# ------------------------- 4b. `-a` / `-i` stage the working tree into index.lock
# git hands an `-a` / `-i` commit's hooks `.git/index.lock`, not `next-index-*`, and until
# 2026-09-25 this guard examined only the latter. So `git commit -a` swept a peer's
# unstaged edit past it with rc=0 (and past foreign-index too, since the stage log never
# records staging into index.lock):
# docs/issues/archive/2026-09-25-git-commit-a-sweeps-a-peers-edit-past-both-ownership-guards.md.
# These cases drive REAL commits through a pre-commit shim instead of handing the guard a
# hand-named index as section 4 does. The defect was the guard not recognising the name
# git actually uses, and a copied `index.lock` would pass whether or not git still used it.
echo "== -a / -i commits (index.lock)"
unreviewed_repo() {
    new_repo
    echo base > a.txt
    echo base > b.txt
    git add a.txt b.txt > /dev/null 2>&1
    git commit -qm base
    # Installed AFTER the base commit, so the base is never judged.
    cat > .git/hooks/pre-commit <<SHIM
#!/usr/bin/env bash
exec bash "$SRC/pre-commit-unreviewed-content.sh"
SHIM
    chmod +x .git/hooks/pre-commit
}

unreviewed_repo
echo mine > a.txt
# A peer's edit, never staged: the load-bearing detail. Staged, it would be foreign-index's case.
echo "THEIR LINE" > b.txt
out="$(git commit -a -qm sweep 2>&1; echo "EXIT=$?")"
has "-a sweeping a peer's unstaged file -> refuse" "$out" "EXIT=1"
has "-a refusal names the swept file" "$out" "    b.txt"
eq "-a refused: nothing reached HEAD" "$(git log -1 --format=%s)" "base"
# The pathspec branch prints `git add <the list>`; here that line would stage the peer's file.
hasnt "-a remedy does not tell you to stage the swept list" "$out" "git add a.txt b.txt"
has "-a remedy warns the list can hold a peer's files" "$out" "can include a PEER'S files"
rm -rf "$T"

# The over-refusal control. Every change is already staged, so index.lock and .git/index
# agree on every blob and there is nothing unreviewed. A guard that refused every
# index.lock commit would pass the case above and fail this one.
unreviewed_repo
echo mine > a.txt
git add a.txt
out="$(git commit -a -qm staged-first 2>&1; echo "EXIT=$?")"
has "-a over fully-staged changes -> allowed" "$out" "EXIT=0"
eq "-a over fully-staged changes: committed" "$(git log -1 --format=%s)" "staged-first"
rm -rf "$T"

# `-i <path>` takes the same index.lock route for the path it names.
unreviewed_repo
echo mine > a.txt
git add a.txt
echo "mine + THEIR LINE" > a.txt
out="$(git commit -i a.txt -qm include 2>&1; echo "EXIT=$?")"
has "-i over a file that moved after staging -> refuse" "$out" "EXIT=1"
has "-i refusal names the file" "$out" "    a.txt"
rm -rf "$T"

# The widening must stop at the two temporary indexes. Two cases, because they guard
# different things (measured 2026-09-25 by mutating `*) exit 0` to examine everything):
#  - A bare commit hands the hook `.git/index` ITSELF, and comparing an index with itself
#    finds nothing, so this case holds whatever the `*)` arm does. It guards BEHAVIOUR: a
#    peer's unstaged edit elsewhere is not in a bare commit and must not be refused as if
#    it were. It does NOT guard the arm. That mutation survives it.
#  - With GIT_INDEX_FILE UNSET (a direct call, and this suite's own model of a bare commit
#    for foreign-index), the `*)` arm is all that stands between the guard and an empty
#    index, where every tracked path reads as unreviewed. That mutation dies here only.
unreviewed_repo
echo mine > a.txt
git add a.txt
echo "THEIR LINE" > b.txt
out="$(git commit -qm bare 2>&1; echo "EXIT=$?")"
has "bare commit beside a peer's unstaged edit -> allowed" "$out" "EXIT=0"
eq "bare commit took only the index" "$(git show --name-only --format= HEAD)" "a.txt"
out="$(env -u GIT_INDEX_FILE bash "$SRC/pre-commit-unreviewed-content.sh" 2>&1; echo "EXIT=$?")"
has "no GIT_INDEX_FILE -> silent, not an empty-index refusal" "$out" "EXIT=0"
rm -rf "$T"

# ------------------------------- 5. the JOINT predicate and CODESCOUT_INDEX_ACK
# docs/issues/archive/2026-09-16-archiving-a-peers-bug-file-refuses-both-parties-from-opposite-sides.md
#
# BACKFILL. `b37b888a` shipped the `joint` predicate and the ack arm with NO coverage at
# all -- `grep -rn "CODESCOUT_INDEX_ACK\|joint" tests/` returned nothing -- and was reported
# as "101 passed", which was true of the suite and vacuous for the branch it added. These
# cases pin the behaviour AS IT SHIPPED, before any widening, so a later change to the
# admitting side has a baseline to red against rather than tests written alongside it.
#
# The fixture splits ONE rename across two sessions, which is the shape `joint` exists for
# and which `git mv` cannot produce on its own (it stages both halves under one id). B
# removes the source from the index; A adds the destination. `git diff --cached
# --name-status -M` still reports `R100 r1.txt arch/r1.txt`, so the pair is recoverable
# even though its halves have different owners -- which is exactly what the predicate reads.
echo "== joint archive + index ack"

# The ack arm needs an env var `guard()` does not pass, so this is its sibling rather than
# a change to it: same unset of GIT_INDEX_FILE, same stderr merge, same EXIT= marker.
guard_ack() {
    env -u GIT_INDEX_FILE CLAUDE_CODE_SESSION_ID="$1" CODESCOUT_INDEX_ACK="$2" \
        bash "$SRC/pre-commit-foreign-index.sh" 2>&1
    echo "EXIT=$?"
}

new_repo
echo r1 > r1.txt
git add r1.txt > /dev/null 2>&1
git commit -qm base
mkdir -p arch
CLAUDE_CODE_SESSION_ID="$B" git rm -q --cached r1.txt
mv r1.txt arch/r1.txt
CLAUDE_CODE_SESSION_ID="$A" git add arch/r1.txt

# The fixture's own premise, asserted before anything reads the guard. Without this a
# mis-built fixture makes every case below pass for the wrong reason -- § 7's rule.
eq "joint fixture: source is FOREIGN"     "$(owner_of r1.txt)"      "$B"
eq "joint fixture: destination is OURS"   "$(owner_of arch/r1.txt)" "$A"

jout="$(guard "$A")"
has "split rename -> refuses"                  "$jout" "EXIT=1"
has "refusal names the JOINT ARCHIVE shape"    "$jout" "JOINT ARCHIVE"
has "joint refusal names the foreign half"     "$jout" "r1.txt"
has "joint refusal prefills the ack"                "$jout" "CODESCOUT_INDEX_ACK"

# The pathspec arm is a DIFFERENT branch of the same refusal and carries the remedy
# correction that matters. Written as its own case because the bare fixture above cannot
# reach it: `guard()` unsets GIT_INDEX_FILE by design, so `pathspec` is 0 there and an
# assertion on this text passes or fails for reasons unrelated to `joint`. Found by
# asserting it against the bare case first and watching it red.
cp .git/index .git/next-index-5.lock
jpout="$(env -u CODESCOUT_INDEX_ACK CLAUDE_CODE_SESSION_ID="$A" \
    GIT_INDEX_FILE=".git/next-index-5.lock" \
    bash "$SRC/pre-commit-foreign-index.sh" 2>&1; echo "EXIT=$?")"
has "joint pathspec commit -> refuses"              "$jpout" "EXIT=1"
# The generic pathspec remedy is "ask the owner to commit theirs". For a joint archive that
# owner is refused by this same guard over the other half, so printing it unqualified sends
# the reader back into the refusal they arrived from. This is the remedy half a predicate
# assertion cannot reach.
has "joint refusal disowns the ask-the-owner route" "$jpout" "DOES NOT APPLY HERE"
rm -f .git/next-index-5.lock

# The admitting side. Four inputs, one accepted -- and the three refusals are what make the
# acceptance mean anything.
has "ack naming the foreign owner -> passes"   "$(guard_ack "$A" "$B")" "EXIT=0"
has "accepted ack says so on stderr"           "$(guard_ack "$A" "$B")" "note: CODESCOUT_INDEX_ACK"
has "accepted ack prints the trailer to record" "$(guard_ack "$A" "$B")" "Co-Authored-Session-Id"
has "ack naming the WRONG sid still refuses"   "$(guard_ack "$A" "cccccccc-2222-2222-2222-cccccccccccc")" "EXIT=1"
has "an empty ack still refuses"               "$(guard_ack "$A" "")" "EXIT=1"
# There is deliberately no wildcard at this gate. The sibling pre-push guard HAS one, and
# its own history argues against it: an `all` form answers "is this sid authorised?" and
# accumulates nothing for "who is owed a notification?", so it publishes every foreign
# session's work and tells none of them. Pinned here so the absence is a decision rather
# than an omission somebody later "fixes".
has "the literal 'all' is NOT a wildcard here" "$(guard_ack "$A" "all")" "EXIT=1"
rm -rf "$T"

# Two foreign owners, one named. The ack loop requires EVERY foreign owner, so a partial
# list must refuse -- the case that separates "names an owner" from "names them all", and
# the one a short-circuit on first match would pass.
new_repo
echo r1 > r1.txt
echo r2 > r2.txt
git add r1.txt r2.txt > /dev/null 2>&1
git commit -qm base
mkdir -p arch
C2="cccccccc-2222-2222-2222-cccccccccccc"
CLAUDE_CODE_SESSION_ID="$B"  git rm -q --cached r1.txt
CLAUDE_CODE_SESSION_ID="$C2" git rm -q --cached r2.txt
mv r1.txt arch/r1.txt
mv r2.txt arch/r2.txt
CLAUDE_CODE_SESSION_ID="$A" git add arch/r1.txt arch/r2.txt
eq "two-owner fixture: first source is B"  "$(owner_of r1.txt)" "$B"
eq "two-owner fixture: second source is C" "$(owner_of r2.txt)" "$C2"
has "ack naming only ONE of two owners refuses" "$(guard_ack "$A" "$B")"       "EXIT=1"
has "ack naming BOTH owners passes"             "$(guard_ack "$A" "$B,$C2")"   "EXIT=0"
rm -rf "$T"

# `-` IS NOT A PARTY, and the ack must not accept it as one.
#
# post-index-change-stage-log.sh records `-` for a pair it could not attribute (:395, :401)
# -- four routes reach it, and its own header calls such a row "frequently a PEER's". The
# ack test is plain string membership on ",$index_ack," (:284), so `CODESCOUT_INDEX_ACK="-"`
# satisfies it: one character, naming nobody, covering an unbounded number of unattributable
# pairs. That is a functional wildcard over exactly the rows :278-279 refuses to give a
# wildcard to -- "Deliberately NO `all` form".
#
# Reproduced before this case was written: EXIT=1 with no ack, EXIT=0 with `-`.
new_repo
echo r1 > r1.txt
git add r1.txt > /dev/null 2>&1
git commit -qm base
mkdir -p arch
env -u CLAUDE_CODE_SESSION_ID git rm -q --cached r1.txt
mv r1.txt arch/r1.txt
CLAUDE_CODE_SESSION_ID="$A" git add arch/r1.txt
eq "dash fixture: the source is UNATTRIBUTED" "$(owner_of r1.txt)" "-"
has "an unattributed half refuses without an ack" "$(guard "$A")"        "EXIT=1"
has "ack of '-' does NOT clear an unattributed half" "$(guard_ack "$A" "-")" "EXIT=1"
# And the refusal has to say WHY, or the caller retries the same string. Asserted as an
# ALL-CAPS role token per tests/pre-push-foreign-session-guard.sh:156-157, so a rewrite of
# the surrounding prose does not red and a deletion of the explanation does.
has "refusal explains the UNATTRIBUTED half" "$(guard_ack "$A" "-")" "UNATTRIBUTED"
rm -rf "$T"

# ---- the ALL-CONTESTED route: every path YOU NAMED is someone else's ----
#
# This is the shape that deadlocked on 2026-09-16. `joint` cannot reach it: `joint` requires
# each contested path's partner to be in `mine`, and here `mine` is EMPTY, so the two
# predicates are provably disjoint rather than overlapping. That disjointness is why a green
# case is owed on BOTH sides of the `||` -- a suite green on only one side cannot tell `||`
# from `&&`, and `&&` would make the whole ack arm globally inert.
pguard_ack() {
    cp .git/index ".git/next-index-$3.lock"
    env -u GIT_INDEX_FILE 2>/dev/null; :
    CLAUDE_CODE_SESSION_ID="$1" CODESCOUT_INDEX_ACK="$2" \
        GIT_INDEX_FILE=".git/next-index-$3.lock" \
        bash "$SRC/pre-commit-foreign-index.sh" 2>&1
    echo "EXIT=$?"
    rm -f ".git/next-index-$3.lock"
}

new_repo
echo base > a.txt
echo base > c.txt
git add -A > /dev/null 2>&1
git commit -qm base
echo theirs > a.txt
CLAUDE_CODE_SESSION_ID="$B" git add a.txt
eq "all-contested fixture: the named path is FOREIGN" "$(owner_of a.txt)" "$B"

has "pathspec naming only foreign paths + correct ack -> passes" \
    "$(pguard_ack "$A" "$B" 41)" "EXIT=0"
has "pathspec all-contested + NO ack still refuses" \
    "$(pguard_ack "$A" "" 42)" "EXIT=1"
has "pathspec all-contested + WRONG sid still refuses" \
    "$(pguard_ack "$A" "cccccccc-2222-2222-2222-cccccccccccc" 43)" "EXIT=1"
# The comma anchors in the membership test. Without them a sid that is a PREFIX of the
# real owner satisfies it, which is a silent widening no other case reaches.
has "a sid that is a PREFIX of the owner does not satisfy the ack" \
    "$(pguard_ack "$A" "bbbbbbbb" 44)" "EXIT=1"

# THE BARE FORM IS NOT WIDENED, and this is one of the two cases carrying the design.
# A bare commit takes the ENTIRE shared index rather than a set the committer named, so
# `mine` empty there means "every path any peer staged is foreign" -- the whole-index sweep
# this guard exists for, not a corner with no compliant route. Deleting the `pathspec`
# conjunct is the cheapest wrong simplification and this is its only killer.
has "BARE commit, all foreign, WITH an ack -> still refuses" "$(guard_ack "$A" "$B")" "EXIT=1"
rm -rf "$T"

# The second design-carrying case. `mine` non-empty means a compliant route still exists --
# the guard prints `git commit -- <mine>` -- so the ack must not fire. Dropping the
# mine-empty test makes every acked commit pass, and nothing else here would notice.
new_repo
echo base > a.txt
echo base > c.txt
git add -A > /dev/null 2>&1
git commit -qm base
echo theirs > a.txt
echo mine > c.txt
CLAUDE_CODE_SESSION_ID="$B" git add a.txt
CLAUDE_CODE_SESSION_ID="$A" git add c.txt
eq "mixed fixture: a.txt is FOREIGN" "$(owner_of a.txt)" "$B"
eq "mixed fixture: c.txt is OURS"    "$(owner_of c.txt)" "$A"
mixout="$(pguard_ack "$A" "$B" 45)"
has "pathspec with one path of MINE + an ack -> still refuses" "$mixout" "EXIT=1"
has "and it still offers the narrowing remedy"                 "$mixout" "git commit -- c.txt"
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
echo "== named route attributes a path by WRITE when the record shows a peer wrote it"
# docs/issues/2026-09-07-the-stage-log-records-the-stager-so-git-add--A-makes-you-the-owner.md
# `git add <peer's untracked file>` used to record the stager as its owner (route `named`),
# and the guard then had nothing to refuse. The recorder now asks usage.db whether another
# session wrote the path through a codescout tool since its last commit, and if the stager
# did not, records THAT session with route `named-foreign`.
#
# Every "kept as named" case below is an input the legacy rule already handled, so each one
# passes on unchanged code. Their evidence is the mutations that make them fail, not this
# run; the cases that go RED on unchanged code are 1, 4b, 5a, 7, 10 and 15.
C="cccccccc-2222-2222-2222-cccccccccccc"
add_as() { CODESCOUT_USAGE_DB="$DB" CLAUDE_CODE_SESSION_ID="$1" git add -- "${@:2}"; }
DB="$(mktemp -u "${TMPDIR:-/tmp}/usage-XXXXXX.db")"

# 1. the reproduced bug, and the guard's side of it.
new_repo; mkdb "$DB"
echo "peer is mid-write" > f.txt
wrote "$DB" "$B" edit_file '{"action":"edit","path":"f.txt","new_string":"x"}'
add_as "$A" f.txt
eq "1: staging a path a peer wrote records the PEER as owner" "$(owner_of f.txt)" "$B"
eq "1: and says why" "$(route_of f.txt)" "named-foreign"
out="$(guard "$A")"
has "1: the stager's bare commit is refused" "$out" "EXIT=1"
has "1: the refusal names the writer" "$out" "$B"
has "1: and says the attribution is by write, not by staging" "$out" "ATTRIBUTED BY WRITE"
# What the refusal sends the stager to do, and what it says was examined: a stager whose staged diff is
# their own change has to be told the row can be stale and how to ask again, and a reader has to be told
# the window and the closing rule the lookup used, not left to assume "ever".
has "1: names what the lookup examined" "$out" "WHAT WAS EXAMINED"
has "1: and how a stale row is asked again" "$out" "asks the record again"
has "1: and that re-adding a pair still staged does not" "$out" "still staged"
has "1: the writer may commit it" "$(guard "$B")" "EXIT=0"
# 15. the row is a fact about the pair and survives an unrelated stage.
echo o > other.txt
add_as "$A" other.txt
eq "15: a later unrelated add leaves the foreign row's owner" "$(owner_of f.txt)" "$B"
eq "15: and its route" "$(route_of f.txt)" "named-foreign"
rm -rf "$T"

# 2. the stager wrote it too: a mixed file is not refused, which is the entry-grain case this
# lookup cannot separate and must not over-refuse.
new_repo; mkdb "$DB"
echo mixed > f.txt
wrote "$DB" "$B" edit_file '{"action":"edit","path":"f.txt"}'
wrote "$DB" "$A" edit_file '{"action":"edit","path":"f.txt"}'
add_as "$A" f.txt
eq "2: a path BOTH sessions wrote stays the stager's" "$(owner_of f.txt)" "$A"
eq "2: on the legacy route" "$(route_of f.txt)" "named"
rm -rf "$T"

# 3. control: no record at all is the old behaviour exactly.
new_repo; mkdb "$DB"
echo mine > f.txt
add_as "$A" f.txt
eq "3: no write on record keeps the legacy claim" "$(owner_of f.txt)" "$A"
eq "3: and the legacy route" "$(route_of f.txt)" "named"
rm -rf "$T"

# 4. liveness: a write is live until its OWN writer commits the path after it. Rows carry
# explicit times so no case depends on two events landing in different seconds.
hrs() { date -u -d "$1" '+%Y-%m-%d %H:%M:%S.000'; }
giso() { date -u -d "$1" '+%Y-%m-%dT%H:%M:%SZ'; }
commit_as() { # commit_as <sid> <path> [committer date]: a commit whose Session-Id trailer names <sid>
    git add -- "$2" && env ${3:+"GIT_COMMITTER_DATE=$3"} git commit -q -m "touch $2" -m "Session-Id: $1"
}
new_repo; mkdb "$DB"
echo v1 > g.txt; git add g.txt; git commit -qm g
echo v2 > g.txt
wrote "$DB" "$B" edit_file '{"action":"edit","path":"g.txt"}' success "$(hrs '1 hour ago')"
commit_as "$B" g.txt
echo v3 > g.txt
add_as "$A" g.txt
eq "4a: a peer's write is not live once the PEER has committed the path since" "$(owner_of g.txt)" "$A"
git reset -q -- g.txt
wrote "$DB" "$B" edit_file '{"action":"edit","path":"g.txt"}' success "$(hrs '1 hour')"
add_as "$A" g.txt
eq "4b: the same peer writing AFTER that commit is live again" "$(owner_of g.txt)" "$B"
rm -rf "$T"

# 4c. the case the old commit-time bound got wrong, from a measured tracker: B wrote, then a
# DIFFERENT session committed the file from a blob that predates B's write, so B's lines are
# still uncommitted. Someone else's commit must not clear B.
new_repo; mkdb "$DB"
echo v1 > g.txt; git add g.txt; git commit -qm g
echo v2 > g.txt
wrote "$DB" "$B" edit_file '{"action":"edit","path":"g.txt"}' success "$(hrs '1 hour ago')"
echo v1-and-more > g.txt
commit_as "$C" g.txt
echo v3 > g.txt
add_as "$A" g.txt
eq "4c: a third session's commit does not clear a peer's still-uncommitted write" "$(owner_of g.txt)" "$B"
rm -rf "$T"

# 4d. the stager's own earlier write, since committed, is no claim on the file: staging the
# path again because a PEER has since written it must name the peer.
new_repo; mkdb "$DB"
echo v1 > g.txt; git add g.txt; git commit -qm g
echo v2 > g.txt
wrote "$DB" "$A" edit_file '{"action":"edit","path":"g.txt"}' success "$(hrs '2 hours ago')"
commit_as "$A" g.txt
echo v3 > g.txt
wrote "$DB" "$B" edit_file '{"action":"edit","path":"g.txt"}' success "$(hrs '1 hour')"
add_as "$A" g.txt
eq "4d: the stager's own write, already committed by the stager, is not live" "$(owner_of g.txt)" "$B"
rm -rf "$T"

# 4e. the lookback: a write older than three days is out of reach, however uncommitted.
new_repo; mkdb "$DB"
echo f > f.txt
wrote "$DB" "$B" edit_file '{"action":"edit","path":"f.txt"}' success "$(hrs '4 days ago')"
add_as "$A" f.txt
eq "4e: a write older than the lookback is not seen" "$(owner_of f.txt)" "$A"
rm -rf "$T"

# 4f. a session that commits the path more than once is judged by its NEWEST commit: B commits,
# then writes, then commits again, so the write is cleared by the second commit, not kept live
# by the first.
new_repo; mkdb "$DB"
echo v1 > g.txt; git add g.txt; git commit -qm g
echo v2 > g.txt; commit_as "$B" g.txt "$(giso '3 hours ago')"
wrote "$DB" "$B" edit_file '{"action":"edit","path":"g.txt"}' success "$(hrs '2 hours ago')"
echo v3 > g.txt; commit_as "$B" g.txt "$(giso '1 hour ago')"
echo v4 > g.txt
add_as "$A" g.txt
eq "4f: a write is cleared by its writer's newest commit, not held live by an older one" "$(owner_of g.txt)" "$A"
rm -rf "$T"

# 4g. one commit may name several sessions, and each is taken as having committed.
new_repo; mkdb "$DB"
echo v1 > g.txt; git add g.txt; git commit -qm g
echo v2 > g.txt
wrote "$DB" "$B" edit_file '{"action":"edit","path":"g.txt"}' success "$(hrs '1 hour ago')"
git add g.txt && git commit -q -m "joint" --trailer "Session-Id: $C" --trailer "Session-Id: $B"
echo v3 > g.txt
add_as "$A" g.txt
eq "4g: a commit naming two sessions clears both" "$(owner_of g.txt)" "$A"

# 5. doc writes carry the artifact id (first 16 hex of sha256 of the absolute path).
new_repo; mkdb "$DB"
echo h > h.txt; echo i > i.txt
hid="$(printf '%s' "$(git rev-parse --show-toplevel)/h.txt" | sha256sum | cut -c1-16)"
iid="$(printf '%s' "$(git rev-parse --show-toplevel)/i.txt" | sha256sum | cut -c1-16)"
wrote "$DB" "$B" doc "{\"action\":\"update\",\"id\":\"$hid\",\"patch\":{}}"
wrote "$DB" "$B" doc "{\"action\":\"find\",\"id\":\"$iid\"}"
add_as "$A" h.txt i.txt
eq "5a: a doc update by id is a write to that path" "$(owner_of h.txt)" "$B"
eq "5b: a doc READ of an id is not" "$(owner_of i.txt)" "$A"
rm -rf "$T"

# 6. a refused or failed call wrote nothing.
new_repo; mkdb "$DB"
echo f > f.txt
wrote "$DB" "$B" edit_file '{"action":"edit","path":"f.txt"}' recoverable_error
add_as "$A" f.txt
eq "6: a write that did not succeed is ignored" "$(owner_of f.txt)" "$A"
rm -rf "$T"

# 7. two other writers: the most recent one is named.
new_repo; mkdb "$DB"
echo f > f.txt
wrote "$DB" "$B" edit_file '{"action":"edit","path":"f.txt"}'
wrote "$DB" "$C" edit_file '{"action":"edit","path":"f.txt"}'
add_as "$A" f.txt
eq "7: with two other writers the newest is the owner" "$(owner_of f.txt)" "$C"
rm -rf "$T"

# 8-9. the match is exact: `_` is not a wildcard and a longer path is not this one.
new_repo; mkdb "$DB"
echo a > axb.txt; echo k > k.txt
wrote "$DB" "$B" edit_file '{"action":"edit","path":"a_b.txt"}'
wrote "$DB" "$B" edit_file '{"action":"edit","path":"k.txt.bak"}'
add_as "$A" axb.txt k.txt
eq "8: a_b.txt (underscore) does not match axb.txt" "$(owner_of axb.txt)" "$A"
eq "9: k.txt.bak does not match k.txt" "$(owner_of k.txt)" "$A"
rm -rf "$T"

# 10. an absolute `path` argument names the same file.
new_repo; mkdb "$DB"
echo f > f.txt
wrote "$DB" "$B" edit_file "{\"action\":\"edit\",\"path\":\"$(git rev-parse --show-toplevel)/f.txt\"}"
add_as "$A" f.txt
eq "10: an absolute path argument matches" "$(owner_of f.txt)" "$B"
rm -rf "$T"

# 11. every failure to read the record falls back to the legacy claim.
new_repo
echo f > f.txt
CODESCOUT_USAGE_DB=/nonexistent/usage.db CLAUDE_CODE_SESSION_ID="$A" git add -- f.txt
eq "11a: an absent database keeps the legacy claim" "$(owner_of f.txt)" "$A"
git reset -q -- f.txt
echo "this is not a database" > "$DB"
add_as "$A" f.txt
eq "11b: an unreadable database keeps the legacy claim" "$(owner_of f.txt)" "$A"
rm -rf "$T"

# 12. a blanket add never consults the record: it names no path to ask about.
new_repo; mkdb "$DB"
echo f > f.txt
wrote "$DB" "$B" edit_file '{"action":"edit","path":"f.txt"}'
CODESCOUT_USAGE_DB="$DB" CLAUDE_CODE_SESSION_ID="$A" git add .
eq "12: a blanket add still records unnamed" "$(route_of f.txt)" "unnamed"
rm -rf "$T"

# 13. only writing tools count.
new_repo; mkdb "$DB"
echo f > f.txt
wrote "$DB" "$B" read_file '{"path":"f.txt"}'
add_as "$A" f.txt
eq "13: a peer's READ of the path is not a write" "$(owner_of f.txt)" "$A"
rm -rf "$T"

# 14. the lookback is bounded: a write more than 50000 calls ago is out of reach, so the
# lookup stays cheap on a table that only grows.
new_repo; mkdb "$DB"
echo f > f.txt
wrote "$DB" "$B" edit_file '{"action":"edit","path":"f.txt"}'
sqlite3 "$DB" "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<50001) INSERT INTO tool_calls(tool_name, called_at, outcome, cc_session_id, input_json) SELECT 'grep', strftime('%Y-%m-%d %H:%M:%f','now'), 'success', 'zz', '{}' FROM n;"
add_as "$A" f.txt
eq "14: a write older than the lookback window is not seen" "$(owner_of f.txt)" "$A"
rm -rf "$T" "$DB"

# 16-17. every writing tool counts, one case each: a tool dropped from the list is a hole the
# edit_file cases above cannot see.
new_repo; mkdb "$DB"
echo c > c.txt; echo d > d.txt
wrote "$DB" "$B" create_file '{"path":"c.txt","content":"x"}'
wrote "$DB" "$B" edit_code '{"action":"replace","path":"d.txt","symbol":"s","body":"x"}'
add_as "$A" c.txt d.txt
eq "16: a create_file by a peer is a write" "$(owner_of c.txt)" "$B"
eq "17: an edit_code by a peer is a write" "$(owner_of d.txt)" "$B"
rm -rf "$T" "$DB"

# 5c. every id-bearing doc action that writes, one case each (5a covers `update`).
new_repo; mkdb "$DB"
for act in append_entry update_entry move delete; do
    echo x > "doc-$act.txt"
    did="$(printf '%s' "$(git rev-parse --show-toplevel)/doc-$act.txt" | sha256sum | cut -c1-16)"
    wrote "$DB" "$B" doc "{\"action\":\"$act\",\"id\":\"$did\"}"
done
add_as "$A" doc-append_entry.txt doc-update_entry.txt doc-move.txt doc-delete.txt
for act in append_entry update_entry move delete; do
    eq "5c: doc $act by id is a write to that path" "$(owner_of "doc-$act.txt")" "$B"
done
rm -rf "$T" "$DB"

# 18. a database another process holds locked must not stall or fail the add: the answer is
# a refinement of `named`, so being unable to refine returns the old claim.
new_repo; mkdb "$DB"
echo f > f.txt
wrote "$DB" "$B" edit_file '{"action":"edit","path":"f.txt"}'
{ echo "BEGIN EXCLUSIVE;"; sleep 2; } | sqlite3 "$DB" >/dev/null 2>&1 &
sleep 0.5
_t0=$SECONDS
add_as "$A" f.txt
_took=$((SECONDS - _t0))
wait
eq "18: a locked database keeps the legacy claim" "$(owner_of f.txt)" "$A"
[ "$_took" -le 1 ] && ok "18: and the add did not wait on the lock" || no "18: and the add did not wait on the lock" "took ${_took}s"
rm -rf "$T" "$DB"

# 19. a path quoted inside an edit's TEXT is not a write to that path. The record is read as JSON by
# its top-level `path`, so a peer editing another file whose body mentions "path":"f.txt" (as a
# bug file about this lookup does) is not named as f.txt's writer.
new_repo; mkdb "$DB"
echo f > f.txt
wrote "$DB" "$B" edit_file '{"action":"edit","path":"other.txt","new_string":"{\"path\":\"f.txt\"}"}'
add_as "$A" f.txt
eq "19: a path quoted in another file's edit text is not a write to it" "$(owner_of f.txt)" "$A"
rm -rf "$T" "$DB"

# 20. one malformed row must not blind the lookup to the valid ones: SQLite raises on the first
# malformed JSON it evaluates, which would fail the whole query and silently fall back to the
# legacy claim for every path. The malformed row is NEWER than the valid one on purpose: rows are
# read newest first, so the error arrives before any row has been printed. With the valid row
# newer the CLI prints it first and the error costs nothing, which is how this case once passed
# under the very mutation it exists to kill.
new_repo; mkdb "$DB"
echo f > f.txt
wrote "$DB" "$B" edit_file '{"action":"edit","path":"f.txt"}'
wrote "$DB" "$C" edit_file 'this is not json'
add_as "$A" f.txt
eq "20: a malformed row beside a valid one does not hide the valid one" "$(owner_of f.txt)" "$B"
rm -rf "$T" "$DB"

# 21. a doc move writes its DESTINATION, whose own id appears in no call: the id a move carries
# is the source's, so only `new_rel_path` names the file. A move to another path is not a write
# to this one.
new_repo; mkdb "$DB"
mkdir -p archive; echo moved > archive/m.txt; echo other > archive/n.txt
oid="$(printf '%s' "$(git rev-parse --show-toplevel)/m.txt" | sha256sum | cut -c1-16)"
wrote "$DB" "$B" doc "{\"action\":\"move\",\"id\":\"$oid\",\"new_rel_path\":\"archive/m.txt\"}"
add_as "$A" archive/m.txt archive/n.txt
eq "21a: a doc move's destination is attributed to the mover" "$(owner_of archive/m.txt)" "$B"
eq "21b: a path the move did not name is not" "$(owner_of archive/n.txt)" "$A"
rm -rf "$T" "$DB"

# 22. usage.db files a SUBAGENT's calls under `<parent session id>/<agent id>` (a third of the rows of
# a three-day window, measured 2026-10-01), while the stager's CLAUDE_CODE_SESSION_ID and every
# commit's Session-Id trailer name the PARENT alone. Read whole, the composite never equals the
# stager, so a session's own subagent's file was named as a PEER's (and its owner column held a
# string nobody can be asked), and never matched a trailer, so its writer's own commit could not
# clear it. The agent suffix is stripped on read; the parent is the only party that can answer or commit.
# Red on unchanged code: 22a's owner, 22b (both assertions), 22c, 22d. 22a's ROUTE assertion passes on
# unchanged code and rests on mutation.
AG="a0b1c2d3e4f5a6b7c"
new_repo; mkdb "$DB"
echo f > f.txt
wrote "$DB" "$B/$AG" edit_file '{"action":"edit","path":"f.txt"}'
add_as "$A" f.txt
eq "22a: a peer's SUBAGENT write names the peer's session, not the agent" "$(owner_of f.txt)" "$B"
eq "22a: on the by-write route" "$(route_of f.txt)" "named-foreign"
rm -rf "$T"

new_repo; mkdb "$DB"
echo f > f.txt
wrote "$DB" "$A/$AG" edit_file '{"action":"edit","path":"f.txt"}'
add_as "$A" f.txt
eq "22b: the stager's OWN subagent wrote it: the stager's, not a peer's" "$(owner_of f.txt)" "$A"
eq "22b: on the legacy route" "$(route_of f.txt)" "named"
rm -rf "$T"

# 22c. the parent's commit clears its subagent's write, as it clears its own: the trailer names
# the parent. Mirrors 4a with the writer filed the way the real table files it.
new_repo; mkdb "$DB"
echo v1 > g.txt; git add g.txt; git commit -qm g
echo v2 > g.txt
wrote "$DB" "$B/$AG" edit_file '{"action":"edit","path":"g.txt"}' success "$(hrs '1 hour ago')"
commit_as "$B" g.txt
echo v3 > g.txt
add_as "$A" g.txt
eq "22c: the parent's commit clears its subagent's earlier write" "$(owner_of g.txt)" "$A"
rm -rf "$T"

# 22d. a peer's parent and subagent both wrote it: one writer, named once and by its parent.
new_repo; mkdb "$DB"
echo f > f.txt
wrote "$DB" "$B" edit_file '{"action":"edit","path":"f.txt"}'
wrote "$DB" "$B/$AG" edit_file '{"action":"edit","path":"f.txt"}'
add_as "$A" f.txt
eq "22d: a peer and its subagent are one writer, named by the peer's session" "$(owner_of f.txt)" "$B"
rm -rf "$T" "$DB"

# 23. `doc(create)` writes a NEW file and addresses it by `rel_path`: the artifact has no id until the
# call returns, so the id clause cannot see it, and the bug files and trackers sessions file are
# mostly created this way. A `doc` READ may carry the same key (`find` takes `rel_path` as a
# shorthand), so only the `create` action counts. Red on unchanged code: 23a. 23b and 23c pass on
# unchanged code, so their evidence is the mutations that make them fail.
new_repo; mkdb "$DB"
echo c > c.txt; echo d > d.txt; echo e > e.txt
wrote "$DB" "$B" doc '{"action":"create","kind":"bug","rel_path":"c.txt","title":"t"}'
wrote "$DB" "$B" doc '{"action":"create","kind":"bug","rel_path":"elsewhere.txt","title":"t"}'
wrote "$DB" "$B" doc '{"action":"find","rel_path":"e.txt"}'
add_as "$A" c.txt d.txt e.txt
eq "23a: a doc create of this path is a write to it" "$(owner_of c.txt)" "$B"
eq "23b: a create of ANOTHER path is not" "$(owner_of d.txt)" "$A"
eq "23c: a doc READ carrying rel_path is not" "$(owner_of e.txt)" "$A"
rm -rf "$T" "$DB"

# 24. A write is closed when its TEXT is already committed, whoever committed it. The liveness rule of
# case 4 closes a write only when its own writer commits the path, which never happens when a DIFFERENT
# session's commit sweeps the writer's work (a session that ends before committing, a "commit all"):
# the write stayed live for the whole lookback and named a writer whose work was in HEAD. Measured on
# the live ledger 2026-10-01: four entries by one session, committed under another's trailer, named
# that session as the owner of the next stager's two-line change. The opposite direction is case 4c's:
# a commit by anyone is NOT proof the write was taken, since it can carry an older staged blob, so the
# test is the write's own new text, found in HEAD and not already in the version before the write.
LINE="a distinctive sentence the writer put in, longer than sixteen characters"
BUL="- **Status:** a bullet shaped line that begins with a dash and a star"
old() { giso '3 hours ago'; }
# swept_case <tool> <json> <swept|pending>: B wrote <json> an hour ago; in `swept` C then commits the file
# carrying B's text, in `pending` nobody has; then A edits and stages the path by name.
swept_case() {
    new_repo; mkdb "$DB"
    gid="$(printf '%s' "$(git rev-parse --show-toplevel)/g.txt" | sha256sum | cut -c1-16)"
    echo base > g.txt; git add g.txt; GIT_COMMITTER_DATE="$(old)" git commit -qm base
    printf '%s\n%s\n' "$LINE" "$BUL" >> g.txt
    wrote "$DB" "$B" "$1" "${2//GID/$gid}" success "$(hrs '1 hour ago')"
    [ "$3" = swept ] && commit_as "$C" g.txt
    echo mine >> g.txt
    add_as "$A" g.txt
}
# One swept/pending pair per place the writer's text can sit in a call. The `swept` half is the one that
# goes red when extraction is broken (no text found, so nothing closes); the `pending` half guards the
# other direction and passes on unchanged code, so its evidence is the mutation that always closes.
while IFS='|' read -r label tool json; do
    swept_case "$tool" "$json" swept
    eq "24 $label: a write whose text another session committed is closed" "$(owner_of g.txt)" "$A"
    rm -rf "$T"
    swept_case "$tool" "$json" pending
    eq "24 $label: a write whose text is still uncommitted stays live" "$(owner_of g.txt)" "$B"
    rm -rf "$T"
done <<EOF
edit_file new_string|edit_file|{"action":"edit","path":"g.txt","old_string":"base","new_string":"$LINE"}
edit_file edits[]|edit_file|{"path":"g.txt","edits":[{"old_string":"base","new_string":"$LINE"}]}
edit_file edits[] content|edit_file|{"path":"g.txt","edits":[{"heading":"## X","action":"replace","content":"$LINE"}]}
edit_file body|edit_file|{"path":"g.txt","heading":"## X","action":"replace","body":"$LINE"}
edit_file bullet line|edit_file|{"action":"edit","path":"g.txt","new_string":"$BUL"}
edit_code body|edit_code|{"action":"replace","path":"g.txt","symbol":"f","body":"$LINE"}
create_file content|create_file|{"path":"g.txt","content":"$LINE"}
doc append_entry title|doc|{"action":"append_entry","id":"GID","id_prefix":"F","title":"$LINE","body":"x"}
doc append_entry body|doc|{"action":"append_entry","id":"GID","id_prefix":"F","title":"t","body":"$LINE"}
doc update patch.body|doc|{"action":"update","id":"GID","patch":{"body":"$LINE"}}
doc update body_edits content|doc|{"action":"update","id":"GID","patch":{"body_edits":[{"heading":"## X","action":"replace","content":"$LINE"}]}}
doc update body_edits new_string|doc|{"action":"update","id":"GID","patch":{"body_edits":[{"heading":"## X","action":"edit","old_string":"base","new_string":"$LINE"}]}}
doc create body|doc|{"action":"create","kind":"bug","rel_path":"g.txt","body":"$LINE"}
EOF

# 24n. What must NOT close a write, each a way to read "committed" off too little.
# a. no distinctive line: nothing to find, so the lookup cannot say the write landed.
new_repo; mkdb "$DB"
echo base > g.txt; git add g.txt; GIT_COMMITTER_DATE="$(old)" git commit -qm base
echo ok >> g.txt
wrote "$DB" "$B" edit_file '{"action":"edit","path":"g.txt","new_string":"ok"}' success "$(hrs '1 hour ago')"
commit_as "$C" g.txt
echo mine >> g.txt
add_as "$A" g.txt
eq "24n-a: a write with no distinctive line stays live even when its text is in HEAD" "$(owner_of g.txt)" "$B"
rm -rf "$T"
# b. only part of the text landed: every line must.
new_repo; mkdb "$DB"
echo base > g.txt; git add g.txt; GIT_COMMITTER_DATE="$(old)" git commit -qm base
echo "$LINE" >> g.txt
commit_as "$C" g.txt
wrote "$DB" "$B" edit_file "{\"action\":\"edit\",\"path\":\"g.txt\",\"new_string\":\"$LINE\\nthis second line was written and never reached any commit\"}" success "$(hrs '1 hour ago')"
echo mine >> g.txt
add_as "$A" g.txt
eq "24n-b: a write only part of whose text is in HEAD stays live" "$(owner_of g.txt)" "$B"
rm -rf "$T"
# c. the text was ALREADY in the file before the write (a move, a reorder): its presence in HEAD says
# nothing about whether the write landed.
new_repo; mkdb "$DB"
printf 'base\n%s\n' "$LINE" > g.txt; git add g.txt; GIT_COMMITTER_DATE="$(old)" git commit -qm base
wrote "$DB" "$B" edit_file "{\"action\":\"edit\",\"path\":\"g.txt\",\"new_string\":\"$LINE\"}" success "$(hrs '1 hour ago')"
echo mine >> g.txt
add_as "$A" g.txt
eq "24n-c: text already in the version before the write is not evidence it landed" "$(owner_of g.txt)" "$B"
rm -rf "$T"
# d. a path HEAD does not hold has no committed text to find.
new_repo; mkdb "$DB"
echo "$LINE" > g.txt
wrote "$DB" "$B" create_file "{\"path\":\"g.txt\",\"content\":\"$LINE\"}" success "$(hrs '1 hour ago')"
add_as "$A" g.txt
eq "24n-d: a path HEAD does not hold stays the writer's" "$(owner_of g.txt)" "$B"
rm -rf "$T"
# e. the writer's own commit still closes a write whose text is NOT found (case 4a's rule, kept).
new_repo; mkdb "$DB"
echo base > g.txt; git add g.txt; GIT_COMMITTER_DATE="$(old)" git commit -qm base
echo "$LINE" >> g.txt
wrote "$DB" "$B" edit_file "{\"action\":\"edit\",\"path\":\"g.txt\",\"new_string\":\"$LINE\"}" success "$(hrs '1 hour ago')"
echo other > g.txt
commit_as "$B" g.txt
echo mine >> g.txt
add_as "$A" g.txt
eq "24n-e: the writer's own later commit still closes it, text found or not" "$(owner_of g.txt)" "$A"
rm -rf "$T"

# f. only the writer's NEWEST live write is asked, so a writer who kept working in one place and whose
# later edit rewrote the words of an earlier one is still closed once the newest is in HEAD. (Asking
# each row separately leaves this writer live: the first row's line is gone from HEAD.)
LINE2="a second distinctive sentence the same writer put in later on"
new_repo; mkdb "$DB"
echo base > g.txt; git add g.txt; GIT_COMMITTER_DATE="$(old)" git commit -qm base
wrote "$DB" "$B" edit_file "{\"action\":\"edit\",\"path\":\"g.txt\",\"new_string\":\"$LINE\"}" success "$(hrs '2 hours ago')"
wrote "$DB" "$B" edit_file "{\"action\":\"edit\",\"path\":\"g.txt\",\"new_string\":\"$LINE2\"}" success "$(hrs '1 hour ago')"
printf 'base\n%s\n' "$LINE2" > g.txt
commit_as "$C" g.txt
echo mine >> g.txt
add_as "$A" g.txt
eq "24n-f: a later write that rewrote an earlier one's words closes the writer once it is in HEAD" "$(owner_of g.txt)" "$A"
rm -rf "$T"
# g. and the other way: the older write is in HEAD, the NEWEST is not, so the writer is live.
new_repo; mkdb "$DB"
echo base > g.txt; git add g.txt; GIT_COMMITTER_DATE="$(old)" git commit -qm base
wrote "$DB" "$B" edit_file "{\"action\":\"edit\",\"path\":\"g.txt\",\"new_string\":\"$LINE\"}" success "$(hrs '2 hours ago')"
printf 'base\n%s\n' "$LINE" > g.txt
commit_as "$C" g.txt
wrote "$DB" "$B" edit_file "{\"action\":\"edit\",\"path\":\"g.txt\",\"new_string\":\"$LINE2\"}" success "$(hrs '1 hour ago')"
printf '%s\n' "$LINE2" >> g.txt
echo mine >> g.txt
add_as "$A" g.txt
eq "24n-g: an older write in HEAD does not close a writer whose NEWEST write is not" "$(owner_of g.txt)" "$B"
rm -rf "$T"

# h/i. THREE lines are asked, the longest three, and every one must be in HEAD.
H1="the longest of the four lines, which is a fairly long sentence indeed"
H2="the second line in length, a little shorter than the first"
H3="the third line, shorter again"
H4="the fourth short line"
for variant in lacks-fourth lacks-third; do
    new_repo; mkdb "$DB"
    echo base > g.txt; git add g.txt; GIT_COMMITTER_DATE="$(old)" git commit -qm base
    wrote "$DB" "$B" edit_file "{\"action\":\"edit\",\"path\":\"g.txt\",\"new_string\":\"$H1\\n$H2\\n$H3\\n$H4\"}" success "$(hrs '1 hour ago')"
    if [ "$variant" = lacks-fourth ]; then printf 'base\n%s\n%s\n%s\n' "$H1" "$H2" "$H3" > g.txt
    else printf 'base\n%s\n%s\n%s\n' "$H1" "$H2" "$H4" > g.txt; fi
    commit_as "$C" g.txt
    echo mine >> g.txt
    add_as "$A" g.txt
    if [ "$variant" = lacks-fourth ]; then
        eq "24n-h: the fourth-longest line is not asked, so a write whose longest three landed is closed" "$(owner_of g.txt)" "$A"
    else
        eq "24n-i: the third-longest line is asked, so a write missing it stays live" "$(owner_of g.txt)" "$B"
    fi
    rm -rf "$T"
done
# j. a line repeated in the text counts once, or three copies of it would stand in for lines never found.
new_repo; mkdb "$DB"
echo base > g.txt; git add g.txt; GIT_COMMITTER_DATE="$(old)" git commit -qm base
wrote "$DB" "$B" edit_file "{\"action\":\"edit\",\"path\":\"g.txt\",\"new_string\":\"$H1\\n$H1\\n$H1\\n$H2\"}" success "$(hrs '1 hour ago')"
printf 'base\n%s\n' "$H1" > g.txt
commit_as "$C" g.txt
echo mine >> g.txt
add_as "$A" g.txt
eq "24n-j: a repeated line is one line, so the shorter one still has to be found" "$(owner_of g.txt)" "$B"
rm -rf "$T"
# k. whitespace around a line is not part of it: an editor re-indents and strips trailing spaces.
new_repo; mkdb "$DB"
echo base > g.txt; git add g.txt; GIT_COMMITTER_DATE="$(old)" git commit -qm base
wrote "$DB" "$B" edit_file "{\"action\":\"edit\",\"path\":\"g.txt\",\"new_string\":\"    $LINE   \"}" success "$(hrs '1 hour ago')"
printf 'base\n%s\n' "$LINE" > g.txt
commit_as "$C" g.txt
echo mine >> g.txt
add_as "$A" g.txt
eq "24n-k: leading and trailing whitespace does not stop a line being found" "$(owner_of g.txt)" "$A"
rm -rf "$T"
# l. a file whose FIRST commit is the sweep has no earlier version to hold the text, and is closed.
new_repo; mkdb "$DB"
echo base > other.txt; git add other.txt; GIT_COMMITTER_DATE="$(old)" git commit -qm base
echo "$LINE" > g.txt
wrote "$DB" "$B" create_file "{\"path\":\"g.txt\",\"content\":\"$LINE\"}" success "$(hrs '1 hour ago')"
commit_as "$C" g.txt
echo mine >> g.txt
add_as "$A" g.txt
eq "24n-l: a path first committed by the sweeping commit is closed" "$(owner_of g.txt)" "$A"
rm -rf "$T"

# 24h. A stale foreign row heals when the pair is asked again. The recorder keeps a row for a pair that
# left the index ("retained") and drops it when a NAMED add puts the pair back, so the lookup runs again
# with the rule as it now stands. A pair that never left the index keeps its row: unstage it first.
# Built the way the measured one arose: A staged B's pending write (row B), unstaged it, and the text
# reached HEAD by someone else's commit while the working-tree blob stayed the same.
sweep_head() { # commit B's text to HEAD under C's trailer without touching the index or worktree
    bh="$(printf 'base\n%s\n' "$LINE" | git hash-object -w --stdin)"
    tre="$(printf '100644 blob %s\tg.txt\n' "$bh" | git mktree)"
    cm="$(GIT_COMMITTER_DATE="$(giso '30 minutes ago')" git commit-tree "$tre" -p HEAD -m sweep -m "Session-Id: $C")"
    git update-ref HEAD "$cm"
}
stale_repo() {
    new_repo; mkdb "$DB"
    echo base > g.txt; git add g.txt; GIT_COMMITTER_DATE="$(old)" git commit -qm base
    printf '%s\nmine\n' "$LINE" >> g.txt   # `mine` keeps the working-tree blob different from the swept one
    wrote "$DB" "$B" edit_file "{\"action\":\"edit\",\"path\":\"g.txt\",\"new_string\":\"$LINE\"}" success "$(hrs '1 hour ago')"
    add_as "$A" g.txt
}
stale_repo
eq "24h: precondition, B's write is pending so A staging it records B" "$(owner_of g.txt)" "$B"
git reset -q -- g.txt
sweep_head
add_as "$A" g.txt
eq "24h: a pair that left the index is asked again, and B's text is now in HEAD" "$(owner_of g.txt)" "$A"
rm -rf "$T"
stale_repo
sweep_head
add_as "$A" g.txt
eq "24i: a pair still staged keeps its row, which is why the refusal says to unstage first" "$(owner_of g.txt)" "$B"
git reset -q -- g.txt
add_as "$A" g.txt
eq "24i: and unstaging then re-adding by name asks again" "$(owner_of g.txt)" "$A"
rm -rf "$T" "$DB"

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

# ----------------- 7b. the ABSENT-LOG stand-down, CHARACTERISED and not endorsed
#
# `pre-commit-foreign-index.sh`'s `[ -s "$log" ] || exit 0` stands the guard down whenever
# the stage log is missing or empty. Unlike every other stand-down in that file it carried
# no comment and no case: present since the guard's first commit (99d5acac, 2026-09-01),
# and a mutation deleting the line outright killed nothing, so neither direction was
# asserted by anything.
#
# THIS SECTION RECORDS WHAT THE BEHAVIOUR IS. IT DOES NOT RULE THAT IT IS RIGHT — and the
# measurement below narrowed what "it" even refers to, so read the split before citing this.
#
# THE STAND-DOWN DOES NOT DECIDE THE VERDICT. Measured 2026-09-20 by deleting the line from
# a copy and running an identical fixture: still exit 0, because with no log there are no
# foreign owners and `((${#theirs[@]})) || exit 0` downstream reaches the same answer. The
# fail-open is STRUCTURAL, not a choice made at that line — which dissolves the question
# this section was opened to settle. What the line buys is one thing only: it stops an
# unguarded `awk` from printing `awk: fatal: cannot open file` to the caller.
#
# That makes the earlier mutation result readable. Deleting the line killed nothing, and the
# natural reading — "untested" — was wrong. It is CLAUDE.md's third reading of a SURVIVED
# mutation: semantically inert for the verdict, because a sibling path already covers its
# domain. The assertions below are split accordingly, one per reason.
#
# On whether the fail-open itself is right, unresolved and left that way: `install-hooks.sh`'s
# seeding comment states this file's principle as "prefer the noisy wrong answer when the
# quiet one is unobservable", and an absent-log exit 0 is the quiet one. Against that,
# measured the same day, 12 of 13 `git_dir`s on this checkout carry a seeded log, and the
# one without is a linked worktree whose INDEX IS PRIVATE TO IT, so no peer's staged work
# exists there to capture. The exposure is a fresh clone or an un-hooked worktree, where
# this hook is not installed and does not run at all.
#
# The CONTROL is what makes the silence assertion mean anything. Identical fixture, one
# difference. Without it, `EXIT=0` is equally what "nothing foreign was staged" produces —
# which is § 7's own false-green warning, and the reason this is a pair rather than a case.
echo "== absent-log stand-down (characterisation)"

new_repo
echo base > a.txt
git add a.txt > /dev/null 2>&1
git commit -qm base
echo mine > a.txt
CLAUDE_CODE_SESSION_ID="$B" git add a.txt
eq "absent-log fixture stages a FOREIGN path" "$(owner_at a.txt)" "$B"
has "log PRESENT + foreign path -> refuse" "$(guard "$A")" "EXIT=1"
# No git command between the removal and the guard, so `post-index-change` cannot fire and
# recreate it — this is deterministic where § 2b's cases are not.
rm -f "$(git rev-parse --git-dir)/session-stage-log"
# OVER-DETERMINED, and labelled so nobody credits this green to the `[ -s "$log" ]` line:
# with no log there are no foreign owners either, so the downstream
# `((${#theirs[@]})) || exit 0` produces this same 0 on its own. Measured 2026-09-20 by
# deleting the stand-down from a copy and re-running this fixture — still exit 0. So this
# assertion pins the OUTCOME and not the line, which is the whole reason the next one exists.
has "log ABSENT -> guard is silent" "$(guard "$A")" "EXIT=0"
# THIS is what the stand-down actually buys, and it is the only assertion in this section
# that reds when the line is deleted: without it the unguarded `awk` below it opens a file
# that is not there and prints `awk: fatal: cannot open file ...` to the caller. Verified
# both ways on a modified copy — present: clean; deleted: fatal on stderr, exit still 0.
#
# Same unguarded-awk shape as this suite's own `owner_of`, fixed in aa831668. The
# difference is that `owner_of` was guarded at the read while this one is only MASKED by an
# early exit, so the fatal is one deleted line away rather than absent.
eq "log ABSENT -> no awk fatal reaches the caller" "$(guard "$A" | grep -c 'awk: fatal')" "0"
# THE STAND-DOWN IS NOW LOUD, and this is the assertion that pins it.
#
# The three above characterise a guard that returns EXIT=0 and says NOTHING — output
# byte-identical to "I checked and nothing foreign was staged". That is the unobservable
# silence `install-hooks.sh`'s seeding comment states this file's own principle against:
# "prefer the noisy wrong answer when the quiet one is unobservable". A reader cannot tell
# "the guard cleared you" from "the guard could not run", and both look like success.
#
# WHAT THE NOTICE DOES NOT CHANGE: the verdict. `log ABSENT -> guard is silent` above still
# asserts EXIT=0 and is deliberately left ABOVE this line, so a notice that ever started
# refusing reds there rather than here. P1 ("refuse whenever the log is cold") was measured
# and REJECTED on 2026-09-01 — it refuses your own routine commits, which is the version
# that gets the guard switched off — see the decision table in
# docs/issues/archive/2026-09-01-an-absent-stage-log-makes-the-foreign-index-guard-pass.md.
# This is not P1: it changes the OUTPUT on a path whose verdict is already settled, which is
# an axis that table never had a column for.
#
# Asserted on the ENTITY the notice must name, not on its prose. CLAUDE.md § Testing
# Discipline: pinning a sentence reds on every rewording and is rightly avoided, while
# asserting the message still names the thing it is about is cheap and reds exactly on the
# deletion. Reworded freely, this holds; deleted, it reds.
has "log ABSENT -> the guard names what it could not read" "$(guard "$A")" "session-stage-log"
rm -rf "$T"

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

# The remedy the `unnamed` refusal prints must be one that WORKS (0cbb244c). It used to say
# "Re-stage by explicit path and the bare commit passes", which is false for the common case:
# the refused session has just staged that content, so a re-add is not an index change and the
# recorder writes nothing. The first two assertions are the PREMISE, measured, so the text can
# change back only if the behaviour does. The next two are the step that works, and the last
# pair pins the text: it must say that step and no longer promise the one that fails.
new_repo
mkdir -p sub
echo b > sub/b.txt
CLAUDE_CODE_SESSION_ID="$S_A" git add sub/
remedy_out="$(guard "$S_A")"
CLAUDE_CODE_SESSION_ID="$S_A" git add -- sub/b.txt
eq "a plain re-add of already staged content does not re-claim it" \
    "$(owner_of sub/b.txt)" "-"
has "...so the bare commit is still refused" "$(guard "$S_A")" "EXIT=1"
CLAUDE_CODE_SESSION_ID="$S_A" git reset -q -- sub/b.txt
CLAUDE_CODE_SESSION_ID="$S_A" git add -- sub/b.txt
eq "unstaging first and then adding by explicit path claims it" \
    "$(owner_of sub/b.txt)" "$S_A"
has "...and the bare commit then passes" "$(guard "$S_A")" "EXIT=0"
has "the blanket-add remedy says why a plain re-add fails" "$remedy_out" "records nothing"
has "the blanket-add remedy limits the unstage to the reader's own paths" \
    "$remedy_out" "Only for paths YOU wrote"
hasnt "the blanket-add remedy no longer promises a plain re-add works" \
    "$remedy_out" "Re-stage by explicit path"
rm -rf "$T"

# `--pathspec-from-file` is a blanket form to the recorder: it never reads the list, so the
# paths are not named on argv. The cause line must name it, or a reader whose staging used it
# cannot tell from the text that theirs was the cause.
new_repo
mkdir -p sub
echo d > sub/d.txt
echo sub/d.txt > list.txt
CLAUDE_CODE_SESSION_ID="$S_A" git add --pathspec-from-file=list.txt
eq "--pathspec-from-file is recorded as unnamed" "$(route_of sub/d.txt)" "unnamed"
has "the blanket-add cause names --pathspec-from-file" "$(guard "$S_A")" "pathspec-from-file"
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
# docs/issues/archive/2026-09-02-a-refused-pathspec-commit-stamps-your-own-content-unowned.md
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

# ---------------------------------------------------------------------------
# 11. THE FMT CHECK READS THE COMMIT, NOT THE DISK
#
# scripts/pre-commit-cargo-fmt.sh replaces the `cargo-fmt` entry in
# .pre-commit-config.yaml, which passed FILENAMES to rustfmt and so read the working
# tree. That was correct only because the pre-commit framework stashes every unstaged
# change in the checkout first — the repo-wide stash this migration exists to remove
# (docs/issues/archive/2026-09-03-pre-commit-stash-window-feeds-peers-wrong-bytes-or-enoent.md).
#
# CASES 3 AND 4 ARE THE WHOLE POINT AND THEY POINT IN OPPOSITE DIRECTIONS. A stash-less
# filename hook fails 3 (refusing a good commit over an unstaged edit) and — worse —
# PASSES 4, shipping unformatted content because the disk happened to be tidy. Testing
# only one of them would be satisfied by a hook that reads either source: 1 and 2 alone
# pass against the old implementation.
#
# CASE 7 IS NOT DECORATION. The predicate is `exit != 0 OR stdout non-empty`, and reading
# the exit code ALONE yields a check that cannot fail: `rustfmt --check` on stdin reports
# formatting differences on stdout and still exits 0 (measured 2026-09-07, rustfmt
# 1.9.0-stable). Case 7 is the parse-error row, which is the half stdout cannot see — it
# reports on stderr with an empty stdout. Delete either arm of the predicate and exactly
# one of cases 1 and 7 reds.
FMT="$SRC/pre-commit-cargo-fmt.sh"
GOODRS='fn main() {
    let x = 1;
    println!("{x}");
}
'
BADRS='fn main() {
let x=1;
   println!("{x}");
  }
'

fmt_exit() { bash "$FMT" >/dev/null 2>&1; echo "$?"; }

new_repo
git commit -q --allow-empty -m base

printf '%s' "$BADRS" > a.rs; git add a.rs
eq "unformatted committed bytes are refused" "$(fmt_exit)" "1"

printf '%s' "$GOODRS" > a.rs; git add a.rs
eq "formatted committed bytes pass" "$(fmt_exit)" "0"

# 3. The old hook needed the stash for exactly this: an unstaged mess on disk over a
#    clean index must NOT refuse, because the mess is not what is being committed.
printf '%s' "$GOODRS" > a.rs; git add a.rs
printf '%s' "$BADRS"  > a.rs
eq "reads the index, not the working tree" "$(fmt_exit)" "0"

# 4. The inverse, and the expensive one: a tidy working tree must not launder an
#    unformatted blob into the commit. This is the silent false PASS.
printf '%s' "$BADRS"  > a.rs; git add a.rs
printf '%s' "$GOODRS" > a.rs
eq "cannot be fooled by a clean working tree" "$(fmt_exit)" "1"

# `-f` is required: case 4 leaves index and worktree disagreeing on purpose, and plain
# `git rm --cached` refuses that state. Without it a.rs stays staged carrying case 4's
# blob and case 5 silently re-measures case 4 — observed while writing this suite.
git rm -q --cached -f a.rs; rm -f a.rs
printf 'hello\n' > notes.md; git add notes.md
eq "a commit with no rust paths is a clean pass" "$(fmt_exit)" "0"

# 6. Given a FILENAME, rustfmt resolves the `mod` children of the file and errors with
#    "failed to resolve mod" when one is absent; given stdin it formats the buffer alone.
#    This asserts the recursion is GONE rather than merely unused — the old hook inherited
#    it, at a measured 1868 ms on a crate-root commit, and checked files not being
#    committed.
printf 'mod nowhere;\nfn main() {let y=2;}\n' > lib.rs; git add lib.rs
eq "a crate root whose mod child is absent is still checked" "$(fmt_exit)" "1"

# 6b. THE MOD-CHILD RECURSION LOSS, PINNED IN BOTH DIRECTIONS SO IT STAYS DELIBERATE.
#
#     Case 6 shows the recursion is gone; it cannot show that losing it is safe, because
#     an ABSENT child cannot distinguish "no recursion" from "nothing to recurse into".
#     These two do, and they must be read as a pair: the first alone would be satisfied by
#     a check that never looks at children at all, the second alone by one that recurses.
#
#     The behaviour: an unstaged misformatted child is NOT the commit's problem and must
#     not refuse it; the same child, once staged, gets its own pass through the loop and
#     must refuse. That is the index-vs-worktree correction again, seen from the module
#     graph instead of from one file.
git rm -q --cached -f lib.rs; rm -f lib.rs
printf 'mod child;\n\nfn main() {\n    child::go();\n}\n' > root.rs
printf 'pub fn go() {let z=3;println!("{z}");}\n' > child.rs
git add root.rs
eq "an unstaged misformatted mod child does not refuse the commit" "$(fmt_exit)" "0"
git add child.rs
eq "the same child, once staged, is checked on its own" "$(fmt_exit)" "1"
git rm -q --cached -f root.rs child.rs; rm -f root.rs child.rs

# 7. The parse-error row: stdout is EMPTY and the exit code is 1. Only the exit-code arm
#    of the predicate sees this, as case 1 is only seen by the stdout arm.
#
#    No `git rm lib.rs` here: case 6b now owns that cleanup, and a second copy printed a
#    live `fatal: pathspec 'lib.rs' did not match any files` into a suite that reported
#    91 passed / 0 failed. Harmless, and worth removing anyway — a stray fatal in green
#    output is how a reader learns that this suite's noise can be skimmed.
printf 'fn main( {\n  not rust\n' > broken.rs; git add broken.rs
eq "content that does not parse is refused" "$(fmt_exit)" "1"

# 8. The refusal reaches the shared sequence tail, like every other refusing hook.
#
# `broken.rs` MUST be unstaged first, and this line is load-bearing. Case 7 leaves it
# staged, and it refuses via the exit-code arm — so with it still in the index this case
# passes on a refusal it did not cause. Caught by mutation: dropping the stdout arm from
# the predicate reddened cases 1, 4 and 6 while THIS one stayed green, which is the
# signature of a case measuring its neighbour's fixture rather than its own.
git rm -q --cached -f broken.rs; rm -f broken.rs
printf '%s' "$BADRS" > c.rs; git add c.rs
has "the refusal emits the shared commit-sequence tail" \
    "$(bash "$FMT" 2>&1)" "This is one rule in a sequence"
rm -rf "$T"

# ==============================================================================
# 12. A MOVE THAT STRANDS A CITATION OUTSIDE THE COMMIT
# ==============================================================================
# scripts/pre-commit-orphaned-citations.sh. Measured at `215a5cad`: six archive renames
# landed while the tracker citing them stayed uncommitted, leaving HEAD with seven live
# `../issues/2026-09-13-...` citation lines against paths absent from that tree.
#
# THE DISCRIMINATION IS THE COMPLEMENT, not the rename. A commit that moves a file AND
# repoints its citer is the correct shape and must stay silent; one that moves it and
# leaves the citer behind must speak. A check that fired on every rename would be noise on
# every archive, which is how `--no-verify` gets learned.
echo "== orphaned citations (move vs complement)"

# Exit code and output must be captured SEPARATELY here. The warning path exits 0, so an
# `EXIT=` marker cannot discriminate "warned" from "did nothing" -- the two differ only in
# bytes printed. Same reasoning as tests/pre-commit-dead-artifact-ids.sh:160-161.
orph() { OUT="$(bash "$SRC/pre-commit-orphaned-citations.sh" 2>&1)"; RC=$?; }

new_repo
mkdir -p docs/issues/archive docs/trackers
echo '# foo' > docs/issues/2026-09-13-foo.md
printf 'See [foo](../issues/2026-09-13-foo.md) for the detail.\n' > docs/trackers/t.md
git add -A > /dev/null 2>&1
git commit -qm base

# The citation is RELATIVE, which is the common form here and the reason the scan is keyed
# on the STEM rather than the full path -- a path-anchored grep matches none of these.
git mv docs/issues/2026-09-13-foo.md docs/issues/archive/2026-09-13-foo.md
orph
eq  "a stranded citation exits 0 (warn, never refuse)" "$RC" "0"
has "it names the moved source"                       "$OUT" "docs/issues/2026-09-13-foo.md"
has "it names the citer left behind"                  "$OUT" "docs/trackers/t.md"
has "it says it is not a refusal"                     "$OUT" "Not a refusal"

# THE SILENCE CONTROL, and it is the case that makes the one above mean anything. Repoint
# the citer in the SAME commit and the check must print NOTHING -- asserted in bytes,
# because exit 0 is true of both a silent pass and a warning.
sed -i 's|\.\./issues/2026-09-13-foo\.md|../issues/archive/2026-09-13-foo.md|' docs/trackers/t.md
git add docs/trackers/t.md
orph
eq "citer repointed in the same commit -> exits 0"   "$RC" "0"
eq "and prints nothing at all"                       "$(printf '%s' "$OUT" | wc -c)" "0"
rm -rf "$T"

# THE SAME SILENCE, WITH THE CITER ITSELF RENAMED -- and the control above cannot express
# this case, which is why it shipped broken. That one repoints the citer with `sed` + `git
# add`, so the citer is an `M` and its path is IDENTICAL in both of the hook's inputs. Here
# it is an `R`, and the two inputs disagree:
#
#   git diff --cached --name-only   -> the rename's DESTINATION only   (suppression set)
#   git grep -l -F "$stem" HEAD     -> the PRE-rename name             (citer list)
#
# A literal path match between those can never succeed, so the hook warns on the one shape
# its own header calls "the correct shape [that] must stay silent". Both bug files archived
# at 5a61efee hit it live.
#
# LOAD-BEARING: the citer must move to a directory at a DIFFERENT DEPTH, so its repointed
# link is `../../issues/archive/...` rather than `../issues/...`. A same-depth move would
# leave the two relative links textually equal and the case would pass for the wrong reason.
# LOAD-BEARING, AND THE REASON THE FIRST VERSION OF THIS CASE PASSED AGAINST THE BROKEN
# SCRIPT: the citer must be big enough that git DETECTS its rename. `--name-only` reports a
# detected rename as its DESTINATION ONLY, but reports an undetected one as a `D` plus an
# `A` -- which puts the old path back in the list and suppresses the warning for the wrong
# reason. A one-line citer whose only line is rewritten scores below git's 50% similarity
# cutoff, so the defect is invisible to it. Forty filler lines put this at R090; the two
# real archives that hit this live were R096 and R097.
#
# That is the `R`-is-a-similarity-verdict caveat from doc(action="move")'s stage_hint,
# reaching a test fixture: the same change is one row or two depending on content size.
new_repo
mkdir -p docs/issues/archive docs/trackers/archive
echo '# foo' > docs/issues/2026-09-13-foo.md
{ echo '# tracker'
  for i in $(seq 1 40); do echo "filler line $i"; done
  echo 'See [foo](../issues/2026-09-13-foo.md) for the detail.'
} > docs/trackers/t.md
git add -A > /dev/null 2>&1
git commit -qm base

git mv docs/issues/2026-09-13-foo.md docs/issues/archive/2026-09-13-foo.md
git mv docs/trackers/t.md docs/trackers/archive/t.md
sed -i 's|\.\./issues/2026-09-13-foo\.md|../../issues/archive/2026-09-13-foo.md|' \
    docs/trackers/archive/t.md
git add docs/trackers/archive/t.md
# The fixture's own precondition: if git stopped detecting this rename the case would pass
# vacuously, exactly as its first version did.
eq "fixture: the citer's rename IS detected" \
   "$(git diff --cached --name-status -M | awk '$1 ~ /^R/ && $2 == "docs/trackers/t.md" {print "yes"}')" "yes"
orph
eq "a citer RENAMED in the same commit -> exits 0"     "$RC" "0"
eq "a renamed citer is suppressed like a modified one" "$(printf '%s' "$OUT" | wc -c)" "0"
rm -rf "$T"

# Self-gating: a commit staging no rename must not pay for a scan, and must say nothing.
new_repo
echo a > a.md
git add a.md > /dev/null 2>&1
git commit -qm base
echo b > b.md
git add b.md
orph
eq "no rename staged -> exits 0"        "$RC" "0"
eq "no rename staged -> prints nothing" "$(printf '%s' "$OUT" | wc -c)" "0"
rm -rf "$T"

# A file whose own content mentions its own stem is its own subject, not a citer of itself.
# Without the self-exclusion every archive move warns about the file being moved, which is
# noise on the commonest operation this check sees.
new_repo
mkdir -p docs/issues/archive
printf '# 2026-09-13-solo\n\nThis file names 2026-09-13-solo in its own body.\n' \
    > docs/issues/2026-09-13-solo.md
git add -A > /dev/null 2>&1
git commit -qm base
git mv docs/issues/2026-09-13-solo.md docs/issues/archive/2026-09-13-solo.md
orph
eq "a self-mentioning file is not its own citer" "$(printf '%s' "$OUT" | wc -c)" "0"
rm -rf "$T"

echo
echo "passed=$PASS failed=$FAIL"
[ "$FAIL" = "0" ]
