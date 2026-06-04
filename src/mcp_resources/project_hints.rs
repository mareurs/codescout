//! Manifest-derived project hints surfaced in the `activate_project` response.
//!
//! Goal: give agents useful project context even when the explicit `onboarding`
//! workflow has never been run. All probes are cheap (a handful of `exists()`
//! calls at the project root) and entirely read-only.

use serde::Serialize;
use std::path::Path;

/// Structured project hints populated from manifest files at the project root.
///
/// Returned as part of `activate_project`. Agents that never call `onboarding`
/// still get a minimum of project context (language, manifest, build commands,
/// likely entry-point files).
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ProjectHints {
    /// Primary language detected from a manifest file (rust, typescript, etc.).
    /// `None` if no recognised manifest exists at the project root.
    pub primary_language: Option<String>,
    /// Filename of the manifest that drove detection (e.g. `Cargo.toml`).
    pub manifest: Option<String>,
    /// Entry-point files that actually exist on disk (capped at 3).
    pub entry_points: Vec<String>,
    /// Canonical build / test / run commands for the detected manifest.
    pub build_commands: Vec<String>,
    /// `true` when an `onboarding` memory is present — indicates full onboarding
    /// was run at some point. When `false`, these hints are the agent's primary
    /// project context.
    pub onboarded: bool,
}

/// Probe manifest files at `project_root` and return a [`ProjectHints`].
///
/// `memories` is the list of existing memory names for the project. Used to
/// set `onboarded`.
pub fn probe_project_hints(project_root: &Path, memories: &[String]) -> ProjectHints {
    let onboarded = memories.iter().any(|m| m == "onboarding");
    let info = detect_manifest_info(project_root);
    // primary_language reflects the dominant language by file count, not just the
    // build manifest — a polyglot root whose manifest ≠ dominant language (e.g. a
    // Python repo keeping package.json for tooling) was mislabeled. Manifest
    // language is the fallback when no source files are found.
    // (2026-06-03-project-languages-from-manifest-not-files)
    let primary_language = crate::workspace::dominant_language(project_root)
        .or_else(|| info.as_ref().map(|m| m.language.to_string()));
    let (manifest, entry_points, build_commands) = match &info {
        Some(m) => (
            Some(m.manifest.to_string()),
            probe_entry_points(project_root, m.language),
            m.build_commands.iter().map(|s| s.to_string()).collect(),
        ),
        None => (None, Vec::new(), Vec::new()),
    };
    ProjectHints {
        primary_language,
        manifest,
        entry_points,
        build_commands,
        onboarded,
    }
}

/// Information extracted from a single manifest file detection.
struct ManifestInfo {
    /// Manifest filename, e.g. `"Cargo.toml"`.
    manifest: &'static str,
    /// Canonical language tag.
    language: &'static str,
    /// Build / test / run commands associated with the manifest.
    build_commands: &'static [&'static str],
}

/// First match wins. `package.json` is checked before the other manifests
/// because Node/TS projects sometimes also ship a `pyproject.toml` for tooling,
/// but the primary ecosystem is still Node.
fn detect_manifest_info(project_root: &Path) -> Option<ManifestInfo> {
    if project_root.join("package.json").exists() {
        let is_ts = project_root.join("tsconfig.json").exists();
        return Some(ManifestInfo {
            manifest: "package.json",
            language: if is_ts { "typescript" } else { "javascript" },
            build_commands: &["npm test", "npm run build"],
        });
    }

    const MANIFESTS: &[ManifestInfo] = &[
        ManifestInfo {
            manifest: "Cargo.toml",
            language: "rust",
            build_commands: &["cargo build", "cargo test", "cargo run"],
        },
        ManifestInfo {
            manifest: "pyproject.toml",
            language: "python",
            build_commands: &["pytest", "python -m <package>"],
        },
        ManifestInfo {
            manifest: "setup.py",
            language: "python",
            build_commands: &["pytest", "python setup.py test"],
        },
        ManifestInfo {
            manifest: "go.mod",
            language: "go",
            build_commands: &["go build ./...", "go test ./..."],
        },
        ManifestInfo {
            manifest: "pom.xml",
            language: "java",
            build_commands: &["mvn test", "mvn package"],
        },
        ManifestInfo {
            manifest: "build.gradle.kts",
            language: "kotlin",
            build_commands: &["./gradlew build", "./gradlew test"],
        },
        ManifestInfo {
            manifest: "build.gradle",
            language: "kotlin",
            build_commands: &["./gradlew build", "./gradlew test"],
        },
    ];

    for m in MANIFESTS {
        if project_root.join(m.manifest).exists() {
            return Some(ManifestInfo {
                manifest: m.manifest,
                language: m.language,
                build_commands: m.build_commands,
            });
        }
    }
    None
}

/// Return the first up-to-3 candidate entry points that exist on disk for the
/// given language. The order mirrors the language's conventional priority
/// (`src/main.rs` before `src/lib.rs` for Rust binaries, etc.).
fn probe_entry_points(project_root: &Path, language: &str) -> Vec<String> {
    let candidates: &[&str] = match language {
        "rust" => &["src/main.rs", "src/lib.rs"],
        "typescript" => &["src/index.ts", "src/main.ts", "index.ts"],
        "javascript" => &["src/index.js", "src/main.js", "index.js"],
        "python" => &["__main__.py", "main.py", "src/main.py"],
        "go" => &["main.go", "cmd/main.go"],
        _ => &[],
    };
    candidates
        .iter()
        .filter(|p| project_root.join(p).exists())
        .take(3)
        .map(|s| s.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_memories() -> Vec<String> {
        Vec::new()
    }

    #[test]
    fn rust_project_with_main() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();

        let hints = probe_project_hints(dir.path(), &empty_memories());
        assert_eq!(hints.primary_language.as_deref(), Some("rust"));
        assert_eq!(hints.manifest.as_deref(), Some("Cargo.toml"));
        assert_eq!(hints.entry_points, vec!["src/main.rs"]);
        assert!(hints.build_commands.iter().any(|c| c == "cargo test"));
        assert!(!hints.onboarded);
    }

    #[test]
    fn rust_library_only() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "// lib").unwrap();

        let hints = probe_project_hints(dir.path(), &empty_memories());
        assert_eq!(hints.entry_points, vec!["src/lib.rs"]);
    }

    #[test]
    fn typescript_project_detects_tsconfig() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();
        std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/index.ts"), "").unwrap();

        let hints = probe_project_hints(dir.path(), &empty_memories());
        assert_eq!(hints.primary_language.as_deref(), Some("typescript"));
        assert_eq!(hints.manifest.as_deref(), Some("package.json"));
        assert_eq!(hints.entry_points, vec!["src/index.ts"]);
    }

    #[test]
    fn javascript_project_without_tsconfig() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();

        let hints = probe_project_hints(dir.path(), &empty_memories());
        assert_eq!(hints.primary_language.as_deref(), Some("javascript"));
    }

    #[test]
    fn package_json_preferred_over_pyproject() {
        let dir = tempfile::tempdir().unwrap();
        // Some Node repos ship a pyproject.toml for tooling; Node must win.
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();
        std::fs::write(dir.path().join("pyproject.toml"), "").unwrap();

        let hints = probe_project_hints(dir.path(), &empty_memories());
        assert_eq!(hints.primary_language.as_deref(), Some("javascript"));
        assert_eq!(hints.manifest.as_deref(), Some("package.json"));
    }

    #[test]
    fn no_manifest_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let hints = probe_project_hints(dir.path(), &empty_memories());
        assert_eq!(hints.primary_language, None);
        assert_eq!(hints.manifest, None);
        assert!(hints.entry_points.is_empty());
        assert!(hints.build_commands.is_empty());
    }

    #[test]
    fn onboarded_flag_reflects_memory_presence() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "").unwrap();

        let with = probe_project_hints(dir.path(), &["onboarding".to_string()]);
        assert!(with.onboarded);

        let without = probe_project_hints(dir.path(), &["architecture".to_string()]);
        assert!(!without.onboarded);
    }

    #[test]
    fn entry_points_capped_at_three() {
        // Not reachable today (no language has > 3 candidates) but guard against
        // future additions.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();
        std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/index.ts"), "").unwrap();
        std::fs::write(dir.path().join("src/main.ts"), "").unwrap();
        std::fs::write(dir.path().join("index.ts"), "").unwrap();

        let hints = probe_project_hints(dir.path(), &empty_memories());
        assert!(hints.entry_points.len() <= 3);
        assert_eq!(
            hints.entry_points,
            vec!["src/index.ts", "src/main.ts", "index.ts"]
        );
    }

    #[test]
    fn python_project_probes_entry_points() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("pyproject.toml"), "").unwrap();
        std::fs::write(dir.path().join("main.py"), "").unwrap();

        let hints = probe_project_hints(dir.path(), &empty_memories());
        assert_eq!(hints.primary_language.as_deref(), Some("python"));
        assert_eq!(hints.entry_points, vec!["main.py"]);
    }

    #[test]
    fn go_project() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("go.mod"), "module x").unwrap();
        std::fs::write(dir.path().join("main.go"), "").unwrap();

        let hints = probe_project_hints(dir.path(), &empty_memories());
        assert_eq!(hints.primary_language.as_deref(), Some("go"));
        assert!(hints.build_commands.iter().any(|c| c == "go test ./..."));
    }

    #[test]
    fn kotlin_project_via_gradle_kts() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("build.gradle.kts"), "").unwrap();

        let hints = probe_project_hints(dir.path(), &empty_memories());
        assert_eq!(hints.primary_language.as_deref(), Some("kotlin"));
        assert_eq!(hints.manifest.as_deref(), Some("build.gradle.kts"));
    }
}
