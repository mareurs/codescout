#!/usr/bin/env bash
# pre-commit-dead-artifact-ids.sh — refuse a commit that publishes a citation of an
# artifact id nothing resolves.
#
# WHY THIS EXISTS AT COMMIT TIME AND NOT IN CI
#
# `id = sha256(ABSOLUTE path)`, and the librarian catalog is machine-wide by design —
# an id minted in a sibling repo is a live id. Both properties put this check out of
# CI's reach permanently: a runner checks out ONE repo at a path no author shares, so
# the ids it mints differ from every id the docs cite. Measured 2026-09-14 — a clone of
# this repo at a second path, reindexed, reported 50+ `artifact_missing` findings where
# the authors' own path reported 9, and all 9 of those were cross-repo. The decision and
# its alternatives:
# docs/adrs/2026-09-14-an-id-keyed-on-an-absolute-path-cannot-be-checked-off-the-machine.md
#
# A developer machine is the only observer with the whole namespace. That is what makes
# this placement the design rather than a retreat from CI.
#
# WHAT IT READS, AND THE ONE THING IT REFUSES TO CLAIM
#
# The other checks in this hook read `git show :<path>` — the INDEX — because a pathspec
# or partial commit makes the worktree and the staged bytes disagree, and reading the
# worktree instead is already a filed defect here
# (docs/issues/archive/2026-09-01-cluster-count-gate-lists-the-index-but-reads-the-worktree.md).
#
# This one CANNOT do that, and the reason is the same absolute-path property above:
# materializing staged bytes to a temp path changes every file's own id, so the
# resolver's self-citation carve-out — a tracker quoting its own id in its header, which
# they routinely do — would report every one of them dead. An out-of-tree audit trades a
# real miss for a flood of false alarms.
#
# So it compares the two and only claims where they agree: a file whose staged bytes are
# byte-identical to its worktree copy is audited exactly, and one that diverges is
# REPORTED AND SKIPPED rather than guessed at. Declining to check is not grounds to
# refuse a commit, so a divergent file warns and passes. That is the same rule this
# repo's mutation probe learned the same day: do not render a verdict over bytes you did
# not examine.
#
# SCOPE, narrower than the invariant on purpose — the reasoning `pre-commit-run.sh`
# already applies to the ledger check. Only the committer whose own staged files carry a
# dead citation is refused. An already-broken doc elsewhere blocks nobody, because that
# state cannot be COMMITTED by anyone it would falsify, and a hook that fires on every
# commit to say nothing is how `--no-verify` gets learned.
set -uo pipefail

root="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0
cd "$root" || exit 0

# Markdown only. The audit also scans source comments, but a dead id in a comment is
# capped to `med` by `scan_code_comments` and would never reach the band this refuses —
# including them would cost time to find nothing.
staged="$(git diff --cached --name-only --diff-filter=ACMR -- '*.md' 2>/dev/null)"
[ -n "$staged" ] || exit 0

# DEGRADE OPEN AND LOUDLY, the rule the rest of this hook applies: a guard whose absence
# blocks every commit is worse than the hole it closes, and without a binary it cannot
# ask its question either way. `--version` rather than `-x`, because a stale or broken
# binary is the case a path test passes and the run does not.
bin=""
for cand in "${CODESCOUT_BIN:-}" "$root/target/release/codescout" "$(command -v codescout 2>/dev/null || true)"; do
    [ -n "$cand" ] && [ -x "$cand" ] && "$cand" --help >/dev/null 2>&1 && { bin="$cand"; break; }
done
if [ -z "$bin" ]; then
    echo "dead-artifact-ids: SKIPPED — no runnable codescout binary." >&2
    echo "  Looked at \$CODESCOUT_BIN, target/release/codescout, and codescout on PATH." >&2
    echo "  Passing open: this check cannot ask its question, which is not a reason to" >&2
    echo "  refuse your commit. Build one (\`cargo rb\`) to re-arm it." >&2
    exit 0
fi

# Only files whose staged bytes ARE the worktree bytes can be audited (see the header).
checkable=(); divergent=()
while IFS= read -r p; do
    [ -n "$p" ] || continue
    if [ ! -f "$p" ]; then divergent+=("$p"); continue; fi
    staged_hash="$(git show ":$p" 2>/dev/null | git hash-object --stdin 2>/dev/null)"
    work_hash="$(git hash-object "$p" 2>/dev/null)"
    if [ -n "$staged_hash" ] && [ "$staged_hash" = "$work_hash" ]; then
        checkable+=("$p")
    else
        divergent+=("$p")
    fi
done <<< "$staged"

if [ "${#divergent[@]}" -gt 0 ]; then
    echo "dead-artifact-ids: NOT CHECKED — staged bytes differ from the worktree:" >&2
    printf '  %s\n' "${divergent[@]}" >&2
    echo "  This check audits files on disk, and for these the bytes you are committing" >&2
    echo "  are not the bytes on disk. It declines rather than guessing. Passing open." >&2
fi
[ "${#checkable[@]}" -gt 0 ] || exit 0

args=(); for p in "${checkable[@]}"; do args+=(--paths "$p"); done
report="$(mktemp)"
trap 'rm -f "$report"' EXIT

# `--fail-on never`: the exit code would answer a WIDER question than this check asks.
# The severity band is read below instead, so the audit's own location exemptions
# (archive_drop, issues_drop, code_block) apply for free rather than being re-derived
# here — one implementation of "which citations are exempt", not two that can disagree.
if ! "$bin" audit-doc-refs "${args[@]}" --no-emit-tracker --fail-on never --json \
        --project . > "$report" 2>/dev/null; then
    echo "dead-artifact-ids: SKIPPED — the audit did not complete. Passing open." >&2
    exit 0
fi

# An EMPTY catalog disables the id check upstream (`live_ids_or_disabled`), so a machine
# that has never indexed this repo yields zero findings rather than declaring every
# citation dead. That fail-open is deliberate and lives in the resolver; nothing here
# needs to repeat it.
python3 - "$report" <<'PY'
import json, sys, pathlib
try:
    d = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
except Exception as e:                      # noqa: BLE001 — any parse failure passes open
    print(f"dead-artifact-ids: SKIPPED — unreadable audit report ({e}). Passing open.",
          file=sys.stderr)
    sys.exit(0)

dead = [f for f in d.get("findings", [])
        if f.get("ref_kind") == "artifact_id"
        and f.get("verdict") == "artifact_missing"
        and f.get("severity") == "high"]
if not dead:
    sys.exit(0)

print("dead-artifact-ids: REFUSED — this commit cites artifact ids nothing resolves:",
      file=sys.stderr)
for f in dead:
    print(f"  {f.get('md_file')}:{f.get('md_line')}  {f.get('raw_ref')}", file=sys.stderr)
print("", file=sys.stderr)
print("  An id is sha256 of the artifact's ABSOLUTE PATH, so archiving or moving a file",
      file=sys.stderr)
print("  re-keys it and every citation of the old id silently resolves to nothing.",
      file=sys.stderr)
print("", file=sys.stderr)
print("  TWO REMEDIES, and picking the wrong one destroys the record:", file=sys.stderr)
print("    stale CITATION  -> repoint it. Find the new id with", file=sys.stderr)
print("       doc(action=\"find\", filter={\"rel_path\": {\"contains\": \"<basename>\"}})",
      file=sys.stderr)
print("       or recompute: sha256 of the file's current absolute path, first 16 hex.",
      file=sys.stderr)
print("    deliberate MENTION (the sentence says that id is dead) -> FENCE it.",
      file=sys.stderr)
print("       Inline backticks are not an escape; a fenced block caps to med and passes.",
      file=sys.stderr)
print("", file=sys.stderr)
print("  If the id belongs to a sibling repo and resolves for you, your catalog is stale:",
      file=sys.stderr)
print("  run librarian(action=\"reindex\") and retry.", file=sys.stderr)
sys.exit(1)
PY
