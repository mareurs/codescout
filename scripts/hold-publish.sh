#!/usr/bin/env bash
#
# hold-publish.sh - record "my work is not ready to publish" for the current session.
#
# A hold is a blob in the repo of the current directory:
#   Claude: refs/holds/<sid> (legacy); Codex: refs/holds/_codex/<sid>.
# The public Codex token is codex:<sid>; a bare token always means legacy Claude.
#   reason: <text>
#   set-at: <UTC ISO-8601>
#   head: <sha at hold time>
# The pre-push guard reads these refs (OB-20).
#
#   hold-publish.sh set [reason...]   hold the resolved Claude or Codex root session; with no
#                                     reason, an existing hold keeps its reason
#   hold-publish.sh release [sid]     drop a hold (default: the caller's own)
#   hold-publish.sh list              one row per hold: sid, LIVE|gone|?, age, reason

set -uo pipefail

usage() {
    printf 'usage: hold-publish.sh set [reason...] | release [sid] | list\n' >&2
}

# `list` and `release` read refs/holds of the repo of the current directory; outside one, git's own
# fatal message is not an answer and an exit 0 would read as "no holds". Same exit code as
# `set`'s usage failures.
in_repo() {
    git rev-parse --git-dir >/dev/null 2>&1 && return 0
    printf 'hold-publish: not a git repository\n' >&2
    return 1
}

cmd="${1:-}"
[ "$#" -gt 0 ] && shift
_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$_dir/publish-identity.sh" || exit 2
sid=""
if [ "$cmd" != list ]; then
    sid="$(publish_identity)" || exit 2
fi

case "$cmd" in
    set)
        if [ -z "$sid" ] || ! ref="$(publish_hold_ref "$sid")"; then
            printf 'hold-publish: CLAUDE_CODE_SESSION_ID / CODEX_SESSION_ID or explicit context is missing or invalid; cannot record a hold\n' >&2
            exit 2
        fi
        # Flatten: a newline would shift the blob's fixed line layout, a tab would break `list` columns.
        reason="$(printf '%s' "$*" | tr '\n\r\t' '   ')"
        head="$(git rev-parse HEAD 2>/dev/null)" || head=""
        set_at=""
        if old="$(git cat-file -p "$ref" 2>/dev/null)"; then
            set_at="$(printf '%s\n' "$old" | sed -n 's/^set-at: //p' | sed -n 1p)"
            # A `set` with NO reason is a refresh of head, not a request to blank the reason: it
            # keeps the one already recorded. (A first reasonless set still writes `reason: `.)
            if [ "$#" -eq 0 ]; then
                reason="$(printf '%s\n' "$old" | sed -n 's/^reason: //p' | sed -n 1p)"
            fi
        fi
        [ -n "$set_at" ] || set_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
        blob="$(printf 'reason: %s\nset-at: %s\nhead: %s\n' "$reason" "$set_at" "$head" | git hash-object -w --stdin)" || exit 1
        git update-ref "$ref" "$blob" || exit 1
        printf 'hold set for session %s\n' "$sid"
        exit 0
        ;;
    release)
        in_repo || exit 2
        target="${1:-$sid}"
        if [ -z "$target" ] || ! ref="$(publish_hold_ref "$target")"; then
            printf 'hold-publish: no session id to release (pass a legacy sid or codex:<sid>, or provide session context)\n' >&2
            exit 0
        fi
        case "$ref" in
            refs/holds/_codex/*)
                git show-ref --exists "$ref" >/dev/null 2>&1
                rc=$?
                case "$rc" in
                    0) ;;
                    2) printf 'no hold for session %s\n' "$target"; exit 0 ;;
                    *) printf 'hold-publish: could not inspect the hold of %s (git exited %s)\n' "$target" "$rc" >&2; exit 1 ;;
                esac
                ;;
            *)
                if ! git rev-parse --verify -q "$ref" >/dev/null 2>&1; then
                    printf 'no hold for session %s\n' "$target"
                    exit 0
                fi
                ;;
        esac
        if ! git update-ref -d "$ref"; then
            printf 'hold-publish: could not release the hold of %s (git update-ref -d failed)\n' "$target" >&2
            exit 1
        fi
        if [ "$target" = "$sid" ]; then
            printf 'released your hold (%s)\n' "$target"
        else
            printf 'released the hold of another session (%s)\n' "$target"
        fi
        exit 0
        ;;
    list)
        in_repo || exit 2
        _dir="$(dirname "${BASH_SOURCE[0]}")"
        if [ -r "$_dir/resolve-sids.sh" ]; then . "$_dir/resolve-sids.sh"; else resolve_sids() { printf '%s\t?\t\n' "$@"; }; fi
        # hold_age is the ONE definition of the age units, shared with the pre-push guard's refusal.
        # Missing file: every age is `?`, never a wrong number.
        if [ -r "$_dir/hold-age.sh" ]; then . "$_dir/hold-age.sh"; else hold_age() { return 1; }; fi
        sids=()
        while IFS= read -r r; do
            [ -n "$r" ] && sids+=("$(publish_hold_token "$r")")
        done < <(git for-each-ref --format='%(refname)' refs/holds/)
        [ "${#sids[@]}" -gt 0 ] || exit 0
        while IFS="$(printf '\t')" read -r s state _sock; do
            ref="$(publish_hold_ref "$s")" || continue
            body="$(git cat-file -p "$ref" 2>/dev/null || true)"
            reason="$(printf '%s\n' "$body" | sed -n 's/^reason: //p' | sed -n 1p)"
            set_at="$(printf '%s\n' "$body" | sed -n 's/^set-at: //p' | sed -n 1p)"
            age="$(hold_age "$set_at")" || age="?"
            printf '%s\t%s\t%s\t%s\n' "$s" "$state" "$age" "$reason"
        done < <(resolve_sids "${sids[@]}")
        exit 0
        ;;
    *)
        usage
        exit 2
        ;;
esac
