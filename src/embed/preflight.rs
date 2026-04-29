//! Preflight check for `index_project`: scope guard against pathologically broad
//! roots (home dir, system paths) and oversized source trees. Triggers an MCP
//! elicitation in the caller when confirmation is required.

use std::path::{Path, PathBuf};

/// Why a path is considered broad enough to warrant confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuspiciousReason {
    /// Root exactly matches the user's home directory.
    HomeDirectory,
    /// Root is the parent of the user's home directory (e.g. `/home`).
    HomeParent,
    /// Root is a known system path (`/`, `/usr`, `/etc`, ...).
    SystemPath(PathBuf),
}

/// Summary produced by the preflight scan — used to build the elicitation message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightInfo {
    pub root: PathBuf,
    pub file_count: usize,
    pub approx_bytes: u64,
    pub suspicious_reason: Option<SuspiciousReason>,
    pub size_exceeds_threshold: bool,
}

/// Verdict returned by [`check_index_scope`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreflightVerdict {
    /// Proceed to `build_index` — no confirmation needed.
    Clear,
    /// Caller must elicit confirmation from the user before proceeding.
    RequiresConfirmation(PreflightInfo),
}

/// Human-readable byte size. Always 1 decimal place, KB/MB/GB.
#[allow(dead_code)] // used in Task 4 (check_index_scope walker)
pub(crate) fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Classify a root path against the known-broad list.
/// Returns `None` for ordinary project directories.
pub(crate) fn classify_path(root: &Path) -> Option<SuspiciousReason> {
    let canon = crate::platform::canonicalize_or(root);

    if let Some(home) = crate::platform::home_dir() {
        let home_canon = crate::platform::canonicalize_or(&home);
        if canon == home_canon {
            return Some(SuspiciousReason::HomeDirectory);
        }
        if let Some(parent) = home_canon.parent() {
            if canon == parent && canon != Path::new("/") {
                return Some(SuspiciousReason::HomeParent);
            }
        }
    }

    for sys in crate::platform::system_path_prefixes() {
        let sys_path = Path::new(sys);
        let sys_canon = crate::platform::canonicalize_or(sys_path);
        if canon == sys_canon {
            return Some(SuspiciousReason::SystemPath(canon.clone()));
        }
    }

    None
}

/// Preflight scan: walk `root` (respecting `.gitignore` and hidden-file rules,
/// matching `build_index`'s walker), accumulate file count and approximate
/// source-byte total, then compare against `max_bytes` and classify the root.
///
/// Returns `PreflightVerdict::Clear` if neither trigger fires — caller
/// proceeds to `build_index`. Otherwise returns
/// `RequiresConfirmation(PreflightInfo)`; caller must elicit user confirmation.
///
/// Per-file `metadata()` errors are silently skipped (matching `WalkBuilder::flatten`).
/// Only failure to read the root itself propagates as an error.
pub fn check_index_scope(
    root: &Path,
    max_bytes: u64,
    ignored_paths: &[String],
) -> anyhow::Result<PreflightVerdict> {
    if !root.exists() {
        anyhow::bail!("project root does not exist: {}", root.display());
    }

    let ignored = ignored_paths.to_vec();
    let walker = ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .filter_entry(move |entry| {
            let name = entry.file_name().to_string_lossy();
            !ignored.iter().any(|p| p.as_str() == name.as_ref())
        })
        .build();

    let mut file_count: usize = 0;
    let mut approx_bytes: u64 = 0;

    for entry in walker.flatten() {
        let Some(ftype) = entry.file_type() else {
            continue;
        };
        if !ftype.is_file() {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        file_count += 1;
        approx_bytes = approx_bytes.saturating_add(meta.len());
    }

    let suspicious_reason = classify_path(root);
    let size_exceeds_threshold = approx_bytes > max_bytes;

    if suspicious_reason.is_none() && !size_exceeds_threshold {
        return Ok(PreflightVerdict::Clear);
    }

    let canonical_root = crate::platform::canonicalize_or(root);

    Ok(PreflightVerdict::RequiresConfirmation(PreflightInfo {
        root: canonical_root,
        file_count,
        approx_bytes,
        suspicious_reason,
        size_exceeds_threshold,
    }))
}

impl PreflightInfo {
    /// Build the human-readable confirmation message shown in the elicitation
    /// dialog. Lines that don't apply (no suspicious reason, etc.) are omitted.
    pub fn elicitation_message(&self) -> String {
        let mut lines: Vec<String> = Vec::new();

        let header = if self.size_exceeds_threshold && self.suspicious_reason.is_some() {
            "⚠ Index scope confirmation required (broad root + large size)"
        } else if self.size_exceeds_threshold {
            "⚠ Large index scope detected"
        } else {
            "⚠ Broad index scope detected"
        };
        lines.push(header.to_string());
        lines.push(String::new());

        let root_line = match &self.suspicious_reason {
            Some(SuspiciousReason::HomeDirectory) => {
                format!("Root: {}  (home directory)", self.root.display())
            }
            Some(SuspiciousReason::HomeParent) => {
                format!("Root: {}  (parent of home directory)", self.root.display())
            }
            Some(SuspiciousReason::SystemPath(p)) => {
                format!(
                    "Root: {}  (system directory: {})",
                    self.root.display(),
                    p.display()
                )
            }
            None => format!("Root: {}", self.root.display()),
        };
        lines.push(root_line);

        lines.push(format!(
            "Eligible files: ~{}",
            format_count(self.file_count)
        ));
        lines.push(format!(
            "Approx source content: ~{}",
            format_bytes(self.approx_bytes)
        ));

        // Rough estimate: build_index chunk_size ≈ 4000 chars. Integer math, no decimals.
        let est_chunks = usize::try_from(self.approx_bytes / 4000).unwrap_or(usize::MAX);
        lines.push(format!("Estimated chunks: ~{}", format_count(est_chunks)));

        lines.push(String::new());
        lines.push("This will use significant RAM and CPU time.".to_string());
        lines.push("Confirm indexing this directory?".to_string());

        lines.join("\n")
    }
}

/// Format an integer with thousand separators (e.g. `3,200`).
fn format_count(n: usize) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(b as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_rounds_to_one_decimal() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(2 * 1024 * 1024), "2.0 MB");
        assert_eq!(
            format_bytes((2 * 1024 * 1024 * 1024) + (500 * 1024 * 1024)),
            "2.5 GB"
        );
    }

    use std::path::Path;

    #[test]
    fn classify_path_detects_home_directory() {
        let Some(home) = crate::platform::home_dir() else {
            return;
        };
        assert_eq!(classify_path(&home), Some(SuspiciousReason::HomeDirectory),);
    }

    #[test]
    fn classify_path_detects_home_parent() {
        let Some(home) = crate::platform::home_dir() else {
            return;
        };
        let Some(parent) = home.parent() else { return };
        // Skip when home's parent is '/' — that hits SystemPath, not HomeParent.
        if parent == Path::new("/") {
            return;
        }
        assert_eq!(classify_path(parent), Some(SuspiciousReason::HomeParent),);
    }

    #[cfg(unix)]
    #[test]
    fn classify_path_detects_root_system_path() {
        // '/' is a system path. It also tests canonicalization doesn't break it.
        let v = classify_path(Path::new("/"));
        assert!(matches!(v, Some(SuspiciousReason::SystemPath(_))));
    }

    #[cfg(unix)]
    #[test]
    fn classify_path_detects_usr_etc_var() {
        for p in ["/usr", "/etc", "/var", "/tmp", "/opt"] {
            if !Path::new(p).exists() {
                continue;
            }
            let v = classify_path(Path::new(p));
            assert!(
                matches!(v, Some(SuspiciousReason::SystemPath(_))),
                "{p} should classify as SystemPath, got {v:?}",
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn classify_path_detects_windows_system_roots() {
        // Subset that always exists on a stock Windows install.
        for p in [r"C:\Windows", r"C:\Users", r"C:\ProgramData"] {
            if !Path::new(p).exists() {
                continue;
            }
            let v = classify_path(Path::new(p));
            assert!(
                matches!(v, Some(SuspiciousReason::SystemPath(_))),
                "{p} should classify as SystemPath, got {v:?}",
            );
        }
    }

    #[test]
    fn classify_path_allows_normal_project_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(classify_path(tmp.path()), None);
    }

    use std::io::Write;

    fn make_tempdir_with_bytes(total: u64) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        // Single file with the requested byte count.
        let mut f = std::fs::File::create(dir.path().join("big.rs")).unwrap();
        f.write_all(&vec![b'x'; total as usize]).unwrap();
        dir
    }

    #[test]
    fn check_index_scope_returns_clear_for_small_dir() {
        let dir = make_tempdir_with_bytes(1024); // 1 KB
        let v = check_index_scope(dir.path(), 500 * 1024 * 1024, &[]).unwrap();
        assert!(matches!(v, PreflightVerdict::Clear), "got {v:?}");
    }

    #[test]
    fn check_index_scope_flags_oversized_dir() {
        let dir = make_tempdir_with_bytes(2048);
        let v = check_index_scope(dir.path(), 1024, &[]).unwrap();
        match v {
            PreflightVerdict::RequiresConfirmation(info) => {
                assert!(info.size_exceeds_threshold);
                assert_eq!(info.suspicious_reason, None);
                assert_eq!(info.file_count, 1);
                assert!(info.approx_bytes >= 2048);
            }
            other => panic!("expected RequiresConfirmation, got {other:?}"),
        }
    }

    #[test]
    fn check_index_scope_respects_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        // Initialise a real git repo so WalkBuilder (require_git=true default)
        // actually applies .gitignore rules — matching build_index's walker.
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(dir.path())
            .status()
            .unwrap();
        // Big file that's gitignored
        let mut gi = std::fs::File::create(dir.path().join(".gitignore")).unwrap();
        gi.write_all(b"big.bin\n").unwrap();
        let mut f = std::fs::File::create(dir.path().join("big.bin")).unwrap();
        f.write_all(&vec![b'x'; 2048]).unwrap();
        // Small real source file
        let mut s = std::fs::File::create(dir.path().join("small.rs")).unwrap();
        s.write_all(b"fn main() {}\n").unwrap();

        let v = check_index_scope(dir.path(), 1024, &[]).unwrap();
        // big.bin should be ignored → under threshold → Clear.
        assert!(matches!(v, PreflightVerdict::Clear), "got {v:?}");
    }

    #[test]
    fn elicitation_message_includes_home_reason() {
        let info = PreflightInfo {
            root: PathBuf::from("/home/alice"),
            file_count: 3200,
            approx_bytes: 2 * 1024 * 1024 * 1024 + 400 * 1024 * 1024, // 2.4 GB
            suspicious_reason: Some(SuspiciousReason::HomeDirectory),
            size_exceeds_threshold: true,
        };
        let msg = info.elicitation_message();
        assert!(msg.contains("home directory"), "msg={msg}");
        assert!(msg.contains("/home/alice"), "msg={msg}");
        assert!(msg.contains("2.4 GB"), "msg={msg}");
        assert!(msg.contains("3,200") || msg.contains("3200"), "msg={msg}");
        assert!(msg.contains("Confirm"), "msg={msg}");
    }

    #[test]
    fn elicitation_message_size_only_omits_suspicious_line() {
        let info = PreflightInfo {
            root: PathBuf::from("/workspace/big"),
            file_count: 10_000,
            approx_bytes: 700 * 1024 * 1024,
            suspicious_reason: None,
            size_exceeds_threshold: true,
        };
        let msg = info.elicitation_message();
        assert!(!msg.to_lowercase().contains("home directory"), "msg={msg}");
        assert!(
            !msg.to_lowercase().contains("system directory"),
            "msg={msg}"
        );
        assert!(msg.contains("700.0 MB"), "msg={msg}");
    }

    #[test]
    fn elicitation_message_system_path_labelled() {
        let info = PreflightInfo {
            root: PathBuf::from("/usr"),
            file_count: 100_000,
            approx_bytes: 8 * 1024 * 1024 * 1024,
            suspicious_reason: Some(SuspiciousReason::SystemPath(PathBuf::from("/usr"))),
            size_exceeds_threshold: true,
        };
        let msg = info.elicitation_message();
        assert!(msg.to_lowercase().contains("system directory"), "msg={msg}");
        assert!(msg.contains("/usr"), "msg={msg}");
    }
}
