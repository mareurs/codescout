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

fn is_body_bearing(kind: &SymbolKind) -> bool {
    matches!(kind, SymbolKind::Function | SymbolKind::Method | SymbolKind::Constructor)
}

/// Recurse the symbol tree, collecting body-bearing symbols.
fn collect_bodies<'a>(syms: &'a [SymbolInfo], out: &mut Vec<&'a SymbolInfo>) {
    for s in syms {
        if is_body_bearing(&s.kind) {
            out.push(s);
        }
        collect_bodies(&s.children, out);
    }
}

/// The source text of a symbol's body, plus its line count. Empty when the range
/// is degenerate or out of bounds.
fn body_text(lines: &[String], sym: &SymbolInfo) -> (String, u32) {
    if lines.is_empty() {
        return (String::new(), 0);
    }
    let start = sym.range_start_line.unwrap_or(sym.start_line) as usize;
    let end = (sym.end_line as usize).min(lines.len() - 1);
    if start > end {
        return (String::new(), 0);
    }
    (lines[start..=end].join("\n"), (end - start + 1) as u32)
}

/// Index-lane detector: function/method bodies that exceed the inline budget
/// (so `symbols(include_body=true)` truncates them).
pub fn over_budget_bodies(files: &[FileSymbols]) -> Vec<StructuralDefect> {
    let mut out = Vec::new();
    for f in files {
        let mut bodies = Vec::new();
        collect_bodies(&f.symbols, &mut bodies);
        for sym in bodies {
            let (body, lines) = body_text(&f.lines, sym);
            if !body.is_empty() && crate::tools::exceeds_inline_limit(&body) {
                out.push(StructuralDefect {
                    rel_file: f.rel_file.clone(),
                    name_path: sym.name_path.clone(),
                    defect: Defect::OverBudgetBody,
                    tokens: body.len() / 4,
                    lines,
                });
            }
        }
    }
    out
}

/// Recurse the symbol tree, collecting ALL symbols (any kind).
fn collect_all<'a>(syms: &'a [SymbolInfo], out: &mut Vec<&'a SymbolInfo>) {
    for s in syms {
        out.push(s);
        collect_all(&s.children, out);
    }
}

/// Index-lane detector: a name_path that resolves to more than one symbol within
/// the same file (e.g. an inherent impl + a trait impl method) — the ambiguity
/// that hard-fails `edit_code`. Cross-file ambiguity is a softer signal, deferred.
pub fn name_collisions(files: &[FileSymbols]) -> Vec<StructuralDefect> {
    let mut out = Vec::new();
    for f in files {
        let mut all = Vec::new();
        collect_all(&f.symbols, &mut all);
        let mut counts: HashMap<&str, u32> = HashMap::new();
        for s in &all {
            *counts.entry(s.name_path.as_str()).or_insert(0) += 1;
        }
        let mut keys: Vec<&str> = counts.iter().filter(|(_, c)| **c > 1).map(|(k, _)| *k).collect();
        keys.sort_unstable(); // deterministic output order
        for np in keys {
            out.push(StructuralDefect {
                rel_file: f.rel_file.clone(),
                name_path: np.to_string(),
                defect: Defect::NameCollision,
                tokens: 0,
                lines: 0,
            });
        }
    }
    out
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

    fn sym(name_path: &str, kind: SymbolKind, start: u32, end: u32) -> SymbolInfo {
        SymbolInfo {
            name: name_path.rsplit('/').next().unwrap_or(name_path).to_string(),
            name_path: name_path.to_string(),
            kind,
            file: std::path::PathBuf::from("x.rs"),
            start_line: start,
            end_line: end,
            range_start_line: None,
            start_col: 0,
            children: vec![],
            detail: None,
        }
    }

    fn file_with(rel: &str, body_lines: usize, syms: Vec<SymbolInfo>) -> FileSymbols {
        // each line is 200 bytes so `body_lines` lines ≈ body_lines*200 bytes
        FileSymbols {
            rel_file: rel.to_string(),
            lines: (0..body_lines).map(|_| "x".repeat(200)).collect(),
            symbols: syms,
        }
    }

    #[test]
    fn over_budget_bodies_flags_only_over_budget_functions() {
        // big fn spans lines 0..=70 → ~70*201 ≈ 14k bytes > 10k budget
        let big = sym("Foo/big", SymbolKind::Method, 0, 70);
        // small fn spans lines 0..=5 → ~6*201 ≈ 1.2k bytes < budget
        let small = sym("Foo/small", SymbolKind::Method, 0, 5);
        let files = vec![file_with("src/foo.rs", 71, vec![big, small])];
        let defects = over_budget_bodies(&files);
        assert_eq!(defects.len(), 1, "only the big body");
        assert_eq!(defects[0].name_path, "Foo/big");
        assert_eq!(defects[0].defect, Defect::OverBudgetBody);
        assert!(defects[0].tokens > crate::tools::MAX_INLINE_TOKENS);
    }

    #[test]
    fn over_budget_ignores_non_body_kinds() {
        // a Struct spanning many lines is NOT a body-bearing symbol
        let s = sym("BigStruct", SymbolKind::Struct, 0, 70);
        let files = vec![file_with("src/foo.rs", 71, vec![s])];
        assert!(over_budget_bodies(&files).is_empty());
    }

    #[test]
    fn name_collisions_flags_same_file_duplicate_name_path() {
        // inherent impl + trait impl both expose "LspManager/get_or_start"
        let a = sym("LspManager/get_or_start", SymbolKind::Method, 0, 5);
        let b = sym("LspManager/get_or_start", SymbolKind::Method, 10, 15);
        let unique = sym("LspManager/do_start", SymbolKind::Method, 20, 25);
        let files = vec![file_with("src/lsp/manager.rs", 30, vec![a, b, unique])];
        let defects = name_collisions(&files);
        assert_eq!(defects.len(), 1);
        assert_eq!(defects[0].name_path, "LspManager/get_or_start");
        assert_eq!(defects[0].defect, Defect::NameCollision);
    }

}
