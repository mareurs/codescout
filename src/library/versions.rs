use std::path::Path;

#[derive(Debug, Clone)]
pub struct ResolvedVersion {
    pub name: String,
    pub version: String,
}

/// The most bytes a lock file may hold before it is not read. Lock files are the largest manifests
/// there are (a monorepo `package-lock.json` runs to megabytes; the largest one on this machine
/// is 0.75 MB and the largest `Cargo.lock` 0.4 MB), so the 1 MiB used for the small manifests
/// would refuse real projects; a lock file past this is not one.
// cap-class: NOT_A_CAP — bounds how many bytes of an INPUT lock file are read; a file over it is treated as absent, no result is shortened and nothing is presented as complete
const MAX_LOCKFILE_BYTES: u64 = 64 * 1024 * 1024;

/// Read a lock file as text, or `None` when it cannot be used. The project root's lock files are
/// repository content, so a name can be a committed link to a FIFO or a device; a plain read would
/// hold the index build. Absent, unreadable, not UTF-8, not a regular file and over
/// [`MAX_LOCKFILE_BYTES`] are one answer (the reason, never the content, goes to the debug log).
fn read_lockfile(path: &Path) -> Option<String> {
    match crate::util::fs::read_regular_text(path, MAX_LOCKFILE_BYTES) {
        Ok(text) => Some(text),
        Err(e) => {
            if !e.is_not_found() {
                tracing::debug!("lock file {} skipped: {e}", path.display());
            }
            None
        }
    }
}

/// Detect project type and parse lockfile to get dependency versions.
pub fn resolve_dependency_versions(project_root: &Path) -> Vec<ResolvedVersion> {
    // Try Cargo.lock (Rust) — P0
    let cargo_lock = project_root.join("Cargo.lock");
    if cargo_lock.exists() {
        if let Some(content) = read_lockfile(&cargo_lock) {
            return parse_cargo_lock(&content);
        }
    }

    // Try package-lock.json (JS/TS) — P0
    let pkg_lock = project_root.join("package-lock.json");
    if pkg_lock.exists() {
        if let Some(content) = read_lockfile(&pkg_lock) {
            return parse_package_lock_json(&content);
        }
    }

    // TODO P1: yarn.lock, pnpm-lock.yaml, go.sum, uv.lock, poetry.lock

    Vec::new()
}

/// Parse Cargo.lock to extract package versions.
pub fn parse_cargo_lock(content: &str) -> Vec<ResolvedVersion> {
    let mut versions = Vec::new();
    let mut current_name: Option<String> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("name = ") {
            current_name = line
                .strip_prefix("name = ")
                .and_then(|s| s.strip_prefix('"'))
                .and_then(|s| s.strip_suffix('"'))
                .map(|s| s.to_string());
        } else if line.starts_with("version = ") {
            if let (Some(name), Some(ver)) = (
                current_name.take(),
                line.strip_prefix("version = ")
                    .and_then(|s| s.strip_prefix('"'))
                    .and_then(|s| s.strip_suffix('"')),
            ) {
                versions.push(ResolvedVersion {
                    name,
                    version: ver.to_string(),
                });
            }
        }
    }
    versions
}

/// Parse package-lock.json (v2/v3 format) to extract package versions.
pub fn parse_package_lock_json(content: &str) -> Vec<ResolvedVersion> {
    let mut versions = Vec::new();
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(content) else {
        return versions;
    };

    if let Some(packages) = parsed.get("packages").and_then(|p| p.as_object()) {
        for (key, val) in packages {
            if key.is_empty() {
                continue; // Skip root package
            }
            let name = key.strip_prefix("node_modules/").unwrap_or(key).to_string();
            if let Some(version) = val.get("version").and_then(|v| v.as_str()) {
                versions.push(ResolvedVersion {
                    name,
                    version: version.to_string(),
                });
            }
        }
    }
    versions
}

/// Look up a specific library's version from resolved dependencies.
pub fn find_version(versions: &[ResolvedVersion], name: &str) -> Option<String> {
    versions
        .iter()
        .find(|v| v.name == name)
        .map(|v| v.version.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cargo_lock_extracts_versions() {
        let content = r#"
[[package]]
name = "tokio"
version = "1.38.0"

[[package]]
name = "serde"
version = "1.0.203"
"#;
        let versions = parse_cargo_lock(content);
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].name, "tokio");
        assert_eq!(versions[0].version, "1.38.0");
        assert_eq!(versions[1].name, "serde");
        assert_eq!(versions[1].version, "1.0.203");
    }

    #[test]
    fn parse_package_lock_json_extracts_versions() {
        let content = r#"{
            "packages": {
                "": { "version": "1.0.0" },
                "node_modules/lodash": { "version": "4.17.21" },
                "node_modules/@types/node": { "version": "20.11.0" }
            }
        }"#;
        let versions = parse_package_lock_json(content);
        assert_eq!(versions.len(), 2); // root package skipped
        assert!(versions
            .iter()
            .any(|v| v.name == "lodash" && v.version == "4.17.21"));
        assert!(versions
            .iter()
            .any(|v| v.name == "@types/node" && v.version == "20.11.0"));
    }

    #[test]
    fn find_version_looks_up_by_name() {
        let versions = vec![
            ResolvedVersion {
                name: "tokio".into(),
                version: "1.38.0".into(),
            },
            ResolvedVersion {
                name: "serde".into(),
                version: "1.0.203".into(),
            },
        ];
        assert_eq!(find_version(&versions, "tokio"), Some("1.38.0".to_string()));
        assert_eq!(find_version(&versions, "unknown"), None);
    }

    #[test]
    fn resolve_dependency_versions_reads_cargo_lock() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.lock"),
            r#"
[[package]]
name = "anyhow"
version = "1.0.82"
"#,
        )
        .unwrap();
        let versions = resolve_dependency_versions(dir.path());
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].name, "anyhow");
    }
}
/// The lock files are read from the project root on every index build, and a committed link can
/// name a FIFO or a device there.
/// docs/issues/2026-10-20-reads-of-repository-files-that-remain-unbounded-after-the-config-loader-fix.md
#[cfg(all(test, unix))]
mod bounded_lockfile_tests {
    use super::*;
    use crate::util::fs::special_file_fixtures::{make_fifo, within_deadline};

    const CARGO_LOCK: &str = "[[package]]\nname = \"anyhow\"\nversion = \"1.0.82\"\n";
    const PACKAGE_LOCK: &str = r#"{"packages":{"node_modules/lodash":{"version":"4.17.21"}}}"#;

    fn resolved_in_time(what: &str, root: &Path) -> Vec<(String, String)> {
        let root = root.to_path_buf();
        within_deadline(what, move || {
            resolve_dependency_versions(&root)
                .into_iter()
                .map(|v| (v.name, v.version))
                .collect()
        })
    }

    fn pair(name: &str, version: &str) -> (String, String) {
        (name.to_string(), version.to_string())
    }

    /// A FIFO, and a link to one, in place of either lock file held the index build.
    #[test]
    fn a_fifo_or_a_link_to_one_in_place_of_a_lock_file_is_skipped_and_never_a_hang() {
        for name in ["Cargo.lock", "package-lock.json"] {
            for linked in [false, true] {
                let dir = tempfile::tempdir().unwrap();
                let file = dir.path().join(name);
                if linked {
                    let fifo = dir.path().join("elsewhere-pipe");
                    make_fifo(&fifo);
                    std::os::unix::fs::symlink(&fifo, &file).unwrap();
                } else {
                    make_fifo(&file);
                }
                let got = resolved_in_time(&format!("{name} a FIFO"), dir.path());
                assert!(got.is_empty(), "{name} linked={linked}: {got:?}");
            }
        }
    }

    /// An unusable `Cargo.lock` still leaves `package-lock.json` to be tried, as a failed read did.
    #[test]
    fn an_unusable_cargo_lock_still_falls_through_to_package_lock() {
        let dir = tempfile::tempdir().unwrap();
        make_fifo(&dir.path().join("Cargo.lock"));
        std::fs::write(dir.path().join("package-lock.json"), PACKAGE_LOCK).unwrap();
        let got = resolved_in_time("Cargo.lock a FIFO", dir.path());
        assert_eq!(got, vec![pair("lodash", "4.17.21")]);
    }

    /// Control: regular files, and a link to one, are read as before; missing, empty and invalid
    /// UTF-8 give nothing.
    #[test]
    fn regular_lock_files_are_read_and_missing_empty_or_invalid_ones_give_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(resolved_in_time("no lock file", dir.path()).is_empty());

        std::fs::write(dir.path().join("Cargo.lock"), CARGO_LOCK).unwrap();
        assert_eq!(
            resolved_in_time("regular Cargo.lock", dir.path()),
            vec![pair("anyhow", "1.0.82")]
        );

        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real-lock");
        std::fs::write(&real, PACKAGE_LOCK).unwrap();
        std::os::unix::fs::symlink(&real, dir.path().join("package-lock.json")).unwrap();
        assert_eq!(
            resolved_in_time("linked package-lock.json", dir.path()),
            vec![pair("lodash", "4.17.21")]
        );

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.lock"), "").unwrap();
        assert!(resolved_in_time("empty Cargo.lock", dir.path()).is_empty());

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.lock"), [0xff, 0xfe]).unwrap();
        assert!(resolved_in_time("invalid Cargo.lock", dir.path()).is_empty());
    }

    /// A lock file past the limit is not read. The file starts with a valid lock and is then
    /// extended sparse (the length is reported without the bytes held), so only the length
    /// check can produce the empty answer: a read would find `anyhow`.
    #[test]
    fn a_lock_file_over_the_limit_is_not_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Cargo.lock");
        std::fs::write(&path, CARGO_LOCK).unwrap();
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(MAX_LOCKFILE_BYTES + 1).unwrap();
        drop(file);
        assert!(resolved_in_time("oversized Cargo.lock", dir.path()).is_empty());
    }
}
