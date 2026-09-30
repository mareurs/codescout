//! Correct pyright's name-only ranges for Python assignments.
//!
//! pyright reports a variable's `DocumentSymbol.range` as the name node alone, so a
//! multi-line constant arrives as a one-line symbol. Every consumer of the range then
//! agrees on the wrong answer: `symbols(include_body=true)` shows `NAME = (` as the whole
//! body, and `edit_code(replace)` rewrites that one line and orphans the rest.
//! BUG docs/issues/2026-09-24-python-multiline-constant-range-is-its-first-line.md

use std::collections::HashMap;

use tree_sitter::{Node, Parser};

use crate::lsp::symbols::{SymbolInfo, SymbolKind};

/// Extend the `end_line` of every Python `Constant`/`Variable` symbol that names the
/// target of an assignment statement to that statement's last line. Recurses into
/// children. Never shrinks a range.
///
/// Symbols are matched to statements by the `(row, column)` of the target identifier,
/// which is exactly where pyright puts `selectionRange.start`. The row alone is not
/// enough: `a = 1; b = (...)` puts two statements on one line. The column is compared
/// as-is although tree-sitter counts bytes and LSP counts UTF-16 units; they differ
/// only after non-ASCII text earlier on the line, and a mismatch leaves the range
/// short (today's behaviour), never wrong.
///
/// Takes any iterator of symbols so a caller holding a multi-file list can pass one
/// file's symbols in place, without reordering the list.
///
/// Source that does not parse cleanly is left alone — an error-recovered tree can give
/// a statement an end row it does not have, and a wrong range is worse than a short one.
pub fn extend_python_assignment_ranges<'a>(
    symbols: impl IntoIterator<Item = &'a mut SymbolInfo>,
    source: &str,
) {
    let Some(ends) = assignment_target_ends(source) else {
        return;
    };
    if ends.is_empty() {
        return;
    }
    extend_with(symbols, &ends);
}

fn extend_with<'a>(
    symbols: impl IntoIterator<Item = &'a mut SymbolInfo>,
    ends: &HashMap<(u32, u32), u32>,
) {
    for sym in symbols {
        if matches!(sym.kind, SymbolKind::Constant | SymbolKind::Variable) {
            if let Some(&end) = ends.get(&(sym.start_line, sym.start_col)) {
                sym.end_line = sym.end_line.max(end);
            }
        }
        extend_with(sym.children.iter_mut(), ends);
    }
}

/// Map each assignment target identifier's `(row, column)` to the last row of the
/// statement that assigns it. `None` when the source does not parse cleanly.
fn assignment_target_ends(source: &str) -> Option<HashMap<(u32, u32), u32>> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .ok()?;
    let tree = parser.parse(source, None)?;
    let root = tree.root_node();
    if root.has_error() {
        return None;
    }
    let mut ends = HashMap::new();
    collect(root, &mut ends);
    Some(ends)
}

/// Walk the whole tree: pyright reports assignments at module level, in class bodies,
/// and inside functions, and each is name-only.
fn collect(node: Node, ends: &mut HashMap<(u32, u32), u32>) {
    if node.kind() == "assignment" {
        let end_row = node.end_position().row as u32;
        if let Some(left) = node.child_by_field_name("left") {
            record_targets(left, end_row, ends);
        }
        // `a = b = (...)` nests the second assignment in `right`; the recursion below
        // records its targets too.
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect(child, ends);
    }
}

/// Record every identifier in an assignment's target, so `a, b = (...)` extends both.
fn record_targets(node: Node, end_row: u32, ends: &mut HashMap<(u32, u32), u32>) {
    if node.kind() == "identifier" {
        let start = node.start_position();
        ends.insert((start.row as u32, start.column as u32), end_row);
        return;
    }
    // An attribute or subscript target (`self.x = ...`, `d[k] = ...`) names no symbol
    // pyright would report at this position.
    if matches!(node.kind(), "attribute" | "subscript") {
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        record_targets(child, end_row, ends);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A symbol as pyright reports it: `start_line`/`start_col` at the name, and a
    /// range that ends on the same line (the name node only). 0-indexed, like LSP.
    fn sym(name: &str, kind: SymbolKind, line: u32, col: u32) -> SymbolInfo {
        SymbolInfo {
            name: name.into(),
            name_path: name.into(),
            kind,
            file: PathBuf::from("x.py"),
            start_line: line,
            end_line: line,
            start_col: col,
            range_start_line: Some(line),
            children: vec![],
            detail: None,
        }
    }

    fn extended(source: &str, mut syms: Vec<SymbolInfo>) -> Vec<SymbolInfo> {
        extend_python_assignment_ranges(&mut syms, source);
        syms
    }

    #[test]
    fn multi_line_tuple_constant_extends_to_its_closing_line() {
        let src = "MARKERS = (\n    \"alpha\",\n    \"beta\",\n)\n\nOTHER = 1\n";
        let out = extended(src, vec![sym("MARKERS", SymbolKind::Constant, 0, 0)]);
        assert_eq!(out[0].end_line, 3, "closing `)` is on line 3");
    }

    #[test]
    fn annotated_assignment_extends_too() {
        // The shape from the source report: a typed tuple constant.
        let src = "DELETE: tuple[str, ...] = (\n    \"a\",\n)\n";
        let out = extended(src, vec![sym("DELETE", SymbolKind::Constant, 0, 0)]);
        assert_eq!(out[0].end_line, 2);
    }

    #[test]
    fn variable_kind_is_extended_as_well_as_constant() {
        // pyright reports lower-case module names as Variable, not Constant.
        let src = "config = {\n    \"k\": 1,\n}\n";
        let out = extended(src, vec![sym("config", SymbolKind::Variable, 0, 0)]);
        assert_eq!(out[0].end_line, 2);
    }

    #[test]
    fn single_line_assignment_is_unchanged() {
        let src = "A = 1\nB = (\n    2,\n)\n";
        let out = extended(src, vec![sym("A", SymbolKind::Constant, 0, 0)]);
        assert_eq!(
            out[0].end_line, 0,
            "must not borrow B's range from the next line"
        );
    }

    #[test]
    fn two_statements_on_one_line_are_told_apart_by_column() {
        // Load-bearing: `a` and `b` share row 0, so a row-only key would hand `a` the
        // end line of `b`'s statement. The column is the only discriminator.
        let src = "a = 1; b = (\n    2,\n)\n";
        let out = extended(
            src,
            vec![
                sym("a", SymbolKind::Variable, 0, 0),
                sym("b", SymbolKind::Variable, 0, 7),
            ],
        );
        assert_eq!(out[0].end_line, 0, "`a` is a one-line statement");
        assert_eq!(out[1].end_line, 2, "`b`'s statement closes on line 2");
    }

    #[test]
    fn class_attribute_children_are_extended() {
        // pyright nests class-level assignments as children of the class symbol.
        let src = "class K:\n    ITEMS = [\n        1,\n    ]\n";
        let mut class = sym("K", SymbolKind::Class, 0, 6);
        class.end_line = 3;
        class.children = vec![sym("ITEMS", SymbolKind::Constant, 1, 4)];
        let out = extended(src, vec![class]);
        assert_eq!(out[0].children[0].end_line, 3);
    }

    #[test]
    fn every_name_in_an_unpacking_target_is_extended() {
        let src = "a, b = (\n    1,\n    2,\n)\n";
        let out = extended(
            src,
            vec![
                sym("a", SymbolKind::Variable, 0, 0),
                sym("b", SymbolKind::Variable, 0, 3),
            ],
        );
        assert_eq!((out[0].end_line, out[1].end_line), (3, 3));
    }

    #[test]
    fn chained_assignment_extends_the_inner_target() {
        // `b` lives in the outer assignment's `right` field, so only the recursion
        // reaches it.
        let src = "a = b = (\n    1,\n)\n";
        let out = extended(
            src,
            vec![
                sym("a", SymbolKind::Variable, 0, 0),
                sym("b", SymbolKind::Variable, 0, 4),
            ],
        );
        assert_eq!((out[0].end_line, out[1].end_line), (2, 2));
    }

    #[test]
    fn non_variable_kinds_are_left_alone() {
        // A Function symbol sitting on an assignment row (a lambda binding) is not ours
        // to widen — the pass is scoped to the kinds pyright under-reports.
        let src = "f = (\n    lambda: 1\n)\n";
        let out = extended(src, vec![sym("f", SymbolKind::Function, 0, 0)]);
        assert_eq!(out[0].end_line, 0);
    }

    #[test]
    fn a_range_already_wider_is_never_shrunk() {
        // A future pyright (or another server) reporting the full range must not be
        // narrowed back to the statement.
        let src = "A = (\n    1,\n)\n";
        let mut s = sym("A", SymbolKind::Constant, 0, 0);
        s.end_line = 5;
        let out = extended(src, vec![s]);
        assert_eq!(out[0].end_line, 5);
    }

    #[test]
    fn unparseable_source_leaves_ranges_untouched() {
        // Load-bearing: tree-sitter recovers this into ONE assignment for `A` that ends
        // on row 2 and swallows `B = 3` into an ERROR node (probed 2026-09-24), so without
        // the `has_error` guard `A` would be widened over the next statement. Most broken
        // inputs do NOT exercise the guard: `A = (\n    1,\n\ndef f(): ...` recovers into a
        // bare ERROR with no assignment node at all, and passed with the guard deleted.
        let src = "A = [1,\n    2\nB = 3\n";
        let out = extended(src, vec![sym("A", SymbolKind::Constant, 0, 0)]);
        assert_eq!(out[0].end_line, 0);
    }
}
