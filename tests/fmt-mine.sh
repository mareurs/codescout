#!/usr/bin/env bash
# tests/fmt-mine.sh — cases for scripts/fmt-mine.sh
#
# WHAT THIS GUARDS, AND WHY THE MESSAGE ASSERTIONS ARE NOT DECORATION
#
# The first version of `fmt-mine.sh` partitioned the provenance scan with
# `sed -n 's|^\(SHARED\|PEER\|UNKNOWN\)  *\(.*\)$|\2|p'` — where `|` was BOTH the
# `s///` delimiter and the intended alternation. sed reads an escaped delimiter as
# a literal, so that pattern matched the string "SHARED|PEER|UNKNOWN" and never
# fired. `NOT_MINE` was always empty, every refusal fell through to the wrong
# branch, and the script reported "nothing attributable to this session needs
# formatting" for a file a live peer owned.
#
# **It still refused, and wrote nothing.** A suite asserting only on the exit code
# and on "was the file left alone" is GREEN against that defect — both are correct.
# What was broken was the half no exit code reaches: where the refusal sends you.
# `CLAUDE.md` § *Testing Discipline* — a suite tests a guard's PREDICATE and never
# its REMEDY TEXT, and the remedy is where the reader's next action comes from.
#
# So the cases below assert on the exit code, on the bytes, AND on the message
# naming the owner's sid and socket. Deleting the sid line from the refusal keeps
# every outcome assertion green and reds exactly one — which is the regression that
# actually happened.
#
# FIXTURE: a throwaway cargo project, not this repo. `cargo fmt` needs a real
# manifest, and a suite that de-formats files in the checkout it runs from would be
# rewriting other sessions' trees to test a script whose whole purpose is not doing
# that.
set -u

SRC="$(cd "$(dirname "$0")/../scripts" && pwd)"
TOOL="$SRC/fmt-mine.sh"
PASS=0
FAIL=0

has() { # has <label> <haystack> <needle>
    if printf '%s' "$2" | grep -qF -- "$3"; then
        PASS=$((PASS + 1)); echo "  ok   $1"
    else
        FAIL=$((FAIL + 1)); echo "  FAIL $1 -- expected to find: $3"
        printf '       got: %s\n' "$2" | head -12
    fi
}

hasnt() { # hasnt <label> <haystack> <needle>
    if printf '%s' "$2" | grep -qF -- "$3"; then
        FAIL=$((FAIL + 1)); echo "  FAIL $1 -- expected NOT to find: $3"
    else
        PASS=$((PASS + 1)); echo "  ok   $1"
    fi
}

eq() { # eq <label> <actual> <expected>
    if [ "$2" = "$3" ]; then
        PASS=$((PASS + 1)); echo "  ok   $1"
    else
        FAIL=$((FAIL + 1)); echo "  FAIL $1 -- expected '$3', got '$2'"
    fi
}

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# Stubs. Attribution is stubbed rather than driven by real transcripts because the
# thing under test is the PARTITION and the REFUSAL, not the scan — and a suite that
# needed this machine's live profiles would run nowhere else.
mkstub() { # mkstub <name> <verdict> [extra lines...]
    local f="$WORK/prov-$1.sh"
    local rc=0
    # EXIT CODE FIDELITY IS LOAD-BEARING. The real tool returns
    # `1 if unknown == len(paths) else 0` — an all-UNKNOWN scan exits 1 while having run
    # perfectly. A stub that exits 0 for UNKNOWN hides the branch where the caller reads
    # that 1 as failure, which is exactly the defect this suite failed to catch the first
    # time: the guard refused correctly and blamed the scan. Do not "simplify" this to a
    # uniform exit 0.
    [ "$2" = "UNKNOWN" ] && rc=1
    {
        echo '#!/usr/bin/env bash'
        # A FILE, not stderr. The first version echoed a marker to stderr, which
        # `fmt-mine.sh` captures into $PROV and parses — so the marker never reached
        # the script's own output and the "did the scan run?" control could only ever
        # fail. A control asserting on a channel the subject CONSUMES observes nothing.
        echo '[ -n "${FMT_MINE_TEST_MARKER:-}" ] && : > "$FMT_MINE_TEST_MARKER"'
        echo 'for f in "$@"; do'
        printf '  printf "%%-9s %%s\\n" "%s" "$f"\n' "$2"
        shift 2
        for line in "$@"; do printf '  printf "%%s\\n" %s\n' "$(printf '%q' "$line")"; done
        echo 'done'
        echo "exit $rc"
    } > "$f"
    chmod +x "$f"
    echo "$f"
}

PEER_STUB=$(mkstub peer PEER \
    "          written by 5399543d-22d6-4ed9-9ebb-876be459989f  [LIVE]" \
    '            ask it: SendMessage to="uds:/run/user/1000/cc-socks/1849060.sock"')
UNKNOWN_STUB=$(mkstub unknown UNKNOWN "          no record of any session writing this path in the window.")
SHARED_STUB=$(mkstub shared SHARED "          written by THIS session and 5399543d")
MINE_STUB=$(mkstub mine MINE "          written by THIS session")

# A project per case: `cargo fmt` rewrites in place, so cases must not share a tree.
newproj() {
    local d="$WORK/p$RANDOM$RANDOM"
    mkdir -p "$d/src"
    cat > "$d/Cargo.toml" <<'EOF'
[package]
name = "fixture"
version = "0.1.0"
edition = "2021"
EOF
    git -C "$d" init -q
    echo "$d"
}
deformed() { printf 'pub fn f(  ) -> u32     {\n    1\n}\n' > "$1/src/lib.rs"; }
formatted() { printf 'pub fn f() -> u32 {\n    1\n}\n' > "$1/src/lib.rs"; }

run() { # run <projdir> <stub|-> [env...] -> sets OUT and RC
    local d="$1" stub="$2"; shift 2
    local prov=()
    [ "$stub" != "-" ] && prov=(FMT_MINE_PROVENANCE="$stub")
    rm -f "$WORK/consulted"
    OUT=$(cd "$d" && env "${prov[@]}" CLAUDE_CODE_SESSION_ID=test-sid \
        FMT_MINE_TEST_MARKER="$WORK/consulted" "$@" "$TOOL" 2>&1)
    RC=$?
}

consulted() { [ -f "$WORK/consulted" ] && echo yes || echo no; }

echo "== 1. clean tree: the fast path costs nothing and says nothing =="
P=$(newproj); formatted "$P"
run "$P" "$MINE_STUB"
eq "clean tree exits 0" "$RC" "0"
eq "clean tree is silent" "$OUT" ""
hasnt "clean tree never reaches the scan" "$(consulted)" "yes"

echo "== 2. MINE: formats, and the bytes actually change =="
P=$(newproj); deformed "$P"
run "$P" "$MINE_STUB"
eq "MINE exits 0" "$RC" "0"
has "MINE says what it did" "$OUT" "formatted"
has "MINE reached the scan" "$(consulted)" "yes"
eq "MINE rewrote the file" "$(grep -c 'pub fn f() -> u32 {' "$P/src/lib.rs")" "1"

echo "== 3. --check: reports without writing =="
P=$(newproj); deformed "$P"
run "$P" "$MINE_STUB" -- --check
# NOTE: --check is passed through `run`'s trailing args; the script reads \$1.
OUT=$(cd "$P" && FMT_MINE_PROVENANCE="$MINE_STUB" CLAUDE_CODE_SESSION_ID=test-sid "$TOOL" --check 2>&1); RC=$?
eq "--check exits 0" "$RC" "0"
has "--check names the file" "$OUT" "src/lib.rs"
eq "--check wrote NOTHING" "$(grep -c 'pub fn f(  )' "$P/src/lib.rs")" "1"

echo "== 4. PEER: refuses, leaves the bytes, and NAMES WHO TO ASK =="
P=$(newproj); deformed "$P"
run "$P" "$PEER_STUB"
eq "PEER exits 1" "$RC" "1"
eq "PEER left the file unwritten" "$(grep -c 'pub fn f(  )' "$P/src/lib.rs")" "1"
has "PEER says REFUSED" "$OUT" "REFUSED"
# The four below are the remedy half. Each reds on a message regression that every
# assertion above stays green through -- which is the defect this suite was born from.
has "PEER names the owning sid" "$OUT" "5399543d-22d6-4ed9-9ebb-876be459989f"
has "PEER names the socket to reach them" "$OUT" "uds:/run/user/1000/cc-socks/1849060.sock"
has "PEER names the LIVE marker" "$OUT" "[LIVE]"
has "PEER says why there is no --force" "$OUT" "no --force"

echo "== 5. UNKNOWN is 'cannot tell', never 'nobody owns it' =="
P=$(newproj); deformed "$P"
run "$P" "$UNKNOWN_STUB"
eq "UNKNOWN exits 1" "$RC" "1"
eq "UNKNOWN left the file unwritten" "$(grep -c 'pub fn f(  )' "$P/src/lib.rs")" "1"
has "UNKNOWN refuses" "$OUT" "REFUSED"
has "UNKNOWN is framed as coverage" "$OUT" "COVERAGE"
# The real tool exits 1 on an all-UNKNOWN scan. Reading that as a failed scan is a
# DIFFERENT refusal with a different diagnosis, and it sends the reader to debug a scan
# that worked. Both refuse and both fail closed, so only the message separates them.
hasnt "UNKNOWN is not misreported as a broken scan" "$OUT" "could not RUN"
eq "UNKNOWN exits 1, not 2" "$RC" "1"

echo "== 6. SHARED is refused too -- a peer's bytes are still in there =="
# Without this case a partition handling only PEER and UNKNOWN passes everything above.
P=$(newproj); deformed "$P"
run "$P" "$SHARED_STUB"
eq "SHARED exits 1" "$RC" "1"
eq "SHARED left the file unwritten" "$(grep -c 'pub fn f(  )' "$P/src/lib.rs")" "1"
has "SHARED refuses" "$OUT" "REFUSED"

echo "== 7. no session id: fails CLOSED and says why =="
P=$(newproj); deformed "$P"
rm -f "$WORK/consulted"
OUT=$(cd "$P" && env -u CLAUDE_CODE_SESSION_ID FMT_MINE_PROVENANCE="$MINE_STUB" \
    FMT_MINE_TEST_MARKER="$WORK/consulted" "$TOOL" 2>&1); RC=$?
eq "no sid exits 2, not 1" "$RC" "2"
eq "no sid wrote nothing" "$(grep -c 'pub fn f(  )' "$P/src/lib.rs")" "1"
has "no sid says the referent is missing" "$OUT" "has no referent"
has "no sid names the fallback for a solo checkout" "$OUT" "cargo fmt"
hasnt "no sid never reached the scan" "$(consulted)" "yes"

echo
echo "fmt-mine: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] || exit 1
