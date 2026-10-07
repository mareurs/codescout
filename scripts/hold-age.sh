#!/usr/bin/env bash
# Sourced, never executed: defines only `hold_age`.
#
# Callers: scripts/pre-push-foreign-session-guard.sh and scripts/hold-publish.sh. Both source
# this file so the age of a hold is written in ONE unit scheme and cannot drift between the
# refusal and `hold-publish.sh list`.
#
# Usage: hold_age <set-at ISO-8601>  -> prints Ns | Nm | Nh | Nd (under a minute, under an hour,
# under a day, then days). A clock that reads the future clamps to 0s.
#
# An empty or unparseable set-at prints NOTHING and returns 1, so each caller keeps its own
# wording for "unknown" (the refusal says `unknown time`, `list` prints `?`). GNU date only (-d);
# anywhere else the parse fails and the answer is "unknown", never a wrong number.
hold_age() {
    local _then _d
    [ -n "${1:-}" ] || return 1
    _then="$(date -u -d "$1" +%s 2>/dev/null)" || return 1
    [ -n "$_then" ] || return 1
    _d=$(( $(date -u +%s) - _then ))
    [ "$_d" -ge 0 ] || _d=0
    if [ "$_d" -lt 60 ]; then printf '%ds' "$_d"
    elif [ "$_d" -lt 3600 ]; then printf '%dm' $((_d / 60))
    elif [ "$_d" -lt 86400 ]; then printf '%dh' $((_d / 3600))
    else printf '%dd' $((_d / 86400)); fi
}
