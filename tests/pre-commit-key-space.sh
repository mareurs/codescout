#!/usr/bin/env bash
# Discriminators for scripts/pre-commit-key-space.sh.
#
# WHY A STUB BINARY. The hook's own question is answered by `codescout key-gate --staged`,
# which has its own Rust tests (src/cli/key_gate.rs, src/librarian/identity/gate.rs). What
# this script owns, and what the cases below test, is the half around it: WHEN the binary is
# asked (a staged markdown file or a staged key-space file, and at no other time), WHICH
# binary counts as live, and what each answer becomes (exit 1 refuses and prints the problem
# lines; exit 2 or anything unexpected passes open and says so; exit 0 is silent). A stub
# reached through `CODESCOUT_BIN` prints a canned answer, so none of this needs a Rust
# toolchain, and a case cannot skip itself for a missing one and report success while testing
# nothing.
#
# The end-to-end path (a real binary, a real index, a real new collision) is what
# `codescout key-gate`'s own tests and the commit that introduced this hook ran by hand.
#
# ISOLATION. The throwaway repos never read the developer's git config, and the repo-building
# subshell drops GIT_DIR / GIT_WORK_TREE / GIT_INDEX_FILE, so this script is safe to run from
# inside a git hook.
set -uo pipefail

SELF_ROOT=$(cd "$(dirname "$0")/.." && pwd)
CHECK="$SELF_ROOT/scripts/pre-commit-key-space.sh"
PASS=0; FAIL=0

has() { # has <label> <haystack> <needle>
    case "$2" in
        *"$3"*) PASS=$((PASS + 1)) ;;
        *) FAIL=$((FAIL + 1)); echo "FAIL: $1"; echo "  expected to find: $3"; echo "  in: $2" ;;
    esac
}
lacks() { # lacks <label> <haystack> <needle>
    case "$2" in
        *"$3"*) FAIL=$((FAIL + 1)); echo "FAIL: $1"; echo "  expected NOT to find: $3"; echo "  in: $2" ;;
        *) PASS=$((PASS + 1)) ;;
    esac
}
eq() { # eq <label> <actual> <expected>
    if [ "$2" = "$3" ]; then PASS=$((PASS + 1))
    else FAIL=$((FAIL + 1)); echo "FAIL: $1"; echo "  actual:   $2"; echo "  expected: $3"; fi
}

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# Every git call this script makes goes through here: no global or system config, and none of
# the variables a surrounding hook would have exported.
g() {
    env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE \
        GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null \
        git -c user.name=t -c user.email=t@t "$@"
}

# A stub standing in for `codescout`. It answers `key-gate --help` (the liveness probe) with
# $STUB_HELP_RC, and any other `key-gate` call by logging its argv and the GIT_INDEX_FILE it
# inherited to $STUB_LOG, printing $STUB_OUT on stdout and exiting $STUB_RC. The probe is not
# logged, so "the log is empty" means the hook never ASKED.
STUB="$WORK/stub.sh"
cat > "$STUB" <<'STUBEOF'
#!/usr/bin/env bash
if [ "${1:-}" = "key-gate" ] && [ "${2:-}" = "--help" ]; then exit "${STUB_HELP_RC:-0}"; fi
if [ "${1:-}" = "--help" ]; then exit 0; fi
printf 'argv: %s\nindex: %s\n' "$*" "${GIT_INDEX_FILE:-<unset>}" >> "${STUB_LOG:-/dev/null}"
printf '%s' "${STUB_OUT:-}"
[ -z "${STUB_ERR:-}" ] || printf '%s\n' "$STUB_ERR" >&2
exit "${STUB_RC:-0}"
STUBEOF
chmod +x "$STUB"

newrepo() { # newrepo <file>... -> echoes a repo path with each file created and STAGED
    local p="$WORK/repo$RANDOM" f
    mkdir -p "$p"
    g -C "$p" init -q
    for f in "$@"; do
        mkdir -p "$p/$(dirname "$f")"
        printf '# t\n' > "$p/$f"
        g -C "$p" add -- "$f" >/dev/null
    done
    echo "$p"
}

LOG="$WORK/log"
run() { # run <repo> [VAR=value ...] -> sets OUT (stdout+stderr) and RC; the stub is CODESCOUT_BIN
    local repo="$1"; shift
    : > "$LOG"
    OUT=$(cd "$repo" && env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE \
        GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null \
        CODESCOUT_BIN="$STUB" STUB_LOG="$LOG" "$@" bash "$CHECK" 2>&1); RC=$?
    CALLS=$(cat "$LOG")
}

PROBLEM='key-gate: 1 new key problem in the staged changes (index vs HEAD)
  - the key `readme` is used by 2 documents: docs/a/readme.md, docs/b/readme.md
scope: 2 markdown files in the key space, 0 outside; no .codescout/key-space.toml in the index (nothing is excluded)'
NOTCHECKED='NOT CHECKED — .codescout/key-space.toml in the index is malformed: unknown field `exlcude`'

# --- 1. No live binary anywhere: degrade open, loudly --------------------------------------
# PATH is narrowed to the system dirs rather than emptied: the hook still needs `git` and
# `bash`, and `codescout` installs elsewhere. CODESCOUT_BIN names nothing.
R=$(newrepo docs/a/readme.md)
OUT=$(cd "$R" && env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE \
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null \
    PATH=/usr/bin:/bin CODESCOUT_BIN=/nonexistent/codescout bash "$CHECK" 2>&1); RC=$?
eq   "1 no live binary passes open" "$RC" "0"
has  "1 and says NOT CHECKED" "$OUT" "NOT CHECKED"
has  "1 and names where it looked" "$OUT" "CODESCOUT_BIN"
has  "1 and says why passing open is right" "$OUT" "cannot ask its question"

# --- 2. key-gate exit 1: refuse, with the problem lines on stderr ---------------------------
R=$(newrepo docs/a/readme.md)
run "$R" STUB_RC=1 STUB_OUT="$PROBLEM"
eq   "2 key-gate exit 1 refuses the commit" "$RC" "1"
has  "2 prints the problem line" "$OUT" "the key \`readme\` is used by 2 documents"
has  "2 prints the scope line" "$OUT" "scope: 2 markdown files in the key space"
has  "2 says REFUSED" "$OUT" "REFUSED"
# The refusal must reach STDERR: a hook's stdout is not shown to the committer by every
# front end, and this test reads stderr alone to prove it.
ERR_ONLY=$(cd "$R" && env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE \
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null \
    CODESCOUT_BIN="$STUB" STUB_LOG="$LOG" STUB_RC=1 STUB_OUT="$PROBLEM" \
    bash "$CHECK" 2>&1 >/dev/null)
has  "2 the problem lines are on stderr, not only stdout" "$ERR_ONLY" "docs/a/readme.md, docs/b/readme.md"

# --- 3. key-gate exit 2: it could not ask — pass open, print NOT CHECKED --------------------
run "$R" STUB_RC=2 STUB_OUT="$NOTCHECKED"
eq   "3 key-gate exit 2 passes open" "$RC" "0"
has  "3 prints the NOT CHECKED line" "$OUT" "NOT CHECKED — .codescout/key-space.toml in the index is malformed"
ERR_ONLY=$(cd "$R" && env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE \
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null \
    CODESCOUT_BIN="$STUB" STUB_LOG="$LOG" STUB_RC=2 STUB_OUT="$NOTCHECKED" \
    bash "$CHECK" 2>&1 >/dev/null)
has  "3 the NOT CHECKED line is on stderr, where a committer sees it" "$ERR_ONLY" "NOT CHECKED"
lacks "3 a cannot-ask is not a REFUSAL" "$OUT" "REFUSED"
has   "3 the reason is prefixed with the check's name" "$OUT" "key-space: NOT CHECKED — "
lacks "3 and it is told apart from a crash" "$OUT" "not a verdict"

# --- 4. Nothing relevant staged: silent, and the binary is never asked ----------------------
R=$(newrepo src/lib.rs notes.txt docs/page.mdx)
run "$R" STUB_RC=1 STUB_OUT="$PROBLEM"
eq   "4 no staged markdown, no key-space file: exit 0" "$RC" "0"
eq   "4 and prints nothing at all" "$(printf '%s' "$OUT" | wc -c | tr -d ' ')" "0"
eq   "4 and never calls the binary" "$CALLS" ""

# --- 5. A staged key-space file alone does call the binary ---------------------------------
R=$(newrepo .codescout/key-space.toml)
run "$R" STUB_RC=1 STUB_OUT="$PROBLEM"
eq   "5 a staged key-space file alone refuses when key-gate does" "$RC" "1"
has  "5 and the binary WAS called" "$CALLS" "key-gate"

# --- 6. Clean: exit 0, silent ---------------------------------------------------------------
R=$(newrepo docs/a/readme.md)
run "$R" STUB_RC=0 STUB_OUT="key-gate: OK — no new key problems in the staged changes (index vs HEAD)
"
eq   "6 key-gate exit 0 passes" "$RC" "0"
eq   "6 and is silent, so a clean commit costs the committer no output" \
     "$(printf '%s' "$OUT" | wc -c | tr -d ' ')" "0"
has  "6 yet the binary was called" "$CALLS" "key-gate"
# A note on key-gate's stderr (a path that was not checked) is a statement about the scope
# and survives a clean exit.
run "$R" STUB_RC=0 STUB_ERR="key-gate: note: 1 markdown path is not valid UTF-8, not checked"
eq   "6 a clean exit with a note still passes" "$RC" "0"
has  "6 and shows the note" "$OUT" "not valid UTF-8, not checked"

# --- 7. What the hook passes to key-gate ----------------------------------------------------
has  "7 passes --staged (judging the index against HEAD, not the whole tree)" "$CALLS" "--staged"
has  "7 passes the project root" "$CALLS" "--project $R"
# GIT_INDEX_FILE is inherited, never set or cleared: inside a pathspec commit git points it at
# the index the commit will use. Naming the real index here proves it arrives untouched.
run "$R" GIT_INDEX_FILE="$R/.git/index" STUB_RC=0
has  "7 an inherited GIT_INDEX_FILE reaches key-gate unchanged" "$CALLS" "index: $R/.git/index"
run "$R" STUB_RC=0
has  "7 and an unset one stays unset" "$CALLS" "index: <unset>"

# --- 8. Any markdown case triggers it; other extensions do not ------------------------------
R=$(newrepo docs/NOTES.MD)
run "$R" STUB_RC=0
has  "8 an upper-case .MD file is a markdown file" "$CALLS" "key-gate"

# A rename away from markdown stages the old path as a deletion; `--no-renames` keeps it
# visible (git would otherwise list only the new `.txt` path), and removing a document can
# change what a scope pattern covers.
R=$(newrepo docs/a/readme.md)
g -C "$R" commit -qm init
g -C "$R" mv docs/a/readme.md docs/a/readme.txt
run "$R" STUB_RC=0
has  "8 a rename away from .md is judged" "$CALLS" "key-gate"

# --- 9. A stale binary that lacks `key-gate` is not live ------------------------------------
# `--help` alone would pass for an old build; the probe is `key-gate --help`.
R=$(newrepo docs/a/readme.md)
run "$R" STUB_HELP_RC=2 STUB_RC=1 STUB_OUT="$PROBLEM"
eq   "9 a binary without key-gate passes open" "$RC" "0"
has  "9 and says NOT CHECKED" "$OUT" "NOT CHECKED"
eq   "9 and was never asked the question" "$CALLS" ""

# --- 10. An unexpected exit code is not a refusal -------------------------------------------
# A panic exits 101 and a missing loader exits 127; neither is "the commit adds a problem".
run "$R" STUB_RC=101
eq   "10 an unexpected exit code passes open" "$RC" "0"
has  "10 and says NOT CHECKED, naming the code" "$OUT" "101"
has  "10 NOT CHECKED is printed" "$OUT" "NOT CHECKED"

# --- 11. Probe order: a dead $CODESCOUT_BIN falls through to target/release -----------------
R=$(newrepo docs/a/readme.md)
mkdir -p "$R/target/release"
cp "$STUB" "$R/target/release/codescout"
: > "$LOG"
OUT=$(cd "$R" && env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE \
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null \
    PATH=/usr/bin:/bin CODESCOUT_BIN=/nonexistent/codescout STUB_LOG="$LOG" \
    STUB_RC=1 STUB_OUT="$PROBLEM" bash "$CHECK" 2>&1); RC=$?
eq   "11 target/release/codescout is used when CODESCOUT_BIN is dead" "$RC" "1"
has  "11 and it was the one asked" "$(cat "$LOG")" "key-gate"

echo
echo "key-space: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] || exit 1
