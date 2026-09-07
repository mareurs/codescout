#!/usr/bin/env bash
#
# Refuse a push that would publish another session's commits, until someone says so.
#
# WHY THIS EXISTS
# ---------------
# Several Claude Code sessions share this checkout and commit to `experiments`. A commit
# a session is deliberately WITHHOLDING — pending its operator's say-so — is byte-identical
# to one merely not-yet-pushed. Nothing in git records the difference, because authorisation
# lives in an operator conversation no peer can read. So any peer's `git push` publishes it,
# the push succeeds, the diff is correct, and nothing looks wrong afterwards.
#
# Measured 2026-09-06: exactly that happened. The pusher had run
# `git log origin/experiments..HEAD --stat` BEFORE pushing — more diligence than any stated
# rule asks — and it could not have helped, because that command answers *what am I sending*
# and the question that mattered was *may its author publish it*.
#
# Full class: docs/trackers/observer-blindness.md OB-20.
#
# WHAT THIS GUARD CAN AND CANNOT DO
# ---------------------------------
# It cannot decide authorisation. Nothing can — the fact is not in the substrate. What it
# does is make the QUESTION unskippable at the only moment it is answerable, which is the
# most a mechanism can do for a fact that lives outside the repository.
#
# It is the complement to the rule "a session that cannot publish must not COMMIT to a
# shared branch". That rule needs no coordination and is the right primary defence, but it
# is silent on the pusher — and the pusher is the party who acts. This covers the case the
# rule exists for: someone forgot.
#
# WHY THE `Session-Id` TRAILER IS THE DISCRIMINATOR
# -------------------------------------------------
# `%an` is the same human on every commit in this checkout, so git's own author field
# carries zero ownership signal here. `scripts/prepare-commit-msg-session-id.sh` stamps a
# `Session-Id:` trailer on every commit made by a session; it has been unconditional since
# 2026-09-04 and the field is what separates us.
#
# Read through `%(trailers:key=...)` rather than `grep '^Session-Id:'`. Some commits carry
# the line TWICE — a hand-written trailer above the `Co-Authored-By` block opens a second
# trailer paragraph that the stamping hook's `--if-exists doNothing` does not inspect
# (48 of 802 commits, measured 2026-09-06). git's parser returns the single correct value
# on those; a grep returns two and would report a phantom disagreement.
#
# WHY IT READS STDIN RATHER THAN `@{upstream}..HEAD`
# --------------------------------------------------
# git hands a pre-push hook the refs actually being pushed, one per line:
#
#     <local ref> <local sha> <remote ref> <remote sha>
#
# That is strictly better than assuming the branch tip, because `git push origin
# <sha>:experiments` publishes a PREFIX — that commit and all its ancestors, nothing above
# it. Verified 2026-09-06 by enumerating `origin/experiments..<sha>` for three targets.
# A hook keyed on HEAD would refuse a partial push that is entirely legitimate, and would
# miss the range a partial push actually sends. Reading stdin gets both right for free.
#
# WHAT IT DELIBERATELY DOES NOT REFUSE
# ------------------------------------
#   - A commit with NO trailer. Commits made from a plain terminal get none, by design
#     ("absence is honest; a default would be a fabricated owner"). Refusing them would
#     break every non-session push, so they are REPORTED and allowed. Stated here rather
#     than left to be discovered: this guard cannot see a withheld commit that carries no
#     trailer, and that is a real hole, not an oversight.
#   - Anything, when the pusher has no CLAUDE_CODE_SESSION_ID. Without one there is no
#     "mine" to compare against, so the guard has no predicate — a human running the
#     release flow is exactly this case, and must not be blocked by a guard that cannot
#     even form its question.
#   - Non-branch refs (tags, notes). Nothing here is about them.

set -uo pipefail

ZERO="0000000000000000000000000000000000000000"

me="${CLAUDE_CODE_SESSION_ID:-}"
if [ -z "$me" ]; then
    # No predicate — see "WHAT IT DELIBERATELY DOES NOT REFUSE". Silent: a human running
    # the release flow should not be told about a guard that does not apply to them.
    exit 0
fi

# The escape hatch. Naming the sids is the point: a blanket bypass would let the guard be
# turned off by habit, and the ack is meant to record a decision, not dismiss a prompt.
#   CODESCOUT_PUSH_ACK="<sid>[,<sid>...]"   authorise these specific sessions' commits
#   CODESCOUT_PUSH_ACK="all"                authorise whatever is in this push
ack="${CODESCOUT_PUSH_ACK:-}"

acked() {
    [ "$ack" = "all" ] && return 0
    case ",$ack," in
        *",$1,"*) return 0 ;;
        *) return 1 ;;
    esac
}

foreign_sids=""
foreign_report=""
untrailered_report=""
untrailered_n=0

while read -r local_ref local_sha _remote_ref remote_sha; do
    [ -n "${local_sha:-}" ] || continue
    [ "$local_sha" = "$ZERO" ] && continue          # branch deletion
    case "$local_ref" in refs/heads/*) ;; *) continue ;; esac

    if [ "$remote_sha" = "$ZERO" ]; then
        # New remote branch: everything not already on some remote. Without --not this
        # would enumerate the entire history and refuse on the first ancestor it met.
        range=("$local_sha" --not --remotes)
    else
        range=("$remote_sha..$local_sha")
    fi

    # One `git log` for the whole range, not one per commit: this runs on every push and a
    # guard people find slow is a guard people uninstall.
    #
    # DELIMITER IS 0x1F (unit separator), NOT TAB, and that is load-bearing. Tab is IFS
    # *whitespace*, and bash collapses runs of IFS-whitespace into a single delimiter — so
    # a commit with no trailer emits `sha<TAB><TAB>subject`, the two tabs read as one, and
    # the SUBJECT lands in `sid`. Every untrailered commit was then reported as foreign,
    # authored by its own subject line. Caught by the T10 case in
    # `tests/pre-push-foreign-session-guard.sh`; keep that case if you touch this line.
    # 0x1F is not whitespace, so empty fields survive.
    while IFS=$'\x1f' read -r sha sid subject; do
        [ -n "${sha:-}" ] || continue
        if [ -z "${sid:-}" ]; then
            untrailered_n=$((untrailered_n + 1))
            untrailered_report="${untrailered_report}    ${sha:0:8}  ${subject}"$'\n'
        elif [ "$sid" != "$me" ]; then
            acked "$sid" && continue
            case ",$foreign_sids," in
                *",$sid,"*) ;;
                *) foreign_sids="${foreign_sids:+$foreign_sids,}$sid" ;;
            esac
            foreign_report="${foreign_report}    ${sha:0:8}  ${sid}  ${subject}"$'\n'
        fi
    done < <(git log --format='%H%x1f%(trailers:key=Session-Id,valueonly,separator=%x2C)%x1f%s' "${range[@]}" 2>/dev/null)
done

if [ "$untrailered_n" -gt 0 ]; then
    printf '\n  note: %d commit(s) in this push carry no Session-Id trailer, so this guard\n' "$untrailered_n" >&2
    printf '  cannot tell whose they are. Allowed, not vouched for:\n\n%s\n' "$untrailered_report" >&2
fi

[ -n "$foreign_report" ] || exit 0

branch="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo '<branch>')"

cat >&2 <<EOF

  REFUSING THE PUSH: it would publish commits belonging to another session.

$foreign_report
  Your session id:  $me

  This is not a claim that anything is wrong. A commit another session is holding
  back pending its operator's decision looks exactly like one it simply has not
  pushed yet, and git records nothing that tells them apart. This guard exists so
  the question gets asked at the one moment it can still be answered.

  ASK THE AUTHOR WHICH OF THREE STATES IT IS IN — three, not two, and the middle one
  is the common case. That is the fact only they hold:

    withheld              pending their operator's decision. You hold.
    not withheld, UNCLEARED   ordinary mid-task work, and they have no push
                          authorisation of their own either, because "push only when
                          the user asks" is every session's standing instruction.
    cleared               their operator said push; they simply have not yet.

  "Not withheld" is an answer about their INTENT. It is never an answer about your
  authorisation, and reading it as one is how this guard gets satisfied and bypassed
  in the same breath.

  The sid above is the address; resolve it to a live session with:

      scripts/peer-sessions.sh

  THEN ASK YOUR OPERATOR, and do not stop at the author. A peer can tell you what
  they were told; a peer CANNOT grant. Routing the ask sideways is what turns this
  into a standoff — measured 2026-09-06: four sessions held for eight hours, each
  correctly refusing to decide something none of them had the authority to decide.
  It resolved in one exchange the moment the question reached a person, with the
  specific consequence attached: which commits, whose, and the fact that no refspec
  can skip an ancestor so there is no push-only-mine. Where one human operates every
  session, your operator IS the author's operator — which no session can tell from
  the inside, and is why this is asked rather than inferred.

  DO NOT COMPRESS THAT INTO "same operator, just push". It is true of some checkouts
  and false of one shared by two humans, where the two steps still hold and your
  operator may simply say no — which is a complete answer, not a failure of the
  procedure. The clause earns its place by making the question ASKABLE, never by
  predicting how it is answered.

  If the author is gone, the commits are already unowned and pushing them is the
  least-bad option — ack and say so in your next commit message.

  IF IT IS CLEARED, name the sessions you are authorised to publish:

      CODESCOUT_PUSH_ACK="$foreign_sids" git push <args>

  TO PUBLISH ONLY YOUR OWN WORK, if the foreign commits are stacked ABOVE it:

      git push origin <your-last-sha>:$branch

  That sends that commit and all its ANCESTORS and nothing above it. It cannot skip
  a commit BELOW yours — if the foreign commit is underneath, there is no refspec
  that helps. AND THE AUTHOR CANNOT CLEAR IT EITHER, unless their operator has
  already said so. Measured 2026-09-07 on this guard's first real refusal: the author
  answered "not withheld" and "do not wait on me" in one message, holding no push
  authorisation themselves. This line previously read "the author is the only one who
  can clear it", which sends you to wait on a party who may be waiting on a person
  too — the same standoff this guard exists to prevent, wearing the guard's own text.

  What the author CAN do is push their own work, which needs nothing from you. Ask
  them to, and ask your own operator in parallel.

  THE STACK IS A LADDER, AND IT CLEARS WITH ZERO ACKS. That is the resolution, and it
  holds at any depth and any interleaving: each commit becomes pushable BY ITS OWN
  AUTHOR the moment the one below it is published. You push yours by refspec, say
  "done", they push theirs, say "done", up to the top. Every commit is published by
  the party who wrote it, no operator is ever asked to authorise someone else's work,
  and no sid is ever acked. Demonstrated 2026-09-07 on a six-deep stack shared by
  three sessions: two rungs cleared inside a minute once the property was noticed,
  after the stack had stood blocked while both parties correctly refused to publish
  each other's work. FOUR SESSIONS MISSED IT FOR EIGHT HOURS AND THIS GUARD DID NOT
  MENTION IT. The coordination it needs is the word "done", and nothing else.

  Two things break the ladder, so use a refspec at every rung: pushing the BRANCH
  NAME publishes the whole stack including commits above yours, and so does taking a
  rung out of order. Check your rung is clear before you push it, not after:

      git rev-list --count origin/$branch..<your-sha>      must be 1

  Check it BEFORE the push, not after: the push output tells you what happened, the
  count tells you what is about to, and only the second can stop you.

  THE LADDER'S PRECONDITION, and it is the reason this guard reads trailers at all: it
  holds only while every commit in the stack has an IDENTIFIED author. That is what
  the Session-Id trailer buys. Attribute the stack by adjacency instead and the ladder
  is destroyed — you cannot know whose rung is whose, so there is no order to take
  them in and every step is a guess about someone else's work.

  Why this guard is here: docs/trackers/observer-blindness.md OB-20.
EOF
exit 1
