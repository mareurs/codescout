#!/usr/bin/env bash
# End-to-end MCP smoke test for codescout.
# Calls the real binary over stdio via the `mcp` CLI tool.
# Run from the project root: ./tests/mcp-smoke-rust.sh
set -euo pipefail

PASS=0
FAIL=0
STEPS=0
FAILURES=""
RESULT=""

# ── Helpers ──────────────────────────────────────────────────────────────────

call() {
    local tool="$1" params="$2"
    STEPS=$((STEPS + 1))
    RESULT=$(mcp call "$tool" -p "$params" ce-test 2>/dev/null) || RESULT=""
    # `mcp call` prints the tool's reply as plain text. A fresh server is started per call,
    # so the first reply of every call also carries the injected `get_guide` body after an
    # `<!-- auto-injected` marker. That body names `symbols(name=`, `index` and `error`, so
    # an assertion on those strings would be satisfied by the guide and not by the tool.
    # Cut it off: RESULT is the tool's own reply.
    RESULT="${RESULT%%<!-- auto-injected*}"
}

assert_contains() {
    echo "$RESULT" | grep -q "$1"
}

# There is deliberately NO `assert_not_contains` here. `call()` blanks RESULT on a
# failed `mcp call` (see `|| RESULT=""` above), and an absence assertion over an empty
# string is satisfied by construction — so such a helper passes precisely when the
# server is unreachable, which is the one case a smoke test exists to catch.
# `assert_contains` is safe under the same fallback only because it fails loudly on an
# empty RESULT; the two are blind in opposite directions and are not a symmetric pair.
# If you need a negative check, assert RESULT is non-empty FIRST — the discriminator is
# already in the variable, unused. Re-adding a negated grep over $RESULT reds
# `no_smoke_script_asserts_absence_over_a_blankable_result`
# (tests/mcp_smoke_scripts_reference_real_tools.rs).
# docs/issues/archive/2026-09-09-assert-not-contains-cannot-fail-when-the-call-it-checks-failed.md

# There is deliberately NO JSON helper either (`assert_json_has`, `assert_symbols_found`):
# `mcp call` prints plain text, so `json.load(sys.stdin)` over RESULT never succeeds, and
# with `-f json` a `"content"` key is present on every reply, errors included. Assert on
# the text the server prints. Re-adding one reds `no_smoke_script_parses_the_reply_as_json`
# (tests/mcp_smoke_scripts_reference_real_tools.rs).

pass() {
    local steps="$1" name="$2"
    PASS=$((PASS + 1))
    printf "  \033[32mPASS\033[0m  [%d]  %s\n" "$steps" "$name"
}

fail() {
    local steps="$1" name="$2" reason="$3"
    FAIL=$((FAIL + 1))
    FAILURES="${FAILURES}\n  - ${name}: ${reason}"
    printf "  \033[31mFAIL\033[0m  [%d]  %s — %s\n" "$steps" "$name" "$reason"
}

# ── Setup ────────────────────────────────────────────────────────────────────

echo "Building codescout (release)..."
cargo build --release 2>&1 | tail -1
echo "done"
echo ""

export PATH="$HOME/.local/bin:$PATH"
mcp alias add ce-test ./target/release/codescout start --project .

# Temp files for extensions we don't have in the project
echo "test" > _test_smoke.py
echo "test" > _test_smoke.txt
echo "test: value" > _test_smoke.yaml
echo "a,b,c" > _test_smoke.csv

cleanup() {
    mcp alias remove ce-test 2>/dev/null || true
    rm -f _test_smoke.*
}
trap cleanup EXIT

# ── Category 1: Tool Access Control ─────────────────────────────────────────

echo "=== Tool Access Control ==="

# read_file no longer refuses source files (the `debug_enforce_symbol_tools` gate was
# retired; `is_source_path` now gates only multi-line edit_file edits). A long file of any
# kind comes back as an outline with a buffer handle; a short one comes back whole.
test_read_file_outlines_large_source() {
    call read_file '{"path": "src/main.rs"}'
    if assert_contains "Outline only" && assert_contains "symbols(path)"; then
        pass 1 "read_file outlines a large source file"
    else
        fail 1 "read_file outlines a large source file" "expected an outline naming symbols(path)"
    fi
}

test_read_file_returns_small_source() {
    call read_file '{"path": "_test_smoke.py"}'
    if assert_contains "^1 line" && assert_contains "^test$"; then
        pass 1 "read_file returns a short .py file whole"
    else
        fail 1 "read_file returns a short .py file whole" "expected '1 line' then the file's text"
    fi
}

test_read_file_allows_toml() {
    call read_file '{"path": "Cargo.toml"}'
    if assert_contains "(TOML)" && assert_contains "\[package\]"; then
        pass 1 "read_file reads .toml files (section outline)"
    else
        fail 1 "read_file reads .toml files (section outline)" "expected a TOML section outline"
    fi
}

test_read_file_allows_md() {
    call read_file '{"path": "README.md"}'
    if assert_contains "^# codescout"; then
        pass 1 "read_file reads .md files (heading map)"
    else
        fail 1 "read_file reads .md files (heading map)" "expected the README heading map"
    fi
}

test_read_file_allows_txt() {
    call read_file '{"path": "_test_smoke.txt"}'
    if assert_contains "^1 line" && assert_contains "^test$"; then
        pass 1 "read_file returns a short .txt file whole"
    else
        fail 1 "read_file returns a short .txt file whole" "expected '1 line' then the file's text"
    fi
}

test_replace_content_removed() {
    STEPS=$((STEPS + 1))
    # Require the server's own "unknown tool" refusal, not just a non-zero exit: an
    # unreachable server also exits non-zero, and passed this step before.
    local out rc=0
    out=$(mcp call replace_content -p '{"path":"x","old":"a","new":"b"}' ce-test 2>&1) || rc=$?
    if [ "$rc" -ne 0 ] && echo "$out" | grep -q "unknown tool"; then
        pass 1 "replace_content tool does not exist"
    else
        fail 1 "replace_content tool does not exist" "expected an 'unknown tool' refusal (exit $rc)"
    fi
}

test_outline_hint_names_symbol_tools() {
    call read_file '{"path": "src/main.rs"}'
    if assert_contains "Outline only" && assert_contains "symbols(path)" && assert_contains "symbols(name="; then
        pass 1 "source outline names the symbol tools"
    else
        fail 1 "source outline names the symbol tools" "missing tool suggestions"
    fi
}

test_read_file_outlines_large_source
test_read_file_returns_small_source
test_read_file_allows_toml
test_read_file_allows_md
test_read_file_allows_txt
test_replace_content_removed
test_outline_hint_names_symbol_tools

# ── Category 2: Symbol Navigation ───────────────────────────────────────────

echo ""
echo "=== Symbol Navigation ==="

test_symbols_overview() {
    call symbols '{"path": "src/server.rs", "depth": 0}'
    if assert_contains "CodeScoutServer" && assert_contains "route_tool_error"; then
        pass 1 "symbols overview finds symbols in server.rs"
    else
        fail 1 "symbols overview finds symbols in server.rs" "missing expected symbols"
    fi
}

test_symbols() {
    call symbols '{"name": "route_tool_error"}'
    if assert_contains "route_tool_error" && assert_contains "server.rs"; then
        pass 1 "symbols locates route_tool_error"
    else
        fail 1 "symbols locates route_tool_error" "function not found"
    fi
}

test_symbols_with_body() {
    call symbols '{"name": "route_tool_error", "include_body": true}'
    if assert_contains "RecoverableError" && assert_contains "Route a tool"; then
        pass 1 "symbols with include_body returns source"
    else
        fail 1 "symbols with include_body returns source" "body not returned or missing RecoverableError"
    fi
}

test_symbols_transport() {
    call symbols '{"path": "src/lsp/transport.rs"}'
    if assert_contains "read_message" && assert_contains "write_message"; then
        pass 1 "symbols returns signatures for transport.rs"
    else
        fail 1 "symbols returns signatures for transport.rs" "missing expected functions"
    fi
}

test_symbols_directory() {
    call symbols '{"name": "route_tool_error", "path": "src"}'
    if assert_contains "route_tool_error" && assert_contains "server.rs"; then
        pass 1 "symbols with a directory path finds symbols"
    else
        fail 1 "symbols with a directory path finds symbols" "not found via directory path"
    fi
}

test_symbols_glob() {
    call symbols '{"name": "route_tool_error", "path": "src/**/*.rs"}'
    if assert_contains "route_tool_error" && assert_contains "server.rs"; then
        pass 1 "symbols with a glob path finds symbols"
    else
        fail 1 "symbols with a glob path finds symbols" "not found via glob path"
    fi
}

test_symbols_name_path() {
    call symbols '{"name": "impl Tool for Symbols/call", "path": "src/tools/symbol/symbols.rs"}'
    if assert_contains "impl Tool for Symbols/call" && assert_contains "async fn call"; then
        pass 1 "symbols with name_path pattern finds method"
    else
        fail 1 "symbols with name_path pattern finds method" "name_path pattern not matched"
    fi
}

test_symbols_overview
test_symbols
test_symbols_with_body
test_symbols_transport
test_symbols_directory
test_symbols_glob
test_symbols_name_path

# ── Category 3: Search Workflows ────────────────────────────────────────────

echo ""
echo "=== Search ==="

test_search_pattern() {
    call grep '{"pattern": "RecoverableError"}'
    if assert_contains "RecoverableError"; then
        pass 1 "grep finds RecoverableError"
    else
        fail 1 "grep finds RecoverableError" "no matches found"
    fi
}

test_find_file() {
    call tree '{"glob": "**/transport.rs"}'
    if assert_contains "transport.rs"; then
        pass 1 "tree glob locates transport.rs"
    else
        fail 1 "tree glob locates transport.rs" "file not found"
    fi
}

# This scratch project has no semantic index, so the reply is an empty `results` envelope.
# That is all this step can show: the tool answered with its envelope. It does not rank.
test_semantic_search() {
    call semantic_search '{"query": "embedding pipeline chunking"}'
    if assert_contains '"results"' && assert_contains '"total"'; then
        pass 1 "semantic_search answers with a results envelope"
    else
        fail 1 "semantic_search answers with a results envelope" "no results envelope"
    fi
}

test_search_pattern
test_find_file
test_semantic_search

# ── Category 4: Non-code file access ────────────────────────────────────────

echo ""
echo "=== Non-code Files ==="

test_read_cargo_toml() {
    call read_file '{"path": "Cargo.toml"}'
    if assert_contains "(TOML)" && assert_contains "\[dependencies\]"; then
        pass 1 "read_file returns the Cargo.toml section outline"
    else
        fail 1 "read_file returns the Cargo.toml section outline" "outline not returned"
    fi
}

test_read_readme() {
    call read_file '{"path": "README.md"}'
    if assert_contains "## Quick Start"; then
        pass 1 "read_file returns the README.md heading map"
    else
        fail 1 "read_file returns the README.md heading map" "heading map not returned"
    fi
}

test_read_cargo_toml
test_read_readme

# ── Category 5: Multi-step Exploration ───────────────────────────────────────

echo ""
echo "=== Multi-step Exploration ==="

test_explore_error_routing() {
    call symbols '{"name": "route_tool_error"}'
    if ! assert_contains "route_tool_error"; then
        fail 1 "explore: find error routing function" "function not found"
        return
    fi
    call symbols '{"name": "route_tool_error", "include_body": true}'
    if assert_contains "RecoverableError" && assert_contains "CallToolResult"; then
        pass 2 "explore: find and read error routing implementation"
    else
        fail 2 "explore: find and read error routing implementation" "body incomplete"
    fi
}

test_explore_tool_architecture() {
    call symbols '{"path": "src/server.rs", "depth": 0}'
    if ! assert_contains "CodeScoutServer"; then
        fail 2 "explore: discover tool registry structure" "CodeScoutServer not in the overview"
        return
    fi
    call symbols '{"name": "CodeScoutServer/from_parts"}'
    if assert_contains "from_parts" && assert_contains "from_parts_with_env"; then
        pass 2 "explore: overview then drill into CodeScoutServer/from_parts"
    else
        fail 2 "explore: discover tool registry structure" "from_parts not found under CodeScoutServer"
    fi
}

test_explore_directory_then_drilldown() {
    call symbols '{"name": "OutputGuard", "path": "src/tools"}'
    if ! assert_contains "output.rs"; then
        fail 2 "explore: directory search then drilldown" "OutputGuard not found in src/tools"
        return
    fi
    call symbols '{"name": "OutputGuard", "path": "src/tools/output.rs", "include_body": true}'
    if assert_contains "max_results"; then
        pass 2 "explore: directory search then drilldown into OutputGuard"
    else
        fail 2 "explore: directory search then drilldown into OutputGuard" "body missing or incomplete"
    fi
}

test_explore_error_routing
test_explore_tool_architecture
test_explore_directory_then_drilldown

# ── Category 5: Write Lock Gate ─────────────────────────────────────────────

echo ""
echo "=== Write Lock Gate ==="

test_write_lock_gate_accepts_single_write() {
    call edit_file '{"path": "README.md", "old_string": "nonexistent_sentinel_XYZ", "new_string": "nonexistent_sentinel_XYZ"}'
    if assert_contains "error"; then
        pass 1 "edit_file returns an error for unmatched old_string (lock gate did not block path)"
    else
        fail 1 "edit_file with bad old_string" "expected an error response"
    fi
}

test_write_lock_gate_accepts_single_write

# ── Report ───────────────────────────────────────────────────────────────────

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
total=$((PASS + FAIL))
echo "$PASS/$total passed ($FAIL failed) in $STEPS steps"
if [ "$FAIL" -gt 0 ]; then
    printf "\nFailures:%b\n" "$FAILURES"
    exit 1
fi
