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

# Print the token without one leading host qualifier. Parameter expansion only.
publish_principal_root() {
    local t="${1:-}"
    case "$t" in codex:*) t="${t#codex:}" ;; claude-code:*) t="${t#claude-code:}" ;; esac
    printf '%s\n' "$t"
}

# 0 iff both are non-empty, neither is "-", and their roots are equal.
# "codex:U" vs "claude-code:U" compare equal: a documented trade-off (same session id).
# The strip below is deliberately INLINE and repeats the one in publish_principal_root above (the three
# sites: root, and the `a` and `b` lines here): a function that must be called as `$(...)` forks a
# subshell, and this runs per comparison on hook paths, so _equal cannot call _root. Keep all three in
# step: ONE leading `codex:` or `claude-code:`, nothing more.
publish_principal_equal() {
    local a="${1:-}" b="${2:-}"
    case "$a" in '' | -) return 1 ;; esac
    case "$b" in '' | -) return 1 ;; esac
    case "$a" in codex:*) a="${a#codex:}" ;; claude-code:*) a="${a#claude-code:}" ;; esac
    case "$b" in codex:*) b="${b#codex:}" ;; claude-code:*) b="${b#claude-code:}" ;; esac
    # A qualified sentinel ("codex:-", "codex:") is a sentinel too, so test the ROOTS.
    case "$a" in '' | -) return 1 ;; esac
    case "$b" in '' | -) return 1 ;; esac
    [ "$a" = "$b" ]
}

publish_hold_token() {
    local suffix="${1#refs/holds/}"
    case "$suffix" in
        _codex/*) printf 'codex:%s\n' "${suffix#_codex/}" ;;
        *) printf '%s\n' "$suffix" ;;
    esac
}
