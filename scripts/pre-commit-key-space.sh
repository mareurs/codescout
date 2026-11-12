#!/usr/bin/env bash
# pre-commit-key-space.sh — refuse a commit that creates a NEW document-key problem.
#
# WHAT IS ASKED. Every markdown document has a key, its lowercase filename stem, and the
# key must be unique among the documents in the key space. `codescout key-gate --staged`
# builds the problem report over the staged index and over HEAD and prints the problems the
# index has that HEAD did not: a collision between a staged file and any other file, a
# staged file whose stem is not a valid key, a scope rule that excludes a folder's
# `readme.md` but not its siblings. This script runs that command and turns its answer into
# a commit decision. The rule itself lives in `.codescout/key-space.toml` (which subtrees
# have no key) and in src/librarian/identity/.
#
# SCOPE, narrower than the invariant on purpose — the reasoning `pre-commit-run.sh` already
# applies to the ledger check and `pre-commit-dead-artifact-ids.sh` to the citation check.
# Only the committer whose own commit ADDS a problem is refused. A file already in a
# collision can be edited freely, and a collision between two files this commit does not
# touch blocks nobody: that state cannot be committed by anyone it would falsify, and a hook
# that fires on every commit to say nothing is how `--no-verify` gets learned. (The migration
# that renames the existing collisions is separate work, and until it lands this repo's own
# collisions are expected to remain.)
#
# WHEN IT RUNS. Only when the commit stages a markdown file (any case of `.md`) or the scope
# file itself. Nothing else can change a key or the scope, so a commit of Rust alone costs
# nothing and prints nothing. Deletions and both sides of a rename count: removing a
# folder's `readme.md` can change what a scope pattern covers.
#
# WHAT IT READS. The INDEX, through the binary: `key-gate` lists `git ls-files` for the
# staged side and `git ls-tree HEAD` for the other, and reads the scope file from each tree
# with `git show`, never from the working tree. So unlike the citation check this one is
# exact under a pathspec or partial commit, and needs no staged-versus-worktree comparison.
# GIT_INDEX_FILE is inherited on purpose and neither set nor cleared: git points it at the
# index the commit will use (a `next-index-*.lock` for a pathspec commit), and that is the
# index to judge.
#
# READ-ONLY AND NO COMPILE. It writes nothing but a private temp file, and it never builds:
# anything that compiles takes the shared `target/` lock and serialises every concurrent
# session's commits. It uses a binary that already exists.
#
# DEGRADE OPEN AND LOUDLY, the rule the rest of this hook applies: a guard whose absence
# blocks every commit is worse than the hole it closes, and without a binary it cannot ask
# its question either way. Each case below passes the commit and says so on stderr:
#   - no live binary (probed in order: $CODESCOUT_BIN, target/release/codescout in this
#     checkout, `codescout` on PATH). Live means `key-gate --help` runs: a stale build that
#     predates the command passes a bare `--help` and then fails the real run, so the probe
#     asks the command itself.
#   - key-gate exits 2: it could not ask (not a git repository, a malformed scope file, a
#     failing git). Its stdout carries `NOT CHECKED — <reason>`, which is printed.
#   - key-gate exits anything else (a panic, a missing loader): not a verdict on the commit.
# Only exit 1 refuses. Exit 0 is silent.
set -uo pipefail

root="$(git rev-parse --show-toplevel 2>/dev/null)" || {
    echo "key-space: NOT CHECKED — \`git rev-parse --show-toplevel\` failed, so the repository could not be located." >&2
    echo "  Passing open: a failing git is not grounds to refuse your commit." >&2
    exit 0
}
cd "$root" || exit 0

list="$(mktemp)" || exit 0
errf="$(mktemp)" || exit 0
trap 'rm -f "$list" "$errf"' EXIT

# Is a markdown file or the scope file part of this commit? `-z` and a `case` on the name
# rather than a pathspec: a git pathspec is case-sensitive and `README.MD` is a document.
# `--no-renames` lists the old path of a rename as well as the new one. The list goes through a
# file, not a process substitution: a failing git must be seen, and a null-separated list cannot
# be held in a shell variable.
if ! git diff --cached --name-only --no-renames -z >"$list" 2>"$errf"; then
    echo "key-space: NOT CHECKED — \`git diff --cached\` failed, so the staged files are not known." >&2
    [ ! -s "$errf" ] || sed 's/^/  stderr: /' "$errf" >&2
    echo "  Passing open: a failing git is not grounds to refuse your commit." >&2
    exit 0
fi
relevant=0
while IFS= read -r -d '' p; do
    case "$p" in
        .codescout/key-space.toml | *.[mM][dD]) relevant=1; break ;;
    esac
done <"$list"
[ "$relevant" -eq 1 ] || exit 0

bin=""
for cand in "${CODESCOUT_BIN:-}" "$root/target/release/codescout" "$(command -v codescout 2>/dev/null || true)"; do
    [ -n "$cand" ] && [ -x "$cand" ] && "$cand" key-gate --help >/dev/null 2>&1 && { bin="$cand"; break; }
done
# A named binary that is not live is the operator's mistake, so say so; the next candidate is
# still used.
if [ -n "$bin" ] && [ -n "${CODESCOUT_BIN:-}" ] && [ "$bin" != "$CODESCOUT_BIN" ]; then
    echo "key-space: note — \$CODESCOUT_BIN ($CODESCOUT_BIN) is not a live codescout with a \`key-gate\` command; using $bin." >&2
fi
if [ -z "$bin" ]; then
    echo "key-space: NOT CHECKED — no live codescout binary with a \`key-gate\` command." >&2
    echo "  Looked at \$CODESCOUT_BIN, target/release/codescout, and codescout on PATH." >&2
    echo "  Passing open: this check cannot ask its question, which is not a reason to" >&2
    echo "  refuse your commit. Build one (\`cargo rb\`) to re-arm it." >&2
    exit 0
fi

: > "$errf"

out="$("$bin" key-gate --staged --project "$root" 2>"$errf")"
rc=$?

case "$rc" in
    0)
        # Clean. key-gate's stderr carries only notes (a path that is not UTF-8 and so was
        # not checked); show them rather than swallow a statement about the scope.
        [ ! -s "$errf" ] || cat "$errf" >&2
        exit 0
        ;;
    1)
        echo "key-space: REFUSED — this commit creates a new document-key problem:" >&2
        printf '%s\n' "$out" >&2
        [ ! -s "$errf" ] || cat "$errf" >&2
        {
            echo ""
            echo "  A document's key is its lowercase filename stem, and it must be unique among"
            echo "  the documents in the key space. Remedies, by cause:"
            echo "    two files share a stem   -> rename one of them (\`docs/a/notes.md\` and"
            echo "       \`docs/b/notes.md\` collide). A \`readme.md\` is keyed by its folder's name,"
            echo "       so rename the folder."
            echo "    a stem is not a key      -> rename the file to lowercase letters, digits and"
            echo "       single hyphens (no spaces, underscores or dots)."
            echo "    the files are fixtures, evidence or another project's notes and need no key"
            echo "       -> exclude the WHOLE subtree in .codescout/key-space.toml. Exclusion also"
            echo "       removes the documents' identity, so exclude only material that has none."
            echo "    a partial folder exclusion -> exclude the whole folder, not just its readme.md."
            echo "  Run \`codescout key-gate\` (without --staged) to see every problem and the scope."
        } >&2
        exit 1
        ;;
    2)
        # `NOT CHECKED — <reason>` is on stdout, one line. An argument-parser usage error exits 2
        # too, and writes only to stderr, so print that as well.
        if [ -n "$out" ]; then
            printf 'key-space: %s\n' "$out" >&2
        else
            echo "key-space: NOT CHECKED — \`codescout key-gate\` exited 2 and printed no reason." >&2
        fi
        [ ! -s "$errf" ] || sed 's/^/  stderr: /' "$errf" >&2
        echo "  Passing open: the key-gate could not ask its question, which is not a reason to" >&2
        echo "  refuse your commit." >&2
        exit 0
        ;;
    *)
        echo "key-space: NOT CHECKED — \`codescout key-gate\` exited $rc, which is not a verdict." >&2
        [ -z "$out" ] || printf '  stdout: %s\n' "$out" >&2
        [ ! -s "$errf" ] || sed 's/^/  stderr: /' "$errf" >&2
        echo "  Passing open: a crash is not grounds to refuse your commit." >&2
        exit 0
        ;;
esac
