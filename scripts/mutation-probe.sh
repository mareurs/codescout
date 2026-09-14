#!/usr/bin/env bash
# mutation-probe.sh — run a mutation where no peer can read it, and leave a record either way.
#
# WHY THIS EXISTS
#
# Mutation testing requires putting a KNOWN-BAD version of the code into a tree,
# running the suite, and reading the failure as evidence. On a shared checkout
# that failure is published to every other session's `cargo test`, where it is
# byte-identical to a real regression. Measured instances and the full account:
# `docs/issues/2026-09-08-an-armed-mutation-is-a-deliberate-red-no-observer-can-distinguish.md`.
#
# WHY ISOLATION RATHER THAN A WARNING
#
# That record spent two weeks on the assumption that isolating the mutation was
# too expensive, so the only available remedy was to ANNOUNCE it and hope peers
# read the announcement. Both halves were wrong.
#
# Announcing backfires: the prescribed response is "stand down", and complying is
# what removes the observer whose build log would have resolved the arming
# session's own anomaly. The better the announcement works, the less it can
# observe (`observer-blindness:OB-23`).
#
# And the cost was never measured. Measured 2026-09-14, predictions written first:
#
#     main checkout, warm, `cargo test --lib <module>`        13.8 s
#     fresh worktree, COLD, same command                      87   s   (2.8 G)
#     that worktree, second run                               11   s
#
# A kept worktree is FASTER per run than the shared tree, because its `target/`
# is small and uncontended while the shared one is ~108 G and fought over by
# several sessions. So isolation is not a sacrifice bought for safety; after 87 s
# paid once it is the cheaper path. That is `CLAUDE.md` § *Observer Blindness*
# position 3 — make the correct path end in a safe state — rather than a policy
# someone has to remember.
#
# WHAT `--shared` IS FOR, AND WHY IT IS NOT A LOOPHOLE
#
# Some mutations cannot be isolated, because their SUBJECT is the shared
# checkout. Two in this repo, both verified rather than imagined:
#
#   * `src/agent/build_check.rs` gates on `checkout_is_shared(&live_sessions(),
#     root)` — in a worktree only you are present, so it returns false and the
#     code under test never runs.
#   * `scripts/fmt-mine.sh` refuses based on OTHER live sessions owning files.
#
# For those, `--shared` mutates the real tree and the window is genuinely open.
# It is the one path where the marker below is load-bearing.
#
# THE MARKER IS PASSIVE, DELIBERATELY
#
# It labels; it does not prescribe. It names a red a reader was going to see
# anyway and asks for nothing, so their build still happens and their log still
# exists. Anything that reads as "a peer is mid-mutation, stand down" reproduces
# OB-23 exactly. It is written UNCONDITIONALLY at arm time, in the same
# invocation that arms — so it cannot be defeated by anyone's compliance, and it
# does not depend on the arming session remembering a separate step.
#
# Read by `scripts/attribute-red.py`, which `src/tools/run_command/attribution.rs`
# already materializes and runs on every failure-shaped command. A marker nothing
# reads is decoration.
set -uo pipefail

usage() {
    cat >&2 <<'USAGE'
mutation-probe.sh — run one mutation in an isolated tree and report kill/survive.

  --file <path>        source file to mutate, repo-relative
  --find <literal>     exact text to replace. Must occur EXACTLY ONCE.
  --replace <literal>  text to put in its place ("" deletes it)
  --shared             mutate the real checkout instead of a worktree.
                       Only for mutations whose subject IS the shared tree.
  --                   everything after this is the test command

  mutation-probe.sh --file src/a.rs --find 'st.defer();' --replace '' \
      -- cargo test --lib agent::build_check
USAGE
    exit 2
}

FILE=""; FIND=""; REPL=""; SHARED=0
while [ $# -gt 0 ]; do
    case "$1" in
        --file) FILE="${2:-}"; shift 2 ;;
        --find) FIND="${2:-}"; shift 2 ;;
        --replace) REPL="${2:-}"; shift 2 ;;
        --shared) SHARED=1; shift ;;
        --) shift; break ;;
        *) usage ;;
    esac
done
[ -n "$FILE" ] && [ -n "$FIND" ] && [ $# -gt 0 ] || usage

ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || { echo "mutation-probe: not a git checkout" >&2; exit 2; }

# Count occurrences of a LITERAL in a file. Python, not `grep -F -c`, and the
# substitution is the whole point: `grep -c` counts matching LINES, so it is
# wrong twice over. Given a multi-line literal it treats each line as a separate
# alternative and returns their union — a two-line pattern here returned 11.
# Given a single-line one it still counts lines, so a line holding the pattern
# twice reads as 1. Neither errors; both return a plausible integer meaning
# something else, which is precisely the defect class this script exists to
# study. `str.count` is also exactly what the patcher below uses, so the guard
# and the patch cannot disagree about what "exactly once" means.
count_lit() {
    python3 -c 'import sys,pathlib; print(pathlib.Path(sys.argv[1]).read_text().count(sys.argv[2]))' "$1" "$2"
}

# `mine` has no referent without a session id, and an unattributable marker is
# worse than none: it tells a reader a mutation is live and gives them nobody to
# ask. Same refusal shape as fmt-mine.sh.
SID="${CLAUDE_CODE_SESSION_ID:-}"
if [ -z "$SID" ]; then
    echo "mutation-probe: CLAUDE_CODE_SESSION_ID is not set, so the marker would name no author." >&2
    echo "  Outside a Claude session, mutate a scratch clone by hand instead." >&2
    exit 2
fi

MARKER_DIR="$ROOT/.codescout/mutations"
MARKER="$MARKER_DIR/$SID.json"
mkdir -p "$MARKER_DIR"

if [ "$SHARED" -eq 1 ]; then
    TREE="$ROOT"; MODE="shared"
else
    TREE="${ROOT}.worktrees/mutation-$SID"; MODE="isolated"
fi
TARGET="$TREE/$FILE"
BACKUP=$(mktemp)

# Revert and unmark BEFORE this process exits — not after the test command does.
# `cargo test` returning is the same event as the shared build lock freeing, so a
# queued peer acquires it at exactly that instant. A revert placed after the test
# is aimed at the window rather than away from it.
cleanup() {
    [ -f "$BACKUP" ] && [ -f "$TARGET" ] && cp "$BACKUP" "$TARGET"
    rm -f "$BACKUP" "$MARKER"
}
trap cleanup EXIT INT TERM

[ -f "$ROOT/$FILE" ] || { echo "mutation-probe: no such file: $ROOT/$FILE" >&2; exit 2; }

# Cheap gate first, against the file you can see, BEFORE paying for a worktree.
# Found by using this script: an over-broad `--find` was refused only after 87 s
# of worktree creation had already been spent. The authoritative check is still
# the one below, against the tree actually mutated — this one exists to fail fast
# on the common mistake.
pre=$(count_lit "$ROOT/$FILE" "$FIND")
if [ "$pre" != "1" ]; then
    echo "mutation-probe: --find must occur exactly once in $FILE; found $pre." >&2
    echo "  A pattern matching 0 makes a survival meaningless; one matching >1 mutates" >&2
    echo "  more than you named. Widen the literal until it is unique." >&2
    exit 2
fi

if [ "$MODE" = "isolated" ]; then
    if [ ! -d "$TREE" ]; then
        echo "mutation-probe: creating isolated worktree (one-off, ~87 s + 2.8 G on this repo)" >&2
        git -C "$ROOT" worktree add --detach "$TREE" HEAD >&2 || exit 2
    else
        # Re-point an existing probe worktree at current HEAD so the mutation is
        # measured against the tree you are actually working on.
        git -C "$TREE" checkout --detach HEAD >/dev/null 2>&1
        git -C "$TREE" reset --hard "$(git -C "$ROOT" rev-parse HEAD)" >/dev/null 2>&1
    fi

    # A worktree is created at HEAD, so it does NOT carry your uncommitted work —
    # and mutation testing is at its most useful on code you have just written and
    # not yet committed. Without this copy the probe would silently test the
    # COMMITTED version and report a survival for a guard that does not exist
    # there yet, which is the "mutation never applied" failure wearing a different
    # hat. Found by using this script, not by reading it.
    cp "$ROOT/$FILE" "$TARGET"

    # Only the file under test is carried across. If your uncommitted work spans
    # several files the worktree build may not match the one in your head, so say
    # so rather than let a confusing result be read as a finding.
    others=$(git -C "$ROOT" status --porcelain -- '*.rs' | awk '{print $2}' | grep -v -F -x -- "$FILE" | wc -l)
    if [ "$others" -gt 0 ]; then
        echo "mutation-probe: NOTE — $others other .rs file(s) are dirty in the shared tree and" >&2
        echo "  are NOT carried into the isolated worktree, which builds them at HEAD." >&2
    fi
fi

[ -f "$TARGET" ] || { echo "mutation-probe: no such file: $TARGET" >&2; exit 2; }
cp "$TARGET" "$BACKUP"

# A mutation that never applied is indistinguishable from one that survived: the
# suite compiles unmutated source and reports a clean pass, which is exactly what
# "this guard is untested" looks like. A bare replace would no-op silently, so
# the count is asserted rather than assumed.
occurrences=$(count_lit "$TARGET" "$FIND")
if [ "$occurrences" != "1" ]; then
    echo "mutation-probe: --find must occur exactly once in the target; found $occurrences." >&2
    exit 2
fi

# Written at the moment of arming, in the same invocation. Passive by content:
# it states what is true and asks for nothing.
cat > "$MARKER" <<JSON
{
  "session_id": "$SID",
  "mode": "$MODE",
  "tree": "$TREE",
  "file": "$FILE",
  "armed_at": "$(date -Is)",
  "pid": $$,
  "note": "A deliberate mutation is live in the tree named above. If a red names this file, this is a likely cause. This is a label, not a request: your build and its log are wanted, not stood down."
}
JSON

python3 - "$TARGET" "$FIND" "$REPL" <<'PY' || exit 2
import sys, pathlib
p = pathlib.Path(sys.argv[1]); t = p.read_text()
assert t.count(sys.argv[2]) == 1, "pattern count changed between check and patch"
p.write_text(t.replace(sys.argv[2], sys.argv[3]))
PY

after=$(count_lit "$TARGET" "$FIND")
[ "$after" = "0" ] || { echo "mutation-probe: patch did not apply (still $after)" >&2; exit 2; }

echo "mutation-probe: ARMED  mode=$MODE  file=$FILE  tree=$TREE" >&2
( cd "$TREE" && "$@" )
rc=$?

# Revert here, explicitly, rather than leaving it to the EXIT trap: the trap is
# the fallback for a signal, not the plan.
cp "$BACKUP" "$TARGET"; rm -f "$MARKER"
restored=$(count_lit "$TARGET" "$FIND")
[ "$restored" = "1" ] || echo "mutation-probe: WARNING — revert left $restored occurrences, expected 1" >&2

if [ "$rc" -eq 0 ]; then
    echo "mutation-probe: SURVIVED (rc=0) — the mutation applied and no test caught it." >&2
    echo "  Two readings, and they take opposite repairs: the line is UNTESTED (write the test)," >&2
    echo "  or it is UNREACHABLE by any test you could write (the code needs a seam first)." >&2
else
    echo "mutation-probe: KILLED (rc=$rc) — a test caught it. Applied-ness needs no further audit:" >&2
    echo "  a red cannot be produced by a mutation that never applied." >&2
fi
exit "$rc"
