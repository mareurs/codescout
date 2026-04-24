//! A `String` newtype that refuses to disclose itself in `Debug` output.
//!
//! Use for API keys, bearer tokens, and any field whose accidental appearance
//! in a `tracing::debug!(?config)` statement or a diagnostic dump would be a
//! security incident. The `Serialize`/`Deserialize` impls pass the value
//! through unchanged — the protection is purely against `Debug`-format leakage.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A string that hides its value from `Debug` output.
///
/// `Display` / `Serialize` / `AsRef<str>` pass the value through unchanged.
/// The only thing that changes vs. `String` is the `Debug` format, which
/// always prints `"<redacted>"` (or `"<redacted-empty>"` for empty values,
/// so log readers can still tell "set" apart from "unset" without seeing
/// the actual secret).
#[derive(Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SensitiveString(String);

impl SensitiveString {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for SensitiveString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            f.write_str("SensitiveString(<redacted-empty>)")
        } else {
            f.write_str("SensitiveString(<redacted>)")
        }
    }
}

impl fmt::Display for SensitiveString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Display is intentionally pass-through — this is for when the caller
        // explicitly wants the value (e.g. HTTP header construction). Debug
        // is the one that hides it. Do not change this without reviewing the
        // call sites that rely on Display.
        self.0.fmt(f)
    }
}

impl AsRef<str> for SensitiveString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<String> for SensitiveString {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for SensitiveString {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_hides_value() {
        let s = SensitiveString::new("sk-abcdef1234567890");
        let rendered = format!("{s:?}");
        assert!(
            !rendered.contains("sk-abcdef"),
            "debug format must not leak the value, got {rendered}"
        );
        assert!(rendered.contains("redacted"));
    }

    #[test]
    fn debug_distinguishes_empty_from_set() {
        let empty = SensitiveString::default();
        let set = SensitiveString::new("x");
        assert_ne!(format!("{empty:?}"), format!("{set:?}"));
    }

    #[test]
    fn display_passes_value_through() {
        let s = SensitiveString::new("value");
        assert_eq!(format!("{s}"), "value");
    }

    #[test]
    fn serde_roundtrips_transparently() {
        let s = SensitiveString::new("secret");
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "\"secret\"");
        let back: SensitiveString = serde_json::from_str(&json).unwrap();
        assert_eq!(back.as_str(), "secret");
    }
}
