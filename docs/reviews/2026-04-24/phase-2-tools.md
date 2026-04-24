# Phase 2 — Tools Layer

**Date:** 2026-04-24
**Scope:** all of `src/tools/` EXCEPT `src/tools/symbol/` (Phase 4)
**Reviewer:** superpowers:code-reviewer + buddy:security-ibex
**Status:** open

---

## Security (Ibex)

### F1 — MEDIUM — Argv flag-confusion in `gh` CLI calls
- **Location:** `src/tools/github.rs:680-690` (`pr create`), `:701-720` (`pr update`), `:933-945` (`GithubFile`).
- **Evidence:** `head`, `base`, `title`, `body`, `branch`, `sha`, `message` all forwarded to `gh` argv unvalidated. A value like `"--repo=other/repo"` or `"--add-label=critical"` is parsed by `gh`'s clap as a flag, not a value.
- **Exploit:** Prompt-injected agent calls `github_pr(method="create", title="--add-label=critical")` → silent label add. More dangerous variants could redirect target repo.
- **Fix:** Insert `--` after fixed flags so positional args following are not flag-interpreted, OR use `--head=<value>` form so `gh` cannot reinterpret. Also call `require_owner_repo` consistently across all `gh_pr` arms (currently only some).
- **Confidence:** medium-high.

### F2 — MEDIUM — URL path/query injection in `github_file`
- **Location:** `src/tools/github.rs:902-908`
- **Evidence:**
  ```rust
  let mut endpoint = format!("/repos/{owner}/{repo}/contents/{path}");
  if let Some(r) = params["ref"].as_str() { endpoint.push_str(&format!("?ref={r}")); }
  let out = run_gh(&["api", &endpoint]).await?;
  ```
  `path` and `ref` appended raw — no URL-encoding, no `?`/`#`/`..` rejection.
- **Exploit:** `path="../../user/keys"` pivots GitHub API target away from `contents/`. `ref="main&path=other"` injects second query param. Same untrusted format used in `create_or_update` and `delete` (PUT/DELETE) → more dangerous redirect potential.
- **Fix:** Percent-encode each path segment with `utf8_percent_encode`, joining with literal `/`, rejecting `..` and `.` segments. Reject `path` containing `?` or `#`. URL-encode `ref` and reject chars outside `[A-Za-z0-9._/-]`.
- **Confidence:** high.

### F3 — LOW — `run_command.cwd` accepts `/tmp` literal, ignores `platform::temp_dir()`
- **Location:** `src/tools/run_command.rs:589-612` (`resolve_work_dir`).
- **Evidence:** `under_tmp` hardcoded to `/tmp`; on macOS `$TMPDIR=/var/folders/...` so divergence from `validate_write_path`. Absolute `/tmp/...` accepted because `Path::join("/tmp")` returns `/tmp`. `cwd="../../../tmp/x"` resolves and is accepted.
- **Fix:** Use `crate::platform::temp_dir()`. Document `cwd` semantics in schema. Reject absolute `cwd` not under project/temp before canonicalize.
- **Confidence:** medium. Hardening, not a hole — `run_command` is privileged shell.

### F4 — LOW (or design question) — `validate_read_path` does NOT enforce project-root containment
- **Location:** `src/util/path_security.rs:189-231` (cross-cuts tools layer; every read tool trusts it).
- **Evidence:** Reads checked against deny-list ONLY. `read_file(path="/home/user/.aws/credentials")` permitted unless deny-listed. `glob(path="/", pattern="**/*.pem")` walks entire FS.
- **Cross-check answer to Phase 1 S3:** Mixed. Write paths re-validate against project root + allowed roots. **Read paths do NOT re-validate** — only deny-list.
- **Fix:** Either contain reads to `project_root ∪ library_paths ∪ extra_read_roots`, OR document deliberately and audit `denied_read_paths` for `~/.aws`, `~/.ssh`, `~/.config/gh`, `~/.netrc`, `/etc/shadow`.
- **Confidence:** high on semantic; finding-status depends on intent.

### F5 — INFO — `SecurityProfile::Root` bypass
- **Location:** `src/util/path_security.rs:251-258`
- **Evidence:** Under `Root`, write paths accepted without containment, deny-list, or `..` rejection. By design.
- **Verify:** `ActivateProject` does not write `profile=root` from a tool-callable surface.
- **Confidence:** high.

---

## Critical (non-security)

### C1 — `index_project` is heavy-mutating but NOT in `WRITE_TOOLS`
- **Location:** `src/server.rs:45-52` + `src/tools/semantic.rs:343-650`.
- **Issue:** Writes `.codescout/index.db`. Excluded from cross-process write lock. Two concurrent `index_project` calls trample sqlite. `IndexingState::Running` is per-process only.
- **Fix:** Add to `WRITE_TOOLS` OR document why inner sqlite locks are sufficient. server.rs comment is silent on `index_project` (vs explicit on `register_library`/`onboarding`).

### C2 — `onboarding` excluded from `WRITE_TOOLS` with weak rationale
- **Location:** `src/server.rs:54-57` (comment), `src/tools/onboarding.rs`.
- **Issue:** Comment says "infrequent and memory writes are small." But `perform_full_onboarding` writes system prompt, registers libraries, writes multiple memory files. Concurrent onboarding + memory write races.
- **Fix:** Add `"onboarding"` to `WRITE_TOOLS`.

### C3 — `register_library` excluded; "idempotent" claim broken under concurrent different-lib writes
- **Location:** `src/server.rs:53-55`.
- **Issue:** Two concurrent `register_library` (different libs) → both read registry, both append, both `save()` → last-writer-wins. Same hazard from `auto_register_deps` triggered by `activate_project`.
- **Fix:** Add to `WRITE_TOOLS`, OR in-process lock around `library_registry.register + save`.

### C4 — `EditFile.batch` mode skips definition-keyword guard
- **Location:** `src/tools/edit_file.rs:130-176` vs single-edit gate at `:222-237`.
- **Issue:** Multi-line `fn`/`struct`/`class` replacement allowed via batch — contradicts CLAUDE.md Iron Law 2 (structural edits must use `replace_symbol`).
- **Fix:** Lift def-keyword check into the per-edit loop in batch mode.

### C5 — `EditFile.prepend/append` skips def-keyword gate
- **Location:** `src/tools/edit_file.rs:180-209`.
- **Issue:** Same root cause as C4. Less likely abuse (typically imports) but worth closing.

---

## Important

### I1 — `EditFile` returns `{status:"ok", warning|hint}` — soft no-echo violation
- **Location:** `src/tools/edit_file.rs:300-315, 337-345`.
- **Issue:** CLAUDE.md says writes return `json!("ok")`. The "syntax error after edit" warning is allowed by spec ("LSP diagnostics after a write"). Unread-section coverage hint is borderline — session-scoped state, not edit-derived.
- **Fix:** Decide and document policy.

### I2 — `CreateFile` overwrites silently
- **Location:** `src/tools/create_file.rs:31-42`.
- **Issue:** No `O_EXCL`, no pre-check. LLM thinking "create" is safe creates-or-overwrites.
- **Fix:** `if resolved.exists() { return RecoverableError("file exists; use edit_file or pass overwrite=true") }`.

### I3 — `summarize_list` swallows JSON parse errors
- **Location:** `src/tools/github.rs:93-110`.
- **Issue:** `serde_json::from_str(&content).ok()?` silent on parse failure. Mid-output truncation from `gh` produces no summary, no error.
- **Fix:** Add tracing.

### I4 — `spawn_background_command` 5s warm-up has no cancel-aware kill
- **Location:** `src/tools/run_command.rs:637`.
- **Issue:** During the 5s `tokio::time::sleep`, cancellation leaves the spawned process detached without kill. Orphan process.
- **Fix:** Document, or attach cancel-aware kill.

### I5 — `inject_tee` pipe detection by string-parse is fragile
- **Location:** `src/tools/run_command.rs:670` (`detect_terminal_filter`).
- **Issue:** `echo "a | b" | head` may misidentify `|` inside quoted string as pipe operator. Tee injected mid-quoted-string → broken command.
- **Fix:** Deeper look needed. Consider shellwords-aware tokenization.

### I6 — Cross-check Phase 1 I4: no test holds write-guard while injecting cancel
- Suggest adding one.

---

## Minor (grouped)

- **No-echo violations (3 places):** `EditFile` (warning, hint), `EditMarkdown` (hint). Decide policy + document.
- **`run_gh` lossy UTF-8** — `String::from_utf8_lossy` silently mojibakes.
- **`require_owner_repo` inconsistent across `gh_pr` arms** — list/search/get/get_diff/get_files/get_comments/get_status skip it.
- **`GithubPr.merge` silently defaults unknown `merge_method` to `"merge"`** (`src/tools/github.rs:723-728`). Should error.
- **`memory(action="write")` no topic length/charset validation at boundary** — relies on deeper `sanitize_topic`. Defense-in-depth: reject newline/control chars at boundary.
- **`ProjectStatus`** silently swallows `WorkspaceConfig` parse failures (`src/tools/config.rs:240-260`).
- **`perform_edit`** O(2N) `matches` + `match_indices`. Tiny.
- **Positive:** `run_command_inner` SIGPIPE/process-group comment block — excellent signal-to-noise.

---

## Cross-check answers (Phase 1)

- **S3 (path traversal):** Mixed. **Writes** re-validate against project root. **Reads** do not — F4. Phase 1 S3 confirmed for write surface; reads have a wider semantic that needs an intent decision.
- **S6 (`api_key` leak):** Zero hits in `src/tools/` for `api_key|secret|password`. `ProjectStatus` returns `embeddings_model` only, no credentials. Out-of-scope: agent.rs logging, `ProjectConfig` deserialization paths.
- **I3 (`WRITE_TOOLS` coverage):** `index_project` (C1), `onboarding` (C2), `register_library` (C3) silently bypass. `activate_project` mutates session state only — probably OK. `MEMORY_WRITE_ACTIONS` correctly covers `write|remember|forget|delete|refresh_anchors`.
- **I4 (cancel + write-guard test):** Not present in `tools/` scope.

---

## Open questions

1. F4 — is whole-FS read (modulo deny-list) intentional? Cross-project nav vs scope.
2. C1 — concurrent `index_project` across MCP sessions: contract?
3. F2 — do you want sub-path support (`path="src/lib.rs"` with `/`)? Affects fix shape (segment-encode vs reject `/`).
4. C4 — confirm batch mode should enforce def-keyword guard.
