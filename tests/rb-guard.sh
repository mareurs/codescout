#!/usr/bin/env bash
# Suite for scripts/rb.sh — the guard that stops `cargo rb` compiling a tree behind
# origin. Incident it exists for: embedder-stack-ops-session-log:F-6.
#
# THE POSITIVE CONTROL IS THE POINT OF THIS FILE, not a courtesy row. A guard exercised
# only on its refusals is indistinguishable from one that refuses everything, and
# "refuses everything" is the failure mode a build wrapper can actually ship — every
# refusal row would stay green while `cargo rb` became unusable for every session on the
# checkout. So every REFUSE row below is paired with an ALLOW row reached through the
# same code path, and the allow rows assert the build command RAN.
#
# Reaching the allow path needs a seam: rb.sh ends in `exec cargo rb`, a multi-minute
# release build. `CODESCOUT_RB_BUILD_CMD` substitutes a marker script; nothing else in
# the repo sets it.

set -u

PASS=0; FAIL=0
ok()   { echo "  PASS  $1"; PASS=$((PASS + 1)); }
no()   { echo "  FAIL  $1"; echo "        $2"; FAIL=$((FAIL + 1)); }
eq()   { [ "$2" = "$3" ] && ok "$1" || no "$1" "expected $3, got $2"; }
has()  { printf '%s' "$2" | grep -qF -- "$3" && ok "$1" || no "$1" "expected to find: $3"; }
hasnt(){ printf '%s' "$2" | grep -qF -- "$3" && no "$1" "should NOT contain: $3" || ok "$1"; }

GUARD="$(cd "$(dirname "$0")/.." && pwd)/scripts/rb.sh"
[ -x "$GUARD" ] || [ -f "$GUARD" ] || { echo "no scripts/rb.sh at $GUARD"; exit 2; }

WORK="$(mktemp -d "${TMPDIR:-/tmp}/rb-guard-XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

# The marker: records that the build path was reached, and its argv. Its EXISTENCE is
# what the allow rows assert -- "no refusal printed" is monotone under the script dying
# early, so absence of a complaint proves nothing on its own.
BUILDMARK="$WORK/built"
cat > "$WORK/fakebuild" <<EOF
#!/usr/bin/env bash
echo "ARGS:\$*" >> "$BUILDMARK"
exit 0
EOF
chmod +x "$WORK/fakebuild"

# origin is a real bare repo; a clone gives a genuine upstream, so `@{upstream}` and
# `git fetch` exercise the real plumbing rather than a stub of it.
new_pair() {
    rm -rf "$WORK/origin" "$WORK/clone"; : > "$BUILDMARK"
    git init -q --bare -b main "$WORK/origin"
    git init -q -b main "$WORK/seed"
    git -C "$WORK/seed" config user.email t@example.invalid
    git -C "$WORK/seed" config user.name Test
    git -C "$WORK/seed" config core.hooksPath /dev/null
    echo base > "$WORK/seed/f"; git -C "$WORK/seed" add f
    git -C "$WORK/seed" commit -qm base
    git -C "$WORK/seed" remote add origin "$WORK/origin"
    git -C "$WORK/seed" push -q origin main
    git clone -q "$WORK/origin" "$WORK/clone"
    git -C "$WORK/clone" config user.email t@example.invalid
    git -C "$WORK/clone" config user.name Test
    git -C "$WORK/clone" config core.hooksPath /dev/null
    rm -rf "$WORK/seed"
}

# Advance origin by N commits WITHOUT touching the clone, so the clone is behind and its
# remote-tracking ref is stale -- the state F-6 was built in.
advance_origin() {
    local n="$1" tmp="$WORK/adv"
    rm -rf "$tmp"; git clone -q "$WORK/origin" "$tmp"
    git -C "$tmp" config user.email t@example.invalid
    git -C "$tmp" config user.name Test
    for i in $(seq 1 "$n"); do
        echo "c$i" >> "$tmp/f"; git -C "$tmp" add f
        git -C "$tmp" commit -qm "origin commit $i"
    done
    git -C "$tmp" push -q origin main
    rm -rf "$tmp"
}

run() {   # run <dir> [env assignments...]
    local dir="$1"; shift
    OUT="$( cd "$dir" && env CODESCOUT_RB_BUILD_CMD="$WORK/fakebuild" "$@" bash "$GUARD" 2>&1 )"
    EC=$?
}

built() { [ -s "$BUILDMARK" ] && echo yes || echo no; }

echo
echo "== level with origin: BUILDS (positive control) =="
new_pair
run "$WORK/clone"
eq  "allowed"                          "$EC" 0
eq  "the build command actually ran"   "$(built)" yes
hasnt "and says nothing about being behind" "$OUT" "REFUSING"

echo
echo "== behind origin: REFUSES, and does not build =="
new_pair
advance_origin 3
run "$WORK/clone"
eq  "refused"                          "$EC" 1
eq  "the build command did NOT run"    "$(built)" no
has "names the count"                  "$OUT" "3 commit(s) behind"
has "names the incident"               "$OUT" "F-6"
# The remedy must reach a party who can act AND say what they can do -- CLAUDE.md
# § Testing Discipline, the guard-remedy ceiling. Both halves, by shape not wording.
has "remedy names the pull"            "$OUT" "git pull --rebase"
has "remedy names the shared-tree cost" "$OUT" "EVERY session"
has "remedy names the deliberate escape" "$OUT" "CODESCOUT_RB_ACK=behind"
# Naming what origin holds is what makes the refusal actionable rather than a verdict.
has "lists what origin has and you do not" "$OUT" "origin commit"

echo
echo "== behind origin + ack: BUILDS, and says it is deliberate =="
# The ALLOW twin of the row above, through the same branch. Without it, a guard that
# ignored the ack entirely would leave every assertion above green.
new_pair
advance_origin 2
run "$WORK/clone" CODESCOUT_RB_ACK=behind
eq  "allowed with ack"                 "$EC" 0
eq  "the build command ran"            "$(built)" yes
has "and records the decision"         "$OUT" "deliberately"
has "naming the distance"              "$OUT" "2 commit(s) behind origin"

echo
echo "== the STALE-REF trap: a fetch it cannot perform is not a pass =="
# This is the defect the guard would otherwise reproduce one layer up. `git rev-list
# --count HEAD..@{upstream}` reads a LOCAL ref; with origin unreachable and no fetch, it
# returns 0 for a checkout that is arbitrarily far behind. A zero there means NOT LOOKED
# AT, and must not read as NOT BEHIND.
new_pair
advance_origin 4
# Point origin at a path that does not exist: fetch fails, the stale ref still says 0.
git -C "$WORK/clone" remote set-url origin "$WORK/origin-gone"
STALE="$(git -C "$WORK/clone" rev-list --count HEAD..@{upstream} 2>/dev/null || echo ERR)"
eq  "fixture: the stale ref really does report 0" "$STALE" 0
run "$WORK/clone"
eq  "refused rather than trusting it"  "$EC" 2
eq  "the build command did NOT run"    "$(built)" no
has "names the fetch as the failure"   "$OUT" "fetch of origin"
has "says why a zero would be wrong"   "$OUT" "NOT LOOKED AT"

echo
echo "== fetch failure + ack: BUILDS, flagged as unverified =="
new_pair
git -C "$WORK/clone" remote set-url origin "$WORK/origin-gone"
run "$WORK/clone" CODESCOUT_RB_ACK=behind
eq  "allowed with ack"                 "$EC" 0
eq  "the build command ran"            "$(built)" yes
has "flagged as unverified"            "$OUT" "unverified tree"

echo
echo "== no upstream: BUILDS, but says the question has no answer =="
# Not a refusal: a detached HEAD or an untracked branch is legitimate. Asserted because
# an unremarked pass is byte-identical to a check that ran and succeeded.
new_pair
git -C "$WORK/clone" checkout -q --detach
run "$WORK/clone"
eq  "allowed"                          "$EC" 0
eq  "the build command ran"            "$(built)" yes
has "says the question has no answer"  "$OUT" "has no answer here"
hasnt "and does not call it a refusal" "$OUT" "REFUSING"

echo
echo "== not a git checkout at all: BUILDS, silently =="
new_pair
mkdir -p "$WORK/plain"
run "$WORK/plain"
eq  "allowed"                          "$EC" 0
eq  "the build command ran"            "$(built)" yes
eq  "and is completely silent"         "$OUT" ""

echo
echo "== arguments reach the build command =="
new_pair
OUT="$( cd "$WORK/clone" && env CODESCOUT_RB_BUILD_CMD="$WORK/fakebuild" bash "$GUARD" --locked 2>&1 )"
has "argv forwarded"                   "$(cat "$BUILDMARK")" "ARGS:--locked"

echo
echo "-------------------------------------------"
echo "  $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
