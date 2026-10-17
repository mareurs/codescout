#!/usr/bin/env bash
# End-to-end MCP smoke test for codescout against a Kotlin project.
# Calls the real binary over stdio via the `mcp` CLI tool.
# Run from the project root: ./tests/mcp-smoke-kotlin.sh
#
# Requires:
#   - `mcp` CLI (~/.local/bin/mcp)
#   - kotlin-language-server on PATH (without it the symbols steps depend on the
#     tree-sitter fallback and a red there is not a Kotlin-LSP signal)
#
# The target is a THROWAWAY COPY of tests/fixtures/kotlin-library, never a project on a
# developer's disk: codescout writes `.codescout/` state into whatever it is started
# against, and a script that names one machine's checkout goes stale the moment that
# checkout is reorganised (docs/issues/2026-10-16-mcp-smoke-kotlin-targets-a-project-
# layout-that-no-longer-exists.md). The fixture carries no README, .properties or .yml,
# so the copy gets one of each written below; `the_kotlin_smoke_script_targets_a_checked_
# in_fixture_and_every_path_it_reads_exists` (tests/mcp_smoke_scripts_reference_real_tools.rs)
# re-reads every `"path"` in this script against the fixture and those writes.
set -euo pipefail

PASS=0
FAIL=0
STEPS=0
FAILURES=""
RESULT=""

KOTLIN_FIXTURE="tests/fixtures/kotlin-library"
KOTLIN_PROJECT="$(mktemp -d)"

# ── Helpers ──────────────────────────────────────────────────────────────────

call() {
    local tool="$1" params="$2"
    STEPS=$((STEPS + 1))
    RESULT=$(mcp call "$tool" -p "$params" ce-kt-test 2>/dev/null) || RESULT=""
    # See tests/mcp-smoke-rust.sh: the reply is plain text and carries the injected guide
    # body after an `<!-- auto-injected` marker, which would satisfy a string assertion on
    # its own. RESULT is the tool's reply only.
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

# There is deliberately NO JSON helper either (`assert_json_has`): `mcp call` prints plain
# text, so `json.load(sys.stdin)` over RESULT never succeeds, and with `-f json` a
# `"content"` key is present on every reply, errors included. Assert on the text the
# server prints. Re-adding one reds `no_smoke_script_parses_the_reply_as_json`
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

cp -R "$KOTLIN_FIXTURE"/. "$KOTLIN_PROJECT"/
printf 'kotlin.code.style=official\n' > "$KOTLIN_PROJECT/gradle.properties"
printf '# Kotlin Library Fixture\n\nA throwaway copy used by the MCP smoke test.\n' > "$KOTLIN_PROJECT/README.md"
printf 'services:\n  catalog:\n    image: example/catalog\n' > "$KOTLIN_PROJECT/docker-compose.yml"

export PATH="$HOME/.local/bin:$PATH"
mcp alias add ce-kt-test ./target/release/codescout start --project "$KOTLIN_PROJECT"

cleanup() {
    mcp alias remove ce-kt-test 2>/dev/null || true
    rm -rf "$KOTLIN_PROJECT"
}
trap cleanup EXIT

# ── Category 1: Tool Access Control ─────────────────────────────────────────

echo "=== Tool Access Control ==="

# read_file no longer refuses source files (the `debug_enforce_symbol_tools` gate was
# retired; `is_source_path` now gates only multi-line edit_file edits), so the former
# "read_file blocks .kt/.kts" and "blocked error has hints" steps asserted a refusal the
# server does not make. tests/mcp-smoke-rust.sh covers the outline behaviour.
test_read_file_allows_properties() {
    call read_file '{"path": "gradle.properties"}'
    if assert_contains "kotlin.code.style"; then
        pass 1 "read_file allows .properties files"
    else
        fail 1 "read_file allows .properties files" "content not returned"
    fi
}

test_read_file_allows_md() {
    call read_file '{"path": "README.md"}'
    if assert_contains "^# "; then
        pass 1 "read_file allows .md files"
    else
        fail 1 "read_file allows .md files" "blocked or error"
    fi
}

test_read_file_allows_yml() {
    call read_file '{"path": "docker-compose.yml"}'
    if assert_contains "services"; then
        pass 1 "read_file allows .yml files"
    else
        fail 1 "read_file allows .yml files" "blocked or error"
    fi
}

test_read_file_allows_properties
test_read_file_allows_md
test_read_file_allows_yml

# ── Category 2: Symbol Navigation ───────────────────────────────────────────

echo ""
echo "=== Symbol Navigation ==="

test_symbols_overview_catalog() {
    call symbols '{"path": "src/main/kotlin/library/services/Catalog.kt"}'
    if assert_contains "Catalog" && assert_contains "search" && assert_contains "createDefaultCatalog"; then
        pass 1 "symbols overview finds symbols in Catalog.kt"
    else
        fail 1 "symbols overview finds symbols in Catalog.kt" "missing expected symbols"
    fi
}

test_symbols_search() {
    call symbols '{"name": "search", "path": "src/main/kotlin/library/services/Catalog.kt"}'
    if assert_contains "search" && assert_contains "Catalog"; then
        pass 1 "symbols locates search method"
    else
        fail 1 "symbols locates search method" "function not found"
    fi
}

test_symbols_with_body() {
    call symbols '{"name": "search", "path": "src/main/kotlin/library/services/Catalog.kt", "include_body": true}'
    if assert_contains "filter" && assert_contains "searchText"; then
        pass 1 "symbols with include_body returns search source"
    else
        fail 1 "symbols with include_body returns search source" "body not returned or missing expected code"
    fi
}

test_symbols_sealed_class() {
    call symbols '{"name": "SearchResult", "path": "src/main/kotlin/library/extensions/Results.kt", "include_body": true}'
    if assert_contains "SearchResult" && assert_contains "NotFound" && assert_contains "sealed class"; then
        pass 1 "symbols finds SearchResult sealed class with subclasses"
    else
        fail 1 "symbols finds SearchResult sealed class with subclasses" "sealed class not found"
    fi
}

test_symbols_book() {
    call symbols '{"path": "src/main/kotlin/library/models/Book.kt"}'
    if assert_contains "Book" && assert_contains "isAvailable"; then
        pass 1 "symbols returns Book members"
    else
        fail 1 "symbols returns Book members" "missing expected members"
    fi
}

test_symbols_overview_catalog
test_symbols_search
test_symbols_with_body
test_symbols_sealed_class
test_symbols_book

# ── Category 3: Search Workflows ────────────────────────────────────────────

echo ""
echo "=== Search ==="

test_search_pattern() {
    call grep '{"pattern": "suspend fun", "limit": 5}'
    if assert_contains "suspend fun"; then
        pass 1 "grep finds suspend fun declarations"
    else
        fail 1 "grep finds suspend fun declarations" "no matches found"
    fi
}

test_find_file_models() {
    call tree '{"glob": "**/models/*.kt"}'
    if assert_contains "Book" && assert_contains "Genre"; then
        pass 1 "tree glob locates model files"
    else
        fail 1 "tree glob locates model files" "model files not found"
    fi
}

test_semantic_search() {
    call semantic_search '{"query": "search catalog items matching a query"}'
    if assert_contains "search" || assert_contains "Catalog"; then
        pass 1 "semantic_search finds catalog-search content"
    elif assert_contains "index" || assert_contains "No index"; then
        pass 1 "semantic_search reports index status (index not built or stale)"
    else
        fail 1 "semantic_search finds catalog-search content" "no relevant results"
    fi
}

test_search_pattern
test_find_file_models
test_semantic_search

# ── Category 4: Non-code file access ────────────────────────────────────────

echo ""
echo "=== Non-code Files ==="

test_read_gradle_properties() {
    call read_file '{"path": "gradle.properties"}'
    if assert_contains "kotlin.code.style"; then
        pass 1 "read_file returns gradle.properties content"
    else
        fail 1 "read_file returns gradle.properties content" "content not returned"
    fi
}

test_read_readme() {
    call read_file '{"path": "README.md"}'
    if assert_contains "Kotlin Library Fixture"; then
        pass 1 "read_file returns README.md with project name"
    else
        fail 1 "read_file returns README.md with project name" "blocked or error"
    fi
}

test_read_gradle_properties
test_read_readme

# ── Category 5: Multi-step Exploration ───────────────────────────────────────

echo ""
echo "=== Multi-step Exploration ==="

test_explore_search_flow() {
    # Step 1: Find the Catalog class
    call symbols '{"path": "src/main/kotlin/library/services/Catalog.kt"}'
    if ! assert_contains "Catalog" || ! assert_contains "search"; then
        fail 1 "explore search: find Catalog structure" "class or methods not found"
        return
    fi
    # Step 2: Read search method body
    call symbols '{"name": "search", "path": "src/main/kotlin/library/services/Catalog.kt", "include_body": true}'
    if assert_contains "searchText" && assert_contains "filter"; then
        pass 2 "explore search: discover search flow (find class → read method)"
    else
        fail 2 "explore search: discover search flow (find class → read method)" "search body incomplete"
    fi
}

test_explore_model_layer() {
    # Step 1: Find all model files (need enough results to include both)
    call tree '{"glob": "**/models/*.kt"}'
    if ! assert_contains "Book" || ! assert_contains "Genre"; then
        fail 1 "explore models: find model layer" "model files not found"
        return
    fi
    # Step 2: Overview a specific model
    call symbols '{"path": "src/main/kotlin/library/models/Genre.kt"}'
    if assert_contains "Genre" && assert_contains "label"; then
        pass 2 "explore models: discover model layer (find files → symbols)"
    else
        fail 2 "explore models: discover model layer (find files → list functions)" "functions not found"
    fi
}

test_explore_search_flow
test_explore_model_layer

# ── Report ───────────────────────────────────────────────────────────────────

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
total=$((PASS + FAIL))
echo "$PASS/$total passed ($FAIL failed) in $STEPS steps"
if [ "$FAIL" -gt 0 ]; then
    printf "\nFailures:%b\n" "$FAILURES"
    exit 1
fi
