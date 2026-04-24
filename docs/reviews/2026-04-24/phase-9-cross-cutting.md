# Phase 9 — Cross-Cutting

**Date:** 2026-04-24
**Scope:** `src/platform/`, `src/util/`, `src/usage/`, `src/logging.rs`, `src/hardware.rs`
**Reviewer:** superpowers:code-reviewer + buddy:security-ibex
**Status:** open

---

## Cross-check answers (Phase 1-8)

- **Phase 1 S1 (`generate_auth_token` weakness):** **Confirmed.** `src/util/` exposes no CSPRNG primitive. Right fix: add `util::rand::secure_token(bytes)` wrapper around `rand::rngs::OsRng` / `getrandom`.
- **Phase 2 F4 / Phase 3 S2 (`validate_read_path` containment):** **Confirmed.** No project-root requirement on absolute paths. Deny-list is the only check. Audit table below shows multiple gaps.
- **Phase 1 C2 (write-guard timeout):** **Refuted at this surface.** `PathSecurityConfig.write_lock_timeout_secs` (default 5s, configurable). The "unbounded" Phase 1 claim was about queue depth, not the timeout itself.
- **Phase 5 I3 (`LibraryRegistry::save` atomicity):** **Confirmed non-atomic.** `crate::util::fs::atomic_write` exists at `src/util/fs.rs:52-68`, correctly implemented (write to `.tmp`, preserve mode, rename), used in 13 places. Library registry skips it. → C9-1.
- **Phase 6 (uncached `Repository::discover`):** `src/util/` has no cached-repo helper. Centralizing here is the right home if added.
- **Windows parity for `terminate_process`:** `src/platform/windows.rs:54-69` shells to `taskkill /PID <pid> /F` — `/F` is force, Windows analogue of SIGKILL not SIGTERM. Doc on trait promises `TerminateProcess`. Divergence. → I9-1, S9-3.

### Deny-list audit (`src/util/path_security.rs:130-148` + `src/platform/{unix,windows}.rs`)

| Path | Linux | macOS | Windows |
|---|---|---|---|
| `~/.ssh` | ✓ | ✓ | ✓ |
| `~/.aws` | ✓ | ✓ | ✓ |
| `~/.gnupg` | ✓ | ✓ | ✓ |
| `~/.config/gh` | ✓ | ✓ | ✓ |
| `~/.config/gcloud` | ✓ | ✓ | **MISSING** |
| `~/.netrc` | ✓ | ✓ | ✓ |
| `~/.npmrc` | ✓ | ✓ | ✓ |
| `~/.docker/config.json` | ✓ | ✓ | ✓ |
| `~/.kube/config` | ✓ | ✓ | ✓ |
| `~/.git-credentials` | **MISSING** | **MISSING** | ✓ |
| `~/.pypirc` | **MISSING** | **MISSING** | ✓ |
| `/etc/shadow` / `/etc/gshadow` | ✓ | n/a | n/a |
| `/etc/master.passwd` | n/a | ✓ | n/a |

**Universal gaps (every platform):**
- `~/.config/git/credentials`, `~/.git-credentials` (Unix uses git-credentials too)
- `~/.config/op/` (1Password), `~/.password-store` (pass), `~/.config/Bitwarden/`
- `~/.cargo/credentials.toml`, `~/.cargo/credentials`
- `~/.pgpass`, `~/.my.cnf`
- `~/.config/atuin`, `~/.bash_history`, `~/.zsh_history`, `~/.psql_history`, `~/.python_history`
- `~/.local/share/keyrings/` (GNOME keyring)
- `/private/etc/sudoers`, `/etc/sudoers`, `/etc/sudoers.d/`
- macOS Keychain: `~/Library/Keychains/`
- `/proc/<pid>/environ`, `/proc/<pid>/mem` (Linux — environ leaks any process's env)

---

## Security (Ibex)

### S9-1 — MEDIUM — Read deny-list incomplete; only check on absolute reads
- **Location:** `src/util/path_security.rs:130-148, 189-231`; `src/platform/unix.rs:11-23`; `src/platform/windows.rs:11-24`.
- **Evidence:** See deny-list audit table above. No project-root containment on absolute reads. `read_file`/`read_markdown` go straight here.
- **Exploit:** Prompt injection in indexed content coerces `read_file("~/.cargo/credentials.toml")` or `read_file("~/.pgpass")`. Passes validation; contents flow back into model context.
- **Fix:** (a) Extend `denied_read_prefixes` per table. (b) Stronger: invert model — for `Default` profile, require absolute reads under `project_root`, explicit `extra_read_root`, or registered `library_paths`. Deny-list becomes belt-and-suspenders. Block `/proc/<pid>/environ` and `/proc/<pid>/mem` patterns on Linux.
- **Confidence:** high (incompleteness); medium (exploitability — depends on calling tool's gates, but `read_file` goes straight here).

### S9-2 — MEDIUM — `generate_auth_token` predictable to co-located attacker
- **Location:** `src/server.rs:765-781`.
- **Evidence:** `format!("{:016x}{:016x}", nanos_as_u64, pid * 0x517cc1b727220a95)`. `pid` observable via `/proc`; `nanos` at startup bounded by attacker's launch-time observation.
- **Exploit:** Local unprivileged process reads `/proc/<server_pid>/stat` (start time → narrow nanosecond window) + pid, brute-forces remaining nanosecond uncertainty (millions of candidates, hashable in seconds), connects to bound HTTP port with guessed bearer.
- **Fix:** Replace with `OsRng`/`getrandom` (already in dep tree). Add `util::rand::secure_token(usize)` so future callers don't reinvent.
- **Confidence:** high.

### S9-3 — LOW — `taskkill` shell-out vulnerable to PATH hijacking on Windows
- **Location:** `src/platform/windows.rs:54-69` (`terminate_process`), `:71-80` (`process_alive`).
- **Evidence:** `Command::new("taskkill")` and `Command::new("tasklist")` rely on PATH resolution. Windows `.` first under default PATH search → attacker-dropped `taskkill.exe` in CWD runs with server privileges.
- **Fix:** Use absolute `%SYSTEMROOT%\System32\taskkill.exe`, OR call `TerminateProcess` via `windows`/`winapi` crate (matches trait doc on `src/platform/mod.rs:46`). Same for `tasklist.exe`.
- **Confidence:** medium.

### Q9-1 — debug-mode `usage.db` stores raw input/output JSON
- `src/usage/mod.rs:44-87` (`write_content`) stores `serde_json::to_string(input)` + full output blocks into `tool_calls.input_json`/`output_json` when `debug=true`. Opt-in flag, lives under `.codescout/usage.db` (typically gitignored — verify). **Not a finding** but: docs should warn that with `debug=true`, `usage.db` becomes secret-bearing (any `replace_symbol`/`edit_file` payload lands in there).

---

## Critical (non-security)

### C9-1 — `LibraryRegistry::save` non-atomic; partial write on crash corrupts registry
- **Location:** `src/library/registry.rs:63-70`.
- **Evidence:** `std::fs::write(path, data)?` directly. `crate::util::fs::atomic_write` exists for exactly this; used in 13 other places.
- **Impact:** Crash mid-write → truncated JSON → next startup fails to parse → user loses all `register_library` records, no workaround.
- **Fix:** One-line change to `crate::util::fs::atomic_write(path, &data)?`. Add regression test simulating partial write.

### C9-2 — `Default` security profile's `validate_read_path` is effectively `Root` outside the deny-list
- **Location:** `src/util/path_security.rs:189-231`.
- **Evidence:** Only difference between `Default` and `Root` for absolute paths is deny-list intersection. Given S9-1 gaps, closer to "Root with tiny exclusion set" than "containment by default."
- **Fix:** Same containment redesign as S9-1. Listed critical because it's the system's load-bearing assumption ("Default profile is bounded") and the assumption does not hold.

---

## Important

### I9-1 — Windows `terminate_process` uses `/F` (force-kill); Unix uses SIGTERM (graceful)
- **Location:** `src/platform/unix.rs:63-70` vs `src/platform/windows.rs:54-69`.
- **Evidence:** Cross-platform contract is asymmetric. Windows children killed without warning → kotlin-LSP can leave stale lock files where it wouldn't on Linux. Doc on `src/platform/mod.rs:46` says "Windows: TerminateProcess" but impl is `taskkill /F`.
- **Fix:** Send `Ctrl+Break` first (`GenerateConsoleCtrlEvent`) with short grace, fall back to `TerminateProcess`. Or call `TerminateProcess` directly via `windows-sys` and update doc.

### I9-2 — `probe_ram` swallows macOS errors; spawns `sysctl` on Linux unnecessarily
- **Location:** `src/hardware.rs:172-203`.
- **Evidence:** macOS branch executes regardless of platform (no `cfg`); on Linux spawns `sysctl` if `/proc/meminfo` parse failed. Returns `0` on failure — telemetry can't distinguish "couldn't probe" from "0 GiB system."
- **Fix:** Wrap macOS branch in `#[cfg(target_os = "macos")]`; add Windows branch via `GlobalMemoryStatusEx`. Return `Option<u64>`.

### I9-3 — `is_denied` deny-list bypass when `$HOME` is a symlink
- **Location:** `src/util/path_security.rs:151-155, 118-127`.
- **Evidence:** `expand_home("~/.ssh")` returns `$HOME/.ssh` un-canonicalized. If `$HOME` is symlink (`/home/user → /var/users/user`), `validate_read_path` canonicalizes input to `/var/users/user/.ssh/id_rsa`, compares to `/home/user/.ssh` → `starts_with` false → deny-list bypassed.
- **Fix:** Canonicalize each entry of deny-list once at build time (or in `denied_read_paths`); compare canonicalized forms. Currently only resolved input is canonicalized.
- **Confidence:** medium — depends on real $HOME-as-symlink deployments (macOS FileVault, NFS-mounted homes).

---

## Minor (grouped)

- **`shell_tokenize` parity:** Unix (`unix.rs:29-61`) handles single quotes, double quotes, backslash escapes. Windows (`windows.rs:30-52`) only double quotes — no single-quote support, no escape, hardcoded `' '` (tabs/newlines don't split). Document or use real tokenizer (`shlex` Unix, `winsplit-rs` Windows).
- **`probe_nvidia` / `probe_amd` shell out without absolute paths** — same PATH-hijack class as S9-3, lower risk (read-only). `which::which` once at startup, log resolved.
- **`SizeRotatingFile::rotate` (`src/logging.rs:113-126`)** — uses `rename` to shift backups; mid-rename failure → inconsistent rotation chain. Defensive `if rename fails, log to stderr and continue` instead of silent partial state.
- **`open_db` migration (`src/usage/db.rs:36-43`)** — `has_session_id` probe + `ALTER TABLE` works once but no version table to coordinate future migrations. Will get painful at v0.10.
- **`window_to_modifier` (`src/usage/db.rs:378-385`)** maps unknown windows silently to "30 days." Caller passing `"7days"` typo gets 30d data with no warning. Strict enum or surface unknown.
- **`expand_home` returns `Some(PathBuf)` for any non-`~` input** — falls through to `PathBuf::from(pattern)`. Naming suggests `~`-only success. Cosmetic.

---

## SQL injection check
Reviewed all `conn.execute`/`conn.prepare`/`query_map` in `src/usage/db.rs`. All user-controllable values flow through `params!` or `[modifier]` bind arrays. **No SQL injection candidates found.** Confidence: high.

---

## Logging — secret leakage check

`src/logging.rs::install_panic_hook` writes `format!("epoch={epoch}  PANIC  {info}\n")` to `crash.log`. `PanicInfo`'s `Display` includes payload — if anywhere panics with a string containing a secret (`panic!("failed to parse token {token}")`), it lands on disk. Spot-checked: most `panic!`/`unwrap` sites in cross-cutting modules don't carry user secrets. Convention: never include `auth_token`, file contents, or `output_json` in panic messages.

`tracing::warn`/`error` calls in scope don't log auth tokens or file contents. Debug-mode `usage.db` is bigger surface (Q9-1).

---

## Observation: workarounds vs clean abstractions

~80% clean abstractions, ~20% incident-driven scaffolding.

**Clean:**
- `src/util/fs.rs` — small, focused; `atomic_write` is real abstraction (13 call sites).
- `src/util/text.rs` — tight.
- `src/platform/mod.rs` — thin facade with per-OS impls is textbook.

**Accumulated workarounds:**
- `src/util/path_security.rs` — **1,500+ lines and growing.** Deny-lists, allow-lists, command parsers, regex source-file detection, `split_outside_quotes`, `check_source_file_access` with HEREDOC handling. The file where threat model accumulates as patches rather than primitives. Mixing path validation + shell command inspection in one module is the smell. **Split into `path_security` (paths) + `shell_security` (commands) before another 500 lines.**
- `SizeRotatingFile` (BUG-047) in `logging.rs` — clean retrofit forced by an outage, well-tested now. Existence is workaround for upstream `rmcp` bug + original `ResilientStdin` spinning-pending mistake. Wouldn't exist in clean architecture.
