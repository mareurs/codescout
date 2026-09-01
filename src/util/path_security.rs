//! Path security: read deny-list and write sandboxing.
//!
//! # Permission Model
//!
//! The model is intentionally asymmetric:
//!
//! - **Reads** are allowed anywhere on disk *except* a built-in deny-list of
//!   sensitive credential paths (`~/.ssh`, `~/.aws`, `~/.gnupg`, etc.) plus
//!   Use [`validate_read_path`].
//!
//! - **Writes** are restricted to the active project root by default. The
//!   caller may extend this with `extra_write_roots` in [`PathSecurityConfig`],
//!   but the deny-list always applies first — `extra_write_roots` cannot unlock
//!   credential paths. Use [`validate_write_path`].
//!
//! # Write Validation Flow
//!
//! [`validate_write_path`] runs three sequential checks:
//!
//! 1. **Null/empty rejection** — malformed paths fail immediately.
//! 2. **Deny-list** — checked before the root boundary so it cannot be
//!    bypassed by configuration.
//! 3. **Workspace boundary** — the path's parent directory is canonicalized
//!    (not the target file, which may not exist yet) and checked against
//!    `project_root` and each `extra_write_roots` entry. This catches
//!    symlink escapes.
//!
//! # Agent Safety
//!
//! Violations return a hard [`anyhow::Error`] carrying a corrective message
//! (e.g. "outside the project root. Call approve_write(…)"). The MCP layer
//! surfaces this as `isError: true` — a path/security-boundary violation is a
//! **fatal** tool error. This is a deliberate exception to the general
//! "input-driven failure → RecoverableError" convention (see
//! `get_guide("error-handling")`): a boundary breach fails loudly rather than
//! being silently absorbed by sibling parallel calls. The hard-fail behavior is
//! pinned by `validate_write_path_still_bails_outside_with_unchanged_message`.

use anyhow::{bail, Result};
use regex::Regex;
use std::path::{Component, Path, PathBuf};

/// Paths that are always denied for read access (expanded from `~`).
#[cfg(target_os = "linux")]
const DEFAULT_DENIED_EXACT: &[&str] = &["/etc/shadow", "/etc/gshadow"];

#[cfg(target_os = "macos")]
const DEFAULT_DENIED_EXACT: &[&str] = &["/etc/master.passwd"];

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
const DEFAULT_DENIED_EXACT: &[&str] = &[];

// ---------------------------------------------------------------------------
// Public config type
// ---------------------------------------------------------------------------

/// Security profile controlling how strict path and command validation is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecurityProfile {
    /// Standard sandbox: deny-lists, write boundaries, dangerous command checks.
    #[default]
    Default,
    /// Unrestricted: all path and command gates are disabled.
    /// For system-administration projects that need full filesystem access.
    Root,
}

/// Security configuration for path validation.
#[derive(Debug, Clone)]
pub struct PathSecurityConfig {
    /// Security profile: `Default` (sandboxed) or `Root` (unrestricted).
    pub profile: SecurityProfile,
    /// Additional directories where writes are allowed (beyond project root).
    pub extra_write_roots: Vec<PathBuf>,
    /// Shell command mode: "unrestricted", "warn" (default), "disabled"
    pub shell_command_mode: String,
    /// Enable file write tools (default: true)
    pub file_write_enabled: bool,
    /// Enable semantic search and indexing tools (default: true)
    pub indexing_enabled: bool,
    /// Read-only library paths (registered via LibraryRegistry).
    pub library_paths: Vec<PathBuf>,
    /// Additional regex patterns to flag as dangerous commands.
    pub shell_dangerous_patterns: Vec<String>,
    /// Approx raw source-byte threshold above which `index(action='build')` requires confirmation.
    pub max_index_bytes: u64,
    /// Why writes are blocked, when they are — so the refusal can STATE a cause
    /// instead of guessing between two, and can name the project it is about.
    ///
    /// `None` when writes are enabled, and also when the config was built by a
    /// path that has no project root to name (`SecuritySection::to_path_security_config`,
    /// `Default`): the refusal then falls back to its original wording rather
    /// than asserting a cause it does not know.
    /// docs/issues/2026-08-26-workspace-read-only-flips-mid-session.md
    pub write_block: Option<WriteBlock>,
}

/// Why file writes are refused, and for which project.
///
/// Exists because one message served two unrelated causes with opposite
/// remedies, and hedged between them: *"If this project was activated in
/// read-only mode…"*. A refusal that guesses at its own cause reads as
/// speculative, so the natural response is to doubt the path or the tool rather
/// than the workspace state — which is exactly what happened across four
/// occurrences before anyone suspected the activation.
///
/// `root` is the load-bearing half. The failure mode this was built for is a
/// caller's `default_workspace_root` being reassigned out from under it by
/// something else sharing the process, so the single most useful fact is WHICH
/// project answered: a session that believes it is working in `codescout` and
/// reads `writes are disabled for /home/…/some-other-repo` has its diagnosis in
/// one line, with no `workspace(action="status")` round trip.
#[derive(Debug, Clone)]
pub struct WriteBlock {
    /// The project the refusal is about — not necessarily the one the caller
    /// thinks is active.
    pub root: PathBuf,
    pub cause: WriteBlockCause,
}

/// The two ways writes end up off. They need different remedies, which is the
/// whole reason for distinguishing them: suggesting `read_only: false` to
/// someone whose project config disables writes is wrong advice that costs a
/// call and teaches the wrong lesson.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteBlockCause {
    /// `security.file_write_enabled = false` in project config. Durable, and
    /// re-activating writable does NOT clear it.
    ConfiguredOff,
    /// The workspace was activated read-only — the state `activate` assigns to
    /// any non-home root that did not explicitly ask for writes.
    ActivatedReadOnly,
}

impl WriteBlockCause {
    /// The precedence rule, as a pure function.
    ///
    /// Extracted deliberately: inside `project_security_config` it would need a
    /// fully-built `ActiveProject` to exercise, and an untested precedence here
    /// does not fail loudly — it ships confident, actionable, WRONG advice
    /// ("re-activate writable") to someone whose project config turns writes
    /// off, where that call will succeed and change nothing.
    ///
    /// Config wins over activation state because their remedies differ and only
    /// one of them is durable.
    pub fn classify(config_allows_writes: bool, read_only: bool) -> Option<Self> {
        if !config_allows_writes {
            Some(Self::ConfiguredOff)
        } else if read_only {
            Some(Self::ActivatedReadOnly)
        } else {
            None
        }
    }
}

impl Default for PathSecurityConfig {
    fn default() -> Self {
        Self {
            profile: SecurityProfile::Default,
            extra_write_roots: Vec::new(),
            shell_command_mode: "warn".into(),
            file_write_enabled: true,
            indexing_enabled: true,
            library_paths: Vec::new(),
            shell_dangerous_patterns: Vec::new(),
            max_index_bytes: 500 * 1024 * 1024,
            write_block: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn home_dir() -> Option<PathBuf> {
    crate::platform::home_dir()
}

/// Expand a leading `~` to `$HOME`.
fn expand_home(pattern: &str) -> Option<PathBuf> {
    if let Some(rest) = pattern.strip_prefix("~/") {
        home_dir().map(|h| h.join(rest))
    } else if pattern == "~" {
        home_dir()
    } else {
        Some(PathBuf::from(pattern))
    }
}

/// Build the full list of denied read paths (defaults + user-configured).
///
/// Each entry is canonicalized once here so that a `$HOME` symlink (e.g.
/// `/home/user -> /var/users/user` on some macOS FileVault / NFS-mounted
/// setups) cannot bypass the deny-list. Input paths get canonicalized by
/// `validate_read_path`; without canonicalizing the deny-list too, the
/// `starts_with` check compares a resolved input against an unresolved
/// prefix and silently passes.
fn denied_read_paths(_config: &PathSecurityConfig) -> Vec<PathBuf> {
    let mut denied = Vec::new();
    for p in crate::platform::denied_read_prefixes()
        .iter()
        .chain(DEFAULT_DENIED_EXACT.iter())
    {
        if let Some(expanded) = expand_home(p) {
            denied.push(best_effort_canonicalize(&expanded));
        }
    }
    // Windows-specific system paths
    #[cfg(windows)]
    {
        if let Ok(sysroot) = std::env::var("SYSTEMROOT") {
            let p = PathBuf::from(&sysroot).join("System32").join("config");
            denied.push(best_effort_canonicalize(&p));
        }
    }
    denied
}

/// Check if `resolved` falls under any denied path.
fn is_denied(resolved: &Path, denied: &[PathBuf]) -> bool {
    // On Windows, `fs::canonicalize` yields extended-length (`\\?\C:\...`) paths
    // while a not-yet-existing input stays plain. `Path::starts_with` is
    // component-wise and treats `\\?\` as a distinct leading component, so a plain
    // input never `starts_with` a verbatim deny prefix even when they denote the
    // same location — a silent deny-list bypass. Normalise both sides to the same
    // form before comparing. No-op off Windows and on already-plain paths.
    fn normalize(p: &Path) -> PathBuf {
        #[cfg(windows)]
        {
            if let Some(s) = p.to_str() {
                if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
                    return PathBuf::from(format!(r"\\{rest}"));
                }
                if let Some(rest) = s.strip_prefix(r"\\?\") {
                    return PathBuf::from(rest);
                }
            }
        }
        p.to_path_buf()
    }
    let resolved = normalize(resolved);
    denied.iter().any(|d| {
        let d = normalize(d);
        resolved.starts_with(&d) || resolved == d
    })
}

/// Best-effort canonicalization: use `fs::canonicalize` when the path exists
/// and is accessible, otherwise return the path as-is.
///
/// This deliberately swallows all errors (not just NotFound) because it's used
/// for write targets that may not exist yet and for paths where the user may
/// lack read permission on intermediate directories.
fn best_effort_canonicalize(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Canonicalize a write target: the parent must exist (or be canonicalized
/// best-effort), then append the file name.
fn canonicalize_write_target(path: &Path) -> PathBuf {
    if let Some(parent) = path.parent() {
        let canon_parent = best_effort_canonicalize(parent);
        if let Some(name) = path.file_name() {
            return canon_parent.join(name);
        }
    }
    best_effort_canonicalize(path)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Validate a path for **read** access.
///
/// - Relative paths are resolved against `project_root` (if available).
/// - Absolute paths are used as-is.
/// - The resolved path is checked against the deny-list (unless `Root` profile).
/// - Library paths are subject to the same deny-list as all other reads.
pub fn validate_read_path(
    raw: &str,
    project_root: Option<&Path>,
    config: &PathSecurityConfig,
) -> Result<PathBuf> {
    if raw.is_empty() {
        bail!("path must not be empty");
    }
    if raw.contains('\0') {
        bail!("path contains null byte");
    }

    if config.profile == SecurityProfile::Root {
        let path = Path::new(raw);
        let resolved = if path.is_absolute() {
            PathBuf::from(raw)
        } else if let Some(root) = project_root {
            root.join(raw)
        } else {
            bail!("relative path '{}' requires an active project", raw);
        };
        return Ok(best_effort_canonicalize(&resolved));
    }

    let path = Path::new(raw);
    let resolved = if path.is_absolute() {
        PathBuf::from(raw)
    } else if let Some(root) = project_root {
        root.join(raw)
    } else {
        bail!("relative path '{}' requires an active project", raw);
    };

    // Canonicalize to resolve symlinks and `..` components.
    let resolved = best_effort_canonicalize(&resolved);

    let denied = denied_read_paths(config);
    if is_denied(&resolved, &denied) {
        bail!("access denied: '{}' is in a protected location", raw);
    }

    Ok(resolved)
}

/// Outcome of classifying a write target against the project's write policy.
///
/// `OutsideRoot` is the one *approvable* failure — it can be turned into a
/// pending-ack handle. `Denied` covers the hard failures (empty / null byte /
/// unresolved `..` / deny-listed location) that must never be approved.
#[derive(Debug)]
pub enum WritePathDecision {
    Allowed(PathBuf),
    OutsideRoot { resolved: PathBuf },
    Denied(String),
}

/// Classify a write target without committing to an error type. The pure core
/// of `validate_write_path`; lets the ack layer distinguish the approvable
/// outside-root case from hard denials without matching on bail strings.
pub fn classify_write_path(
    raw: &str,
    project_root: &Path,
    config: &PathSecurityConfig,
    session_roots: &[PathBuf],
) -> WritePathDecision {
    if raw.is_empty() {
        return WritePathDecision::Denied("path must not be empty".to_string());
    }
    if raw.contains('\0') {
        return WritePathDecision::Denied("path contains null byte".to_string());
    }

    if config.profile == SecurityProfile::Root {
        let path = Path::new(raw);
        let resolved = if path.is_absolute() {
            PathBuf::from(raw)
        } else {
            project_root.join(raw)
        };
        return WritePathDecision::Allowed(canonicalize_write_target(&resolved));
    }

    let path = Path::new(raw);
    let resolved = if path.is_absolute() {
        PathBuf::from(raw)
    } else {
        project_root.join(raw)
    };
    // For write targets the file may not exist yet, canonicalize via parent.
    let resolved = canonicalize_write_target(&resolved);

    // If canonicalization couldn't resolve `..` components (because an
    // intermediate directory doesn't exist), the path still contains them.
    // `starts_with` is component-wise and would match the project root prefix
    // even though `..` would escape it at the OS level.  Reject early.
    if resolved
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        // Says what will NOT work, deliberately. This message shares the
        // `write_scope_denied` family with the approvable case above, so a reader who has
        // learned "write denied → call approve_write" would otherwise spend a call
        // discovering that the hard denials cannot be approved.
        return WritePathDecision::Denied(format!(
            "write denied: '{}' contains '..' that could not be resolved. \
             approve_write cannot grant this — the path itself is rejected, not its \
             location; pass a path with no unresolved '..' segments.",
            raw
        ));
    }

    let project_root = best_effort_canonicalize(project_root);

    // Check deny-list first (blocks writes to ~/.ssh even if somehow under
    // an extra_write_root).
    let denied = denied_read_paths(config);
    if is_denied(&resolved, &denied) {
        return WritePathDecision::Denied(format!(
            "write denied: '{}' is in a protected location. approve_write cannot grant \
             this — the deny-list is checked first and holds even inside an approved \
             directory.",
            raw
        ));
    }

    // Check that the path is under an allowed root.
    let mut allowed = vec![project_root];
    // System temp directory is always writable — useful for scratch files,
    // intermediate output, and cross-process coordination without polluting
    // the project root.
    allowed.push(crate::platform::temp_dir());
    // CWD at server startup — Claude Code launches MCP servers from the
    // project directory, so this covers the case where an absolute path
    // targets the user's working directory even when --project points
    // elsewhere (e.g. a companion tool project).
    //
    // Guard: skip overly broad roots (`/` and `$HOME`).  If CWD happens to be
    // one of these, adding it as a write root would allow writes anywhere on
    // the filesystem or inside the entire home directory.
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_canon = best_effort_canonicalize(&cwd);
        let is_broad = cwd_canon == Path::new("/") || home_dir().is_some_and(|h| cwd_canon == h);
        if !is_broad {
            allowed.push(cwd_canon);
        }
    }
    for extra in &config.extra_write_roots {
        allowed.push(best_effort_canonicalize(extra));
    }
    for root in session_roots {
        allowed.push(best_effort_canonicalize(root));
    }

    let under_allowed_root = allowed.iter().any(|root| resolved.starts_with(root));
    if !under_allowed_root {
        return WritePathDecision::OutsideRoot { resolved };
    }

    WritePathDecision::Allowed(resolved)
}

/// Validate a path for **write** access.
///
/// - Relative paths are resolved against `project_root`.
/// - The resolved path must be under `project_root` or one of the
///   configured `extra_write_roots`.
/// - The deny-list is also checked (writes to `~/.ssh/` are always blocked).
pub fn validate_write_path(
    raw: &str,
    project_root: &Path,
    config: &PathSecurityConfig,
    session_roots: &[PathBuf],
) -> Result<PathBuf> {
    match classify_write_path(raw, project_root, config, session_roots) {
        WritePathDecision::Allowed(p) => Ok(p),
        // The directory to approve is derivable right here — `OutsideRoot` carries
        // `resolved` — and this arm used to match `{ .. }` and throw it away, printing a
        // literal `'<dir>'` placeholder. Worse, `approve_write('<dir>')` is not a callable
        // shape at all: the tool takes a NAMED `path` parameter, so an agent following the
        // message verbatim earned a second error. `write_ack.rs` derives the same
        // directory the same way when minting an ack handle.
        //
        // Measured 2026-08-15: 26% of write denials are followed by retrying the same
        // denied write — the highest immediate-repeat rate in the corpus. The comparison
        // that makes the mechanism legible is `il3_pipe_to_trimmer`, whose message carries
        // a concrete corrective action and repeats at 3%.
        //
        // See `docs/issues/archive/2026-08-15-write-scope-denial-does-not-name-approve-write.md`.
        WritePathDecision::OutsideRoot { resolved } => {
            let dir = resolved.parent().unwrap_or(resolved.as_path());
            bail!(
                "write denied: '{}' is outside the project root. \
                 Call approve_write(path=\"{}\") first to grant write access for this \
                 session.",
                raw,
                dir.display()
            )
        }
        WritePathDecision::Denied(msg) => bail!("{msg}"),
    }
}

/// Validate a path for **session approval** via the `approve_write` tool.
///
/// Checks:
/// 1. Rejects the filesystem root (`/`) and `$HOME` — too broad.
/// 2. Checks the deny-list — protected paths can never be approved.
///
/// Returns the canonicalized path on success.
pub fn validate_approve_path(
    raw: &str,
    project_root: &Path,
    config: &PathSecurityConfig,
) -> Result<PathBuf> {
    if raw.is_empty() {
        bail!("path must not be empty");
    }
    if raw.contains('\0') {
        bail!("path must not contain null bytes");
    }

    let path = Path::new(raw);
    let resolved = if path.is_absolute() {
        best_effort_canonicalize(path)
    } else {
        best_effort_canonicalize(&project_root.join(raw))
    };

    // Breadth guard: reject / and $HOME
    let is_fs_root = resolved == Path::new("/");
    let is_home = home_dir()
        .map(|h| best_effort_canonicalize(&h) == resolved)
        .unwrap_or(false);
    if is_fs_root || is_home {
        bail!(
            "approve_write: '{}' is too broad — specify a subdirectory",
            resolved.display()
        );
    }

    // Deny-list: protected paths can never be approved
    let denied = denied_read_paths(config);
    if is_denied(&resolved, &denied) {
        bail!(
            "approve_write: '{}' is in a protected location and cannot be approved",
            resolved.display()
        );
    }

    Ok(resolved)
}

/// List the root paths of all linked git worktrees for `project_root`.
///
/// Reads `.git/worktrees/<name>/gitdir` files, which contain absolute paths
/// like `/path/to/worktree/.git`. Returns the parent (the worktree root).
/// Returns an empty vec if no worktrees exist (the common case).
pub fn list_git_worktrees(project_root: &Path) -> Vec<PathBuf> {
    let worktrees_dir = project_root.join(".git").join("worktrees");
    if !worktrees_dir.is_dir() {
        return vec![];
    }
    let entries = match std::fs::read_dir(&worktrees_dir) {
        Ok(e) => e,
        Err(_) => return vec![],
    };
    let mut paths = Vec::new();
    for entry in entries.flatten() {
        let gitdir_file = entry.path().join("gitdir");
        if let Ok(content) = std::fs::read_to_string(&gitdir_file) {
            let raw = content.trim();
            // Reject null bytes
            if raw.contains('\0') {
                tracing::warn!(
                    "worktree gitdir contains null byte, skipping: {:?}",
                    gitdir_file
                );
                continue;
            }
            let worktree_git = PathBuf::from(raw);
            // Must be absolute
            if !worktree_git.is_absolute() {
                tracing::warn!("worktree gitdir is not absolute, skipping: {:?}", raw);
                continue;
            }
            if let Some(worktree_root) = worktree_git.parent() {
                paths.push(worktree_root.to_path_buf());
            }
        }
    }
    paths
}

/// True iff `root` is a *linked* git worktree (created by `git worktree add`),
/// as opposed to a main checkout, a submodule, or a non-git directory.
///
/// Filesystem-only (no `git` subprocess): a linked worktree's `.git` is a
/// *file* containing `gitdir: <main>/.git/worktrees/<name>`. A submodule's
/// `.git` file points into `.git/modules/<name>` instead, so we require a
/// `worktrees` path component — skipping a submodule root would be wrong.
///
/// Lives here, beside [`list_git_worktrees`], so worktree *detection* and
/// worktree *enumeration* share a home and neither depends on the optional
/// `librarian` feature. `librarian::current_project` re-exports it.
pub fn is_linked_worktree(root: &Path) -> bool {
    let dot_git = root.join(".git");
    let Ok(meta) = std::fs::symlink_metadata(&dot_git) else {
        return false;
    };
    if !meta.file_type().is_file() {
        return false;
    }
    let Ok(pointer) = std::fs::read_to_string(&dot_git) else {
        return false;
    };
    pointer
        .lines()
        .find_map(|l| l.strip_prefix("gitdir:").map(str::trim))
        .map(|gitdir| {
            Path::new(gitdir)
                .components()
                .any(|c| c.as_os_str() == "worktrees")
        })
        .unwrap_or(false)
}

/// Given a linked-worktree root, derives its MAIN repo root from the
/// `.git`-file `gitdir: <main>/.git/worktrees/<name>` pointer — the same file
/// [`is_linked_worktree`] reads. Filesystem-only (no `git` subprocess).
///
/// Returns `None` if `root` has no readable `.git` file, or if the pointer's
/// `gitdir:` path has no `.git` path component to split the main root on
/// (i.e. `root` is not actually a linked worktree).
pub fn worktree_main_root(root: &Path) -> Option<PathBuf> {
    let pointer = std::fs::read_to_string(root.join(".git")).ok()?;
    let gitdir = pointer
        .lines()
        .find_map(|l| l.strip_prefix("gitdir:").map(str::trim))?;
    let mut main = PathBuf::new();
    for component in Path::new(gitdir).components() {
        if component.as_os_str() == ".git" {
            return Some(main);
        }
        main.push(component);
    }
    None
}

// ---------------------------------------------------------------------------
// Tool access controls
// ---------------------------------------------------------------------------

/// Check if a tool is allowed by the current security configuration.
/// Returns Ok(()) if allowed, or an error message explaining how to enable it.
pub fn check_tool_access(tool_name: &str, config: &PathSecurityConfig) -> Result<()> {
    match tool_name {
        "approve_write" | "create_file" | "edit_file" | "edit_code" | "library"
        | "edit_markdown"
            if !config.file_write_enabled =>
        {
            // State the cause when it is known. The hedged single message this
            // replaces named one of two possible causes with an "if", and the
            // reader could not tell which applied without a second call.
            match &config.write_block {
                Some(WriteBlock {
                    root,
                    cause: WriteBlockCause::ActivatedReadOnly,
                }) => bail!(
                    "File writes are disabled: the active project is {} and it was activated \
                     read-only. To write somewhere else without disturbing it, pass \
                     workspace='<absolute path of the project you meant>' on this call — every \
                     mutating tool takes it, and it resolves the project per-call. To lift the \
                     block here instead, call workspace(action='activate', path='{}', \
                     read_only: false) — but that is process-wide: activation replaces the \
                     default for every caller sharing this process, so if a subagent or another \
                     caller on this session activated it read-only, re-activating flips it under \
                     them mid-task.",
                    root.display(),
                    root.display()
                ),
                Some(WriteBlock {
                    root,
                    cause: WriteBlockCause::ConfiguredOff,
                }) => bail!(
                    "File writes are disabled for {} by security.file_write_enabled = false in \
                     its .codescout/project.toml. Re-activating with read_only: false will NOT \
                     clear this — change the config.",
                    root.display()
                ),
                // No project root was available to the config builder, so say
                // only what is known rather than asserting a cause.
                None => bail!(
                    "File writes are disabled for this project. To write somewhere else without changing anything process-wide, pass workspace='<absolute path of the project you meant>' on this call — every mutating tool takes it. If this project was activated in read-only mode, call workspace(action='activate', read_only: false) to enable writes — but that is process-wide and affects every caller sharing this process. If you didn't expect this, a subagent may have changed the active project — call workspace(action='status') to check."
                ),
            }
        }
        "semantic_search" | "index" if !config.indexing_enabled => {
            bail!(
                    "Indexing tools are disabled. Set security.indexing_enabled = true in .codescout/project.toml to enable."
                );
        }
        _ => {} // All other tools are always allowed
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Dangerous command detection
// ---------------------------------------------------------------------------

/// Default patterns that indicate a dangerous/destructive command.
/// Each entry is (regex_pattern, human-readable description).
const DEFAULT_DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    (
        r"rm\s+(-[a-zA-Z]*f|-[a-zA-Z]*r|--force|--recursive)",
        "rm with --force or --recursive",
    ),
    (r"git\s+push\s+.*--force", "git push --force"),
    (r"git\s+reset\s+--hard", "git reset --hard"),
    (r"git\s+branch\s+-D\b", "git branch -D (force delete)"),
    (
        r"git\s+checkout\s+--\s+\.",
        "git checkout -- . (discard all changes)",
    ),
    (
        r"git\s+clean\s+-[a-zA-Z]*f",
        "git clean -f (remove untracked files)",
    ),
    (r"(?i)DROP\s+(TABLE|DATABASE)", "SQL DROP TABLE/DATABASE"),
    (r"chmod\s+777", "chmod 777 (world-writable)"),
    (r"kill\s+-9", "kill -9 (SIGKILL)"),
    (r"\bmkfs\b", "mkfs (format filesystem)"),
    (r"\bdd\s+if=", "dd (raw disk write)"),
];

/// Shell-normalized form of `command`, for a second pattern-matching pass.
///
/// Tokenizes with [`crate::platform::posix_tokenize`] — the same quote and escape
/// rules `sh -c` and Git Bash `bash -c` apply — and rejoins with single spaces.
/// Returns `None` when the result equals the input (the second pass would be
/// redundant) or when the command cannot be tokenized at all.
///
/// **Why a second pass rather than a replacement.** The raw string catches shapes a
/// token list does not, so matching only the normalized form would LOSE catches.
/// Matching both can only add them, which makes this safe by construction: every
/// command `is_dangerous_command` rejected before, it still rejects.
///
/// **The cost, stated plainly.** Rejoining erases the difference between "two words"
/// and "one quoted word", so `grep 'rm' '-rf' notes.txt` now matches `rm\s+-rf` and
/// gets flagged. That is a false positive, and it is the price of the pass. It is
/// tolerable because a flag is not a refusal — the caller re-invokes with the
/// returned `@ack_*` handle — and because `is_dangerous_command` has never checked
/// command position, so this class already existed via the raw pass
/// (`grep 'rm -rf' notes.txt` was flagged long before normalization, since the raw
/// string literally contains `rm -rf`).
///
/// A NUL-substitution scheme was tried to keep quoted arguments un-bridgeable and
/// then removed: no case could be constructed where it changed the outcome, because
/// for whitespace to sit *inside* a token the quotes must sit outside it, which
/// leaves the dangerous substring intact in the raw string where the raw pass
/// already finds it. Reinstating it needs a demonstrated case, not a plausible one.
///
/// An unclosed quote yields `None`, so only the raw pass runs — unchanged from the
/// previous behaviour. The shell would fail to parse such a command anyway.
fn shell_normalized(command: &str) -> Option<String> {
    let joined = crate::platform::posix_tokenize(command).ok()?.join(" ");
    (joined != command).then_some(joined)
}

/// Tokens of `cmd` under the rules of the shell that will execute it.
///
/// Wraps [`crate::platform::posix_tokenize`] with the one policy every caller on
/// the safety path needs: **when tokenization fails, fall back to
/// `split_whitespace` rather than skipping the check.** An unclosed quote must
/// never be a way to make a helper answer "nothing to see here". The fallback is
/// exactly the model these helpers used before conversion, so anything the old
/// model caught, the new one still catches.
///
/// The fallback is still reachable, but no longer *routinely* so. Until 2026-08-14
/// [`il3_offending_lead`] split a pipeline on a bare `|`, which handed
/// [`stage_trims`] fragments with unbalanced quotes by construction — `grep 'a|b' f`
/// yielded the stage `b' f`. Both splitters are quote-aware now, so unbalanced
/// fragments arrive only when the *user* supplied an unclosed quote, which is the
/// case the fallback exists for. Do not delete it on the strength of that: an
/// unclosed quote must still never let a check be skipped.
///
/// **Unlike [`shell_normalized`], this REPLACES the old token source rather than
/// unioning with it.** The union argument that makes `is_dangerous_command` safe
/// by construction does not transfer: these callers read *head tokens and flags*,
/// so quote-awareness changes which token is the head. Each conversion is a real
/// behaviour change, documented on the helper it applies to.
fn shell_tokens(cmd: &str) -> Vec<String> {
    crate::platform::posix_tokenize(cmd)
        .unwrap_or_else(|_| cmd.split_whitespace().map(str::to_string).collect())
}

/// Check if a command matches a dangerous pattern.
///
/// Returns the matched pattern description if dangerous, `None` if safe.
///
/// Patterns are matched against the raw command **and** its shell-normalized form
/// (see [`shell_normalized`]). Quote and escape tricks — `r''m -rf /`, `rm -r\f /` —
/// leave the raw string unmatchable while the shell still executes the destructive
/// command; the normalized pass reads it the way the shell will.
///
/// The union is deliberate and is what makes the second pass safe by construction:
/// the raw string catches shapes a token list does not, so matching only the
/// normalized form would LOSE catches. Every command this rejected before the
/// normalized pass existed, it still rejects — pinned by
/// `raw_only_matches_are_still_caught`. The sibling helpers in this file cannot use
/// that argument and do not: see [`shell_tokens`].
///
/// **Heredoc bodies are deliberately NOT stripped, unlike in [`detect_il3_violation`] and
/// [`check_source_file_access`].** Those gates analyse *syntax*, where a body is never a
/// pipeline stage or a filename argument, so removing it is sound. This gate asks what will
/// EXECUTE, and a body executes whenever the command consuming it is an interpreter —
/// `bash <<'EOF' … rm -rf / … EOF` runs its body. Adopting the siblings' carve-out here
/// would hide a real deletion behind a fix written for a false positive.
///
/// What the false positive gets instead is a **discriminating reason**: when a pattern
/// matched only inside a body, the description says so and quotes the opener, so
/// acknowledging is a judgement rather than a reflex. The stripped text below decides only
/// *where* a match was, never *whether* there was one.
/// (`docs/issues/archive/2026-08-31-dangerous-command-gate-scans-heredoc-body.md`.)
pub fn is_dangerous_command(command: &str, config: &PathSecurityConfig) -> Option<String> {
    if config.profile == SecurityProfile::Root {
        return None;
    }

    let normalized = shell_normalized(command);
    let mut haystacks: Vec<&str> = vec![command];
    if let Some(n) = normalized.as_deref() {
        haystacks.push(n);
    }

    // The same two haystacks with heredoc bodies removed — the "executable position" view.
    // Consulted ONLY after a match, to describe it. Never in the match decision itself.
    let stripped = strip_heredoc_bodies(command);
    let stripped_norm = shell_normalized(stripped.as_ref());
    let mut executable: Vec<&str> = vec![stripped.as_ref()];
    if let Some(n) = stripped_norm.as_deref() {
        executable.push(n);
    }
    let locate = |re: &Regex| -> String {
        if executable.iter().any(|h| re.is_match(h)) {
            String::new()
        } else {
            heredoc_body_note(command)
        }
    };

    // Check built-in dangerous patterns (cached).
    static DANGEROUS_REGEXES: std::sync::OnceLock<Vec<(Regex, &'static str)>> =
        std::sync::OnceLock::new();
    let cached = DANGEROUS_REGEXES.get_or_init(|| {
        DEFAULT_DANGEROUS_PATTERNS
            .iter()
            .filter_map(|(pattern, desc)| Regex::new(pattern).ok().map(|re| (re, *desc)))
            .collect()
    });
    for (re, description) in cached.iter() {
        if haystacks.iter().any(|h| re.is_match(h)) {
            return Some(format!("{description}{}", locate(re)));
        }
    }

    // Check user-configured dangerous patterns.
    for pattern in &config.shell_dangerous_patterns {
        if let Ok(re) = Regex::new(pattern) {
            if haystacks.iter().any(|h| re.is_match(h)) {
                return Some(format!("matches custom pattern: {pattern}{}", locate(&re)));
            }
        }
    }

    None
}

/// The note appended when a dangerous pattern matched ONLY inside a heredoc body.
///
/// Quotes the **opener line verbatim** rather than naming "the consuming command". Naming it
/// would mean a first-token parse, which `env FOO=1 bash <<'EOF'` and `time bash <<'EOF'`
/// both defeat — and a confidently wrong command name is worse here than none, because the
/// entire purpose of the note is to let a reader decide whether the body is inert. The
/// opener line cannot be wrong; it is the text that was there.
fn heredoc_body_note(command: &str) -> String {
    let re = heredoc_opener();
    let openers: Vec<String> = command
        .lines()
        .map(str::trim)
        // `<<<word` is a here-string with no body, excluded exactly as the two heredoc
        // scanners exclude it — a third opinion here would be the divergence
        // `heredoc_opener` was extracted to prevent.
        .filter(|l| !l.contains("<<<") && re.is_match(l))
        .map(|l| {
            let mut head: String = l.chars().take(80).collect();
            if l.chars().count() > 80 {
                head.push('…');
            }
            head
        })
        .collect();

    let opened_by = if openers.is_empty() {
        String::new()
    } else {
        format!(" Opened by: {}.", openers.join(" | "))
    };

    format!(
        " — matched ONLY inside a heredoc body, never in executable position. A body is \
         inert data unless the command consuming it is an interpreter (bash, sh, zsh, ssh, \
         python …), in which case it runs and this flag is real.{opened_by} This gate \
         cannot tell those two apart, so it flags both rather than stripping bodies the way \
         the IL3 and source-file gates safely can. Read the opener, then acknowledge."
    )
}

/// Remove heredoc bodies from a command before IL3 inspects it.
///
/// IL3 analyses the command as text, and a heredoc body is part of that text —
/// so a `git commit -F - <<'EOF' … EOF` whose *message* describes a piped
/// command trips the gate on a pipe the shell never interprets. The body is
/// data, not syntax; `<<'EOF'` is even single-quoted, so not so much as
/// parameter expansion applies to it.
///
/// Both the body and its terminator line are dropped. `<<<` (here-string) takes
/// no body and is left alone.
fn strip_heredoc_bodies(command: &str) -> std::borrow::Cow<'_, str> {
    if !command.contains("<<") {
        return std::borrow::Cow::Borrowed(command);
    }
    let re = heredoc_opener();

    let mut out = String::with_capacity(command.len());
    let mut awaiting: Option<String> = None;
    for line in command.lines() {
        if let Some(delim) = awaiting.as_deref() {
            // A heredoc terminator is the delimiter alone on its line; `<<-` allows
            // leading tabs, and trimming covers both forms.
            if line.trim() == delim {
                awaiting = None;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
        // `<<<word` would otherwise match the opener pattern starting one byte in.
        if !line.contains("<<<") {
            if let Some(c) = re.captures(line) {
                awaiting = c
                    .get(1)
                    .or_else(|| c.get(2))
                    .or_else(|| c.get(3))
                    .map(|m| m.as_str().to_string());
            }
        }
    }
    std::borrow::Cow::Owned(out)
}

/// The heredoc opener pattern, shared by [`strip_heredoc_bodies`] and
/// [`mask_heredoc_bodies`].
///
/// Extracted rather than copied: this file has already paid for a rule that "existed
/// TWICE and the copies had already diverged" — one recognised `../` and the other did
/// not (`dbaeb78b`). Two heredoc scanners disagreeing about what opens a body would be
/// the same defect, and the two callers here are a gate and a rewrite, so a disagreement
/// would mean the analysis and the transform seeing different commands.
fn heredoc_opener() -> &'static Regex {
    static HEREDOC_OPEN: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    HEREDOC_OPEN.get_or_init(|| {
        Regex::new(r#"<<-?\s*(?:'([A-Za-z_][A-Za-z0-9_]*)'|"([A-Za-z_][A-Za-z0-9_]*)"|([A-Za-z_][A-Za-z0-9_]*))"#)
            .expect("HEREDOC_OPEN regex compiles")
    })
}

/// Blank out heredoc bodies **in place**, preserving every byte offset.
///
/// The sibling of [`strip_heredoc_bodies`], and the difference is the entire reason it
/// exists: that one *removes* body lines, which serves a yes/no gate perfectly and is
/// useless to a caller that must then splice at an offset the scan returned. Removing
/// bytes moves every later index. Replacing each body byte with a space keeps `len()`
/// and every index identical, so an offset found in the masked string addresses the same
/// character in the original.
///
/// That is what `run_command`'s tee instrumentation needs. It asks
/// `detect_terminal_filter` for the byte offset of the last pipe and splices
/// `| tee '<tmp>' |` there — and `detect_terminal_filter`, though quote-aware, has no
/// notion of a heredoc, so a `|` in a heredoc *body* read as a pipeline stage. The body
/// is data destined for a file, so the rewrite landed in written content: measured
/// 2026-08-19, a documentation line was written as
/// `git log --all -p | git patch-id --stable | tee '/tmp/codescout-unfiltered-hUMfFa' | grep …`,
/// a temp path that will never exist, recorded in a permanent file as an instruction.
/// Exit code 0, no warning. See
/// `docs/issues/archive/2026-08-19-run-command-rewrites-pipes-inside-heredoc-content.md`.
///
/// Masking rather than skipping-on-`<<` (the bug's own cheaper suggestion) keeps
/// instrumentation working where the pipe is real — `cat <<'EOF' | grep x` pipes on the
/// opener line, outside any body.
///
/// Multi-byte characters are replaced by as many spaces as they occupy, not one, or the
/// offsets this function exists to preserve would shift on the first non-ASCII byte in a
/// heredoc.
pub fn mask_heredoc_bodies(command: &str) -> std::borrow::Cow<'_, str> {
    if !command.contains("<<") {
        return std::borrow::Cow::Borrowed(command);
    }
    let re = heredoc_opener();

    let mut out = String::with_capacity(command.len());
    let mut awaiting: Option<String> = None;
    // `split_inclusive` keeps each line's terminator, so a command with no trailing
    // newline is not silently given one — `lines()` plus a pushed `\n` would add a byte
    // and break the offset guarantee at the tail.
    for line in command.split_inclusive('\n') {
        if let Some(delim) = awaiting.as_deref() {
            if line.trim() == delim {
                awaiting = None;
            }
            for ch in line.chars() {
                if ch == '\n' {
                    out.push('\n');
                } else {
                    for _ in 0..ch.len_utf8() {
                        out.push(' ');
                    }
                }
            }
            continue;
        }
        out.push_str(line);
        // `<<<word` is a here-string with no body; it would otherwise match the opener
        // pattern starting one byte in.
        if !line.contains("<<<") {
            if let Some(c) = re.captures(line) {
                awaiting = c
                    .get(1)
                    .or_else(|| c.get(2))
                    .or_else(|| c.get(3))
                    .map(|m| m.as_str().to_string());
            }
        }
    }
    debug_assert_eq!(
        out.len(),
        command.len(),
        "mask_heredoc_bodies must preserve byte offsets"
    );
    std::borrow::Cow::Owned(out)
}

/// The backticked text in a `git commit` message that the shell will EVALUATE, or
/// `None` when there is none.
///
/// `run_command` hands its input to `sh -c`, so a commit message written in this
/// project's house style — symbols and paths in backticks — has those backticks run as
/// command substitution before `git` ever sees them. When the backticked text is not a
/// command the shell says so and [`crate::tools::run_command::output`]'s substitution
/// diagnostic names the cause. When it *is* a command it runs, its stdout is spliced
/// into the message, and nothing reports anything. That silent half is what this gate
/// exists for, and it is not hypothetical: `da5176d5`'s committed message permanently
/// reads `per memory  §` because `` `conventions` `` was executed, matched no command,
/// and substituted empty.
///
/// **Both scoping decisions were settled by measuring `.codescout/usage.db` (9,790
/// `run_command` calls), not argued:**
///
/// - **Heredoc bodies are stripped first.** 283 of the 291 backtick-bearing `git commit`
///   calls put the message in a heredoc, where the shell evaluates nothing. Inspecting
///   the raw string would reject 283 correct commands to catch 2 — the false positive
///   that matters here is exposure, not intent.
/// - **Only the message-flag shape is examined.** Backticks inside a commit message are
///   never intended as substitution (0 of 291), but elsewhere on a command line a
///   backtick may be a deliberate one, so this does not look there.
///
/// A backtick counts as evaluated unless it is inside single quotes or backslash-escaped.
/// Both are protections the shell honours, and the escaped form is already in use in the
/// corpus as the manual workaround — flagging it would punish the fix.
///
/// See `docs/issues/archive/2026-08-16-run-command-backticks-substituted-in-quoted-message.md`.
pub fn commit_message_backtick_hazard(command: &str) -> Option<String> {
    let stripped = strip_heredoc_bodies(command);
    let s: &str = stripped.as_ref();

    // `-m`, `-am`, `-a -m`, `--message`, `--message=`. Bounded repetition keeps a long
    // command from turning this into a scan of the whole string.
    static COMMIT_MESSAGE_FLAG: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = COMMIT_MESSAGE_FLAG.get_or_init(|| {
        Regex::new(r"(?s)\bcommit\b.{0,200}?\s(?:-[A-Za-z]*m|--message)[\s=]")
            .expect("COMMIT_MESSAGE_FLAG regex compiles")
    });
    if !re.is_match(s) {
        return None;
    }

    let mut in_single = false;
    let mut in_double = false;
    let mut it = s.char_indices();
    while let Some((i, c)) = it.next() {
        match c {
            // Inside single quotes a backslash is literal; everywhere else it escapes
            // the next character, a backtick included.
            '\\' if !in_single => {
                it.next();
            }
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '`' if !in_single => {
                let rest = &s[i + c.len_utf8()..];
                let inner = match rest.find('`') {
                    Some(end) => &rest[..end],
                    None => rest,
                };
                return Some(inner.chars().take(40).collect());
            }
            _ => {}
        }
    }
    None
}

/// Split a command into independently-piped segments at top-level `;`, `&&`, `||`.
///
/// Without this, `a; b | head` is analysed as one pipeline whose left-hand side is
/// `a; b` — so a chain that merely *starts* with an unbounded producer is blocked
/// because some later, unrelated segment happens to pipe into a trimmer. A single
/// `|` is a pipe and stays inside its segment; `||` is a boundary.
fn pipeline_segments(command: &str) -> Vec<String> {
    // `&&` and `||` before `;` is irrelevant here (disjoint first bytes), but the
    // ordering contract of `split_outside_quotes` is why multi-char separators are
    // listed first. `2>&1` does not read as `&&`: at the `&`, the remainder is `&1`.
    //
    // A newline is a command separator too, and omitting it was a false NEGATIVE here:
    // a multi-line command collapsed into one segment, so the pipe's real LHS was not
    // the segment's first token and `echo hi\ncargo test | grep FAILED` went undetected.
    // Quote-safe without further work — `split_outside_quotes` tracks quote state across
    // line breaks, and a backslash-newline continuation is consumed by its escape branch.
    // BUG docs/issues/archive/2026-08-17-source-gate-does-not-split-on-newlines.md
    split_outside_quotes(command, &["&&", "||", ";", "\n"])
}

/// Detect Iron Law 3 violation: piping a **live, potentially-unbounded**
/// command's output to a **trimmer** that hides a subset of it
/// (`tail`/`head`/`grep`/`less`/`sed`/`awk`/`sort`/`uniq`/`fmt`).
///
/// IL3 exists because piping destroys the `@cmd_*` buffer: a filtered/truncated
/// slice reaches the agent, but the full output is gone. Re-running just to grep
/// wastes a tool call. Server-side enforcement covers all MCP clients (Claude
/// Code, Copilot, Gemini, …).
///
/// *(The doc comment here used to name `codescout-companion/hooks/il3-deny-hook.sh`
/// as a mirror to keep in sync. That file still exists but is no longer wired:
/// measured 2026-08-27, no `hooks.json` PreToolUse matcher targets `run_command`,
/// and `cargo --version; ls docs | head -3` — which the hook's non-segment-splitting
/// logic would refuse — is allowed end to end. This function is the only live
/// enforcement.)*
///
/// **Pure aggregators on the RHS are allowed** — they collapse output to a
/// bounded summary you cannot reconstruct from a partial view, so piping to them
/// SAVES context rather than hiding it (the opposite of what IL3 guards against):
///
///   - `wc` (any flags) — emits only counts.
///   - counting `grep -c` / `--count` — emits a match count, not the matches.
///
/// **A pipeline that collapses ANYWHERE is allowed whatever follows it** — see
/// [`stage_collapses`]. **Field selectors never trim** — see [`stage_trims`].
///
/// **Bounded LHS is allowed.** The original rule blocked any allowlisted LHS
/// piped to a trimmer; this over-triggered on ad-hoc finite probes
/// (`ls <dir> | head`, `grep <pat> <one-file> | wc`, `cat <file> | grep`) —
/// see `docs/issues/archive/2026-05-18-il3-overtriggers-bounded-lhs.md`. Only LHS
/// shapes known to produce arbitrarily large output are blocked now:
///
///   - **Unbounded prefixes:** `cargo`, `npm`, `pnpm`, `yarn`, `python`,
///     `pytest`, `go`, `mvn`, `gradle`, `git`, `rg`, `fd`. Always block.
///   - **Recursive grep:** `grep` with `-r` / `-R` / `--recursive`. Block.
///   - **Bare find:** `find` without `-maxdepth`. Block.
///   - **Everything else** (`ls`, `cat`, `stat`, `du`, `diff`, `awk`, `sed`,
///     non-recursive `grep`, `find` with `-maxdepth`): allow.
///
/// **Buffer-op pipes are also allowed.** If the pre-pipe segment references a
/// buffer handle (`@cmd_*`, `@bg_*`, `@file_*`, `@tool_*`, `@ack_*`), the LHS
/// is operating on already-captured data — `grep PATTERN @cmd_xxx | sort -u`
/// is fine.
///
/// Returns `Some(hint)` when the command violates IL3; `None` otherwise.
pub fn detect_il3_violation(command: &str) -> Option<String> {
    // Analyse shell *structure*, not the raw string: drop heredoc bodies (data, not
    // syntax) and consider each `;`/`&&`/`||`-separated command on its own. Both
    // were real false positives — a commit message describing a pipe, and a
    // `;`-chain whose first word happened to be `git`.
    let stripped = strip_heredoc_bodies(command);

    // Carry the offending SEGMENT out alongside its lead, and quote the segment
    // rather than the whole `command`, so a multi-statement script is localized.
    // Quoting the whole input rendered a five-line script with a `for` loop as
    // "the thing piped to a log-trimmer" — including a trailing `git branch
    // --contains` with no pipe in it at all, leaving the reader to find the
    // offending pipe themselves. The segment comes from the heredoc-stripped
    // text, so a heredoc's body is elided from the echo; that is the syntax the
    // decision was actually made on.
    let (segment, lead) = pipeline_segments(&stripped)
        .into_iter()
        .find_map(|seg| il3_offending_lead(&seg).map(|lead| (seg, lead)))?;
    let segment = segment.trim();
    let lead = lead.trim();

    Some(format!(
        "IL3 violation — piped `{segment}` to a log-trimmer. BLOCKED.\n\n\
         The @cmd_* buffer system saves context tokens:\n  \
         1. run_command(\"{lead}\")               — full output stored as @cmd_xxx\n  \
         2. grep PATTERN @cmd_xxx                 — query the buffer at any granularity\n  \
                                                    (also: tail -20 @cmd_xxx, head -50 @cmd_xxx)\n\n\
         ⚠ That buffer is CAPPED. When the response carries `unfiltered_truncated: true`\n\
         it holds only a PREFIX, and nothing in the buffer marks where it was cut — so a\n\
         grep over it can report `0` for something present, and a hash of it\n\
         (`git patch-id`, `sha256sum`) is a valid-looking WRONG digest. For whole-input\n\
         work, redirect to a file instead:\n  \
         git show <sha> > /tmp/x.patch && git patch-id --stable < /tmp/x.patch\n\n\
         Bounded LHS (ls, cat, stat, du, diff, awk, sed, non-recursive grep) is allowed,\n\
         as are pure aggregators on the RHS (wc, grep -c). A pipeline that collapses\n\
         ANYWHERE (wc, grep -c, sha256sum, git patch-id) is allowed whatever follows it,\n\
         and field selectors (cut, tr) are 1:1 on records so they never trim.\n\
         Only unbounded LHS (cargo, npm, pytest, rg, fd, grep -r, bare find, ...) piped to a\n\
         trimmer (head, tail, grep, sort, ...) is blocked.\n\
         `git` is unbounded ONLY without an output limiter: `git log -3`,\n\
         `git status --short`, `git show --stat` are bounded and may be piped;\n\
         `--oneline` is not a limiter (it bounds width, not line count).\n\
         Single-line plumbing (rev-parse, patch-id, merge-base, describe) is always bounded.\n\n\
         Rerun the command bare and query the returned @cmd_* buffer."
    ))
}

/// The offending left-hand side of a single command segment, if it violates IL3.
fn il3_offending_lead(segment: &str) -> Option<String> {
    static IL3_BUFFER_REF: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let buf_re = IL3_BUFFER_REF.get_or_init(|| {
        Regex::new(r"@(cmd|bg|file|tool|ack)_[A-Za-z0-9_]+").expect("IL3_BUFFER_REF regex compiles")
    });

    // Quote-aware. A `|` inside a quoted argument is data, not a pipe, and splitting
    // on it fabricates stages the shell will never create — see
    // `il3_allows_a_quoted_pipe_inside_an_argument`.
    //
    // Only `|` is listed, and that is safe *because* `pipeline_segments` already
    // consumed every unquoted `||`. A `||` that survives to here is inside quotes, so
    // `split_outside_quotes` correctly leaves it alone. The invariant is pinned by
    // `il3_does_not_treat_the_rhs_of_a_logical_or_as_a_pipe_stage` rather than left as
    // a comment, because it is held by a caller.
    let stages = split_outside_quotes(segment, &["|"]);
    let (pre_pipe, downstream) = match stages.split_first() {
        Some((first, rest)) if !rest.is_empty() => (first.clone(), rest),
        // No pipe at all — nothing to trim.
        _ => return None,
    };

    // Cheap reject: nothing downstream TRIMS output → never IL3. Pure aggregators
    // (`wc`, counting `grep -c`) collapse output to a summary and do not count.
    if !downstream.iter().any(|s| stage_trims(s)) {
        return None;
    }

    // A collapsing stage anywhere downstream bounds the whole pipeline: the agent
    // receives a count, a hash or one summary line no matter what follows, so no
    // hidden subset reaches it and there is nothing for IL3 to protect. See
    // [`stage_collapses`] for why this is checked over the whole chain rather than
    // per stage.
    if downstream.iter().any(|s| stage_collapses(s)) {
        return None;
    }

    // Allow buffer-ops: pre-pipe segment references an already-captured handle.
    if buf_re.is_match(&pre_pipe) {
        return None;
    }

    if !is_unbounded_lhs(&pre_pipe) {
        return None;
    }

    Some(pre_pipe)
}

/// Does this single pipe stage TRIM output (truncate / filter), as opposed to
/// collapsing it to a bounded summary or reshaping it 1:1?
///
/// Pure aggregators are NOT trimmers: `wc` (emits only counts) and a counting
/// `grep -c` / `--count` (emits a match count) reduce output to a summary you
/// cannot reconstruct from a partial view — piping to them SAVES context, the
/// opposite of the trim IL3 guards against. See [`stage_collapses`], which
/// generalises that judgement to the whole pipeline.
///
/// **Field selectors are NOT trimmers either.** `cut` and `tr` are 1:1 on
/// records — `cut` picks fields *within* every line, `tr` maps characters —
/// so neither can hide a record, which is the information loss IL3 exists to
/// prevent. They sat in this list until 2026-08-27 purely because they appear
/// next to `head`/`tail` in the same mental category ("small text utilities"),
/// and the guard's own name for the class, *log-trimmer*, never described
/// them. Measured over 703 IL3 refusals in 37 `usage.db` files, 12 were a
/// field selector and nothing else.
///
/// `sed`, `awk` and `sort` deliberately STAY, and the line is drawn at
/// *capability*, not at typical use: `sed -n '1,10p'` and `awk 'NR<10'` select
/// records, and `sort -u` drops duplicates. That they are usually called in a
/// 1:1 shape is not something this function can check without parsing two
/// embedded languages, so they keep the conservative classification.
///
/// Tokenizes with [`shell_tokens`], so `'head' -50` is recognised as `head`.
/// This is also the helper that exercises the tokenizer's fallback in normal
/// operation: its caller [`il3_offending_lead`] splits on a bare `|`, so a
/// stage arrives with unbalanced quotes whenever the pipeline had a quoted
/// `|` in it.
fn stage_trims(stage: &str) -> bool {
    let tokens = shell_tokens(stage);
    let head = match tokens.first() {
        Some(h) => h.as_str(),
        None => return false,
    };
    match head {
        // Aggregators — collapse to a bounded summary. Allowed.
        "wc" => false,
        // grep filters (hides non-matches) UNLESS counting, which aggregates.
        "grep" => !grep_is_counting(stage),
        // Truncators / filters — hide a subset of records.
        "tail" | "head" | "less" | "sed" | "awk" | "sort" | "uniq" | "fmt" => true,
        // Anything else (`cut`, `tr`, jq, a custom tool, …) is not a known trimmer.
        _ => false,
    }
}

/// Does this stage collapse an arbitrarily large stream to BOUNDED output?
///
/// The distinction that matters to IL3 is not "does this stage hide records"
/// but "can a hidden subset still reach the agent". Once a stage emits a
/// count, a hash, or a single summary line, nothing downstream of it can
/// re-expand the stream — so a trimmer after it has nothing left to trim, and
/// the agent receives bounded output either way.
///
/// This closes an inconsistency the guard shipped with. `git log | grep -c fix`
/// was allowed (counting grep is not a trim) while `git log | grep fix | wc -l`
/// was blocked — the same single number reaching the agent, one spelling
/// permitted and the other refused. [`stage_trims`] cannot see that, because it
/// judges one stage at a time; only the caller knows the whole pipeline.
///
/// **Note this is stronger than "stop scanning at the collapser"**, which was
/// the shape originally proposed in
/// `docs/issues/archive/2026-08-27-il3-blocks-already-collapsed-pipelines-and-its-remedy-yields-a-wrong-hash.md`.
/// That rule leaves trimmers *upstream* of the collapser counting, and so does
/// not move the bug's own `git show X | cut -f1 | wc -l` example — a
/// discrepancy found by running a classifier against the refusal corpus rather
/// than by re-reading the bug.
///
/// Conservative by construction: an unrecognised stage collapses nothing, so
/// the pipeline keeps whatever verdict [`stage_trims`] gives it.
fn stage_collapses(stage: &str) -> bool {
    let tokens = shell_tokens(stage);
    let head = match tokens.first() {
        Some(h) => h.as_str(),
        None => return false,
    };
    match head {
        // Counts only — never the lines themselves.
        "wc" => true,
        "grep" => grep_is_counting(stage),
        // Whole-input digests: one line regardless of input size.
        "sha1sum" | "sha224sum" | "sha256sum" | "sha384sum" | "sha512sum" | "md5sum" | "b2sum"
        | "cksum" | "sum" => true,
        // `git patch-id` reduces an entire diff to one `<patch-id> <commit>` line.
        // This is the case that motivated the whole rule: see the bug above.
        "git" => tokens.get(1).is_some_and(|sub| sub == "patch-id"),
        _ => false,
    }
}

/// True when a `grep` stage carries a count flag (`-c` / `--count`, including
/// bundled short flags like `-ic`), making it an aggregator rather than a filter.
///
/// Tokenizes with [`shell_tokens`]. This conversion makes the gate *less*
/// restrictive, which is the correct direction: a quoted `-c` used to be
/// invisible here, so a counting grep read as a trimmer and IL3 blocked a pipe
/// it is meant to allow. `grep '-c' p f` now reads as counting.
fn grep_is_counting(stage: &str) -> bool {
    shell_tokens(stage).iter().any(|tok| {
        tok == "--count" || (tok.starts_with('-') && !tok.starts_with("--") && tok.contains('c'))
    })
}

/// Classify an LHS shell command as unbounded (arbitrarily-large output) for
/// IL3 purposes. Conservative: when shape parsing is ambiguous, treat as
/// bounded (allow the pipe) — false negatives cost a buffer dance, false
/// positives cost user friction.
///
/// Tokenizes with [`shell_tokens`], which changes two things. The head is now
/// the head the shell sees, so `'cargo' test` is unbounded (it used to read as
/// the unknown command `'cargo'` and fall through to bounded). And `-maxdepth`
/// is matched as a token rather than as the substring `" -maxdepth "`, so a
/// tab-separated or quoted flag is found. The token form has one pathological
/// false-bounded case the substring form did not — `find . -name -maxdepth`,
/// a file literally named `-maxdepth` — which is accepted: IL3 governs output
/// size, not safety, and the cost is one unbuffered pipe.
fn is_unbounded_lhs(lhs: &str) -> bool {
    let tokens = shell_tokens(lhs);
    let head = match tokens.first() {
        Some(h) => h.as_str(),
        None => return false,
    };

    // Always-unbounded executables: project-scale tools, package managers,
    // language runtimes, fast recursive searchers (rg/fd default-recurse).
    const UNBOUNDED_PREFIXES: &[&str] = &[
        "cargo", "npm", "pnpm", "yarn", "python", "python3", "pytest", "go", "mvn", "gradle", "rg",
        "fd",
    ];
    if UNBOUNDED_PREFIXES.contains(&head) {
        return true;
    }

    // grep is bounded by its file args unless promoted to recursive.
    if head == "grep" {
        return has_recursive_flag(lhs);
    }

    // find defaults to recursive; -maxdepth bounds it.
    if head == "find" {
        return !tokens
            .iter()
            .any(|tok| tok == "-maxdepth" || tok.starts_with("-maxdepth="));
    }

    // git defaults to unbounded — `log`, `diff` and `show` can each emit an
    // entire history — but half of its real piped uses name an explicit limit.
    // See [`git_output_is_bounded`] for the token set and why `--oneline` is
    // not in it.
    if head == "git" {
        return !git_output_is_bounded(&tokens);
    }

    false
}

/// True if the command line carries a `-r` / `-R` / `--recursive` flag as a
/// standalone token (avoids matching `-rich` or paths containing `-r`).
///
/// Tokenizes with [`shell_tokens`], so a quoted flag counts. `grep '-r' p .` is
/// recursive to the shell that runs it; before the conversion the token was
/// `'-r'` with the quotes attached, matched nothing, and quoting was a way to
/// hide a recursive grep from the IL3 unbounded check.
fn has_recursive_flag(cmd: &str) -> bool {
    shell_tokens(cmd)
        .iter()
        .any(|tok| tok == "-r" || tok == "-R" || tok == "--recursive")
}

/// True if this `git` subcommand emits O(1) lines *by construction*, so no
/// output-limiter flag exists for it to carry.
///
/// [`git_output_is_bounded`] asks whether the command line names a limiter, and
/// its whole vocabulary — `-n`, `--max-count`, `-3`, `--show-current`,
/// `--porcelain`, `--stat` — is `git log` / `git status` / `git diff` grammar.
/// The plumbing subcommands below have no such flag because they have nothing
/// to limit: `git rev-parse HEAD` prints one object id and stops. Judged purely
/// on the limiter test they are permanently unbounded, which is how
/// `git rev-parse HEAD | head -1` — 40 characters piped to a trimmer — came to
/// be refused as a context-flooding risk.
///
/// The two subcommands here that DO have an enumerating mode are excluded on
/// that flag rather than dropped from the list, since the single-value spelling
/// is the common one.
fn git_subcommand_is_single_line(tokens: &[String]) -> bool {
    let Some(sub) = tokens.get(1) else {
        return false;
    };
    // Flags are searched from index 2: index 0 is `git`, index 1 the subcommand.
    let has = |names: &[&str]| {
        tokens.iter().skip(2).any(|tok| {
            let flag = tok.split_once('=').map_or(tok.as_str(), |(name, _)| name);
            names.contains(&flag)
        })
    };
    match sub.as_str() {
        // One line, no enumerating mode at all.
        "patch-id" | "merge-base" | "symbolic-ref" | "describe" | "hash-object" => true,
        // `git rev-parse HEAD` is one id; `--all` and friends walk every ref.
        "rev-parse" => !has(&[
            "--all",
            "--branches",
            "--tags",
            "--remotes",
            "--glob",
            "--exclude",
        ]),
        // `git config <name>` reads one value; `--list` dumps the whole file.
        "config" => !has(&["--list", "-l", "--get-all", "--get-regexp"]),
        _ => false,
    }
}

/// True if a `git` command line carries an explicit output limiter, making its
/// output bounded for IL3 purposes.
///
/// `git` sat on `UNBOUNDED_PREFIXES` unconditionally until 2026-08-16. That is
/// right for `git log` and wrong for how agents actually call git: measured over
/// 94 `git` IL3 refusals in one project's `usage.db`, **47 carried one of the
/// tokens below** — half the family, refused against this module's own stated
/// bias that when shape parsing is ambiguous we treat as bounded.
///
/// The cost asymmetry is what licenses being generous here. A false negative
/// (allowing the pipe) cannot flood the transcript, because the trimmer on the
/// right-hand side is what bounds the output — it costs only the queryable
/// `@cmd_*` buffer. A false positive costs a refusal on a command that was fine.
///
/// **Deliberately NOT a limiter: `--oneline`.** It bounds line *width*, not line
/// *count* — `git log --oneline` still emits one line per commit for every
/// commit. It appeared in 25 of those 94 refusals and is the most tempting false
/// entry; `il3_blocks_git_pipe_head_still` (the U-16 case) already pinned that
/// judgement before this function existed.
///
/// Mirrors the `grep` and `find` branches in [`is_unbounded_lhs`]: a head-token
/// match refined by a flag check, rather than a bare name match.
///
/// (GF-1 / GF-2 in `docs/trackers/2026-08-16-iron-law-gate-firing-audit.md`,
/// which also records that this is the *incomplete* half of the fix in
/// `docs/issues/archive/2026-05-18-il3-overtriggers-bounded-lhs.md` — that one
/// split LHS into bounded/unbounded and put `git` wholesale on the wrong side.)
fn git_output_is_bounded(tokens: &[String]) -> bool {
    // Checked first: a subcommand that emits one line by construction carries no
    // limiter flag, so the token scan below can only ever return false for it.
    if git_subcommand_is_single_line(tokens) {
        return true;
    }
    // skip(1): the head is `git` itself; a limiter is always an argument.
    tokens.iter().skip(1).any(|tok| {
        // Compare against the FLAG NAME, not the whole token. git spells a
        // valued long option both ways — `--stat` and `--stat=200`,
        // `--porcelain` and `--porcelain=v1` — and a whole-token equality test
        // matches the bare spelling while silently missing the attached one,
        // which is the same limiter and just as bounded. This subsumes the
        // `--max-count=` prefix test that used to be the only attached form
        // handled; a fourth special case was the alternative.
        let flag = tok.split_once('=').map_or(tok.as_str(), |(name, _)| name);
        matches!(
            flag,
            // an explicit commit/line count
            "-n" | "--max-count"
                // a single value by construction
                | "--show-current"
                // porcelain status — bounded by the working tree, not by history
                | "--porcelain" | "--short" | "-s"
                // a name/stat listing rather than a diff body
                | "--stat" | "--name-only" | "--name-status"
        )
            // git's attached count shorthands: `-3`, `-20`, and `-n5`. Strip
            // `-n` first so `-n5` is not tested as the bare `-<digits>` form
            // and rejected on the `n`; a bare `-n` yields an empty rest, fails
            // here, and is caught by the table above.
            || tok
                .strip_prefix("-n")
                .or_else(|| tok.strip_prefix('-'))
                .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
    })
}

/// Source file extensions that should be accessed via codescout tools,
/// not raw shell commands. Mirrors `crate::ast::detect_language()` minus markdown.
const SOURCE_EXTENSIONS: &str = r"\.(rs|py|ts|tsx|js|cjs|mjs|jsx|go|java|kt|kts|c|cpp|cc|cxx|cs|rb|php|swift|scala|ex|exs|hs|lua|sh|bash)\b";

/// Shell commands whose primary job is reading file CONTENT.
///
/// `wc` was on this list until 2026-08-16 and is deliberately absent now: it
/// emits a measurement OF the content, never the content itself, and codescout
/// ships no tool that returns a line count — so the refusal named an alternative
/// that does not exist. The pipe gate already applies the same reasoning to `wc`
/// as an RHS aggregator. The distinction this list encodes is *content vs a
/// measurement of content*, not read-only vs mutating: `head` and `tail` are
/// read-only and stay blocked, because they return the file's bytes.
///
/// (GF-3 in `docs/trackers/2026-08-16-iron-law-gate-firing-audit.md`; 18 of 111
/// measured `il3_shell_on_source` refusals.)
///
/// A list rather than a hand-written alternation, because
/// `get_guide("iron-laws-detail")` documents these names to the agent and drifted
/// from them twice: once claiming a bounded-file carve-out that never existed
/// (B-9), then claiming the gate ignores the command entirely — which outlived
/// `wc`'s removal by a day and told the agent `wc` and `ls` were blocked when
/// neither was. `iron_laws_detail_gate_names_every_blocked_command` now derives
/// the guide's list from this one, so the next edit here fails the build until
/// the guide follows.
pub(crate) const SOURCE_ACCESS_COMMANDS: &[&str] =
    &["cat", "head", "tail", "sed", "awk", "less", "more", "grep"];

/// Split `s` on any separator in `seps` that appears *outside* single- or
/// double-quoted strings. Separators are checked in order — put longer
/// multi-char separators (e.g. `"&&"`) before their prefix (e.g. `"|"`) to
/// avoid a prefix match stealing the first character.
///
/// Backslash escaping outside single quotes is respected (`\"` does not close
/// a double-quoted string). Unclosed quotes are treated as closed at end-of-string.
/// Empty segments are silently dropped.
fn split_outside_quotes(s: &str, seps: &[&str]) -> Vec<String> {
    let mut segments: Vec<String> = Vec::new();
    let mut seg_start = 0usize; // byte offset of current segment start
    let mut in_single = false;
    let mut in_double = false;
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let mut i = 0usize;

    'outer: while i < chars.len() {
        let (byte_pos, c) = chars[i];

        // Backslash: skip next char (escape) — only outside single quotes.
        if c == '\\' && !in_single {
            i += 2;
            continue;
        }

        // Toggle quote state.
        if c == '\'' && !in_double {
            in_single = !in_single;
            i += 1;
            continue;
        }
        if c == '"' && !in_single {
            in_double = !in_double;
            i += 1;
            continue;
        }

        // Outside quotes: check separators in order.
        if !in_single && !in_double {
            let remaining = &s[byte_pos..];
            for sep in seps {
                if remaining.starts_with(sep) {
                    let seg = s[seg_start..byte_pos].trim();
                    if !seg.is_empty() {
                        segments.push(seg.to_string());
                    }
                    let sep_char_count = sep.chars().count();
                    i += sep_char_count;
                    seg_start = chars.get(i).map(|(b, _)| *b).unwrap_or(s.len());
                    continue 'outer;
                }
            }
        }

        i += 1;
    }

    // Remaining segment after the last separator.
    let last = s[seg_start..].trim();
    if !last.is_empty() {
        segments.push(last.to_string());
    }

    segments
}

/// Extracts the pattern argument from a grep shell segment.
/// Skips the command name, any flag tokens (starting with `-`), and numeric
/// arguments that immediately follow value-taking flags like `-A`, `-B`, `-C`, `-m`.
///
/// Tokenizing with [`shell_tokens`] is what removes the quotes now. The
/// hand-rolled `trim_matches('"').trim_matches('\'')` this used to end with
/// stripped one character off each end of the first whitespace-delimited word,
/// so `grep "foo bar" f` yielded `foo` — half a pattern, and the half that
/// decides whether the caller is offered the symbol ladder or the generic hint.
///
/// Returns an owned `String` because the tokenizer produces owned tokens; the
/// borrow into `segment` is no longer available.
fn extract_grep_pattern(segment: &str) -> Option<String> {
    let mut skip_next = false;
    for token in shell_tokens(segment).into_iter().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }
        if token.starts_with('-') {
            // Short value-taking flags: -A, -B, -C, -m (numeric context/count args)
            let flag = token.trim_start_matches('-');
            if matches!(flag, "A" | "B" | "C" | "m") {
                skip_next = true;
            }
            continue;
        }
        return Some(token);
    }
    None
}

/// Returns a hint string if `command` is a file-reading tool targeting a source file,
/// `None` if the command is safe to execute.
///
/// Two-part heuristic: both a blocked command name AND a source file extension must be
/// present in the command string. Use codescout tools instead:
/// - `read_file`, `symbols` for reading
/// - `grep` for regex extraction
///
/// The command name is matched against the segment's first token as the *shell* sees
/// it ([`shell_tokens`]). This closes a bypass: `'cat' src/main.rs` used to yield the
/// first token `'cat'` — quotes attached — which matched no blocked command name, so
/// the block was skipped while the shell happily ran `cat`. The same applies to
/// `\cat` and `c"at"`. The extension half still scans the whole raw segment, on
/// purpose, so quoted paths like `cat "src/main.rs"` stay caught.
///
/// Known limits:
/// - Variable expansion (`cat $FILE`) is undetectable at parse time — accepted.
/// - Heredocs (`cat <<'EOF'`) read stdin, not a file; any source extension appearing
///   inside the heredoc body is not a filename argument. The body is removed by
///   [`strip_heredoc_bodies`] before the segment split, so it cannot contribute
///   either a filename or a segment boundary. Stripping before the split is
///   load-bearing: a `|` inside the body would otherwise cut the body into
///   segments that are each read as a command.
pub fn check_source_file_access(command: &str, project_root: &Path) -> Option<String> {
    static CMD_RE: std::sync::OnceLock<Option<Regex>> = std::sync::OnceLock::new();
    static EXT_RE: std::sync::OnceLock<Option<Regex>> = std::sync::OnceLock::new();
    let cmd_re = CMD_RE
        .get_or_init(|| Regex::new(&format!(r"\b({})\b", SOURCE_ACCESS_COMMANDS.join("|"))).ok())
        .as_ref()?;
    let ext_re = EXT_RE
        .get_or_init(|| Regex::new(SOURCE_EXTENSIONS).ok())
        .as_ref()?;

    // Analyse shell *structure*, not the raw string: a heredoc body is data, so it
    // has to go BEFORE the split. Testing for `<<` per-segment afterwards protects
    // only the segment holding the opener — the body's own pipes have already become
    // segment boundaries, and each following fragment is then read as a command.
    // `detect_il3_violation` has stripped first since the pipe gate's heredoc fix;
    // this gate kept the older approximation. Dropping the body also closes a
    // bypass the per-segment skip created: `cat src/main.rs <<< x` contains `<<`,
    // so the whole segment used to be skipped and the read went through.
    // BUG docs/issues/archive/2026-08-17-heredoc-carve-out-defeated-by-a-pipe-in-the-body.md
    let stripped = strip_heredoc_bodies(command);

    // Split on compound-command operators, pipes and newlines, respecting quoted
    // strings. Order: "&&"/"||" before "|" so that "||" is not mis-split as two "|"
    // tokens; "\n" shares no prefix with the others so its position is free.
    //
    // The newline matters because a segment's command is its FIRST token: without it a
    // multi-line command was one segment, and `echo hi\ncat src/main.rs` read project
    // source unchecked. Quote-safety needs no extra work — `split_outside_quotes` carries
    // quote state across line breaks, so a newline inside "..." is data.
    // BUG docs/issues/archive/2026-08-17-source-gate-does-not-split-on-newlines.md
    // Two-level split, because a `cd` moves the shell for what comes AFTER it and
    // the gate has to know where "relative" now points. Sequential operators bound
    // a *run* — the unit a `cd` can move — and a pipeline inside one run shares a
    // single cwd, since `cd x | cmd` puts the cd in a subshell that cannot affect
    // the other stage. Propagating cwd across a pipe would be a bypass rather than
    // a carve-out.
    // BUG docs/issues/archive/2026-08-17-source-gate-treats-relative-paths-after-cd-as-in-project.md
    let runs = split_outside_quotes(&stripped, &["&&", "||", ";", "\n"]);

    // `run_command` starts at the project root, so that is the cwd until a `cd`
    // says otherwise — which is exactly the old "relative is inside by
    // construction" assumption, now written down as state instead of assumed.
    let mut cwd = Cwd::At(project_root.to_path_buf());
    let mut blocked: Option<String> = None;
    'runs: for run in &runs {
        let stages = split_outside_quotes(run, &["|"]);
        // Only a `cd` that is a whole run moves the shell for later runs.
        if stages.len() == 1 {
            if let Some(next) = cd_effect(&stages[0], &cwd) {
                cwd = next;
                continue;
            }
        }
        for seg in &stages {
            // Only the *first token* of a segment is the actual command being executed.
            // Matching against the first token (not the full segment string) prevents
            // false positives from quoted arguments containing command names, e.g.:
            //   git commit -m "feat: tail-50 of log, output_buffer.rs"
            let first_token = shell_tokens(seg).into_iter().next().unwrap_or_default();
            if !cmd_re.is_match(&first_token) {
                continue;
            }
            // The file must live inside the project, because the hint routes to
            // symbols/read_file and those resolve against the active project.
            if segment_reads_project_source(seg, ext_re, project_root, &cwd) {
                blocked = Some(seg.clone());
                break 'runs;
            }
        }
    }
    let blocked = blocked?;

    // Derive the hint from the specific command that triggered the block.
    let first_cmd = shell_tokens(blocked.as_str())
        .into_iter()
        .next()
        .unwrap_or_default();
    let hint: String = match first_cmd.as_str() {
        "grep" => {
            let pat = extract_grep_pattern(blocked.as_str()).unwrap_or_default();
            if is_identifier_pattern(&pat) {
                let name = pat.split('|').next().unwrap_or(pat.as_str());
                format!(
                    "use symbols(name='{name}') for declarations, \
                     references(symbol='{name}') for direct callers, \
                     call_graph(symbol='{name}', direction='callers') for transitive blast radius. \
                     Re-run with acknowledge_risk: true if you need raw shell grep."
                )
            } else {
                "use grep(pattern, path) codescout tool instead. \
                 Re-run with acknowledge_risk: true if you need raw shell access."
                    .to_string()
            }
        }
        "sed" | "awk" => "use read_file(path, start_line, end_line), symbols(path), \
                 symbols(name=..., include_body=true), or grep(regex) instead. \
                 Re-run with acknowledge_risk: true if you need raw shell access."
            .to_string(),

        _ => "use read_file(path, start_line, end_line) or symbols(path) + \
             symbols(name=..., include_body=true) instead. \
             Re-run with acknowledge_risk: true if you need raw shell access."
            .to_string(),
    };

    Some(hint)
}

/// The shell's working directory for a segment, as far as the gate can tell.
///
/// [`Cwd::At`] is a directory the gate resolved completely; [`Cwd::Unknown`] is a
/// `cd` it could not, and behaves exactly like the pre-`cd`-tracking gate — every
/// relative token counts as in-project. Two variants rather than three because
/// "no `cd` yet" is just `At(project_root)`: that is where `run_command` starts.
enum Cwd {
    At(PathBuf),
    Unknown,
}

/// The cwd a `cd` segment produces, or `None` when `seg` is not a `cd` at all.
///
/// Only a target the gate can resolve completely yields [`Cwd::At`]; anything
/// else yields [`Cwd::Unknown`], which keeps the old blocking verdict. The gate
/// must open only on a move it fully understands — the point is to stop
/// unfollowable refusals, not to widen shell access to project source.
///
/// A bare `cd` and `cd ~` mean `$HOME` and are deliberately left unresolved. The
/// alternative makes this gate's verdict depend on the environment, and the only
/// cost of not resolving them is that a rare command keeps being refused, which
/// is the safe direction.
fn cd_effect(seg: &str, cwd: &Cwd) -> Option<Cwd> {
    let tokens = shell_tokens(seg);
    if tokens.first().map(String::as_str) != Some("cd") {
        return None;
    }
    // Bare `cd` → $HOME. Deliberately unresolved; see the doc comment.
    let Some(target) = tokens.get(1) else {
        return Some(Cwd::Unknown);
    };
    // `~`/`~/…` is $HOME again; `cd -` is the previous directory, which the gate
    // does not track; `$VAR` and command substitution cannot be resolved at parse
    // time at all.
    if target.starts_with('~') || target == "-" || target.contains('$') || target.contains('`') {
        return Some(Cwd::Unknown);
    }
    let path = Path::new(target.as_str());
    if path.is_absolute() {
        return Some(Cwd::At(path.to_path_buf()));
    }
    // A relative target is only as good as the base it joins onto, and `..` would
    // need lexical normalisation before any later `starts_with` could be trusted.
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Some(Cwd::Unknown);
    }
    Some(match cwd {
        Cwd::At(base) => Cwd::At(base.join(path)),
        Cwd::Unknown => Cwd::Unknown,
    })
}

/// True if `seg` names a source file that lives inside `project_root`, given the
/// shell's effective `cwd` for this segment.
///
/// The gate's remedy is *"use symbols / read_file instead"*, and both resolve
/// against the **active project** — they cannot serve a path the index does not
/// cover. Until 2026-08-16 the extension match alone decided, so reading a
/// dependency's source under `~/.cargo/registry`, a sibling repo, or a file in
/// `~/.config` was refused with a suggestion that could not be followed. That is
/// a worse failure than a strict gate: a strict gate at least leaves a correct
/// path open. Measured: **25 of 111** `il3_shell_on_source` refusals in
/// codescout's own `usage.db` named a path outside the project.
///
/// Token-level rather than whole-segment, which is what makes the path check
/// possible at all. [`shell_tokens`] strips quoting, so `cat "src/main.rs"` is
/// still caught — the case the previous whole-segment scan existed to cover.
///
/// (GF-3 in `docs/trackers/2026-08-16-iron-law-gate-firing-audit.md`.)
fn segment_reads_project_source(seg: &str, ext_re: &Regex, project_root: &Path, cwd: &Cwd) -> bool {
    let tokens = shell_tokens(seg);
    // Positive evidence that this segment searches somewhere else entirely: an
    // operand — not an option — that is absolute and does not live under the root.
    let targets_outside = tokens.iter().any(|tok| {
        let p = Path::new(tok.as_str());
        !tok.starts_with('-') && p.is_absolute() && !p.starts_with(project_root)
    });
    tokens.iter().any(|tok| {
        if !ext_re.is_match(tok) {
            return false;
        }
        // An option carrying an extension is a FILTER, not a file operand:
        // nothing is read *from* `--include='*.mjs'`. Being relative, it used to
        // force the in-project verdict on its own and refuse a sweep of a sibling
        // repo. Discount it only when the segment says where it is really
        // searching — absent that evidence the glob is the sole token naming the
        // extension, and `grep -rn x src/ --include='*.rs'` genuinely is a
        // project source read.
        if tok.starts_with('-') && targets_outside {
            return false;
        }
        path_is_within_project(tok, project_root, cwd)
    })
}

/// Whether a path token resolves inside `project_root`, given the shell's
/// effective working directory for the segment.
///
/// Conservative in the blocking direction: anything that cannot be resolved
/// counts as inside, so an unparseable or exotic path keeps the pre-2026-08-16
/// behaviour rather than silently opening the gate.
///
/// `cwd` is what a preceding `cd` did. It starts at `project_root` — which is
/// where `run_command` actually starts — so a relative token is inside by
/// construction until something moves the shell, exactly the old behaviour.
/// [`Cwd::Unknown`] means a `cd` the gate could not resolve, and also keeps the
/// blocking verdict: the gate opens only on a move it fully understands.
///
/// BUG docs/issues/archive/2026-08-17-source-gate-treats-relative-paths-after-cd-as-in-project.md
fn path_is_within_project(tok: &str, project_root: &Path, cwd: &Cwd) -> bool {
    let expanded: PathBuf = match tok.strip_prefix("~/") {
        Some(rest) => match std::env::var_os("HOME") {
            Some(home) => PathBuf::from(home).join(rest),
            // No HOME to expand against — cannot tell, so keep blocking.
            None => return true,
        },
        None => PathBuf::from(tok),
    };
    if expanded.is_relative() {
        let Cwd::At(base) = cwd else {
            return true;
        };
        // A `..` component would need lexical normalisation before `starts_with`
        // could compare it correctly. Refuse to guess and keep blocking.
        if expanded
            .components()
            .any(|c| matches!(c, Component::ParentDir))
        {
            return true;
        }
        return base.join(&expanded).starts_with(project_root);
    }
    expanded.starts_with(project_root)
}

/// Returns true if the path refers to a source code file (by extension).
/// Used to gate `edit_file` multi-line source edits.
pub fn is_source_path(path: &str) -> bool {
    static RE: std::sync::OnceLock<Option<Regex>> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(SOURCE_EXTENSIONS).ok())
        .as_ref()
        .is_some_and(|re| re.is_match(path))
}
/// Returns true if `s` is a plain identifier or pipe-alternation of identifiers.
/// Used to decide whether to suggest symbol tools instead of grep.
pub fn is_identifier_pattern(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.split('|').all(|part| {
        if part.is_empty() {
            return false;
        }
        let mut chars = part.chars();
        match chars.next() {
            Some(c) if c.is_alphabetic() || c == '_' => {}
            _ => return false,
        }
        chars.all(|c| c.is_alphanumeric() || c == '_')
    })
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn default_config() -> PathSecurityConfig {
        PathSecurityConfig::default()
    }
    fn default_session_roots() -> Vec<PathBuf> {
        vec![]
    }

    // ── Read validation ──────────────────────────────────────────────────

    #[test]
    fn read_empty_path_rejected() {
        let result = validate_read_path("", None, &default_config());
        assert!(result.is_err());
    }

    #[test]
    fn read_null_byte_rejected() {
        let result = validate_read_path("hello\0world", None, &default_config());
        assert!(result.is_err());
    }

    #[test]
    fn read_relative_without_project_errors() {
        let result = validate_read_path("src/main.rs", None, &default_config());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires an active project"));
    }

    #[test]
    fn read_relative_with_project_resolves() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("hello.txt");
        std::fs::write(&file, "hi").unwrap();

        let result = validate_read_path("hello.txt", Some(dir.path()), &default_config());
        assert!(result.is_ok());
        assert!(result.unwrap().ends_with("hello.txt"));
    }

    #[test]
    fn read_absolute_outside_project_allowed() {
        // An absolute path to a non-sensitive location should work
        let dir = tempdir().unwrap();
        let file = dir.path().join("readable.txt");
        std::fs::write(&file, "data").unwrap();

        let result = validate_read_path(file.to_str().unwrap(), None, &default_config());
        assert!(result.is_ok());
    }

    #[test]
    fn read_ssh_key_denied() {
        if let Some(home) = home_dir() {
            let ssh_path = home.join(".ssh/id_rsa");
            let result = validate_read_path(ssh_path.to_str().unwrap(), None, &default_config());
            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("protected location"));
        }
    }

    #[test]
    fn read_aws_credentials_denied() {
        if let Some(home) = home_dir() {
            let aws_path = home.join(".aws/credentials");
            let result = validate_read_path(aws_path.to_str().unwrap(), None, &default_config());
            assert!(result.is_err());
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn read_etc_shadow_denied() {
        let result = validate_read_path("/etc/shadow", None, &default_config());
        assert!(result.is_err());
    }

    #[test]
    fn validate_read_path_accepts_library_paths() {
        let dir = tempdir().unwrap();
        let lib_root = dir.path().join("libs/tokio");
        std::fs::create_dir_all(&lib_root).unwrap();
        let lib_file = lib_root.join("src/runtime.rs");
        std::fs::create_dir_all(lib_file.parent().unwrap()).unwrap();
        std::fs::write(&lib_file, "// runtime").unwrap();

        let config = PathSecurityConfig {
            library_paths: vec![lib_root.clone()],
            ..Default::default()
        };
        let result = validate_read_path(
            lib_file.to_str().unwrap(),
            Some(Path::new("/tmp/other_project")),
            &config,
        );
        // Path is not on the deny-list — it happens to be inside a library root,
        // but library roots receive no special exemption from deny-list checks.
        assert!(result.is_ok());
    }

    // ── Write validation ─────────────────────────────────────────────────

    #[test]
    fn write_empty_path_rejected() {
        let dir = tempdir().unwrap();
        let result =
            validate_write_path("", dir.path(), &default_config(), &default_session_roots());
        assert!(result.is_err());
    }

    #[test]
    fn write_null_byte_rejected() {
        let dir = tempdir().unwrap();
        let result = validate_write_path(
            "file\0evil",
            dir.path(),
            &default_config(),
            &default_session_roots(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn write_within_project_allowed() {
        let dir = tempdir().unwrap();
        // Create the target directory so canonicalize resolves properly
        std::fs::create_dir_all(dir.path().join("src")).unwrap();

        let result = validate_write_path(
            "src/new.rs",
            dir.path(),
            &default_config(),
            &default_session_roots(),
        );
        assert!(result.is_ok());
        assert!(result
            .unwrap()
            .starts_with(dir.path().canonicalize().unwrap()));
    }

    #[test]
    fn write_outside_project_rejected() {
        let project = tempdir().unwrap();
        // Use a hardcoded path outside both the project root and /tmp so the
        // test remains valid now that /tmp is an allowed write root.
        let target = "/var/outside_ce_test/evil.rs";

        let result = validate_write_path(
            target,
            project.path(),
            &default_config(),
            &default_session_roots(),
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("outside the project root"));
    }

    #[test]
    fn write_traversal_outside_project_rejected() {
        let project = tempdir().unwrap();
        std::fs::create_dir_all(project.path().join("src")).unwrap();

        // Traverse to /var (not /tmp) so the result lands outside both the
        // project root and the /tmp allowed root.
        let result = validate_write_path(
            "../../../var/evil.rs",
            project.path(),
            &default_config(),
            &default_session_roots(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn write_traversal_via_nonexistent_dir_rejected() {
        // Regression test for: when an intermediate directory does not exist,
        // best_effort_canonicalize falls back to the raw path (with `..`).
        // `starts_with` is component-wise and matches the project root prefix
        // even though `..` would escape it at the OS level.
        //
        // Example: "nonexistent/../../var/evil.rs" with project root /tmp/X
        // canonicalize_write_target: parent = /tmp/X/nonexistent/..
        //   -> canonicalize fails (nonexistent/ does not exist)
        //   -> returns /tmp/X/nonexistent/.. as-is
        //   -> resolved = /tmp/X/nonexistent/../../var/evil.rs
        // starts_with(/tmp/X) is TRUE (prefix matches before .. escapes)
        // Without the ParentDir check this would be allowed.
        let project = tempdir().unwrap();
        // Do NOT create "nonexistent/" — that's the point of this test.
        let result = validate_write_path(
            "nonexistent/../../var/evil.rs",
            project.path(),
            &default_config(),
            &default_session_roots(),
        );
        assert!(
            result.is_err(),
            "traversal via non-existent dir must be rejected"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("'..'"),
            "error should mention '..', got: {msg}"
        );
    }

    #[test]
    fn write_extra_root_allowed() {
        let project = tempdir().unwrap();
        let extra = tempdir().unwrap();
        std::fs::create_dir_all(extra.path().join("sub")).unwrap();

        let config = PathSecurityConfig {
            extra_write_roots: vec![extra.path().to_path_buf()],
            ..Default::default()
        };

        let target = extra.path().join("sub/file.rs");
        let result = validate_write_path(
            target.to_str().unwrap(),
            project.path(),
            &config,
            &default_session_roots(),
        );
        assert!(result.is_ok());
    }

    #[cfg_attr(
        target_os = "windows",
        ignore = "Windows lacks /tmp; absolute Unix-path test inputs need a portable rewrite. See docs/issues/archive/2026-05-24-ci-windows-test-portability-rot.md"
    )]
    #[cfg_attr(
        target_os = "macos",
        ignore = "/tmp → /private/tmp symlink; allowlist comparison happens before canonicalization on macOS, see docs/issues/archive/2026-05-24-ci-macos-tempdir-canonicalization.md"
    )]
    #[test]
    fn write_to_tmp_allowed() {
        let project = tempdir().unwrap();
        // /tmp itself must exist on the system for this test to be meaningful
        let target = PathBuf::from("/tmp/codescout-test-write.txt");
        let result = validate_write_path(
            target.to_str().unwrap(),
            project.path(),
            &default_config(),
            &default_session_roots(),
        );
        assert!(
            result.is_ok(),
            "writes to /tmp should be allowed: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap(), target);
    }

    #[test]
    fn write_within_cwd_allowed_even_outside_project_root() {
        // NOTE: This test changes the process-global CWD via set_current_dir().
        // It could interfere with parallel tests that depend on current_dir().
        // If flaky failures occur, consider adding the serial_test crate and
        // #[serial] attribute.

        // Simulate the case where Claude Code launches the MCP server from
        // a project directory different from --project.  The CWD at server
        // startup should be an additional allowed write root.
        let project = tempdir().unwrap();
        let cwd_project = tempdir().unwrap();
        std::fs::create_dir_all(cwd_project.path().join("src")).unwrap();

        // Temporarily change the process CWD to cwd_project.
        // We use a guard struct to ensure CWD is restored even on panic.
        let original_cwd = std::env::current_dir().unwrap();
        struct CwdGuard(std::path::PathBuf);
        impl Drop for CwdGuard {
            fn drop(&mut self) {
                let _ = std::env::set_current_dir(&self.0);
            }
        }
        let _guard = CwdGuard(original_cwd);
        std::env::set_current_dir(cwd_project.path()).unwrap();

        let target = cwd_project.path().join("src/Routing.kt");
        let result = validate_write_path(
            target.to_str().unwrap(),
            project.path(), // active project root is different
            &default_config(),
            &default_session_roots(),
        );

        assert!(
            result.is_ok(),
            "writes to a path under CWD should be allowed even if outside project root: {:?}",
            result.err()
        );
    }

    #[test]
    fn write_to_ssh_denied_even_if_under_project() {
        // If somehow ~/.ssh were under the project root, it should still be denied
        if let Some(home) = home_dir() {
            let ssh_path = home.join(".ssh/authorized_keys");
            let result = validate_write_path(
                ssh_path.to_str().unwrap(),
                &home, // pretend home is the project root
                &default_config(),
                &default_session_roots(),
            );
            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("protected location"));
        }
    }

    // ── Symlink resolution ───────────────────────────────────────────────

    /// Unix-only, and compiled out elsewhere rather than merely inert.
    ///
    /// Every assertion here lived inside `#[cfg(unix)]`, so on Windows the body
    /// reduced to "look up `$HOME/.ssh`, then stop" — a test that ran, asserted
    /// nothing, and reported `ok`. That is the failure shape this repo already
    /// paid for once with the `server-stack` lane (see `tests/feature_lanes.rs`):
    /// coverage that looks present because a name is in the test list.
    ///
    /// It also explains the `needless_return` that only ever fired on Windows —
    /// with the `#[cfg(unix)]` blocks erased, the early `return` became the last
    /// statement in the function. Gating the whole test fixes the lint by
    /// removing the thing the lint was correctly complaining about.
    #[cfg(unix)]
    #[test]
    fn symlink_to_denied_path_is_caught_on_read() {
        let Some(home) = home_dir() else {
            return;
        };
        let ssh_dir = home.join(".ssh");
        if !ssh_dir.exists() {
            return; // skip if no .ssh directory
        }

        let dir = tempdir().unwrap();
        let link = dir.path().join("sneaky_link");
        std::os::unix::fs::symlink(&ssh_dir, &link).unwrap();

        // Find an actual file inside ~/.ssh to test against.
        // If none exists, test the directory symlink itself.
        let target = std::fs::read_dir(&ssh_dir).ok().and_then(|mut entries| {
            entries.find_map(|e| {
                let e = e.ok()?;
                e.file_type().ok()?.is_file().then(|| e.file_name())
            })
        });
        let test_path = match &target {
            Some(file) => link.join(file),
            None => link.clone(), // test directory itself
        };
        let result = validate_read_path(
            test_path.to_str().unwrap(),
            Some(dir.path()),
            &default_config(),
        );
        // After canonicalization the symlink resolves to ~/.ssh/...
        assert!(
            result.is_err(),
            "symlink to ~/.ssh should be denied, path: {:?}",
            test_path
        );
    }

    #[test]
    fn symlink_write_escape_caught() {
        #[cfg(unix)]
        let project = tempdir().unwrap();

        // Create symlink inside the project pointing to /var/tmp — a real
        // directory that is outside both the project root and /tmp, so the
        // path-security check should still block the write.
        #[cfg(unix)]
        let link = project.path().join("sneaky");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/var/tmp", &link).unwrap();
            let result = validate_write_path(
                "sneaky/escaped.txt",
                project.path(),
                &default_config(),
                &default_session_roots(),
            );
            // After canonicalization the symlink resolves to /var/tmp/escaped.txt
            // which is outside both the project root and /tmp.
            assert!(result.is_err());
        }
    }

    // ── Tool access controls ─────────────────────────────────────────────

    #[test]
    fn file_write_enabled_by_default() {
        let config = PathSecurityConfig::default();
        assert!(config.file_write_enabled);
        assert!(check_tool_access("create_file", &config).is_ok());
        assert!(check_tool_access("edit_code", &config).is_ok());
    }

    #[test]
    fn file_write_disabled_blocks_all_write_tools() {
        let config = PathSecurityConfig {
            file_write_enabled: false,
            ..PathSecurityConfig::default()
        };
        for tool in &["create_file", "edit_file", "edit_code", "library"] {
            assert!(
                check_tool_access(tool, &config).is_err(),
                "{} should be blocked",
                tool
            );
        }
    }

    /// The precedence rule, table-driven over all four inputs.
    ///
    /// The third row is the one that matters. Config-off AND read-only both
    /// hold, and if activation won, the refusal would tell the caller to
    /// re-activate writable — a call that succeeds, changes nothing, and sends
    /// them looking somewhere else. A wrong-but-confident remedy is worse than
    /// the hedged message this replaces.
    #[test]
    fn write_block_cause_precedence_prefers_the_durable_cause() {
        use WriteBlockCause::*;
        let cases = [
            (true, false, None, "writes on, not read-only"),
            (
                true,
                true,
                Some(ActivatedReadOnly),
                "activation is the cause",
            ),
            (
                false,
                true,
                Some(ConfiguredOff),
                "BOTH hold — config must win, its remedy is the only one that works",
            ),
            (false, false, Some(ConfiguredOff), "config alone"),
        ];
        for (allows, ro, want, why) in cases {
            assert_eq!(
                WriteBlockCause::classify(allows, ro),
                want,
                "classify(config_allows_writes={allows}, read_only={ro}): {why}"
            );
        }
    }

    /// The read-only refusal must NAME the project, because the failure it was
    /// built for is the active project having been changed out from under the
    /// caller. A message that says "this project" is uninformative in exactly
    /// the case that matters.
    #[test]
    fn read_only_refusal_names_the_project_and_the_remedy() {
        let config = PathSecurityConfig {
            file_write_enabled: false,
            write_block: Some(WriteBlock {
                root: PathBuf::from("/work/some-other-repo"),
                cause: WriteBlockCause::ActivatedReadOnly,
            }),
            ..PathSecurityConfig::default()
        };
        let err = check_tool_access("edit_file", &config)
            .expect_err("writes are off, this must refuse")
            .to_string();
        assert!(
            err.contains("/work/some-other-repo"),
            "must name the project that actually answered: {err}"
        );
        assert!(
            err.contains("read_only: false"),
            "must carry the remedy that works for this cause: {err}"
        );
        assert!(
            err.contains("sharing this process"),
            "must point at the mechanism, since the caller did not activate it: {err}"
        );
    }

    /// Fix 1 of `docs/issues/2026-09-01-workspace-activation-is-process-wide-and-a-subagent-can-flip-it.md`,
    /// and Phase 5(a) of the per-request-pinning plan: the refusal must offer the
    /// per-call `workspace=` pin BEFORE re-activation.
    ///
    /// **Order is the assertion, deliberately.** Two `contains` checks would pass
    /// with the pin advice appended at the end — and that arrangement is the bug,
    /// not a fix: a caller takes the first remedy that fits, and re-activation is
    /// process-wide, so following it flips the default under whichever peer set
    /// it read-only. `find()` positions are what discriminate; do not relax these
    /// to `contains`.
    #[test]
    fn read_only_refusal_offers_the_per_call_pin_before_reactivation() {
        let config = PathSecurityConfig {
            file_write_enabled: false,
            write_block: Some(WriteBlock {
                root: PathBuf::from("/work/some-other-repo"),
                cause: WriteBlockCause::ActivatedReadOnly,
            }),
            ..PathSecurityConfig::default()
        };
        let err = check_tool_access("edit_file", &config)
            .expect_err("writes are off, this must refuse")
            .to_string();

        let pin = err
            .find("workspace=")
            .unwrap_or_else(|| panic!("must name the per-call pin parameter: {err}"));
        let reactivate = err
            .find("read_only: false")
            .unwrap_or_else(|| panic!("must still carry the re-activation remedy: {err}"));
        assert!(
            pin < reactivate,
            "the non-destructive remedy must come FIRST — a caller takes the first \
             one that fits, and re-activation is process-wide: {err}"
        );
        assert!(
            err.contains("every caller"),
            "must say WHY re-activation is the second choice, not merely that it is: {err}"
        );
    }

    /// The config-off refusal must NOT offer the read-only remedy, and must say
    /// so explicitly — someone who has seen the other message will otherwise try
    /// it first.
    #[test]
    fn configured_off_refusal_rejects_the_reactivation_remedy() {
        let config = PathSecurityConfig {
            file_write_enabled: false,
            write_block: Some(WriteBlock {
                root: PathBuf::from("/work/locked"),
                cause: WriteBlockCause::ConfiguredOff,
            }),
            ..PathSecurityConfig::default()
        };
        let err = check_tool_access("create_file", &config)
            .expect_err("writes are off, this must refuse")
            .to_string();
        assert!(err.contains("/work/locked"), "{err}");
        assert!(
            err.contains("security.file_write_enabled"),
            "must name the setting to change: {err}"
        );
        assert!(
            err.contains("will NOT clear this"),
            "must actively steer away from the wrong remedy: {err}"
        );
    }

    /// A config built by a path with no project root in scope keeps the original
    /// hedged wording. Saying nothing false is the requirement here: the point of
    /// the change is that the refusal stops asserting causes, so a builder that
    /// cannot attribute one must not gain a confident message.
    #[test]
    fn an_unattributed_block_falls_back_to_the_original_wording() {
        let config = PathSecurityConfig {
            file_write_enabled: false,
            ..PathSecurityConfig::default()
        };
        assert!(config.write_block.is_none(), "Default must not invent one");
        let err = check_tool_access("edit_markdown", &config)
            .expect_err("writes are off, this must refuse")
            .to_string();
        assert!(
            err.contains("If this project was activated in read-only mode"),
            "{err}"
        );
    }

    #[test]
    fn library_disabled_when_file_write_false() {
        let config = PathSecurityConfig {
            file_write_enabled: false,
            ..PathSecurityConfig::default()
        };
        assert!(
            check_tool_access("library", &config).is_err(),
            "library should be blocked when file_write_enabled = false"
        );
        let config = PathSecurityConfig {
            file_write_enabled: true,
            ..PathSecurityConfig::default()
        };
        assert!(
            check_tool_access("library", &config).is_ok(),
            "library should be allowed when file_write_enabled = true"
        );
    }

    #[test]
    fn indexing_disabled_blocks_search_tools() {
        let config = PathSecurityConfig {
            indexing_enabled: false,
            ..PathSecurityConfig::default()
        };
        for tool in &["semantic_search", "index"] {
            assert!(
                check_tool_access(tool, &config).is_err(),
                "{} should be blocked",
                tool
            );
        }
    }

    #[test]
    fn read_tools_always_allowed() {
        let config = PathSecurityConfig {
            file_write_enabled: false,
            indexing_enabled: false,
            ..PathSecurityConfig::default()
        };
        // Read tools should always work
        for tool in &[
            "read_file",
            "tree",
            "grep",
            "read_markdown",
            "symbols",
            "onboarding",
            "workspace",
        ] {
            assert!(
                check_tool_access(tool, &config).is_ok(),
                "{} should always be allowed",
                tool
            );
        }
    }

    #[test]
    fn home_dir_returns_some_on_all_platforms() {
        // home_dir() must return Some on every platform we support.
        // On Linux/macOS it reads $HOME, on Windows $USERPROFILE.
        let home = home_dir();
        assert!(
            home.is_some(),
            "home_dir() returned None — deny-list will be empty (security bug)"
        );
    }

    #[test]
    fn file_write_enabled_disabled_blocks_approve_write() {
        let config = PathSecurityConfig {
            file_write_enabled: false,
            ..PathSecurityConfig::default()
        };
        let err = check_tool_access("approve_write", &config).unwrap_err();
        assert!(
            err.to_string().contains("disabled"),
            "should block approve_write when writes disabled: {err}"
        );
    }

    #[test]
    fn file_write_disabled_message_points_to_workspace_status() {
        let config = PathSecurityConfig {
            file_write_enabled: false,
            ..PathSecurityConfig::default()
        };
        let err = check_tool_access("create_file", &config).unwrap_err();
        assert!(
            err.to_string().contains("workspace(action='status')"),
            "read-only refusal should point at workspace(action='status') so a caller can \
             discover the active project may have been changed by a subagent: {err}"
        );
    }

    #[test]
    fn library_paths_default_is_empty() {
        let config = PathSecurityConfig::default();
        assert!(config.library_paths.is_empty());
    }

    #[test]
    fn list_git_worktrees_empty_when_no_git_dir() {
        let dir = tempfile::tempdir().unwrap();
        let result = list_git_worktrees(dir.path());
        assert!(result.is_empty());
    }

    #[test]
    fn list_git_worktrees_finds_linked_worktrees() {
        let dir = tempfile::tempdir().unwrap();
        let wt_root = tempfile::tempdir().unwrap();
        let wt_entry = dir.path().join(".git").join("worktrees").join("feat");
        std::fs::create_dir_all(&wt_entry).unwrap();
        let gitdir_content = format!("{}/.git\n", wt_root.path().display());
        std::fs::write(wt_entry.join("gitdir"), &gitdir_content).unwrap();

        let result = list_git_worktrees(dir.path());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], wt_root.path());
    }

    #[test]
    fn list_git_worktrees_rejects_relative_path() {
        let dir = tempfile::tempdir().unwrap();
        let wt_entry = dir.path().join(".git").join("worktrees").join("evil");
        std::fs::create_dir_all(&wt_entry).unwrap();
        std::fs::write(wt_entry.join("gitdir"), "...etc/.git\n").unwrap();

        let result = list_git_worktrees(dir.path());
        assert!(result.is_empty(), "relative path should be rejected");
    }

    #[test]
    fn list_git_worktrees_rejects_null_byte() {
        let dir = tempfile::tempdir().unwrap();
        let wt_entry = dir.path().join(".git").join("worktrees").join("evil2");
        std::fs::create_dir_all(&wt_entry).unwrap();
        std::fs::write(wt_entry.join("gitdir"), "/tmp/evil\0injected/.git\n").unwrap();

        let result = list_git_worktrees(dir.path());
        assert!(result.is_empty(), "null byte path should be rejected");
    }

    // ── Dangerous command detection ──────────────────────────────────────

    #[test]
    fn dangerous_command_detected() {
        let config = PathSecurityConfig::default();
        assert!(is_dangerous_command("rm -rf /tmp/foo", &config).is_some());
        assert!(is_dangerous_command("git push --force origin main", &config).is_some());
        assert!(is_dangerous_command("git reset --hard", &config).is_some());
        assert!(is_dangerous_command("git branch -D feature", &config).is_some());
        assert!(is_dangerous_command("git clean -fd", &config).is_some());
        assert!(is_dangerous_command("chmod 777 script.sh", &config).is_some());
        assert!(is_dangerous_command("kill -9 1234", &config).is_some());
    }

    #[test]
    fn safe_command_not_flagged() {
        let config = PathSecurityConfig::default();
        assert!(is_dangerous_command("cargo test", &config).is_none());
        assert!(is_dangerous_command("git status", &config).is_none());
        assert!(is_dangerous_command("git push origin main", &config).is_none());
        assert!(is_dangerous_command("rm temp.txt", &config).is_none());
        assert!(is_dangerous_command("npm run build", &config).is_none());
    }

    /// The discriminating cases for matching raw-OR-normalized. Every command here
    /// leaves the RAW string unmatchable by `rm\s+-[a-zA-Z]*f` while the shell still
    /// executes `rm -rf`, because quote and escape characters sit between the letters
    /// the regex needs adjacent.
    ///
    /// A revert to raw-only matching fails these. A switch to normalized-only passes
    /// these but fails `raw_only_matches_are_still_caught` below — which is the point
    /// of asserting both directions: the union is the invariant, not either arm.
    #[test]
    fn dangerous_command_catches_quote_and_escape_evasion() {
        let config = PathSecurityConfig::default();

        for evasion in [
            r"r''m -rf /tmp/x",   // empty single-quote pair splits the word
            r#"r""m -rf /tmp/x"#, // same trick with double quotes
            r"rm -r\f /tmp/x",    // backslash escape inside the flag
            r"'rm' -rf /tmp/x",   // whole command word quoted
            r"git push --force'' origin main",
        ] {
            assert!(
                is_dangerous_command(evasion, &config).is_some(),
                "the shell runs this destructively, so the gate must catch it: {evasion}"
            );
        }
    }

    /// Guards the raw arm of the union. `\s+` in the patterns already tolerates odd
    /// spacing, and normalization collapses it — so if someone later replaces the raw
    /// pass with the normalized one, these must keep passing on their own.
    #[test]
    fn raw_only_matches_are_still_caught() {
        let config = PathSecurityConfig::default();
        for raw in [
            "rm -rf /tmp/x",
            "rm    -rf    /tmp/x",
            "git push --force origin main",
        ] {
            assert!(
                is_dangerous_command(raw, &config).is_some(),
                "must be caught without relying on normalization: {raw}"
            );
        }
    }

    /// A gate that returned `Some` unconditionally would pass every evasion case
    /// above, so the negative control belongs next to them rather than only in
    /// `safe_command_not_flagged`. The quoted case matters specifically: normalization
    /// rewrites the command before matching, so a benign command that merely *contains*
    /// spaces inside quotes must survive it.
    #[test]
    fn normalization_does_not_flag_benign_commands() {
        let config = PathSecurityConfig::default();
        for benign in [
            "echo hello",
            r#"echo "hello world""#,
            "cargo test --all",
            r#"git commit -m "reset the hard way""#,
        ] {
            assert!(
                is_dangerous_command(benign, &config).is_none(),
                "must stay allowed after normalization: {benign}"
            );
        }
    }

    /// The false-positive class, asserted so it is a recorded property rather than a
    /// surprise — and so that a future attempt to remove it has a test to change.
    ///
    /// `is_dangerous_command` has never checked command *position*: it looks for the
    /// pattern anywhere in the string. So a command that merely quotes dangerous text
    /// as data is flagged. The first case predates normalization entirely (the raw
    /// string literally contains `rm -rf`); the second is added BY normalization,
    /// which rejoins `'rm' '-rf'` into `rm -rf`.
    ///
    /// A flag is not a refusal — the caller re-invokes with the returned `@ack_*`
    /// handle — which is why this is an acceptable price for catching quote evasion.
    #[test]
    fn quoted_dangerous_text_is_flagged_and_the_raw_pass_did_it_first() {
        let config = PathSecurityConfig::default();

        // Pre-existing: raw string contains `rm -rf`, quotes or not.
        assert!(
            is_dangerous_command("grep 'rm -rf' notes.txt", &config).is_some(),
            "pre-existing behaviour, unrelated to normalization"
        );

        // Added by normalization: the raw string has `rm' '-rf` (no whitespace
        // directly after `rm`), so only the rejoined form matches.
        assert!(
            is_dangerous_command("grep 'rm' '-rf' notes.txt", &config).is_some(),
            "normalization rejoins the tokens; this is the documented cost"
        );
    }

    /// A match found ONLY inside a heredoc body is still flagged, but says so.
    ///
    /// The sibling gates (`detect_il3_violation`, `check_source_file_access`) strip heredoc
    /// bodies before looking, and it is safe there because they analyse *syntax* — a pipe in
    /// a body is not a pipeline stage, a `.rs` in a body is not a filename argument. This
    /// gate cannot copy that: a heredoc body is inert data only when the command consuming
    /// it is not an interpreter, and `bash <<'EOF' … rm -rf / … EOF` executes its body. So
    /// stripping here would hide a real deletion, which is why the carve-out is absent by
    /// design and pinned by the test above.
    ///
    /// The complaint in
    /// `docs/issues/archive/2026-08-31-dangerous-command-gate-scans-heredoc-body.md` is therefore
    /// answered by making the flag DISCRIMINATING rather than by removing it: the reason
    /// names where the match was, so acknowledging is a judgement instead of a reflex.
    #[test]
    fn a_heredoc_only_match_is_flagged_and_the_reason_says_where_it_matched() {
        let config = PathSecurityConfig::default();
        let command = "cat > notes.txt <<'EOF'\nwe removed it with rm -rf\nEOF\nwc -l notes.txt";

        let reason = is_dangerous_command(command, &config)
            .expect("still flagged — the gate must not go quiet on a body it cannot classify");
        assert!(
            reason.contains("heredoc body"),
            "the reason must locate the match, or the reader learns to acknowledge without \
             reading — the one habit this gate depends on not forming: {reason}"
        );
        assert!(
            reason.contains("cat > notes.txt <<'EOF'"),
            "and it must quote the opener verbatim, which is what lets the reader decide \
             whether the body is data or is about to be executed: {reason}"
        );
    }

    /// Non-vacuity: a real dangerous command must NOT get the heredoc note.
    ///
    /// Without this, the test above is equally satisfied by appending the note to every
    /// reason unconditionally — which would restore exactly the reflex-acknowledgement the
    /// note exists to prevent, while passing.
    #[test]
    fn a_match_in_executable_position_carries_no_heredoc_note() {
        let config = PathSecurityConfig::default();

        let reason = is_dangerous_command("rm -rf /tmp/foo", &config).expect("flagged");
        assert!(
            !reason.contains("heredoc"),
            "nothing about this command is a heredoc; saying so would make the note noise \
             and destroy its signal: {reason}"
        );

        // The same command with an UNRELATED heredoc present: the match is still in
        // executable position, so the note must stay off. This is the case a naive
        // "does the command contain <<" check would get wrong.
        let mixed = "cat > m.txt <<'EOF'\njust a message\nEOF\nrm -rf /tmp/foo";
        let reason = is_dangerous_command(mixed, &config).expect("flagged");
        assert!(
            !reason.contains("heredoc body"),
            "the rm is outside the body — the presence of a heredoc elsewhere must not \
             excuse it: {reason}"
        );
    }

    /// The security property the filed fix would have broken.
    ///
    /// `bash <<'EOF'` EXECUTES its body. Had this gate adopted the sibling gates'
    /// `strip_heredoc_bodies` carve-out, this command would pass unflagged — a real
    /// `rm -rf /` hidden by a fix written for a false positive. It is flagged, and the note
    /// is honest about the ambiguity rather than reassuring.
    #[test]
    fn an_interpreter_heredoc_is_still_flagged_because_its_body_executes() {
        let config = PathSecurityConfig::default();
        let command = "bash <<'EOF'\nrm -rf /tmp/whatever\nEOF";

        let reason = is_dangerous_command(command, &config).expect(
            "a body fed to an interpreter executes — stripping bodies before this gate \
             would hide a real deletion",
        );
        assert!(
            reason.contains("interpreter"),
            "the note must name the case in which a body is NOT data, or it reads as an \
             all-clear on exactly the command that is dangerous: {reason}"
        );
        assert!(
            reason.contains("bash <<'EOF'"),
            "quoting the opener is what makes the interpreter case visible: {reason}"
        );
    }

    /// An unclosed quote cannot be tokenized, so only the raw pass runs. Asserted so
    /// the fallback is a recorded decision rather than incidental behaviour: the shell
    /// would fail to parse such a command anyway, and silently *skipping* the gate on
    /// a tokenizer error would be the dangerous reading of the same situation.
    #[test]
    fn unclosed_quote_still_gets_the_raw_pass() {
        let config = PathSecurityConfig::default();
        assert!(
            is_dangerous_command(r#"rm -rf /tmp/x "unterminated"#, &config).is_some(),
            "raw pass must still fire when normalization is unavailable"
        );
    }

    // ---- The six `split_whitespace` -> `shell_tokens` conversions ----
    //
    // Every assertion marked "was: ..." fails on the pre-conversion code. That is
    // the bar here. Unlike the `is_dangerous_command` union — which could only add
    // catches, so a passing old test proved it safe — these REPLACE the token
    // source, so a test that passes before and after says nothing about the change.

    #[test]
    fn a_quoted_recursive_flag_is_seen() {
        // `grep '-r' p .` runs a recursive grep. The old token was `'-r'` with the
        // quotes attached and matched nothing, so quoting hid an unbounded LHS
        // from IL3.
        assert!(has_recursive_flag("grep '-r' pattern ."), "was: false");
        assert!(has_recursive_flag(r"grep -\r pattern ."), "was: false");

        // Unchanged — bare flags, and the lookalikes the doc comment promises.
        assert!(has_recursive_flag("grep -r pattern ."));
        assert!(has_recursive_flag("cp --recursive a b"));
        assert!(!has_recursive_flag("ls -rich"));
        assert!(!has_recursive_flag("cat some-r-file"));
    }

    #[test]
    fn a_quoted_count_flag_makes_grep_an_aggregator_again() {
        // The one conversion that RELAXES a check, and the relaxation is the
        // correct direction: a counting grep collapses output to a summary, so
        // IL3 is meant to allow the pipe. Quoting used to make it read as a
        // trimmer and blocked a legitimate command.
        assert!(grep_is_counting("grep '-c' pattern file"), "was: false");
        assert!(
            grep_is_counting(r#"grep "--count" pattern file"#),
            "was: false"
        );

        // Unchanged.
        assert!(grep_is_counting("grep -c pattern file"));
        assert!(grep_is_counting("grep -ic pattern file"));
        assert!(!grep_is_counting("grep -i pattern file"));
    }

    #[test]
    fn a_quoted_multiword_grep_pattern_survives_whole() {
        // `trim_matches` stripped one quote character off each end of the first
        // whitespace-delimited word, so this came back as `foo` — half a pattern,
        // and the half that decides whether the caller is handed the symbol ladder
        // (`foo` looks like an identifier) or the generic grep hint.
        assert_eq!(
            extract_grep_pattern(r#"grep "foo bar" src/main.rs"#).as_deref(),
            Some("foo bar"),
            "was: Some(\"foo\")"
        );

        // Unchanged.
        assert_eq!(
            extract_grep_pattern("grep WriteMemory src/tools/memory.rs").as_deref(),
            Some("WriteMemory")
        );
        assert_eq!(
            extract_grep_pattern("grep 'WriteMemory|ReadMemory' src/x.rs").as_deref(),
            Some("WriteMemory|ReadMemory")
        );
        assert_eq!(
            extract_grep_pattern("grep -A 3 WriteMemory src/x.rs").as_deref(),
            Some("WriteMemory")
        );
    }

    #[test]
    fn a_quoted_head_still_names_the_command() {
        // `is_unbounded_lhs` and `stage_trims` both key off the head token, so
        // both inherit the same evasion and the same fix.
        assert!(is_unbounded_lhs("'cargo' test"), "was: false");
        assert!(is_unbounded_lhs(r"\git log"), "was: false");
        assert!(stage_trims("'head' -50"), "was: false");
        assert!(stage_trims(r#""tail" -n 20"#), "was: false");

        // Unchanged.
        assert!(is_unbounded_lhs("cargo test"));
        assert!(!is_unbounded_lhs("ls -la"));
        assert!(stage_trims("head -50"));
        assert!(!stage_trims("wc -l"));
    }

    #[test]
    fn maxdepth_is_matched_as_a_token_not_a_substring() {
        // The substring form needed a literal `" -maxdepth "`; a tab or a quote
        // hid it and the `find` read as unbounded.
        assert!(
            !is_unbounded_lhs("find . '-maxdepth' 2 -name '*.rs'"),
            "was: true"
        );
        assert!(!is_unbounded_lhs("find .\t-maxdepth\t2"), "was: true");

        // Unchanged.
        assert!(!is_unbounded_lhs("find . -maxdepth 2"));
        assert!(!is_unbounded_lhs("find . -maxdepth=2"));
        assert!(is_unbounded_lhs("find . -name '*.rs'"));

        // KNOWN AND ACCEPTED, not missed: a file named `-maxdepth` now reads as
        // the flag. Pinned so the next reader sees a decision rather than a gap.
        // IL3 governs output size, not safety; the cost is one unbuffered pipe.
        assert!(!is_unbounded_lhs("find . -name -maxdepth"));
    }

    #[test]
    fn quoting_the_command_name_no_longer_bypasses_the_source_file_block() {
        // The security-relevant conversion. Each of these reads the file when the
        // shell runs it, and each used to produce a first token the blocked-command
        // regex did not match, so the block was skipped entirely.
        assert!(
            check_source_file_access_at_root("'cat' src/main.rs").is_some(),
            "was: None"
        );
        assert!(
            check_source_file_access_at_root(r"\cat src/main.rs").is_some(),
            "was: None"
        );
        assert!(
            check_source_file_access_at_root(r#"c"at" src/main.rs"#).is_some(),
            "was: None"
        );
    }

    #[test]
    fn an_unclosed_quote_falls_back_instead_of_skipping_the_check() {
        // The load-bearing control. If `shell_tokens` propagated the tokenize
        // error — returning an empty list, or the helpers bailing to their
        // permissive branch — an unclosed quote would be a universal bypass of
        // every check converted above. It must behave as the old model did.
        assert!(check_source_file_access_at_root("cat 'src/main.rs").is_some());
        assert!(has_recursive_flag("grep -r 'pattern ."));
        assert!(stage_trims("head -50 'x"));
        assert!(is_unbounded_lhs("cargo test 'x"));
    }

    #[test]
    fn shell_tokens_never_returns_nothing_for_a_command_with_words() {
        // The property the fallback exists to guarantee: whatever the quoting, a
        // command with words yields at least one token. No caller can be handed an
        // empty list and conclude there is no command here.
        for cmd in [
            "cat src/main.rs",
            "cat 'src/main.rs",
            r#"cat "src/main.rs"#,
            "'cat' src/main.rs",
            r"\cat src/main.rs",
        ] {
            assert!(
                !shell_tokens(cmd).is_empty(),
                "empty token list for {cmd:?}"
            );
        }
        // Whitespace-only is genuinely no command, and stays that way.
        assert!(shell_tokens("   ").is_empty());
    }

    #[test]
    fn custom_dangerous_patterns() {
        let config = PathSecurityConfig {
            shell_dangerous_patterns: vec!["kubectl delete".to_string()],
            ..PathSecurityConfig::default()
        };
        assert!(is_dangerous_command("kubectl delete pod nginx", &config).is_some());
    }

    // ── Source file access detection ─────────────────────────────────────

    #[test]
    fn source_file_access_blocks_cat_on_rs() {
        assert!(check_source_file_access_at_root("cat src/main.rs").is_some());
    }

    #[test]
    fn source_file_access_blocks_head_on_ts() {
        assert!(check_source_file_access_at_root("head -20 src/tools/mod.ts").is_some());
    }

    #[test]
    fn source_file_access_blocks_tail_on_go() {
        assert!(check_source_file_access_at_root("tail -n 50 server.go").is_some());
    }

    /// Every pre-existing source-access case predates the `project_root`
    /// parameter and uses a RELATIVE path, which is inside the project by
    /// construction — so routing them through one root keeps their verdicts
    /// unchanged while making the new dimension explicit where it matters.
    fn check_source_file_access_at_root(command: &str) -> Option<String> {
        check_source_file_access(command, Path::new("/home/u/work/myproj"))
    }

    #[test]
    fn source_file_access_allows_a_source_read_outside_the_project() {
        // The gate's remedy is "use symbols / read_file" — and both resolve
        // against the ACTIVE project, so neither can serve a dependency's
        // source, a sibling repo, or a config file elsewhere on disk. Refusing
        // these named an alternative that does not exist, which is worse than a
        // strict gate: a strict gate leaves a correct path open.
        //
        // 25 of 111 measured `il3_shell_on_source` refusals. GF-3 in
        // docs/trackers/2026-08-16-iron-law-gate-firing-audit.md.
        let root = Path::new("/home/u/work/myproj");
        assert!(
            check_source_file_access(
                "grep -n 'fn main' /home/u/.cargo/registry/src/foo-1.0/lib.rs",
                root
            )
            .is_none(),
            "a dependency's source is not reachable via symbols"
        );
        assert!(
            check_source_file_access("cat /home/u/work/otherrepo/src/main.rs", root).is_none(),
            "a sibling repo is not the active project"
        );
    }

    #[test]
    fn source_file_access_still_blocks_paths_inside_the_project() {
        // The control for the carve-out above, on both path forms. A relative
        // path is inside by construction (run_command's cwd IS the root), and an
        // absolute path under the root is the same file named the long way.
        let root = Path::new("/home/u/work/myproj");
        assert!(check_source_file_access("cat src/main.rs", root).is_some());
        assert!(
            check_source_file_access("cat /home/u/work/myproj/src/main.rs", root).is_some(),
            "an absolute path INSIDE the project must still block"
        );
    }

    #[test]
    fn source_file_access_keeps_blocking_when_a_path_cannot_be_resolved() {
        // Conservative direction. The carve-out must not become a bypass for
        // anything merely unusual to parse — only for paths demonstrably outside
        // the root. A bare relative path with no directory still blocks.
        let root = Path::new("/home/u/work/myproj");
        assert!(check_source_file_access("cat main.rs", root).is_some());
        assert!(check_source_file_access("sed -n '1,5p' ./src/lib.rs", root).is_some());
    }

    #[test]
    fn a_cd_out_of_the_project_makes_a_relative_source_read_reachable_again() {
        // Same defect GF-3 removed for absolute paths, arriving by the other
        // path form. `run_command` STARTS at the root, so a relative token was
        // "inside by construction" — but `cd` moves the shell, and the gate
        // already splits on `&&`, so the cd sits in the segment list the
        // decision walks and was simply never consulted. The refusal named
        // symbols/read_file, which resolve against the active project and
        // cannot serve the file.
        let root = Path::new("/home/u/work/myproj");
        assert!(
            check_source_file_access("cd /tmp/scratch && awk '{print}' head.rs", root).is_none(),
            "after cd out of the project a relative source path is not in the index"
        );
        assert!(
            check_source_file_access("cd /tmp/scratch; cat lib.rs", root).is_none(),
            "the carve-out is about the shell moving, not about which operator moved it"
        );
    }

    #[test]
    fn a_cd_that_stays_inside_the_project_still_blocks() {
        // The control that a careless fix breaks: only a cd whose target LEAVES
        // the root may open the gate. Moving around inside it changes nothing,
        // because symbols/read_file can still serve the file.
        let root = Path::new("/home/u/work/myproj");
        assert!(
            check_source_file_access("cd /home/u/work/myproj/src && cat main.rs", root).is_some(),
            "still inside the project — the remedy is followable, so keep refusing"
        );
    }

    #[test]
    fn a_cd_the_gate_cannot_resolve_keeps_blocking() {
        // Conservative direction, restated for the new dimension: the gate opens
        // only on a cd it fully understands. Everything else keeps the old
        // verdict, so an unparseable target can never be a way through.
        let root = Path::new("/home/u/work/myproj");
        assert!(
            check_source_file_access("cd $SCRATCH && cat main.rs", root).is_some(),
            "a variable target is not resolvable at parse time"
        );
        assert!(
            check_source_file_access("cd .. && cat src/main.rs", root).is_some(),
            "a parent traversal is not resolved lexically — keep blocking"
        );
        assert!(
            check_source_file_access("cd && cat main.rs", root).is_some(),
            "bare cd means $HOME; deliberately left unresolved to keep this hermetic"
        );
    }

    #[test]
    fn a_cd_inside_a_pipeline_does_not_move_the_shell_for_other_stages() {
        // `cd x | cmd` runs the cd in a subshell — the other stage keeps the
        // original cwd. Propagating cwd across a pipe would be a bypass rather
        // than a carve-out, so cwd only advances when the cd is a whole run.
        let root = Path::new("/home/u/work/myproj");
        assert!(
            check_source_file_access("cd /tmp/scratch | cat src/main.rs", root).is_some(),
            "a cd inside a pipeline must not move the gate's cwd"
        );
    }

    #[test]
    fn an_option_glob_does_not_force_the_in_project_verdict() {
        // `--include='*.mjs'` is a FILTER pattern, not a file operand: nothing
        // is read from it. It is relative, so the relative-token branch counted
        // it as in-project and refused a sweep of a sibling repo whose search
        // root was absolute and outside — with a hint (`grep(pattern, path)`)
        // that resolves against the active project and therefore has no correct
        // invocation for that repo at all.
        // Drive-prefixed on Windows, where `/home/...` is RELATIVE — without this the
        // search root is not "absolute and outside" at all, the premise the case rests on
        // collapses, and the assertion fails for a reason unrelated to the filter/operand
        // distinction it exists to pin.
        #[cfg(windows)]
        const P: &str = "C:";
        #[cfg(not(windows))]
        const P: &str = "";

        let root = format!("{P}/home/u/work/myproj");
        let root = Path::new(&root);
        assert!(
            check_source_file_access(
                &format!("grep -rn 'x' {P}/home/u/work/otherrepo --include='*.mjs'"),
                root
            )
            .is_none(),
            "a glob filter beside an out-of-project search root is not a project read"
        );
    }

    #[test]
    fn an_option_glob_still_counts_without_evidence_of_an_outside_target() {
        // The discriminating control, and the reason the carve-out above is
        // keyed on positive evidence rather than on "options are not paths".
        // `grep -rn x src/ --include='*.rs'` genuinely reads project source, and
        // the GLOB IS THE ONLY TOKEN THAT SAYS SO — `src/` carries no extension.
        // Skipping option tokens outright would have opened exactly this hole
        // while looking like a tidier rule.
        let root = Path::new("/home/u/work/myproj");
        assert!(
            check_source_file_access("grep -rn 'x' src/ --include='*.rs'", root).is_some(),
            "an in-project recursive grep is still a project source read"
        );
        assert!(
            check_source_file_access("grep -rn 'x' --include='*.rs'", root).is_some(),
            "no operand at all means the cwd, which is the project root"
        );
    }

    #[test]
    fn source_file_access_allows_wc_on_source() {
        // `wc` emits a COUNT, never content. This gate exists to route content
        // reads to symbols/read_file — and codescout has no tool that returns a
        // line count, so the refusal named an alternative that does not exist.
        // Same reasoning the pipe gate already applies to `wc` as an RHS
        // aggregator (`il3_allows_git_status_pipe_wc`).
        //
        // 18 of 111 measured `il3_shell_on_source` refusals. GF-3 in
        // docs/trackers/2026-08-16-iron-law-gate-firing-audit.md.
        //
        // Supersedes `source_file_access_blocks_wc_on_rs`, which asserted the
        // opposite on this exact input. That case carried no rationale and
        // arrived with the feature itself (`8dc6a18c`) as one row of a
        // per-verb matrix — it documented that `wc` was on the list, which is
        // the thing being changed, so it was retired rather than flipped.
        assert!(check_source_file_access_at_root("wc -l src/tools/markdown/tests.rs").is_none());
        assert!(check_source_file_access_at_root("wc -c src/lib.rs").is_none());
    }

    #[test]
    fn source_file_access_still_blocks_content_readers_after_the_wc_carve_out() {
        // The control for the carve-out above. `head`/`tail`/`cat` return the
        // file's bytes, so they stay blocked — the distinction is content vs
        // a measurement OF content, not read-only vs mutating.
        assert!(check_source_file_access_at_root("head -20 src/lib.rs").is_some());
        assert!(check_source_file_access_at_root("tail -5 src/lib.rs").is_some());
        assert!(check_source_file_access_at_root("cat src/lib.rs").is_some());
    }

    #[test]
    fn source_file_access_blocks_sed_on_py() {
        assert!(check_source_file_access_at_root("sed -n '1,100p' lib.py").is_some());
    }

    #[test]
    fn source_file_access_blocks_awk_on_java() {
        assert!(check_source_file_access_at_root("awk '{print}' Foo.java").is_some());
    }

    #[test]
    fn source_file_access_blocks_less_on_rs() {
        assert!(check_source_file_access_at_root("less src/agent.rs").is_some());
    }

    #[test]
    fn source_file_access_allows_cat_on_markdown() {
        // markdown is excluded — it's not source code
        assert!(check_source_file_access_at_root("cat README.md").is_none());
    }

    #[test]
    fn source_file_access_allows_wc_on_txt() {
        assert!(check_source_file_access_at_root("wc -l output.txt").is_none());
    }

    #[test]
    fn source_file_access_allows_sed_on_toml() {
        assert!(check_source_file_access_at_root("sed 's/foo/bar/g' config.toml").is_none());
    }

    #[test]
    fn source_file_access_allows_cat_without_source_ext() {
        assert!(check_source_file_access_at_root("cat Makefile").is_none());
    }

    #[test]
    fn source_file_access_hint_mentions_read_file() {
        let hint = check_source_file_access_at_root("cat src/main.rs").unwrap();
        assert!(
            hint.contains("read_file"),
            "hint should mention read_file, got: {hint}"
        );
    }

    #[test]
    fn source_file_access_hint_mentions_symbols() {
        let hint = check_source_file_access_at_root("head -5 lib.rs").unwrap();
        assert!(
            hint.contains("symbols"),
            "hint should mention symbols, got: {hint}"
        );
    }

    #[test]
    fn grep_on_source_with_identifier_gives_symbol_ladder() {
        let hint =
            check_source_file_access_at_root("grep WriteMemory src/tools/memory.rs").unwrap();
        assert!(hint.contains("symbols(name='WriteMemory')"), "got: {hint}");
        assert!(
            hint.contains("references(symbol='WriteMemory')"),
            "got: {hint}"
        );
        assert!(
            hint.contains("call_graph(symbol='WriteMemory'"),
            "got: {hint}"
        );
    }

    #[test]
    fn grep_on_source_with_regex_gives_generic_hint() {
        let hint = check_source_file_access_at_root("grep 'foo.*bar' src/main.rs").unwrap();
        assert!(hint.contains("grep(pattern"), "got: {hint}");
        // must NOT show symbol ladder for regex patterns
        assert!(!hint.contains("call_graph"), "got: {hint}");
    }

    #[test]
    fn grep_pipe_alternation_uses_first_part_in_hint() {
        let hint =
            check_source_file_access_at_root("grep 'WriteMemory|ReadMemory' src/tools/memory.rs")
                .unwrap();
        assert!(hint.contains("symbols(name='WriteMemory')"), "got: {hint}");
    }

    #[test]
    fn grep_value_taking_flag_skipped_for_identifier() {
        let hint =
            check_source_file_access_at_root("grep -A 3 WriteMemory src/tools/memory.rs").unwrap();
        assert!(hint.contains("symbols(name='WriteMemory')"), "got: {hint}");
    }

    #[test]
    fn source_file_access_sed_hint_mentions_grep() {
        let hint = check_source_file_access_at_root("sed -n '1p' foo.ts").unwrap();
        assert!(
            hint.contains("grep"),
            "sed hint should mention grep, got: {hint}"
        );
    }

    #[test]
    fn source_file_access_allows_non_blocked_command() {
        // cp, mv, diff are not in the blocked command set
        assert!(check_source_file_access_at_root("cp src/main.rs src/main2.rs").is_none());
    }

    #[test]
    fn source_file_access_allows_git_diff_piped_to_head() {
        // `head` is in the second segment; the `.rs` file is in the first (git diff arg).
        // Per-segment check means this should NOT be blocked.
        assert!(check_source_file_access_at_root("git diff src/server.rs | head -80").is_none());
    }

    #[test]
    fn source_file_access_blocks_cat_in_same_segment_as_source_file() {
        // `cat` and `.rs` are in the same segment — still blocked.
        assert!(check_source_file_access_at_root("cat src/main.rs | grep fn").is_some());
    }

    #[test]
    fn source_file_access_allows_cat_heredoc_with_source_ext_in_content() {
        // `cat <<'EOF'` reads stdin via a heredoc — the `.rs` extension appears
        // only in the heredoc body (e.g. a commit message), not as a filename
        // argument to cat. The `<<` operator marks the segment as stdin-reading
        // so it must not be blocked.
        assert!(check_source_file_access_at_root(
            "git commit -m \"$(cat <<'EOF'\nFix bug in path_security.rs\nEOF\n)\""
        )
        .is_none());
    }

    #[test]
    fn source_file_access_blocks_cat_rs_file_after_heredoc_segment() {
        // A pipe AFTER a heredoc segment must still be checked independently.
        // `cat <<'EOF' ... EOF | cat src/main.rs` — second segment is a real read.
        assert!(
            check_source_file_access_at_root("cat <<'EOF'\nhello\nEOF\n | cat src/main.rs")
                .is_some()
        );
    }

    /// BUG docs/issues/archive/2026-08-17-heredoc-carve-out-defeated-by-a-pipe-in-the-body.md
    ///
    /// The per-segment `seg.contains("<<")` skip protects only the segment holding
    /// the opener. One `|` in the body splits the rest of it into segments that are
    /// then read as commands — so a `git commit -F -` whose message quotes a shell
    /// pipeline is refused as source-file access. `strip_heredoc_bodies` already
    /// solved this for the pipe gate (`detect_il3_violation`); this gate never
    /// adopted it.
    #[test]
    fn source_file_access_allows_a_pipe_inside_a_heredoc_body() {
        assert_eq!(
            check_source_file_access_at_root("true <<'EOF'\nx | head -1 foo.rs\nEOF"),
            None,
            "a heredoc body is data — a pipe in it must not expose the body to the gate"
        );
    }

    /// The reported symptom, reproduced exactly: a commit message quoting a shell
    /// pipeline, where the text after the pipe begins with a reader name and a
    /// source-extension filename appears later in the same span. That is `tail` plus
    /// `il3-warn-hook.mjs` here, which is the pair that refused the real commit.
    ///
    /// Two earlier versions of this test passed before the fix and so reproduced
    /// nothing: the bare alternation put no reader at a segment head, and
    /// `plugin.json` is not a source extension. Both are recorded because the near
    /// miss is the lesson — a green test proves nothing until you know why it is red.
    #[test]
    fn source_file_access_allows_a_commit_message_quoting_a_pipe_alternation() {
        let cmd = "git commit -F - <<'MSG'\n\
                   docs: the advisory flags what its own message calls bounded\n\
                   A `git log -3 | tail -30` drew an IL3 warning, and -3 is a limiter.\n\
                   il3-warn-hook.mjs:23 decides unbounded LHS from one flat alternation.\n\
                   MSG";
        assert_eq!(
            check_source_file_access_at_root(cmd),
            None,
            "committing two markdown files must not read as shell access to source"
        );
    }

    /// `<<<` is a here-string: it takes no body, so nothing after it may be
    /// swallowed. Mirrors `il3_treats_a_here_string_as_having_no_body` on the pipe
    /// gate — the risk a heredoc-stripping fix introduces is hiding a real read.
    ///
    /// The pipe is load-bearing: without an operator there is only one segment, and
    /// the gate reads a segment's command from its FIRST token, so a read on a later
    /// line is invisible for reasons that have nothing to do with here-strings. See
    /// `source_file_access_does_not_split_on_newlines` below.
    #[test]
    fn source_file_access_here_string_does_not_swallow_a_following_read() {
        assert!(
            check_source_file_access_at_root("cargo test <<< word | cat src/main.rs").is_some(),
            "a here-string has no body; the following real read must still be caught"
        );
    }

    /// A bypass the old per-segment `<<` skip created, found while removing it: a
    /// here-string puts `<<` in the SAME segment as a real read, so the whole segment
    /// was skipped and `cat src/main.rs` went through. `<<<` takes no body, so there
    /// was never anything to excuse here.
    #[test]
    fn source_file_access_blocks_a_source_read_on_a_here_string_line() {
        assert!(
            check_source_file_access_at_root("cat src/main.rs <<< x").is_some(),
            "a here-string on the line must not excuse the read next to it"
        );
    }

    /// A newline is a command separator in shell, exactly like `;`. Before the fix the
    /// splitter broke on `&&`, `||`, `;` and `|` only, and a segment's command is its
    /// FIRST token — so a source read on a second line was never seen and
    /// `echo hi\ncat src/main.rs` read project source unchecked.
    ///
    /// This test asserted the permissive behaviour as a deliberate tripwire until the
    /// gap was closed; it now asserts the fix.
    /// BUG docs/issues/archive/2026-08-17-source-gate-does-not-split-on-newlines.md
    #[test]
    fn source_file_access_splits_on_newlines() {
        assert!(
            check_source_file_access_at_root("echo hi\ncat src/main.rs").is_some(),
            "a newline separates commands; a read on the second line must be caught"
        );
    }

    /// The false-positive guard, and the one a careless fix breaks: a newline INSIDE a
    /// quoted argument is data. `split_outside_quotes` tracks quote state across the
    /// whole string, newlines included, so this holds — but it had never been exercised
    /// with `\n` in the separator list before.
    #[test]
    fn source_file_access_does_not_split_a_newline_inside_a_quoted_argument() {
        assert_eq!(
            check_source_file_access_at_root("git commit -m \"line one\ncat src/main.rs\""),
            None,
            "a newline inside a quoted argument is data, not a separator"
        );
    }

    /// A backslash-newline is a line continuation: one command spanning two lines. The
    /// escape handler skips both characters, so no split happens — and this command
    /// genuinely IS a read, so it must still block.
    #[test]
    fn source_file_access_blocks_a_read_across_a_line_continuation() {
        assert!(
            check_source_file_access_at_root("cat \\\n  src/main.rs").is_some(),
            "a line continuation is one command, and this one reads source"
        );
    }

    #[test]
    fn is_source_path_recognizes_supported_extensions() {
        assert!(is_source_path("src/main.rs"));
        assert!(is_source_path("lib.py"));
        assert!(is_source_path("index.ts"));
        assert!(is_source_path("main.go"));
        assert!(is_source_path("App.java"));
        assert!(is_source_path("Main.kt"));
        assert!(is_source_path("server.js"));
        assert!(!is_source_path("README.md"));
        assert!(!is_source_path("Cargo.toml"));
        assert!(!is_source_path("config.json"));
    }

    #[test]
    fn split_outside_quotes_no_separators() {
        let parts = split_outside_quotes("git status", &["&&", "||", ";", "|"]);
        assert_eq!(parts, vec!["git status"]);
    }

    #[test]
    fn split_outside_quotes_pipe() {
        let parts = split_outside_quotes("cat foo.rs | grep fn", &["&&", "||", ";", "|"]);
        assert_eq!(parts, vec!["cat foo.rs", "grep fn"]);
    }

    #[test]
    fn split_outside_quotes_ampersand() {
        let parts = split_outside_quotes("./build.sh && cat src/main.rs", &["&&", "||", ";", "|"]);
        assert_eq!(parts, vec!["./build.sh", "cat src/main.rs"]);
    }

    #[test]
    fn split_outside_quotes_ampersand_inside_double_quotes() {
        // The && inside "..." must NOT split
        let parts = split_outside_quotes(
            r#"git commit -m "fix && cat src/main.rs""#,
            &["&&", "||", ";", "|"],
        );
        assert_eq!(parts, vec![r#"git commit -m "fix && cat src/main.rs""#]);
    }

    #[test]
    fn split_outside_quotes_pipe_inside_single_quotes() {
        // The | inside '...' must NOT split
        let parts = split_outside_quotes("sed -n '1|2p' foo.rs", &["&&", "||", ";", "|"]);
        assert_eq!(parts, vec!["sed -n '1|2p' foo.rs"]);
    }

    #[test]
    fn split_outside_quotes_double_pipe_before_single_pipe() {
        // "||" must be matched as one token, not split into two "|" segments
        let parts = split_outside_quotes("cmd1 || cmd2", &["&&", "||", ";", "|"]);
        assert_eq!(parts, vec!["cmd1", "cmd2"]);
    }

    #[test]
    fn split_outside_quotes_semicolon() {
        let parts = split_outside_quotes("echo done; cat src/main.rs", &["&&", "||", ";", "|"]);
        assert_eq!(parts, vec!["echo done", "cat src/main.rs"]);
    }

    /// A newline is a separator like any other once it is in `seps`.
    /// BUG docs/issues/archive/2026-08-17-source-gate-does-not-split-on-newlines.md
    #[test]
    fn split_outside_quotes_newline() {
        let parts = split_outside_quotes("echo hi\ncat src/main.rs", &["&&", "||", ";", "|", "\n"]);
        assert_eq!(parts, vec!["echo hi", "cat src/main.rs"]);
    }

    /// And a newline inside quotes is not. This is why adding `\n` to the gate's
    /// separator list is safe without any other change: quote state is tracked
    /// character by character across the whole string, so it spans line breaks.
    #[test]
    fn split_outside_quotes_newline_inside_double_quotes() {
        let parts =
            split_outside_quotes("git commit -m \"one\ntwo\"", &["&&", "||", ";", "|", "\n"]);
        assert_eq!(parts, vec!["git commit -m \"one\ntwo\""]);
    }

    #[test]
    fn split_outside_quotes_escaped_quote() {
        // \" inside a double-quoted string must not close the string
        let parts =
            split_outside_quotes(r#"echo "say \"hi\" && bye" && ls"#, &["&&", "||", ";", "|"]);
        assert_eq!(parts.len(), 2);
        assert!(parts[0].contains("say"));
        assert_eq!(parts[1].trim(), "ls");
    }

    #[test]
    fn split_outside_quotes_empty_segments_skipped() {
        // Trailing semicolon — empty last segment is dropped
        let parts = split_outside_quotes("echo hi;", &["&&", "||", ";", "|"]);
        assert_eq!(parts, vec!["echo hi"]);
    }

    // --- quote-aware splitting ---

    #[test]
    fn git_commit_with_tail_in_message_not_blocked() {
        // "tail" and ".rs" appear inside the commit message — must NOT block
        assert!(check_source_file_access_at_root(
            r#"git commit -m "feat: tail-50 of log, output_buffer.rs, workflow.rs""#
        )
        .is_none());
    }

    #[test]
    fn git_commit_with_ampersand_and_source_in_message_not_blocked() {
        // "&&" and "cat src/main.rs" inside the quoted message — must NOT block
        assert!(check_source_file_access_at_root(
            r#"git commit -m "fix && cat src/main.rs was broken""#
        )
        .is_none());
    }

    #[test]
    fn compound_and_then_cat_blocked() {
        // cat src/main.rs is a real command after &&
        assert!(check_source_file_access_at_root("./build.sh && cat src/main.rs").is_some());
    }

    #[test]
    fn semicolon_then_cat_blocked() {
        assert!(check_source_file_access_at_root("echo done; cat src/main.rs").is_some());
    }

    #[test]
    fn or_then_tail_blocked() {
        assert!(check_source_file_access_at_root("cargo build || tail src/lib.rs").is_some());
    }

    #[test]
    fn pipe_chain_with_source_blocked() {
        // tail is the first token of its segment — blocked
        assert!(check_source_file_access_at_root("tail src/main.rs | grep error").is_some());
    }

    // ── SecurityProfile tests ───────────────────────────────────────────

    #[test]
    fn root_profile_bypasses_read_deny_list() {
        let dir = tempdir().unwrap();
        let ssh_dir = dir.path().join(".ssh");
        std::fs::create_dir_all(&ssh_dir).unwrap();
        let key_file = ssh_dir.join("id_rsa");
        std::fs::write(&key_file, "secret").unwrap();

        let config = PathSecurityConfig {
            profile: SecurityProfile::Root,
            ..PathSecurityConfig::default()
        };

        let result = validate_read_path(key_file.to_str().unwrap(), Some(dir.path()), &config);
        assert!(result.is_ok(), "root profile should bypass read deny-list");
    }

    #[test]
    fn root_profile_bypasses_write_boundary() {
        let dir = tempdir().unwrap();
        let outside = dir.path().join("outside_project");
        std::fs::create_dir_all(&outside).unwrap();
        let target = outside.join("file.txt");

        let project_root = dir.path().join("project");
        std::fs::create_dir_all(&project_root).unwrap();

        let config = PathSecurityConfig {
            profile: SecurityProfile::Root,
            ..PathSecurityConfig::default()
        };

        let result = validate_write_path(
            target.to_str().unwrap(),
            &project_root,
            &config,
            &default_session_roots(),
        );
        assert!(result.is_ok(), "root profile should bypass write boundary");
    }

    #[test]
    fn validate_write_path_outside_root_mentions_approve_write() {
        let dir = tempdir().unwrap();
        // Use /var — outside both the project root and /tmp, so the boundary check
        // fires and the error message must mention approve_write.
        let target = "/var/outside_ce_test_approve_write_hint.rs";
        let result = validate_write_path(
            target,
            dir.path(),
            &default_config(),
            &default_session_roots(),
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("approve_write"),
            "error should mention approve_write: {err}"
        );
    }

    #[test]
    fn validate_write_path_allows_session_approved_root() {
        let dir = tempdir().unwrap();
        let other = tempdir().unwrap();
        let session_roots = vec![other.path().to_path_buf()];
        let target = other.path().join("file.txt");
        let result = validate_write_path(
            target.to_str().unwrap(),
            dir.path(),
            &default_config(),
            &session_roots,
        );
        assert!(
            result.is_ok(),
            "approved root should allow writes: {:?}",
            result
        );
    }

    #[test]
    fn validate_write_path_session_root_still_respects_deny_list() {
        let dir = tempdir().unwrap();
        let home = crate::platform::home_dir().unwrap();
        let ssh = home.join(".ssh");
        // Even if someone manages to sneak ~/.ssh into session_roots, deny-list wins
        let session_roots = vec![ssh.clone()];
        let target = ssh.join("authorized_keys");
        let result = validate_write_path(
            target.to_str().unwrap(),
            dir.path(),
            &default_config(),
            &session_roots,
        );
        assert!(result.is_err(), "deny-list must win over session roots");
    }

    #[test]
    fn root_profile_bypasses_dangerous_command_check() {
        let config = PathSecurityConfig {
            profile: SecurityProfile::Root,
            ..PathSecurityConfig::default()
        };

        let result = is_dangerous_command("rm -rf /", &config);
        assert!(
            result.is_none(),
            "root profile should skip dangerous command check"
        );
    }

    #[test]
    fn default_profile_still_enforces_all_gates() {
        let config = PathSecurityConfig::default();
        assert_eq!(config.profile, SecurityProfile::Default);

        let result = is_dangerous_command("rm -rf /", &config);
        assert!(result.is_some());
    }
    #[test]
    fn is_identifier_pattern_accepts_single() {
        assert!(is_identifier_pattern("WriteMemory"));
        assert!(is_identifier_pattern("snake_case"));
        assert!(is_identifier_pattern("_private"));
        assert!(is_identifier_pattern("CamelCase123"));
    }

    #[test]
    fn is_identifier_pattern_accepts_pipe_alternation() {
        assert!(is_identifier_pattern("WriteMemory|ReadMemory|ListMemories"));
    }

    // ── Approve write validation ──────────────────────────────────────────

    #[test]
    fn validate_approve_path_accepts_normal_directory() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("other");
        std::fs::create_dir_all(&target).unwrap();
        let result = validate_approve_path(target.to_str().unwrap(), dir.path(), &default_config());
        assert!(
            result.is_ok(),
            "normal directory should be approved: {:?}",
            result
        );
    }

    #[cfg_attr(
        target_os = "windows",
        ignore = "/ is not the Windows filesystem root; needs platform-specific drive-root rejection logic. See docs/issues/archive/2026-05-24-ci-windows-test-portability-rot.md"
    )]
    #[test]
    fn validate_approve_path_rejects_filesystem_root() {
        let dir = tempdir().unwrap();
        let result = validate_approve_path("/", dir.path(), &default_config());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too broad"));
    }

    #[test]
    fn validate_approve_path_rejects_home_directory() {
        let dir = tempdir().unwrap();
        let home = home_dir().unwrap();
        let result = validate_approve_path(home.to_str().unwrap(), dir.path(), &default_config());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too broad"));
    }

    #[test]
    fn validate_approve_path_rejects_denied_path() {
        let dir = tempdir().unwrap();
        let home = home_dir().unwrap();
        let ssh = home.join(".ssh");
        let result = validate_approve_path(ssh.to_str().unwrap(), dir.path(), &default_config());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("protected location"));
    }

    #[test]
    fn validate_approve_path_resolves_relative_path() {
        let dir = tempdir().unwrap();
        let result = validate_approve_path("subdir", dir.path(), &default_config());
        // subdir doesn't need to exist — best_effort_canonicalize handles it
        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert!(resolved.ends_with("subdir"));
    }

    #[test]
    fn validate_approve_path_rejects_null_byte() {
        let dir = tempdir().unwrap();
        let result = validate_approve_path("sub\0dir", dir.path(), &default_config());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("null bytes"));
    }

    #[test]
    fn is_identifier_pattern_rejects_regex_and_empty() {
        assert!(!is_identifier_pattern(""));
        assert!(!is_identifier_pattern("foo.*bar"));
        assert!(!is_identifier_pattern("^start"));
        assert!(!is_identifier_pattern("foo(bar)"));
        assert!(!is_identifier_pattern("foo[0-9]"));
        assert!(!is_identifier_pattern("||")); // empty parts
    }

    // -----------------------------------------------------------------
    // IL3 detection — server-side enforcement of "no piping live output
    // to log-trimmers". Mirrors codescout-companion/hooks/il3-deny-hook.test.sh.
    // -----------------------------------------------------------------

    #[test]
    fn il3_blocks_cargo_test_pipe_grep() {
        let hint = detect_il3_violation("cargo test | grep FAILED").expect("should block");
        assert!(hint.contains("IL3 violation"));
        assert!(hint.contains("cargo test"));
    }

    #[test]
    fn il3_allows_buffer_op_grep_cmd_sort() {
        assert!(detect_il3_violation("grep -c EnterWorktree @cmd_3b8e6cc5 | sort -u").is_none());
    }

    #[test]
    fn il3_allows_buffer_op_cat_bg_head() {
        assert!(detect_il3_violation("cat @bg_abc123 | head -50").is_none());
    }

    #[test]
    fn il3_allows_buffer_op_grep_tool_sort() {
        assert!(detect_il3_violation("grep error @tool_xyz | sort").is_none());
    }

    #[test]
    fn il3_allows_buffer_op_file_ref() {
        assert!(detect_il3_violation("grep TODO @file_abc | wc -l").is_none());
    }

    #[test]
    fn il3_allows_no_pipe() {
        assert!(detect_il3_violation("cargo test").is_none());
    }

    #[test]
    fn il3_allows_pipe_to_jq() {
        assert!(detect_il3_violation("cargo metadata | jq .packages").is_none());
    }

    #[test]
    fn il3_blocks_when_ref_only_on_rhs() {
        let hint = detect_il3_violation("cargo test | grep FAIL @cmd_abc").expect("should block");
        assert!(hint.contains("IL3 violation"));
    }

    #[test]
    fn il3_allows_grep_single_file_pipe_sort() {
        // Bounded LHS — single file arg, no recursive flag. Allowed per
        // docs/issues/archive/2026-05-18-il3-overtriggers-bounded-lhs.md.
        assert!(detect_il3_violation("grep -oE 'pat' src/lib.rs | sort -u").is_none());
    }

    // -----------------------------------------------------------------
    // git: bounded when the command carries an explicit output limiter.
    // GF-1/GF-2 in docs/trackers/2026-08-16-iron-law-gate-firing-audit.md —
    // `git` sat on UNBOUNDED_PREFIXES unconditionally, and 47 of 94 measured
    // `git` IL3 refusals carried one of these tokens.
    // -----------------------------------------------------------------

    #[test]
    fn il3_allows_git_log_with_explicit_count() {
        // `-3` bounds the commit count. The single most common refused shape.
        assert!(detect_il3_violation("git log --oneline -3 | head -20").is_none());
    }

    #[test]
    fn il3_allows_git_log_with_dash_n_count() {
        assert!(detect_il3_violation("git log -n 5 | tail -20").is_none());
    }

    #[test]
    fn il3_allows_git_branch_show_current() {
        // One value by construction.
        assert!(detect_il3_violation("git branch --show-current | head -1").is_none());
    }

    #[test]
    fn il3_allows_git_status_porcelain() {
        // Bounded by the working tree, not by history.
        assert!(detect_il3_violation("git status --short | head -30").is_none());
        assert!(detect_il3_violation("git status --porcelain | grep foo").is_none());
    }

    #[test]
    fn il3_limiter_matches_gits_attached_value_spellings() {
        // git spells each of these limiters two ways. The gate used to compare
        // WHOLE TOKENS, so the attached form matched nothing in the table and a
        // genuinely bounded command was refused — while the refusal text
        // recommended `git log -3`, which is the same bound spelled the other
        // documented way. `--max-count=` was the sole attached form handled,
        // via a hard-coded prefix test.
        for cmd in [
            "git status --porcelain=v1 | head -5",
            "git status --porcelain=v2 | head -3",
            "git show --stat=200 HEAD | head -3",
            "git log --max-count=3 | head -5",
            "git log -n5 | head -3",
        ] {
            assert!(
                detect_il3_violation(cmd).is_none(),
                "attached-value limiter must read as bounded: {cmd}"
            );
        }

        // The bare spellings each of those is equivalent to, so a regression
        // that breaks the `=` split cannot hide behind the new cases alone.
        for cmd in [
            "git status --porcelain | head -5",
            "git show --stat HEAD | head -3",
            "git log --max-count 3 | head -5",
            "git log -n 5 | head -3",
            "git log -3 | head -3",
        ] {
            assert!(
                detect_il3_violation(cmd).is_none(),
                "bare limiter must still read as bounded: {cmd}"
            );
        }

        // The other half of the pair, and the reason the assertions above are
        // worth anything: every one of them would ALSO pass in a world where
        // the gate simply stopped inspecting `git`. These fail in that world.
        // `--oneline` stays a non-limiter (it bounds width, not line count),
        // and splitting on `=` must not admit any flag that merely carries a
        // value — `--pretty=format:%h` formats output without limiting it.
        assert!(detect_il3_violation("git log | head -3").is_some());
        assert!(detect_il3_violation("git log --oneline | head -3").is_some());
        assert!(detect_il3_violation("git log --pretty=format:%h | head -3").is_some());
    }

    #[test]
    fn il3_allows_git_show_stat() {
        // A diffstat for one commit, not a diff body.
        assert!(detect_il3_violation("git show --stat --oneline abc1234 | head -20").is_none());
    }

    #[test]
    fn il3_blocks_bare_git_log() {
        // No limiter: `git log` emits the entire history.
        let hint = detect_il3_violation("git log | head -40").expect("should block");
        assert!(hint.contains("IL3 violation"));
    }

    #[test]
    fn il3_blocks_git_oneline_without_a_count() {
        // `--oneline` bounds line WIDTH, not line COUNT — `git log --oneline`
        // still emits one line per commit for every commit. It is the most
        // tempting false entry on the bounding list (25 of the 94 measured
        // refusals carried it), and admitting it would make the gate wrong.
        let hint = detect_il3_violation("git log --oneline | head -40").expect("should block");
        assert!(hint.contains("IL3 violation"));
    }

    #[test]
    fn il3_blocks_git_diff_without_a_limiter() {
        // A single-file diff is still an arbitrarily large diff body.
        assert!(detect_il3_violation("git diff -- src/lib.rs | tail -50").is_some());
    }

    #[test]
    fn il3_does_not_read_the_limiter_from_the_trimmer() {
        // The limiter must be found on the LHS. `head -20` on the RHS is the
        // trimmer's own count and must not bound the producer — the exact
        // contamination that inflated the first measurement of this defect.
        let hint = detect_il3_violation("git log --oneline | head -20").expect("should block");
        assert!(hint.contains("IL3 violation"));
    }

    #[test]
    fn il3_blocks_grep_recursive() {
        let hint = detect_il3_violation("grep -r FAILED src/ | head").expect("should block");
        assert!(hint.contains("IL3 violation"));
    }

    #[test]
    fn il3_blocks_grep_long_recursive() {
        assert!(detect_il3_violation("grep --recursive pat src/ | sort").is_some());
    }

    #[test]
    fn il3_blocks_find_no_maxdepth() {
        assert!(detect_il3_violation("find / -name '*.rs' | head").is_some());
    }

    #[test]
    fn il3_allows_find_with_maxdepth() {
        assert!(detect_il3_violation("find . -maxdepth 2 -name '*.rs' | head").is_none());
    }

    #[test]
    fn il3_allows_cat_pipe_grep() {
        // Single-file cat is bounded. Was over-blocked before the
        // 2026-05-18 bounded-LHS fix.
        assert!(detect_il3_violation("cat items.txt | grep apple").is_none());
    }

    #[test]
    fn il3_allows_ls_pipe_head() {
        assert!(detect_il3_violation("ls /some/dir | head -20").is_none());
    }

    #[test]
    fn il3_allows_awk_file_pipe_sort() {
        assert!(detect_il3_violation("awk '{print $1}' file.log | sort -u").is_none());
    }

    #[test]
    fn il3_allows_sed_file_pipe_head() {
        assert!(detect_il3_violation("sed 's/foo/bar/' file.txt | head").is_none());
    }

    #[test]
    fn il3_blocks_rg_pipe_head() {
        // rg defaults to recursive — treated as unbounded.
        assert!(detect_il3_violation("rg pattern | head").is_some());
    }

    #[test]
    fn il3_friction_repro_allowed() {
        let cmd = r#"grep -oE "\"cwd\":\"[^\"]*\"" @cmd_3b8e6cc5 | sort -u"#;
        assert!(detect_il3_violation(cmd).is_none());
    }

    #[test]
    fn il3_allows_a_pipe_that_only_appears_inside_a_heredoc_body() {
        // The real block: a commit message DESCRIBING a piping mistake tripped the
        // gate. The heredoc is single-quoted, so the shell does not even expand it —
        // the `| tail -1` below is prose.
        let cmd = r#"git commit -F - <<'EOF'
Five errors this session, one shape: pgrep | tail -1 picked an orphan, and
ls | wc -l counted sidecars.
EOF"#;
        assert!(
            detect_il3_violation(cmd).is_none(),
            "a pipe inside a heredoc body is data, not syntax"
        );
    }

    #[test]
    fn il3_still_blocks_a_real_pipe_on_the_line_that_opens_a_heredoc() {
        // Stripping bodies must not blind the gate to the command itself. Only the
        // body and its terminator are dropped; the opener line stays.
        let cmd = "cargo test | grep FAILED <<'EOF'\nnot a pipe here\nEOF";
        assert!(detect_il3_violation(cmd).is_some());
    }

    #[test]
    fn il3_treats_a_here_string_as_having_no_body() {
        // `<<<` takes no body, so nothing after it may be swallowed as one.
        let cmd = "cargo test <<< word\ncargo build | grep warning";
        assert!(
            detect_il3_violation(cmd).is_some(),
            "the second line's pipe must still be seen"
        );
    }

    /// Sibling of the source gate's newline gap, in the pipe gate: `pipeline_segments`
    /// splits on `&&`, `||` and `;` but not on a newline, so a multi-line command is one
    /// segment and the pipe's real LHS is not the segment's first token.
    /// BUG docs/issues/archive/2026-08-17-source-gate-does-not-split-on-newlines.md
    #[test]
    fn il3_detects_a_piped_unbounded_command_on_a_later_line() {
        let cmd = "echo hi\ncargo test | grep FAILED";
        assert!(
            detect_il3_violation(cmd).is_some(),
            "a piped cargo test on the second line is still a piped cargo test"
        );
    }

    /// The matching false-positive guard: a newline inside quotes must not become a
    /// segment boundary here either.
    #[test]
    fn il3_does_not_split_a_newline_inside_a_quoted_argument() {
        let cmd = "git commit -m \"line one\ncargo test | grep FAILED\"";
        assert!(
            detect_il3_violation(cmd).is_none(),
            "a pipeline described inside a quoted message is data, not syntax"
        );
    }

    #[test]
    fn il3_analyses_each_semicolon_separated_command_on_its_own() {
        // The second real block. Flat splitting made the left-hand side of the first
        // pipe read as `git status --short; ls docs/*.md`, i.e. "starts with git", so
        // an unrelated later segment supplied the trimmer. Neither is piped to the
        // other, and the only piping segment here has a bounded LHS.
        let cmd = "git status --short; ls docs/issues/*.md | wc -l";
        assert!(detect_il3_violation(cmd).is_none());

        // A violation in a LATER segment is still caught — the split must not become
        // a way to smuggle one past the gate.
        let blocked = detect_il3_violation("ls src; cargo test | grep FAILED")
            .expect("a real violation in a later segment must still block");
        assert!(
            blocked.contains("cargo test"),
            "the hint must name the offending segment, not the whole chain: {blocked}"
        );
    }

    #[test]
    fn il3_segment_split_does_not_confuse_redirection_or_a_lone_pipe() {
        // `2>&1` must not read as the `&&` separator...
        assert!(detect_il3_violation("cargo test 2>&1 | grep FAILED").is_some());
        // ...and `||` is a separator while a single `|` is a pipe.
        assert!(detect_il3_violation("false || cargo test | grep FAILED").is_some());
        assert!(detect_il3_violation("cargo build && ls | head -3").is_none());
    }

    /// The false positive: a `|` inside a quoted argument is data, not a pipe.
    ///
    /// `git log --grep='fix|head foo'` has no pipeline at all — git receives
    /// `fix|head foo` as one pattern. The naive `split('|')` manufactured the stage
    /// `head foo'`, whose head token reads as a trimmer, and blocked the command.
    /// The hint it printed was itself unusable: `run_command("git log --grep='fix")`
    /// carries an unterminated quote.
    #[test]
    fn il3_allows_a_quoted_pipe_inside_an_argument() {
        assert!(
            detect_il3_violation("git log --grep='fix|head foo' --oneline -3").is_none(),
            "a `|` inside a quoted argument is not a pipe"
        );
        // Double quotes too, and with the trimmer name adjacent to the quote.
        assert!(detect_il3_violation(r#"git log --grep="a|head" -3"#).is_none());
        // A quoted pipe on a command that ALSO has a real pipe must still block --
        // the fix must not turn quote-awareness into a blanket exemption.
        assert!(
            detect_il3_violation("git log --grep='a|head' | head -3").is_some(),
            "a real pipe still violates even when a quoted one precedes it"
        );
    }

    /// Pins the invariant `il3_offending_lead` relies on: it splits only on `|`,
    /// which is sound only because `pipeline_segments` consumed every unquoted `||`
    /// first. If that ever stops being true, the RHS of a logical-or starts being
    /// analysed as a pipe stage and `false || cargo test` becomes a violation.
    #[test]
    fn il3_does_not_treat_the_rhs_of_a_logical_or_as_a_pipe_stage() {
        // `head -3` here runs only if the LHS fails. Nothing is piped into it.
        assert!(
            detect_il3_violation("cargo build || head -3 log.txt").is_none(),
            "`||` is a command separator; its RHS receives no piped output"
        );
        // A quoted `||` never reaches the separator logic at all.
        assert!(detect_il3_violation("git log --grep='a||head' -3").is_none());
    }

    /// The bypass, and the more serious half of this defect: `pipeline_segments` was
    /// quote-blind too, so a quoted `;` fabricated segment boundaries that hid a
    /// genuine pipe from the enforcer.
    ///
    /// Measured 2026-08-14 through the live MCP server before the fix:
    /// `git log --oneline -50 --grep='a;b' | head -3` returned exit_code 0 -- an
    /// unbounded producer piped to a trimmer, allowed. The split at the quoted `;`
    /// left the pipe in a segment whose pre-pipe lead was the fragment `b'`, which is
    /// not an unbounded command, so the check passed.
    ///
    /// A false positive costs a retry. This cost the guarantee.
    ///
    /// **Fixture amended 2026-08-16, deliberately.** The original carried `-50`,
    /// which [`git_output_is_bounded`] now reads as a real commit-count limit — so
    /// the command became legitimately allowed and the case stopped exercising the
    /// property it exists to test. The `-50` was incidental realism; the subject is
    /// quote-aware segmentation, and dropping it keeps the producer unbounded and the
    /// quoted `;` in place.
    ///
    /// Mutation-verified: putting `git` back on `UNBOUNDED_PREFIXES` (which makes the
    /// new branch dead) turns all five `il3_allows_git_*` cases red while THIS one
    /// stays green — so the amended fixture still blocks for the reason it always did,
    /// and the new cases discriminate. The other direction is not re-run here; the
    /// pre-fix measurement above is the record for it, and the `-50` played no part in
    /// that mechanism (a quote-blind split leaves the lead `b'` either way).
    #[test]
    fn il3_still_blocks_when_a_quoted_separator_precedes_a_real_pipe() {
        assert!(
            detect_il3_violation("git log --oneline --grep='a;b' | head -3").is_some(),
            "a quoted `;` must not hide a real pipe from the check"
        );
        assert!(
            detect_il3_violation("cargo test --test 'a&&b' | head -5").is_some(),
            "a quoted `&&` must not hide a real pipe from the check"
        );
        assert!(
            detect_il3_violation("cargo test --test 'a||b' | grep FAILED").is_some(),
            "a quoted `||` must not hide a real pipe from the check"
        );
    }

    /// Control for the direction of the change. Making the splitters quote-aware
    /// strictly reduces fabricated stages, so the risk is under-blocking, not
    /// over-blocking. These are the plain violations that must never stop firing.
    #[test]
    fn il3_control_plain_violations_still_fire_after_quote_awareness() {
        for cmd in [
            "cargo test | head -50",
            "git log | grep fix",
            "rg pattern | head",
            "npm test | tail -20",
            "grep -r foo . | head",
            "find . -name '*.rs' | head",
        ] {
            assert!(
                detect_il3_violation(cmd).is_some(),
                "genuine violation stopped firing: {cmd}"
            );
        }
    }

    #[test]
    fn il3_allows_unknown_lhs_command() {
        // awk is not in LHS_COMMANDS; conservative list keeps false positives low.
        assert!(detect_il3_violation("awk '{print $1}' file.txt | head").is_none());
    }

    // ── RHS aggregators (wc, grep -c) collapse to a summary → allowed even from
    //    an unbounded LHS. Truncators/filters (head, tail, plain grep) still block.

    #[test]
    fn il3_allows_git_status_pipe_wc() {
        // Reported friction (2026-06-15): counting changed files. wc aggregates.
        assert!(detect_il3_violation("git status --porcelain | wc -l").is_none());
    }

    #[test]
    fn il3_allows_cargo_pipe_wc() {
        // Unbounded LHS, but wc collapses to a count — context-saving, allowed.
        assert!(detect_il3_violation("cargo test | wc -l").is_none());
    }

    #[test]
    fn il3_allows_grep_recursive_pipe_wc() {
        // Even a recursive (unbounded) grep piped to wc yields a count, not a trim.
        assert!(detect_il3_violation("grep -R pat src/ | wc -l").is_none());
    }

    #[test]
    fn il3_allows_fd_pipe_wc() {
        // Counting files: `fd .rs | wc -l` aggregates, not trims.
        assert!(detect_il3_violation("fd .rs | wc -l").is_none());
    }

    #[test]
    fn il3_allows_counting_grep() {
        // grep -c emits a match COUNT, not the matches — an aggregator.
        assert!(detect_il3_violation("git log --oneline | grep -c fix").is_none());
    }

    #[test]
    fn il3_allows_counting_grep_long_flag() {
        assert!(detect_il3_violation("cargo test | grep --count PASS").is_none());
    }

    #[test]
    fn il3_allows_counting_grep_bundled_flag() {
        // Bundled short flags (-ic = ignore-case + count) still read as counting.
        assert!(detect_il3_violation("cargo test | grep -ic warning").is_none());
    }

    #[test]
    fn il3_blocks_filtering_grep_still() {
        // Plain grep (no -c) hides non-matching lines — still a trim.
        let hint = detect_il3_violation("git log --oneline | grep fix").expect("should block");
        assert!(hint.contains("IL3 violation"));
    }

    #[test]
    fn il3_blocks_context_grep_still() {
        // -C (context, capital) shows lines around matches — a filter, not a count.
        assert!(detect_il3_violation("cargo test | grep -C 2 warning").is_some());
    }

    #[test]
    fn il3_blocks_git_pipe_head_still() {
        // U-16 case: head truncates — still blocked from an unbounded LHS.
        assert!(detect_il3_violation("git log --oneline master..experiments | head -20").is_some());
    }

    // ── field selectors are not trimmers (2026-08-27) ──────────────────
    //
    // `cut` and `tr` are 1:1 on records and cannot hide one. `sed`/`awk`/`sort`
    // can, and stay blocked — the control tests below are what make that a
    // measured line rather than a claim.

    #[test]
    fn il3_allows_a_field_selector_after_an_unbounded_producer() {
        // `cut` picks fields WITHIN each line; every record still arrives.
        assert!(detect_il3_violation("git show abc123 | cut -d' ' -f1").is_none());
    }

    #[test]
    fn il3_allows_tr_after_an_unbounded_producer() {
        assert!(detect_il3_violation("git branch -r --contains abc123 | tr -d ' '").is_none());
    }

    #[test]
    fn il3_still_blocks_sed_which_can_select_records() {
        // Control for the pair above: `sed -n '1,10p'` hides records, so the
        // field-selector exemption must NOT have widened to sed.
        assert!(detect_il3_violation("git show abc123 | sed -n '1,10p'").is_some());
    }

    #[test]
    fn il3_still_blocks_awk_which_can_select_records() {
        // Control: `awk NR<10` truncates. Classified on capability, not on the
        // 1:1 `{print $1}` shape it is usually called with.
        assert!(detect_il3_violation("cargo test | awk 'NR<10'").is_some());
    }

    // ── a collapsing stage bounds the whole pipeline (2026-08-27) ──────

    #[test]
    fn il3_allows_a_pipeline_that_trims_then_collapses() {
        // `git log | grep -c fix` was already allowed; this spelling delivers
        // the identical single number and was refused. Same information
        // reaching the agent, so the same verdict.
        assert!(detect_il3_violation("git log | grep fix | wc -l").is_none());
    }

    #[test]
    fn il3_allows_a_trimmer_downstream_of_a_collapsing_stage() {
        // `git patch-id` reduces an arbitrary diff to one line; `head -1` on one
        // line hides nothing.
        assert!(
            detect_il3_violation("git show abc123 | git patch-id --stable | head -1").is_none()
        );
    }

    #[test]
    fn il3_allows_a_digest_stage_to_bound_the_pipeline() {
        assert!(detect_il3_violation("git show abc123 | sha256sum | cut -c1-8").is_none());
    }

    #[test]
    fn il3_still_blocks_when_nothing_in_the_pipeline_collapses() {
        // Control for the three above: `sort -u` reduces, but not to a bounded
        // summary — an arbitrarily large stream can survive it.
        assert!(detect_il3_violation("cargo test | grep FAILED | sort -u").is_some());
    }

    // ── single-line git plumbing (2026-08-27) ──────────────────────────

    #[test]
    fn il3_allows_single_line_git_plumbing_piped_to_a_trimmer() {
        // 40 characters. There is no limiter flag for `rev-parse` to carry, so
        // the limiter heuristic classified it unbounded forever.
        assert!(detect_il3_violation("git rev-parse HEAD | head -1").is_none());
        assert!(detect_il3_violation("git merge-base master experiments | head -1").is_none());
        assert!(detect_il3_violation("git describe --tags | head -1").is_none());
    }

    #[test]
    fn il3_still_blocks_rev_parse_when_it_enumerates_refs() {
        // Control: `--all` walks every ref, so rev-parse is only single-line in
        // its single-value spelling.
        assert!(detect_il3_violation("git rev-parse --all | head -20").is_some());
    }

    #[test]
    fn il3_still_blocks_git_config_listing_the_whole_file() {
        // Control: `git config --get x` is one value; `--list` dumps the file.
        assert!(detect_il3_violation("git config --get user.email | head -1").is_none());
        assert!(detect_il3_violation("git config --list | grep user").is_some());
    }

    #[test]
    fn il3_allows_the_patch_id_workflow_tracker_conventions_mandates() {
        // The reported case, end to end. `get_guide("tracker-conventions")`
        // requires recording a patch-id beside a fix SHA; the guard blocked the
        // command and its error text then recommended an @cmd_* buffer, which
        // truncates — yielding a syntactically perfect WRONG hash.
        // BUG docs/issues/archive/2026-08-27-il3-blocks-already-collapsed-pipelines-and-its-remedy-yields-a-wrong-hash.md
        for cmd in [
            "git show abc123 | git patch-id --stable",
            "git show abc123 | git patch-id --stable | cut -d' ' -f1",
            "git show abc123 | cut -d' ' -f1 | wc -l",
            "git patch-id --stable < /tmp/x.patch | awk '{print $1}'",
        ] {
            assert!(
                detect_il3_violation(cmd).is_none(),
                "should be allowed but was blocked: {cmd}"
            );
        }
    }

    // ── diagnostics: localize the segment, distrust the buffer (2026-08-27) ──

    #[test]
    fn il3_error_names_the_offending_segment_not_the_whole_script() {
        // A multi-statement script used to be echoed back whole as "the thing
        // piped to a log-trimmer" — including a trailing segment containing no
        // pipe at all — leaving the reader to locate the real pipe themselves.
        let hint = detect_il3_violation(
            "echo start; cargo test | grep FAILED; git branch --contains abc123",
        )
        .expect("should block");
        assert!(
            hint.contains("piped `cargo test | grep FAILED` to a log-trimmer"),
            "should quote only the offending segment, got: {hint}"
        );
        assert!(
            !hint.contains("git branch --contains abc123"),
            "innocent trailing segment must not be echoed as the offender: {hint}"
        );
    }

    #[test]
    fn il3_error_warns_that_the_buffer_it_recommends_is_capped() {
        // The remedy this error prescribes routes the caller to an @cmd_* buffer
        // that may hold only a prefix. For a grep that is a partial answer; for a
        // hash it is a confident wrong one, in the exact workflow
        // `get_guide("tracker-conventions")` mandates.
        let hint = detect_il3_violation("cargo test | grep FAILED").expect("should block");
        assert!(
            hint.contains("unfiltered_truncated"),
            "must name the flag: {hint}"
        );
        assert!(
            hint.contains("WRONG"),
            "must not soften the hash failure: {hint}"
        );
        assert!(
            hint.contains("git show <sha> > /tmp/x.patch"),
            "must offer the redirect pattern as the whole-input remedy: {hint}"
        );
    }

    // ── classify_write_path tests ──────────────────────────────────────

    #[test]
    fn classify_in_project_is_allowed() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let cfg = PathSecurityConfig::default();
        let decision = classify_write_path("sub/file.rs", root, &cfg, &[]);
        assert!(
            matches!(decision, WritePathDecision::Allowed(_)),
            "got: {decision:?}"
        );
    }

    #[test]
    fn classify_outside_root_is_outsideroot() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let cfg = PathSecurityConfig::default();
        // /var is outside the project root, the temp dir, and cwd.
        let decision = classify_write_path("/var/ce_classify_test/x.rs", root, &cfg, &[]);
        assert!(
            matches!(decision, WritePathDecision::OutsideRoot { .. }),
            "got: {decision:?}"
        );
    }

    /// A write denial must hand back a corrective action the reader can *execute*, not
    /// merely the name of a tool.
    ///
    /// The message used to read `Call approve_write('<dir>')`, which fails twice over: the
    /// directory is a literal placeholder, and `approve_write` has no positional form — it
    /// takes a named `path`. An agent following it verbatim earned a second error, which
    /// is the mechanism behind the measured behaviour: 26% of write denials are followed
    /// by retrying the same denied write, the highest immediate-repeat rate in the corpus,
    /// against 3% for `il3_pipe_to_trimmer` whose message carries a concrete action.
    ///
    /// See `docs/issues/archive/2026-08-15-write-scope-denial-does-not-name-approve-write.md`.
    #[test]
    fn write_denial_names_an_approve_write_call_that_can_be_run_verbatim() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = PathSecurityConfig::default();
        let err = validate_write_path("/var/ce_denial_msg/x.rs", tmp.path(), &cfg, &[])
            .expect_err("a path outside the project root must be denied");
        let msg = err.to_string();

        // Keeps `usage::db::normalize_err_family` classifying this as write_scope_denied —
        // the family the 26% was measured over.
        assert!(msg.contains("write denied"), "{msg}");

        // The actual directory, not a placeholder. This is the whole fix.
        assert!(
            msg.contains("/var/ce_denial_msg"),
            "the denial must name the directory to approve, which OutsideRoot already \
             carries: {msg}"
        );
        assert!(
            !msg.contains("<dir>"),
            "a placeholder is not a corrective action: {msg}"
        );

        // The named-parameter form, because the positional one does not exist.
        assert!(
            msg.contains("approve_write(path="),
            "approve_write takes a named `path`; a positional call shape sends the reader \
             into a second error: {msg}"
        );
    }

    /// The hard denials share the `write_scope_denied` family with the approvable case, so
    /// they must say that approving will not help — otherwise a reader who has learned
    /// "write denied → approve_write" spends a call finding out.
    ///
    /// **Unix-only, and that is a platform semantic rather than a skip.** The fixture needs
    /// `..` to SURVIVE canonicalization, which happens only when an intermediate directory
    /// does not exist. Windows normalizes `..` lexically instead, without consulting the
    /// filesystem, so this input lands on `<root>/escape.rs` and is allowed — measured under
    /// wine 2026-08-26 as `Allowed("\\?\C:\…\.tmpC5EDa4\escape.rs")`. The *decision* is
    /// correct on both platforms; only this arm's MESSAGE contract is unreachable on Windows,
    /// and the sibling below pins the difference instead of leaving it implicit.
    #[cfg(not(windows))]
    #[test]
    fn hard_denials_say_that_approve_write_will_not_help() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = PathSecurityConfig::default();

        // `..` only survives canonicalization when an intermediate directory does not
        // exist — the branch's own comment says so, and `/var/..` resolves cleanly, so a
        // path built from real directories takes the OutsideRoot arm instead.
        let decision = classify_write_path("no-such-dir-xyz/../escape.rs", tmp.path(), &cfg, &[]);
        let WritePathDecision::Denied(msg) = decision else {
            // If this ever classifies differently the message contract still needs a home;
            // fail loudly rather than skipping the assertion — and NAME what it got, since
            // "not Denied" has more than one value and a bare panic sends the reader off to
            // re-derive which arm fired. Windows is where this bites, and the answer there
            // may be that a different fixture is needed rather than a different assertion.
            panic!("an unresolved '..' must be a hard Denied, not approvable; got: {decision:?}");
        };
        assert!(
            msg.contains("approve_write cannot grant this"),
            "an unapprovable denial must say so: {msg}"
        );
    }

    /// The Windows half of the split above. Pinned rather than skipped: if Windows ever
    /// stops normalizing `..` lexically, the unresolved-`..` arm becomes reachable there,
    /// this test fails, and that is the signal to re-unify the two rather than a mystery.
    #[cfg(windows)]
    #[test]
    fn windows_resolves_dotdot_lexically_so_the_unresolved_arm_is_unreachable() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = PathSecurityConfig::default();
        let decision = classify_write_path("no-such-dir-xyz/../escape.rs", tmp.path(), &cfg, &[]);
        assert!(
            matches!(&decision, WritePathDecision::Allowed(p) if p.ends_with("escape.rs")),
            "Windows normalizes `..` without touching the filesystem, so this must resolve \
             INSIDE the root and be allowed: {decision:?}"
        );
    }

    #[test]
    fn classify_empty_is_denied() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = PathSecurityConfig::default();
        let decision = classify_write_path("", tmp.path(), &cfg, &[]);
        assert!(
            matches!(decision, WritePathDecision::Denied(_)),
            "got: {decision:?}"
        );
    }

    #[test]
    fn validate_write_path_still_bails_outside_with_unchanged_message() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = PathSecurityConfig::default();
        let err = validate_write_path("/var/ce_classify_test/x.rs", tmp.path(), &cfg, &[])
            .unwrap_err()
            .to_string();
        assert!(err.contains("is outside the project root"), "got: {err}");
        assert!(err.contains("Call approve_write"), "got: {err}");
    }

    // ── Commit-message backtick gate ─────────────────────────────────────
    //
    // Every fixture below is a shape taken from `.codescout/usage.db` (9,790
    // `run_command` calls), not invented. The gate's whole difficulty is that the
    // dangerous shape and the safe one differ only in quoting.

    /// The confirmed instance. `da5176d5`'s committed message permanently reads
    /// `per memory  §` because this exact shape ran `conventions` as a command,
    /// matched nothing, and substituted empty. No error, no diagnostic.
    #[test]
    fn commit_backtick_gate_flags_the_shape_that_silently_mangled_a_real_commit() {
        let cmd = r#"git commit -m "per memory `conventions` § Environment-Agnostic Tuning""#;
        assert_eq!(
            commit_message_backtick_hazard(cmd).as_deref(),
            Some("conventions"),
            "the gate must name the text the shell would execute"
        );
    }

    /// The dominant safe convention in the corpus — heredoc into a file, then
    /// `git commit -F`. 283 of the 291 backtick-bearing `git commit` calls protect their
    /// message this way, so flagging the shape would reject correct commands two orders
    /// of magnitude more often than it catches a defect.
    ///
    /// Be precise about what guards it, because a mutation run said otherwise: this
    /// fixture SURVIVES deleting `strip_heredoc_bodies` (checked 2026-08-19), since `-F`
    /// carries no message flag and the scope test returns first. Heredoc stripping is
    /// pinned by the sibling below, which uses `-m`. Kept as the `-F` scope control.
    #[test]
    fn commit_backtick_gate_ignores_a_quoted_heredoc_message() {
        let cmd = "cat > /tmp/m <<'EOF'\nfixes `foo::bar` in `src/x.rs`\nEOF\ngit commit -F /tmp/m";
        assert_eq!(commit_message_backtick_hazard(cmd), None);
    }

    /// The message flag IS present here; the backticks are confined to a heredoc body
    /// the shell never evaluates. Stripping has to happen before the flag test.
    #[test]
    fn commit_backtick_gate_ignores_backticks_confined_to_a_heredoc_body() {
        let cmd = "git commit -m \"$(cat <<'EOF'\ncites `symbols` and `edit_code`\nEOF\n)\"";
        assert_eq!(commit_message_backtick_hazard(cmd), None);
    }

    /// The workaround already in use in the corpus — one of the two exposed calls used
    /// it deliberately. Flagging it would punish the fix.
    #[test]
    fn commit_backtick_gate_ignores_backslash_escaped_backticks() {
        let cmd = r#"git commit -m "labels dbaeb78b as an \`experiments\` SHA""#;
        assert_eq!(commit_message_backtick_hazard(cmd), None);
    }

    /// Single quotes suppress substitution outright — the shell does not even expand
    /// parameters inside them.
    #[test]
    fn commit_backtick_gate_ignores_single_quoted_backticks() {
        let cmd = "git commit -m 'touches `foo` and `bar`'";
        assert_eq!(commit_message_backtick_hazard(cmd), None);
    }

    /// Scope control, and the reason the gate is narrow: outside a commit message a
    /// backtick may be a deliberate substitution, while 0 of 291 commit messages ever
    /// wanted one. The gate refuses only where the measurement says refusing is safe.
    #[test]
    fn commit_backtick_gate_ignores_substitution_outside_a_commit_message() {
        assert_eq!(commit_message_backtick_hazard("echo `date +%Y`"), None);
        assert_eq!(commit_message_backtick_hazard("git log --oneline -1"), None);
        assert_eq!(
            commit_message_backtick_hazard("git commit -F /tmp/msg"),
            None
        );
    }

    /// Flag spellings that all reach the same hazard. `-am` is the one a plain `-m`
    /// substring test misses, and it is ordinary usage.
    #[test]
    fn commit_backtick_gate_covers_every_message_flag_spelling() {
        for cmd in [
            r#"git commit -m "cites `x`""#,
            r#"git commit -am "cites `x`""#,
            r#"git commit -a -m "cites `x`""#,
            r#"git commit --message "cites `x`""#,
            r#"git commit --message="cites `x`""#,
        ] {
            assert_eq!(
                commit_message_backtick_hazard(cmd).as_deref(),
                Some("x"),
                "missed the message flag in: {cmd}"
            );
        }
    }

    // ── Heredoc masking (offset-preserving) ──────────────────────────────

    /// The invariant the function exists for. `strip_heredoc_bodies` REMOVES lines, which
    /// moves every later index; a caller that splices at a returned offset would write
    /// into the wrong place. Masking must be length-exact, trailing newline or not, and
    /// multi-byte characters are where a naive one-space-per-char version breaks.
    #[test]
    fn mask_heredoc_bodies_preserves_byte_offsets() {
        for cmd in [
            "cat > f <<'EOF'\na | b\nEOF",
            "cat > f <<'EOF'\na | b\nEOF\n",
            "echo hi",
            "cat > f <<'EOF'\nünïcodé — pipe → ok\nEOF",
        ] {
            assert_eq!(
                mask_heredoc_bodies(cmd).len(),
                cmd.len(),
                "offset drift on: {cmd:?}"
            );
        }
    }

    #[test]
    fn mask_heredoc_bodies_blanks_a_pipe_in_the_body() {
        let cmd = "cat > f <<'EOF'\na | grep b\nEOF";
        let masked = mask_heredoc_bodies(cmd);
        assert!(!masked.contains('|'), "body pipe survived: {masked:?}");
        assert!(
            masked.starts_with("cat > f <<'EOF'"),
            "the opener line must survive: {masked:?}"
        );
    }

    /// A pipe on the OPENER line is real pipeline syntax. Keeping it is what separates
    /// this fix from the cheaper "skip instrumentation whenever `<<` appears" — that one
    /// would trade a corruption bug for silently losing unfiltered capture.
    #[test]
    fn mask_heredoc_bodies_keeps_a_pipe_outside_the_body() {
        let cmd = "cat <<'EOF' | grep x\nbody | not a pipe\nEOF";
        let masked = mask_heredoc_bodies(cmd);
        assert!(masked.starts_with("cat <<'EOF' | grep x"), "{masked:?}");
        assert_eq!(
            masked.matches('|').count(),
            1,
            "only the real pipe may survive: {masked:?}"
        );
    }

    #[test]
    fn mask_heredoc_bodies_leaves_a_command_without_a_heredoc_untouched() {
        let cmd = "git log --all -p | grep foo";
        assert_eq!(mask_heredoc_bodies(cmd).as_ref(), cmd);
    }

    /// Both halves together — this is the question `inject_tee` actually asks. The first
    /// assertion is a **precondition, not decoration**: it pins that the raw string really
    /// does fool the detector, so a later change that stopped reproducing the bug would
    /// fail here rather than leave the second assertion passing vacuously.
    #[test]
    fn masking_hides_body_pipes_from_terminal_filter_detection() {
        use crate::tools::command_summary::detect_terminal_filter;
        let cmd = "cat > f <<'EOF'\n- Resolve: git log -p | git patch-id | grep abc\nEOF";
        assert!(
            detect_terminal_filter(cmd).is_some(),
            "precondition: the raw string is what fooled the detector"
        );
        assert!(
            detect_terminal_filter(&mask_heredoc_bodies(cmd)).is_none(),
            "masking must hide the body pipe from the detector"
        );
    }
}
