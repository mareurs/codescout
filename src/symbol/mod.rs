//! Symbol-level provider: LSP/AST lookup, classification, and text-edit
//! application. Lifted out of `src/tools/symbol/` during refactor Phase 6.1
//! so tool files stay thin adapters over domain logic.

pub mod edit;
pub mod query;
