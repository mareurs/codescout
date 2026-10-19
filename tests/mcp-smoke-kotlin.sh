#!/usr/bin/env bash
# End-to-end MCP smoke test for codescout against a Kotlin project.
# Calls the real binary over stdio via the `mcp` CLI tool.
# Run from the project root: ./tests/mcp-smoke-kotlin.sh
#
# Requires:
#   - `mcp` CLI (~/.local/bin/mcp)
#   - `kotlin-lsp` on PATH (the server codescout starts for Kotlin; without it the symbols
#     steps fall back to tree-sitter and a red there is not a Kotlin-LSP signal)
#
# The target is a THROWAWAY COPY of tests/fixtures/kotlin-library, never a project on a
# developer's disk: codescout writes `.codescout/` state into whatever it is started
# against, and a script that names one machine's checkout goes stale the moment that
# checkout is reorganised.
# docs/issues/2026-10-16-mcp-smoke-kotlin-targets-a-project-layout-that-no-longer-exists.md
# The fixture carries no README, .properties or .yml, so the copy gets one of each written
# below. The copy is of the TRACKED files only (git archive), so untracked local state in
# the fixture directory (.codescout/, .code-explorer/) cannot change the result.
# TMPDIR is pointed inside the per-run directory, so the kotlin-lsp system, gradle and
# analyzer-home directories (all under temp_dir()) are removed with it by the trap.
# The static gate for all of this is
# the_kotlin_smoke_script_targets_a_checked_in_fixture_and_every_path_it_reads_exists
# in tests/mcp_smoke_scripts_reference_real_tools.rs
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

PASS=0
FAIL=0
STEPS=0
FAILURES=""
RESULT=""

KOTLIN_FIXTURE="tests/fixtures/kotlin-library"
KOTLIN_WORK="$(mktemp -d)"
KOTLIN_PROJECT="$KOTLIN_WORK/project"
KOTLIN_ALIAS="ce-kt-test-$$"

# The trap is armed BEFORE anything below can fail (the release build, the copy, the alias
# add), or a failure in between leaks the directory. Everything here is safe to run early.
cleanup() {
    mcp alias remove "$KOTLIN_ALIAS" 2>/dev/null || true
    # The kotlin-lsp mux (idle-timeout 300s) outlives the last call and re-creates its
    # directories under $TMPDIR as it shuts down, so removing them first leaves a copy
    # behind (measured: the run's temp directory reappeared after the rm). $KOTLIN_WORK is
    # unique to this run and is on the mux's and the server's command lines (--cwd,
    # --system-path), so it selects exactly this run's processes.
    pkill -f -- "$KOTLIN_WORK" 2>/dev/null || true
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        pgrep -f -- "$KOTLIN_WORK" >/dev/null 2>&1 || break
        sleep 1
    done
    rm -rf "$KOTLIN_WORK"
}
trap cleanup EXIT

# ── Helpers ──────────────────────────────────────────────────────────────────

call() {
    local tool="$1" params="$2"
    STEPS=$((STEPS + 1))
    RESULT=$(mcp call "$tool" -p "$params" "$KOTLIN_ALIAS" 2>/dev/null) || RESULT=""
    # See tests/mcp-smoke-rust.sh: the reply is plain text and carries the injected guide
    # body after an `<!-- auto-injected` marker, which would satisfy a string assertion on
    # its own. RESULT is the tool's reply only.
    # Since 2026-10-18 the marker opens on a line of its own (`trailing_block`), so a line-oriented
    # strip no longer eats the tool's last line. Before that the marker followed the last line
    # WITHOUT a newline, and doing it that way is how a body, and an overview, once looked one
    # line short. This parameter expansion cuts at the marker whichever binary answered, so it
    # stays: an older binary on PATH still glues.
    # docs/issues/2026-10-17-symbols-include-body-on-kotlin-lsp-results-is-shifted-up-one-line.md
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

mkdir -p "$KOTLIN_PROJECT" "$KOTLIN_WORK/tmp"
if git -C "$REPO_ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    git -C "$REPO_ROOT" archive HEAD "$KOTLIN_FIXTURE" | tar -x --strip-components=3 -C "$KOTLIN_PROJECT"
else
    echo "warning: not a git checkout, copying the fixture directory as it is (untracked files included)"
    cp -R "$REPO_ROOT/$KOTLIN_FIXTURE"/. "$KOTLIN_PROJECT"/
fi
printf 'kotlin.code.style=official\n' > "$KOTLIN_PROJECT/gradle.properties"
printf '# Kotlin Library Fixture\n\nA throwaway copy used by the MCP smoke test.\n' > "$KOTLIN_PROJECT/README.md"
printf 'services:\n  catalog:\n    image: example/catalog\n' > "$KOTLIN_PROJECT/docker-compose.yml"

export PATH="$HOME/.local/bin:$PATH"
export TMPDIR="$KOTLIN_WORK/tmp"
mcp alias add "$KOTLIN_ALIAS" ./target/release/codescout start --project "$KOTLIN_PROJECT"

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

# Each `mcp call` starts a fresh codescout, and the first call against a project can reach it
# while its Kotlin mux is still starting; that used to answer `0 matches`, and this script
# retried it. The client now waits for a mux another process is starting
# (docs/issues/2026-10-17-symbols-name-lookup-on-kotlin-lsp-says-0-matches-for-a-symbol-that-exists.md),
# so a `0 matches` here is a real failure and is not retried.

# Pass criteria below are what the LIVE output of this fixture contains, and are written so a
# wrong answer cannot satisfy them: each is a member or function name (with its line range)
# that appears nowhere in the request. A single-file overview prints `<Kind>  <name>  L<a>-<b>`;
# the top-level functions are asserted too, since a fixture whose KDoc held an unterminated
# nested comment once listed none of them.
# docs/issues/2026-10-17-symbols-overview-of-a-kotlin-file-omits-members-and-top-level-functions.md
test_symbols_overview_catalog() {
    call symbols '{"path": "src/main/kotlin/library/services/Catalog.kt"}'
    if assert_contains "add  *L15-17" && assert_contains "search  *L20-21" \
        && assert_contains "stats  *L24" \
        && assert_contains "createNamedCatalog  *L41-44" && assert_contains "toSearchText  *L51"; then
        pass 1 "symbols overview finds Catalog members and top-level functions in Catalog.kt"
    else
        fail 1 "symbols overview finds Catalog members and top-level functions in Catalog.kt" "missing expected symbols"
    fi
}

# `$` anchors the name-path: a `Catalog/searchAsync` or a top-level `searchAsync` would not do.
test_symbols_search() {
    call symbols '{"name": "search", "path": "src/main/kotlin/library/services/Catalog.kt"}'
    if assert_contains "20-21 *Catalog/search$"; then
        pass 1 "symbols locates the Catalog/search method"
    else
        fail 1 "symbols locates the Catalog/search method" "function not found"
    fi
}

# `search` is an expression-bodied member (`fun search(...) =` then the expression on the next
# line); the body must reach the LAST line, `items.filter { it.searchText()... }`, which is the
# line an off-by-one at the end of the slice drops.
# docs/issues/2026-10-17-symbols-include-body-on-kotlin-lsp-results-is-shifted-up-one-line.md
test_symbols_with_body() {
    call symbols '{"name": "Catalog/search", "path": "src/main/kotlin/library/services/Catalog.kt", "include_body": true}'
    if assert_contains "fun search(query: String)" && assert_contains "items.filter" \
        && assert_contains "searchText"; then
        pass 1 "symbols with include_body returns the whole search source"
    else
        fail 1 "symbols with include_body returns the whole search source" "body not returned or missing expected code"
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
    if assert_contains "isAvailable"; then
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

# This scratch copy has no semantic index, so the reply is an empty `results` envelope.
# That is all this step can show: the tool answered with its envelope. It does not rank.
# (The earlier form asserted on "search"/"index" text, which the empty envelope does not
# contain, and which only an ERROR reply ("stack search failed") could satisfy.)
test_semantic_search() {
    call semantic_search '{"query": "search catalog items matching a query"}'
    if assert_contains '"results"' && assert_contains '"total"'; then
        pass 1 "semantic_search answers with a results envelope"
    else
        fail 1 "semantic_search answers with a results envelope" "no results envelope"
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
    if ! assert_contains "add  *L15-17" || ! assert_contains "search  *L20-21"; then
        fail 1 "explore search: find Catalog structure" "class or methods not found"
        return
    fi
    # Step 2: Read search method body
    call symbols '{"name": "search", "path": "src/main/kotlin/library/services/Catalog.kt", "include_body": true}'
    if assert_contains "fun search(query: String)" && assert_contains "20-21 *Catalog/search$"; then
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
    if assert_contains "label"; then
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
