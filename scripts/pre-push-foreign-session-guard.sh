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
ack_matched=""

# TIGHTENED 2026-09-09. The previous form was `[ "$ack" = "all" ]` plus a comma-wrapped
# SUBSTRING test, which was looser and stricter than it looked in three separate ways:
#   * `ALL` and ` all` fell through the equality test and matched nothing, silently;
#   * the list was never split, so `CODESCOUT_PUSH_ACK="a, b"` did not ack `b` -- the stored
#     token is " b", with the space;
#   * an ack naming a sid that is not in the push was INERT and said so nowhere, which is the
#     failure mode that matters: you believe you authorised something and you did not.
# The first two are silent no-ops on an input the operator clearly meant; the third is a
# silent no-op on an input they clearly meant AND cannot see the effect of.
acked() {
    _a="$(printf '%s' "$ack" | tr -d '[:space:]' | tr '[:upper:]' '[:lower:]')"
    [ "$_a" = "all" ] && { ack_matched="all"; return 0; }
    case ",$_a," in
        *",$1,"*)
            case ",$ack_matched," in
                *",$1,"*) ;;
                *) ack_matched="${ack_matched:+$ack_matched,}$1" ;;
            esac
            return 0 ;;
        *) return 1 ;;
    esac
}

foreign_sids=""
foreign_report=""
untrailered_report=""
untrailered_n=0
# Every commit in the push, oldest first, for the computed stack table in the refusal.
# 0x1F for the same reason the git log format uses it below: an untrailered commit emits an
# EMPTY sid field, and tab is IFS whitespace, so a tab-delimited row would collapse and put
# the SUBJECT into the sid. Prepended rather than appended because git log runs newest-first
# and the ladder is only legible bottom-up.
commit_rows=""

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
        commit_rows="${sha:0:8}"$'\x1f'"${sid:-}"$'\x1f'"${subject}"$'\n'"${commit_rows}"
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

# AN ACK THAT MATCHED NOTHING IS THE ONE WORTH SAYING OUT LOUD. The two looseness bugs fixed
# above produce a REFUSAL, which is visible. This one produces a PUSH: you typed a sid, the
# guard let the push through for unrelated reasons, and nothing anywhere records that your
# authorisation applied to no commit in the range. Reported, never fatal -- a stale ack left in
# a shell history is a harmless habit, and refusing on it would punish the careful.
if [ -n "$ack" ] && [ "$ack_matched" != "all" ]; then
    _ifs2="$IFS"; IFS=,
    # shellcheck disable=SC2086
    set -- $(printf '%s' "$ack" | tr -d '[:space:]')
    IFS="$_ifs2"
    for _tok in "$@"; do
        [ -n "$_tok" ] || continue
        case ",$ack_matched," in
            *",$_tok,"*) ;;
            *) printf '\n  note: CODESCOUT_PUSH_ACK named %s, which authored no commit in\n' "$_tok" >&2
               printf '  this push, so the ack had no effect on it. Check you named the sid\n' >&2
               printf '  you meant -- an ack matching nothing is silent otherwise.\n' >&2 ;;
        esac
    done
fi

[ -n "$foreign_report" ] || exit 0

branch="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo '<branch>')"

# RESOLVE EACH FOREIGN SID TO A LIVE ADDRESS HERE, rather than telling the reader to go and
# run peer-sessions.sh. The dead-author case then answers itself at the point of refusal
# instead of costing a round trip, and the live case arrives with somewhere to send it.
#
# Liveness is the three-part conjunction src/librarian/session_registry.rs:292-323 settled on:
# the messaging socket exists, /proc/<pid> exists, and /proc/<pid>/stat field 22 STRING-equals
# the row's procStart. The third part is what closes pid reuse, and it is a string compare on
# purpose.
#
# Parsed with python3 rather than sed. A registry row carries `formerNames`, a LIST OF OBJECTS
# with their own keys, so a greedy sed binds to the LAST match and can silently read a nested
# value -- the defect already recorded against the peer-enumeration regex. python3 is already
# on the hook path (scripts/pre-commit-ledger-counts.py runs from pre-commit), and this is ONE
# process for all sids rather than one per sid.
#
# DEGRADES TO THE OLD BEHAVIOUR, never to a wrong answer: no python3, no registry, or an
# unreadable row yields `?` and the banner prints the bare sid as it always did. Three-valued
# on purpose -- LIVE / gone / ? -- because collapsing "cannot tell" into "gone" would print
# "unowned, push it" about a session that is running.
resolve_sids() {
    command -v python3 >/dev/null 2>&1 || { printf '%s\t?\t\n' "$@"; return; }
    printf '%s\n' "$@" | python3 -c '
import glob, json, os, sys
want = [l.strip() for l in sys.stdin if l.strip()]
rows = {}
for f in glob.glob(os.path.expanduser("~/.claude*/sessions/*.json")):
    try:
        d = json.load(open(f))
    except Exception:
        continue
    sid = d.get("sessionId")
    if sid not in want:
        continue
    pid, sock, ps = d.get("pid"), d.get("messagingSocketPath"), d.get("procStart")
    state, addr = "gone", ""
    if pid and sock and ps and os.path.exists(sock):
        try:
            with open("/proc/%d/stat" % int(pid)) as fh:
                if fh.read().rsplit(")", 1)[1].split()[19] == str(ps):
                    state, addr = "LIVE", "uds:" + sock
        except Exception:
            state = "?"
    if rows.get(sid, ("gone",))[0] != "LIVE":
        rows[sid] = (state, addr)
for sid in want:
    st, ad = rows.get(sid, ("?", ""))
    print("%s\t%s\t%s" % (sid, st, ad))
' 2>/dev/null || printf '%s\t?\t\n' "$@"
}

_ifs="$IFS"; IFS=,
# shellcheck disable=SC2086
set -- $foreign_sids
IFS="$_ifs"
addr_table="$(resolve_sids "$@")"

state_of() { printf '%s\n' "$addr_table" | awk -F'\t' -v s="$1" '$1==s{print $2; exit}'; }
addr_of()  { printf '%s\n' "$addr_table" | awk -F'\t' -v s="$1" '$1==s{print $3; exit}'; }

# THE LADDER, COMPUTED. This was ~60 lines of prose asking the reader to derive their own rung
# with rev-list. The guard holds the range and the trailers, so it can name the rung outright.
# Prose that asks for a derivation the emitter could have performed is prose that gets skipped.
plan=""
rung_sha=""
rung_sid=""
while IFS=$'\x1f' read -r _sha _sid _subj; do
    [ -n "${_sha:-}" ] || continue
    if [ -z "$_sid" ]; then
        plan="${plan}    ${_sha}  (no Session-Id trailer -- allowed, not vouched for)"$'\n'"              ${_subj}"$'\n'
    elif [ "$_sid" = "$me" ]; then
        plan="${plan}    ${_sha}  (yours)"$'\n'"              ${_subj}"$'\n'
    else
        if [ -z "$rung_sha" ]; then rung_sha="$_sha"; rung_sid="$_sid"; fi
        plan="${plan}    ${_sha}  ${_sid}  [$(state_of "$_sid")]  $(addr_of "$_sid")"$'\n'"              ${_subj}"$'\n'
    fi
done <<TABLE
$commit_rows
TABLE

case "$(state_of "$rung_sid")" in
    LIVE) rung_line="its author is LIVE at $(addr_of "$rung_sid") -- ask them to push it" ;;
    gone) rung_line="its author has exited, so it is unowned; ack it and say so in your next commit message" ;;
    *)    rung_line="its author could not be resolved from this host -- that is not the same as gone" ;;
esac

cat >&2 <<EOF

  REFUSING THE PUSH: it would publish commits belonging to another session.

  A commit held back pending its operator's decision and one merely not yet pushed are
  byte-identical, and git records nothing that separates them. This guard does not decide
  that. It asks the question at the one moment it is still answerable.

  THE STACK, oldest first. Addresses resolved live, just now:

$plan
  Your session id:  $me

  THE LADDER CLEARS THIS WITH ZERO ACKS, and it is the resolution rather than a fallback:
  each commit becomes pushable BY ITS OWN AUTHOR the moment the one below it is published.
  The lowest foreign rung is

    $rung_sha
    $rung_line

  You push yours by refspec once nothing foreign sits beneath it, say "done", they push
  theirs, up to the top. Every commit is published by whoever wrote it and no sid is ever
  acked. Use a refspec at EVERY rung -- pushing the branch name publishes the whole stack
  including commits above you:

      git push origin <your-sha>:$branch

  IT STALLS ON AN UNCLEARED AUTHOR, which is the common case and not the exception. Ask the
  AUTHOR which of three states they are in:

    withheld                 pending their operator. You hold.
    not withheld, UNCLEARED  ordinary mid-task work -- and they hold no push authorisation
                             of their own either, because "push only when the user asks" is
                             every session's standing instruction. This is the usual answer.
    cleared                  their operator said push.

  "Not withheld" is about their INTENT, never about your authorisation. Reading it as
  clearance is how this guard gets satisfied and bypassed in the same breath.

  THEN ASK YOUR OPERATOR. A peer can report what they were told; a peer CANNOT grant.
  Routing the ask sideways is what makes a standoff -- four sessions held for eight hours
  that way, each correctly refusing to decide what none of them had authority over.

  A RUNG ASSIGNMENT EXPIRES. "I am last, blocking nobody" is true when formed and decays
  with no signal, and a session that believes it is last stops using refspecs -- which is
  exactly when a branch push publishes everyone beneath it. Carry the instant: "I am last as
  of $(date -u '+%H:%M:%SZ')" can expire; "I am last" does not know how to.

  IF EVERY AUTHOR BELOW YOU IS UNCLEARED, YOU HAVE NO RUNG AND NO REFSPEC. The ack is then
  the only route, and it goes to your operator, never to a peer:

      CODESCOUT_PUSH_ACK="$foreign_sids" git push <args>

  AN AUTHORISATION NAMES A SET; A BRANCH PUSH SENDS A PREFIX. They coincide only when nothing
  lands between the decision and the push, which on a shared tree is the unusual case.
  \`git push origin $branch\` satisfies "push what I authorised" to the letter while sending
  whatever arrived since. Re-derive the range, compare it to what was actually decided, then
  send the decided set by sha. Measured window: ninety seconds.

  Full derivations, and why every line above exists: docs/RELEASE.md § Concurrent-Work Rules.
  Class: docs/trackers/observer-blindness.md OB-20.
EOF
exit 1
