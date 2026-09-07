#!/usr/bin/env bash
#
# rustfmt --check against the BYTES BEING COMMITTED, not the working tree.
#
# WHY THIS EXISTS AND WHAT IT REPLACES
# ------------------------------------
# The `cargo-fmt` hook in .pre-commit-config.yaml ran `rustfmt --edition 2021 --check`
# on FILENAMES, so rustfmt read them from disk. That is correct only because the
# pre-commit framework stashes every unstaged change in the checkout first, making the
# working tree equal to the index for the duration of the hook run.
#
# That stash is the defect. It is repo-wide and unconditional, so on a shared checkout
# it reverts every OTHER session's in-flight work for the length of the hook run —
# docs/issues/2026-09-03-pre-commit-stash-window-feeds-peers-wrong-bytes-or-enoent.md,
# six symptoms, `high`, open. This script exists so the stash can be dropped: it is the
# only one of the four commit-stage checks that read the working tree at all. The other
# three already read the index by construction (`git rev-parse ":$path"`,
# `git diff --cached --raw`, `git show :<path>`) and need nothing from the stash.
#
# THE PREDICATE IS `exit != 0 OR stdout non-empty`, AND BOTH HALVES ARE LOAD-BEARING.
# Measured 2026-09-07 on rustfmt 1.9.0-stable, all four cases:
#
#   committed bytes                     exit   stdout
#   well-formatted                       0     empty
#   unformatted                          0     the diff        <- exit code says PASS
#   parse error                          1     empty (stderr)  <- stdout says PASS
#   unformatted + unresolvable `mod`     0     the diff
#
# Reading the exit code ALONE gives a check that cannot fail on the case it exists for:
# `rustfmt --check` on stdin reports formatting differences on stdout and still exits 0.
# Reading stdout alone is blind to a file that does not parse. That is `IC-16`, an
# assertion that cannot fail, and it would have been silent forever — a fmt gate that
# passes every input looks exactly like a tree that is always formatted.
# `the_fmt_check_reds_on_unformatted_committed_bytes` in tests/hook_config.rs pins the
# first row by running this script, not by re-implementing it.
#
# STDIN ALSO DROPS MOD-CHILD RECURSION, WHICH IS A DELIBERATE BEHAVIOUR CHANGE.
# Given a FILENAME rustfmt resolves and formats the `mod` children of that file; given
# stdin it has no file location to resolve them against and formats the buffer alone.
# Measured 2026-09-07 on rustfmt 1.9.0-stable, with a crate root whose child EXISTS on
# disk and is misformatted — the case that actually tests coverage, unlike an absent
# child, which cannot distinguish "no recursion" from "nothing to recurse into":
#
#   A  crate root by path, child misformatted   exit 1, 279 B   recursion found it
#   B  the same bytes on stdin                  exit 0,   0 B   recursion is gone
#   C  control: that child by path              exit 1, 279 B   the child IS detectable
#   D  control: that child on stdin             exit 0, 171 B   THIS script still sees it
#
# C is what makes B's silence a measurement rather than an absence. D is what makes the
# change safe rather than merely intentional: a child that is part of the commit gets its
# own iteration of the loop below and its diff lands on stdout, so it stays covered. The
# only thing that stops being checked is an UNSTAGED misformatted child — a file that is
# not in the commit, which is precisely what this script is not supposed to check. The
# recursion loss and the index read are the same correction, seen twice.
#
# Two consequences worth claiming rather than discovering: the ~1868 ms crate-root case
# (8 of the last 300 `.rs`-touching commits) drops to per-file cost, and the window that
# shrinks is the misattribution window itself. Raised by sessionId
# cda3afe5-17b8-4863-9f4c-9fe4eadbc17b, who measured A/B/C before saying so; D is added
# here because A/B/C establish the loss without establishing that it is harmless.
#
# THE COMMITTED BYTES ARE `:<path>`, WHICH IS CORRECT FOR BOTH COMMIT SHAPES.
# git points GIT_INDEX_FILE at a temporary `next-index-<pid>.lock` for a pathspec commit
# and at the real index otherwise, and `git show :<path>` follows it either way. So this
# checks what the commit will contain — for a pathspec commit that is the working tree
# at those paths, for an index commit it is what was staged. Neither is read from disk
# here, which is the whole point: a peer writing to one of these files mid-hook cannot
# change this hook's answer.
#
# --edition IS NOT OPTIONAL: standalone rustfmt defaults to 2015 and would misparse this
# workspace rather than fail cleanly. It duplicates a manifest value, so
# `the_fmt_script_edition_matches_the_manifest` in tests/hook_config.rs reds if the two
# ever drift. There is no rustfmt.toml in this repo, so style is otherwise identical to
# what `cargo fmt` applies.
set -uo pipefail

EDITION="--edition 2021"

command -v rustfmt >/dev/null 2>&1 || {
    # Degrade open and loudly, the same rule the hook shims apply: a missing toolchain
    # must not block every commit in the checkout, and this check cannot answer its own
    # question either way without rustfmt.
    echo "warning: rustfmt not on PATH — skipping the staged-format check" >&2
    exit 0
}

if git rev-parse --verify -q HEAD >/dev/null 2>&1; then
    against=HEAD
else
    # First commit in a fresh clone: diff against the empty tree rather than a ref that
    # does not exist, so this reports the real file list instead of exiting on an error.
    against="$(git hash-object -t tree /dev/null)"
fi

failed=0
first=1

while IFS= read -r path; do
    case "$path" in
        *.rs) ;;
        *) continue ;;
    esac

    blob="$(git show ":$path" 2>/dev/null)" || continue

    # `printf %s` rather than a here-string: a here-string appends a newline, which would
    # make a file with no trailing newline look correctly terminated to rustfmt and hide a
    # real diff. The command substitution above already strips trailing newlines, so this
    # is the closest faithful reproduction available — and a missing final newline is
    # reported by rustfmt as a diff either way.
    err="$(mktemp)"
    out="$(printf '%s\n' "$blob" | rustfmt $EDITION --check 2>"$err")"
    rc=$?

    if [ $rc -ne 0 ] || [ -n "$out" ]; then
        failed=1
        if [ $first -eq 1 ]; then
            first=0
            echo "REFUSING: the content being committed is not rustfmt-clean." >&2
            echo >&2
        fi
        echo "  $path" >&2
        [ -n "$out" ] && printf '%s\n' "$out" | sed 's/^/    /' >&2
        [ -s "$err" ] && sed 's/^/    /' "$err" >&2
    fi
    rm -f "$err"
done < <(git diff --cached --name-only --diff-filter=ACMR "$against")

[ $failed -eq 0 ] && exit 0

{
    echo
    echo "This checked the bytes in the COMMIT, not the files on disk — so running"
    echo "\`cargo fmt\` is not enough on its own if you commit by pathspec or have"
    echo "unstaged edits. Format, then re-stage (or re-run the same pathspec commit)."
    echo
    echo "\`--no-verify\` also works and is the wrong habit."
    # Forward reach — see scripts/commit-sequence-tail.txt. Single emitted copy shared by
    # every refusing hook; this one refuses commits too, so it owes the same route out.
    _tail="$(dirname "$0")/commit-sequence-tail.txt"
    [ -r "$_tail" ] && { echo; cat "$_tail"; }
} >&2

exit 1
