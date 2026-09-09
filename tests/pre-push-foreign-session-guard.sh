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
    # `timeout` is load-bearing, not defensive tidiness. The guard's refusal banner is an
    # UNQUOTED heredoc, so a stray backtick in its prose becomes a command substitution that
    # bash runs while expanding it -- and if that command blocks, `cat` never completes and
    # this capture waits forever. Without the timeout a hang HANGS THE SUITE rather than
    # redding it, which is strictly worse than a failure: no assertion reports, no exit code
    # is produced, and CI shows a job that never finished instead of a test that failed.
    # Measured 2026-09-07, docs/issues/2026-09-07-the-pre-push-guards-refusal-text-executes-its-own-example-commands.md.
    OUT="$(printf '%s\n' "$line" | (cd "$REPO" && timeout 20 env -u CLAUDE_CODE_SESSION_ID -u CODESCOUT_PUSH_ACK "${env[@]}" "$GUARD" origin git@example.invalid:x) 2>&1)"
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
# THE READER WITH NO RUNG, FOUND IN THE GUARD'S FIRST REAL ACK-USE.
#
# The three assertions above check that the exit is NAMED and BOUNDED. None checks that the reader
# is ROUTED to the right one of them. The ladder and the rev-list advice both assume the reader is
# AT a rung; a reader on TOP of a stack whose authors are all uncleared has neither a rung nor a
# refspec, and the text reaches the ladder first -- correct in general and a dead end here. In that
# configuration the ack is not the exception it reads as further down: it is the ONLY route, and it
# goes to their operator.
#
# This is a distinct failure from the three above, and the distinction is what earns a fourth
# assertion: not a missing addressee, not an unanswerable question, not an unnamed exit, but a
# correctly-named exit the reader is steered PAST. A message can hold every route and still route
# you wrongly.
#
# Observed rather than reasoned -- it is the exact shape of this guard's first real ack-use: three
# authors, one uncleared, the pusher on top of the stack. Reported by sessionId
# 8dba66b0-af4b-4cda-a333-54a0605b318e, who was that pusher.
has "remedy routes a reader with NO RUNG to the ack" "$OUT" "NO RUNG AND NO REFSPEC"
# THE AUTHORISATION ITSELF DECAYS, AND THE BRANCH NAME HIDES IT.
#
# Every assertion above is about the refusal: who is named, which exit, whether the reader is
# routed to it. None is about what happens AFTER an operator says yes. An authorisation names a
# SET of commits; `git push origin <branch>` sends a PREFIX; they coincide only if nothing lands
# in between. On a five-session tree that is the unusual case.
#
# The failure needs no error from anyone. Ordinary churn opens it, the branch name still reads as
# "the thing I was told to push", and it fails toward publishing work the operator never saw --
# silent, and toward the harm. Measured 2026-09-07 with a ninety-second window: a fourth commit
# from another session landed on top of an authorised three-commit stack while the operator was
# answering. Caught only by comparing the decided set against the live range before pushing.
#
# This is the FIFTH distinct defect in this remedy text and the first that is not about the
# refusal at all -- it is about the state the reader enters once the guard has been satisfied.
# A guard that is correct up to the moment it stops applying is still leaving a cliff there.
has "remedy warns that an authorisation NAMES A SET, not a prefix" "$OUT" "AN AUTHORISATION NAMES A SET"



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
#
# THE STUB LIST BELOW IS LOAD-BEARING AND GREW ON 2026-09-07. `install-hooks.sh` gained a
# pre-commit shim when the framework was retired (074b749e), so `--check` now resolves a THIRD
# target, `scripts/pre-commit-run.sh`. Omit it from this fixture and the aggregate exits 1 while
# the pre-push line still reports `shim matches generator` -- an unrelated hook`s failure wearing
# this section`s name, which is the exact confusion the paired assertion below exists to catch.
# It caught it. Add a stub here whenever install-hooks.sh learns a new target.
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
# `prepare-commit-msg-session-id` joined this list on 2026-09-07: `--check`'s opt-in arm now
# routes through `install_shim`, so it resolves that target too. Without the stub it reports
# MISSING and fails the aggregate for a reason unrelated to whatever section is asserting.
for t in pre-push-foreign-session-guard post-index-change-stage-log pre-commit-run prepare-commit-msg-session-id; do
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
echo "== --check byte-compares the OPT-IN hook too, not just the unconditional ones =="
# `prepare-commit-msg` is opt-in (`--with-session-id`), so `--check` reports it from a SEPARATE
# `elif` arm rather than through `install_shim`. That arm asked `[ -x "$dest" ]` -- presence --
# while every other hook had moved to a byte-comparison against `render_shim` on 2026-09-06. The
# fix did not reach it because it is not an `install_shim` call site, which is the whole shape of
# the defect: a law implemented at N sites, repaired at N-1.
#
# Measured 2026-09-07 on a real checkout, seconds apart, same shim:
#     scripts/install-hooks.sh --check                     -> ok      prepare-commit-msg
#     scripts/install-hooks.sh --check --with-session-id   -> STALE   prepare-commit-msg
# and `grep -c 'DEGRADE OPEN' .git/hooks/prepare-commit-msg` returned 0 -- the installed shim
# genuinely predated the clause and carried the exec-on-missing-target 127 trap. The reassuring
# word was on the worse state, and the footer stayed silent because `fail` never set.
#
# The two assertions below are the discrimination: the flagged form was ALREADY correct, so
# asserting only on it would prove nothing. It is the unflagged form that regressed, and both
# must now agree.
STALE_PCM="$REPO/.git/hooks/prepare-commit-msg"
cat > "$STALE_PCM" <<'OLD'
#!/usr/bin/env bash
# Installed by scripts/install-hooks.sh. Thin shim: edit the tracked script, not this.
set -uo pipefail
root="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0
exec "$root/scripts/prepare-commit-msg-session-id.sh" "$@"
OLD
chmod +x "$STALE_PCM"

# Establish the OLD predicate is satisfied, or "the new check refuses it" proves nothing about
# being stronger -- same reasoning as the pre-push section above.
if [ -x "$STALE_PCM" ]; then
    ok "the stale opt-in shim satisfies the OLD -x predicate"
else
    no "the stale opt-in shim satisfies the OLD -x predicate" "fixture no longer reproduces"
fi

check
eq  "--check WITHOUT the flag refuses it"        "$EC" 1
has "and names prepare-commit-msg stale"         "$OUT" "STALE   prepare-commit-msg"

OUT="$( cd "$REPO" && PATH="$REPO/fakebin:$PATH" bash scripts/install-hooks.sh --check --with-session-id 2>&1 )"
has "and the flagged form agrees with it"        "$OUT" "STALE   prepare-commit-msg"

# Absent must stay distinguishable from stale, and must NOT fail the run: this stage is opt-in,
# so a checkout that never asked for it is healthy. Reporting MISSING here would make the
# fix louder than the defect.
rm -f "$STALE_PCM"
check
has "an absent opt-in hook reports off"          "$OUT" "off     prepare-commit-msg"
hasnt "not as missing"                           "$OUT" "MISSING prepare-commit-msg"
eq  "and does not fail the run"                  "$EC" 0

echo
echo "== the refusal banner is TEXT, never a program =="
# REGRESSION. `cat >&2 <<EOF` is unquoted -- deliberately, because the banner interpolates
# $foreign_report, $me, $branch and $foreign_sids -- so a backtick in its PROSE is not
# formatting, it is a command substitution bash runs while expanding the message. Shipped
# 2026-09-07 at 41377049 as markdown habit around an inline example: `git push origin
# $branch`. On the real repo that executed a push, which re-fired pre-push, which re-expanded
# the banner: 45 processes and no banner ever printed, so the authorisation question OB-20
# exists to force was silently never asked.
#
# WHY THE 69-ASSERTION SUITE WAS GREEN THROUGHOUT, which is the part worth keeping: the
# throwaway repo has NO `origin` remote, so the injected `git push origin main` failed
# instantly and its stderr vanished into the substitution. The bug needs a reachable remote
# to recurse, and this harness structurally cannot have one -- cluster/repro-env-diverges-
# from-gate-env. So "assert the banner is present" is NOT the test: it passes under the bug,
# because a failed substitution still lets `cat` finish. What discriminates is asserting the
# example survives as LITERAL TEXT -- under the bug it is replaced by the substitution's
# (empty) output, which is exactly the mutation a future prose edit would reintroduce.
new_repo
commit "$ALICE" "alice base"; BASE=$(sha)
commit "$BOB"   "bob's commit"; TIP=$(sha)
run "$ALICE" - "refs/heads/main $TIP refs/heads/main $BASE"
eq  "refused"                                    "$EC" 1
has "and the banner actually printed"            "$OUT" "REFUSING THE PUSH"
# The discriminating assertion. Single-quoted so THIS file does not substitute it either.
has "inline example survived as literal text"    "$OUT" '`git push origin main`'

echo
echo "== no unescaped backtick survives in any unquoted heredoc body =="
# The site-specific assertion above cannot see the NEXT inline example someone adds. This is
# the class-level guard: scan every unquoted heredoc body for a backtick that is not
# backslash-escaped.
#
# THE SCANNER IS DELIMITER-AGNOSTIC, and that is a fix rather than a flourish. It used to pin
# the literal `<<EOF`, so a second banner opened as `<<WARN` was silently never scanned while
# the old non-vacuity control still passed on the strength of the first body. Subtraction was
# caught, addition was not. See docs/issues/2026-09-09-the-heredoc-scanner-sees-one-delimiter
# -and-its-control-is-blind-to-an-added-opener.md.
#
# Defined as a function so the POSITIVE CONTROL below exercises this exact code rather than a
# re-implementation of it — a second copy asserting about itself is indistinguishable from
# coverage until you break the one that ships.
scan_heredoc_bodies() {   # $1 = file
    awk '
        !inb && /<<[A-Za-z_][A-Za-z_0-9]*$/ { d = $0; sub(/^.*<</, "", d); inb = 1; next }
        inb && $0 == d                     { inb = 0; next }
        inb                                { print }
    ' "$1"
}
HEREDOC_BODY="$(scan_heredoc_bodies "$GUARD")"
LIVE_TICKS="$(printf '%s\n' "$HEREDOC_BODY" | grep -nE '(^|[^\\])`' || true)"
is_empty() { [ -z "$2" ] && ok "$1" || no "$1" "unescaped backtick(s) in a heredoc body:
$2"; }
is_empty "no heredoc body has a live backtick"   "$LIVE_TICKS"

# CONTROL 1 — COVERAGE, not size. The floor this replaces was `BODY_LINES >= 100` against a
# 114-line banner, which encodes "the banner is long" and not "the scanner is live"; the two
# coincided by 14 lines. It reddened on an ordinary prose trim and stayed green on an added
# opener, and its failure text named a cause ("the selector is stale") it had not measured.
#
# Counted with a DELIBERATELY DIFFERENT expression from the scanner's own regex, so this is
# not the scanner agreeing with itself. The broad form admits the `<<-` indented variant and
# a trailing space; the scanner's does not. Quoted openers (`<<'EOF'`) are excluded from both
# on purpose — they do not interpolate, so they cannot substitute, and counting them would
# red on a construct that is safe by definition.
UNQUOTED_OPENERS="$(grep -cE '<<-?[A-Za-z_][A-Za-z_0-9]*[[:space:]]*$' "$GUARD" || true)"
SCANNED_OPENERS="$(awk '!inb && /<<[A-Za-z_][A-Za-z_0-9]*$/{n++; inb=1; d=$0; sub(/^.*<</,"",d); next} inb && $0==d{inb=0} END{print n+0}' "$GUARD")"
[ "${SCANNED_OPENERS:-0}" -eq "${UNQUOTED_OPENERS:-0}" ] \
    && ok "the scanner reaches every unquoted heredoc ($SCANNED_OPENERS of $UNQUOTED_OPENERS)" \
    || no "the scanner reaches every unquoted heredoc" "scanned $SCANNED_OPENERS of $UNQUOTED_OPENERS unquoted heredoc opener(s) — one is outside the selector, so its body was never checked above"

# CONTROL 2 — POSITIVE, by mutating the production scanner's INPUT. "Never selects the wrong
# one" and "never selects one at all" are the same assertion until something pins the
# accepting case (.codescout/memories/test-design-discipline.md). So inject one live backtick
# into every body of a COPY and require the scanner to find one hit per body. Invariant to
# banner length and to rewording; red exactly when the selector goes stale or an opener form
# appears that it cannot see.
MUT="$(mktemp)"
awk '
    !inb && /<<[A-Za-z_][A-Za-z_0-9]*$/ { d = $0; sub(/^.*<</, "", d); print; print "  injected `probe` line"; inb = 1; next }
    inb && $0 == d                     { inb = 0 }
    { print }
' "$GUARD" > "$MUT"
MUT_HITS="$(scan_heredoc_bodies "$MUT" | grep -cE '(^|[^\\])`' || true)"
[ "${MUT_HITS:-0}" -eq "${UNQUOTED_OPENERS:-0}" ] \
    && ok "and it detects an injected backtick in every body ($MUT_HITS of $UNQUOTED_OPENERS)" \
    || no "and it detects an injected backtick in every body" "found $MUT_HITS of $UNQUOTED_OPENERS injected ticks — the scanner is not reading the bodies it appears to"
rm -f "$MUT"

echo
echo "== install-hooks.sh writes into the hooks dir git READS, from a linked worktree =="
# WHY THIS EXISTS. Until 2026-09-09 `install_shim` wrote to "$git_dir/hooks/$hook_name". In
# a linked worktree --git-dir is .git/worktrees/<name>, which has no hooks/ directory at
# all, while git READS hooks from the common dir. Every write failed -- and because
# install-hooks.sh runs `set -uo pipefail` with no `-e`, the unconditional
# `echo "ok ... shim installed"` still printed three times and the script exited 0. No hook
# anywhere, reported as installed. `--check` from the same cwd then said MISSING and its
# printed remedy is the run that prints ok, so the two halves formed a loop with no exit and
# each half was locally correct.
#
# THE LINKED WORKTREE IS THE LOAD-BEARING DETAIL OF THIS FIXTURE. Every other installer
# section in this file uses a plain `git init` repo, where --git-dir and --git-path hooks
# COINCIDE -- so the divergence cannot arise there and no assertion added there can ever
# reach this, however many are written. Delete the `git worktree add` below and this whole
# section still passes while testing nothing.
#
# ITS OBSERVED RED: restore `dest="$git_dir/hooks/$hook_name"` in scripts/install-hooks.sh
# and the three PRESENT assertions plus the record-vs-reality pairing below all fail.
# Observed 2026-09-09 against the production script before this section shipped.

# ABSOLUTE, ALWAYS. `git rev-parse --git-path hooks` returns a RELATIVE `.git/hooks` from a
# main checkout and an ABSOLUTE path from a worktree, so the bare value means different
# things depending on who reads it and from where. Measured 2026-09-09: a verification
# script for this very fix took the relative form and ran `rm -f "$hooks/pre-push"` from
# another directory, deleting three live hooks out of the real checkout. Resolve once, here.
hooks_dir_of() {
    local d
    d="$(git -C "$1" rev-parse --git-path hooks)"
    case "$d" in /*) printf '%s\n' "$d" ;; *) printf '%s\n' "$1/$d" ;; esac
}

# No fakebin/pre-commit stub here, unlike the sections above: install-hooks.sh retired the
# framework and now names `pre-commit` only inside an error string, so that stub is inert.
installer_fixture() {  # -> $REPO, seeded and committed, with all three targets executable
    new_repo
    mkdir -p "$REPO/scripts"
    cp "$INSTALLER" "$REPO/scripts/install-hooks.sh"
    chmod +x "$REPO/scripts/install-hooks.sh"
    for t in pre-commit-run post-index-change-stage-log pre-push-foreign-session-guard; do
        printf '#!/usr/bin/env bash\nexit 0\n' > "$REPO/scripts/$t.sh"
        chmod +x "$REPO/scripts/$t.sh"
    done
    git -C "$REPO" config --unset core.hooksPath 2>/dev/null || true
    git -C "$REPO" add -A
    git -C "$REPO" commit -q -m "seed with scripts"
}

installer_fixture
WT="$REPO.wt"
git -C "$REPO" worktree add -q -b wt "$WT" 2>/dev/null
WT_GITDIR="$(git -C "$WT" rev-parse --git-dir 2>/dev/null)"
WT_HOOKS="$(hooks_dir_of "$WT" 2>/dev/null)"

# THE FIXTURE ASSERTS ITS OWN DISCRIMINATING PROPERTY FIRST. If these two ever coincide the
# section below is vacuous -- it would pass identically against the broken script -- and
# nothing else here would say so. Red on a `worktree add` that silently did not happen.
if [ -n "$WT_GITDIR" ] && [ "$WT_GITDIR/hooks" != "$WT_HOOKS" ]; then
    ok "fixture: a linked worktree's --git-dir diverges from --git-path hooks"
else
    no "fixture: a linked worktree's --git-dir diverges from --git-path hooks" \
       "git-dir=$WT_GITDIR hooks=$WT_HOOKS -- the fixture cannot express the defect"
fi

( cd "$WT" && bash scripts/install-hooks.sh ) > "$WT.log" 2>&1
WT_EC=$?
eq "worktree install exits 0" "$WT_EC" 0

WT_PRESENT=0
for h in pre-commit post-index-change pre-push; do
    if [ -x "$WT_HOOKS/$h" ]; then
        ok "worktree install: $h landed where git reads hooks"
        WT_PRESENT=$((WT_PRESENT + 1))
    else
        no "worktree install: $h landed where git reads hooks" \
           "absent from $WT_HOOKS -- see $WT.log"
    fi
done

# PAIRED, and not load-bearing on its own: it is monotone under removal, since an installer
# that writes nothing anywhere also leaves this directory absent. It is here to catch the
# other repair someone reaches for -- `mkdir -p "$git_dir/hooks"` -- which makes the three
# assertions above red and this one the only witness that a hook git never reads was written.
if [ ! -d "$WT_GITDIR/hooks" ]; then
    ok "worktree install: nothing written into the worktree's private gitdir"
else
    no "worktree install: nothing written into the worktree's private gitdir" \
       "$WT_GITDIR/hooks exists -- git does not read hooks from there"
fi

# RECORD VS REALITY. This is the assertion the defect was actually about: the script printed
# three `ok ... shim installed` lines with zero shims behind them. Equality alone is
# satisfied by 0 == 0, so the count is pinned too -- one of each, in both directions.
WT_OK="$(grep -c 'shim installed' "$WT.log" || true)"
if [ "${WT_OK:-0}" -eq 3 ] && [ "$WT_PRESENT" -eq 3 ]; then
    ok "every 'shim installed' line has a shim behind it ($WT_OK claimed, $WT_PRESENT present)"
else
    no "every 'shim installed' line has a shim behind it" \
       "$WT_OK claimed installed, $WT_PRESENT actually present -- see $WT.log"
fi

# And the loop closes: --check from the same cwd must now agree with the install that
# preceded it. Before the fix this printed MISSING for all three and exited 1.
( cd "$WT" && bash scripts/install-hooks.sh --check ) > "$WT.check.log" 2>&1
eq "worktree --check agrees with the install that just ran" "$?" 0

echo
echo "== a failed write does not print ok, and does not exit 0 =="
# INDEPENDENT OF THE PATH. Fixing only the destination leaves the defect underneath it: the
# success line was never conditional on the write. `set -uo pipefail` omits `-e`, so a failed
# `cat >` neither stops install_shim nor sets `fail`. This case removes write permission
# instead of moving the path, so it stays meaningful even if the hooks dir is resolved some
# third way later.
installer_fixture
FAIL_HOOKS="$(hooks_dir_of "$REPO")"
chmod a-w "$FAIL_HOOKS"
if [ "$(id -u)" = "0" ] || ( : > "$FAIL_HOOKS/.probe" ) 2>/dev/null; then
    # Root ignores the mode bits, so the fixture cannot express a failed write. Say so out
    # loud rather than passing: a silent skip is indistinguishable from a green assertion.
    rm -f "$FAIL_HOOKS/.probe" 2>/dev/null
    no "unwritable hooks dir: install reports FAILED, not ok" \
       "SKIPPED -- $FAIL_HOOKS is still writable (running as uid $(id -u)); this case needs a non-root user"
else
    ( cd "$REPO" && bash scripts/install-hooks.sh ) > "$REPO/fail.log" 2>&1
    FAIL_EC=$?
    FAIL_OK="$(grep -c 'shim installed' "$REPO/fail.log" || true)"
    FAIL_LOUD="$(grep -c '^FAILED' "$REPO/fail.log" || true)"
    [ "${FAIL_OK:-1}" -eq 0 ] \
        && ok "unwritable hooks dir: prints no 'shim installed'" \
        || no "unwritable hooks dir: prints no 'shim installed'" \
              "$FAIL_OK ok-line(s) with nothing written -- see $REPO/fail.log"
    [ "${FAIL_LOUD:-0}" -ge 1 ] \
        && ok "unwritable hooks dir: says FAILED and names the path" \
        || no "unwritable hooks dir: says FAILED and names the path" "see $REPO/fail.log"
    eq "unwritable hooks dir: exits non-zero" "$FAIL_EC" 1
fi
chmod u+w "$FAIL_HOOKS" 2>/dev/null || true

echo
echo "-------------------------------------------"
echo "  $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] || exit 1
