#!/usr/bin/env bash
# Sourced by hold and commit/push hooks. No state changes at source time.
# A nonzero resolver exit must be propagated: conflicting identities are not manual.
_publish_identity_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

publish_identity() {
    # Manual hooks stay silent even when Python or Git is unavailable.
    [ -n "${CLAUDE_CODE_SESSION_ID:-}${CODEX_SESSION_ID:-}${CODESCOUT_CONTEXT_FILE:-}" ] || return 0
    python3 "$_publish_identity_dir/publish-identity.py"
}

publish_hold_ref() {
    local token="${1:-}" sid harness
    case "$token" in
        codex:*) harness=codex; sid="${token#codex:}" ;;
        claude-code:*) harness=claude-code; sid="${token#claude-code:}" ;;
        *) harness=claude-code; sid="$token" ;;
    esac
    case "$sid" in
        '' | *[!A-Za-z0-9-]*) return 1 ;;
    esac
    if [ "$harness" = codex ]; then
        # "_" is outside the legacy sid grammar: even a Claude sid "codex" cannot collide.
        printf 'refs/holds/_codex/%s\n' "$sid"
    else
        printf 'refs/holds/%s\n' "$sid"
    fi
}

publish_hold_token() {
    local suffix="${1#refs/holds/}"
    case "$suffix" in
        _codex/*) printf 'codex:%s\n' "${suffix#_codex/}" ;;
        *) printf '%s\n' "$suffix" ;;
    esac
}
