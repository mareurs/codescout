#!/usr/bin/env bash
#
# Native pre-commit dispatch — the four commit-stage checks, with NO STASH.
#
# WHAT THIS REPLACES AND WHY
# --------------------------
# .pre-commit-config.yaml runs these same four checks through the pre-commit.com
# framework. The framework stashes EVERY unstaged change in the checkout before running
# any hook and restores it afterwards. That is correct for a single-user clone and is a
# defect here: on a shared checkout it reverts every OTHER session's in-flight work for
# the length of the hook run, several times an hour, and the two lines announcing it
#
#     [INFO] Stashing unstaged files to ~/.cache/pre-commit/patch<ts>-<pid>
#     [INFO] Restored changes from  …
#
# (The real lines carry an absolute path. It is written `~` here because
# `no_tracked_script_hardcodes_a_personal_home_path` scans tracked scripts for
# machine-specific homes and does not exempt comments — correctly, since a hardcoded home
# in a comment is copied into code as readily as one in a command.)
#
# print in the COMMITTER's terminal — the one session for whom nothing is wrong. Six
# symptoms, `high`, open:
# docs/issues/2026-09-03-pre-commit-stash-window-feeds-peers-wrong-bytes-or-enoent.md.
# pre-commit 4.6.2 exposes no way to disable the stash (verified: `pre-commit run --help`
# names it zero times), so not using the framework is the only route to not stashing.
#
# THE MIGRATION IS CHEAP BECAUSE THREE OF THE FOUR CHECKS NEVER NEEDED THE STASH.
# Measured 2026-09-07 by reading each script rather than inferring from its name:
#
#   unreviewed-content   `git rev-parse ":$path"` against two index files      index
#   foreign-index        `git diff --cached --raw`, GIT_INDEX_FILE             index
#   ledger-counts        `git ls-files` + `git show :<path>`                   index
#   cargo-fmt            filenames -> rustfmt reads them from disk             WORKING TREE
#
# Only the fourth was working-tree-shaped, and it is rewritten as
# scripts/pre-commit-cargo-fmt.sh to read `:<path>` like the other three. So the stash
# was buying correctness for exactly one check, and that check is better off without it
# — see that script's header for the crate-root recursion it also drops.
#
# THE PRECEDENT IS ALREADY IN THIS REPO. scripts/install-hooks.sh routes
# `prepare-commit-msg` outside the framework deliberately, and says why: "The framework
# stashes every unstaged change in the checkout while its hooks run — not just the
# committing session's — so each installed stage adds a window in which a peer's
# in-flight work transiently reverts to HEAD." That judgement was made and acted on for
# a hook that fires on some commits. This applies it to the stage that fires on all of
# them.
#
# WHAT IS LOST, STATED RATHER THAN DISCOVERED LATER
# -------------------------------------------------
#   - `pre-commit run --all-files` and `pre-commit autoupdate`. Run the scripts directly.
#   - The framework's `files:` / `types:` filtering, re-implemented below. It is two
#     regexes and they are pinned by tests/hook_config.rs.
#   - The framework's whole-tree diff check (`files_modified = diff_before != diff_after`,
#     commands/run.py:203-206). On a shared checkout it is an unconditional whole-tree
#     diff with no per-hook opt-out that assumes the hook is the only writer, so any file
#     ANY session writes during the run is attributed to the hook. It fired twice on
#     2026-09-01 on a six-session checkout and the second one taught a `--no-verify`.
#     Three authors reached that reading independently and none cites the others:
#     install-hooks.sh's prepare-commit-msg routing, .pre-commit-config.yaml's header, and
#     pre-commit-ledger-counts.py:105, whose `_prime_index` was optimised to shrink that
#     very window.
#
#     BUT IT IS NOT A MISFEATURE, AND CALLING IT ONE WOULD MISLEAD THE NEXT PERSON.
#     That check is the mechanism by which pre-commit supports AUTO-FIXING hooks: black,
#     prettier, `rustfmt --write` rewrite a file, the diff fires, the run fails, and you
#     re-stage the fixed content. It is load-bearing for a class of hook this repo does
#     not have. Raised by sessionId cda3afe5-17b8-4863-9f4c-9fe4eadbc17b.
#
# >>> THE INVARIANT THAT REPLACES IT, AND THE ONE THING TO READ BEFORE ADDING A HOOK <<<
#
#     EVERY COMMIT-STAGE CHECK HERE MUST BE READ-ONLY. Do not add a hook that rewrites,
#     formats, or stages a file.
#
#     Today's four are read-only, verified by reading them rather than by their names:
#     every write-shaped token in the two bash guards is inside a comment or an `echo`;
#     pre-commit-ledger-counts.py's only `open()` is read-mode and every subprocess is
#     `git show` / `git ls-files`; the fmt check is `--check`, which does not write by
#     definition. That is true by OBSERVATION of these four, not by enforcement — and
#     dropping the framework leaves it guarded by nothing.
#
#     The failure mode if a fifth hook ever writes is the worst shape available: the
#     rewrite lands in the working tree, the commit proceeds with the UN-rewritten content
#     staged, the fix is never staged at all, and nobody sees an error. That is why this
#     paragraph is here rather than in the config or a tracker — this file is what the
#     next person adding a hook opens.
#
# STAGE CHOICE IS STILL LOAD-BEARING. Anything that COMPILES takes the shared `target/`
# lock, so a commit-stage cargo build serialises every concurrent session's commits
# (`Blocking waiting for file lock`, observed 2026-08-31 with four sessions active).
# Only checks that do not build may run here. rustfmt parses without compiling and is
# therefore safe. The full gate stays where it is documented and run deliberately:
# CLAUDE.md § Development Commands, four commands in a load-bearing order.
#
# THERE IS STILL NO PUSH STAGE. `cargo clippy` and `cargo test --workspace` ran at
# pre-push from 2026-08-31 to 2026-09-01 and were withdrawn after refusing a push on a
# GREEN test run — the whole-tree diff above, over a 30-80s window. CI runs exactly those
# commands and is authoritative, and a noisy hook teaches `--no-verify`, which also
# disarms the quiet checks that work.
set -uo pipefail

root="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0
cd "$root" || exit 0

if git rev-parse --verify -q HEAD >/dev/null 2>&1; then
    against=HEAD
else
    against="$(git hash-object -t tree /dev/null)"
fi

# The paths this commit will contain. `git diff --cached` follows GIT_INDEX_FILE, which
# git points at a temporary `next-index-<pid>.lock` for a pathspec commit and at the real
# index otherwise — so this is the committed set under both shapes, and it is read from
# an index rather than from disk.
paths="$(git diff --cached --name-only --diff-filter=ACMR "$against" 2>/dev/null)"

status=0

# `run <name> <script> [args…]` — prints the framework's Passed/Failed line shape so a
# reader who has seen the old output can still recognise this one.
run() {
    _name="$1"; shift
    _script="$1"; shift
    if [ ! -x "$root/$_script" ]; then
        # Degrade OPEN and loudly, the same rule scripts/install-hooks.sh applies to its
        # shims: a guard whose ABSENCE blocks every commit in the checkout is worse than
        # the hole it closes, and it cannot ask its question either way. The observer that
        # does not depend on anyone reading stderr is `install-hooks.sh --check`.
        printf '%-72s%s\n' "$_name" "Skipped (missing $_script)" >&2
        return 0
    fi
    if "$root/$_script" "$@"; then
        printf '%-72s%s\n' "$_name" "Passed"
    else
        printf '%-72s%s\n' "$_name" "Failed"
        status=1
    fi
}

# `matches <extended-regex>` — did this commit touch a path the check is scoped to?
matches() {
    [ -n "$paths" ] && printf '%s\n' "$paths" | grep -qE "$1"
}

# always_run — these answer a question about the COMMIT itself, not about any file in it.
run "refuse a pathspec commit carrying unstaged content" \
    scripts/pre-commit-unreviewed-content.sh
run "refuse an index commit carrying another session's staged paths" \
    scripts/pre-commit-foreign-index.sh

# Scoped, and the scope is narrower than the invariant ON PURPOSE. `files:` matched the
# paths in YOUR COMMIT, not the shared index, so an already-broken ledger state does not
# block an arbitrary committer — only one whose own commit touches the ledger or a bug
# file. That is a LATENCY bound, not a correctness hole: the state cannot be committed by
# anyone it would falsify. Widening it would refuse commits that neither caused the break
# nor can repair it, and a hook that fires on every commit to say nothing is how
# `--no-verify` gets learned. Regex mirrors the retired `files:` key byte for byte.
if matches '^(docs/trackers/issue-clusters\.md|docs/issues/.*\.md)$'; then
    run "refuse a stored count, or a class gaining a member it does not name" \
        scripts/pre-commit-ledger-counts.py
else
    printf '%-72s%s\n' \
        "refuse a stored count, or a class gaining a member it does not name" \
        "Skipped (no files to check)"
fi

# Self-gating: the fmt script computes its own `.rs` list from the same index and exits 0
# when that list is empty. Deliberately NOT gated here as well — one derivation of "which
# files are Rust" rather than two that can disagree, which is the drift the config file
# spent a paragraph holding in sync between its Python hook and a Rust test.
run "rustfmt --check (committed bytes)" scripts/pre-commit-cargo-fmt.sh

exit $status
