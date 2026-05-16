// src/librarian/tools/audit_doc_refs/mod.rs
use serde::{Deserialize, Serialize};

pub mod parser;
pub mod resolver;
pub mod severity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefKind {
    FilePath,
    FileLine,
    FileSymbol,
    ModulePath,
    Link,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefPosition {
    InlineSpan,
    FencedBlock,
    LinkTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefCandidate {
    pub md_file: String,
    pub md_line: u32,
    pub raw_ref: String,
    pub ref_kind: RefKind,
    pub position: RefPosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParseWarning {
    pub md_file: String,
    pub line: u32,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Resolved,
    Missing,
    FileMissing,
    SymbolMissing,
    LineOob,
    AnchorMissing,
    Unknown,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    High,
    Med,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    pub verdict: Verdict,
    pub severity: Severity,
    pub severity_reason: &'static str,
    pub notes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub candidate: RefCandidate,
    pub resolution: Resolution,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanMeta {
    pub last_scan_at: Option<String>,
    pub last_scan_commit: Option<String>,
    pub n_files_scanned: u32,
    pub n_refs_found: u32,
    pub degraded: bool,
    pub lsp_languages_offline: Vec<String>,
}
