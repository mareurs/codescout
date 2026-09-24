#!/usr/bin/env bash
#
# Refuse an INDEX commit that would carry another session's staged paths.
#
# WHY THIS EXISTS
# ---------------
# `git commit` with no pathspec commits the WHOLE index, and a checkout has exactly
# one index shared by every session working it. So `git add <one file>` followed by a
# bare `git commit` takes whatever any peer left staged, under your message.
#
# This is the INDEX-commit twin of scripts/pre-commit-unreviewed-content.sh, which
# guards PATHSPEC commits. Neither covers the other's form, and the two hazards pull
# in opposite directions — the pathspec form is the one that ignores the shared index,
# and it is the form that hook refuses when your content is unstaged. Together they
# cover both, and the safe composition is to satisfy both at once:
#
#     git add <paths>            # staging is what satisfies unreviewed-content
#     git commit -- <same paths> # the pathspec is what ignores the shared index
#
# Measured 2026-09-01, in this repo: a `git add` on one file joined an index that
# already held 16 foreign staged files, and a peer's entire OB-6 promotion was
# committed inside a commit about something else. Both are this guard's case.
#
# WHAT THIS GUARD DOES NOT COVER — and it is the incident people will assume it does
# ------------------------------------------------------------------------------------
# It does NOT catch `d617051b`, the capture that prompted this work. Established by a
# peer review on 2026-09-01 rather than by its author, which is the point.
#
# Two different axes:
#
#   CROSS-path  my index holds YOUR file      -> this guard
#   INTRA-path  my file holds YOUR lines      -> nothing here
#
# In `d617051b` the committing session verified `git diff` on one path, then ran
# `git add` on that same path ~40s later, and a peer wrote INTO that file during the
# window. The path was staged by the committing session, legitimately; the stage log
# records them as its stager; a bare commit sees only their own staged path and this
# guard exits 0. It would have passed. Path ownership was never in dispute — the
# contamination was inside a path both parties agree is yours, so no ownership check
# can see it.
#
# The remedy for that half is a time-of-check/time-of-use guard: record each path's
# blob at `git add`, re-hash at pre-commit, refuse if it moved. Not built here. Say so
# out loud, because the next session to hit that race will otherwise have been told
# this covered them — which is the guard-narrower-than-its-name defect
# (docs/trackers/issue-clusters.md IC-14) shipped inside a guard against capture.
#
# Full record of both shapes:
#   docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md
#
# MECHANISM
# ---------
# scripts/post-index-change-stage-log.sh records
# `<owner>\t<blob>\t<path>\t<route>[\t retained]` for every staged blob as it appears,
# and keeps rows for pairs that have LEFT the index so a transiently empty index cannot
# erase ownership. This reads that log, keyed on the same (blob, path) pair, and refuses
# when any currently-staged pair is owned by a different session id. The trailing
# `retained` marker is the recorder's own bookkeeping about whether a claim may override
# the row; nothing here reads it, and this guard treats a retained row like any other.
#
# WHAT THIS DOES NOT CATCH
# ------------------------
# Everything the stage log cannot see. Its fail-open race is documented at the top of
# that script and is inherited here unchanged: where the log mis-attributes a peer's
# pair to THIS session, this guard stays SILENT.
#
# Name the direction precisely, because the short form of this sentence is the part
# that travels into a commit message or an IC entry. The mechanism OVER-attributes
# ownership to SELF, and that over-attribution is exactly what produces the silence.
# It under-reports FOREIGN paths, and raises no foreign-path false alarms — which
# sounds like the safe direction and is not, because a missing refusal emits nothing
# while a false alarm would at least print something a reader could act on. So a clean
# run is not proof the index is yours; it is proof that nothing recorded says
# otherwise.
#
# (Corrected 2026-09-01 after peer review. This paragraph read "it under-reports and
# never over-reports", which is true only with "foreign paths" as the referent and
# resolves the reassuring way on a skim. The reviewer's argument for bothering was
# that a short reassuring clause is the one that gets pasted onward, at which point a
# reader concludes the silence is the safe failure and stops looking.)
#
# `git diff --cached --name-only`, read in its own call before you commit, remains the
# check that answers the question directly and depends on none of this.
#
# With no CLAUDE_CODE_SESSION_ID this exits silently rather than refusing. A commit
# from a plain terminal is a deliberate human act, and this guard has no id to
# discriminate with — blocking it would be a false alarm, and a guard that fires on
# ordinary work teaches `--no-verify`, which disarms the quiet one that works.

set -uo pipefail

# `--classify` is a SECOND ENTRY POINT, for `scripts/commit-mine.sh` and nothing else: print
# which staged paths THIS guard attributes to the caller and which to whom, then exit 0 without
# judging. It exists so the helper asks the guard instead of re-implementing the lookup --
# the same law as the two copy-pasted `--raw` pipelines below, where a fix to one site left
# the other wrong (docs/issues/archive/2026-09-08-the-stage-log-records-a-renames-source-path-and-drops-its-destination.md).
# Output, one line per staged path:   mine\t<your id>\t<path>   |   theirs\t<owner>\t<path>
# Three NON-EMPTY fields, always: a tab in IFS is whitespace to `read`, so an empty middle
# field would collapse and the path would be read as the owner.
#
# EVERY stand-down below that exits 0 in the default mode exits NON-ZERO here, and that
# asymmetry is the point. The guard fails OPEN because refusing a routine commit teaches
# `--no-verify`; the helper must fail CLOSED, because an empty classification it read as
# "nothing foreign" would let it claim every staged path as the caller's. 2 = no session id,
# 3 = cannot classify (sequencer stop, or no stage log). tests/commit-mine.sh F5/F8/F9.
classify=0
[ "${1:-}" = "--classify" ] && classify=1

me="${CLAUDE_CODE_SESSION_ID:-}"
if [ -z "$me" ]; then
    ((classify)) && { echo "classify: no CLAUDE_CODE_SESSION_ID, so no owner to classify for" >&2; exit 2; }
    exit 0
fi

# A PATHSPEC commit gets a temporary index named `next-index-<pid>.lock`. This block used
# to `exit 0` here, on the stated premise that such a commit "IGNORES the shared index
# entirely, so it cannot capture staged content and needs no guard."
#
# THE PREMISE IS FALSE, and the stand-down was the entire defect. A pathspec commit
# ignores the index only for paths it does NOT name; for a path it DOES name it commits
# that path's WORKING-TREE content, which is a peer's whenever a peer is editing that file.
#
# Measured 2026-09-14 in a throwaway repo, and the control is what makes it a measurement:
# session A stages f.txt, session B runs the guard over a temp index carrying A's blob.
# Identical content, identical ownership, one difference — the bare form REFUSED and named
# f.txt, the pathspec form exited 0. It had already fired twice in production, most
# recently taking a live session's staged tracker edit into a peer's commit under the
# peer's Session-Id.
#
# The discriminator is KEPT and its meaning inverted: it now selects the REMEDY rather than
# standing the guard down. That is not cosmetic — the bare form's remedy IS "commit by
# pathspec", and printing that to someone whose pathspec commit just failed routes them
# back into the thing that failed. A guard whose message prescribes the refused action is
# how you teach `--no-verify`.
#
# What this does NOT do is refuse a peer's unrelated staged paths. Verified 2026-09-14:
# with P.txt and Q.txt both staged, `git commit -- Q.txt` builds a temp index whose
# `git diff --cached --name-only` is Q.txt ALONE. The temp index holds only the named
# paths, so the loop below never sees P.txt and the false-positive case cannot arise.
# docs/issues/archive/2026-09-02-a-pathspec-commit-does-capture-staged-content-and-both-guards-stand-down.md
idx="${GIT_INDEX_FILE:-}"
pathspec=0
case "${idx##*/}" in
    next-index-*) pathspec=1 ;;
esac

git_dir="$(git rev-parse --git-dir 2>/dev/null)" || exit 0

# A sequencer stop — a conflicted cherry-pick or merge, or a rebase stopped mid-pick —
# makes this guard's OWN prescribed remedy impossible. git refuses `git commit -- <path>`
# there with "cannot do a partial commit during a cherry-pick", while the bare form
# refused below is the only one it will accept. Refusing here leaves no compliant route
# at all, which is what teaches `--no-verify`. See
# docs/issues/archive/2026-09-02-foreign-index-prescribes-a-remedy-git-refuses.md.
#
# Keyed on the sequencer HEADs, NOT on "a rebase is running": measured 2026-09-02, a
# rebase stopped with rebase-merge/ present and CHERRY_PICK_HEAD absent commits by
# pathspec fine, so the wider test would stand the guard down while its remedy still
# works. Asked via `--git-path` rather than "$git_dir/..." so git decides per-worktree
# vs common itself — both resolve per-worktree today, and only one stays right if that
# ever changes.
#
# This is a stand-down, not a hole: the guard still refuses in ordinary work, which is
# the case the fix could most easily have broken. All four arms are covered by
# tests/hooks-discrimination.sh § 7, whose `no sequencer -> still refuses` case is the
# one that fails if this is ever widened to an unconditional exit.
if [ -e "$(git rev-parse --git-path CHERRY_PICK_HEAD)" ] ||
   [ -e "$(git rev-parse --git-path MERGE_HEAD)" ]; then
    ((classify)) && { echo "classify: a cherry-pick or merge is in progress; a commit now would conclude it" >&2; exit 3; }
    exit 0
fi

log="$git_dir/session-stage-log"
# STAND DOWN when there is no log to read — and what that line is FOR is narrower than it
# looks. Documented 2026-09-20 because this was the only stand-down in this file carrying
# no reason and no case, present since the guard's first commit (99d5acac).
#
# IT DOES NOT DECIDE THE VERDICT. Measured by deleting it from a copy and running an
# identical fixture: still exit 0, because with no log there are no foreign owners and
# `((${#theirs[@]})) || exit 0` below reaches the same answer. The fail-open here is
# STRUCTURAL rather than a choice made at this line. What the line actually buys is one
# thing: it stops the unguarded `awk` below from printing `awk: fatal: cannot open file`
# at the caller. Deleting it is a cosmetic regression, not a policy change.
#
# That is why an earlier mutation of this line killed nothing. The natural reading of that
# SURVIVED — "untested" — was wrong; it is inert for the verdict because a sibling path
# covers its domain, which is CLAUDE.md's third reading.
#
# WHETHER THE STRUCTURAL FAIL-OPEN IS RIGHT was left open here until 2026-09-24, and the
# answer turned out to be already half-written. `install-hooks.sh`'s seeding comment states
# this file's principle as "prefer the noisy wrong answer when the quiet one is
# unobservable", which argues against a silent exit. Measured against that: 12 of 13
# `git_dir`s on this checkout carry a seeded log, and the one without is a linked worktree
# whose index is private to it, so no peer's staged work exists there to capture.
#
# REFUSING here was already ruled out, on measurement, and not by this session.
# docs/issues/archive/2026-09-01-an-absent-stage-log-makes-the-foreign-index-guard-pass.md
# carries the decision table: P1, "`-` whenever the log is cold", is recorded REJECTED as
# "the version that gets the guard switched off" — a guard that refuses your own routine
# commits teaches `--no-verify`, which disarms the quiet one that works. P2 shipped instead
# (`fa9b3aff`) and closed the COLD-log route: any index write rebuilds the log, unattributable
# rows record `-`, and `-` reads as foreign. What that fix does not reach, and says so, is the
# window while the log is STILL MISSING — this line.
#
# So the remaining move is neither P1 nor P0: keep the verdict, change the OUTPUT. The table
# had no column for that, because all three of its policies decide an owner. An EXIT 0 that
# says nothing is byte-identical to "I checked and nothing foreign was staged"; the notice
# below makes "could not check" distinguishable from "checked and clear" at no false-alarm
# cost, since it refuses nothing. Its remedy is the one this file's own header already names
# as depending on none of this machinery, so the reader can act on it unaided.
#
# COST, stated because it is real and recurring: this prints on EVERY commit from a git_dir
# with no seeded log. Rare across git_dirs (1 of 13), constant within that one. If it ever
# becomes noise, scope it to git_dirs with more than one live session — more logic, and more
# that can be wrong.
#
# tests/hooks-discrimination.sh § 7b pins three things separately — the silence as an
# OVER-DETERMINED outcome that survives this block's deletion, the absence of the awk fatal
# as the assertion that reds when the early exit goes, and the notice naming the artifact it
# could not read. The EXIT=0 assertion sits ABOVE the notice assertion on purpose, so a
# notice that ever starts refusing reds as a verdict change rather than as a text change.
if [ ! -s "$log" ]; then
    ((classify)) && { printf 'classify: no session-stage-log at %s, so no path can be attributed to anyone\n' "$git_dir" >&2; exit 3; }
    printf 'pre-commit: foreign-index guard did NOT run — no session-stage-log at %s\n' "$git_dir" >&2
    printf '  So this exit 0 means "could not check", not "nothing foreign is staged".\n' >&2
    printf '  Read it yourself before committing:  git diff --cached --name-only\n' >&2
    exit 0
fi

# Resolve a session id to a LIVE session, printing "<pid>|<name>" or nothing.
#
# Without this the refusal can only DESCRIBE the dead-incarnation case while being
# unable to detect one, which is a gate keyed on an event it cannot observe — the very
# class this repo tracks as IC-2, shipped inside a guard against capture.
#
# The registry record carries `sessionId` alongside `pid` and `name`, and the file is
# named for the pid, so this is a direct lookup rather than a scan of anything. The
# `comm` check guards pid reuse the same way scripts/peer-sessions.sh does: a recycled
# pid running something else must not be reported as a live peer.
resolve_session() {
    _rs_id="$1"
    for _rs_f in "$HOME"/.claude*/sessions/*.json; do
        [ -f "$_rs_f" ] || continue
        grep -q "sessionId\"[[:space:]]*:[[:space:]]*\"$_rs_id\"" "$_rs_f" 2>/dev/null || continue
        _rs_pid="$(basename "$_rs_f" .json)"
        [ -r "/proc/$_rs_pid/comm" ] || continue
        [ "$(tr -d '\0' < "/proc/$_rs_pid/comm")" = "claude" ] || continue
        _rs_name="$(sed -n 's/.*"name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$_rs_f")"
        printf '%s|%s\n' "$_rs_pid" "${_rs_name:-?}"
        return
    done
}

mine=()
theirs=()
foreign_owners=()
# Routes seen for `-` rows. `foreign_owners` dedupes to a single `-` entry however many
# unrecorded paths there are, so the branch that produced them has to be collected
# separately or it is lost with the duplicates.
unrecorded_routes=()
# path -> owner, for `--classify` only: the refusal below needs owners deduplicated, the
# helper needs them per path.
declare -A owner_of=()

while IFS=$'\t' read -r blob path; do
    [ -n "$path" ] || continue
    prior="$(awk -F'\t' -v b="$blob" -v p="$path" \
        '$2 == b && $3 == p { print $1 "\t" $4; exit }' "$log")"
    IFS=$'\t' read -r owner route <<< "$prior"
    if [ -n "$owner" ] && [ "$owner" != "$me" ]; then
        theirs+=("$path")
        owner_of["$path"]="$owner"
        case " ${foreign_owners[*]-} " in
            *" $owner "*) ;;
            *) foreign_owners+=("$owner") ;;
        esac
        if [ "$owner" = "-" ]; then
            # A row written before route recording has an empty $4; name that state
            # rather than letting it read as a recorded branch.
            case " ${unrecorded_routes[*]-} " in
                *" ${route:-pre-route} "*) ;;
                *) unrecorded_routes+=("${route:-pre-route}") ;;
            esac
        fi
    else
        mine+=("$path")
    fi
done < <(git diff --cached --raw --no-renames 2>/dev/null |
    awk -F'\t' '{ split($1, a, " "); print a[4] "\t" $2 }')
# `--no-renames` here for the same reason as in post-index-change-stage-log.sh, and this is
# the SECOND site of one law -- the pipeline is copy-pasted between the two scripts, so
# fixing the recorder alone left this reader still resolving a rename to (destination blob,
# SOURCE path). That pair matches nothing in a correctly-recorded log, so the lookup fell to
# `mine` and the refusal never named the destination: the recorder knew who owned the
# archive path and the guard never asked about it.
#
# Caught by `tests/hooks-discrimination.sh`'s "refusal names the rename DESTINATION", which
# failed with the recorder already fixed. Keep BOTH call sites in step; a mutation of either
# alone leaves the other's assertion green.
# docs/issues/archive/2026-09-08-the-stage-log-records-a-renames-source-path-and-drops-its-destination.md

# The classification is complete here and nothing below changes it -- the joint and
# all-contested arms decide whether to REFUSE, not whose a path is -- so this is the one
# point where printing it cannot disagree with the verdict a real commit would get.
if ((classify)); then
    for p in "${mine[@]-}"; do [ -n "$p" ] && printf 'mine\t%s\t%s\n' "$me" "$p"; done
    for p in "${theirs[@]-}"; do [ -n "$p" ] && printf 'theirs\t%s\t%s\n' "${owner_of[$p]}" "$p"; done
    exit 0
fi

((${#theirs[@]})) || exit 0

# ------------------------------------------------------------------------------------
# JOINT ARCHIVE: one rename, two authors, and neither party can commit it.
#
# `CLAUDE.md` MANDATES archiving a bug file through `doc(action="move")`. When the file
# was authored by one session and archived by another, that procedure necessarily splits
# ONE path's authorship: the source content is the author's, the current content (status
# flip, fix SHA, patch-id) is the archiver's. This guard is keyed on one author per path,
# so it refuses the archiver over the source half AND the author over the destination —
# each refusal's remedy naming the other party, who is themselves refused.
#
# Measured from both sides on six files, 2026-09-16:
# `docs/issues/2026-09-16-archiving-a-peers-bug-file-refuses-both-parties-from-opposite-sides.md`
#
# WHY THE PAIR IS RECOVERED HERE RATHER THAN BY DROPPING `--no-renames` ABOVE. That flag
# is load-bearing and its removal reintroduces an archived defect: with rename detection
# on, `--raw` emits `R<score>\t<src>\t<dst>` and the awk takes `$2`, the SOURCE, so the
# lookup becomes (destination blob, source path), matches nothing in a correctly-recorded
# log, falls to `mine`, and the refusal never names the destination at all
# (`docs/issues/archive/2026-09-08-the-stage-log-records-a-renames-source-path-and-drops-its-destination.md`,
# pinned by `tests/hooks-discrimination.sh` "refusal names the rename DESTINATION"). The
# lookup keeps the split view; the PAIRING is recovered from a second call that is used
# for nothing else.
#
# THIS DOES NOT RELAX THE GUARD. A joint archive is still refused. What changes is that
# the refusal names the situation correctly and offers a remedy the caller can actually
# perform, instead of directing them to a party who is also blocked.
declare -A rename_dst_of=()
declare -A rename_src_of=()
while IFS=$'\t' read -r _status _src _dst; do
    case "$_status" in
        R*) [ -n "$_dst" ] || continue
            rename_dst_of["$_src"]="$_dst"
            rename_src_of["$_dst"]="$_src" ;;
    esac
done < <(git diff --cached --name-status -M 2>/dev/null)

# The change is a joint archive when EVERY contested path is one half of a rename whose
# other half is yours. A contested path with no partner, or whose partner is also theirs,
# is an ordinary capture and must keep the ordinary refusal.
joint=1
for path in "${theirs[@]}"; do
    partner="${rename_dst_of[$path]:-${rename_src_of[$path]:-}}"
    if [ -z "$partner" ]; then joint=0; break; fi
    case " ${mine[*]-} " in
        *" $partner "*) ;;
        *) joint=0; break ;;
    esac
done
((${#theirs[@]})) || joint=0

# THE ALL-CONTESTED ROUTE. Every path the committer NAMED is someone else's, so no
# narrowing exists -- the branch below says exactly that, and until now said nothing after
# it. `joint` cannot reach this shape: it requires each contested path's partner to be in
# `mine`, and here `mine` is empty, so the two are provably disjoint rather than nested.
#
# RESTRICTED TO THE PATHSPEC FORM, and the conjunct is the whole safety argument. A pathspec
# commit builds a temporary index from the paths the committer typed, so `mine` empty means
# "every path I named is theirs" -- bounded by an act of enumeration a human performed. A
# BARE commit takes the entire shared index, where `mine` empty means "every path any peer
# left staged is theirs", bounded by nothing. That is the whole-index sweep this guard was
# built for: measured 2026-09-01, a `git add` on one file joined an index already holding 16
# foreign staged files. Remove the one file and an unrestricted widening accepts it.
#
# The other half of the argument is that a guard should only offer an escape where it has
# certified there is no compliant route. Under pathspec it says so in its own words below.
# Under bare a narrower route always exists and the guard prints it -- `git commit -- <your
# paths>` -- so an ack firing there is not an escape from a corner, it is a shortcut past one.
#
# A `-` owner is NOT separately excluded here because the ack loop already rejects it and
# says why; see the `ack_has_dash` arm. Stated because its absence otherwise reads as an
# oversight.
all_contested=0
if ((pathspec)) && ((${#theirs[@]})) && ((${#mine[@]} == 0)); then
    all_contested=1
fi

# The ack mirrors `CODESCOUT_PUSH_ACK` in the pre-push guard deliberately: same shape, same
# reason, and a pre-commit hook cannot read the commit message, so an env acknowledgement is
# the only surface on which the committer can NAME the other author before the fact.
# Deliberately NO `all` form — a joint archive has exactly one other party, so a wildcard
# would buy nothing and would import the blast radius that `all` was filed for.
index_ack="$(printf '%s' "${CODESCOUT_INDEX_ACK:-}" | tr -d '[:space:]')"
ack_has_dash=0
if { ((joint)) || ((all_contested)); } && [ -n "$index_ack" ]; then
    ack_ok=1
    for owner in "${foreign_owners[@]}"; do
        # `-` IS NOT A PARTY AND CANNOT BE ACKED. The recorder writes `-` for a pair it
        # could not attribute -- four routes reach it, and its own header calls such a row
        # "frequently a PEER's". The membership test below is plain string matching, so
        # without this arm `CODESCOUT_INDEX_ACK="-"` satisfies it: one character, naming
        # nobody, covering an unbounded number of unattributable pairs. That is the `all`
        # form this gate deliberately does not have, reached by a different spelling.
        # Reproduced 2026-09-16 against the shipped guard: EXIT=1 with no ack, EXIT=0 with
        # `-`. Raised by a design review of a proposed widening; the hole predates it.
        if [ "$owner" = "-" ]; then
            ack_ok=0
            ack_has_dash=1
            break
        fi
        case ",$index_ack," in
            *",$owner,"*) ;;
            *) ack_ok=0; break ;;
        esac
    done
    if ((ack_ok)); then
        printf '\n  note: CODESCOUT_INDEX_ACK names every other author of this joint archive.\n' >&2
        printf '  Proceeding. Their work lands under YOUR commit message, so record them:\n' >&2
        for owner in "${foreign_owners[@]}"; do
            printf '      Co-Authored-Session-Id: %s\n' "$owner" >&2
        done
        printf '  The ack records your operator decision about their content. It does not\n' >&2
        printf '  speak for their operator, and it leaves them as UNCLEARED as they were.\n\n' >&2
        exit 0
    fi
fi

{
    echo
    if ((joint)); then
        echo "Refusing this commit: it is a JOINT ARCHIVE — one rename, two authors."
    elif ((pathspec)); then
        echo "Refusing this pathspec commit: it captures content another session wrote."
    else
        echo "Refusing a bare commit: the index holds paths staged by another session."
    fi
    echo
    echo "  theirs:"
    for path in "${theirs[@]}"; do
        echo "      $path"
    done
    if ((${#mine[@]})); then
        echo
        echo "  yours:"
        for path in "${mine[@]}"; do
            echo "      $path"
        done
    fi
    echo
    if ((pathspec)); then
        echo "\`git commit -- <path>\` commits that path's WORKING-TREE content. It bypasses"
        echo "the shared index for paths it does not NAME, which is what makes this form the"
        echo "right remedy in general — but the content at the path you named is theirs, so"
        echo "committing now would file their work under your message, where it is durable"
        echo "and no longer theirs to attribute."
        echo
        echo "You cannot narrow further: the contested path IS one you named."
        echo
        if ((${#mine[@]})); then
            echo "Drop it and commit the rest:"
            echo
            echo "    git commit -- ${mine[*]}"
        else
            echo "Every path you named is contested, so there is nothing to narrow to."
            echo
            echo "THAT IS NOT A DEAD END, and until 2026-09-16 this is where the text stopped."
            echo "You named only their paths, so there is no smaller commit to make -- which is"
            echo "exactly the state the ack exists for. Two steps, both yours, neither waiting"
            echo "on them:"
            echo
            echo "  1. TELL them you are committing it. They cannot see this refusal."
            echo "     Addresses: /codescout-companion:reaching-peer-sessions"
            echo
            echo "  2. Re-run with their session id named:"
            echo
            printf '    CODESCOUT_INDEX_ACK="%s" git commit -- <the same paths>\n' "$(IFS=,; echo "${foreign_owners[*]}")"
            echo
            echo "     and put these in the FINAL PARAGRAPH of the message, beside any other"
            echo "     trailers. git parses ONLY the last paragraph as trailers, so one placed"
            echo "     above an existing block is readable prose and invisible to every query:"
            echo
            for _o in "${foreign_owners[@]}"; do
                echo "         Co-Authored-Session-Id: $_o"
            done
            echo
            echo "The ack does not make the attribution correct -- it makes it RECORDED, which"
            echo "\`--no-verify\` does not. It carries YOUR operator's decision about their"
            echo "content and says nothing for their operator, who has not been asked."
        fi
        echo
        if ((joint)); then
            echo "THAT REMEDY DOES NOT APPLY HERE and following it terminates where it started:"
            echo "this is a rename whose source you authored and whose current content they"
            echo "wrote, so they are refused over the source exactly as you are over the"
            echo "content. Asking them to commit theirs sends you to a party this same guard"
            echo "has already stopped."
        else
            echo "Then ask the owner below to commit theirs. Once their change is in HEAD your"
            echo "next commit of that path carries only your own, and this guard goes quiet."
        fi
        echo
        echo "Do NOT \`git checkout\` or \`git stash\` the path to clear this. Their work is"
        echo "in the working tree and is not committed anywhere — discarding it destroys it,"
        echo "which is worse than the mislabelling this guard exists to prevent."
    else
        echo "\`git commit\` with no pathspec commits the WHOLE index, and this checkout"
        echo "shares one index across every session working it. Committing now would file"
        echo "their work under your message, where it is durable and no longer theirs to"
        echo "attribute."
        echo
        echo "Commit your own paths by pathspec — that form ignores the shared index for"
        echo "paths it does not name:"
        echo
        if ((${#mine[@]})); then
            echo "    git commit -- ${mine[*]}"
        else
            echo "    git commit -- <your paths>   # <- none of the staged paths look like yours"
        fi
        echo
        echo "If THAT is refused too -- a file you staged also carries a peer's UNSTAGED edit,"
        echo "and a pathspec commit takes the whole working-tree file -- commit exactly what"
        echo "you staged instead. It builds a private index from HEAD plus only the staged"
        echo "paths this guard attributes to you, so every hook sees your paths and no others:"
        echo
        echo "    scripts/commit-mine.sh -m \"...\""
        echo
        echo "Leave theirs staged; it is not yours to unstage either. \`git reset\` here"
        echo "would take their work out of the index seconds before they commit it."
    fi
    echo
    if ((joint)); then
        echo
        echo "WHAT TO DO INSTEAD, and both steps are yours — neither waits on them:"
        echo
        echo "  1. TELL them you are committing it. They cannot see this refusal, and the"
        echo "     archive will carry their status flip, fix SHA and patch-id under your"
        echo "     message. Addresses: /codescout-companion:reaching-peer-sessions"
        echo
        echo "  2. Re-run with their session id named, and record them in the message:"
        echo
        printf '    CODESCOUT_INDEX_ACK="%s" git commit ...\n' "$(IFS=,; echo "${foreign_owners[*]}")"
        echo
        for _o in "${foreign_owners[@]}"; do
            echo "    Co-Authored-Session-Id: $_o"
        done
        echo
        echo "The ack does not make the attribution correct — it makes it RECORDED, which"
        echo "\`--no-verify\` does not. It carries your operator decision about their"
        echo "content and says nothing for their operator, who has not been asked."
        echo
        echo "Why this guard cannot simply accept the pair: a rename spanning two authors"
        echo "is a real two-party commit, and passing it silently would file their work"
        echo "under your name with nothing anywhere saying so."
        echo
        echo "Class: docs/issues/2026-09-16-archiving-a-peers-bug-file-refuses-both-parties-from-opposite-sides.md"
    fi
    if ((ack_has_dash)); then
        echo
        echo "YOUR ACK CANNOT CLEAR THIS: one contested half is UNATTRIBUTED (owner \`-\`)."
        echo
        echo "\`-\` is not a session. It is every session the recorder could not attribute,"
        echo "so naming it would acknowledge an unbounded set of parties instead of the ones"
        echo "you mean -- the wildcard this gate deliberately does not have. An ack has to"
        echo "name a real sid, and no sid owns that row."
        echo
        echo "Find out who staged it before committing: the row is \`-\` because the write"
        echo "that created it was not a recognised staging command, or carried no session id."
        echo "Re-staging it yourself makes it yours by the recorder's rule and this guard goes"
        echo "quiet -- which is attribution by stager, not approval, so record them anyway."
        echo
    fi
    echo "Staged by:"
    for owner in "${foreign_owners[@]-}"; do
        if [ "$owner" = "-" ]; then
            echo "      (unrecorded) — no session id is on these rows. What the recorder"
            echo "          logged about why, one line per distinct cause:"
            for _r in "${unrecorded_routes[@]-}"; do
                case "$_r" in
                    unnamed)
                        echo "            • blanket add — the staging command did not NAME"
                        echo "              these paths: \`git add\` with -A, -u, \`.\`, or a"
                        echo "              directory, or a patch carrying no default prefix."
                        echo "              THIS IS PROBABLY YOUR OWN STAGING, and it is the"
                        echo "              case this guard exists for — a blanket add followed"
                        echo "              by a bare commit is exactly how a peer's work gets"
                        echo "              filed under your message. Re-stage by explicit path"
                        echo "              and the bare commit passes."
                        ;;
                    pre-staged)
                        echo "            • already staged when a later command ran — that"
                        echo "              command named other paths, so this pair's row was"
                        echo "              lost or never written and the recorder cannot"
                        echo "              attribute it. DO NOT read this one as yours: an"
                        echo "              unclaimed pre-existing pair is frequently a peer's."
                        echo "              Ask, and commit by pathspec meanwhile."
                        ;;
                    not-staging)
                        echo "            • not a staging command — the index write came from"
                        echo "              an unreadable or unrecognised parent, so nothing"
                        echo "              could be claimed. Ask before assuming it is yours."
                        ;;
                    id-unset)
                        echo "            • no session id — whoever staged these had no"
                        echo "              CLAUDE_CODE_SESSION_ID, so there was no id to"
                        echo "              record. A terminal commit is the usual cause."
                        ;;
                    pre-route)
                        echo "            • route not recorded — these rows predate route"
                        echo "              logging, so the branch is genuinely unknown here."
                        echo "              Find the owner by asking, not by assuming."
                        ;;
                    *)
                        echo "            • $_r"
                        ;;
                esac
            done
            echo "          Unknown ownership stays deliberate: it over-refuses, where"
            echo "          claiming would have gone silent."
            continue
        fi
        info="$(resolve_session "$owner")"
        if [ -n "$info" ]; then
            echo "      $owner"
            echo "          LIVE — ${info#*|} (pid ${info%%|*})"
            echo "          SendMessage(to: \"uds:/run/user/$(id -u)/cc-socks/${info%%|*}.sock\")"
        else
            echo "      $owner"
            echo "          NOT LIVE — a dead incarnation, not an abandoned file."
            echo "          Compaction and resume mint a NEW id for the same agent doing"
            echo "          the same work, so the owner is often alive under a different"
            echo "          one. Read \$HOME/.claude*/projects/<encoded>/$owner.jsonl"
            echo "          before concluding anything, and do not read an unreachable id"
            echo "          as permission to take the file."
        fi
    done
    echo
    echo "ASK before assuming. scripts/peer-sessions.sh lists every live session,"
    echo "including the ones ListAgents hides from you."
    echo
    echo "\`--no-verify\` also works and is the wrong habit."
    # Forward reach — see scripts/commit-sequence-tail.txt. This hook's rule is step 4's
    # consequence; the rest of the sequence is what stops the next collision rather than
    # this one. Single emitted copy shared by all three refusing hooks.
    _tail="$(dirname "$0")/commit-sequence-tail.txt"
    [ -r "$_tail" ] && { echo; cat "$_tail"; }
} >&2

exit 1
