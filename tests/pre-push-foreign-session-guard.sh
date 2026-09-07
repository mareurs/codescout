#!/usr/bin/env bash
#
# Discrimination matrix for `scripts/pre-push-foreign-session-guard.sh`.
#
# WHY THIS EXISTS
# ---------------
# The guard refuses a push that would publish another session's commits. It is the
# pusher-side complement to "a session that cannot publish must not COMMIT to a shared
# branch" (docs/trackers/observer-blindness.md OB-20), and it is the kind of guard that
# fails silently in the wrong direction: one that refuses everything looks exactly as
# safe as one that works, right up until someone disables it, and one that refuses
# nothing looks identical to a quiet week.
#
# So every case below is a DISCRIMINATION — each asserts silence where silence is
# required AND refusal where refusal is required. Same posture as
# `tests/hooks-discrimination.sh`, whose idiom this borrows.
#
# HERMETIC ON PURPOSE. Every case builds its own throwaway repo under $TMPDIR with
# commits whose `Session-Id` trailers this file controls. It never reads this checkout's
# history: the first version of these cases cited live SHAs from the unpushed pile, which
# made the suite a measurement of one instant — `experiments` is rebased after every ship,
# so those SHAs die and the test would have started failing for a reason having nothing to
# do with the guard.
#
# VERIFYING THE LIVE GUARD BY DISABLING IT IS A TRADE WITH A DIRECTION. To confirm the
# shim degrades open, this session parked `scripts/pre-push-foreign-session-guard.sh` and
# ran `--check` and a real `git push --dry-run` against the same state. That is the
# stronger evidence — both exits observed in one state, and reading a predicate cannot rule
# out an earlier line returning first — but for the seconds it runs, the guard is OFF on a
# tree four sessions share, which is precisely the window the guard exists to cover.
# Verifying a safety mechanism by disabling it creates the state it protects against.
#
# The cheaper alternative, which `codescout-7f` used to confirm the same claim: read the
# predicate (`install-hooks.sh`'s `[ ! -x "$PROJECT_ROOT/$target" ]` arm) and exercise the
# instrument in the HEALTHY state as a positive control. Weaker, and free.
#
# On a single-session checkout the disable costs nothing and you should always take it.
# Here it is not free, and neither party priced it at the time. Prefer the cases below —
# they disable nothing, because every one of them runs in a throwaway repo.
#
# Usage:
#   tests/pre-push-foreign-session-guard.sh     # non-zero exit on any failure

set -uo pipefail

GUARD="$(cd "$(dirname "$0")/../scripts" && pwd)/pre-push-foreign-session-guard.sh"
ZERO="0000000000000000000000000000000000000000"

ALICE="aaaaaaaa-1111-2222-3333-444444444444"
BOB="bbbbbbbb-5555-6666-7777-888888888888"
CAROL="cccccccc-9999-0000-1111-222222222222"

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

# Just the per-commit rows of a refusal (`    <sha8>  <sid>  <subject>`). Asserting
# "your sid is absent from the OUTPUT" would be wrong: the message prints `Your session
# id: <you>` on purpose, so the scope of that claim is the report rows, not the page.
report_rows() { printf '%s' "$1" | grep -E '^    [0-9a-f]{8}  ' || true; }

[ -x "$GUARD" ] || { echo "FATAL: $GUARD is not executable"; exit 1; }

# ---------------------------------------------------------------- throwaway repo builder
# commit <sid|-> <subject>   — `-` means NO trailer, the plain-terminal case.
REPO=""
new_repo() {
    REPO="$(mktemp -d "${TMPDIR:-/tmp}/prepush-guard-XXXXXX")"
    git -C "$REPO" init -q -b main
    git -C "$REPO" config user.email t@example.invalid
    git -C "$REPO" config user.name  Test
    # No hooks: this suite tests the guard directly, not the stamping hook.
    git -C "$REPO" config core.hooksPath /dev/null
}
commit() {
    local sid="$1" subject="$2" msg
    echo "$RANDOM$RANDOM" >> "$REPO/f.txt"
    git -C "$REPO" add f.txt
    if [ "$sid" = "-" ]; then
        msg="$subject"
    else
        printf -v msg '%s\n\nCo-Authored-By: T <t@example.invalid>\nSession-Id: %s' "$subject" "$sid"
    fi
    git -C "$REPO" commit -q -m "$msg"
}
sha() { git -C "$REPO" rev-parse "${1:-HEAD}"; }

# run <pusher-sid|-> <ack|-> <stdin-line>  -> sets OUT, EC
OUT=""; EC=0
run() {
    local pusher="$1" ack="$2" line="$3"
    local -a env=()
    [ "$pusher" != "-" ] && env+=("CLAUDE_CODE_SESSION_ID=$pusher")
    [ "$ack" != "-" ] && env+=("CODESCOUT_PUSH_ACK=$ack")
    OUT="$(printf '%s\n' "$line" | (cd "$REPO" && env -u CLAUDE_CODE_SESSION_ID -u CODESCOUT_PUSH_ACK "${env[@]}" "$GUARD" origin git@example.invalid:x) 2>&1)"
    EC=$?
}

echo
echo "== a push carrying only your own commits =="
new_repo
commit "$ALICE" "alice one"; BASE=$(sha)
commit "$ALICE" "alice two"; TIP=$(sha)
run "$ALICE" - "refs/heads/main $TIP refs/heads/main $BASE"
eq  "allowed"                        "$EC" 0
eq  "and says nothing at all"        "$(printf '%s' "$OUT" | wc -c)" 0

echo
echo "== a push carrying another session's commit =="
new_repo
commit "$ALICE" "alice base"; BASE=$(sha)
commit "$BOB"   "bob's withheld work"
commit "$ALICE" "alice on top"; TIP=$(sha)
run "$ALICE" - "refs/heads/main $TIP refs/heads/main $BASE"
eq  "refused"                        "$EC" 1
has "names the foreign sid"          "$OUT" "$BOB"
has "names the offending subject"    "$OUT" "bob's withheld work"
hasnt "your own commits are not in the report" "$(report_rows "$OUT")" "$ALICE"
hasnt "nor is your own subject"       "$(report_rows "$OUT")" "alice on top"
has "offers the ack, prefilled"      "$OUT" "CODESCOUT_PUSH_ACK=\"$BOB\""
has "points at the class"            "$OUT" "OB-20"

# THE REMEDY TEXT, ASSERTED AS SHAPE RATHER THAN AS PROSE.
#
# Every other assertion in this file is about the guard's PREDICATE -- who gets refused.
# None was about where the refusal SENDS you, and that is how the first version shipped
# saying only "ASK THE AUTHOR": a party who can report what they were told and cannot
# grant. Four sessions followed it and held eight hours while the pile went 2 -> 14, and a
# 54-assertion suite stayed green throughout, because the predicate was never wrong.
#
# Pinning the sentence would red on every rewording and is rightly avoided -- but pinning
# the SHAPE does not. These two lines survive a complete rewrite of the message and fail
# exactly when someone deletes the second step, which is the mutation that produced
# tonight. Deliberately weak, and say so rather than bank it: it cannot tell you the remedy
# is CORRECT, only that both addressees are still named. "The sideways-only form cannot
# silently return" is the whole claim.
#
# Proposed by sessionId 4a2f34f7-0669-487d-9ce9-39b77881642f, retracting their own "there
# may be no fix" -- which this file had already recorded as correct, and which was
# over-stated in the direction of giving up.
has "remedy names the AUTHOR step"   "$OUT" "AUTHOR"
has "remedy names the OPERATOR step" "$OUT" "OPERATOR"

# THE THIRD ADDRESSEE-SHAPED FAILURE, FOUND BY USING THE GUARD RATHER THAN BY READING IT.
#
# The two assertions above fixed a message that named ONE party. They cannot catch a
# message that names both parties and asks the WRONG QUESTION of the first. Shipped text
# read "ASK THE AUTHOR WHETHER IT IS WITHHELD", a binary -- and on the guard's first real
# refusal (2026-09-07) the author's true state was neither branch: not withheld, and not
# cleared either, because "push only when the user asks" is every session's standing
# instruction, so an author mid-task holds no authorisation to give. They replied "do not
# wait on me". A reader taking "not withheld" as clearance satisfies the guard and bypasses
# it in one step, and the old closing line -- "the author is the only one who can clear it"
# -- actively sent them to wait on that party.
#
# Same reasoning as above: the sentence is not pinned, the SHAPE is. This reds on deleting
# the third state and survives rewording it, and "not withheld is not cleared" is the whole
# claim. Note what made it findable: the predicate was correct, both addressees were named,
# and the suite was green -- it took a real author in the uncovered state to surface it.
# Reported by sessionId 8dba66b0-af4b-4cda-a333-54a0605b318e, who was that author.
has "remedy names the UNCLEARED third state" "$OUT" "UNCLEARED"

# THE RESOLUTION, WHICH THE GUARD WITHHELD WHILE CORRECTLY DESCRIBING THE PROBLEM.
#
# Every version up to now told you who to ask and never what the exit looks like. There is a
# mechanical one: an interleaved stack clears BY ITSELF, at any depth, because each commit
# becomes pushable by its own author as soon as the one below is published. Push yours, say
# "done", they push theirs. Nobody acks a foreign sid; nobody's operator is asked to authorise
# another session's work. Demonstrated 2026-09-07 on a six-deep stack across three sessions --
# two rungs in under a minute, after it had stood blocked with both parties correctly refusing.
#
# The eight-hour standoff in OB-20 was FOUR sessions failing to find this, and the guard written
# afterwards still did not name it: describing a blocker accurately is not the same as naming its
# exit, and a reader who has the diagnosis and no procedure waits. That is a distinct failure from
# the two above -- not a missing addressee, not an unanswerable question, but a correct message
# with no way out of it.
#
# Reds on deleting the ladder, survives rewording it. Property found by sessionId
# 8dba66b0-af4b-4cda-a333-54a0605b318e, by using the guard rather than reading it -- the second
# time that route beat review on this same file in one morning.
has "remedy names the LADDER resolution" "$OUT" "LADDER"

# THE LADDER'S TWO FAILURE MODES, BOTH FOUND ONE RUNG AFTER IT SHIPPED.
#
# The assertion above only checks the exit is named. It passed against text claiming the ladder
# "holds at any depth and any interleaving" -- an over-claim measured false within the hour.
#
# STALLS: the ladder needs every author CLEARED, not merely identified. An author in the
# not-withheld-but-UNCLEARED state cannot take their rung, and everything above it stalls. Then
# the ladder reverts to the original question, asked of YOUR operator. Found by being on the
# receiving end of it: sessionId 4eac25ba-b181-4dac-a5a1-ec88502a5bc5 declined to push their own
# rungs precisely because a peer cannot stand in for an operator -- the correct call, and the
# mirror of this guard's own rule.
#
# EXPIRES: a rung assignment is valid only at its derivation instant, and it decays SILENTLY
# TOWARD THE HARM. "I am last, blocking nobody" is true when formed; the natural next move for a
# session that believes it is last is to stop using refspecs, because being last is exactly when
# the branch name is safe. When the belief goes stale that push publishes everyone beneath. Argued
# by sessionId 8dba66b0-af4b-4cda-a333-54a0605b318e against my own "wait until it bites once",
# which was wrong here for a reason about the failure mode rather than about caution: waiting to
# observe it means the observation ARRIVES AS the incident. Same shape as the peer-count timestamp
# rule, which did not wait for one either. Corroborated the same morning by a five-commit ordering
# that re-ordered between being sent and being read.
#
# Two tokens, two deletions, two reds. Neither is pinned prose.
has "remedy names the STALLS precondition" "$OUT" "STALLS"
has "remedy names that a rung EXPIRES"    "$OUT" "EXPIRES"

echo
echo "== the ack is per-session, not a switch =="
run "$ALICE" "$BOB"   "refs/heads/main $TIP refs/heads/main $BASE"
eq  "acking the right sid allows"    "$EC" 0
run "$ALICE" "$CAROL" "refs/heads/main $TIP refs/heads/main $BASE"
eq  "acking a DIFFERENT sid still refuses" "$EC" 1
run "$ALICE" "all"    "refs/heads/main $TIP refs/heads/main $BASE"
eq  "ack=all allows"                 "$EC" 0

echo
echo "== two foreign sessions: the ack must name both =="
new_repo
commit "$ALICE" "base"; BASE=$(sha)
commit "$BOB"   "bob work"
commit "$CAROL" "carol work"; TIP=$(sha)
run "$ALICE" - "refs/heads/main $TIP refs/heads/main $BASE"
eq  "refused"                        "$EC" 1
# Order is git-log order (newest first), not commit order, so assert MEMBERSHIP rather
# than a concatenation — pinning the string would test the traversal, not the guard.
ACKLINE="$(printf '%s' "$OUT" | grep -F 'CODESCOUT_PUSH_ACK=' || true)"
has "prefilled ack names bob"        "$ACKLINE" "$BOB"
has "prefilled ack names carol"      "$ACKLINE" "$CAROL"
run "$ALICE" "$BOB" "refs/heads/main $TIP refs/heads/main $BASE"
eq  "acking one of two still refuses" "$EC" 1
run "$ALICE" "$BOB,$CAROL" "refs/heads/main $TIP refs/heads/main $BASE"
eq  "acking both allows"             "$EC" 0

echo
echo "== a commit with NO trailer is reported, never attributed =="
# REGRESSION, and the reason this file exists at its current shape. With a TAB
# delimiter, `sha<TAB><TAB>subject` collapses (tab is IFS whitespace) and the SUBJECT
# lands in the sid field: every untrailered commit was refused as foreign, "authored by"
# its own subject line. Observed red before the fix. The guard now uses 0x1F.
new_repo
commit "$ALICE" "alice base"; BASE=$(sha)
commit "-"      "a plain terminal commit"; TIP=$(sha)
run "$ALICE" - "refs/heads/main $TIP refs/heads/main $BASE"
eq  "allowed, not refused"           "$EC" 0
has "but reported"                   "$OUT" "no Session-Id trailer"
has "and named"                      "$OUT" "a plain terminal commit"
hasnt "subject never read as a sid"  "$OUT" "CODESCOUT_PUSH_ACK=\"a plain terminal commit\""
hasnt "and no refusal banner"        "$OUT" "REFUSING"

echo
echo "== untrailered AND foreign: report the one, refuse for the other =="
new_repo
commit "$ALICE" "alice base"; BASE=$(sha)
commit "-"      "terminal commit"
commit "$BOB"   "bob work"; TIP=$(sha)
run "$ALICE" - "refs/heads/main $TIP refs/heads/main $BASE"
eq  "refused"                        "$EC" 1
has "still reports the untrailered"  "$OUT" "no Session-Id trailer"
has "refuses for the foreign one"    "$OUT" "$BOB"

echo
echo "== a partial refspec publishes a PREFIX, and the guard follows it =="
# The escape that matters: the author of a bottom commit can always publish it alone,
# and nobody else can publish it by pushing their own work on top.
new_repo
commit "$ALICE" "alice base"; BASE=$(sha)
commit "$BOB"   "bob bottom"; BOB_C=$(sha)
commit "$ALICE" "alice above bob"; TIP=$(sha)
run "$BOB"   - "refs/heads/main $BOB_C refs/heads/main $BASE"
eq  "bob may publish bob's own commit"        "$EC" 0
run "$ALICE" - "refs/heads/main $BOB_C refs/heads/main $BASE"
eq  "alice may not publish it for him"        "$EC" 1
run "$ALICE" - "refs/heads/main $TIP refs/heads/main $BASE"
eq  "nor by pushing her own work above it"    "$EC" 1

echo
echo "== cases the guard must stay out of =="
new_repo
commit "$ALICE" "base"; BASE=$(sha)
commit "$BOB"   "bob work"; TIP=$(sha)
run "-"      - "refs/heads/main $TIP refs/heads/main $BASE"
eq  "no session id (a human): allowed, silent" "$EC" 0
eq  "  and truly silent"             "$(printf '%s' "$OUT" | wc -c)" 0
run "$ALICE" - "refs/heads/main $ZERO refs/heads/main $BASE"
eq  "branch deletion: allowed"       "$EC" 0
run "$ALICE" - "refs/tags/v1 $TIP refs/tags/v1 $ZERO"
eq  "tag push: allowed"              "$EC" 0
run "$ALICE" - ""
eq  "empty stdin: allowed"           "$EC" 0

echo
echo "== a brand-new remote branch does not enumerate all of history =="
# remote_sha is all-zeros for a branch the remote has never seen. Without `--not
# --remotes` the range is the whole history and the guard refuses on the first ancestor
# it meets, which would make every new branch unpushable.
new_repo
commit "$BOB"   "ancient, already published"
git -C "$REPO" update-ref refs/remotes/origin/main HEAD
commit "$ALICE" "new work"; TIP=$(sha)
run "$ALICE" - "refs/heads/main $TIP refs/heads/main $ZERO"
eq  "allowed: bob's published commit is not re-litigated" "$EC" 0

echo
echo "== the generated shim degrades OPEN when its target is missing =="
# WHY THIS IS HERE, AND WHY IT DOES NOT ASSERT AGAINST A HAND-WRITTEN SHIM.
#
# Everything above tests the guard SCRIPT. The shim in `.git/hooks/` is a separate
# member with, until this section, zero assertions — and the suite's own headline count
# concealed that: "34 passed" is an aggregate over the guard, and an aggregate reads as
# coverage for both members. That is this repo's population-vs-member law turned on its
# own test suite. Raised by sessionId ba061586-6581-4656-b0c5-acad83474de5, who ran the
# discrimination before saying so.
#
# What it guards: `exec` on a missing target exits 127, and git refuses on any non-zero
# hook exit, so losing the guard clause in `install_shim` would make EVERY push in the
# checkout fail with a bare "No such file or directory". The suite would have stayed
# green through that.
#
# It regenerates the shim from the REAL `scripts/install-hooks.sh`, copied byte-for-byte
# into a throwaway repo — `install_shim` is not sourceable, and the script cds to its own
# `$0/..`, so a copy is how you make it generate somewhere else. Asserting against a
# shim written here would be a second implementation checking itself; mutating
# `install_shim` must turn this red.
INSTALLER="$(cd "$(dirname "$0")/../scripts" && pwd)/install-hooks.sh"
new_repo
mkdir -p "$REPO/scripts" "$REPO/fakebin"
cp "$INSTALLER" "$REPO/scripts/install-hooks.sh"
chmod +x "$REPO/scripts/install-hooks.sh"
# A stub keeps `pre-commit install` from aborting the script in a repo with no config.
# The framework stage is not what this section is about.
printf '#!/bin/sh\nexit 0\n' > "$REPO/fakebin/pre-commit"
chmod +x "$REPO/fakebin/pre-commit"
git -C "$REPO" config --unset core.hooksPath 2>/dev/null || true
commit "$ALICE" "seed"

# The targets must EXIST at install time — `install_shim` refuses to wire a hook whose
# script is missing, which is a sound precondition and means the vanishing has to happen
# AFTER installation. That is the real hazard anyway: `git clean`, a branch switch, or a
# checkout predating the script, on a checkout where the hook is already wired.
for t in pre-push-foreign-session-guard post-index-change-stage-log; do
    printf '#!/usr/bin/env bash\necho "GUARD-RAN" >&2\nexit 1\n' > "$REPO/scripts/$t.sh"
    chmod +x "$REPO/scripts/$t.sh"
done
( cd "$REPO" && PATH="$REPO/fakebin:$PATH" bash scripts/install-hooks.sh ) > "$REPO/install.log" 2>&1

SHIM="$REPO/.git/hooks/pre-push"
TARGET="$REPO/scripts/pre-push-foreign-session-guard.sh"
# MUST run with cwd inside the throwaway. The shim resolves its repo at RUN time
# (`git rev-parse --show-toplevel`) — that is its documented feature, the one that makes
# it survive the checkout moving. Invoked from anywhere else it resolves to THIS repo and
# execs the live guard, so the assertions below silently measure the wrong repository:
# first written that way, and it reported exit 0 with empty output for every case.
fire() {
    OUT="$(cd "$REPO" && printf 'refs/heads/main %s refs/heads/main %s\n' "$(sha)" "$ZERO" \
        | CLAUDE_CODE_SESSION_ID="$ALICE" "$SHIM" origin git@example.invalid:x 2>&1)"
    EC=$?
}
if [ ! -x "$SHIM" ]; then
    no "installer produced a pre-push shim" "see $REPO/install.log"
else
    ok "installer produced a pre-push shim"

    # Case B first: the shim really delegates. Without this, Case A cannot tell
    # degrade-open from a shim that exits 0 unconditionally.
    fire
    eq  "target present: shim delegates"      "$EC" 1
    has "target present: guard actually ran"  "$OUT" "GUARD-RAN"
    hasnt "no spurious warning"               "$OUT" "missing or not executable"

    # Case A: the target vanishes after install.
    mv "$TARGET" "$TARGET.parked"
    fire
    eq  "target vanished: does NOT block"     "$EC" 0
    has "target vanished: says so, loudly"    "$OUT" "missing or not executable"
    # Correctly PAIRED, not load-bearing: this is monotone under the drop-the-clause
    # mutation (a bare `exec` on a missing target prints nothing either), so it cannot
    # fire on its own. It is here to stop a degrade-open shim that silently succeeds
    # while still somehow invoking the target. Do not credit it with catching the
    # mutation; #5 and #6 do that. Noted by sessionId ba061586-6581-4656-b0c5-acad83474de5.
    hasnt "and does not silently run nothing" "$OUT" "GUARD-RAN"

    # And it recovers rather than latching.
    mv "$TARGET.parked" "$TARGET"
    fire
    eq  "target restored: refuses again"      "$EC" 1
fi

echo
echo "== install-hooks.sh --check refuses a STALE shim, not just a missing one =="
# `--check` is the ONLY thing that ever inspects the installed artifact rather than the
# generator, and until 2026-09-06 its predicate was `grep -q "$target" "$dest"` -- "does
# the shim mention the target path". Every shim this repo has ever written mentions it,
# including the pre-degrade-open shape, so the predicate was monotone under exactly the
# drift that matters. A checkout wired before that clause and never re-installed keeps the
# `exec`-on-missing 127 trap while `--check` says ok. Measured by sessionId
# ba061586-6581-4656-b0c5-acad83474de5 in a throwaway before reporting it.
#
# This section is also `--check`'s FIRST caller. tests/hook_config.rs:263 records that
# nothing ran it -- "no CI job, no test, no task runner reference, only a line of prose" --
# so it was unreachable AND wrong in the same direction, and wiring up the caller without
# fixing the predicate would have wired up an instrument that answers ok.
new_repo
mkdir -p "$REPO/scripts" "$REPO/fakebin"
cp "$INSTALLER" "$REPO/scripts/install-hooks.sh"
chmod +x "$REPO/scripts/install-hooks.sh"
# The stub must WRITE the framework's hook, not just exit 0. `--check`'s framework arm
# looks for `generated by pre-commit` in .git/hooks/pre-commit, so a stub that installs
# nothing makes the script's aggregate exit 1 for a reason unrelated to the shim -- which
# would have made every assertion below pass or fail on the wrong cause.
cat > "$REPO/fakebin/pre-commit" <<'PC'
#!/bin/sh
[ "${1:-}" = "install" ] || exit 0
d="$(git rev-parse --git-dir)/hooks"
mkdir -p "$d"
printf '#!/bin/sh\n# generated by pre-commit\nexit 0\n' > "$d/pre-commit"
chmod +x "$d/pre-commit"
PC
chmod +x "$REPO/fakebin/pre-commit"
for t in pre-push-foreign-session-guard post-index-change-stage-log; do
    printf '#!/usr/bin/env bash\nexit 0\n' > "$REPO/scripts/$t.sh"
    chmod +x "$REPO/scripts/$t.sh"
done
commit "$ALICE" "seed"
# new_repo sets core.hooksPath=/dev/null so throwaway commits run no hooks; install-hooks.sh
# correctly REFUSES while it is set (that is its documented hooksPath trap). Unset it after
# the seed commit, before installing. Omitting this makes every assertion below fail on the
# refusal banner rather than on anything to do with shims.
git -C "$REPO" config --unset core.hooksPath 2>/dev/null || true
( cd "$REPO" && PATH="$REPO/fakebin:$PATH" bash scripts/install-hooks.sh ) >/dev/null 2>&1

# Asserts BOTH the aggregate exit (what a real caller reads) and the pre-push line (the
# member this section is about). The aggregate alone would let an unrelated hook's failure
# masquerade as this one's -- which is the same population/member confusion that put this
# section here in the first place.
check() {
    OUT="$( cd "$REPO" && PATH="$REPO/fakebin:$PATH" bash scripts/install-hooks.sh --check 2>&1 )"
    EC=$?
}

check
eq  "a freshly installed shim passes --check"   "$EC" 0
has "and says it matched the generator"         "$OUT" "shim matches generator"

# Plant the pre-2026-09-06 shape: mentions the target, has no degrade-open clause.
STALE_SHIM="$REPO/.git/hooks/pre-push"
cat > "$STALE_SHIM" <<'OLD'
#!/usr/bin/env bash
# Installed by scripts/install-hooks.sh. Thin shim: edit the tracked script, not this.
set -uo pipefail
root="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0
exec "$root/scripts/pre-push-foreign-session-guard.sh" "$@"
OLD
chmod +x "$STALE_SHIM"

# THE DISCRIMINATION. First establish that the OLD predicate is satisfied by this file --
# otherwise the new check passing proves nothing about being stronger.
if grep -q "scripts/pre-push-foreign-session-guard.sh" "$STALE_SHIM"; then
    ok "the stale shim satisfies the OLD grep predicate"
else
    no "the stale shim satisfies the OLD grep predicate" "fixture no longer reproduces the bug"
fi
check
eq  "but --check now REFUSES it"                "$EC" 1
has "and names it stale, not missing"           "$OUT" "STALE"
has "and says it is not inert"                  "$OUT" "NOT inert"
# The FOOTER is the line a skimmer reads, and it collapsed stale into missing after the
# per-hook lines stopped doing so — erring toward the reassuring reading, since "not
# installed" implies inert while the truth is "running the older shape". This run has ZERO
# missing hooks, so the footer has no honest way to say "NOT installed".
hasnt "footer does not call a stale hook missing" "$OUT" "hooks are NOT installed"
has  "footer names the stale state instead"       "$OUT" "hooks are STALE"

# A missing shim must stay distinguishable from a stale one: different remedy, different
# danger, and collapsing them is how the old predicate was reassuring about the worse case.
rm -f "$STALE_SHIM"
check
eq  "a missing shim also refuses"               "$EC" 1
has "but is reported as MISSING"                "$OUT" "MISSING pre-push"
hasnt "not as stale"                            "$OUT" "STALE   pre-push"

# And it recovers: re-installing makes --check pass again.
( cd "$REPO" && PATH="$REPO/fakebin:$PATH" bash scripts/install-hooks.sh ) >/dev/null 2>&1
check
eq  "re-installing clears it"                   "$EC" 0

echo
echo "-------------------------------------------"
echo "  $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] || exit 1
