//! Legibility scan engine — ranks refactor candidates from usage.db friction
//! and the AST symbol index. Pure: (db conn, project root) -> ranked candidates.
//! Phase 2a of docs/superpowers/specs/2026-06-13-dzo-friction-probes-design.md.

use crate::lsp::symbols::{SymbolInfo, SymbolKind};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

/// A structural legibility defect kind — the entry gate. A candidate must have one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Defect {
    OverBudgetBody,
    NameCollision,
    UnMappableFile,
}

/// Tier 1 = biting now (has recorder friction); Tier 2 = latent (structural only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    BitingNow,
    Latent,
}

impl Tier {
    /// 1 for biting-now, 2 for latent — the spec's numeric tier.
    pub fn rank(self) -> u8 {
        match self {
            Tier::BitingNow => 1,
            Tier::Latent => 2,
        }
    }
}

/// Observed cost from the recorder, per target. `retries` is reserved (0 in v1 —
/// same-input-repeat detection is a follow-up).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Friction {
    pub truncations: u32,
    pub retries: u32,
    pub code_class_edit_fails: u32,
    pub other: u32,
    pub sessions: u32,
}

impl Friction {
    pub fn is_empty(&self) -> bool {
        self.truncations == 0 && self.retries == 0 && self.code_class_edit_fails == 0 && self.other == 0
    }
    /// score = 3*truncations + 2*retries + 2*code_class_edit_fails + 1*other.
    /// Infra-class err_family never reaches `code_class_edit_fails` (excluded in the
    /// recorder query), so tool-class noise cannot inflate a code candidate.
    pub fn score(&self) -> u32 {
        3 * self.truncations + 2 * self.retries + 2 * self.code_class_edit_fails + self.other
    }
}

/// A raw structural finding from the index lane, before scoring/tiering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralDefect {
    pub rel_file: String,
    pub name_path: String, // "(file)" for UnMappableFile
    pub defect: Defect,
    pub tokens: usize,
    pub lines: u32,
}

/// A parsed source file: its rel path, its source lines, and its symbol tree.
pub struct FileSymbols {
    pub rel_file: String,
    pub lines: Vec<String>,
    pub symbols: Vec<SymbolInfo>,
}

/// A ranked refactor candidate — the engine's output unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Candidate {
    pub key: String, // "<rel_file>::<name_path>"
    pub rel_file: String,
    pub name_path: String,
    pub defect: Defect,
    pub tier: Tier,
    pub tokens: usize,
    pub budget: usize,
    pub lines: u32,
    pub friction: Friction,
    pub score: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friction_score_and_emptiness() {
        let empty = Friction::default();
        assert!(empty.is_empty());
        assert_eq!(empty.score(), 0);

        let f = Friction { truncations: 14, retries: 0, code_class_edit_fails: 1, other: 2, sessions: 2 };
        assert!(!f.is_empty());
        // 3*14 + 2*0 + 2*1 + 1*2 = 46
        assert_eq!(f.score(), 46);
    }
}
