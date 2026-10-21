//! Library registry: tracks external libraries available for cross-reference search.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

fn default_true() -> bool {
    true
}

/// A registered external library that codescout can search into.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LibraryEntry {
    pub name: String,
    pub version: Option<String>,
    #[serde(default)]
    pub version_indexed: Option<String>,
    #[serde(default)]
    pub db_file: Option<String>,
    #[serde(default)]
    pub nudge_dismissed: bool,
    pub path: PathBuf,
    pub language: String,
    pub discovered_via: DiscoveryMethod,
    pub indexed: bool,
    #[serde(default = "default_true")]
    pub source_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryMethod {
    LspFollowThrough,
    Manual,
    ManifestScan,
}

/// The most bytes `.codescout/libraries.json` may hold. It is rewritten on every activation, so a
/// file refused here is replaced by an empty registry on the next save; the limit is therefore far
/// above anything real (one entry is ~250 bytes: 16 MiB is tens of thousands of libraries) rather
/// than the 1 MiB used for configuration files.
// cap-class: NOT_A_CAP — bounds how many bytes of an INPUT registry file are read; a file over it is treated as absent, no result is shortened and nothing is presented as complete
pub(crate) const MAX_LIBRARY_REGISTRY_BYTES: u64 = 16 * 1024 * 1024;

/// Persistent registry of known external libraries.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LibraryRegistry {
    entries: Vec<LibraryEntry>,
}

impl LibraryRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Load from a JSON file. Returns an empty registry if the file does not exist, and also when
    /// it is not a usable file: `.codescout/libraries.json` is repository content, so it can be a
    /// committed link to a FIFO or a device, which a plain read would hold or exhaust memory on.
    /// It is read as a bounded regular file ([`crate::util::fs::read_regular_text`] at
    /// [`MAX_LIBRARY_REGISTRY_BYTES`]); a refused or oversized file is the same answer as an absent
    /// one (logged at debug, reason only). A file that is readable but not valid JSON is still an
    /// error.
    ///
    /// **A registry that loads empty because it was refused is indistinguishable from a new one,
    /// and the next `save` (activation's `auto_register_deps`) replaces the file.** That was
    /// already so for invalid JSON (the callers `unwrap_or_default`); it is now so for a refused
    /// file too, the reason this limit is generous rather than the 1 MiB config one. Whether a
    /// refused load should instead block the save is an open decision, recorded in
    /// `docs/issues/2026-10-20-reads-of-repository-files-that-remain-unbounded-after-the-config-loader-fix.md`.
    pub fn load(path: &Path) -> Result<Self> {
        let data = match crate::util::fs::read_regular_text(path, MAX_LIBRARY_REGISTRY_BYTES) {
            Ok(data) => data,
            Err(e) if e.is_not_found() => return Ok(Self::new()),
            Err(
                e @ (crate::util::fs::BoundedReadError::NotRegular
                | crate::util::fs::BoundedReadError::TooLarge { .. }),
            ) => {
                tracing::debug!("library registry {} ignored: {e}", path.display());
                return Ok(Self::new());
            }
            Err(crate::util::fs::BoundedReadError::Io(e)) => return Err(e.into()),
        };
        let registry: LibraryRegistry = serde_json::from_str(&data)?;
        Ok(registry)
    }

    /// Persist the registry to a JSON file. Uses `atomic_write` so a crash
    /// or disk-full mid-write cannot leave `libraries.json` half-serialized.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;
        crate::util::fs::atomic_write(path, &data)?;
        Ok(())
    }

    /// Register a library. If a library with the same name exists, update its
    /// path/language/discovered_via. The `indexed` flag is preserved when the
    /// path has not changed; otherwise it resets to `false`.
    /// ManifestScan must not overwrite Manual or LspFollowThrough registrations.
    pub fn register(
        &mut self,
        name: String,
        path: PathBuf,
        language: String,
        discovered_via: DiscoveryMethod,
        source_available: bool,
    ) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.name == name) {
            // ManifestScan must not overwrite Manual registrations
            if existing.discovered_via == DiscoveryMethod::Manual
                && discovered_via == DiscoveryMethod::ManifestScan
            {
                return;
            }
            let path_changed = existing.path != path;
            existing.path = path;
            existing.language = language;
            existing.discovered_via = discovered_via;
            existing.source_available = source_available;
            if path_changed {
                existing.indexed = false;
            }
        } else {
            self.entries.push(LibraryEntry {
                name,
                version: None,
                version_indexed: None,
                db_file: None,
                nudge_dismissed: false,
                path,
                language,
                indexed: false,
                discovered_via,
                source_available,
            });
        }
    }

    /// Look up a library by name.
    pub fn lookup(&self, name: &str) -> Option<&LibraryEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Look up a library by name (mutable).
    pub fn lookup_mut(&mut self, name: &str) -> Option<&mut LibraryEntry> {
        self.entries.iter_mut().find(|e| e.name == name)
    }

    /// Return all registered libraries.
    pub fn all(&self) -> &[LibraryEntry] {
        &self.entries
    }

    /// Resolve a relative path within a library's root directory.
    ///
    /// Rejects paths containing `..` components to prevent traversal outside
    /// the library root.
    pub fn resolve_path(&self, name: &str, relative: &str) -> Result<PathBuf> {
        let entry = self
            .lookup(name)
            .ok_or_else(|| anyhow::anyhow!("Unknown library: {}", name))?;
        // Reject directory traversal attempts.
        let rel = std::path::Path::new(relative);
        for component in rel.components() {
            if matches!(component, std::path::Component::ParentDir) {
                anyhow::bail!(
                    "Path traversal rejected: '{}' contains '..' component",
                    relative
                );
            }
        }
        Ok(entry.path.join(relative))
    }

    /// Check whether an absolute path falls inside any registered library.
    /// Returns the matching entry if found.
    pub fn is_library_path(&self, absolute_path: &Path) -> Option<&LibraryEntry> {
        self.entries
            .iter()
            .find(|e| absolute_path.starts_with(&e.path))
    }

    /// Return registered libraries where `version != version_indexed` (stale).
    /// Only considers libraries that are indexed and have both versions set.
    pub fn stale_libraries(&self) -> Vec<&LibraryEntry> {
        self.entries
            .iter()
            .filter(|e| {
                e.indexed
                    && e.version.is_some()
                    && e.version_indexed.is_some()
                    && e.version != e.version_indexed
            })
            .collect()
    }

    /// Update a library's detected version. Resets nudge_dismissed if version changed.
    pub fn update_version(&mut self, name: &str, new_version: &str) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.name == name) {
            let changed = entry.version.as_deref() != Some(new_version);
            entry.version = Some(new_version.to_string());
            if changed {
                entry.nudge_dismissed = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn json_roundtrip_library_entry() {
        let entry = LibraryEntry {
            name: "serde".into(),
            version: Some("1.0.200".into()),
            version_indexed: None,
            db_file: None,
            nudge_dismissed: false,
            path: PathBuf::from("/home/user/.cargo/registry/serde-1.0.200"),
            language: "rust".into(),
            discovered_via: DiscoveryMethod::LspFollowThrough,
            indexed: true,
            source_available: true,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let restored: LibraryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, restored);
    }

    #[test]
    fn discovery_method_serializes_as_snake_case() {
        let lsp = serde_json::to_string(&DiscoveryMethod::LspFollowThrough).unwrap();
        assert_eq!(lsp, "\"lsp_follow_through\"");

        let manual = serde_json::to_string(&DiscoveryMethod::Manual).unwrap();
        assert_eq!(manual, "\"manual\"");
    }

    #[test]
    fn register_and_lookup() {
        let mut reg = LibraryRegistry::new();
        reg.register(
            "tokio".into(),
            PathBuf::from("/libs/tokio"),
            "rust".into(),
            DiscoveryMethod::Manual,
            true,
        );

        let entry = reg.lookup("tokio").unwrap();
        assert_eq!(entry.name, "tokio");
        assert_eq!(entry.path, PathBuf::from("/libs/tokio"));
        assert_eq!(entry.language, "rust");
        assert!(!entry.indexed);
        assert_eq!(entry.discovered_via, DiscoveryMethod::Manual);

        assert!(reg.lookup("nonexistent").is_none());
    }

    #[test]
    fn register_updates_existing_entry() {
        let mut reg = LibraryRegistry::new();
        reg.register(
            "serde".into(),
            PathBuf::from("/libs/serde-1.0"),
            "rust".into(),
            DiscoveryMethod::Manual,
            true,
        );

        // Mark as indexed
        reg.lookup_mut("serde").unwrap().indexed = true;

        // Re-register with same path: indexed should be preserved
        reg.register(
            "serde".into(),
            PathBuf::from("/libs/serde-1.0"),
            "rust".into(),
            DiscoveryMethod::LspFollowThrough,
            true,
        );
        let entry = reg.lookup("serde").unwrap();
        assert!(
            entry.indexed,
            "indexed should be preserved when path unchanged"
        );
        assert_eq!(entry.discovered_via, DiscoveryMethod::LspFollowThrough);

        // Re-register with different path: indexed should reset
        reg.register(
            "serde".into(),
            PathBuf::from("/libs/serde-2.0"),
            "rust".into(),
            DiscoveryMethod::Manual,
            true,
        );
        let entry = reg.lookup("serde").unwrap();
        assert!(!entry.indexed, "indexed should reset when path changes");
        assert_eq!(entry.path, PathBuf::from("/libs/serde-2.0"));
    }

    #[test]
    fn is_library_path_matches_and_misses() {
        let mut reg = LibraryRegistry::new();
        reg.register(
            "tokio".into(),
            PathBuf::from("/libs/tokio"),
            "rust".into(),
            DiscoveryMethod::Manual,
            true,
        );
        reg.register(
            "serde".into(),
            PathBuf::from("/libs/serde"),
            "rust".into(),
            DiscoveryMethod::Manual,
            true,
        );

        // Path inside tokio
        let found = reg.is_library_path(Path::new("/libs/tokio/src/runtime.rs"));
        assert_eq!(found.unwrap().name, "tokio");

        // Path inside serde
        let found = reg.is_library_path(Path::new("/libs/serde/src/de.rs"));
        assert_eq!(found.unwrap().name, "serde");

        // Path outside any library
        assert!(reg.is_library_path(Path::new("/other/file.rs")).is_none());
    }

    #[test]
    fn resolve_path_works() {
        let mut reg = LibraryRegistry::new();
        reg.register(
            "tokio".into(),
            PathBuf::from("/libs/tokio"),
            "rust".into(),
            DiscoveryMethod::Manual,
            true,
        );

        let resolved = reg.resolve_path("tokio", "src/runtime.rs").unwrap();
        assert_eq!(resolved, PathBuf::from("/libs/tokio/src/runtime.rs"));
    }

    #[test]
    fn resolve_path_errors_for_unknown_library() {
        let reg = LibraryRegistry::new();
        let result = reg.resolve_path("nonexistent", "src/lib.rs");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown library"));
    }

    #[test]
    fn persistence_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("libraries.json");

        let mut reg = LibraryRegistry::new();
        reg.register(
            "tokio".into(),
            PathBuf::from("/libs/tokio"),
            "rust".into(),
            DiscoveryMethod::LspFollowThrough,
            true,
        );
        reg.register(
            "serde".into(),
            PathBuf::from("/libs/serde"),
            "rust".into(),
            DiscoveryMethod::Manual,
            true,
        );
        reg.lookup_mut("serde").unwrap().indexed = true;
        reg.lookup_mut("serde").unwrap().version = Some("1.0.200".into());

        reg.save(&path).unwrap();
        let loaded = LibraryRegistry::load(&path).unwrap();

        assert_eq!(loaded.all().len(), 2);
        let tokio = loaded.lookup("tokio").unwrap();
        assert_eq!(tokio.discovered_via, DiscoveryMethod::LspFollowThrough);
        assert!(!tokio.indexed);

        let serde = loaded.lookup("serde").unwrap();
        assert!(serde.indexed);
        assert_eq!(serde.version, Some("1.0.200".into()));
    }

    #[test]
    fn load_missing_file_returns_empty() {
        let reg = LibraryRegistry::load(Path::new("/nonexistent/path/libraries.json")).unwrap();
        assert!(reg.all().is_empty());
    }

    #[test]
    fn all_returns_all_entries() {
        let mut reg = LibraryRegistry::new();
        assert!(reg.all().is_empty());

        reg.register(
            "a".into(),
            PathBuf::from("/a"),
            "rust".into(),
            DiscoveryMethod::Manual,
            true,
        );
        reg.register(
            "b".into(),
            PathBuf::from("/b"),
            "python".into(),
            DiscoveryMethod::LspFollowThrough,
            true,
        );
        assert_eq!(reg.all().len(), 2);
    }

    #[test]
    fn library_entry_serializes_version_fields() {
        let entry = LibraryEntry {
            name: "tokio".to_string(),
            version: Some("1.38.0".to_string()),
            version_indexed: Some("1.37.0".to_string()),
            db_file: Some("tokio.db".to_string()),
            nudge_dismissed: false,
            path: PathBuf::from("/tmp/tokio"),
            language: "rust".to_string(),
            indexed: true,
            discovered_via: DiscoveryMethod::LspFollowThrough,
            source_available: true,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("version_indexed"));
        assert!(json.contains("db_file"));
        assert!(json.contains("nudge_dismissed"));
    }

    #[test]
    fn library_entry_deserializes_without_new_fields() {
        // Old-format JSON without version_indexed, db_file, nudge_dismissed
        let json = r#"{"name":"serde","version":null,"path":"/tmp/serde","language":"rust","indexed":false,"discovered_via":"lsp_follow_through"}"#;
        let entry: LibraryEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.version_indexed, None);
        assert_eq!(entry.db_file, None);
        assert!(!entry.nudge_dismissed);
    }

    #[test]
    fn stale_libraries_detects_version_mismatch() {
        let mut registry = LibraryRegistry::new();
        registry.register(
            "tokio".into(),
            PathBuf::from("/tmp"),
            "rust".into(),
            DiscoveryMethod::LspFollowThrough,
            true,
        );
        let entry = registry.lookup_mut("tokio").unwrap();
        entry.version = Some("1.38.0".to_string());
        entry.version_indexed = Some("1.37.0".to_string());
        entry.indexed = true;

        let stale = registry.stale_libraries();
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].name, "tokio");
    }

    #[test]
    fn nudge_dismissed_resets_on_version_change() {
        let mut registry = LibraryRegistry::new();
        registry.register(
            "tokio".into(),
            PathBuf::from("/tmp"),
            "rust".into(),
            DiscoveryMethod::LspFollowThrough,
            true,
        );
        let entry = registry.lookup_mut("tokio").unwrap();
        entry.version = Some("1.37.0".to_string());
        entry.nudge_dismissed = true;

        registry.update_version("tokio", "1.38.0");
        let entry = registry.lookup("tokio").unwrap();
        assert!(!entry.nudge_dismissed);
        assert_eq!(entry.version.as_deref(), Some("1.38.0"));
    }

    #[test]
    fn source_available_defaults_to_true_on_deserialize() {
        let json = r#"{"name":"foo","path":"/tmp/foo","language":"rust",
                       "discovered_via":"manual","indexed":false,"nudge_dismissed":false}"#;
        let entry: LibraryEntry = serde_json::from_str(json).unwrap();
        assert!(
            entry.source_available,
            "missing field should default to true"
        );
    }

    #[test]
    fn register_with_source_available_false() {
        let mut reg = LibraryRegistry::new();
        reg.register(
            "jackson".into(),
            PathBuf::new(),
            "java".into(),
            DiscoveryMethod::ManifestScan,
            false,
        );
        let entry = reg.lookup("jackson").unwrap();
        assert!(!entry.source_available);
        assert!(entry.path.as_os_str().is_empty());
    }

    #[test]
    fn manifest_scan_does_not_overwrite_manual() {
        let mut reg = LibraryRegistry::new();
        reg.register(
            "foo".into(),
            PathBuf::from("/manual/path"),
            "rust".into(),
            DiscoveryMethod::Manual,
            true,
        );
        reg.register(
            "foo".into(),
            PathBuf::from("/scan/path"),
            "rust".into(),
            DiscoveryMethod::ManifestScan,
            true,
        );
        let entry = reg.lookup("foo").unwrap();
        assert_eq!(
            entry.path,
            PathBuf::from("/manual/path"),
            "ManifestScan must not overwrite Manual"
        );
    }

    #[test]
    fn manifest_scan_updates_existing_manifest_scan() {
        let mut reg = LibraryRegistry::new();
        reg.register(
            "foo".into(),
            PathBuf::from("/old"),
            "rust".into(),
            DiscoveryMethod::ManifestScan,
            true,
        );
        reg.register(
            "foo".into(),
            PathBuf::from("/new"),
            "rust".into(),
            DiscoveryMethod::ManifestScan,
            true,
        );
        let entry = reg.lookup("foo").unwrap();
        assert_eq!(entry.path, PathBuf::from("/new"));
    }

    #[test]
    fn discovery_method_manifest_scan_serializes() {
        let json = serde_json::to_string(&DiscoveryMethod::ManifestScan).unwrap();
        assert_eq!(json, "\"manifest_scan\"");
    }
}
/// `.codescout/libraries.json` is read on every activation and a repository can commit that path as
/// a link to a FIFO or a device.
/// docs/issues/2026-10-20-reads-of-repository-files-that-remain-unbounded-after-the-config-loader-fix.md
#[cfg(all(test, unix))]
mod bounded_load_tests {
    use super::*;
    use crate::util::fs::special_file_fixtures::{make_fifo, within_deadline};

    fn one_entry_json() -> String {
        let mut reg = LibraryRegistry::new();
        reg.register(
            "tokio".into(),
            PathBuf::from("/libs/tokio"),
            "rust".into(),
            DiscoveryMethod::Manual,
            true,
        );
        serde_json::to_string_pretty(&reg).unwrap()
    }

    /// A FIFO, and a link to one, as the registry file held every activation. It is an empty
    /// registry, as an absent file is.
    #[test]
    fn a_fifo_or_a_link_to_one_is_an_empty_registry_and_never_a_hang() {
        for linked in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("libraries.json");
            if linked {
                let fifo = dir.path().join("elsewhere-pipe");
                make_fifo(&fifo);
                std::os::unix::fs::symlink(&fifo, &path).unwrap();
            } else {
                make_fifo(&path);
            }
            let reg = within_deadline("LibraryRegistry::load on a FIFO", move || {
                let _keep = dir;
                LibraryRegistry::load(&path)
            })
            .unwrap_or_else(|e| panic!("linked={linked}: {e:#}"));
            assert!(reg.all().is_empty(), "linked={linked}");
        }
    }

    /// A device and a directory are refused the same way (`/dev/null`, not `/dev/zero`).
    #[test]
    fn a_device_link_and_a_directory_are_an_empty_registry() {
        let dir = tempfile::tempdir().unwrap();
        let dev = dir.path().join("dev.json");
        std::os::unix::fs::symlink("/dev/null", &dev).unwrap();
        assert!(LibraryRegistry::load(&dev).unwrap().all().is_empty());
        let sub = dir.path().join("dir.json");
        std::fs::create_dir(&sub).unwrap();
        assert!(LibraryRegistry::load(&sub).unwrap().all().is_empty());
    }

    /// Exactly at the limit still loads; one byte over is refused as an absent file. The padding
    /// is JSON whitespace, so the plain read would parse the over-limit file and find the entry.
    /// The limit is the registry's own 16 MiB, not the 1 MiB config limit: a file between the two
    /// still loads.
    #[test]
    fn a_registry_at_the_limit_loads_and_one_byte_over_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("libraries.json");
        let limit = MAX_LIBRARY_REGISTRY_BYTES as usize;
        let mut body = one_entry_json();
        body.push_str(&" ".repeat(limit - body.len()));
        std::fs::write(&path, &body).unwrap();
        assert_eq!(body.len(), limit);
        assert_eq!(LibraryRegistry::load(&path).unwrap().all().len(), 1);

        body.push(' ');
        std::fs::write(&path, &body).unwrap();
        assert!(LibraryRegistry::load(&path).unwrap().all().is_empty());

        let between = crate::util::fs::MAX_CONFIG_FILE_BYTES as usize + 1;
        assert!(between < limit);
        let mut body = one_entry_json();
        body.push_str(&" ".repeat(between - body.len()));
        std::fs::write(&path, &body).unwrap();
        assert_eq!(
            LibraryRegistry::load(&path).unwrap().all().len(),
            1,
            "a registry over the config limit and under its own still loads"
        );
    }

    /// Control: a regular file, and a link to one, load as before; invalid JSON, an empty file
    /// and invalid UTF-8 are still errors, and the error quotes none of the content.
    #[test]
    fn a_regular_registry_still_loads_and_a_broken_one_is_still_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("libraries.json");
        std::fs::write(&path, one_entry_json()).unwrap();
        assert_eq!(LibraryRegistry::load(&path).unwrap().all().len(), 1);

        let link = dir.path().join("link.json");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert_eq!(LibraryRegistry::load(&link).unwrap().all().len(), 1);

        std::fs::write(&path, "SECRETMARKER-77 not json").unwrap();
        let err = format!("{:#}", LibraryRegistry::load(&path).unwrap_err());
        assert!(!err.contains("SECRETMARKER"), "{err}");
        std::fs::write(&path, "").unwrap();
        assert!(LibraryRegistry::load(&path).is_err());
        std::fs::write(&path, [0xff, 0xfe]).unwrap();
        assert!(LibraryRegistry::load(&path).is_err());
    }
}
