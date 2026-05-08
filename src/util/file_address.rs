//! Canonical file-system address that knows how to render itself as both an
//! OS path and an LSP `file://` URI.
//!
//! Centralizes path↔URI conversions that previously lived in three separate
//! helpers (`lsp/client.rs`, `fs/mod.rs`, `tools/symbol/call_edges/resolver.rs`).
//! Each duplicated the same `url::Url`-based round-trip with subtle differences
//! in fallback behavior. `FileAddress` captures the conversion in one place
//! and the standalone helpers now delegate here.

use anyhow::Result;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileAddress {
    path: PathBuf,
}

impl FileAddress {
    /// Construct from a filesystem path. The path is stored as-is (relative or
    /// absolute). `as_lsp_uri()` will canonicalize relative paths against the
    /// current working directory at conversion time.
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Parse an LSP `lsp_types::Uri` into a `FileAddress`.
    ///
    /// Falls back to the raw URI path string when `url::Url::parse` fails —
    /// some LSP servers emit non-RFC-compliant URIs that are still useful when
    /// interpreted as plain paths. Returns `None` only when both the parse and
    /// the raw-path fallback yield nothing usable (empty path).
    pub fn from_lsp_uri(uri: &lsp_types::Uri) -> Option<Self> {
        url::Url::parse(uri.as_str())
            .ok()
            .and_then(|u| u.to_file_path().ok())
            .or_else(|| {
                let s = uri.path().as_str();
                if s.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(s))
                }
            })
            .map(Self::from_path)
    }

    /// Parse a string-form URI (`"file://..."`) into a `FileAddress`. Stricter
    /// than `from_lsp_uri`: no raw-path fallback, since callers of this entry
    /// point expect well-formed URIs.
    pub fn from_uri_str(uri: &str) -> Option<Self> {
        url::Url::parse(uri)
            .ok()
            .and_then(|u| u.to_file_path().ok())
            .map(Self::from_path)
    }

    /// Borrow as a `&Path`.
    pub fn as_path(&self) -> &Path {
        &self.path
    }

    /// Take the inner `PathBuf`.
    pub fn into_path(self) -> PathBuf {
        self.path
    }

    /// Render as an LSP `Uri`. Relative paths are canonicalized against the
    /// current working directory. Returns an error if the path cannot be
    /// converted (non-UTF-8 segments, no current dir, etc.).
    pub fn as_lsp_uri(&self) -> Result<lsp_types::Uri> {
        let abs = if self.path.is_absolute() {
            self.path.clone()
        } else {
            std::env::current_dir()?.join(&self.path)
        };
        let u = url::Url::from_file_path(&abs)
            .map_err(|_| anyhow::anyhow!("cannot convert path to URI: {}", abs.display()))?;
        u.as_str()
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid URI: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_path_round_trips_absolute() {
        let p = if cfg!(windows) {
            PathBuf::from("C:\\tmp\\foo.rs")
        } else {
            PathBuf::from("/tmp/foo.rs")
        };
        let addr = FileAddress::from_path(&p);
        assert_eq!(addr.as_path(), p.as_path());
        let uri = addr.as_lsp_uri().expect("absolute path -> URI");
        let back = FileAddress::from_lsp_uri(&uri).expect("URI -> path");
        assert_eq!(back.as_path(), p.as_path());
    }

    #[test]
    fn from_lsp_uri_unix_uri() {
        let uri: lsp_types::Uri = "file:///tmp/foo.rs".parse().unwrap();
        let addr = FileAddress::from_lsp_uri(&uri).expect("parse");
        if cfg!(unix) {
            assert_eq!(addr.as_path(), Path::new("/tmp/foo.rs"));
        }
    }

    #[test]
    fn from_lsp_uri_falls_back_to_raw_path() {
        // Construct a URI whose scheme is unsupported by `url::Url::to_file_path`
        // but whose `path()` component is non-empty. We expect the raw-path
        // fallback to engage.
        let uri: lsp_types::Uri = "weird:///some/path".parse().unwrap();
        let addr = FileAddress::from_lsp_uri(&uri);
        assert!(addr.is_some(), "raw-path fallback should produce a value");
    }

    #[test]
    fn from_uri_str_returns_none_for_garbage() {
        assert!(FileAddress::from_uri_str("not-a-uri").is_none());
    }
}
