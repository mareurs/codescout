---
kind: bug
status: fixed
title: 'BUG: link_scan''s cross_repo_file_qualified bucket never fires — the split is keyed on a roots registry most repos are absent from'
tags:
- link_scan
- librarian
- citations
- test-design
closed: 2026-08-28
fix_patch_id: 235b5b833777a2b3a04fe43f515e94b01b58d205
fix_sha: 03618605 (experiments)
opened: 2026-08-27
owner: marius
severity: med
---

## Summary

`link_scan`'s `cross_repo_file_qualified` bucket is **empty in every real repo**, and the
3-part citations it was built to hold fall through to `malformed_qualifier` — the bucket
whose documented meaning is *"redundant same-repo prefix, strip it."* Measured today on two
repos: **0** in the new bucket, and at least 11 findings in `malformed_qualifier` whose outer
segment names a different repo.

The split shipped today with a passing unit test. The test constructs its `Root` by hand with
an absolute path; a real workspace does not, which is the one dimension the split's condition
reads.

## Symptom (Effect)

Both repos, `librarian(action="link_scan", write=false)`, whole project:

| repo | `malformed_qualifier` | `cross_repo_file_qualified` |
|---|---|---|
| codescout | 20 | **0** |
| claude-plugins | 4 | **0** |

codescout's 20 `raw` values, verbatim from the buffer:

```text
claude-plugins:roster-audit-session-log:F-14     <- other repo. Wrong bucket.
claude-plugins:roster-audit-session-log:F-14
claude-plugins:roster-audit-session-log:F-5
claude-plugins:roster-audit-session-log:F-5
claude-plugins:roster-audit-session-log:F-5
claude-plugins:roster-audit-session-log:F-3
claude-plugins:roster-audit-session-log:F-4
claude-plugins:roster-audit-session-log:F-6
claude-plugins:repo-hygiene-session-log:F-2
claude-plugins:repo-hygiene-session-log:F-2
claude-plugins:reconnaissance-patterns:R-4
claude-plugins:reconnaissance-patterns:R-4
eduplanner-ui:calendar-insight-panel-session-log-2026-08-18:W-4
codescout:statement-validity-session-log:F-2     <- self-repo. Correct bucket.
codescout:statement-validity-session-log:F-2
codescout:statement-validity-session-log:F-2
citer:target:F-2                                 <- this bug's own test fixture,
claude-plugins:target:F-2                           quoted in repo markdown
codescout:target:F-2
codescout:foo:F-2
```

Thirteen of the twenty name another repo. Not one reached the bucket built for them.

claude-plugins' 4, all in `## References` sections of archived bug files — the exact usage
the `tracker-conventions` guide calls legitimate prose:

```text
codescout:reconnaissance-patterns:R-119
codescout:statement-validity-session-log:F-3
codescout:prompt-surface-compaction-session-log:F-9
codescout:reconnaissance-patterns:R-104
```

**The cost is a wrong inference, not a wrong number.** Nothing miscounts and no edge is
affected — the 3-part form is retracted either way. But `malformed_qualifier`'s documented
remedy is *strip the prefix*, and applied to these it deletes correct cross-repo references.
The two buckets exist precisely so a reader does not have to check each outer segment by
hand; today they must, and the bucket name argues against doing so.

## Reproduction

1. `librarian(action="link_scan", write=false)` in any repo holding a 3-part citation whose
   outer segment is another repo's name.
2. Read `counts.cross_repo_file_qualified` → `0`.
3. Read `malformed_qualifier[*].raw` → the cross-repo ones are in there.

Ruled out, so it is not the pin: the claude-plugins figures are identical whether the scan
runs via a `workspace=` per-call pin with a different project active, or with claude-plugins
genuinely activated. Same counts, same buckets.

## Environment

- `link_scan` at `d794f47b` + `bbc86582`; `cross_repo_file_qualified` added earlier today.
- Reference: `docs/issues/archive/2026-08-27-cross-repo-file-qualified-citation-unsupported.md`
- Observed against real catalogs for `codescout` (1183 artifacts) and `claude-plugins`.

## Root cause

**Confirmed:** the split's condition is

```rust
if citing_repo_name.is_some_and(|name| name != outer) { /* cross_repo_file_qualified */ }
else { /* malformed_qualifier */ }
```

(`src/librarian/tools/link_scan/mod.rs:503`), and `citing_repo_name` comes from
`containing_root(&root_paths, &row.abs_path)` where
`root_paths = ctx.workspace.roots.iter().map(|r| r.path.clone())`. `is_some_and` returns
false for `None`, so **a `None` here is indistinguishable from a self-reference** and takes
the fallback. Given 0 hits across 1183 artifacts in one repo and 4 of 4 in another,
`citing_repo_name` is `None` for real rows.

**Hypothesis for WHY it is `None`, not yet verified — this is the part to check first.**
`containing_root` compares lexically and demands the root be an absolute prefix of
`abs_path` at a component boundary. A registered workspace root's `path` appears to be
stored **relative** to the project (`workspace(activate)` renders codescout's roots as `.`
and `crates/codescout-embed`, claude-plugins' as `.`, `buddy`,
`session-bridge/mcp-server`). An absolute `abs_path` can never be contained by `.`, so every
row returns `None`.

**Decisive check:** print `root_paths` inside a real scan, or build the test's `Root` from
the workspace config instead of a literal. If the paths come out relative, that is the whole
bug — and it is likely not confined to this call site, since `root_paths` is derived the
same way wherever `ctx.workspace.roots` is mapped to paths.

## Evidence — why the test did not catch it

`a_cross_repo_file_qualified_citation_is_reported_separately_from_a_redundant_same_repo_one`
(`mod.rs:1318`) seeds:

```rust
let root = tmp.path().to_path_buf();          // ABSOLUTE
.with_root(Root { name: "citer".into(), path: root.clone() })
```

so `containing_root` matches, `citing_repo_name` is `Some("citer")`, and both branches are
exercised. The test is not vacuous — it can fail — but its fixture differs from production
in exactly the dimension the condition reads, so it passes over an inert feature. Worth
naming as a class: **a test that hand-builds the state a real run derives cannot tell you
the derivation works.** Same family as the four could-not-fail assertions found during the
section-grain run, arrived at from the opposite side.

## Fix

Two parts, and the first is worth doing regardless of the second.

1. **Stop conflating `None` with self-reference.** `None` means *we could not tell*, which is
   neither "strip it" nor "leave it". Either report it as its own third state, or resolve the
   citing repo from a source that cannot be `None` — the artifact's own `git_root` basename is
   already available on the row and needs no workspace registry.
2. **Fix the containment input** if the hypothesis above holds: resolve each `Root.path`
   against the project root before building `root_paths`. Then audit the other
   `ctx.workspace.roots` → paths mappings for the same defect.

**Regression test:** drive the split through a `Root` built the way a real workspace builds
it — relative sub-root included — and assert a cross-repo 3-part citation lands in
`cross_repo_file_qualified`. Keep the existing absolute-path test; it pins the branch logic,
and the new one pins the derivation.

## Workarounds

Read `malformed_qualifier[*].raw` and split by outer segment by hand. Only an outer segment
equal to the citing repo's own name is safe to strip.

## Closed 2026-08-28 — confirmed cause, refuted hypothesis

Fixed in `03618605` (experiments), patch-id
`235b5b833777a2b3a04fe43f515e94b01b58d205`.

### The "Confirmed" half held; the "Hypothesis" half did not

The mechanism this file confirmed was exactly right: `is_some_and` returns false for
`None`, so **a `None` is indistinguishable from a self-reference** and takes the fallback.

The hypothesis for *why* it was `None` — which the file itself flagged as unverified and
put first in line to check — does not survive contact. It reasoned that a root's `path` is
stored **relative** (`.`, `crates/codescout-embed`), so an absolute `abs_path` can never be
contained by it, and prescribed: resolve each `Root.path` against the project root, then
audit every other `ctx.workspace.roots` → paths mapping for the same defect.

That fix would have been a **no-op**, and the audit a hunt for a defect that does not
exist. The reasoning conflated two different types that share a name:

| | `config::workspace::WorkspaceConfig` | `librarian::workspace::WorkspaceConfig` |
|---|---|---|
| read from | the repo's `.codescout/workspace.toml` | `~/.config/librarian/workspace.toml` |
| the list | `[[project]]` → `ProjectEntry` | `[[roots]]` → `Root` |
| path form | **relative** (`.`, `crates/codescout-embed`) | **absolute** (`/home/…/work/mirela`) |
| who reads it | `workspace(status)`, which is what the `.` in this file came from | `ToolContext.workspace`, which is what `link_scan` reads |

The nine registry roots are all absolute. Nothing on this path is relative.

**The cause is absence, not spelling.** `[[roots]]` is an optional per-machine registry a
repo must be hand-added to, and codescout is not in its own — it appears in that file only
as an **umbrella member**. So `containing_root` returned `None` for all 1183 rows.

The decisive evidence was already sitting in the reproduction, before any code was read:
the self-repo `codescout:statement-validity-session-log:F-2` and the cross-repo
`claude-plugins:…` citations were in the **same** bucket. That only happens when
`citing_repo_name` is `None` — a genuinely-`Some("codescout")` run would have separated
them. Which is the CLAUDE.md rule earning its keep: *run the reproduction before reading
the fix plan, because the plan is a hypothesis about the reproduction.*

### What shipped

Two sources of "our own name", **unioned rather than ranked**, since either match is
positive evidence of a self-reference and the code's stated policy only claims
"genuinely cross-repo" on positive evidence:

- the registry root name, when a root does contain the row (unchanged behaviour);
- the **git-root basename** — the convention `artifact(action="create")`'s `repo` field
  already documents ("workspace root name (git repo basename)"), and the one a citation
  author is actually spelling.

The git-root arm is gated on the row living under that git root, so a
`scope="umbrella"`/`"all"` scan cannot stamp this repo's name onto another repo's rows;
`containing_root` is reused for that comparison so component-boundary and
Windows-verbatim handling stay in one place.

### Live-verified 2026-08-28, against a prediction registered before the rebuild

The 20 raw values were classified by hand from the pre-fix scan, giving a falsifiable
number rather than a direction. Recorded in `03618605`'s commit message, then measured on
the rebuilt server:

| bucket | before | predicted | measured |
|---|---|---|---|
| `malformed_qualifier` | 20 | 5 | **5** |
| `cross_repo_file_qualified` | 0 | 15 | **15** |

And the five are exactly the `codescout:*` ones — `codescout:foo:F-2`,
`codescout:target:F-2`, and `codescout:statement-validity-session-log:F-2` ×3.

### Tests

One new test driving the production shape the old one cannot reach: **no `[[roots]]`
entry at all**. Mutation-verified by neutering the git-root arm — the new test fails, the
sibling passes, blast radius exactly one. That asymmetry is the point: the sibling
hand-builds the `Root` a real run derives, so it pins the branch logic and is blind to the
derivation. **A test that constructs the state production computes cannot tell you the
computation works.** It is the mirror of the vacuous-assertion class — a vacuous assertion
cannot fail at all; this one fails only in a world nobody runs in. Both read as coverage.

The new test also caught a defect in **itself** on its first run, by failing on the
self-repo arm while the cross-repo arm passed: `double_qualified_re` requires every
qualifier segment to match `[a-z][a-z0-9_-]{1,119}`, and `tempfile::tempdir()` yields
`.tmpAbC123` — leading dot, uppercase — which matches none of it, so that citation was
never extracted. Had the test asserted only the cross-repo half it would have gone green
over a citation that did not exist. It now creates a named `my-repo` git root and reads
the name back rather than hardcoding it.

Gate: `cargo fmt`, `cargo clippy --workspace --all-targets --features local-embed -D
warnings`, `cargo check --no-default-features`, `cargo test` — **4591 passed, 0 failed**.

## References

- `docs/issues/archive/2026-08-27-cross-repo-file-qualified-citation-unsupported.md` — the
  feature this reports inert.
- `HY-21` — same day, same scan: `prefix_conflicts` also reports a finding no reader can act
  on. Both are report-legibility defects in `link_scan`'s output rather than errors in its
  resolution.
- `get_guide("tracker-conventions")` § *Citing an entry* documents the split as working, so
  it needs a note or a fix — currently a reader is told these land in their own bucket.
