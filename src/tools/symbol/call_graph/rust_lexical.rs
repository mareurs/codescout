//! Conservative lexical shadowing checks for the bounded Rust AST target fallback.
//! Semantic definition results take precedence; unsupported bindings need a warning.
use tree_sitter::Node;

fn mentions(node: Node<'_>, source: &[u8], name: &str) -> bool {
    if matches!(node.kind(), "identifier" | "type_identifier") {
        return node.utf8_text(source).ok() == Some(name);
    }
    let mut cursor = node.walk();
    let found = node
        .named_children(&mut cursor)
        .any(|n| mentions(n, source, name));
    found
}

fn imported(node: Node<'_>, source: &[u8], name: &str) -> bool {
    if node.kind() == "use_wildcard" {
        return true;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|n| imported(n, source, name))
        || mentions(node, source, name)
}

fn pattern_field(node: Node<'_>, field: &str, source: &[u8], name: &str) -> bool {
    node.child_by_field_name(field)
        .is_some_and(|n| mentions(n, source, name))
}

fn parameters(node: Node<'_>, source: &[u8], name: &str) -> bool {
    let Some(parameters) = node.child_by_field_name("parameters") else {
        return false;
    };
    let mut cursor = parameters.walk();
    let found = parameters.named_children(&mut cursor).any(|p| {
        if p.kind() == "parameter" {
            pattern_field(p, "pattern", source, name)
        } else {
            mentions(p, source, name)
        }
    });
    found
}

fn conditions_bind(node: Node<'_>, source: &[u8], name: &str) -> bool {
    if matches!(node.kind(), "let_condition" | "let_declaration") {
        return pattern_field(node, "pattern", source, name);
    }
    let mut cursor = node.walk();
    let found = node
        .named_children(&mut cursor)
        .any(|n| conditions_bind(n, source, name));
    found
}

/// Whether an enclosing lexical binding may select a target that module lookup cannot prove.
/// This deliberately does not resolve imports, values, local items or generic types.
pub(super) fn uncertain(call: Node<'_>, source: &[u8], expression: &str) -> bool {
    let Some(name) = expression.split("::").next() else {
        return true;
    };
    if matches!(name, "self" | "crate" | "super") {
        return false;
    }
    let origin = call.start_byte();
    let mut ancestor = call.parent();
    while let Some(scope) = ancestor {
        if parameters(scope, source, name) || pattern_field(scope, "type_parameters", source, name)
        {
            return true;
        }
        if matches!(scope.kind(), "for_expression" | "match_arm")
            && pattern_field(scope, "pattern", source, name)
        {
            return true;
        }
        if matches!(scope.kind(), "if_expression" | "while_expression")
            && scope
                .child_by_field_name("condition")
                .is_some_and(|n| conditions_bind(n, source, name))
        {
            return true;
        }
        if matches!(scope.kind(), "source_file" | "declaration_list" | "block") {
            let mut cursor = scope.walk();
            for binding in scope.named_children(&mut cursor) {
                if binding.kind() == "use_declaration" && imported(binding, source, name) {
                    return true;
                }
                if binding.kind() == "let_declaration"
                    && binding.end_byte() <= origin
                    && pattern_field(binding, "pattern", source, name)
                {
                    return true;
                }
                let local_item = scope.kind() == "block"
                    && matches!(
                        binding.kind(),
                        "function_item" | "struct_item" | "enum_item" | "mod_item"
                    );
                if (local_item
                    || matches!(binding.kind(), "const_item" | "static_item" | "type_item"))
                    && pattern_field(binding, "name", source, name)
                {
                    return true;
                }
            }
        }
        ancestor = scope.parent();
    }
    false
}
