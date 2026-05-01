use tree_sitter::Tree;

/// Returns true if the byte position lies inside a call-expression node.
pub fn position_is_call(tree: &Tree, byte_offset: usize, language_id: &str) -> bool {
    let node = tree
        .root_node()
        .descendant_for_byte_range(byte_offset, byte_offset);
    let Some(mut n) = node else { return false };
    let call_kinds: &[&str] = match language_id {
        "rust" => &[
            "call_expression",
            "method_call_expression",
            "macro_invocation",
        ],
        "python" => &["call"],
        "typescript" | "javascript" | "tsx" | "jsx" => &["call_expression", "new_expression"],
        "kotlin" => &["call_expression"],
        "java" => &["method_invocation", "object_creation_expression"],
        _ => return false,
    };
    loop {
        if call_kinds.contains(&n.kind()) {
            return true;
        }
        match n.parent() {
            Some(p) => n = p,
            None => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str, lang: tree_sitter::Language) -> Tree {
        let mut p = tree_sitter::Parser::new();
        p.set_language(&lang).unwrap();
        p.parse(src, None).unwrap()
    }

    #[test]
    fn rust_call_expression_classifies() {
        let src = "fn main() { foo(1); }";
        let tree = parse(src, tree_sitter_rust::LANGUAGE.into());
        let byte = src.find("foo").unwrap();
        assert!(position_is_call(&tree, byte, "rust"));
    }

    #[test]
    fn rust_type_ref_does_not_classify_as_call() {
        let src = "fn main() { let x: Foo = bar(); }";
        let tree = parse(src, tree_sitter_rust::LANGUAGE.into());
        let byte = src.find("Foo").unwrap();
        assert!(!position_is_call(&tree, byte, "rust"));
    }

    #[test]
    fn python_call_classifies() {
        let src = "x = foo(1)\n";
        let tree = parse(src, tree_sitter_python::LANGUAGE.into());
        let byte = src.find("foo").unwrap();
        assert!(position_is_call(&tree, byte, "python"));
    }

    #[test]
    fn python_identifier_not_in_call_does_not_classify() {
        let src = "x = foo\n";
        let tree = parse(src, tree_sitter_python::LANGUAGE.into());
        let byte = src.find("foo").unwrap();
        assert!(!position_is_call(&tree, byte, "python"));
    }

    #[test]
    fn typescript_call_classifies() {
        let src = "const x = foo(1);";
        let tree = parse(src, tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into());
        let byte = src.find("foo").unwrap();
        assert!(position_is_call(&tree, byte, "typescript"));
    }

    #[test]
    fn typescript_type_annotation_does_not_classify() {
        let src = "const x: Foo = bar();";
        let tree = parse(src, tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into());
        let byte = src.find("Foo").unwrap();
        assert!(!position_is_call(&tree, byte, "typescript"));
    }
}
