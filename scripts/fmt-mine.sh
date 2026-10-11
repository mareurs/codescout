#!/usr/bin/env bash
# Format proven own/CLEAN paths; --check is read-only and needs no identity.
# --exclusive-checkout authorizes a matching root by operator declaration.
# Exit 0: clean/formatted; 1: differences/refusal; 2: context/tool/usage error.
set -euo pipefail

main() {
    local CHECK_ONLY=0 CONTEXT="" EXCLUSIVE="" arg
    while [ "$#" -gt 0 ]; do
        arg="$1"; shift
        case "$arg" in
            --check) [ "$CHECK_ONLY" -eq 0 ] || { echo "fmt-mine: repeated --check" >&2; return 2; }; CHECK_ONLY=1 ;;
            --context|--exclusive-checkout)
                [ "$#" -gt 0 ] && [ -n "$1" ] || { echo "fmt-mine: $arg requires an absolute path" >&2; return 2; }
                if [ "$arg" = "--context" ]; then
                    [ -z "$CONTEXT" ] || return 2
                    CONTEXT="$1"
                else
                    [ -z "$EXCLUSIVE" ] || return 2
                    EXCLUSIVE="$1"
                fi
                shift ;;
            *) echo "fmt-mine: unknown argument $arg" >&2; return 2 ;;
        esac
    done
    [ "$CHECK_ONLY" -eq 0 ] || [ -z "$EXCLUSIVE" ] || {
        echo "fmt-mine: --check conflicts with --exclusive-checkout" >&2; return 2;
    }
    local ROOT SCRIPT_DIR PATHS PROVENANCE
    ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || { echo "fmt-mine: not inside a git repository" >&2; return 2; }
    cd "$ROOT"
    ROOT=$(pwd -P)
    SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
    PATHS="$SCRIPT_DIR/fmt_paths.py"
    PROVENANCE="${FMT_MINE_PROVENANCE:-$ROOT/scripts/file-provenance.py}"
    if [ -n "$EXCLUSIVE" ]; then
        python3 -c 'import pathlib,sys; p=pathlib.Path(sys.argv[1]); raise SystemExit(0 if p.is_absolute() and p.resolve(strict=True)==pathlib.Path(sys.argv[2]) else 2)' "$EXCLUSIVE" "$ROOT" || {
            echo "fmt-mine: exclusive declaration must match the actual Git root" >&2; return 2;
        }
    fi

    local CHECK_OUT CHECK_RC PATH_LIST
    CHECK_RC=0
    CHECK_OUT=$(cargo fmt -- --check 2>&1) || CHECK_RC=$?
    [ "$CHECK_RC" -ne 0 ] || return 0
    if [ "$CHECK_RC" -ne 1 ]; then
        echo "fmt-mine: cargo formatting check failed (exit $CHECK_RC)" >&2
        printf '%s\n' "$CHECK_OUT" >&2
        return 2
    fi
    PATH_LIST=$(printf '%s\n' "$CHECK_OUT" | python3 "$PATHS" diff "$ROOT") || {
        printf '%s\n' "$CHECK_OUT" >&2; return 2;
    }
    if [ "$CHECK_ONLY" -eq 1 ]; then
        printf '%s\n' "$CHECK_OUT"
        return 1
    fi
    local -a WOULD_CHANGE=() ALLOWED=() REFUSED=() FORMAT_ARGS=() CONTEXT_ARGS=()
    mapfile -t WOULD_CHANGE <<< "$PATH_LIST"
    [ -z "$CONTEXT" ] || CONTEXT_ARGS=(--context "$CONTEXT")
    local RESOLUTION RESOLVE_RC=0 STATE PROV="" PROV_RC=0 ALLOW_LIST REFUSE_LIST NOT_MINE_ROWS=""
    RESOLUTION=$(python3 "$SCRIPT_DIR/session_context.py" --workspace "$ROOT" "${CONTEXT_ARGS[@]}") || RESOLVE_RC=$?
    STATE=$(printf '%s' "$RESOLUTION" | python3 -c 'import json,sys; print(json.load(sys.stdin)["state"])') || return 2
    if [ "$RESOLVE_RC" -ne 0 ] && { [ -z "$EXCLUSIVE" ] || [ "$STATE" != "missing" ]; }; then
        echo "fmt-mine: session context has no referent or is unusable ($STATE)." >&2
        printf '%s\n' "$RESOLUTION" >&2
        echo '  Use --context <absolute file>, or an operator-established --exclusive-checkout <absolute Git root>.' >&2
        echo '  cargo fmt requires the same deliberate ownership decision on a shared checkout.' >&2
        return 2
    fi
    if [ -n "$EXCLUSIVE" ]; then
        ALLOWED=("${WOULD_CHANGE[@]}")
        echo "fmt-mine: operator-declared exclusive authorization for $ROOT" >&2
    else
        CONTEXT_SNAPSHOT=$(mktemp "${TMPDIR:-/tmp}/codescout-context.XXXXXX") || return 2
        trap 'rm -f -- "${CONTEXT_SNAPSHOT:-}"' EXIT
        printf '%s' "$RESOLUTION" | python3 -c 'import json,sys; json.dump(json.load(sys.stdin)["context"],sys.stdout)' > "$CONTEXT_SNAPSHOT" || return 2
        export CODESCOUT_CONTEXT_FILE="$CONTEXT_SNAPSHOT"
        PROV=$(REPO_ROOT="$ROOT" "$PROVENANCE" "${WOULD_CHANGE[@]}" 2>&1) || PROV_RC=$?
        if [ "$PROV_RC" -gt 1 ]; then
            echo "fmt-mine: attribution could not RUN (exit $PROV_RC); nothing written." >&2
            printf '%s\n' "$PROV" >&2
            return 2
        fi
        ALLOW_LIST=$(printf '%s\n' "$PROV" | python3 "$PATHS" allow "$ROOT" "${WOULD_CHANGE[@]}") || return 2
        REFUSE_LIST=$(printf '%s\n' "$PROV" | python3 "$PATHS" refuse "$ROOT" "${WOULD_CHANGE[@]}") || return 2
        [ -z "$ALLOW_LIST" ] || mapfile -t ALLOWED <<< "$ALLOW_LIST"
        [ -z "$REFUSE_LIST" ] || mapfile -t REFUSED <<< "$REFUSE_LIST"
        NOT_MINE_ROWS=$(printf '%s\n' "$PROV" | awk '
            $1=="MINE"||$1=="CLEAN" { drop=1; next }
            $1=="SHARED"||$1=="PEER"||$1=="UNKNOWN" { drop=0 }
            !drop')
        if printf '%s\n' "$PROV" | awk '$1=="CLEAN"{found=1} END{exit !found}'; then
            echo "fmt-mine: CLEAN paths have no uncommitted bytes and may be formatted." >&2
        fi
    fi
    if [ "${#ALLOWED[@]}" -eq 0 ]; then
        [ "${#REFUSED[@]}" -eq 0 ] || refuse_not_mine
        return 1
    fi
    for arg in "${ALLOWED[@]}"; do FORMAT_ARGS+=("./$arg"); done

    # Enumerate ALL descendants, including clean outside modules, before any write.
    local EMIT_OUT EMIT_RC=0 REACH_OUT REACH_RC=0 REACH_LIST collateral="" path
    EMIT_OUT=$(rustfmt --emit stdout --edition 2021 "${FORMAT_ARGS[@]}" 2>&1) || EMIT_RC=$?
    if [ "$EMIT_RC" -ne 0 ]; then
        echo "fmt-mine: could not enumerate formatter reach; nothing written." >&2
        printf '%s\n' "$EMIT_OUT" >&2
        return 2
    fi
    printf '%s\n' "$EMIT_OUT" | python3 "$PATHS" emit "$ROOT" "${ALLOWED[@]}" >/dev/null || return 2
    if [ "${#REFUSED[@]}" -gt 0 ]; then
        REACH_OUT=$(rustfmt --check --edition 2021 "${FORMAT_ARGS[@]}" 2>&1) || REACH_RC=$?
        if [ "$REACH_RC" -ne 0 ] && [ "$REACH_RC" -ne 1 ]; then
            echo "fmt-mine: could not determine descendant differences; nothing written." >&2
            return 2
        fi
        if [ "$REACH_RC" -eq 1 ]; then
            REACH_LIST=$(printf '%s\n' "$REACH_OUT" | python3 "$PATHS" diff "$ROOT") || return 2
            while IFS= read -r path; do
                for arg in "${REFUSED[@]}"; do
                    [ "$path" != "$arg" ] || collateral+="$path"$'\n'
                done
            done <<< "$REACH_LIST"
        fi
        if [ -n "$collateral" ]; then
            echo 'fmt-mine: REFUSED — rustfmt descends into every declared mod, including files this session does not own; nothing written.' >&2
            printf '%s' "$collateral" >&2
            refuse_not_mine
            return 1
        fi
    fi

    local FMT_PASSES=0 FMT_DIRTY="" FMT_RC=0
    while [ "$FMT_PASSES" -lt 3 ]; do
        rustfmt --edition 2021 "${FORMAT_ARGS[@]}" || return 2
        FMT_PASSES=$((FMT_PASSES + 1))
        FMT_RC=0
        FMT_DIRTY=$(rustfmt --check --edition 2021 "${FORMAT_ARGS[@]}" 2>&1) || FMT_RC=$?
        [ "$FMT_RC" -ne 0 ] || break
        [ "$FMT_RC" -eq 1 ] || { printf '%s\n' "$FMT_DIRTY" >&2; return 2; }
        printf '%s\n' "$FMT_DIRTY" | python3 "$PATHS" diff "$ROOT" >/dev/null || return 2
    done
    if [ "$FMT_RC" -ne 0 ]; then
        echo "fmt-mine: NOT a fixed point after $FMT_PASSES rustfmt pass(es)." >&2
        printf '%s\n' "$FMT_DIRTY" >&2
        return 1
    fi
    if [ -n "$EXCLUSIVE" ]; then
        echo "fmt-mine: formatted ${#ALLOWED[@]} file(s) with operator-declared authorization (fixed point after $FMT_PASSES pass(es))."
    else
        echo "fmt-mine: formatted ${#ALLOWED[@]} file(s) written by this session (fixed point after $FMT_PASSES pass(es))."
    fi
    if [ "${#REFUSED[@]}" -gt 0 ]; then refuse_not_mine; return 1; fi
}

refuse_not_mine() {
    echo "fmt-mine: REFUSED — these need formatting and are not this session's to write:" >&2
    printf '%s\n' "$NOT_MINE_ROWS" >&2
    cat >&2 <<'EOF'

  A [LIVE] row includes the Claude contact socket. It does not guarantee they hold anything to format.
  An unknown contact or a not-live marker does not establish that work is abandoned.
  UNKNOWN describes COVERAGE: cannot tell who owns these bytes. SHARED retains a peer's writes.
  There is no --force. An operator who has established exclusive control can use
  --exclusive-checkout <absolute Git root>; that authorization is operator-declared.
EOF
    printf '\n  The narrow form for the listed paths, after coordinating ownership:\n\n    rustfmt --edition 2021' >&2
    printf ' %q' "${REFUSED[@]}" >&2
    printf '\n' >&2
}


main "$@"
