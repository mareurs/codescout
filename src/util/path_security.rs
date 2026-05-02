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
//! Violations return [`anyhow::Error`] wrapping a [`crate::tools::RecoverableError`],
//! which the MCP layer surfaces as `isError: false` with a corrective hint.
//! This means a write-boundary violation does **not** abort sibling parallel
//! tool calls — the agent can recover and continue without user intervention.

use anyhow::{bail, Result};
use regex::Regex;
use std::path::{Path, PathBuf};

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
    /// Max bytes for shell command stdout/stderr (default 100KB)
    pub shell_output_limit_bytes: usize,
    /// Enable shell command execution (default: false)
    pub shell_enabled: bool,
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
    /// When true, `edit_file` on source code files returns a RecoverableError
    /// directing callers to `edit_code` instead. Debug/enforcement flag.
    pub debug_enforce_symbol_tools: bool,
}

impl Default for PathSecurityConfig {
    fn default() -> Self {
        Self {
            profile: SecurityProfile::Default,
            extra_write_roots: Vec::new(),
            shell_command_mode: "warn".into(),
            shell_output_limit_bytes: 100 * 1024,
            shell_enabled: true,
            file_write_enabled: true,
            indexing_enabled: true,
            library_paths: Vec::new(),
            shell_dangerous_patterns: Vec::new(),
            max_index_bytes: 500 * 1024 * 1024,
            debug_enforce_symbol_tools: false,
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
    denied
        .iter()
        .any(|d| resolved.starts_with(d) || resolved == d.as_path())
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
        } else {
            project_root.join(raw)
        };
        return Ok(canonicalize_write_target(&resolved));
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
        bail!(
            "write denied: '{}' contains '..' that could not be resolved",
            raw
        );
    }

    let project_root = best_effort_canonicalize(project_root);

    // Check deny-list first (blocks writes to ~/.ssh even if somehow under
    // an extra_write_root).
    let denied = denied_read_paths(config);
    if is_denied(&resolved, &denied) {
        bail!("write denied: '{}' is in a protected location", raw);
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

    let under_allowed_root = allowed.iter().any(|root| resolved.starts_with(root));
    if !under_allowed_root {
        bail!("write denied: '{}' is outside the project root", raw);
    }

    Ok(resolved)
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

// ---------------------------------------------------------------------------
// Tool access controls
// ---------------------------------------------------------------------------

/// Check if a tool is allowed by the current security configuration.
/// Returns Ok(()) if allowed, or an error message explaining how to enable it.
pub fn check_tool_access(tool_name: &str, config: &PathSecurityConfig) -> Result<()> {
    match tool_name {
        "run_command" => {
            if !config.shell_enabled {
                bail!(
                    "Shell commands are disabled. Set security.shell_enabled = true in .codescout/project.toml to enable."
                );
            }
        }
        "create_file" | "edit_file" | "edit_code" | "library" | "edit_markdown" => {
            if !config.file_write_enabled {
                bail!(
                    "File writes are disabled for this project. If this project was activated in read-only mode, call workspace(action='activate', read_only: false) to enable writes."
                );
            }
        }
        "semantic_search" | "index" => {
            if !config.indexing_enabled {
                bail!(
                    "Indexing tools are disabled. Set security.indexing_enabled = true in .codescout/project.toml to enable."
                );
            }
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

/// Check if a command matches a dangerous pattern.
///
/// Returns the matched pattern description if dangerous, `None` if safe.
pub fn is_dangerous_command(command: &str, config: &PathSecurityConfig) -> Option<String> {
    if config.profile == SecurityProfile::Root {
        return None;
    }

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
        if re.is_match(command) {
            return Some(description.to_string());
        }
    }

    // Check user-configured dangerous patterns.
    for pattern in &config.shell_dangerous_patterns {
        if let Ok(re) = Regex::new(pattern) {
            if re.is_match(command) {
                return Some(format!("matches custom pattern: {}", pattern));
            }
        }
    }

    None
}

/// Source file extensions that should be accessed via codescout tools,
/// not raw shell commands. Mirrors `crate::ast::detect_language()` minus markdown.
const SOURCE_EXTENSIONS: &str = r"\.(rs|py|ts|tsx|js|cjs|mjs|jsx|go|java|kt|kts|c|cpp|cc|cxx|cs|rb|php|swift|scala|ex|exs|hs|lua|sh|bash)\b";

/// Shell commands whose primary job is reading file content.
const SOURCE_ACCESS_COMMANDS: &str = r"\b(cat|head|tail|sed|awk|less|more|wc|grep)\b";

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
fn extract_grep_pattern(segment: &str) -> Option<&str> {
    let mut skip_next = false;
    for token in segment.split_whitespace().skip(1) {
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
        return Some(token.trim_matches('"').trim_matches('\''));
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
/// Known limits:
/// - Variable expansion (`cat $FILE`) is undetectable at parse time — accepted.
/// - Heredocs (`cat <<'EOF'`) read stdin, not a file; any source extension appearing
///   inside the heredoc body is not a filename argument. Segments containing `<<` are
///   skipped — the operator unambiguously means stdin redirection.
pub fn check_source_file_access(command: &str) -> Option<String> {
    static CMD_RE: std::sync::OnceLock<Option<Regex>> = std::sync::OnceLock::new();
    static EXT_RE: std::sync::OnceLock<Option<Regex>> = std::sync::OnceLock::new();
    let cmd_re = CMD_RE
        .get_or_init(|| Regex::new(SOURCE_ACCESS_COMMANDS).ok())
        .as_ref()?;
    let ext_re = EXT_RE
        .get_or_init(|| Regex::new(SOURCE_EXTENSIONS).ok())
        .as_ref()?;

    // Split on compound-command operators and pipes, respecting quoted strings.
    // Order: "&&"/"||" before "|" so that "||" is not mis-split as two "|" tokens.
    let segments = split_outside_quotes(command, &["&&", "||", ";", "|"]);

    let blocked = segments.iter().find(|seg| {
        // Heredoc: the command reads from stdin, not a source file.
        if seg.contains("<<") {
            return false;
        }
        // Only the *first token* of a segment is the actual command being executed.
        // Matching against the first token (not the full segment string) prevents
        // false positives from quoted arguments containing command names, e.g.:
        //   git commit -m "feat: tail-50 of log, output_buffer.rs"
        let first_token = seg.split_whitespace().next().unwrap_or("");
        if !cmd_re.is_match(first_token) {
            return false;
        }
        // Check the full segment for a source extension so that quoted file paths
        // (e.g. `cat "src/main.rs"`) are still caught.
        ext_re.is_match(seg.as_str())
    })?;

    // Derive the hint from the specific command that triggered the block.
    let first_cmd = blocked.split_whitespace().next().unwrap_or("");
    let hint: String = match first_cmd {
        "grep" => {
            let pat = extract_grep_pattern(blocked.as_str()).unwrap_or("");
            if is_identifier_pattern(pat) {
                let name = pat.split('|').next().unwrap_or(pat);
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
        let result = validate_write_path("", dir.path(), &default_config());
        assert!(result.is_err());
    }

    #[test]
    fn write_null_byte_rejected() {
        let dir = tempdir().unwrap();
        let result = validate_write_path("file\0evil", dir.path(), &default_config());
        assert!(result.is_err());
    }

    #[test]
    fn write_within_project_allowed() {
        let dir = tempdir().unwrap();
        // Create the target directory so canonicalize resolves properly
        std::fs::create_dir_all(dir.path().join("src")).unwrap();

        let result = validate_write_path("src/new.rs", dir.path(), &default_config());
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

        let result = validate_write_path(target, project.path(), &default_config());
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
        let result = validate_write_path("../../../var/evil.rs", project.path(), &default_config());
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
        let result = validate_write_path(target.to_str().unwrap(), project.path(), &config);
        assert!(result.is_ok());
    }

    #[test]
    fn write_to_tmp_allowed() {
        let project = tempdir().unwrap();
        // /tmp itself must exist on the system for this test to be meaningful
        let target = PathBuf::from("/tmp/codescout-test-write.txt");
        let result =
            validate_write_path(target.to_str().unwrap(), project.path(), &default_config());
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
            );
            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("protected location"));
        }
    }

    // ── Symlink resolution ───────────────────────────────────────────────

    #[test]
    fn symlink_to_denied_path_is_caught_on_read() {
        if let Some(home) = home_dir() {
            let ssh_dir = home.join(".ssh");
            if !ssh_dir.exists() {
                return; // skip if no .ssh directory
            }

            let dir = tempdir().unwrap();
            let link = dir.path().join("sneaky_link");
            #[cfg(unix)]
            {
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
        }
    }

    #[test]
    fn symlink_write_escape_caught() {
        let project = tempdir().unwrap();

        // Create symlink inside the project pointing to /var/tmp — a real
        // directory that is outside both the project root and /tmp, so the
        // path-security check should still block the write.
        let link = project.path().join("sneaky");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/var/tmp", &link).unwrap();
            let result =
                validate_write_path("sneaky/escaped.txt", project.path(), &default_config());
            // After canonicalization the symlink resolves to /var/tmp/escaped.txt
            // which is outside both the project root and /tmp.
            assert!(result.is_err());
        }
    }

    // ── Tool access controls ─────────────────────────────────────────────

    #[test]
    fn shell_enabled_by_default() {
        let config = PathSecurityConfig::default();
        assert!(config.shell_enabled);
        assert!(check_tool_access("run_command", &config).is_ok());
    }

    #[test]
    fn shell_disabled_when_configured() {
        let config = PathSecurityConfig {
            shell_enabled: false,
            ..PathSecurityConfig::default()
        };
        assert!(check_tool_access("run_command", &config).is_err());
    }

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
            shell_enabled: false,
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
            "list_functions",
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
    fn check_tool_access_error_message_includes_config_hint() {
        let config = PathSecurityConfig {
            shell_enabled: false,
            ..PathSecurityConfig::default()
        };
        let err = check_tool_access("run_command", &config).unwrap_err();
        assert!(
            err.to_string().contains("shell_enabled"),
            "error should mention config key"
        );
        assert!(
            err.to_string().contains("project.toml"),
            "error should mention config file"
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
        assert!(check_source_file_access("cat src/main.rs").is_some());
    }

    #[test]
    fn source_file_access_blocks_head_on_ts() {
        assert!(check_source_file_access("head -20 src/tools/mod.ts").is_some());
    }

    #[test]
    fn source_file_access_blocks_tail_on_go() {
        assert!(check_source_file_access("tail -n 50 server.go").is_some());
    }

    #[test]
    fn source_file_access_blocks_sed_on_py() {
        assert!(check_source_file_access("sed -n '1,100p' lib.py").is_some());
    }

    #[test]
    fn source_file_access_blocks_awk_on_java() {
        assert!(check_source_file_access("awk '{print}' Foo.java").is_some());
    }

    #[test]
    fn source_file_access_blocks_less_on_rs() {
        assert!(check_source_file_access("less src/agent.rs").is_some());
    }

    #[test]
    fn source_file_access_blocks_wc_on_rs() {
        assert!(check_source_file_access("wc -l src/lib.rs").is_some());
    }

    #[test]
    fn source_file_access_allows_cat_on_markdown() {
        // markdown is excluded — it's not source code
        assert!(check_source_file_access("cat README.md").is_none());
    }

    #[test]
    fn source_file_access_allows_wc_on_txt() {
        assert!(check_source_file_access("wc -l output.txt").is_none());
    }

    #[test]
    fn source_file_access_allows_sed_on_toml() {
        assert!(check_source_file_access("sed 's/foo/bar/g' config.toml").is_none());
    }

    #[test]
    fn source_file_access_allows_cat_without_source_ext() {
        assert!(check_source_file_access("cat Makefile").is_none());
    }

    #[test]
    fn source_file_access_hint_mentions_read_file() {
        let hint = check_source_file_access("cat src/main.rs").unwrap();
        assert!(
            hint.contains("read_file"),
            "hint should mention read_file, got: {hint}"
        );
    }

    #[test]
    fn source_file_access_hint_mentions_symbols() {
        let hint = check_source_file_access("head -5 lib.rs").unwrap();
        assert!(
            hint.contains("symbols"),
            "hint should mention symbols, got: {hint}"
        );
    }

    #[test]
    fn grep_on_source_with_identifier_gives_symbol_ladder() {
        let hint = check_source_file_access("grep WriteMemory src/tools/memory.rs").unwrap();
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
        let hint = check_source_file_access("grep 'foo.*bar' src/main.rs").unwrap();
        assert!(hint.contains("grep(pattern"), "got: {hint}");
        // must NOT show symbol ladder for regex patterns
        assert!(!hint.contains("call_graph"), "got: {hint}");
    }

    #[test]
    fn grep_pipe_alternation_uses_first_part_in_hint() {
        let hint =
            check_source_file_access("grep 'WriteMemory|ReadMemory' src/tools/memory.rs").unwrap();
        assert!(hint.contains("symbols(name='WriteMemory')"), "got: {hint}");
    }

    #[test]
    fn grep_value_taking_flag_skipped_for_identifier() {
        let hint = check_source_file_access("grep -A 3 WriteMemory src/tools/memory.rs").unwrap();
        assert!(hint.contains("symbols(name='WriteMemory')"), "got: {hint}");
    }

    #[test]
    fn source_file_access_sed_hint_mentions_grep() {
        let hint = check_source_file_access("sed -n '1p' foo.ts").unwrap();
        assert!(
            hint.contains("grep"),
            "sed hint should mention grep, got: {hint}"
        );
    }

    #[test]
    fn source_file_access_allows_non_blocked_command() {
        // cp, mv, diff are not in the blocked command set
        assert!(check_source_file_access("cp src/main.rs src/main2.rs").is_none());
    }

    #[test]
    fn source_file_access_allows_git_diff_piped_to_head() {
        // `head` is in the second segment; the `.rs` file is in the first (git diff arg).
        // Per-segment check means this should NOT be blocked.
        assert!(check_source_file_access("git diff src/server.rs | head -80").is_none());
    }

    #[test]
    fn source_file_access_blocks_cat_in_same_segment_as_source_file() {
        // `cat` and `.rs` are in the same segment — still blocked.
        assert!(check_source_file_access("cat src/main.rs | grep fn").is_some());
    }

    #[test]
    fn source_file_access_allows_cat_heredoc_with_source_ext_in_content() {
        // `cat <<'EOF'` reads stdin via a heredoc — the `.rs` extension appears
        // only in the heredoc body (e.g. a commit message), not as a filename
        // argument to cat. The `<<` operator marks the segment as stdin-reading
        // so it must not be blocked.
        assert!(check_source_file_access(
            "git commit -m \"$(cat <<'EOF'\nFix bug in path_security.rs\nEOF\n)\""
        )
        .is_none());
    }

    #[test]
    fn source_file_access_blocks_cat_rs_file_after_heredoc_segment() {
        // A pipe AFTER a heredoc segment must still be checked independently.
        // `cat <<'EOF' ... EOF | cat src/main.rs` — second segment is a real read.
        assert!(check_source_file_access("cat <<'EOF'\nhello\nEOF\n | cat src/main.rs").is_some());
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
        assert!(check_source_file_access(
            r#"git commit -m "feat: tail-50 of log, output_buffer.rs, workflow.rs""#
        )
        .is_none());
    }

    #[test]
    fn git_commit_with_ampersand_and_source_in_message_not_blocked() {
        // "&&" and "cat src/main.rs" inside the quoted message — must NOT block
        assert!(
            check_source_file_access(r#"git commit -m "fix && cat src/main.rs was broken""#)
                .is_none()
        );
    }

    #[test]
    fn compound_and_then_cat_blocked() {
        // cat src/main.rs is a real command after &&
        assert!(check_source_file_access("./build.sh && cat src/main.rs").is_some());
    }

    #[test]
    fn semicolon_then_cat_blocked() {
        assert!(check_source_file_access("echo done; cat src/main.rs").is_some());
    }

    #[test]
    fn or_then_tail_blocked() {
        assert!(check_source_file_access("cargo build || tail src/lib.rs").is_some());
    }

    #[test]
    fn pipe_chain_with_source_blocked() {
        // tail is the first token of its segment — blocked
        assert!(check_source_file_access("tail src/main.rs | grep error").is_some());
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

        let result = validate_write_path(target.to_str().unwrap(), &project_root, &config);
        assert!(result.is_ok(), "root profile should bypass write boundary");
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
}
