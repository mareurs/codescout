# Phase 6 — Git

**Date:** 2026-04-24
**Scope:** `src/git/` (single file `mod.rs`, 158 LoC)
**Reviewer:** superpowers:code-reviewer + buddy:security-ibex
**Status:** open

---

## Scope reality check

`src/git/` is **libgit2-only via `git2` crate** — no `Command::new("git")`, no `Command::new("gh")`, no HTTP, no URLs. Audit brief's heaviest categories (argv injection, URL injection, SSRF, credential exposure, path traversal via pathspec) collapse.

---

## Cross-check answers (Phase 1-5)

- **Phase 2 F1 (gh CLI argv flag-confusion):** Does NOT apply to `src/git/`. No shell-out. `gh` lives in `src/tools/github.rs` only.
- **Phase 2 F2 (URL injection in `github_file`):** Does NOT apply. Zero URLs built here.
- **Phase 1 I5 (`Repository::open` per `list_tools`):** **Confirmed and broader.** `open_repo` not cached. Four call sites:
  - `src/embed/index.rs:1497` (per `is_git_repo` check during indexing)
  - `src/embed/index.rs:1829` (per incremental update)
  - `src/embed/index.rs:2369` (another indexing path)
  - `src/dashboard/api/project.rs:45` (per dashboard request)
  Each opens repo, does one short op, drops. Large monorepo → `discover` parent stat-walking on every call. Single `Arc<Repository>` on `ActiveProject` if profiling shows hot.

---

## Security (Ibex) — no findings ≥ MEDIUM

### Q1 — `revparse_single` accepts attacker-influenced revspecs (defense-in-depth)
- **Location:** `src/git/mod.rs:35-36`.
- **Evidence:** `from_sha`/`to_sha` are `&str`; no validation. `revparse_single` accepts full revspec grammar (`HEAD~3`, `:/regex`, `branch@{1}`).
- **Today:** all callers (`embed/index.rs:1497, 1829, 2369`, `dashboard/api/project.rs:45`) pass values they just read from git themselves. Worst-case: confusing wrong diff, not RCE.
- **Ask:** if any future tool exposes `from_sha`/`to_sha` to user/LLM input, add `[0-9a-f]{4,40}` allowlist or `Oid::from_str` validation. One-line doc-comment on `diff_tree_to_tree`: "callers must validate untrusted revspecs."

### Q2 — `Repository::discover` walks upward; no ceiling directories
- **Location:** `src/git/mod.rs:8`.
- **Evidence:** Standard libgit2 behavior. If user activates `/tmp/foo` and there's a stray `.git` above, codescout silently binds wrong repo. Related to MEMORY.md flake (`detect_project_root_finds_cargo_toml`).
- **Ask:** Pass `ceiling_dirs` (parent of project root) to prevent ancestor escape. Assert `repo.workdir() == Some(project_root)` before use.

### INFO — `DiffEntry.path` not validated as in-tree
- **Location:** `src/git/mod.rs:62-65`.
- **Evidence:** libgit2 rejects `..` paths at object-write time. Consumer (`embed/index.rs:1508`) uses for cache-invalidation lookups, not `fs::read` joins. Not exploitable.
- **Ask:** if future caller does `project_root.join(entry.path)` + `fs::read`, add `Path::components` traversal check.

### INFO — `revparse_single` raw error propagation
- **Location:** `src/git/mod.rs:35-36`.
- **Evidence:** `git2::Error` propagates with requested SHA verbatim. All call sites use `.ok()` — never reaches remote. No info disclosure.

---

## Critical / Important
None.

---

## Compliance (CLAUDE.md)

- **`RecoverableError`:** `open_repo` uses `anyhow::anyhow!`. Correct here — library helper, routing belongs to tool layer. Callers use `.ok()` (no `isError: true` leak). **Compliant.**
- **Progressive disclosure on diffs:** `diff_tree_to_tree` returns `Vec<DiffEntry>` with no cap. Current callers internal-only. If future tool surfaces, wrap in `OutputGuard`. Not a finding now.

---

## Minor

- M1 — `let mut diff = diff;` at `:46` no-op rebind. Remove.
- M2 — `find_opts.renames(true)` only rename option; `copies(true)` and `break_rewrites` off. One-line comment for future reader.
- M3 — `_ => continue` at `:60` silently drops `Typechange`/`Copied`/`Untracked`/`Ignored`/`Conflicted`. Fine for indexing cache; if ever feeding LLM "what changed", callers will be surprised. Comment-only change.

---

## Bottom line

Thin, well-scoped libgit2 wrapper. No shell-out, no HTTP, no untrusted-input flow currently reachable, no credentials, no unsafe. One real concern: uncached `Repository::discover` (Phase 1 I5 expanded) — performance not security. Two questions for pre-emptive hardening if surface widens.
