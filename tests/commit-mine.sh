#!/usr/bin/env bash
# tests/commit-mine.sh — the empty intersection, and the commit form that leaves it.
#
# WHAT THIS GUARDS
#
# On a shared index, a session whose own staged change is COUPLED to a file a peer is
# also editing had no compliant commit
# (`docs/issues/2026-09-01-two-correct-pre-commit-guards-have-an-empty-intersection.md`):
#
#   R1  bare `git commit`            -> foreign-index refuses: the index holds a peer's path
#   R2  `git commit -- <both files>` -> unreviewed-content refuses: a pathspec commit takes
#                                       the WHOLE working-tree file, peer's hunk included
#   R3  `git commit -- <bug file>`   -> ledger-counts refuses: the class gained a member and
#                                       its `**Members:**` line did not change with it
#
# R1-R3 assert that reproduction. They are the RED this suite was written from, kept as
# regression cases: if one of them stops refusing, the premise of `scripts/commit-mine.sh`
# has changed and this file should be re-read, not just re-greened.
#
# F1-F9 are the helper's contract. MEASURED with scripts/mutation-probe.sh, eight mutations,
# one per guarded site, all killed against a 37/0 baseline. Which assertion kills each is
# recorded because it is NOT the one the first draft of this header named:
#   - private index built from EVERY staged entry  -> killed by "F1 helper exits 0": the
#     foreign-index guard, judging the PRIVATE index, refuses the peer's path. That is the
#     design's defence in depth working, and it means "b.txt is not in HEAD" is a BACKSTOP
#     that kills nothing on its own while that guard runs.
#   - pathspec commit of my paths                   -> killed by "F1 helper exits 0" too:
#     unreviewed-content refuses first, so "HEAD's IC-1 lacks B's line" is likewise a backstop.
#   - an empty middle field in `--classify` output  -> killed by F1: `read` collapses the two
#     tabs, the path lands in the owner field, and nothing is classified as yours.
#   - rename-split check off -> F6 | no-log fail-open -> F8 | sequencer fail-open -> F9
#   - `--` not refused -> F7 | refusal's pointer to the helper deleted -> R1
# Both backstops stay: each is the only assertion that would see the wrong commit if a
# guard above it ever admitted the input. Inert-as-killer, not inert-as-check.
# F2 kills the hazard the design argument says cannot happen: that committing from a
# private index leaves the SHARED index stale, so the peer's next bare commit reverts mine.
#
# Fixtures are throwaway repos under `mktemp -d` — never this checkout, which several
# sessions share. The corpus is synthetic and minimal for the same reason as
# tests/pre-commit-ledger-divergence.sh, whose fixture rules (ELEVEN classes, LETTERED slugs,
# roster and per-class files kept apart) apply here unchanged and are not re-derived.
set -uo pipefail

SELF_ROOT=$(cd "$(dirname "$0")/.." && pwd)
PASS=0
FAIL=0

ok() { PASS=$((PASS + 1)); }
no() { FAIL=$((FAIL + 1)); echo "FAIL: $1"; [ -n "${2:-}" ] && printf '  %s\n' "$2"; }
eq() { [ "$2" = "$3" ] && ok || no "$1" "actual:   $2
  expected: $3"; }
has() { printf '%s' "$2" | grep -qF -- "$3" && ok || no "$1" "missing: $3"; }
hasnt() { printf '%s' "$2" | grep -qF -- "$3" && no "$1" "unexpected: $3" || ok; }

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

A=aaaaaaaa-0000-4000-8000-00000000000a
B=bbbbbbbb-0000-4000-8000-00000000000b
CLASSES=11
SLUGS=(demo-a demo-b demo-c demo-d demo-e demo-f demo-g demo-h demo-i demo-j demo-k)
IC1=docs/trackers/issue-clusters/IC-1-demo-a.md
BUG=docs/issues/2026-09-24-new-bug.md

# The corpus is committed BEFORE the hooks are installed, so the stage log starts absent and
# every row in it is written by a case below. F8 depends on that: it is the no-log case.
new_repo() {
    local p="$WORK/repo$RANDOM$RANDOM" i s
    mkdir -p "$p/docs/trackers/issue-clusters" "$p/docs/issues" "$p/scripts"
    git -C "$p" init -q
    git -C "$p" config user.email t@t
    git -C "$p" config user.name t
    {
        echo "# Issue clusters"
        echo
        echo "| id | class | slug | promotes to |"
        echo "|---|---|---|---|"
        for i in $(seq 1 "$CLASSES"); do
            printf '| IC-%s | a demo class | `%s` | not yet |\n' "$i" "${SLUGS[$((i - 1))]}"
        done
    } > "$p/docs/trackers/issue-clusters.md"
    for i in $(seq 1 "$CLASSES"); do
        s="${SLUGS[$((i - 1))]}"
        printf '# IC-%s — demo class %s\n\n**Slug:** `cluster/%s`\n\n**Members:** none yet.\n' \
            "$i" "$i" "$s" > "$p/docs/trackers/issue-clusters/IC-$i-$s.md"
    done
    echo "same content" > "$p/old.txt"
    for f in pre-commit-foreign-index.sh pre-commit-unreviewed-content.sh \
             pre-commit-ledger-counts.py post-index-change-stage-log.sh \
             commit-sequence-tail.txt commit-mine.sh; do
        [ -f "$SELF_ROOT/scripts/$f" ] && cp "$SELF_ROOT/scripts/$f" "$p/scripts/"
    done
    git -C "$p" add -A >/dev/null
    git -C "$p" commit -qm corpus
    # The three guards that judge a commit's INDEX, in scripts/pre-commit-run.sh's order, and
    # like it they all run and any refusal fails the commit. rustfmt, dead-artifact-ids and
    # orphaned-citations are left out: none reads ownership, and each needs a repo this is not.
    cat > "$p/.git/hooks/pre-commit" <<'HOOK'
#!/usr/bin/env bash
rc=0
bash scripts/pre-commit-unreviewed-content.sh || rc=1
bash scripts/pre-commit-foreign-index.sh || rc=1
python3 scripts/pre-commit-ledger-counts.py || rc=1
exit $rc
HOOK
    cat > "$p/.git/hooks/post-index-change" <<'HOOK'
#!/usr/bin/env bash
exec bash scripts/post-index-change-stage-log.sh "$@"
HOOK
    chmod +x "$p/.git/hooks/pre-commit" "$p/.git/hooks/post-index-change"
    echo "$p"
}

as() { local sid="$1"; shift; CLAUDE_CODE_SESSION_ID="$sid" "$@"; }

# The entangled state, built the way two sessions actually reach it:
#   A files a bug in class demo-a and names it on IC-1's Members line, staging both;
#   B stages an unrelated file, then edits IC-1 in the working tree WITHOUT staging.
# So IC-1's index entry is A's alone while the working-tree file holds both hunks.
entangle() {
    printf -- '---\nkind: bug\nstatus: open\ntags:\n- cluster/demo-a\n---\n\n# BUG: new\n' > "$BUG"
    sed -i 's/^\*\*Members:\*\* none yet\.$/**Members:** none yet. +1: `new-bug` — filed by A./' "$IC1"
    as "$A" git add -- "$BUG" "$IC1"
    echo theirs > b.txt
    as "$B" git add -- b.txt
    printf '\nA note B is still writing.\n' >> "$IC1"
}

head_of() { git rev-parse HEAD; }

# ------------------------------------------------------------ R1-R3: the reproduction
cd "$(new_repo)" || exit 1
entangle
h0=$(head_of)

out=$(as "$A" git commit -qm r1 2>&1); rc=$?
[ "$rc" -ne 0 ] && ok || no "R1 bare commit is refused"
eq "R1 refusal leaves HEAD alone" "$(head_of)" "$h0"
has "R1 refusal names the peer's path" "$out" "b.txt"
has "R1 refusal points at the helper, the one route this suite proves exists" "$out" "commit-mine.sh"

out=$(as "$A" git commit -qm r2 -- "$BUG" "$IC1" 2>&1); rc=$?
[ "$rc" -ne 0 ] && ok || no "R2 pathspec commit of both files is refused"
eq "R2 refusal leaves HEAD alone" "$(head_of)" "$h0"
has "R2 is refused for the peer's unstaged hunk" "$out" "content you never staged"

out=$(as "$A" git commit -qm r3 -- "$BUG" 2>&1); rc=$?
[ "$rc" -ne 0 ] && ok || no "R3 pathspec commit of the bug file alone is refused"
eq "R3 refusal leaves HEAD alone" "$(head_of)" "$h0"
has "R3 is refused by the ledger rule" "$out" "a class gained a member"

# ------------------------------------------------------------ F1: the helper commits exactly A's
out=$(as "$A" bash scripts/commit-mine.sh -m fix 2>&1); rc=$?
eq "F1 helper exits 0" "$rc" "0"
[ "$rc" -eq 0 ] || printf '  output: %s\n' "$out"
eq "F1 exactly one new commit" "$(git rev-list --count "$h0"..HEAD 2>/dev/null)" "1"
eq "F1 commits exactly A's two paths" \
    "$(git show --name-only --format= HEAD | sort | tr '\n' ' ')" \
    "$(printf '%s\n' "$BUG" "$IC1" | sort | tr '\n' ' ')"
# Kills nothing alone (see header): the foreign-index guard refuses the over-inclusion first.
git cat-file -e HEAD:b.txt 2>/dev/null && no "F1 b.txt is not in HEAD" || ok
committed_ic1=$(git show "HEAD:$IC1" 2>/dev/null)
has "F1 HEAD's IC-1 carries A's Members line" "$committed_ic1" '+1: `new-bug`'
# Kills nothing alone (see header): unreviewed-content refuses a pathspec commit first.
hasnt "F1 HEAD's IC-1 lacks B's unstaged line" "$committed_ic1" "B is still writing"
eq "F1 the shared index still stages only B's path" "$(git diff --cached --name-only)" "b.txt"
has "F1 B's unstaged hunk is still in the working tree" "$(cat "$IC1")" "B is still writing"
has "F1 the helper names the staged path it left out" "$out" "b.txt"
has "F1 ...and whose it is" "$out" "$B"

# ------------------------------------------------------------ F2: the shared index is not stale
out=$(as "$B" git commit -qm b 2>&1); rc=$?
eq "F2 B's bare commit afterwards is accepted" "$rc" "0"
[ "$rc" -eq 0 ] || printf '  output: %s\n' "$out"
eq "F2 B's commit carries only b.txt" "$(git show --name-only --format= HEAD)" "b.txt"
has "F2 B's commit did not revert A's IC-1" "$(git show "HEAD:$IC1")" '+1: `new-bug`'

# ------------------------------------------------------------ F3: nothing staged is mine
cd "$(new_repo)" || exit 1
echo theirs > b.txt
as "$B" git add -- b.txt
h0=$(head_of)
out=$(as "$A" bash scripts/commit-mine.sh -m x 2>&1); rc=$?
eq "F3 nothing mine -> exit 1" "$rc" "1"
eq "F3 nothing mine -> HEAD unchanged" "$(head_of)" "$h0"
has "F3 says why" "$out" "nothing staged is yours"

# ------------------------------------------------------------ F5: no session id
out=$(env -u CLAUDE_CODE_SESSION_ID bash scripts/commit-mine.sh -m x 2>&1); rc=$?
eq "F5 no session id -> exit 2" "$rc" "2"

# ------------------------------------------------------------ F7: a pathspec defeats the helper
echo mine > a.txt
as "$A" git add -- a.txt
h0=$(head_of)
out=$(as "$A" bash scripts/commit-mine.sh -m x -- a.txt 2>&1); rc=$?
eq "F7 a pathspec argument is refused -> exit 2" "$rc" "2"
eq "F7 ...and nothing is committed" "$(head_of)" "$h0"
out=$(as "$A" bash scripts/commit-mine.sh -a -m x 2>&1); rc=$?
eq "F7 -a is refused -> exit 2" "$rc" "2"

# ------------------------------------------------------------ F9: mid-sequencer
# Re-read HEAD: F9 must not inherit F7's state, or an F7 break reads as an F9 one.
h0=$(head_of)
git rev-parse HEAD > .git/CHERRY_PICK_HEAD
out=$(as "$A" bash scripts/commit-mine.sh -m x 2>&1); rc=$?
eq "F9 a cherry-pick in progress -> exit 3" "$rc" "3"
eq "F9 ...and nothing is committed" "$(head_of)" "$h0"
rm -f .git/CHERRY_PICK_HEAD

# ------------------------------------------------------------ F6: a rename split across owners
cd "$(new_repo)" || exit 1
as "$B" git rm -q --cached old.txt
cp old.txt new.txt
as "$A" git add -- new.txt
h0=$(head_of)
out=$(as "$A" bash scripts/commit-mine.sh -m x 2>&1); rc=$?
eq "F6 a rename whose halves have different owners -> exit 1" "$rc" "1"
eq "F6 ...and nothing is committed (no half-rename becomes a copy)" "$(head_of)" "$h0"
has "F6 names the half it cannot take" "$out" "old.txt"

# ------------------------------------------------------------ F8: no stage log
cd "$(new_repo)" || exit 1
[ -e .git/session-stage-log ] && no "F8 fixture: the log must be absent" || ok
out=$(as "$A" bash scripts/commit-mine.sh -m x 2>&1); rc=$?
eq "F8 no stage log -> exit 3, never 'everything is mine'" "$rc" "3"

echo
echo "passed=$PASS failed=$FAIL"
[ "$FAIL" -eq 0 ]
