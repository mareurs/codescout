# Release & Ship Procedures

The full Git workflow for codescout. `CLAUDE.md` carries only Branch Strategy
essentials + a pointer here. SHA-citation and cross-repo-prefix discipline live
in memory `gotchas`.

## Branch Strategy

- **`master` is protected.** Only cherry-picked, thoroughly tested commits land here.
- **All experimental work goes on the `experiments` branch** (or a dedicated feature branch). Iterate freely there.
- **Cherry-pick to `master`** only after: all tests pass, clippy clean, manually verified via MCP (`cargo build --release` + `/mcp` restart).
- **`experiments` is never deleted.** After any merge to `master`, `experiments` continues from the same commit — no recreation, no force-reset.
- **Before any merge or cherry-pick to `master`**, invoke the Docs Lotus Frog (`/buddy:summon frog`) to: (1) audit experimental features eligible for graduation, and (2) identify documentation gaps in the commits being merged.
- Never commit directly to `master` for in-progress or exploratory work.

## Release Cycle

Full release checklist — run from `master`, never from `experiments` or feature branches.

```bash
# 1. Bump version in Cargo.toml
#    Edit version = "X.Y.Z" in Cargo.toml

# 2. Build release binary and verify
cargo build --release
cargo test
cargo clippy -- -D warnings

# 3. Commit the version bump
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to X.Y.Z"

# 4. Tag the release
git tag vX.Y.Z

# 5. Publish to crates.io — WORKSPACE PUBLISH ORDER MATTERS
#    codescout has a path-dep on codescout-embed WITH a version requirement, so
#    crates.io must already host a matching codescout-embed. If crates/codescout-embed
#    changed since its last publish, bump + publish it FIRST (crates.io refuses to
#    re-publish an existing version with changed content), then publish codescout.
#    A non-compatible embed bump (e.g. 0.1.x -> 0.2.0; ^0.1.0 does NOT allow 0.2.0)
#    also requires updating codescout's `codescout-embed = { ..., version = "..." }`.
TOKEN=$(grep CARGO_REGISTRY_TOKEN .env | cut -d= -f2-)   # -f2- keeps '=' in the token
CARGO_REGISTRY_TOKEN=$TOKEN cargo publish -p codescout-embed   # FIRST, only if it changed
CARGO_REGISTRY_TOKEN=$TOKEN cargo publish                      # codescout SECOND

# 6. Push commit + tag
git push
git push --tags

# 7. Create GitHub release with release notes
gh release create vX.Y.Z --title "vX.Y.Z" --notes "release notes here"

# 8. Rebase experiments on the new master
git checkout experiments && git rebase master
```

**Notes:**
- Token is stored in `.env` (gitignored): `CARGO_REGISTRY_TOKEN=...` (use `cut -d= -f2-`, not `-f2` — the token can contain `=`)
- **Workspace publish order:** publish `codescout-embed` before `codescout` whenever the embed crate changed; crates.io cannot re-publish an existing version with new content. Precedent: v0.15.0 bumped embed 0.1.0 -> 0.2.0.
- Use semver: patch for bug fixes, minor for new features, major for breaking changes
- Release notes should list features, dep upgrades, and doc changes
- Always rebase `experiments` after the release push

## Standard Ship Sequence

When a bug fix or tested feature on `experiments` is ready to land in `master`:

```bash
# 1. Commit on experiments (tests passing, clippy clean)
git add <files> && git commit -m "..."

# 2. Cherry-pick to master and push
git checkout master
git cherry-pick <commit-sha>
git push

# 3. Rebase experiments back on master (drops the cherry-picked commit automatically)
git checkout experiments
git rebase master

# 4. Migrate closed bug files whose fix shipped (run AFTER the cherry-pick lands on master)
#    For each <fix-sha> just cherry-picked:
#    - find docs/issues/<date>-<slug>.md whose Fix section cites <fix-sha>
#    - confirm: git branch --contains <fix-sha>  → must show 'master'
#    - if status is `fixed` / `mitigated` / `wontfix`: git mv it to docs/issues/archive/
#    Skip files still `open` / `investigating` — they stay in docs/issues/ regardless.
#    Commit the moves separately: docs: archive bug files for fixes shipped in <date>

# 5. (Optional, recommended after large refactors or batched-bug sessions)
#    Verify doc refs still resolve — bug-file Resume sections cite paths that
#    src refactors may have moved (F-1 friction, multiple datapoints).
#    Run from any active project:
#      mcp call codescout librarian '{"action":"audit_doc_refs","emit_tracker":true}'
#    Inspect findings JSON. Per-finding actions:
#      - verdict=missing, severity=high → real drift; fix the doc OR archive the bug
#      - verdict=ambiguous_basename → doc cites a basename matching multiple files;
#                                      add a path prefix to disambiguate
#      - verdict=resolved_basename → audit auto-resolved by basename match; OK
#                                     (consider adding the prefix anyway for clarity)
#    The audit covers docs/**/*.md by default (which includes docs/issues/).
```

This is the default workflow for all completed work. The rebase step keeps `experiments`
clean — git detects the cherry-pick and skips the duplicate commit automatically. Step 4
keeps `docs/issues/` showing only bugs whose fix is unreleased — see `_TEMPLATE.md` rule
*"Archive moves happen after the fix has shipped to master, not when status flips to fixed."*
Step 5 is the drift-detection step — `audit_doc_refs` is the canonical lint for stale
path / link / line references across all markdown surfaces.

### After cherry-pick: cite the master SHA, not the experiments-side original

When tracking a multi-fix shipping session (running tally in chat, notes in a tracker, F-N entries citing evidence), record the **master-side SHA** assigned by `cherry-pick` — not the original SHA on `experiments`. After the subsequent `git rebase master`, the experiments-side originals become orphans (rebase detects cherry-picks and drops them). `git branch --contains <orphan-sha>` then returns empty for every fix, and the running tally fails the "are they all on master?" audit even though every fix shipped.

```bash
# 2. Cherry-pick — capture the new SHA, do not just use the original
master_sha=$(git rev-parse HEAD)   # immediately after `git cherry-pick`
echo "$master_sha"                  # record this in the tally, not the pre-cherry-pick SHA
```

Or, after the fact: `git log master --oneline --grep="<subject prefix>"` to recover the master SHA by commit message.

This applies to **every SHA-citing surface** — tracker entries (F-N / W-N / U-N / H-N / R-N), `artifact_event` `anchor_commit` / `also_mutates`, `docs/issues/<bug>.md` Fix sections, ADRs. The concise rule + the cross-repo `<repo>:<sha>` prefix convention live in memory `gotchas` (Cherry-Pick SHA Discipline, Cross-Repo Commit References).

## Commit Discipline

- **Batch related changes** into a single well-tested commit rather than committing every incremental step.
- **Only commit when the full fix/feature is working** — all tests pass, clippy clean, manually verified if applicable.
- **Do not push after every commit.** Accumulate local commits during a work session; push once when the work is solid.
- When iterating on a fix, keep working locally until the fix is confirmed, then commit the final state — not every intermediate attempt.

## Chained Git Commands — End With a State-Check

When chaining 4+ git operations with `&&` (e.g. `checkout master && cherry-pick X && push && checkout experiments && rebase && push`), the output stream interleaves all the intermediate results — the final-state confirmation lines (the `..` push outputs) can scroll past mid-output and look like in-progress steps.

**Rule:** end any 4+ step git chain with:

```bash
git rev-parse master experiments origin/master origin/experiments
```

Four identical SHAs prove the ship completed; divergent SHAs catch a silent partial failure (e.g. push rejected, rebase paused on conflict you missed). This is bookkeeping — it converts "scan the output stream for success" into "read four lines at the bottom."

## Concurrent-Work Rules

When working on a shared branch alongside another active agent or session:

- **Never `git reset` to a relative ref** (`HEAD~N`, `HEAD^`, `@{N}`). Relative refs evaluate at execution time, not observation time — the gap between `git log` (read) and `git reset` (write) is enough for another agent to move HEAD, and your reset will silently traverse their commit.
- **Always quote an explicit SHA** for destructive ops. Read `git reflog -N` in the *same command* as the reset; copy the target SHA from the reflog output.
- **Treat your last-observed HEAD as immediately stale.** If any time has elapsed since your last `git log` / `git status`, re-read in the same command as the destructive op.
- **Before any `git rebase`, `git reset`, `git push --force`, or `git commit --amend`** during concurrent work: scout `git reflog -10` first. If unexpected entries appear (commits you didn't author at the tip), pause and reconcile *before* the destructive op.
