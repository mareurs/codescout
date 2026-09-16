#!/usr/bin/env bash
# mutation-probe.sh — run a mutation where no peer can read it, and leave a record either way.
#
# WHY THIS EXISTS
#
# Mutation testing requires putting a KNOWN-BAD version of the code into a tree,
# running the suite, and reading the failure as evidence. On a shared checkout
# that failure is published to every other session's `cargo test`, where it is
# byte-identical to a real regression. Measured instances and the full account:
# `docs/issues/archive/2026-09-08-an-armed-mutation-is-a-deliberate-red-no-observer-can-distinguish.md`.
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
  --strict             exit 3 when the run proved nothing (INCONCLUSIVE), instead
                       of passing the test command's status through. Off by
                       default: SURVIVED and KILLED keep their exit codes either
                       way, and only the no-verdict case is remapped.
                       WHY OPT-IN, since the reason is otherwise only inferable
                       from the exit block at the foot of this file: cases 4, 5
                       and 6 in tests/mutation-probe.sh run `-- true`, which emits
                       no count line and is therefore INCONCLUSIVE — so a --strict
                       that defaulted on would red three fixtures that are testing
                       RESTORATION and ISOLATION rather than verdicts.
                       AND THE COST OF THAT, stated because it is real and was
                       raised by sessionId aa272bed-7d33-4e5e-bcbf-2ccf3b4c4c66:
                       an opt-in flag is a policy someone has to remember, and the
                       caller who most needs it is the one who does not yet know
                       their test is uncommitted — precisely the reader CLAUDE.md
                       § Observer Blindness position 3 says a flag cannot reach.
                       It is accepted rather than unnoticed: the default path's
                       own protection is what those three fixtures assert, and
                       trading their coverage for a reminder is the worse deal.
                       Pass --strict in any wrapper that branches on `$?`.
  --                   everything after this is the test command

  mutation-probe.sh --file src/a.rs --find 'st.defer();' --replace '' \
      -- cargo test --lib agent::build_check
USAGE
    exit 2
}

FILE=""; FIND=""; REPL=""; SHARED=0; STRICT=0
while [ $# -gt 0 ]; do
    case "$1" in
        --file) FILE="${2:-}"; shift 2 ;;
        --find) FIND="${2:-}"; shift 2 ;;
        --replace) REPL="${2:-}"; shift 2 ;;
        --shared) SHARED=1; shift ;;
        --strict) STRICT=1; shift ;;
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
RUNLOG=$(mktemp)
WTPATCH=$(mktemp)

# Revert and unmark BEFORE this process exits — not after the test command does.
# `cargo test` returning is the same event as the shared build lock freeing, so a
# queued peer acquires it at exactly that instant. A revert placed after the test
# is aimed at the window rather than away from it.
cleanup() {
    [ -f "$BACKUP" ] && [ -f "$TARGET" ] && cp "$BACKUP" "$TARGET"
    rm -f "$BACKUP" "$RUNLOG" "$MARKER"
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
    # not yet committed. Without this the probe would silently test the COMMITTED
    # version and report a survival for a guard that does not exist there yet, which
    # is the "mutation never applied" failure wearing a different hat. Found by using
    # this script, not by reading it.
    #
    # THE WHOLE WORKING TREE IS CARRIED, not just --file. Until 2026-09-16 this was a
    # single `cp` of the mutated file, with every other dirty file built at HEAD — correct
    # for a single-file mutation and silently insufficient for anything wider. A slice
    # that alters a type and its call sites is the ordinary shape of real work, and it
    # then did not compile: the run paid a full cold build to end INCONCLUSIVE, having
    # announced the FACT ("N other .rs file(s) are dirty") and never its CONSEQUENCE.
    # BUG docs/issues/2026-09-15-mutation-probe-cannot-verify-a-multi-file-uncommitted-change.md
    #
    # Two mechanisms, because git reports the two populations separately and neither
    # covers the other:
    #   tracked edits, deletions, renames -> one patch, applied
    #   untracked files                   -> copied. A NEW module has no HEAD version to
    #                                        fall back to, so omitting it is a compile
    #                                        error rather than a stale build.
    #
    # ON A SHARED CHECKOUT THIS CARRIES PEERS' IN-FLIGHT WORK TOO, and that is deliberate:
    # it makes the isolated tree match the one your own `cargo test` would compile, so an
    # INCONCLUSIVE here means your real run would have failed too — rather than meaning the
    # probe is lying to you. Isolation of the MUTATION is untouched; nothing is published.
    git -C "$TREE" clean -fdq 2>/dev/null   # no -x: target/ is ignored and must survive

    carried_tracked=0
    if git -C "$ROOT" diff HEAD --binary --no-ext-diff > "$WTPATCH" 2>/dev/null && [ -s "$WTPATCH" ]; then
        if git -C "$TREE" apply --whitespace=nowarn "$WTPATCH" 2>/dev/null; then
            carried_tracked=$(git -C "$ROOT" diff HEAD --name-only | wc -l)
        else
            echo "mutation-probe: REFUSING — your working-tree patch did not apply to the" >&2
            echo "  isolated worktree, which would leave it neither HEAD nor yours. A verdict" >&2
            echo "  from that tree would be about code nobody has. Nothing armed, nothing run." >&2
            exit 2
        fi
    fi

    carried_untracked=0
    while IFS= read -r -d '' f; do
        mkdir -p "$TREE/$(dirname "$f")" && cp "$ROOT/$f" "$TREE/$f" \
            && carried_untracked=$((carried_untracked + 1))
    done < <(git -C "$ROOT" ls-files --others --exclude-standard -z)

    # Belt and braces: --file is carried whatever git thinks its status is.
    cp "$ROOT/$FILE" "$TARGET"

    if [ "$carried_tracked" -gt 0 ] || [ "$carried_untracked" -gt 0 ]; then
        echo "mutation-probe: carried the working tree into the isolated worktree —" >&2
        echo "  $carried_tracked tracked change(s), $carried_untracked untracked file(s)." >&2
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
# Captured as well as streamed, because the verdict below needs to know whether any
# test actually RAN. `tee` merges stderr into stdout for the caller — a visible change,
# and the cheapest way to keep a long `cargo test` streaming rather than appearing all
# at once when it finishes. `PIPESTATUS[0]` is the command's own status; `pipefail`
# would otherwise hand us `tee`'s.
( cd "$TREE" && "$@" ) 2>&1 | tee "$RUNLOG"
rc=${PIPESTATUS[0]}

# Revert here, explicitly, rather than leaving it to the EXIT trap: the trap is
# the fallback for a signal, not the plan.
cp "$BACKUP" "$TARGET"; rm -f "$MARKER"
restored=$(count_lit "$TARGET" "$FIND")
[ "$restored" = "1" ] || echo "mutation-probe: WARNING — revert left $restored occurrences, expected 1" >&2

# A VERDICT REQUIRES THAT TESTS ACTUALLY EXECUTED, and this one predicate closes two
# opposite defects rather than one.
#
#   SURVIVED on zero tests. `cargo test` exits 0 when its filter matches nothing, so a
#   run that executed nothing is byte-identical at the exit code to a mutation no test
#   caught. Measured 2026-09-14: a probe printed `SURVIVED (rc=0)` over `running 0
#   tests` because the caller's test lived in a file the isolated worktree builds at
#   HEAD, and it was not yet committed. The reader is then sent to the verdict's two
#   documented readings — untested, or unreachable — and neither is "your test was
#   absent".
#
#   KILLED on a mutation that never compiled. A malformed `--replace` exits non-zero
#   with no test having run, which this reported as a catch. That is the same error
#   mirrored: a verdict rendered over an absence.
#
# Both are `absence rendered as a value` — the habit that also put `unwrap_or(0)` on an
# absent exit status in `src/tools/run_command/output.rs` and rendered `✓ exit 0` for a
# failed build (fixed at `cc57cd28`). Two instruments, one week, same shape.
#
# THE COUNT IS A PARSE OF SOMEONE ELSE'S STDOUT, and there is no count-free substitute:
# the script takes an arbitrary command, so it cannot know a priori how many tests a
# filter selects, and exit codes cannot stand in for exactly the reason above. So the
# refusal branch is load-bearing rather than defensive — if the format ever changes,
# this must decline to render a finding rather than fall back to one.
ran_lines=$(grep -cE '^running [0-9]+ tests?$' "$RUNLOG" || true)
executed=$(grep -oE '^running [0-9]+ tests?$' "$RUNLOG" | awk '{s+=$2} END {print s+0}')

inconclusive=0
if [ "$ran_lines" -eq 0 ]; then
    inconclusive=1
    echo "mutation-probe: INCONCLUSIVE — no test-count line in the output, so whether any" >&2
    echo "  test ran is unknown and no verdict is available. Three causes, and they differ:" >&2
    echo "  the mutation did not COMPILE; the command was not a test runner; or the runner's" >&2
    echo "  '^running N tests' line has changed shape and this parse needs updating." >&2
    echo "  Read the output above — it says which." >&2
elif [ "$executed" -eq 0 ]; then
    inconclusive=1
    echo "mutation-probe: INCONCLUSIVE — the runner started and selected 0 tests, so nothing" >&2
    echo "  could have caught this mutation. Most often the filter matches no test NAME, or" >&2
    echo "  the filter names a HELPER or a module rather than a test. Name one that exists." >&2
    echo "  Since 2026-09-16 an uncommitted test is NOT a cause here: the isolated worktree" >&2
    echo "  carries your whole working tree, tracked and untracked, not only the mutated file." >&2
elif [ "$rc" -eq 0 ]; then
    echo "mutation-probe: SURVIVED (rc=0, $executed test(s) ran) — no test caught the mutation." >&2
    echo "  Two readings, and they take opposite repairs: the line is UNTESTED (write the test)," >&2
    echo "  or it is UNREACHABLE by any test you could write (the code needs a seam first)." >&2
else
    echo "mutation-probe: KILLED (rc=$rc, $executed test(s) ran) — a test caught it." >&2
    echo "  Applied-ness needs no further audit: a red cannot be produced by a mutation that" >&2
    echo "  never applied, and the count above rules out a compile failure wearing this verdict." >&2
fi

# The exit status stays the TEST COMMAND'S by default, unchanged, and that is a weighed
# trade rather than an inheritance. Three things hold it there: `docs/PROBES.md` pins it, a
# verdict has never been encoded in it — SURVIVED is a real finding and also exits 0 —
# and cases 5, 6 and 14 assert `rc == 0` while running `-- true`, so a distinct exit
# code by default would red fixtures that are testing RESTORATION and ISOLATION rather
# than verdicts.
#
# THE RESIDUAL HAZARD IS NOW LARGER THAN THIS COMMENT ONCE CLAIMED, and `--strict` is
# the answer it named in advance. The version above called the hazard "a caller chaining
# `mutation-probe ... && <next step>`" — a caller who could always have read the verdict
# line instead. Measured 2026-09-14, that was too narrow: through codescout's own
# `run_command`, a `cargo test` wrapped by this script classifies as `type: "test"`, and
# that envelope carried NO stderr field at all. The verdict was not merely easy to skip,
# it did not arrive — and `{"exit_code": 0, "passed": 0}` is the byte-identical rendering
# of a SURVIVED mutant, so the reader got a plausible WRONG verdict rather than a gap.
# Filed as docs/issues/archive/2026-09-14-run-commands-test-envelope-drops-the-stderr-a-wrapper-puts-its-verdict-on.md
# and fixed there; `--strict` is the half that does not depend on which renderer, which
# codescout build, or which harness is on the other end of the pipe. The exit code is the
# only channel that survives every one of them.
#
# So READ THE VERDICT LINE remains the instruction, and `--strict` exists for the caller
# who cannot see it. Credit where it is due: `--strict`'s exit-3 shape was drafted by
# sessionId d52899fd-7490-4408-8deb-1395c3a7f8f6 on 2026-09-14 and talked out of it by
# this author, on the grounds above — which were right about the fixtures and wrong about
# the channel.
#
# SCOPE, deliberately narrow: `--strict` remaps ONLY the two INCONCLUSIVE branches.
# SURVIVED still exits 0 and KILLED still exits the command's status, because those are
# verdicts a caller acts on rather than absences — redefining them would be the
# "redefinition of what `$?` has always meant here" this comment has always refused.
# A test command that itself exits 3 is shadowed under `--strict`; that is the cost of
# the flag and the reason it is opt-in.
if [ "$STRICT" = "1" ] && [ "$inconclusive" = "1" ]; then
    echo "mutation-probe: --strict — exiting 3 rather than $rc, because this run proved" >&2
    echo "  nothing and $rc is indistinguishable from a run that did." >&2
    exit 3
fi
exit "$rc"
