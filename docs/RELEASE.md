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

# 1b. Drop the unreleased-cohort callouts from the manual, in the SAME commit as
#     the version bump — otherwise the published docs tell readers a feature they
#     can now install is unavailable. The callout is byte-identical across every
#     page carrying it, so removal is mechanical, not a per-page judgement:
#       grep -rl 'Unreleased — on the `experiments` branch only' docs/manual/src/
#     Delete the five-line blockquote from each hit. Trigger is the RELEASE, not
#     the merge to master: the callout claims "not in vX.Y.Z and not on crates.io",
#     and both stay true on master until a version is actually published.

# 2. Build release binary, then run the gate
#    The gate is defined ONCE — CLAUDE.md § Development Commands — as four commands
#    in an order that is load-bearing. Do NOT restate it here. This block used to
#    carry its own copy, and that copy had drifted to three commands (no `cargo fmt`,
#    no lean lane) while still reading as a complete list, which is how a reader runs
#    part of the gate and believes they ran the gate.
cargo build --release

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
#    Changed tool-facing OUTPUT? Call the tool live on this repo and read the bytes
#    first — see "Before cherry-pick: read the live output" below. A green suite does
#    not establish that what the tool SAYS is useful.
git add <files> && git commit -m "..."

# 2. Cherry-pick to master and push
git checkout master
git cherry-pick <commit-sha>
git push

# 3. Rebase experiments back on master (drops the cherry-picked commit automatically)
git checkout experiments
git rebase master

# 4. Archive any terminal bug files (NO SHA reconciliation — see below)
#    There is no longer a SHA swap owed here. Bug files record the fix SHA *and*
#    its patch-id at fix time; the patch-id survives the rebase that orphans the
#    SHA, so nothing has to come back and re-cite. See "Citing a fix: SHA +
#    patch-id" below.
#    Archiving itself is independent of shipping — a bug file is archived as soon
#    as its fix is verified on experiments (see get_guide("tracker-conventions")).
#    This step is just the convenient moment to sweep what is still sitting in
#    docs/issues/:
#    - if the file's status is fixed/mitigated/wontfix,
#      archive it now via the catalog (NOT git mv — id = sha256(abs_path)):
#        mcp call codescout artifact '{"action":"move","id":"<id>",
#          "new_rel_path":"docs/issues/archive/<date>-<slug>.md"}'
#    Skip files still `open` / `investigating` — they stay in docs/issues/ regardless.
#    Commit separately: docs: archive verified bug files for <date>

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
clean — git detects the cherry-pick and skips the duplicate commit automatically.

Step 4 used to be the archive step, gated on `master`. It is not any more: a bug file is
archived once its fix is **verified on `experiments`** (`_TEMPLATE.md` / `get_guide(
"tracker-conventions")`), because `experiments` is never deleted and holding files back
only grew a pile of `fixed`-but-unarchived bugs that no query surfaced anyway —
`doc(action="find", kind="bug", status="open")` filters on `status`, not on path.

What step 4 owes instead is the **SHA swap**, and it is the one piece the earlier gate was
silently providing: while archiving waited for `master`, every archived file necessarily
carried a master SHA. Archiving earlier means files land in `archive/` holding an
`experiments` SHA that the very next `git rebase master` orphans (see *After cherry-pick*
below). Nothing re-reads `archive/`, so this step has to look there explicitly — hence the
`grep -rl` across both directories rather than only `docs/issues/`.
Step 5 is the drift-detection step — `audit_doc_refs` is the canonical lint for stale
path / link / line references across all markdown surfaces.

**Run the audit against a fresh clone before believing a local green.** The gate asks
"does this path exist", and your working tree has things a clean checkout does not:
worktrees, untracked runtime state (`.codescout/private-memories/`, `.claude/worktrees/`),
`.git/worktrees/`, build output. A ref naming any of those *resolves* locally and so
produces no finding at all — the local run cannot see the class, at any severity, which is
worse than reporting it below the gate. Measured 2026-08-06: six consecutive local runs
exited 0 while CI reported four `high` findings.

```bash
git clone --quiet --no-hardlinks --depth 1 "file://$PWD" /tmp/ci-proxy
./target/release/codescout audit-doc-refs --no-emit-tracker --fail-on high \
  --json --project /tmp/ci-proxy
```

One command and ~30 seconds, versus a ~12-minute CI round-trip per attempt. Two things to
check in the output, not just the exit code: `n_refs_found` should be in the tens of
thousands (a tiny number means the scan did not find the corpus), and the `med` finding
count should stay large — if broken-ref *reporting* falls along with the `high` count, that
is an extractor regression wearing the costume of a docs improvement.

### Before cherry-pick: read the live output of any tool-facing change (required)

`cargo rb` + `/mcp` establishes that the new code is *running* **in this session**. It does
not establish that what the code *says* is useful — and it does nothing for any other
session. Every Claude Code window already holding a codescout server keeps executing the
binary it started from, serving that build's guides, prompt surfaces and guide routing,
until it is reconnected. Measured 2026-08-21: **21 of 26 servers on this machine were
running a deleted binary, the oldest 17 days old.** Nothing raises — the commits are on the
branch, the binary on disk is current, the suite is green, and only `/proc` disagrees.
Count it with [`scripts/stale-servers.sh`](../scripts/stale-servers.sh); the reconnect is
not hygiene, it is the difference between one session and all of them. For any change to tool-facing **output** — warnings, hints,
completeness notes, summaries, rendered text — invoke the tool once against this repository
and read the bytes before the cherry-pick.

```bash
cargo rb          # then /mcp to reconnect
# then call the changed tool on the REAL tree, not a fixture, and read what it emits:
#   grep(pattern="…", glob="…")   → is the warning it prints actually actionable?
```

Two datapoints, both escapes past a fully green gate:

- The `grep` hidden-skip warning shipped with 7 tests, clippy clean, 3522 green, CI 15/15 on
  attempt 1, bug archived — and its output was useless. Every fixture had exactly one hidden
  entry, so the truncation branch never ran; the real tree has 16, and alphabetical ordering
  cut the single entry the feature existed to surface (`cdfbbe0f` fixed it).
- Round 6's `src/librarian/tools/create.rs` fix was proved inert in a live MCP session despite a green suite —
  the same lesson one layer lower.

Unlike the mutation run below this is **required, not advisory**: it costs one tool call, and
no cheaper check covers the class. The test-side corollary lives in memory
`test-design-discipline` — when a change adds a cap, truncation, aggregation, or ordering,
the fixture must **exceed** the cap, or the branch under test never executes.

### Before cherry-pick: mutation-test the diff (recommended for load-bearing logic)

Static tests + clippy prove code compiles and passes the assertions you wrote — they say
nothing about whether those assertions actually *pin* the behavior. The entry-graph Stage 2
review found three defects a green suite hid: a guard test that still passed with the guard
deleted, two untested resolution branches, and a read/write asymmetry — all invisible to
`cargo test`. Mutation testing is the mechanical catch: it mutates the changed code and
reports mutants no test kills (a surviving mutant = behavior no test discriminates).

Run it scoped to the diff being shipped, before the cherry-pick, whenever the diff touches
load-bearing logic (migrations, catalog schema, resolvers, parity/contract code):

```bash
# One-time: cargo install cargo-mutants
git diff master...experiments > /tmp/ship.diff
cargo mutants --in-diff /tmp/ship.diff --package codescout
# Each surviving mutant is a changed line whose mutation left every test green:
# add a discriminating test, or confirm the mutant is equivalent / unreachable.
```

Advisory, not a hard gate — `cargo-mutants` is not yet a workspace dev-dependency and mutation
runs are minutes-scale. Skip for docs-only or trivial mechanical diffs. Rationale + the
per-defect analysis: tracker `test-escape-hardening` (intervention I-3) and memory
`test-design-discipline`.

### Citing a fix: SHA + patch-id (the SHA alone does not survive)

A SHA is **positional**: it names a commit's place in a branch's history. After `git rebase master`, the experiments-side originals of cherry-picked commits become orphans (rebase detects the cherry-picks and drops them), and `git branch --contains <orphan-sha>` returns empty for every fix even though every fix shipped. Measured 2026-08-19: 10 of 63 archived bug files had already lost their fix pointer this way — objects absent from the object DB, not merely unreferenced.

So record **both** identifiers at fix time:

```bash
sha=$(git rev-parse HEAD)
git show "$sha" | git patch-id --stable    # content hash of the diff
```

The patch-id survives rebase **and** cherry-pick, because it hashes the change rather than its position. Measured across 3594 commits: zero genuine collisions, and all 104 duplicate patch-ids were the same change appearing on two branches — the anchor working, not failing.

**There is no promotion path to check and nothing owed later.** Both paths stay available; recording the pair once replaces the old "capture the master SHA afterwards" follow-up.

**A merge commit has no patch-id.** `git show <merge>` emits no diff (a merge has several parents, so "the diff" is ambiguous), and `git patch-id` given no patch prints nothing and exits **0** — empty output, no error, exactly where the durable half is most needed. Cite the merged branch's **constituent commits** by SHA + patch-id instead; each has an ordinary one. Never record an empty patch-id, never substitute the SHA, and do not manufacture one from `git diff <first-parent>..<merge>` — it hashes, but not to the object a cherry-pick would reproduce.

Verified 2026-08-30 across the whole corpus: 19 merge commits in the repo, **none** cited as a fix in `docs/issues/`; 163 distinct cited patch-id values, **none** resolving under `git cat-file -t` (a real patch-id must not be an object, so a SHA in that field would). The rule below codifies existing practice rather than correcting a mess.

If a cited SHA has already been orphaned, recover the commit by its patch-id. Use
redirects, not pipes — Iron Law 3 blocks an unbounded `git log -p` piped to a trimmer:

```bash
git log --all -p > /tmp/all.patch
git patch-id --stable < /tmp/all.patch > /tmp/patch-ids.txt
grep <first-12-of-patch-id> /tmp/patch-ids.txt
```

Each hit is `<patch-id> <commit>`. Several hits mean the change exists on several branches
and any of them is the fix. `git log master --oneline --grep="<subject prefix>"` is a
weaker fallback — measured 2026-08-19, subject-keyword probes returned between 2 and 153
candidates, which is a search rather than a lookup.

This applies to **every SHA-citing surface** — tracker entries (F-N / W-N / U-N / H-N / R-N), `doc(action="event_create")` `anchor_commit` / `also_mutates`, `docs/issues/<bug>.md` Fix sections, ADRs. The concise rule + the cross-repo `<repo>:<sha>` prefix convention live in memory `gotchas` (Cherry-Pick SHA Discipline, Cross-Repo Commit References).

## Large-Cohort Promotion (Fast-Forward)

The Standard Ship Sequence assumes one commit, or a handful. It does not scale to
a cohort — `experiments` has reached 387 commits ahead of `master` at least once
(2026-08-06) — and cherry-picking that commit-by-commit is both impractical and
wrong: it mints a new SHA for every commit, orphaning every SHA citation across
`docs/issues/`, the trackers, and the ADRs, and it discards the one property that
makes the promotion safe.

Check for that property first:

```bash
git rev-list --left-right --count master...experiments
# "0<TAB>387" — master has NO commits of its own, so it is a strict ancestor and
# the promotion is a fast-forward. Any NON-ZERO left number means master has
# diverged: stop and reconcile before going further.
```

A fast-forward moves `master` onto the exact commits already tested on
`experiments`. No new SHAs, so **step 4 of the Standard Ship Sequence is moot** —
nothing to reconcile, and no rebase owed afterwards.

### Sequence

```bash
# 1. Confirm ancestry (above). Non-zero on the left → stop.

# 2. Working tree must be clean. An uncommitted file is not part of the cohort,
#    and if `cargo fmt --check` fails on it, it fails CI right after the merge.
git status --short

# 3. The gate, run from experiments. Defined ONCE in CLAUDE.md § Development
#    Commands — run it from there, and do NOT restate it here.
#    One deliberate local substitution: `cargo fmt --check` for `cargo fmt`. This is
#    a verification pass on a tree that step 2 just confirmed clean, so formatting
#    must REPORT rather than rewrite — a silent reformat here would add an
#    uncommitted file to a cohort you have already declared complete.
#    What this block used to hold is worth recording, because it is why it now holds
#    nothing: it called itself the "Full gate" while running
#    `cargo clippy --all-targets -- -D warnings` — neither the bare form nor the gate
#    form. `--all-targets` selects target KINDS, not packages, and Cargo.toml declares
#    `members = [".", "crates/codescout-embed"]`, so the root is a real package and
#    default selection is the root ALONE. That line therefore never reached
#    crates/codescout-embed or the local-embed-gated `local` module — under a heading
#    that told the reader it was the full gate.

# 4. Documentation gate. A cohort this size is exactly where the human-facing
#    surfaces rot, because nothing fails the build when they do:
#      - CHANGELOG.md [Unreleased] covers the cohort
#      - docs/manual/src/ has a page per new subsystem, wired into SUMMARY.md
#      - docs/manual/src/experimental/index.md does not claim the branch is empty
#      - README.md's feature claims still match the tool surface
#      - each new subsystem page carries the unreleased-cohort callout. These
#        STAY through the merge and come off at release (step 1b of the Release
#        Cycle above) — master is not crates.io.
mcp call codescout librarian '{"action":"audit_doc_refs","emit_tracker":true}'
#    Then re-run it against a fresh clone before trusting the result — see
#    "Run the audit against a fresh clone" under Standard Ship Sequence for why a
#    local green can be blind to a whole class of finding.

# 5. Merge. --ff-only so git refuses rather than silently making a merge commit
#    if ancestry changed between step 1 and here.
git checkout master
git merge --ff-only experiments
git push

# 6. Nothing to rebase — both refs now point at the same commit. Confirm:
git log --oneline -1 master
git log --oneline -1 experiments
```

### Why `--ff-only` rather than a bare `git merge`

A bare `git merge` succeeds either way: it fast-forwards when it can and creates
a merge commit when it cannot. If anyone pushed to `master` between the ancestry
check and the merge, the bare form quietly produces a merge commit containing
integration that was never tested. `--ff-only` fails instead — which is the
answer you want, because it sends you back to step 1.

### Why the documentation gate is a listed step

The 2026-08-06 cohort shipped 62 `feat` commits whose agent-facing surfaces
(`src/prompts/guides/*`) and design records (ADRs) were fully current, while the
mdBook manual had **zero** mentions of any of the ten new subsystems and
`CHANGELOG.md`'s `[Unreleased]` still described the previous cohort. That is not
carelessness — it is the predictable result of gating one and not the other:
`prompt_surfaces_reference_only_real_tools` and
`claude_md_contains_no_deprecated_tool_names` fail the build on drift, and
nothing does the same for the manual. Until a gate exists, this step is the
substitute.
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
- **Never bare-`git push` on a shared checkout — name the refspec.** A concurrent session can change *which branch is checked out* between your commit and your push. A bare `git push` resolves its target at execution time from the current branch, so it then reports `Everything up-to-date` and **exits 0** while your commit sits unpushed on a branch you are no longer on. Push by refspec — `git push origin experiments:experiments` — which needs no checkout and cannot target the wrong branch. Measured 2026-08-07: `git commit` printed `[experiments 8a631282]`, and the very next command ran on `feat/pi-secret-guard`; `git reflog -1` showed `checkout: moving from experiments to feat/pi-secret-guard`, run by neither that session nor any command it issued (F-15).
- **Do NOT `git checkout` back to recover.** Switching the shared working tree back yanks the other session's files out from under it mid-task. Branch refs are shared per-repo, so your commit was never at risk — only its delivery: confirm with `git branch --contains <sha>`, push by refspec, and leave the checkout exactly where you found it.
- **After a foreign checkout is detected, every working-tree write is suspect — including the ones that return `ok`.** A heading- or text-anchored edit whose anchor exists on *both* branches will apply cleanly to the wrong branch's file and report success. Verify landed content from the object store, not the tree: `git show <sha>:<path> | grep -c '<marker>'`. And stage explicit paths, never `git add -A`, while another session has uncommitted work in the same tree.
- **A DIRECTORY is not an explicit path — `git add <dir>` is `-A` scoped to a subtree.** It stages untracked files under it, so a peer's `??` file becomes yours to commit. This is the sweep from the *other* side: the rule above protects your staged work from a peer's broad commit, and this one stops you doing it to them. Name files. Measured 2026-08-30: `git add docs/issues` picked up a peer's 141-line bug file that had never been committed, and `git commit --only docs/issues` then landed it inside a commit whose subject is about patch-id citations — the file's entire git history is now one message about something else (`823c1e6e`).
- **The `git status --short` readback cannot catch this, and it is important to know why.** The rule below requires the add/status/commit chain to run as ONE command, because a separate `git add` leaves a window for a peer's commit to sweep it. That is correct and should not change — but it means the status output is printed *after the commit has already run*. It is a log, not a gate: in the incident above it displayed `A docs/issues/2026-08-30-bench-worktree-…` and changed nothing. **So the protection has to be in the pathspec being safe by construction, not in reading the output.** An instrument that reports faithfully and cannot act is the failure mode this repo keeps meeting; here it is load-bearing by design, and the remedy is upstream of it.
- **Staging explicit paths is not enough — the index is shared too. Commit with a pathspec, in one command.** `git add <paths>` followed by a separate `git commit` leaves a window in which the *other* session's commit sweeps up whatever you staged, because a checkout has exactly one index and their `git commit -a` (or their own broad `git add`) does not distinguish your rows from theirs. Use `git commit -- <paths>` instead: a pathspec on `git commit` implies `--only`, so it commits the working-tree content of exactly those paths and ignores every other staged change, whoever staged it. The work is never lost — it lands, correctly, inside someone else's commit — but the commit message then describes one file out of eight, and every later session that greps history for a fix's provenance is reading a message about something else. Measured 2026-08-18: seven staged doc/tracker files (an archive move, four citation re-points and a new bug file) were committed by a peer session as `62533fee`, whose subject names only their own `guide_ledger.rs` review fixes. **And it does not end there — their next `--amend` dropped all seven back into the working tree**, rewriting `62533fee` to `514da3fd` and leaving the files uncommitted again while the commit that had briefly held them became unreachable. So the failure is not merely a mislabeled commit: after a sweep, your work's committed-ness is whatever the peer's next history edit leaves it, and `git status` is the only authority on that. Re-check it, then commit by pathspec (landed as `f80fbd58`). **Do not try to repair a sweep by rewriting their commit** — `--amend` and `reset` on a commit another session may already be building on trade a wrong message for lost work. Record the provenance in a follow-up commit and move on; a stale SHA in an already-landed message is corrected in the file's text, not by rewriting the message.
- **When a write genuinely cannot wait**, a `git worktree` on your branch is the correct isolation — but note that a worktree session forks librarian artifacts into shadow rows on first write and needs `librarian(action="merge_worktree")` afterwards (memory `worktree-merge-catalog-reconciliation`). For a couple of doc edits, waiting is cheaper than the reconciliation.
