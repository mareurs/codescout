#!/usr/bin/env bash
#
# Install this repo's git hooks. Idempotent; safe to re-run.
#
# WHY THIS SCRIPT EXISTS
# ----------------------
# `.git/hooks/` is not version-controlled, so an installed hook is invisible to
# review, to CI and to every other session. The last time hook wiring on this repo was
# left to a hand-run command it went wrong silently for a day:
# `core.hooksPath` still pointed at a pre-RENAME absolute path, git does not warn or
# fall back when that directory is missing, and ZERO hooks ran —
#   docs/issues/archive/2026-08-30-core-hookspath-points-at-pre-rename-path.md
# This tracked script is the record of what "installed" means, and it checks for that
# exact trap before doing anything.
#
# WHAT GETS INSTALLED, AND WHY BY TWO DIFFERENT ROUTES
# ---------------------------------------------------
#   pre-commit stage        -> the pre-commit.com framework (.pre-commit-config.yaml)
#   prepare-commit-msg      -> a direct shim in .git/hooks/
#   post-index-change       -> a direct shim in .git/hooks/
#
# `post-index-change` CANNOT go through the framework: it is absent from HOOK_TYPES in
# pre-commit 4.6.2's clientlib.py, which lists ten types and not that one.
#
# `prepare-commit-msg` COULD, and deliberately does not. The framework stashes every
# unstaged change in the checkout while its hooks run — not just the committing
# session's — so each installed stage adds a window in which a peer's in-flight work
# transiently reverts to HEAD and `git status` reports it clean. That is an observed
# harm on this checkout, not a projected one; see "The read-side twin" in
#   docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md
# Routing a second stage through the framework would double the number of those
# windows for every session, to buy discoverability that this script already provides.
#
# Usage:
#   scripts/install-hooks.sh                     # index guard only (default)
#   scripts/install-hooks.sh --with-session-id   # ...and the Session-Id trailer
#   scripts/install-hooks.sh --check             # report only, change nothing
#
# PER-CLONE AND PER-MACHINE, AND THAT IS THE DANGEROUS PART.
# `.git/hooks/` is not version-controlled. A fresh clone gets this script and NONE of
# its effects, silently — same shape as the machine-local catalog layers in
# docs/conventions/cross-machine-catalog-resume.md, where nothing fails and you quietly
# get less. The failure mode is a session that believes it is covered. Run
# `scripts/install-hooks.sh --check` after any clone, and do not infer from the
# presence of these scripts that the hooks are live.
#
# THE TRAILER IS OPT-IN, AND THE ASYMMETRY IS THE WHOLE REASON.
# The index guard is REVERSIBLE: uninstall it and nothing it did persists. The
# Session-Id trailer is NOT. Uninstalling removes the hook and leaves every trailer it
# already wrote, in commits that are pushed and permanent. **A default should be set at
# the reversibility of its worst outcome, not at the value of its best** — so someone
# who runs this script without reading it does not end up with stamped commits.
#
# Requested 2026-09-01 by a peer session's operator, who approved the stamp for this
# project while noting it may not suit every project. The capability stays committed and
# one flag away; only the default moved.

set -uo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT" || exit 1

check_only=0
with_session_id=0
for arg in "$@"; do
    case "$arg" in
        --check) check_only=1 ;;
        --with-session-id) with_session_id=1 ;;
        *)
            echo "unknown argument: $arg" >&2
            echo "usage: $0 [--check] [--with-session-id]" >&2
            exit 1
            ;;
    esac
done

fail=0
# Tracked separately from `fail` because STALE and MISSING are not the same state and the
# summary must not collapse them — see the footer at the end of this script.
stale=0

# ---------------------------------------------------------------- the hooksPath trap
hooks_path="$(git config --get core.hooksPath 2>/dev/null)"
if [ -n "$hooks_path" ]; then
    echo "REFUSING: core.hooksPath is set to:" >&2
    echo "    $hooks_path" >&2
    echo >&2
    echo "It overrides .git/hooks/ unconditionally, so anything installed there is" >&2
    echo "dead on arrival — and git does NOT warn when it names a missing directory." >&2
    echo "That failure is silent by design and cost this repo a day already." >&2
    echo >&2
    echo "    git config --unset core.hooksPath" >&2
    echo >&2
    echo "Then re-run this script. Verify with 'git config --get core.hooksPath'" >&2
    echo "returning nothing — an --unset that was recorded but never run is exactly" >&2
    echo "how the original bug survived being marked fixed." >&2
    exit 1
fi

git_dir="$(git rev-parse --git-dir 2>/dev/null)"
if [ -z "$git_dir" ]; then
    echo "REFUSING: not inside a git repository." >&2
    exit 1
fi

# ------------------------------------------------------- the pre-commit.com framework
if command -v pre-commit >/dev/null 2>&1; then
    if [ "$check_only" = "1" ]; then
        if [ -f "$git_dir/hooks/pre-commit" ] &&
            grep -q 'generated by pre-commit' "$git_dir/hooks/pre-commit" 2>/dev/null; then
            echo "ok      pre-commit stage      framework shim present"
        else
            echo "MISSING pre-commit stage      run without --check"
            fail=1
        fi
    else
        pre-commit install >/dev/null || {
            echo "REFUSING: 'pre-commit install' failed." >&2
            exit 1
        }
        echo "ok      pre-commit stage      installed"
    fi
else
    echo "MISSING pre-commit is not on PATH — install it (pipx install pre-commit)" >&2
    fail=1
fi

# ------------------------------------------------------------------- the direct shims
# Render the shim this script would install for <hook_name> -> <target>, into <out>.
#
# EXTRACTED SO `--check` COMPARES AGAINST THE GENERATOR RATHER THAN A PROXY FOR IT.
# `--check` used to ask `grep -q "$target" "$dest"` — "does the shim MENTION the target
# path". Every shim this script has ever written mentions it, including the pre-2026-09-06
# shape with no degrade-open clause, so the predicate was monotone under exactly the drift
# that matters: a checkout wired before that clause existed and never re-installed keeps
# the `exec`-on-missing-target 127 trap, and `--check` calls it `ok`.
#
# Measured 2026-09-06 by sessionId ba061586-6581-4656-b0c5-acad83474de5, who planted a
# pre-clause shim in a throwaway and got `ok  pre-push  shim present` / CHECK_EXIT=0 from
# this script, then `No such file or directory` / PUSH_EXIT=1 from a real
# `git push --dry-run` on the same shim. Present was the wrong predicate for "will not
# brick this checkout" — and this is the one tool whose own header says not to infer
# liveness from the presence of these scripts.
#
# A byte-comparison needs no update when the shim changes again, which a second grep would.
render_shim() {
    _rs_hook="$1"
    _rs_target="$2"
    _rs_out="$3"

    cat > "$_rs_out" <<'SHIM'
#!/usr/bin/env bash
# Installed by scripts/install-hooks.sh. Thin shim: edit the tracked script, not this.
set -uo pipefail
root="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0
# DEGRADE OPEN, AND LOUDLY. `exec` on a missing target exits 127, and git refuses the
# operation on any non-zero hook exit — so a script that is deleted, `git clean`ed, or
# absent on an older branch would break EVERY commit and push in this checkout, with a
# bare "No such file or directory" and nothing naming the hook. A guard whose ABSENCE
# blocks all work is worse than the hole it closes, and it cannot ask its question either
# way; that is the same principle the pre-push guard already applies when it has no
# session id. The warning is what keeps this from being silent degradation, which is the
# failure the rest of this script is written against.
#
# NAMING THE OBSERVER, because "loud" is what makes an open degradation defensible and an
# alarm nothing reaches is exactly as informative as no alarm. There are TWO paths and only
# one depends on someone reading stderr:
#   1. push time  -> this warning, on stderr. Swallowed by any caller that reads only the
#                    exit code -- and with fail-open that exit is 0, so open-and-unheard is
#                    the worst square of the matrix. Real, and not sufficient on its own.
#   2. `install-hooks.sh --check` -> reports `MISSING <hook>` and exits 1, from the
#                    `-x "$PROJECT_ROOT/$target"` precondition, independently of whether
#                    anyone saw path 1. Verified 2026-09-06 by parking the script: --check
#                    exit=1 with `MISSING pre-push`, while the same state pushed with exit 0.
#                    That path has a test caller (tests/pre-push-foreign-session-guard.sh).
# The exposure window also shrank when the script became tracked: `git clean -fdx` removes
# untracked and ignored files, so it can no longer take this one. The realistic trigger is
# checking out a branch that predates the script. Raised by sessionId
# 4a2f34f7-0669-487d-9ce9-39b77881642f, applying "loudness is a property of a PATH" to the
# warning itself rather than to the refusal.
#
# Raised 2026-09-06 by sessionId
# ba061586-6581-4656-b0c5-acad83474de5, who measured the 127 rather than assuming it, and
# who also asked to be cited by sid rather than by session NAME here: a name is
# registry-minted and re-minted by compaction, resume, or a restart under another profile,
# so it decays silently in a comment that does not. Covered by the shim section of
# tests/pre-push-foreign-session-guard.sh; that section exists because the suite's
# 34-assertion aggregate had zero of them on this clause.
SHIM
    printf 'if [ ! -x "$root/%s" ]; then\n' "$_rs_target" >> "$_rs_out"
    printf '    echo "warning: git hook %s is installed, but %s is missing or not executable - skipping" >&2\n' \
        "$_rs_hook" "$_rs_target" >> "$_rs_out"
    printf '    exit 0\n' >> "$_rs_out"
    printf 'fi\n' >> "$_rs_out"
    printf 'exec "$root/%s" "$@"\n' "$_rs_target" >> "$_rs_out"
}

install_shim() {
    hook_name="$1"
    target="$2"
    dest="$git_dir/hooks/$hook_name"

    if [ ! -x "$PROJECT_ROOT/$target" ]; then
        echo "MISSING $hook_name — $target is not executable" >&2
        fail=1
        return
    fi

    # Never clobber a framework-generated shim; that would silently disable whatever
    # stage the framework had wired there.
    if [ -f "$dest" ] && grep -q 'generated by pre-commit' "$dest" 2>/dev/null; then
        echo "REFUSING: $dest is a pre-commit-generated shim." >&2
        echo "Someone ran 'pre-commit install --hook-type $hook_name'. Resolve that" >&2
        echo "first — two owners for one hook file is not a state this script picks." >&2
        fail=1
        return
    fi

    if [ "$check_only" = "1" ]; then
        # MISSING and STALE are separated because their remedies differ and so does their
        # danger: a missing shim runs nothing, while a stale one RUNS, and runs the older
        # shape — which is how a checkout keeps the 127 trap. Collapsing them into one word
        # is how the old predicate managed to be reassuring about the worse case.
        if [ ! -x "$dest" ]; then
            echo "MISSING $hook_name      run without --check"
            fail=1
            return
        fi
        _want="$(mktemp)"
        render_shim "$hook_name" "$target" "$_want"
        if cmp -s "$_want" "$dest"; then
            echo "ok      $hook_name      shim matches generator"
        else
            echo "STALE   $hook_name      shim differs from what this script generates" >&2
            echo "        Installed before a change to the shim and never re-installed." >&2
            echo "        It is NOT inert - it runs, and runs the older shape. Re-run this" >&2
            echo "        script without --check. Diff (installed vs generated):" >&2
            diff -u "$dest" "$_want" >&2 || true
            fail=1
            stale=1
        fi
        rm -f "$_want"
        return
    fi

    render_shim "$hook_name" "$target" "$dest"
    chmod +x "$dest"
    echo "ok      $hook_name      shim installed -> $target"
}

install_shim post-index-change scripts/post-index-change-stage-log.sh

# Refuses a push that would publish another session's commits. Pusher-side complement to
# "a session that cannot publish must not commit to a shared branch" — that rule needs no
# coordination and is the right primary defence, but it is silent on the party who acts.
# Inert for anyone without CLAUDE_CODE_SESSION_ID, so the human release flow is untouched.
# Why: docs/trackers/observer-blindness.md OB-20. Tests: tests/pre-push-foreign-session-guard.sh.
install_shim pre-push scripts/pre-push-foreign-session-guard.sh

# SEED THE STAGE LOG, and only when it does not exist.
#
# At install time the index may already hold staged paths — put there by sessions that
# never ran this hook, because it did not exist yet. The recording hook attributes any
# pair it has no row for to whoever is running it, so the FIRST run after install would
# claim all of that inherited state for the installer.
#
# That is not a cosmetic mis-label. A guard that reads a peer's staged file as yours is
# silent on exactly the capture it exists to refuse, and silence is the failure nobody
# observes. Measured 2026-09-01, on this repo's own first install: the index held
# `docs/trackers/observer-blindness.md`, staged by a peer, and the first hook run took
# it. Repaired by hand then; this is the repair made structural.
#
# Seed with `-` (unknown) rather than with the installer's id. The direction is
# deliberate: `unknown` OVER-refuses until those pairs churn out of the index, which a
# reader recovers from by reading a message; `mine` UNDER-refuses silently, which
# nobody recovers from because nothing is emitted. Prefer the noisy wrong answer when
# the quiet one is unobservable.
#
# Never overwrite an existing log — that would discard real attributions.
seed_log="$git_dir/session-stage-log"
if [ "$check_only" = "1" ]; then
    if [ -e "$seed_log" ]; then
        echo "ok      stage log             present"
    else
        echo "MISSING stage log             run without --check"
    fi
elif [ -e "$seed_log" ]; then
    echo "ok      stage log             present, left alone"
else
    git diff --cached --raw 2>/dev/null |
        awk -F'\t' '{ split($1, a, " "); print "-\t" a[4] "\t" $2 }' > "$seed_log"
    seeded="$(grep -c . "$seed_log" 2>/dev/null || echo 0)"
    echo "ok      stage log             seeded, $seeded inherited pair(s) marked unknown"
fi

if [ "$with_session_id" = "1" ]; then
    install_shim prepare-commit-msg scripts/prepare-commit-msg-session-id.sh
elif [ "$check_only" = "1" ]; then
    # A --check run REPORTS; it must describe what is on disk, not what this
    # invocation's flags would have installed. Reporting "skip" for a hook that is in
    # fact live would be a status tool lying about the status it exists to report.
    if [ -x "$git_dir/hooks/prepare-commit-msg" ]; then
        echo "ok      prepare-commit-msg    shim present (opt-in, installed earlier)"
    else
        echo "off     prepare-commit-msg    opt-in; not installed"
    fi
else
    echo "skip    prepare-commit-msg    opt-in; pass --with-session-id"
fi

echo
if [ "$fail" != "0" ]; then
    # THE FOOTER MUST NOT COLLAPSE STALE INTO MISSING. The per-hook lines above stopped
    # doing that on 2026-09-06; this line kept doing it, which is worse, because it is the
    # one line a skimmer reads AND it errs toward the reassuring reading: "not installed"
    # implies inert and harmless, while the true state of a stale hook is "running the
    # older shape" — the worse case, and the whole reason the words were split. A reader
    # trusting the summary over the detail concluded the opposite of the truth.
    #
    # Found 2026-09-06 by sessionId ba061586-6581-4656-b0c5-acad83474de5, on a run with two
    # stale hooks and zero missing ones, having first ruled out `off prepare-commit-msg` as
    # the cause by checking that the same line was present in a pre-change run that exited 0.
    if [ "$stale" != "0" ]; then
        echo "One or more hooks are STALE: installed, RUNNING, and running an OLDER shape" >&2
        echo "than this script generates. A stale hook is NOT inert — that is why this line" >&2
        echo "does not say 'not installed'. Some hooks above may ALSO be missing; read the" >&2
        echo "per-hook lines rather than this one. Re-run without --check to regenerate." >&2
    else
        echo "One or more hooks are NOT installed. Nothing above is a substitute for the" >&2
        echo "positive check below." >&2
    fi
    exit 1
fi

# A --check run reports; it installs nothing, so it has no install to verify.
[ "$check_only" = "1" ] && exit 0

echo "Installed. Now VERIFY POSITIVELY — a successful install is compatible with the"
echo "hook never running, which is how the last hooks defect stayed invisible:"
echo
if [ "$with_session_id" = "1" ]; then
    cat <<'EOF'
    git commit --allow-empty -m 'hook probe'
    git log -1 --format='%(trailers:key=Session-Id)'   # <- must print your session id
    git reset --soft HEAD~1                            # <- discard the probe

`--soft`, NOT `--hard`. On a shared checkout `git reset --hard` discards every
session's uncommitted work, not just yours — it is the single most destructive
command in this document, and it would be run here in the name of tidying up after
a safety check. The probe commit is empty, so `--soft` moves the branch pointer and
touches nothing in the working tree.
EOF
else
    cat <<'EOF'
    git add <a file you own>
    cat "$(git rev-parse --git-dir)/session-stage-log"  # <- must name your session id

The stage log is the guard's ONLY input, so an empty one after staging means the
guard is inert whatever this script just reported. The Session-Id trailer was NOT
installed; pass --with-session-id if you want it, and read why it is opt-in first.
EOF
fi
cat <<'EOF'

Several sessions share this checkout, and installing these hooks changes every
session's `git commit`. Tell them before you run this, not after — and wait for an
answer, because silence is not consent.
EOF
