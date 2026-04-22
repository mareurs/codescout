//! Symbol-level tools backed by the LSP client.

use super::ToolContext;

#[cfg(test)]
#[allow(unused_imports)]
use std::path::{Path, PathBuf};

#[cfg(test)]
#[allow(unused_imports)]
use crate::tools::RecoverableError;
#[cfg(test)]
#[allow(unused_imports)]
use serde_json::{json, Value};

#[cfg(test)]
use super::output::{OutputGuard, OverflowInfo};
#[cfg(test)]
#[allow(unused_imports)]
use super::{optional_u64_param, parse_bool_param, Tool};
#[cfg(test)]
#[allow(unused_imports)]
use crate::lsp::SymbolInfo;

mod display;
mod edit_helpers;
mod find_references;
mod find_symbol;
mod goto_definition;
mod hover;
mod insert_code;
mod list_symbols;
mod path_helpers;
mod remove_symbol;
mod rename_symbol;
mod replace_symbol;
mod symbol_query;

#[cfg(test)]
mod tests;

// Transitional re-imports so existing bodies and `use super::*;` inside the
// `mod tests {}` block keep resolving these helpers from this module. Removed
// in Phase 1b.5 once call sites import from `path_helpers` directly.
#[allow(unused_imports)]
use edit_helpers::{
    apply_text_edits, clamp_range_to_parent, collect_all_name_paths, editing_end_line,
    editing_start_line, find_ast_name_path, find_insert_before_line, find_parent_symbol,
    text_sweep, write_lines, TextualMatch,
};
#[allow(unused_imports)]
use path_helpers::{
    classify_reference_path, format_library_path, get_lsp_client, get_path_param,
    guard_not_markdown, is_glob, path_in_excluded_dir, require_path_param, resolve_glob,
    resolve_library_roots, resolve_read_path, resolve_write_path, tag_external_path, uri_to_path,
    LspTimer,
};
#[cfg(test)]
#[allow(unused_imports)]
use symbol_query::find_symbol_by_name_path;
#[allow(unused_imports)]
use symbol_query::{
    collect_matching, collect_matching_symbols, count_symbols_by_name_path, fetch_validated_symbol,
    filter_variable_symbols, find_ast_end_line_in, find_matching_symbol,
    find_unique_symbol_by_name_path, is_lead_in_line, matches_kind_filter,
    resolve_range_via_document_symbols, symbol_name_matches, symbol_to_json,
    validate_symbol_position, validate_symbol_range,
};

pub use find_references::FindReferences;
pub use find_symbol::FindSymbol;
pub use goto_definition::GotoDefinition;
pub use hover::Hover;
pub use insert_code::InsertCode;
pub use list_symbols::ListSymbols;
pub use remove_symbol::RemoveSymbol;
pub use rename_symbol::RenameSymbol;
pub use replace_symbol::ReplaceSymbol;

#[cfg(test)]
use display::format_list_symbols;
#[cfg(test)]
use list_symbols::{flat_symbol_count, LIST_SYMBOLS_SINGLE_FILE_FLAT_CAP};
