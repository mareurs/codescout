//! The on-disk form of an augmentation's **shape**.
//!
//! Augmentation was the one artifact state with no on-disk form: bodies, frontmatter and
//! catalog rows all rebuild from disk on `reindex`, but `prompt`, `params_schema`,
//! `render_template`, `entry_collection`, `append_mode` and `history_cap` lived only in
//! `catalog.db` — machine-local and gitignored. So they did not travel, and the absence was
//! silent in every direction: `reindex` preserves augmentation keyed by id rather than
//! regenerating it, so it reported healthy and repaired nothing.
//!
//! This module is the half that travels. `params` deliberately does NOT: it is live state
//! that churns, and committing it would recreate the params-vs-body drift class BL-29 /
//! BL-40 / BL-42 closed. A restored augmentation starts with empty params and refills from
//! use.
//!
//! docs/issues/2026-08-28-augmentation-declaration-records-existence-not-shape.md

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::librarian::catalog::augmentation::AugmentationRow;

/// Bump when a field's MEANING changes, never merely when one is added — every field
/// carries `#[serde(default)]` precisely so an older sidecar keeps parsing.
pub const SCHEMA_VERSION: u32 = 1;

/// Repo-relative directory holding the sidecars.
///
/// Not `.codescout/` — that tree reads as runtime state and most of it is gitignored, and
/// the whole point of this file is to be committed. Not a dot-directory either: codescout's
/// own `grep` prunes hidden paths by default, which would make these invisible to the tool
/// most likely to go looking for them.
pub const SIDECAR_DIR: &str = "docs/augmentations";

fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// The shape of an augmentation, as committed.
///
/// Every field but `prompt` is `#[serde(default)]`, for the reason `index_state.rs` learned
/// the hard way: a parse failure is indistinguishable from "no sidecar", so a field added
/// later must never invalidate a file written before it existed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AugmentationSidecar {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// The persistent instruction. Authored prose, so it travels.
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_collection: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params_schema: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_template: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub append_mode: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_cap: Option<i64>,
}

/// Where an artifact's sidecar lives, given the artifact's path.
///
/// Keyed on the file STEM rather than the 16-hex artifact id, deliberately: the id is
/// `sha256(abs_path)` and is re-minted by every `artifact(action="move")`, so an
/// id-named sidecar would be orphaned by the archive flow — the single most common
/// thing that happens to these files.
pub fn path_for(repo_root: &Path, artifact_abs_path: &Path) -> PathBuf {
    let stem = artifact_abs_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unnamed".to_string());
    repo_root.join(SIDECAR_DIR).join(format!("{stem}.yaml"))
}

/// The repo-relative form written into the artifact's `expects_augmentation:` declaration.
pub fn rel_path_for(artifact_abs_path: &Path) -> String {
    let stem = artifact_abs_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unnamed".to_string());
    format!("{SIDECAR_DIR}/{stem}.yaml")
}

impl AugmentationSidecar {
    /// Project a catalog row onto its committable shape, dropping `params` and every
    /// bookkeeping column (`refresh_count`, `last_refreshed_at`, `refreshed_at_commit`,
    /// timestamps) — those describe this machine's history with the artifact, not the
    /// artifact.
    pub fn from_row(row: &AugmentationRow) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            prompt: row.prompt.clone(),
            entry_collection: row.entry_collection.clone(),
            // Stored in the catalog as JSON text; parsed here so the sidecar carries a real
            // YAML mapping a human can read and edit rather than an escaped one-line string.
            params_schema: row
                .params_schema
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok()),
            render_template: row.render_template.clone(),
            append_mode: row.append_mode,
            history_cap: row.history_cap,
        }
    }

    /// Build a catalog row for a FRESH attach. `params` starts empty by construction — the
    /// data half never travels, so a restore gives you a working tracker with no rows, not
    /// a tracker with someone else's rows.
    pub fn to_row(&self, artifact_id: &str) -> AugmentationRow {
        // Same format the augment tool writes, so a restored row is indistinguishable from
        // a hand-attached one in the column that dates it.
        let now = chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        AugmentationRow {
            artifact_id: artifact_id.to_string(),
            prompt: self.prompt.clone(),
            params: "{}".to_string(),
            last_refreshed_at: None,
            refresh_count: 0,
            created_at: now.clone(),
            updated_at: now,
            render_template: self.render_template.clone(),
            params_schema: self
                .params_schema
                .as_ref()
                .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "{}".to_string())),
            append_mode: self.append_mode,
            history_cap: self.history_cap,
            entry_collection: self.entry_collection.clone(),
            refreshed_at_commit: None,
        }
    }
}

/// Read a sidecar. A malformed file is an ERROR, not a silent `None`.
///
/// `index_state.rs` treats a parse failure as "no sidecar" and that is right for it — a
/// missing index state means "never indexed", which is recoverable by indexing. Here the
/// same silence would mean "this augmentation has no recorded shape", which is exactly the
/// state this whole mechanism exists to make loud.
pub fn read(path: &Path) -> Result<AugmentationSidecar> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading augmentation sidecar {}", path.display()))?;
    serde_yml::from_str(&text)
        .with_context(|| format!("parsing augmentation sidecar {}", path.display()))
}

pub fn write(path: &Path, sidecar: &AugmentationSidecar) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let yaml = serde_yml::to_string(sidecar).context("serializing augmentation sidecar")?;
    std::fs::write(path, yaml).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> AugmentationRow {
        AugmentationRow {
            artifact_id: "abc123".into(),
            prompt: "Maintain the T-N table.".into(),
            params: r#"{"observations":[{"id":"T-1"}]}"#.into(),
            last_refreshed_at: Some("2026-08-30T00:00:00Z".into()),
            refresh_count: 7,
            created_at: "2026-08-01T00:00:00Z".into(),
            updated_at: "2026-08-30T00:00:00Z".into(),
            render_template: Some("{{ observations | length }}".into()),
            params_schema: Some(r#"{"type":"object"}"#.into()),
            append_mode: true,
            history_cap: Some(20),
            entry_collection: Some("observations".into()),
            refreshed_at_commit: Some("deadbeef".into()),
        }
    }

    #[test]
    fn round_trips_every_shape_field() {
        let s = AugmentationSidecar::from_row(&row());
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.yaml");
        write(&p, &s).unwrap();
        assert_eq!(read(&p).unwrap(), s);
    }

    /// The load-bearing exclusion. `params` is live state; committing it would recreate the
    /// params-vs-body drift class BL-29/BL-40/BL-42 closed.
    ///
    /// Asserted on a PARSED document rather than by substring: `params_schema` contains the
    /// substring `params`, so `!yaml.contains("params")` would fail on a correct file and
    /// `!yaml.contains("params:")` would pass on a file that emitted `params :`. Only asking
    /// the YAML for a top-level key named exactly `params` answers the question.
    #[test]
    fn a_written_sidecar_carries_no_params_key() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.yaml");
        write(&p, &AugmentationSidecar::from_row(&row())).unwrap();

        let raw = std::fs::read_to_string(&p).unwrap();
        let parsed: serde_json::Value = serde_yml::from_str(&raw).unwrap();
        let keys: Vec<&str> = parsed
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();

        assert!(
            !keys.contains(&"params"),
            "sidecar must not carry params; got keys {keys:?}"
        );
        // Control: the test can distinguish the two similar keys, so the assertion above is
        // about `params` and not about the absence of anything params-shaped.
        assert!(
            keys.contains(&"params_schema"),
            "params_schema must survive; got {keys:?}"
        );
    }

    /// Bookkeeping columns describe this machine's history with the artifact, not the
    /// artifact. A restored row must start clean rather than inherit another machine's
    /// refresh count.
    #[test]
    fn to_row_starts_with_empty_params_and_no_refresh_history() {
        let restored = AugmentationSidecar::from_row(&row()).to_row("other-id");
        assert_eq!(restored.params, "{}");
        assert_eq!(restored.refresh_count, 0);
        assert_eq!(restored.last_refreshed_at, None);
        assert_eq!(restored.refreshed_at_commit, None);
        // ...while every shape field survives the trip.
        assert_eq!(restored.entry_collection.as_deref(), Some("observations"));
        assert_eq!(restored.history_cap, Some(20));
        assert!(restored.append_mode);
    }

    /// `index_state.rs`'s convention, inherited: a sidecar written before a field existed
    /// must still parse. Without `#[serde(default)]` this whole document fails, and a failed
    /// read means "no recorded shape" — the exact state the mechanism exists to prevent.
    #[test]
    fn a_sidecar_carrying_only_a_prompt_still_parses() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("old.yaml");
        std::fs::write(&p, "prompt: just the instruction\n").unwrap();

        let s = read(&p).expect("a minimal sidecar must parse");
        assert_eq!(s.prompt, "just the instruction");
        assert_eq!(s.schema_version, SCHEMA_VERSION);
        assert_eq!(s.entry_collection, None);
        assert!(!s.append_mode);
    }

    /// A malformed sidecar must ERROR rather than read as absent — the same argument
    /// `doctor::parse_declaration` makes about a declaration that silently does not count.
    #[test]
    fn a_malformed_sidecar_errors_rather_than_reading_as_absent() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("bad.yaml");
        std::fs::write(&p, "prompt: [this is not a string\n").unwrap();
        assert!(
            read(&p).is_err(),
            "a malformed sidecar must not read as an empty one"
        );
    }

    #[test]
    fn path_is_keyed_on_the_file_stem_not_the_artifact_id() {
        let root = Path::new("/repo");
        let art = Path::new("/repo/docs/trackers/tool-usage-patterns.md");
        assert_eq!(
            path_for(root, art),
            Path::new("/repo/docs/augmentations/tool-usage-patterns.yaml")
        );
        assert_eq!(
            rel_path_for(art),
            "docs/augmentations/tool-usage-patterns.yaml"
        );
    }
}
