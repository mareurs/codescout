use std::path::{Path, PathBuf};

pub struct DiscoveredLibrary {
    pub name: String,
    pub version: Option<String>,
    pub path: PathBuf,
    pub language: String,
}

/// Walk up from a file path to find a package manifest and extract metadata.
pub fn discover_library_root(file_path: &Path) -> Option<DiscoveredLibrary> {
    let mut dir = file_path.parent()?;

    loop {
        if let Some(result) = try_cargo_toml(dir) {
            return Some(result);
        }
        if let Some(result) = try_package_json(dir) {
            return Some(result);
        }
        if let Some(result) = try_pyproject_toml(dir) {
            return Some(result);
        }
        if let Some(result) = try_go_mod(dir) {
            return Some(result);
        }

        match dir.parent() {
            Some(parent) if parent != dir => dir = parent,
            _ => break,
        }
    }

    // Fallback: use the deepest directory name
    let fallback_dir = file_path.parent()?;
    Some(DiscoveredLibrary {
        name: fallback_dir.file_name()?.to_string_lossy().into_owned(),
        version: None,
        path: fallback_dir.to_path_buf(),
        language: "unknown".into(),
    })
}

/// Read a package manifest found while walking up from a library file, or `None` when it cannot
/// be used. The walk runs through third-party source trees, so a manifest name can be a link to a
/// FIFO or a device (or a directory) and a plain read would hold the caller: the read is a
/// bounded regular-file read, and a refused, oversized, non-UTF-8 or unreadable manifest is
/// "no manifest here", so the walk goes on to the parent directory as it did for a failed read.
fn read_manifest_text(path: &Path) -> Option<String> {
    match crate::util::fs::read_regular_config_text(path) {
        Ok(text) => Some(text),
        Err(e) => {
            if !e.is_not_found() {
                tracing::debug!("manifest {} skipped: {e}", path.display());
            }
            None
        }
    }
}

fn try_manifest(
    dir: &Path,
    filename: &str,
    extract: fn(&str, &str) -> Option<String>,
    language: &str,
) -> Option<DiscoveredLibrary> {
    let manifest = dir.join(filename);
    if !manifest.exists() {
        return None;
    }
    let text = read_manifest_text(&manifest)?;

    let name = extract(&text, "name")?;
    let version = extract(&text, "version");

    Some(DiscoveredLibrary {
        name,
        version,
        path: dir.to_path_buf(),
        language: language.into(),
    })
}

fn try_cargo_toml(dir: &Path) -> Option<DiscoveredLibrary> {
    try_manifest(dir, "Cargo.toml", extract_toml_value, "rust")
}

fn try_package_json(dir: &Path) -> Option<DiscoveredLibrary> {
    try_manifest(dir, "package.json", extract_json_value, "javascript")
}

fn try_pyproject_toml(dir: &Path) -> Option<DiscoveredLibrary> {
    try_manifest(dir, "pyproject.toml", extract_toml_value, "python")
}

fn try_go_mod(dir: &Path) -> Option<DiscoveredLibrary> {
    let manifest = dir.join("go.mod");
    if !manifest.exists() {
        return None;
    }
    let text = read_manifest_text(&manifest)?;

    let name = text
        .lines()
        .find(|l| l.starts_with("module "))
        .map(|l| l.trim_start_matches("module ").trim().to_string())?;

    Some(DiscoveredLibrary {
        name,
        version: None,
        path: dir.to_path_buf(),
        language: "go".into(),
    })
}

/// Best-effort: extract `key = "value"` from TOML
fn extract_toml_value(text: &str, key: &str) -> Option<String> {
    let pattern = format!("{} = \"", key);
    text.lines()
        .find(|l| l.trim().starts_with(&pattern))
        .and_then(|l| {
            let start = l.find('"')? + 1;
            let end = l[start..].find('"')? + start;
            Some(l[start..end].to_string())
        })
}

/// Best-effort: extract `"key": "value"` from JSON
fn extract_json_value(text: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    text.lines().find(|l| l.contains(&pattern)).and_then(|l| {
        let after_key = l.find(&pattern)? + pattern.len();
        let rest = &l[after_key..];
        let start = rest.find('"')? + 1;
        let end = rest[start..].find('"')? + start;
        Some(rest[start..end].to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_from_cargo_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"serde\"\nversion = \"1.0.210\"\n",
        )
        .unwrap();

        let result = discover_library_root(&dir.path().join("src/de.rs")).unwrap();
        assert_eq!(result.name, "serde");
        assert_eq!(result.version, Some("1.0.210".into()));
        assert_eq!(result.path, dir.path().to_path_buf());
        assert_eq!(result.language, "rust");
    }

    #[test]
    fn discover_from_package_json() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{ "name": "lodash", "version": "4.17.21" }"#,
        )
        .unwrap();

        let result = discover_library_root(&dir.path().join("index.js")).unwrap();
        assert_eq!(result.name, "lodash");
        assert_eq!(result.version, Some("4.17.21".into()));
        assert_eq!(result.language, "javascript");
    }

    #[test]
    fn discover_from_pyproject_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nname = \"requests\"\nversion = \"2.31.0\"\n",
        )
        .unwrap();

        let result = discover_library_root(&dir.path().join("src/requests/api.py")).unwrap();
        assert_eq!(result.name, "requests");
        assert_eq!(result.version, Some("2.31.0".into()));
        assert_eq!(result.language, "python");
    }

    #[test]
    fn discover_from_go_mod() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("go.mod"),
            "module github.com/user/repo\n\ngo 1.21\n",
        )
        .unwrap();

        let result = discover_library_root(&dir.path().join("main.go")).unwrap();
        assert_eq!(result.name, "github.com/user/repo");
        assert_eq!(result.version, None);
        assert_eq!(result.language, "go");
    }

    #[test]
    fn discover_fallback_uses_dir_name() {
        let dir = tempfile::tempdir().unwrap();
        let result = discover_library_root(&dir.path().join("lib.rs"));
        assert!(result.is_some());
    }

    #[test]
    fn discover_walks_up_parents() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("src").join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"deep_crate\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let result = discover_library_root(&nested.join("mod.rs")).unwrap();
        assert_eq!(result.name, "deep_crate");
        assert_eq!(result.path, dir.path().to_path_buf());
    }

    #[test]
    fn extract_toml_value_basic() {
        let toml = "name = \"hello\"\nversion = \"1.0\"";
        assert_eq!(extract_toml_value(toml, "name"), Some("hello".into()));
        assert_eq!(extract_toml_value(toml, "version"), Some("1.0".into()));
        assert_eq!(extract_toml_value(toml, "missing"), None);
    }

    #[test]
    fn extract_json_value_basic() {
        let json = r#"{ "name": "lodash", "version": "4.17.21" }"#;
        assert_eq!(extract_json_value(json, "name"), Some("lodash".into()));
        assert_eq!(extract_json_value(json, "version"), Some("4.17.21".into()));
        assert_eq!(extract_json_value(json, "missing"), None);
    }
}
/// A library root is a third-party source tree, so any manifest name on the walk up from a file can
/// be a link to a FIFO or a device.
/// docs/issues/2026-10-20-reads-of-repository-files-that-remain-unbounded-after-the-config-loader-fix.md
#[cfg(all(test, unix))]
mod bounded_manifest_tests {
    use super::*;
    use crate::util::fs::special_file_fixtures::{make_fifo, within_deadline};

    const OUTER_CARGO: &str = "[package]\nname = \"outer_crate\"\nversion = \"0.1.0\"\n";

    /// `<root>/Cargo.toml` is a regular file; `<root>/inner/` holds the file asked about and, in
    /// place of a manifest, a FIFO or a link to one.
    fn walk_from_inner(manifest: &str, linked: bool) -> Option<DiscoveredLibrary> {
        let dir = tempfile::tempdir().unwrap();
        let inner = dir.path().join("inner");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), OUTER_CARGO).unwrap();
        let file = inner.join(manifest);
        if linked {
            let fifo = dir.path().join("elsewhere-pipe");
            make_fifo(&fifo);
            std::os::unix::fs::symlink(&fifo, &file).unwrap();
        } else {
            make_fifo(&file);
        }
        let asked = inner.join("lib.rs");
        // The tempdir must outlive the walk, so it moves into the closure.
        within_deadline(
            &format!("discover_library_root, {manifest} a FIFO"),
            move || {
                let found = discover_library_root(&asked);
                drop(dir);
                found
            },
        )
    }

    /// A FIFO, and a link to one, named like each manifest held the walk. It is "no manifest
    /// here": the walk goes on to the parent and finds the regular `Cargo.toml` there.
    #[test]
    fn a_fifo_or_a_link_to_one_in_place_of_a_manifest_is_skipped_and_the_walk_goes_on() {
        for manifest in ["Cargo.toml", "package.json", "pyproject.toml", "go.mod"] {
            for linked in [false, true] {
                let found = walk_from_inner(manifest, linked)
                    .unwrap_or_else(|| panic!("{manifest} linked={linked}: nothing found"));
                assert_eq!(found.name, "outer_crate", "{manifest} linked={linked}");
                assert_eq!(found.language, "rust", "{manifest} linked={linked}");
            }
        }
    }

    /// Control: the same manifest names as regular files are read where they stand, and a
    /// directory, an empty file and invalid UTF-8 are skipped as before.
    #[test]
    fn regular_manifests_are_read_where_they_stand_and_unusable_ones_are_skipped() {
        for (manifest, body, name, language) in [
            (
                "Cargo.toml",
                "[package]\nname = \"inner_crate\"\nversion = \"1.2.3\"\n",
                "inner_crate",
                "rust",
            ),
            (
                "package.json",
                "{\n  \"name\": \"inner-pkg\",\n  \"version\": \"1.2.3\"\n}\n",
                "inner-pkg",
                "javascript",
            ),
            (
                "pyproject.toml",
                "[project]\nname = \"inner-py\"\nversion = \"1.2.3\"\n",
                "inner-py",
                "python",
            ),
            (
                "go.mod",
                "module example.com/inner\n",
                "example.com/inner",
                "go",
            ),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let inner = dir.path().join("inner");
            std::fs::create_dir_all(&inner).unwrap();
            std::fs::write(dir.path().join("Cargo.toml"), OUTER_CARGO).unwrap();
            std::fs::write(inner.join(manifest), body).unwrap();
            let found = discover_library_root(&inner.join("lib.rs")).unwrap();
            assert_eq!(
                (found.name.as_str(), found.language.as_str()),
                (name, language)
            );
            assert_eq!(found.path, inner, "{manifest}");

            // Linked to a regular file: still read.
            std::fs::remove_file(inner.join(manifest)).unwrap();
            let real = dir.path().join("real-manifest");
            std::fs::write(&real, body).unwrap();
            std::os::unix::fs::symlink(&real, inner.join(manifest)).unwrap();
            let found = discover_library_root(&inner.join("lib.rs")).unwrap();
            assert_eq!(found.name, name, "linked {manifest}");

            // Directory, empty and invalid UTF-8 in its place: the outer manifest answers.
            std::fs::remove_file(inner.join(manifest)).unwrap();
            std::fs::create_dir(inner.join(manifest)).unwrap();
            let found = discover_library_root(&inner.join("lib.rs")).unwrap();
            assert_eq!(found.name, "outer_crate", "directory {manifest}");
            std::fs::remove_dir(inner.join(manifest)).unwrap();
            std::fs::write(inner.join(manifest), "").unwrap();
            let found = discover_library_root(&inner.join("lib.rs")).unwrap();
            assert_eq!(found.name, "outer_crate", "empty {manifest}");
            std::fs::write(inner.join(manifest), [0xff, 0xfe]).unwrap();
            let found = discover_library_root(&inner.join("lib.rs")).unwrap();
            assert_eq!(found.name, "outer_crate", "invalid {manifest}");
        }
    }

    /// A manifest over the limit is "no manifest here". It starts with a valid manifest and is
    /// then extended sparse, so only the length check can skip it.
    #[test]
    fn a_manifest_over_the_limit_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let inner = dir.path().join("inner");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), OUTER_CARGO).unwrap();
        let path = inner.join("Cargo.toml");
        std::fs::write(&path, "[package]\nname = \"inner_crate\"\n").unwrap();
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(crate::util::fs::MAX_CONFIG_FILE_BYTES + 1)
            .unwrap();
        drop(file);
        let found = discover_library_root(&inner.join("lib.rs")).unwrap();
        assert_eq!(found.name, "outer_crate");
    }
}
