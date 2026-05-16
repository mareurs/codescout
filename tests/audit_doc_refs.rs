// Entry point for the audit_doc_refs Tier-2 fixture corpus and Tier-3 eval.
// Rust only auto-discovers .rs files directly under tests/ — sub-modules
// must be declared here via #[path = "..."].
#[path = "librarian/audit_doc_refs/corpus.rs"]
mod corpus;
#[path = "librarian/audit_doc_refs/eval_on_codescout_self.rs"]
mod eval_on_codescout_self;
