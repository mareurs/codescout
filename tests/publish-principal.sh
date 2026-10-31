#!/usr/bin/env bash
#
# Suite for publish_principal_root / publish_principal_equal in scripts/publish-identity.sh.
# Pure functions: no repo, no HOME, no environment needed.
#
# Usage:
#   tests/publish-principal.sh     # non-zero exit on any failure

set -uo pipefail
# Pure functions, but a sourced helper must never see an enclosing Claude/Codex/explicit context.
unset CLAUDE_CODE_SESSION_ID CODEX_SESSION_ID CODEX_THREAD_ID CODESCOUT_CONTEXT_FILE

HELPER="$(cd "$(dirname "$0")/../scripts" && pwd)/publish-identity.sh"
U="aaaaaaaa-1111-2222-3333-444444444444"
V="bbbbbbbb-5555-6666-7777-888888888888"

PASS=0
FAIL=0
ok() { echo "  PASS  $1"; PASS=$((PASS + 1)); }
no() {
    echo "  FAIL  $1"
    [ -n "${2:-}" ] && echo "        $2"
    FAIL=$((FAIL + 1))
}
eq() { [ "$2" = "$3" ] && ok "$1" || no "$1" "want '$3', got '$2'"; }

[ -r "$HELPER" ] || { echo "FATAL: $HELPER is not readable"; exit 1; }
# shellcheck source=../scripts/publish-identity.sh
. "$HELPER"

# same <a> <b> -> prints 0 when publish_principal_equal succeeds, 1 otherwise
same() { publish_principal_equal "$1" "$2"; echo $?; }

echo "== publish_principal_equal"
eq "bare_equals_codex_qualified"             "$(same "$U" "codex:$U")" 0
eq "claude_qualified_equals_bare"            "$(same "claude-code:$U" "$U")" 0
eq "different_ids_differ"                    "$(same "$U" "$V")" 1
eq "dash_never_equal (dash vs dash)"         "$(same - -)" 1
eq "dash_never_equal (dash vs id)"           "$(same - "$U")" 1
eq "empty_never_equal (empty vs empty)"      "$(same "" "")" 1
eq "empty_never_equal (empty vs id)"         "$(same "" "$U")" 1
eq "codex_and_claude_qualified_same_id_equal (documented trade-off)" "$(same "codex:$U" "claude-code:$U")" 0
eq "sentinel_in_second_position (id vs dash)"  "$(same "$U" -)" 1
eq "sentinel_in_second_position (id vs empty)" "$(same "$U" "")" 1
eq "unknown_prefix_is_not_stripped"            "$(same "foo:$U" "$U")" 1
eq "double_prefix_strips_only_one"             "$(same "codex:codex:$U" "$U")" 1
eq "double_prefix_strips_only_one (mixed)"     "$(same "codex:claude-code:$U" "$U")" 1
eq "qualified_dash_never_equal (codex:- twice)" "$(same "codex:-" "codex:-")" 1
eq "qualified_dash_never_equal (claude-code:- vs dash)" "$(same "claude-code:-" -)" 1
eq "qualified_dash_never_equal (dash vs codex:-)" "$(same - "codex:-")" 1
eq "qualified_empty_never_equal (codex: twice)" "$(same "codex:" "codex:")" 1
eq "qualified_empty_never_equal (codex: vs claude-code:)" "$(same "codex:" "claude-code:")" 1
eq "qualified_dash_second_position (id vs codex:-)" "$(same "$U" "codex:-")" 1

echo "== publish_principal_root"
eq "root strips codex:"       "$(publish_principal_root "codex:$U")" "$U"
eq "root strips claude-code:" "$(publish_principal_root "claude-code:$U")" "$U"
eq "root leaves a bare id"    "$(publish_principal_root "$U")" "$U"

echo
echo "passed: $PASS  failed: $FAIL"
[ "$FAIL" -eq 0 ]
