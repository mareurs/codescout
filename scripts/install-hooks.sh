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
        if [ -x "$dest" ] && grep -q "$target" "$dest" 2>/dev/null; then
            echo "ok      $hook_name      shim present"
        else
            echo "MISSING $hook_name      run without --check"
            fail=1
        fi
        return
    fi

    # The shim resolves the repo at RUN time, so it keeps working if the checkout
    # moves — which is the failure the hooksPath bug was made of.
    cat > "$dest" <<'SHIM'
#!/usr/bin/env bash
# Installed by scripts/install-hooks.sh. Thin shim: edit the tracked script, not this.
set -uo pipefail
root="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0
SHIM
    printf 'exec "$root/%s" "$@"\n' "$target" >> "$dest"
    chmod +x "$dest"
    echo "ok      $hook_name      shim installed -> $target"
}

install_shim post-index-change scripts/post-index-change-stage-log.sh

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
    echo "One or more hooks are NOT installed. Nothing above is a substitute for the" >&2
    echo "positive check below." >&2
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
