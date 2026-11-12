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

# The PATH every hook run gets: only the directories that hold the commands the hook needs. It is
# built from where those commands are on THIS machine, because `/usr/bin:/bin` holds none of them
# on NixOS, and a hard-coded list there fails every case instead of skipping. It still leaves out
# any other directory, so a real `codescout` the developer installed is not found by the third
# probe. `codescout` itself is left out on purpose.
SYSPATH=""
for c in git bash mktemp env sed cat rm; do
    p=$(command -v "$c") || { echo "key-space: cannot run the tests: no \`$c\` on PATH" >&2; exit 2; }
    d=$(dirname "$p")
    case ":$SYSPATH:" in *":$d:"*) ;; *) SYSPATH="${SYSPATH:+$SYSPATH:}$d" ;; esac
done

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
# PATH is narrowed to the system dirs for every hook run, so the third probe (`command -v
# codescout`) cannot find a real binary the developer has installed: a case that makes the
# stub fail its probe would otherwise fall through to that binary and be judged by it. A case
# that wants `command -v` to succeed puts a stub directory first, as case 12 does.
run() { # run <repo> [VAR=value ...] -> sets OUT (stdout+stderr) and RC; the stub is CODESCOUT_BIN
    local repo="$1"; shift
    : > "$LOG"
    OUT=$(cd "$repo" && env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE \
        GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null PATH="$SYSPATH" \
        CODESCOUT_BIN="$STUB" STUB_LOG="$LOG" "$@" bash "$CHECK" 2>&1); RC=$?
    CALLS=$(cat "$LOG")
}

PROBLEM='key-gate: 1 new key problem in the staged changes (index vs HEAD)
  - the key `notes` is used by 2 documents: docs/a/notes.md, docs/b/notes.md
scope: 2 markdown files in the key space, 0 outside; no .codescout/key-space.toml in the index (nothing is excluded)'
NOTCHECKED='NOT CHECKED — .codescout/key-space.toml in the index is malformed: unknown field `exlcude`'

# --- 1. No live binary anywhere: degrade open, loudly --------------------------------------
# PATH is narrowed to the system dirs rather than emptied: the hook still needs `git` and
# `bash`, and `codescout` installs elsewhere. CODESCOUT_BIN names nothing.
R=$(newrepo docs/a/readme.md)
OUT=$(cd "$R" && env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE \
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null \
    PATH="$SYSPATH" CODESCOUT_BIN=/nonexistent/codescout bash "$CHECK" 2>&1); RC=$?
eq   "1 no live binary passes open" "$RC" "0"
has  "1 and says NOT CHECKED" "$OUT" "NOT CHECKED"
has  "1 and names where it looked" "$OUT" "CODESCOUT_BIN"
has  "1 and says why passing open is right" "$OUT" "cannot ask its question"

# --- 2. key-gate exit 1: refuse, with the problem lines on stderr ---------------------------
R=$(newrepo docs/a/readme.md)
run "$R" STUB_RC=1 STUB_OUT="$PROBLEM"
eq   "2 key-gate exit 1 refuses the commit" "$RC" "1"
has  "2 prints the problem line" "$OUT" "the key \`notes\` is used by 2 documents"
has  "2 prints the scope line" "$OUT" "scope: 2 markdown files in the key space"
has  "2 says REFUSED" "$OUT" "REFUSED"
# The refusal must reach STDERR: a hook's stdout is not shown to the committer by every
# front end, and this test reads stderr alone to prove it.
ERR_ONLY=$(cd "$R" && env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE \
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null \
    PATH="$SYSPATH" CODESCOUT_BIN="$STUB" STUB_LOG="$LOG" STUB_RC=1 STUB_OUT="$PROBLEM" \
    bash "$CHECK" 2>&1 >/dev/null)
has  "2 the problem lines are on stderr, not only stdout" "$ERR_ONLY" "docs/a/notes.md, docs/b/notes.md"

# --- 3. key-gate exit 2: it could not ask — pass open, print NOT CHECKED --------------------
run "$R" STUB_RC=2 STUB_OUT="$NOTCHECKED"
eq   "3 key-gate exit 2 passes open" "$RC" "0"
has  "3 prints the NOT CHECKED line" "$OUT" "NOT CHECKED — .codescout/key-space.toml in the index is malformed"
ERR_ONLY=$(cd "$R" && env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE \
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null \
    PATH="$SYSPATH" CODESCOUT_BIN="$STUB" STUB_LOG="$LOG" STUB_RC=2 STUB_OUT="$NOTCHECKED" \
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
    PATH="$SYSPATH" CODESCOUT_BIN=/nonexistent/codescout STUB_LOG="$LOG" \
    STUB_RC=1 STUB_OUT="$PROBLEM" bash "$CHECK" 2>&1); RC=$?
eq   "11 target/release/codescout is used when CODESCOUT_BIN is dead" "$RC" "1"
has  "11 and it was the one asked" "$(cat "$LOG")" "key-gate"

# --- 12. Probe order, last step: `codescout` on PATH is used when the other two are dead ----
# The stub directory goes first on a narrowed PATH, so the case is deterministic whatever the
# developer has installed.
R=$(newrepo docs/a/readme.md)
STUBDIR="$WORK/pathdir"; mkdir -p "$STUBDIR"; cp "$STUB" "$STUBDIR/codescout"
: > "$LOG"
OUT=$(cd "$R" && env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE \
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null \
    PATH="$STUBDIR:$SYSPATH" CODESCOUT_BIN=/nonexistent/codescout STUB_LOG="$LOG" \
    STUB_RC=1 STUB_OUT="$PROBLEM" bash "$CHECK" 2>&1); RC=$?
eq   "12 codescout on PATH is used when CODESCOUT_BIN and target/release are dead" "$RC" "1"
has  "12 and it was the one asked" "$(cat "$LOG")" "key-gate"
# The same stale-binary rule holds for the PATH candidate: one without `key-gate` is not live.
: > "$LOG"
OUT=$(cd "$R" && env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE \
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null \
    PATH="$STUBDIR:$SYSPATH" CODESCOUT_BIN=/nonexistent/codescout STUB_LOG="$LOG" \
    STUB_HELP_RC=2 STUB_RC=1 STUB_OUT="$PROBLEM" bash "$CHECK" 2>&1); RC=$?
eq   "12 a stale codescout on PATH passes open" "$RC" "0"
has  "12 and says NOT CHECKED" "$OUT" "NOT CHECKED"
eq   "12 and was never asked" "$(cat "$LOG")" ""

# --- 13. key-gate exit 2 with nothing on stdout: its stderr is shown -------------------------
# An argument-parser usage error exits 2 and writes only to stderr.
R=$(newrepo docs/a/readme.md)
run "$R" STUB_RC=2 STUB_OUT="" STUB_ERR="error: unexpected argument '--bogus' found"
eq   "13 exit 2 with an empty stdout still passes open" "$RC" "0"
has  "13 the stderr of key-gate is shown" "$OUT" "unexpected argument '--bogus' found"
has  "13 and it is still said to be NOT CHECKED" "$OUT" "NOT CHECKED"

# --- 14. A git that fails is not a silent pass -----------------------------------------------
# Passing open is right, but it must say so: a silent exit 0 looks identical to "nothing staged".
R=$(newrepo docs/a/readme.md)
printf 'not an index' > "$R/.git/index"
run "$R" STUB_RC=1 STUB_OUT="$PROBLEM"
eq   "14 a git that cannot list the staged files passes open" "$RC" "0"
has  "14 and says NOT CHECKED" "$OUT" "NOT CHECKED"
has  "14 naming the failing command" "$OUT" "git diff --cached"
eq   "14 and the binary is never asked" "$CALLS" ""
NOREPO="$WORK/norepo"; mkdir -p "$NOREPO"
OUT=$(cd "$NOREPO" && env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE \
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null GIT_CEILING_DIRECTORIES="$WORK" \
    PATH="$SYSPATH" CODESCOUT_BIN="$STUB" STUB_LOG="$LOG" bash "$CHECK" 2>&1); RC=$?
eq   "14 a directory git cannot place passes open" "$RC" "0"
has  "14 and says NOT CHECKED" "$OUT" "NOT CHECKED"
has  "14 naming the failing command" "$OUT" "git rev-parse"

# --- 15. A dead $CODESCOUT_BIN says so before the next candidate is used ---------------------
R=$(newrepo docs/a/readme.md)
mkdir -p "$R/target/release"
cp "$STUB" "$R/target/release/codescout"
: > "$LOG"
OUT=$(cd "$R" && env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE \
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null \
    PATH="$SYSPATH" CODESCOUT_BIN=/nonexistent/codescout STUB_LOG="$LOG" \
    STUB_RC=0 bash "$CHECK" 2>&1); RC=$?
eq   "15 the next candidate is still used" "$RC" "0"
has  "15 and the dead \$CODESCOUT_BIN is named" "$OUT" "/nonexistent/codescout"
has  "15 as not live" "$OUT" "not a live"

# --- 16. Probe order with two LIVE candidates: $CODESCOUT_BIN wins over target/release ----------
# Case 11 and 12 make the earlier candidates dead; none has two that are live, so the order itself
# was never asserted. The second stub logs a marker line on every call, probe included, so the
# marker is absent exactly when it was never even probed.
R=$(newrepo docs/a/readme.md)
mkdir -p "$R/target/release"
{ head -1 "$STUB"; echo 'echo "second stub was called" >> "${STUB_LOG:-/dev/null}"'; tail -n +2 "$STUB"; } > "$R/target/release/codescout"
chmod +x "$R/target/release/codescout"
: > "$LOG"
OUT=$(cd "$R" && env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE \
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null \
    PATH="$SYSPATH" CODESCOUT_BIN="$STUB" STUB_LOG="$LOG" \
    STUB_RC=1 STUB_OUT="$PROBLEM" bash "$CHECK" 2>&1); RC=$?
eq    "16 a live \$CODESCOUT_BIN is used" "$RC" "1"
has   "16 the first candidate was the one asked" "$(cat "$LOG")" "argv:"
lacks "16 the second candidate was never probed" "$(cat "$LOG")" "second stub was called"

echo
echo "key-space: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] || exit 1
