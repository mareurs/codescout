---
id: '88398b061917d789'
kind: bug
status: fixed
title: 'BUG: Windows path-separator audit missed .display() sites without chained .to_string()'
owners: []
tags:
- windows
- cross-platform
- test-portability
- follow-up
- audit-methodology
topic: null
time_scope: null
closed: '2026-07-13'
opened: '2026-07-07'
owner: marius
related:
- docs/issues/2026-07-07-windows-glob-overview-path-separator-test-mismatch.md
severity: low
---


# BUG: Windows path-separator survey scoped to `.display().to_string()` — broader `.display()` patterns not swept

## Summary

The 2026-07-07 Windows path-separator audit (two subagent forks) searched specifically for the regex `\.display\(\)\.to_string\(\)` across the codebase and found 54 sites in 33 files, of which ~23 were confirmed RISKY and all were fixed (see References). While fixing `src/server.rs`'s `post_process`, one instance of the *same defect* was found that the survey's own grep pattern couldn't see: `format!("{}/", p.display())` — `.display()` feeding directly into `format!`/`write!` without an explicit `.to_string()` call. This site turned out to be the single highest-leverage fix in the whole pass (the shared root-prefix-stripping mechanism behind every non-`run_command` tool response), and was only found by chance while investigating why `mv.rs`'s severity claim needed correction, not by systematic search.

## Symptom (Effect)

None currently known — this is a scope gap in the audit methodology, not an observed failure. The one instance found (`post_process`) has been fixed. There may be others: any `format!("...{}...", path.display())`, `write!(f, "{}", path.display())`, or `println!`/`tracing::info!` with `path.display()` interpolated directly, where the resulting string later crosses a comparison/persistence boundary.

## Reproduction

Not yet reproducible — no known failing case beyond the already-fixed `post_process`. Best lead: `grep(pattern="\\.display\\(\\)", glob="*.rs")` returned 237 matches in 89 files (superset of the 54-site `.display().to_string()` subset already audited) — the remaining ~183 matches across the delta of files have not been individually triaged for the same RISKY/SAFE distinction the two forks applied.

## Environment

Windows only, same as the parent bug class.

## Root cause

Audit methodology gap, but the mechanism is subtler than "the regex missed some call shapes." Two distinct blind spots, both now closed:

**1. The regex was line-oriented.** `\.display\(\)\.to_string\(\)` cannot match a rustfmt-wrapped chain where `.display()` and `.to_string()` land on different lines. Exactly three such sites exist (`grep '^\s*\.display\(\)'`) — `src/symbol/edit.rs:440`, `src/tools/symbol/edit_code.rs:349`, `src/server.rs:2614` — and **all three were RISKY**. A 100% hit rate on a class the regex structurally could not see.

**2. `.display()` consumed without `.to_string()`** — via `format!`/`write!` interpolation or string concatenation. This is the class the original bug named.

### The correction that actually decides risk

**`Path::display()` does NOT convert `/` → `\`.** It renders the `PathBuf`'s internal string verbatim. Backslashes appear only when the `PathBuf` is **OS-derived** — from a `WalkBuilder`/`read_dir` entry, `canonicalize()`, `current_dir()`, an LSP `Url::to_file_path()`, or a `.join()`/`.push()` (which insert `MAIN_SEPARATOR`).

So **provenance decides risk, not the shape of the call chain.** This retroactively validates the earlier audit's decision to leave `row.abs_path.display().to_string()` alone in `get.rs:248`, `find.rs:503`, `gather.rs:392` and friends: catalog rows are `PathBuf::from(<forward-slash TEXT from SQLite>)`, so `.display()` re-emits forward slashes on every platform. Those were never bugs.

**Second load-bearing fact:** `server.rs::post_process` builds its strip prefix as `format!("{}/", to_forward_slash(&p))`. So on Windows a raw-`.display()` **absolute** path in tool output doesn't merely ship backslashes — it **silently fails to match the strip prefix**, leaking the absolute root AND skipping the "paths are relative to" banner.
## Evidence

- `src/server.rs:454-459` (now fixed, commit `62457959`) — `let root_prefix = ... .map(|p| format!("{}/", p.display())) ...` — no `.to_string()` in the chain, invisible to both audit forks' grep pattern.

## Hypotheses tried

N/A — not yet investigated systematically; the one known instance was found incidentally.

## Fix

**Done (2026-07-13).** Full delta swept: 213 total `.display()` sites, minus the 29 single-line `.display().to_string()` already covered = **184 examined**. Result: **13 RISKY, 169 SAFE, 2 UNCERTAIN**.

All 13 RISKY sites fixed (3 of them — `tree.rs:138`, `edit_code.rs::do_rename`, `symbol/edit.rs::text_sweep` — were also omnibus `49ee6a03` F7/F9, fixed in the same round and corroborated independently by this sweep):

| Site | Risk | Fix |
|---|---|---|
| `retrieval/sync.rs:130` `chunk_id` | **PERSISTED** — vector-store primary key + identity key for the `local_ids`/`server_ids` delete-set diff | extracted a pure `chunk_id()` using `to_forward_slash` |
| `tools/onboarding.rs:715` | **PERSISTED** — written into `workspace.toml` as `ProjectEntry.root` and read back as a path | `to_forward_slash` |
| `tools/onboarding.rs:807` | **PERSISTED** — git-tracked `.codescout/**/memories/onboarding.md` | `to_forward_slash` |
| `tools/onboarding.rs:1000` | JSON `discovered_projects[].root` | `to_forward_slash` |
| `tools/tree.rs:138` | JSON `entries[]` + defeats the `post_process` strip | `to_forward_slash` |
| `symbol/edit.rs::text_sweep` | JSON `textual_matches[].file` | `relative_forward_slash` |
| `tools/symbol/edit_code.rs::do_rename` | JSON `corruption_hints[].file` | `relative_forward_slash` |
| `tools/symbol/references.rs:318` | prose hint | `relative_forward_slash` |
| `prompts/builders.rs:259,766` | generated **tool-call literals** handed to the agent; the draft is persisted to the `system-prompt` memory | `to_forward_slash` |
| `prompts/builders.rs:354,739`, `prompts/mod.rs:376` | same persisted prompt surface (prose) | `to_forward_slash` |
| `server.rs:2610` (test) | **COMPARED** — built the expected root with `.display()` while `post_process` strips with `to_forward_slash`; on Windows the test asserts against a string the code never emits | `to_forward_slash` |
| `librarian/tools/context.rs:441` (test `mk_ctx`) | **PERSISTED** — `UPDATE artifact SET abs_path = REPLACE(...)` wrote native separators into `abs_path`, violating the forward-slash `RepoPath` invariant every LIKE query depends on | `to_forward_slash` |
| `librarian/tools/get.rs:625` (test `mk_ctx_with_root`) | same | `to_forward_slash` |

**Two decisive corroborations** that these are real rather than theoretical — in both cases the correct form already sits adjacent to the broken one:
- `tools/tree.rs` — the sibling `glob_impl:228` *was* fixed to `to_forward_slash(entry.path())` by the earlier pass; `list_dir_impl:138`, same file, same tool, same walker, was missed.
- `retrieval/sync.rs` — line 131 already calls `to_forward_slash(rel_path)` for `file_path`, one line below the un-normalized `chunk_id` built from **the same `rel_path`**.

**Root cause behind 6 of the 13:** `DiscoveredProject.relative_root` is set at `src/workspace.rs:156` from `dir.strip_prefix(workspace_root)` over an OS walk, so a nested project is `crates\librarian-mcp` on Windows. Fixed at each *render* site rather than at construction — `relative_root` is legitimately a `Path` used for path comparisons (`starts_with`, `==`), which are separator-correct as-is; only the stringification was wrong. Normalizing the `PathBuf` itself would have changed a struct invariant with a blast radius I cannot verify on Linux.

### UNCERTAIN — 2, deliberately left alone

Both are error/diagnostic prose that happens to sit inside a structured field, and in both cases an identical sibling was in the *previous* audit's scope and was left unfixed — so changing only these two would create inconsistency without resolving the underlying policy question.

1. `librarian/tools/reindex.rs:226` — `format!("{}: {}", abs_root.display(), e)` pushed to `backfill_errors`, which *is* emitted as a `json!` array. Sibling `:257` (`"targets"`) was in scope before and untouched.
2. `librarian/tools/audit_doc_refs/resolver.rs:70` — `notes: Some(format!("resolved by basename to {}", ...))`, a serialized struct field. Its sibling at `:76` was in scope before and untouched.

**Open policy question these expose:** does the project treat a path inside an *error string* that lives inside a *JSON field* as output (normalize) or as prose (leave)? Worth deciding once and applying uniformly rather than case-by-case.

### Follow-on the regex still cannot see

`to_string_lossy()` reaches the same defect by a third route. `onboarding.rs:715` and `:1000` were found and fixed here only because they sat beside a `.display()` site. **A genuinely complete sweep needs `\.display\(\)|to_string_lossy\(\)`** — not attempted in this pass, and the honest reason this bug is closed as "delta swept" rather than "class eliminated."
## Tests added

`chunk_id_normalizes_native_separators` — `src/retrieval/sync.rs`.

This one gets a **genuine Linux RED**, which most of this class cannot. The trick: `to_forward_slash` is not `cfg(windows)`-gated, so a `PathBuf::from("src\\retrieval\\sync.rs")` reproduces the Windows shape on any host — the same technique `util/fs.rs`'s own tests already use. Mutation-checked: reverting `chunk_id` to `.display()` fails with `left: "proj:src\\retrieval\\sync.rs:deadbeef"` — exactly the string Windows would have persisted as the vector store's primary key.

That test was possible only because the value could be lifted into a pure function taking a `&Path`. The remaining sites read straight from a live `WalkBuilder`/LSP path, so there is nothing to inject and **no Linux test can distinguish before from after** — on Linux the two forms are byte-identical. For those, the guarantee is "identical to the shared helper already used at ~23 audited sibling sites," stated plainly rather than propped up with a tautological test. See the same reasoning in omnibus `49ee6a03` § F7/F9.

Full suite: 3203 passed, 0 failed. `cargo fmt` + `cargo clippy --all-targets -- -D warnings` clean.
## Workarounds

None needed for now — no known live failure beyond the fixed instance.

## Resume

N/A — the `.display()` delta is swept and fixed. Two follow-ons are recorded above rather than silently dropped: (a) the `to_string_lossy()` route to the same defect is **not** swept; (b) the "path inside an error string inside a JSON field" policy question is undecided, which is why 2 sites are left UNCERTAIN.

Cherry-pick to `master` pending; cite the master-side SHA before archiving.
## References

- `docs/issues/2026-07-07-windows-glob-overview-path-separator-test-mismatch.md` — the original bug this whole audit traces back to.
- `docs/issues/2026-07-07-list-overview-remaining-display-path-separator-sites.md` — the first-identified follow-up (now also fixed as part of this pass, via `to_forward_slash` in `list_overview.rs`... note: verify this file's status is updated separately, it predates this survey).
- `docs/trackers/bug-fix-session-log.md` (`2dd9d90bc83f9f49`) — F-27, the session-log entry for the broader audit's provenance.
