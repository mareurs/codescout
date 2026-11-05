//! Whether an edit changed a declaration's header, read from the tree-sitter parse.
//!
//! The header of a declaration is the comment-free run of tokens from the start of
//! its owner node to the start of its body. Two texts have the same header when those
//! token runs are equal, so whitespace, line endings and comments never count. A text
//! this reader cannot place exactly (a parse error, no single owner, an owner without
//! a body, a language without owner kinds) gives `None`: "cannot tell" is never
//! reported as "unchanged" or "changed".

use tree_sitter::{Language, Node, Parser, Tree};

/// The grammar families this reader knows owner kinds for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Family {
    Rust,
    Python,
    /// TypeScript, TSX, JavaScript and JSX (the last two reuse the first two grammars).
    Script,
    Java,
    Kotlin,
}

impl Family {
    fn of(language_id: &str) -> Option<Self> {
        match language_id.to_ascii_lowercase().as_str() {
            "rust" => Some(Self::Rust),
            "python" => Some(Self::Python),
            "typescript" | "tsx" | "javascript" | "jsx" => Some(Self::Script),
            "java" => Some(Self::Java),
            "kotlin" => Some(Self::Kotlin),
            _ => None,
        }
    }

    /// Whether a text whose as-is parse has an error is retried as the sole member of
    /// a wrapper class (a method or constructor alone).
    fn retries_in_wrapper(self) -> bool {
        matches!(self, Self::Script | Self::Java)
    }
}

const RUST_OWNERS: &[&str] = &[
    "function_item",
    "impl_item",
    "trait_item",
    "struct_item",
    "enum_item",
    "union_item",
    "mod_item",
    "foreign_mod_item",
];

const PYTHON_OWNERS: &[&str] = &["function_definition", "class_definition"];

const SCRIPT_OWNERS: &[&str] = &[
    "function_declaration",
    "generator_function_declaration",
    "class_declaration",
    "abstract_class_declaration",
    "interface_declaration",
    "enum_declaration",
    "method_definition",
];

/// Values of a `variable_declarator` that make the declaration a function owner.
const SCRIPT_FUNCTION_VALUES: &[&str] = &[
    "arrow_function",
    "function_expression",
    "generator_function",
];

const JAVA_OWNERS: &[&str] = &[
    "method_declaration",
    "constructor_declaration",
    "class_declaration",
    "interface_declaration",
    "enum_declaration",
    "record_declaration",
    "annotation_type_declaration",
];

/// Kotlin (kotlin-ng) has no `body` field; the body is a child of one of these kinds.
const KOTLIN_FUNCTION_BODIES: &[&str] = &["function_body"];
const KOTLIN_CLASS_BODIES: &[&str] = &["class_body", "enum_class_body"];

/// `Some(true)` when the declaration headers of `old` and `new` differ,
/// `Some(false)` when they are equal, `None` when either header cannot be read.
pub fn declaration_changed(old: &str, new: &str, language_id: &str) -> Option<bool> {
    let old = declaration_tokens(old, language_id)?;
    let new = declaration_tokens(new, language_id)?;
    Some(old != new)
}

/// The comment-free tokens from the start of the single top-level declaration in
/// `text` to the start of its body, or `None` when there is no such declaration.
///
/// The text is parsed as-is. Only when that parse holds an ERROR or MISSING node, and
/// only for Java and the TypeScript family, is it re-parsed as the sole member of a
/// wrapper class (a method or constructor alone). A clean parse that is not exactly one
/// owner is `None`: re-reading it in a class body would turn statements such as
/// `if (a) { ... }` into methods named `if`.
fn declaration_tokens(text: &str, language_id: &str) -> Option<Vec<String>> {
    let family = Family::of(language_id)?;
    let language = crate::ast::get_ts_language(language_id)?;

    let tree = parse(text, &language)?;
    if !tree.root_node().has_error() {
        return sole_member(tree.root_node(), &[])
            .and_then(|owner| header_tokens(owner, family, text));
    }
    if !family.retries_in_wrapper() {
        return None;
    }

    let wrapped = format!("class W {{\n{text}\n}}");
    let tree = parse(&wrapped, &language)?;
    if tree.root_node().has_error() {
        return None;
    }
    let class = sole_member(tree.root_node(), &[])?;
    if class.kind() != "class_declaration" {
        return None;
    }
    // A member's decorator is a sibling of the member in a TS class body, outside the
    // owner node, so it is skipped like a comment.
    let member = sole_member(class.child_by_field_name("body")?, &["decorator"])?;
    header_tokens(member, family, &wrapped)
}

/// Parse `source` with `language`; `None` only when the parser itself fails.
fn parse(source: &str, language: &Language) -> Option<Tree> {
    let mut parser = Parser::new();
    parser.set_language(language).ok()?;
    parser.parse(source, None)
}

fn is_comment(kind: &str) -> bool {
    kind.contains("comment")
}

/// The one named child of `parent` left after skipping comments, Rust attributes and
/// the `extra_skips` kinds; `None` when zero or several are left.
fn sole_member<'t>(parent: Node<'t>, extra_skips: &[&str]) -> Option<Node<'t>> {
    let mut cursor = parent.walk();
    let mut members = parent.named_children(&mut cursor).filter(|n| {
        let kind = n.kind();
        !is_comment(kind)
            && kind != "attribute_item"
            && kind != "inner_attribute_item"
            && !extra_skips.contains(&kind)
    });
    let member = members.next()?;
    members.next().is_none().then_some(member)
}

/// The header tokens of `node` when it is an owner with a body.
fn header_tokens(node: Node<'_>, family: Family, source: &str) -> Option<Vec<String>> {
    let (start, body) = owner(node, family)?;
    collect_tokens(node, start, body.start_byte(), source)
}

/// The header start byte and the body node of `node`, when `node` is an owner.
fn owner(node: Node<'_>, family: Family) -> Option<(usize, Node<'_>)> {
    match family {
        Family::Rust => field_body(node, RUST_OWNERS),
        Family::Python => {
            // A decorator is a child of `decorated_definition`, outside the definition
            // node, so the header starts at the definition.
            let definition = if node.kind() == "decorated_definition" {
                node.child_by_field_name("definition")?
            } else {
                node
            };
            field_body(definition, PYTHON_OWNERS)
        }
        Family::Script => {
            if node.kind() == "export_statement" {
                // `export` (and any decorator written before it) belongs to the
                // top-level node, so the header starts there.
                let (_, body) = script_owner(node.child_by_field_name("declaration")?)?;
                return Some((node.start_byte(), body));
            }
            script_owner(node)
        }
        Family::Java => field_body(node, JAVA_OWNERS),
        Family::Kotlin => match node.kind() {
            "function_declaration" => child_body(node, KOTLIN_FUNCTION_BODIES),
            "class_declaration" | "object_declaration" => child_body(node, KOTLIN_CLASS_BODIES),
            _ => None,
        },
    }
}

/// `node` is an owner when its kind is in `kinds` and it has a `body` field.
fn field_body<'t>(node: Node<'t>, kinds: &[&str]) -> Option<(usize, Node<'t>)> {
    if !kinds.contains(&node.kind()) {
        return None;
    }
    Some((node.start_byte(), node.child_by_field_name("body")?))
}

/// `node` is an owner when it has a child whose kind is in `body_kinds`.
fn child_body<'t>(node: Node<'t>, body_kinds: &[&str]) -> Option<(usize, Node<'t>)> {
    let mut cursor = node.walk();
    let body = node
        .children(&mut cursor)
        .find(|c| body_kinds.contains(&c.kind()))?;
    Some((node.start_byte(), body))
}

fn script_owner(node: Node<'_>) -> Option<(usize, Node<'_>)> {
    match node.kind() {
        "lexical_declaration" | "variable_declaration" => {
            let mut cursor = node.walk();
            let mut declarators = node
                .named_children(&mut cursor)
                .filter(|n| n.kind() == "variable_declarator");
            let declarator = declarators.next()?;
            if declarators.next().is_some() {
                return None;
            }
            let value = declarator.child_by_field_name("value")?;
            if !SCRIPT_FUNCTION_VALUES.contains(&value.kind()) {
                return None;
            }
            Some((node.start_byte(), value.child_by_field_name("body")?))
        }
        _ => field_body(node, SCRIPT_OWNERS),
    }
}

/// The tokens under `root` whose byte range lies in `[start, end)`, in document order.
///
/// A token is the source text of a leaf, or the whole source span of a string node
/// (its kind contains "string"; `concatenated_string` excepted, it is a sequence of
/// string nodes), so escapes and comment-looking content inside a string stay in the
/// token. A comment node is skipped with everything under it, delimiters included.
/// The walk is iterative so its depth never follows the input's nesting.
fn collect_tokens(root: Node<'_>, start: usize, end: usize, source: &str) -> Option<Vec<String>> {
    let mut tokens = Vec::new();
    let mut cursor = root.walk();
    let mut depth = 0usize;
    loop {
        let node = cursor.node();
        let kind = node.kind();
        let string = kind.contains("string") && kind != "concatenated_string";
        let atom = node.child_count() == 0 || string;
        let in_range = node.start_byte() >= start && node.end_byte() <= end;
        if is_comment(kind) {
            // Skipped whole: neither a token nor descended into.
        } else if atom {
            if in_range && node.end_byte() > node.start_byte() {
                tokens.push(source.get(node.byte_range())?.to_owned());
            }
        } else if node.start_byte() < end && cursor.goto_first_child() {
            depth += 1;
            continue;
        }
        loop {
            if depth == 0 {
                return Some(tokens);
            }
            if cursor.goto_next_sibling() {
                break;
            }
            cursor.goto_parent();
            depth -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCRIPT_FAMILY: [&str; 4] = ["typescript", "tsx", "javascript", "jsx"];

    fn ch(old: &str, new: &str, lang: &str) -> Option<bool> {
        declaration_changed(old, new, lang)
    }

    // ---------------------------------------------------------------- Rust

    #[test]
    fn rust_body_edit_is_unchanged() {
        assert_eq!(
            ch(
                "fn a() -> u8 {\n    1\n}",
                "fn a() -> u8 {\n    3\n}",
                "rust"
            ),
            Some(false)
        );
    }

    #[test]
    fn rust_added_parameter_is_changed() {
        assert_eq!(
            ch(
                "fn a() -> u8 {\n    1\n}",
                "fn a(x: u8) -> u8 {\n    1\n}",
                "rust"
            ),
            Some(true)
        );
    }

    #[test]
    fn rust_added_visibility_is_changed() {
        assert_eq!(ch("fn a() {}", "pub fn a() {}", "rust"), Some(true));
    }

    #[test]
    fn rust_header_whitespace_is_unchanged() {
        assert_eq!(
            ch(
                "fn a(x: u8) -> u8 {\n    x\n}",
                "fn  a(x: u8)\n    -> u8 {\n    x\n}",
                "rust"
            ),
            Some(false)
        );
    }

    #[test]
    fn rust_doc_comment_and_attribute_are_outside_the_header() {
        // Decision: `#[inline]` is an `attribute_item` and `/// doc` a `line_comment`,
        // both siblings of `function_item`, so they lie outside the owner node and are
        // excluded from the header.
        assert_eq!(
            ch("/// doc\n#[inline]\nfn a() {}", "fn a() {}", "rust"),
            Some(false)
        );
    }

    #[test]
    fn rust_crlf_and_lf_are_unchanged() {
        assert_eq!(
            ch(
                "fn a() -> u8 {\r\n    1\r\n}",
                "fn a() -> u8 {\n    1\n}",
                "rust"
            ),
            Some(false)
        );
    }

    #[test]
    fn rust_comment_after_the_brace_is_unchanged() {
        assert_eq!(
            ch("fn a() { // one\n 1\n}", "fn a() { // two\n 1\n}", "rust"),
            Some(false)
        );
    }

    #[test]
    fn rust_signature_without_body_is_unknown() {
        assert_eq!(ch("fn a(x: u8);", "fn a(x: u16);", "rust"), None);
    }

    #[test]
    fn rust_static_with_closure_value_is_unknown() {
        assert_eq!(
            ch(
                "static R: Lazy<u8> = Lazy::new(|| {\n    1\n});",
                "static R: Lazy<u8> = Lazy::new(|| {\n    2\n});",
                "rust"
            ),
            None
        );
    }

    #[test]
    fn rust_two_functions_are_unknown() {
        assert_eq!(
            ch("fn a() {}\nfn b() {}", "fn a() {}\nfn b() { 1; }", "rust"),
            None
        );
    }

    #[test]
    fn rust_impl_trait_return_body_edit_is_unchanged() {
        assert_eq!(
            ch(
                "fn a() -> impl Iterator<Item = u8> {\n    [1].into_iter()\n}",
                "fn a() -> impl Iterator<Item = u8> {\n    [2].into_iter()\n}",
                "rust"
            ),
            Some(false)
        );
    }

    #[test]
    fn rust_impl_block_method_body_edit_is_unchanged() {
        // The single declaration is the `impl_item`; the edit is inside its body.
        assert_eq!(
            ch(
                "impl S {\n    fn a(&self) { 1 }\n}",
                "impl S {\n    fn a(&self) { 2 }\n}",
                "rust"
            ),
            Some(false)
        );
    }

    // -------------------------------------------------------------- Python

    #[test]
    fn python_body_edit_is_unchanged() {
        assert_eq!(
            ch(
                "def f(a):\n    return 1",
                "def f(a):\n    return 2",
                "python"
            ),
            Some(false)
        );
    }

    #[test]
    fn python_added_parameter_is_changed() {
        assert_eq!(
            ch(
                "def f(a):\n    return 1",
                "def f(a, b):\n    return 1",
                "python"
            ),
            Some(true)
        );
    }

    #[test]
    fn python_trailing_comment_on_the_header_line_is_unchanged() {
        assert_eq!(
            ch(
                "def f(a):  # one\n    return 1",
                "def f(a):  # two\n    return 2",
                "python"
            ),
            Some(false)
        );
    }

    #[test]
    fn python_url_and_hash_in_string_defaults() {
        let old = "def f(url=\"http://x\", a=2, h=\"#x\"):\n    return 1";
        assert_eq!(
            ch(
                old,
                "def f(url=\"http://x\", a=2, h=\"#x\"):\n    return 2",
                "python"
            ),
            Some(false)
        );
        assert_eq!(
            ch(
                old,
                "def f(url=\"http://x\", a=2, h=\"#y\"):\n    return 1",
                "python"
            ),
            Some(true)
        );
    }

    #[test]
    fn python_one_liner_body_edit_is_unchanged() {
        assert_eq!(
            ch("def f(a): x = 1", "def f(a): y = 1", "python"),
            Some(false)
        );
        assert_eq!(
            ch(
                "def reset(self): self.cache = {}",
                "def reset(self): self.items = {}",
                "python"
            ),
            Some(false)
        );
    }

    #[test]
    fn python_multi_line_header() {
        let old = "def f(\n    a,\n    b,\n):\n    return 1";
        assert_eq!(
            ch(old, "def f(\n    a,\n    b,\n):\n    return 2", "python"),
            Some(false)
        );
        assert_eq!(
            ch(
                old,
                "def f(\n    a,\n    b,\n    c,\n):\n    return 1",
                "python"
            ),
            Some(true)
        );
    }

    #[test]
    fn python_class_body_edit_is_unchanged() {
        assert_eq!(
            ch(
                "class A:\n    def __init__(self):\n        self.cache = {}",
                "class A:\n    def __init__(self):\n        self.items = {}",
                "python"
            ),
            Some(false)
        );
    }

    #[test]
    fn python_class_name_and_bases_are_changed() {
        assert_eq!(
            ch("class A:\n    x = 1", "class B:\n    x = 1", "python"),
            Some(true)
        );
        assert_eq!(
            ch("class A(B):\n    x = 1", "class A(C):\n    x = 1", "python"),
            Some(true)
        );
    }

    #[test]
    fn python_decorated_body_edit_is_unchanged() {
        assert_eq!(
            ch(
                "@pytest.mark.parametrize(\n    \"a\", [1, 2],\n)\ndef test_x(a):\n    x()",
                "@pytest.mark.parametrize(\n    \"a\", [1, 2],\n)\ndef test_x(a):\n    y()",
                "python"
            ),
            Some(false)
        );
    }

    #[test]
    fn python_decorator_is_outside_the_header() {
        // Decision: a Python decorator is a child of `decorated_definition`, outside the
        // `function_definition` owner node, so adding or changing it leaves the header
        // unchanged.
        assert_eq!(
            ch("def f():\n    pass", "@cache\ndef f():\n    pass", "python"),
            Some(false)
        );
        assert_eq!(
            ch(
                "@cache\ndef f():\n    pass",
                "@lru_cache(2)\ndef f():\n    pass",
                "python"
            ),
            Some(false)
        );
    }

    #[test]
    fn python_indented_method_parses_as_is() {
        assert_eq!(
            ch(
                "    def f(self):\n        return 1",
                "    def f(self):\n        return 2",
                "python"
            ),
            Some(false)
        );
        assert_eq!(
            ch(
                "    def f(self):\n        return 1",
                "    def f(self, a):\n        return 1",
                "python"
            ),
            Some(true)
        );
    }

    #[test]
    fn python_single_quote_bracket_does_not_leak() {
        assert_eq!(
            ch(
                "def f(a='('):\n    for x in g(')'):\n        pass",
                "def f(a='('):\n    for y in g(')'):\n        pass",
                "python"
            ),
            Some(false)
        );
    }

    // ------------------------------------------- TypeScript / TSX / JS / JSX

    #[test]
    fn script_function_declaration() {
        let old = "function f(a: number): number {\n  return a;\n}";
        for lang in SCRIPT_FAMILY {
            assert_eq!(
                ch(old, "function f(a: number): number {\n  return 1;\n}", lang),
                Some(false),
                "{lang}"
            );
            assert_eq!(
                ch(
                    old,
                    "function f(a: number, b: number): number {\n  return a;\n}",
                    lang
                ),
                Some(true),
                "{lang}"
            );
            assert_eq!(
                ch(
                    "async function f(a: number): Promise<void> {\n  await g();\n}",
                    "async function f(a: number): Promise<number> {\n  await g();\n}",
                    lang
                ),
                Some(true),
                "{lang}"
            );
        }
    }

    #[test]
    fn script_arrow_with_block_body() {
        let old = "const f = (x) => {\n  a();\n};";
        for lang in SCRIPT_FAMILY {
            assert_eq!(
                ch(old, "const f = (x) => {\n  b();\n};", lang),
                Some(false),
                "{lang}"
            );
            assert_eq!(
                ch(old, "const f = (x, y) => {\n  a();\n};", lang),
                Some(true),
                "{lang}"
            );
        }
    }

    #[test]
    fn script_arrow_with_expression_body() {
        let old = "const f = (x: number) => x + 1;";
        for lang in SCRIPT_FAMILY {
            assert_eq!(
                ch(old, "const f = (x: number) => x + 2;", lang),
                Some(false),
                "{lang}"
            );
            assert_eq!(
                ch(old, "const f = (x: number, y: number) => x + 1;", lang),
                Some(true),
                "{lang}"
            );
        }
    }

    #[test]
    fn script_declared_call_value_is_unknown() {
        for lang in SCRIPT_FAMILY {
            assert_eq!(
                ch(
                    "const save = debounce((x) => {\n  a();\n}, 100);",
                    "const save = debounce((x) => {\n  b();\n}, 100);",
                    lang
                ),
                None,
                "{lang}"
            );
            assert_eq!(
                ch(
                    "export const M = React.memo((p) => {\n  return p.a;\n});",
                    "export const M = React.memo((p) => {\n  return p.b;\n});",
                    lang
                ),
                None,
                "{lang}"
            );
        }
    }

    #[test]
    fn script_arrow_ternary_body_is_unchanged() {
        for lang in SCRIPT_FAMILY {
            assert_eq!(
                ch(
                    "const f = (x) => x ? {a: 1} : {b: 2};",
                    "const f = (x) => !x ? {a: 1} : {b: 2};",
                    lang
                ),
                Some(false),
                "{lang}"
            );
        }
    }

    #[test]
    fn script_nested_arrow_ternary_is_unchanged() {
        // The owner is the outer arrow; its body is the whole ternary, so the inner
        // arrow and the condition are body, not header.
        for lang in SCRIPT_FAMILY {
            assert_eq!(
                ch(
                    "const f = (x) => x ? (y) => {\n  a();\n} : null;",
                    "const f = (x) => !x ? (y) => {\n  a();\n} : null;",
                    lang
                ),
                Some(false),
                "{lang}"
            );
        }
    }

    #[test]
    fn script_class() {
        let old = "class A extends B {\n  m() { return {} }\n}";
        for lang in SCRIPT_FAMILY {
            assert_eq!(
                ch(old, "class A extends B {\n  m() { return [] }\n}", lang),
                Some(false),
                "{lang}"
            );
            assert_eq!(
                ch(old, "class A extends C {\n  m() { return {} }\n}", lang),
                Some(true),
                "{lang}"
            );
        }
    }

    #[test]
    fn script_method_alone_uses_the_wrapper_retry() {
        // Decision: a method alone is not one owner at top level (tree-sitter-typescript
        // reads it as an ERROR-wrapped call `foo(x)` followed by a block), so it is
        // re-parsed inside `class W { ... }` and read as the single `method_definition`
        // of the wrapper class body. The retry fires only because the as-is parse has
        // an error; a clean parse that is not one owner is `None`.
        let old = "foo(x) {\n  return 1;\n}";
        for lang in SCRIPT_FAMILY {
            assert_eq!(
                ch(old, "foo(x) {\n  return 2;\n}", lang),
                Some(false),
                "{lang}"
            );
            assert_eq!(
                ch(old, "foo(x, y) {\n  return 1;\n}", lang),
                Some(true),
                "{lang}"
            );
        }
    }

    #[test]
    fn script_method_decorator_is_outside_the_header() {
        // Decision: inside a class body tree-sitter-typescript makes a method's
        // decorator a sibling of `method_definition`, outside the owner node, so a
        // decorator change on a method alone leaves the header unchanged.
        for lang in SCRIPT_FAMILY {
            assert_eq!(
                ch(
                    "@Input()\nfoo(x) {\n  return 1;\n}",
                    "@Output()\nfoo(x) {\n  return 1;\n}",
                    lang
                ),
                Some(false),
                "{lang}"
            );
        }
    }

    #[test]
    fn script_export_is_part_of_the_header() {
        for lang in SCRIPT_FAMILY {
            assert_eq!(
                ch("class A {}", "export class A {}", lang),
                Some(true),
                "{lang}"
            );
            assert_eq!(
                ch(
                    "const f = (x) => {\n  a();\n};",
                    "export const f = (x) => {\n  a();\n};",
                    lang
                ),
                Some(true),
                "{lang}"
            );
        }
    }

    #[test]
    fn script_decorated_exported_class() {
        // Decision: tree-sitter-typescript attaches a decorator written before `export`
        // to the `export_statement` (field `decorator`), not to `class_declaration`. The
        // header starts at the `export_statement` (so `export` itself counts, like Rust
        // `pub`), which puts the decorator inside the owner node and so in the header:
        // a body edit of `m` is unchanged, a decorator argument change is changed.
        let old = "@Component({\n  selector: \"x\",\n}) export class A {\n  m() {}\n}";
        for lang in SCRIPT_FAMILY {
            assert_eq!(
                ch(
                    old,
                    "@Component({\n  selector: \"x\",\n}) export class A {\n  m() { a(); }\n}",
                    lang
                ),
                Some(false),
                "{lang}"
            );
            assert_eq!(
                ch(
                    old,
                    "@Component({\n  selector: \"y\",\n}) export class A {\n  m() {}\n}",
                    lang
                ),
                Some(true),
                "{lang}"
            );
        }
    }

    #[test]
    fn script_non_function_value_is_unknown() {
        // The shape the text scanner read wrongly: the declared value is a ternary, not
        // a function, so there is no body to stop at.
        for lang in SCRIPT_FAMILY {
            assert_eq!(
                ch(
                    "const x =c ? {a: 1} : {b: 2};",
                    "const x =c ? {a: 2} : {b: 2};",
                    lang
                ),
                None,
                "{lang}"
            );
        }
    }

    // ---------------------------------------------------------------- Java

    #[test]
    fn java_method_alone() {
        // The brief expected this shape to need the wrapper retry, but tree-sitter-java
        // 0.23.5 parses a method alone clean as a top-level `method_declaration`, so the
        // as-is parse reads it; the constructor test pins the wrapper path instead.
        let old = "public int f(int a) {\n  int[] x = {1};\n  return a;\n}";
        assert_eq!(
            ch(
                old,
                "public int f(int a) {\n  int[] x = {2};\n  return a;\n}",
                "java"
            ),
            Some(false)
        );
        assert_eq!(
            ch(
                old,
                "public int f(int a, int b) {\n  int[] x = {1};\n  return a;\n}",
                "java"
            ),
            Some(true)
        );
    }

    #[test]
    fn java_annotation_is_part_of_the_header() {
        // A Java annotation sits in the method's `modifiers`, inside the owner node.
        let old = "@Override public String toString() {\n  return \"a\";\n}";
        assert_eq!(
            ch(
                old,
                "@Override public String toString() {\n  return \"b\";\n}",
                "java"
            ),
            Some(false)
        );
        assert_eq!(
            ch(
                old,
                "public String toString() {\n  return \"a\";\n}",
                "java"
            ),
            Some(true)
        );
    }

    #[test]
    fn java_annotation_with_braces_body_edit_is_unchanged() {
        assert_eq!(
            ch(
                "@SuppressWarnings({\n  \"a\"}) public int f() {\n  log();\n  int[] a = {1};\n}",
                "@SuppressWarnings({\n  \"a\"}) public int f() {\n  warn();\n  int[] a = {1};\n}",
                "java"
            ),
            Some(false)
        );
    }

    #[test]
    fn java_class_interfaces_are_changed() {
        assert_eq!(
            ch(
                "public class A implements B {\n  void m() {}\n}",
                "public class A implements C {\n  void m() {}\n}",
                "java"
            ),
            Some(true)
        );
    }

    #[test]
    fn java_interface_method_without_body_is_unknown() {
        assert_eq!(ch("int f();", "int f(int a);", "java"), None);
    }

    // -------------------------------------------------------------- Kotlin

    #[test]
    fn kotlin_block_function() {
        let old = "fun f(x: Int): Int {\n    log()\n    return run {\n        1\n    }\n}";
        assert_eq!(
            ch(
                old,
                "fun f(x: Int): Int {\n    warn()\n    return run {\n        1\n    }\n}",
                "kotlin"
            ),
            Some(false)
        );
        assert_eq!(
            ch(
                old,
                "fun f(x: Int, y: Int): Int {\n    log()\n    return run {\n        1\n    }\n}",
                "kotlin"
            ),
            Some(true)
        );
    }

    #[test]
    fn kotlin_expression_function() {
        let old = "fun f(x: Int) = x + 1";
        assert_eq!(ch(old, "fun f(x: Int) = x + 2", "kotlin"), Some(false));
        assert_eq!(
            ch(old, "fun f(x: Int, y: Int) = x + 1", "kotlin"),
            Some(true)
        );
    }

    #[test]
    fn kotlin_expression_function_with_when_is_unchanged() {
        assert_eq!(
            ch(
                "fun f() =\n    when (x) {\n        1 -> 2\n        else -> 3\n    }",
                "fun f() =\n    when (x + 1) {\n        1 -> 2\n        else -> 3\n    }",
                "kotlin"
            ),
            Some(false)
        );
    }

    #[test]
    fn kotlin_annotated_function_body_edit_is_unchanged() {
        assert_eq!(
            ch(
                "@JvmStatic fun f(): Int {\n    return 1\n}",
                "@JvmStatic fun f(): Int {\n    return 2\n}",
                "kotlin"
            ),
            Some(false)
        );
    }

    #[test]
    fn kotlin_class_header_changes() {
        assert_eq!(
            ch(
                "class A<T : Any>(x: T) {\n    fun m() {}\n}",
                "class A<T : Any>(x: T, z: T) {\n    fun m() {}\n}",
                "kotlin"
            ),
            Some(true)
        );
        assert_eq!(
            ch(
                "class A : B {\n    fun m() {}\n}",
                "class A : C {\n    fun m() {}\n}",
                "kotlin"
            ),
            Some(true)
        );
        assert_eq!(
            ch(
                "class A : B {\n    fun m() {}\n}",
                "class A : B {\n    fun m() { x() }\n}",
                "kotlin"
            ),
            Some(false)
        );
    }

    #[test]
    fn kotlin_class_without_body_is_unknown() {
        assert_eq!(
            ch(
                "data class P(val x: Int)",
                "data class P(val x: Long)",
                "kotlin"
            ),
            None
        );
    }

    // ---------------------------------------------------------------- Gaps

    #[test]
    fn empty_and_comment_only_texts_are_unknown() {
        assert_eq!(ch("", "", "rust"), None);
        assert_eq!(ch("// only a comment", "// only a comment", "rust"), None);
        assert_eq!(ch("fn a() {}", "", "rust"), None);
    }

    #[test]
    fn unparsable_text_is_unknown() {
        assert_eq!(ch("fn a( {", "fn a() {}", "rust"), None);
    }

    #[test]
    fn language_without_a_grammar_is_unknown() {
        // Decision: a language for which there is no tree-sitter grammar (or no owner
        // kinds) gives None for any text, never a guess.
        for lang in ["ruby", "scala", "c", "unknown"] {
            assert_eq!(ch("fn a() {}", "pub fn a() {}", lang), None, "{lang}");
            assert_eq!(
                ch("def f(a):\n  1\nend", "def f(a, b):\n  1\nend", lang),
                None,
                "{lang}"
            );
        }
    }

    #[test]
    fn language_id_is_case_insensitive() {
        for lang in ["rust", "RUST", "Rust"] {
            assert_eq!(ch("fn a() {}", "pub fn a() {}", lang), Some(true), "{lang}");
            assert_eq!(
                ch("fn a() { 1; }", "fn a() { 2; }", lang),
                Some(false),
                "{lang}"
            );
        }
    }

    #[test]
    fn declaration_tokens_stop_at_the_body() {
        assert_eq!(
            declaration_tokens("pub fn a(x: u8) -> u8 { x }", "rust"),
            Some(
                ["pub", "fn", "a", "(", "x", ":", "u8", ")", "->", "u8"]
                    .map(String::from)
                    .to_vec()
            )
        );
    }
    #[test]
    fn rust_other_owner_kinds() {
        assert_eq!(
            ch(
                "trait T: A {\n    fn a(&self);\n}",
                "trait T: A {\n    fn b(&self);\n}",
                "rust"
            ),
            Some(false)
        );
        assert_eq!(
            ch(
                "trait T: A {\n    fn a(&self);\n}",
                "trait T: B {\n    fn a(&self);\n}",
                "rust"
            ),
            Some(true)
        );
        assert_eq!(
            ch(
                "pub struct S<T> {\n    a: T,\n}",
                "pub struct S<T> {\n    b: T,\n}",
                "rust"
            ),
            Some(false)
        );
        assert_eq!(
            ch("enum E {\n    A,\n}", "enum E {\n    A,\n    B,\n}", "rust"),
            Some(false)
        );
        assert_eq!(
            ch(
                "mod m {\n    fn a() {}\n}",
                "pub mod m {\n    fn a() {}\n}",
                "rust"
            ),
            Some(true)
        );
        assert_eq!(
            ch(
                "extern \"C\" fn a() {}",
                "extern \"system\" fn a() {}",
                "rust"
            ),
            Some(true)
        );
        // A unit struct and an out-of-line module have no body.
        assert_eq!(ch("struct S;", "struct S;", "rust"), None);
        assert_eq!(ch("mod m;", "mod m;", "rust"), None);
    }

    #[test]
    fn rust_comment_inside_the_header_is_unchanged() {
        assert_eq!(
            ch("fn a(/* x */ y: u8) {}", "fn a(/* z */ y: u8) {}", "rust"),
            Some(false)
        );
    }

    #[test]
    fn python_async_and_tab_indent() {
        assert_eq!(
            ch(
                "def f(a):\n    return 1",
                "async def f(a):\n    return 1",
                "python"
            ),
            Some(true)
        );
        assert_eq!(
            ch(
                "\tdef f(self):\n\n\t\treturn 1",
                "\tdef f(self):\n\n\t\treturn 2",
                "python"
            ),
            Some(false)
        );
    }

    #[test]
    fn python_two_definitions_are_unknown() {
        assert_eq!(
            ch(
                "def f():\n    pass\ndef g():\n    pass",
                "def f():\n    pass\ndef g():\n    return 1",
                "python"
            ),
            None
        );
    }

    #[test]
    fn script_other_owner_kinds() {
        for lang in SCRIPT_FAMILY {
            assert_eq!(
                ch(
                    "const f = function (x) {\n  a();\n};",
                    "const f = function (x) {\n  b();\n};",
                    lang
                ),
                Some(false),
                "{lang}"
            );
            assert_eq!(
                ch(
                    "let g = function* (x) {\n  yield 1;\n};",
                    "let g = function* (x, y) {\n  yield 1;\n};",
                    lang
                ),
                Some(true),
                "{lang}"
            );
            assert_eq!(
                ch(
                    "export default function f(a) {\n  return a;\n}",
                    "export default function f(a) {\n  return 1;\n}",
                    lang
                ),
                Some(false),
                "{lang}"
            );
            assert_eq!(
                ch(
                    "function* gen(a) {\n  yield a;\n}",
                    "function* gen(a) {\n  yield 1;\n}",
                    lang
                ),
                Some(false),
                "{lang}"
            );
        }
        // Type-only owners, TypeScript grammars only.
        for lang in ["typescript", "tsx"] {
            assert_eq!(
                ch(
                    "interface I extends J {\n  a: number;\n}",
                    "interface I extends J {\n  b: number;\n}",
                    lang
                ),
                Some(false),
                "{lang}"
            );
            assert_eq!(
                ch(
                    "interface I extends J {\n  a: number;\n}",
                    "interface I extends K {\n  a: number;\n}",
                    lang
                ),
                Some(true),
                "{lang}"
            );
            assert_eq!(
                ch("enum E {\n  A,\n}", "enum E {\n  A,\n  B,\n}", lang),
                Some(false),
                "{lang}"
            );
            assert_eq!(
                ch(
                    "abstract class A {\n  m() {}\n}",
                    "abstract class A<T> {\n  m() {}\n}",
                    lang
                ),
                Some(true),
                "{lang}"
            );
        }
    }

    #[test]
    fn script_several_declarators_are_unknown() {
        for lang in SCRIPT_FAMILY {
            assert_eq!(
                ch(
                    "const a = 1, f = () => {\n  x();\n};",
                    "const a = 1, f = () => {\n  y();\n};",
                    lang
                ),
                None,
                "{lang}"
            );
        }
    }

    #[test]
    fn script_decorator_inside_the_class_node_is_part_of_the_header() {
        // Decision: without `export`, the decorator is a field of `class_declaration`,
        // inside the owner node, so it is part of the header.
        for lang in SCRIPT_FAMILY {
            assert_eq!(
                ch(
                    "@Component({ selector: \"x\" })\nclass A {\n  m() {}\n}",
                    "@Component({ selector: \"y\" })\nclass A {\n  m() {}\n}",
                    lang
                ),
                Some(true),
                "{lang}"
            );
        }
    }

    #[test]
    fn java_constructor_alone_uses_the_wrapper_retry() {
        // Decision: a constructor alone does not parse clean at top level (the grammar
        // reads it as a method missing its name), so it is re-parsed inside
        // `class W { ... }` and read as the wrapper body's single `constructor_declaration`.
        let old = "public A(int a) {\n  this.a = a;\n}";
        assert_eq!(
            ch(old, "public A(int a) {\n  this.b = a;\n}", "java"),
            Some(false)
        );
        assert_eq!(
            ch(old, "public A(int a, int b) {\n  this.a = a;\n}", "java"),
            Some(true)
        );
        assert_eq!(ch(old, "A(int a) {\n  this.a = a;\n}", "java"), Some(true));
    }

    #[test]
    fn java_other_owner_kinds() {
        assert_eq!(
            ch(
                "record P(int x) {\n  P {}\n}",
                "record P(int x, int y) {\n  P {}\n}",
                "java"
            ),
            Some(true)
        );
        assert_eq!(
            ch("enum E {\n  A;\n}", "enum E {\n  A, B;\n}", "java"),
            Some(false)
        );
        assert_eq!(
            ch(
                "interface I extends J {\n  int f();\n}",
                "interface I extends J {\n  int g();\n}",
                "java"
            ),
            Some(false)
        );
        assert_eq!(
            ch(
                "class A {\n  void m() {}\n}\nclass B {}",
                "class A {\n  void m() { x(); }\n}\nclass B {}",
                "java"
            ),
            None
        );
    }

    #[test]
    fn kotlin_object_and_enum_class() {
        assert_eq!(
            ch(
                "object O : B {\n    fun m() {}\n}",
                "object O : B {\n    fun n() {}\n}",
                "kotlin"
            ),
            Some(false)
        );
        assert_eq!(
            ch(
                "object O : B {\n    fun m() {}\n}",
                "object O : C {\n    fun m() {}\n}",
                "kotlin"
            ),
            Some(true)
        );
        assert_eq!(
            ch(
                "enum class E { A, B }",
                "enum class E { A, B, C }",
                "kotlin"
            ),
            Some(false)
        );
        assert_eq!(
            ch("enum class E { A, B }", "enum class F { A, B }", "kotlin"),
            Some(true)
        );
    }

    #[test]
    fn grammar_without_owner_kinds_is_unknown() {
        // `go` and `bash` have grammars but no owner kinds here, so they give None.
        assert_eq!(
            ch(
                "func f(a int) int {\n\treturn a\n}",
                "func f(a int, b int) int {\n\treturn a\n}",
                "go"
            ),
            None
        );
        assert_eq!(ch("f() {\n  a\n}", "f() {\n  b\n}", "bash"), None);
    }

    #[test]
    fn deep_nesting_in_the_body_does_not_recurse() {
        let depth = 5_000;
        let body = format!("{}1{}", "(".repeat(depth), ")".repeat(depth));
        let old = format!("fn a() -> u8 {{ {body} }}");
        let new = format!("fn a() -> u8 {{ {body}; 2 }}");
        assert_eq!(declaration_changed(&old, &new, "rust"), Some(false));
    }
    #[test]
    fn deep_nesting_in_the_header_does_not_recurse() {
        let depth = 5_000;
        // Rust: a 5,000-deep tuple type in a parameter.
        let ty = |inner: &str| format!("{}{inner}{}", "(".repeat(depth), ",)".repeat(depth));
        let rust_old = format!("fn a(x: {}) {{ 1 }}", ty("u8"));
        assert_eq!(
            ch(&rust_old, &format!("fn a(x: {}) {{ 2 }}", ty("u8")), "rust"),
            Some(false)
        );
        assert_eq!(
            ch(
                &rust_old,
                &format!("fn a(x: {}) {{ 1 }}", ty("u16")),
                "rust"
            ),
            Some(true)
        );
        // TypeScript and Python: 5,000-deep parentheses in a default value.
        let parens = |inner: &str| format!("{}{inner}{}", "(".repeat(depth), ")".repeat(depth));
        let ts_old = format!("function f(a = {}) {{\n  x();\n}}", parens("1"));
        assert_eq!(
            ch(
                &ts_old,
                &format!("function f(a = {}) {{\n  y();\n}}", parens("1")),
                "typescript"
            ),
            Some(false)
        );
        assert_eq!(
            ch(
                &ts_old,
                &format!("function f(a = {}) {{\n  x();\n}}", parens("2")),
                "typescript"
            ),
            Some(true)
        );
        let py_old = format!("def f(a={}):\n    return 1", parens("1"));
        assert_eq!(
            ch(
                &py_old,
                &format!("def f(a={}):\n    return 2", parens("1")),
                "python"
            ),
            Some(false)
        );
        assert_eq!(
            ch(
                &py_old,
                &format!("def f(a={}):\n    return 1", parens("2")),
                "python"
            ),
            Some(true)
        );
    }

    #[test]
    fn python_multi_line_string_default_with_indented_body_edit_is_unchanged() {
        // Regression for the removed dedent: it stripped the indent shared by every
        // non-blank line, body lines included, so a body edit that shrank that indent
        // changed the text of a header string spanning lines. The text is now parsed
        // as-is (tree-sitter-python reads an indented module-level `def` clean), so a
        // header token is the exact source span and a body edit cannot reach it.
        assert_eq!(
            ch(
                "    def f(a='''x\n    y'''):\n        return 1",
                "    def f(a='''x\n    y'''):\n        s = '''\nz'''\n        return 1",
                "python"
            ),
            Some(false)
        );
        assert_eq!(
            ch(
                "  def f(a='''x\n    y'''):\n    return 1",
                "  def f(a='''x\n    y'''):\n    s = '''\n z'''\n    return 1",
                "python"
            ),
            Some(false)
        );
    }

    #[test]
    fn python_indented_decorated_method() {
        // Pins the decorator rule and the as-is indentation handling together: an
        // indented decorated method parses clean, the decorator stays outside the
        // `function_definition` owner, and only a real header change is reported.
        let old = "    @property\n    def f(self):\n        return 1";
        assert_eq!(
            ch(
                old,
                "    @property\n    def f(self):\n        return 2",
                "python"
            ),
            Some(false)
        );
        assert_eq!(
            ch(
                old,
                "    @cached_property\n    def f(self):\n        return 1",
                "python"
            ),
            Some(false)
        );
        assert_eq!(
            ch(
                old,
                "    @property\n    def g(self):\n        return 1",
                "python"
            ),
            Some(true)
        );
    }

    #[test]
    fn added_comment_in_the_header_is_unchanged() {
        // Pins the comment filter: a comment node is skipped whole, so its delimiters
        // (`/*`, `*/`, `//`, `///`, anonymous children in tree-sitter-rust) never become
        // tokens. Earlier tests only changed comment text, where delimiters cancel.
        let rust = "fn a(x: u8) {}";
        for new in [
            "fn /* c */ a(x: u8) {}",
            "fn a(x: u8) // c\n{}",
            "fn a(\n    /// d\n    x: u8) {}",
        ] {
            assert_eq!(ch(rust, new, "rust"), Some(false), "{new:?}");
        }
        for lang in SCRIPT_FAMILY {
            for new in ["function /* c */ f(x) {}", "function f(x) // c\n{}"] {
                assert_eq!(
                    ch("function f(x) {}", new, lang),
                    Some(false),
                    "{lang} {new:?}"
                );
            }
        }
        for new in [
            "class /* c */ A {\n}",
            "class A // c\n{\n}",
            "/** d */\nclass A {\n}",
        ] {
            assert_eq!(ch("class A {\n}", new, "java"), Some(false), "{new:?}");
        }
        for new in [
            "fun /* c */ f(x: Int) {}",
            "fun f(\n    // c\n    x: Int) {}",
        ] {
            assert_eq!(
                ch("fun f(x: Int) {}", new, "kotlin"),
                Some(false),
                "{new:?}"
            );
        }
        assert_eq!(
            ch(
                "def f(a):\n    return 1",
                "def f(  # c\n    a):\n    return 1",
                "python"
            ),
            Some(false)
        );
    }

    #[test]
    fn clean_script_statement_is_not_retried_as_a_method() {
        // Pins "retry in the wrapper only when the as-is parse has an error": these
        // parse clean as statements, and inside `class W { ... }` they would read as
        // methods named `if`, `while`, `with` or `foo`.
        for lang in SCRIPT_FAMILY {
            for (old, new) in [
                ("if (a) { b(); }", "if (z) { b(); }"),
                ("while (x) { y(); }", "while (z) { y(); }"),
                ("with (x) { y(); }", "with (z) { y(); }"),
                ("foo(x)\n{\n  a();\n}", "foo(x)\n{\n  b();\n}"),
            ] {
                assert_eq!(ch(old, new, lang), None, "{lang} {old:?}");
            }
        }
    }

    #[test]
    fn clean_java_statement_is_not_retried_as_a_member() {
        for (old, new) in [
            ("int x = 1;", "int x = 2;"),
            ("if (a) { b(); }", "if (z) { b(); }"),
            ("foo(x);", "foo(y);"),
            ("static {}", "static { a(); }"),
        ] {
            assert_eq!(ch(old, new, "java"), None, "{old:?}");
        }
    }
    #[test]
    fn unclean_text_whose_wrapper_parse_errs_is_unknown() {
        // Pins the wrapper's own `has_error` check: each text has an error as-is, so it is
        // retried in `class W { ... }`, and the wrapper parse errs too. Without that second
        // check the error-recovered wrapper tree still yields one member with a body, and
        // the parameter edits below would read as `Some(true)`.
        for lang in SCRIPT_FAMILY {
            for (old, new) in [
                ("if (a) { b(; }", "if (z) { b(; }"),
                ("return foo(x) { a(); }", "return foo(y) { a(); }"),
                ("if (a) { b(); ", "if (z) { b(); "),
                ("foo(x) {", "foo(y) {"),
                ("x =", "y ="),
            ] {
                assert_eq!(ch(old, new, lang), None, "{lang} {old:?}");
            }
        }
        for (old, new) in [
            ("if (a) { b(); ", "if (z) { b(); "),
            ("foo(int x) {", "foo(int y) {"),
            ("A(int a) {", "A(int b) {"),
        ] {
            assert_eq!(ch(old, new, "java"), None, "{old:?}");
        }
    }

    #[test]
    fn python_concatenated_string_parts_stay_separate() {
        // Pins the `concatenated_string` exception to the string-atom rule: its parts are
        // separate string tokens, so whitespace between them is not a header change. Taken
        // as one atom, the span would include that whitespace and read as `Some(true)`.
        let old = "def f(a=\"a\" \"b\"):\n    return 1";
        assert_eq!(
            ch(old, "def f(a=\"a\"   \"b\"):\n    return 1", "python"),
            Some(false)
        );
        // Newline-split variant: the parameter list's parentheses join the lines.
        assert_eq!(
            ch(old, "def f(a=\"a\"\n      \"b\"):\n    return 1", "python"),
            Some(false)
        );
        // A real change to one part is still a header change.
        assert_eq!(
            ch(old, "def f(a=\"a\" \"c\"):\n    return 1", "python"),
            Some(true)
        );
    }

    #[test]
    fn error_inside_an_owner_is_unknown() {
        // Pins the ERROR/MISSING rule: each text keeps a single `function_item` with a
        // body around the error, so without the check it would read as `Some(false)`.
        assert_eq!(
            ch("fn a(x: u8) { 1 }", "fn a(x: u8) { let = ; }", "rust"),
            None
        );
        assert_eq!(ch("fn a(x: u8) { 1 }", "fn a(x: u8 +) { 1 }", "rust"), None);
    }

    #[test]
    fn script_function_declarator_first_of_several_is_unknown() {
        // Pins the one-declarator rule: the function declarator comes first, so a
        // reader that took the first declarator would answer.
        for lang in SCRIPT_FAMILY {
            assert_eq!(
                ch(
                    "const f = () => {\n  x();\n}, a = 1;",
                    "const f = () => {\n  y();\n}, a = 1;",
                    lang
                ),
                None,
                "{lang}"
            );
        }
    }

    #[test]
    fn script_class_expression_value_is_unknown() {
        // Pins the function-value rule: a class expression also has a `body` field, so
        // a reader that accepted any value with a body would answer.
        for lang in SCRIPT_FAMILY {
            assert_eq!(
                ch(
                    "const A = class extends B {\n  m() {}\n};",
                    "const A = class extends B {\n  m() { x(); }\n};",
                    lang
                ),
                None,
                "{lang}"
            );
        }
    }

    #[test]
    fn string_in_the_header_is_one_token() {
        // Pins the string atom: a string's escapes and comment-looking content are part
        // of the token. Read leaf by leaf, `"x\n"` would lose the `x` (its content node
        // has a named escape child) and the change would be missed.
        assert_eq!(
            ch(
                "def f(a=\"x\\n\"):\n    return 1",
                "def f(a=\"y\\n\"):\n    return 1",
                "python"
            ),
            Some(true)
        );
        assert_eq!(
            ch(
                "def f(a=\"//\"):\n    return 1",
                "def f(a=\"#\"):\n    return 1",
                "python"
            ),
            Some(true)
        );
        assert_eq!(
            ch(
                "def f(a=\"x\\n\"):\n    return 1",
                "def f(a=\"x\\n\"):\n    return 2",
                "python"
            ),
            Some(false)
        );
        for lang in SCRIPT_FAMILY {
            assert_eq!(
                ch(
                    "function f(a = \"x\\n\") {}",
                    "function f(a = \"y\\n\") {}",
                    lang
                ),
                Some(true),
                "{lang}"
            );
            assert_eq!(
                ch(
                    "function f(a = \"//\") {}",
                    "function f(a = \"/*\") {}",
                    lang
                ),
                Some(true),
                "{lang}"
            );
            assert_eq!(
                ch(
                    "function f(a = `x\\n`) {}",
                    "function f(a = `y\\n`) {}",
                    lang
                ),
                Some(true),
                "{lang}"
            );
        }
        assert_eq!(
            ch(
                "extern \"C\\n\" fn a() {}",
                "extern \"D\\n\" fn a() {}",
                "rust"
            ),
            Some(true)
        );
        assert_eq!(
            ch(
                "@SuppressWarnings(\"a\\n\") void f() {}",
                "@SuppressWarnings(\"b\\n\") void f() {}",
                "java"
            ),
            Some(true)
        );
        assert_eq!(
            ch(
                "@Suppress(\"a\\n\") fun f() {}",
                "@Suppress(\"b\\n\") fun f() {}",
                "kotlin"
            ),
            Some(true)
        );
    }

    #[test]
    fn python_other_shapes() {
        assert_eq!(
            ch(
                "async def f(a):\n    return 1",
                "async def f(a):\n    return 2",
                "python"
            ),
            Some(false)
        );
        // An `if`-guarded def is an `if_statement`, not an owner.
        assert_eq!(
            ch(
                "if X:\n    def f():\n        pass",
                "if X:\n    def f():\n        return 1",
                "python"
            ),
            None
        );
        // An assigned lambda is an assignment, not an owner.
        assert_eq!(
            ch("f = lambda a: a + 1", "f = lambda a: a + 2", "python"),
            None
        );
        assert_eq!(
            ch(
                "def f(a):\r\n    return 1",
                "def f(a):\r\n    return 2",
                "python"
            ),
            Some(false)
        );
        assert_eq!(
            ch(
                "def naïve(a):\n    return 1",
                "def naïve(a):\n    return 2",
                "python"
            ),
            Some(false)
        );
        assert_eq!(
            ch(
                "def naïve(a):\n    return 1",
                "def naive(a):\n    return 1",
                "python"
            ),
            Some(true)
        );
    }

    #[test]
    fn script_other_shapes() {
        for lang in SCRIPT_FAMILY {
            // Overloads are several declarations.
            assert_eq!(
                ch(
                    "function f(a: string): void;\nfunction f(a: any) {\n  x();\n}",
                    "function f(a: string): void;\nfunction f(a: any) {\n  y();\n}",
                    lang
                ),
                None,
                "{lang}"
            );
            // A getter alone does not parse clean, so it is read in the wrapper.
            assert_eq!(
                ch(
                    "get x() {\n  return 1;\n}",
                    "get x() {\n  return 2;\n}",
                    lang
                ),
                Some(false),
                "{lang}"
            );
            assert_eq!(
                ch("class A {\n  x = 1;\n}", "class A {\n  x = 2;\n}", lang),
                Some(false),
                "{lang}"
            );
        }
        for lang in ["typescript", "tsx"] {
            // An ambient declaration has no body.
            assert_eq!(
                ch(
                    "declare function f(a: number): void;",
                    "declare function f(a: string): void;",
                    lang
                ),
                None,
                "{lang}"
            );
            // A namespace is not an owner kind.
            assert_eq!(
                ch(
                    "namespace N {\n  const a = 1;\n}",
                    "namespace N {\n  const a = 2;\n}",
                    lang
                ),
                None,
                "{lang}"
            );
        }
    }

    #[test]
    fn java_other_shapes() {
        assert_eq!(
            ch(
                "record P(int x) {\n  P {}\n}",
                "record P(int x) {\n  P { a(); }\n}",
                "java"
            ),
            Some(false)
        );
        assert_eq!(
            ch(
                "enum E {\n  A;\n  void m() {}\n}",
                "enum E {\n  A;\n  void m() { x(); }\n}",
                "java"
            ),
            Some(false)
        );
        assert_eq!(
            ch(
                "@Override\npublic String toString() {\n  return \"a\";\n}",
                "@Override\npublic String toString() {\n  return \"b\";\n}",
                "java"
            ),
            Some(false)
        );
        // A local lambda is a variable declaration, not an owner.
        assert_eq!(
            ch(
                "Runnable r = () -> {\n  a();\n};",
                "Runnable r = () -> {\n  b();\n};",
                "java"
            ),
            None
        );
    }

    #[test]
    fn kotlin_other_shapes() {
        assert_eq!(
            ch(
                "suspend fun f(x: Int) {\n    a()\n}",
                "suspend fun f(x: Int) {\n    b()\n}",
                "kotlin"
            ),
            Some(false)
        );
        assert_eq!(
            ch(
                "fun f(x: Int) {\n    a()\n}",
                "suspend fun f(x: Int) {\n    a()\n}",
                "kotlin"
            ),
            Some(true)
        );
        assert_eq!(
            ch(
                "fun String.f(x: Int): Int {\n    return 1\n}",
                "fun String.f(x: Int): Int {\n    return 2\n}",
                "kotlin"
            ),
            Some(false)
        );
        assert_eq!(
            ch(
                "fun String.f(x: Int): Int {\n    return 1\n}",
                "fun Int.f(x: Int): Int {\n    return 1\n}",
                "kotlin"
            ),
            Some(true)
        );
        for (old, new) in [
            (
                "sealed class S {\n    object A : S()\n}",
                "sealed class S {\n    object B : S()\n}",
            ),
            (
                "data class P(val x: Int) {\n    fun m() = 1\n}",
                "data class P(val x: Int) {\n    fun m() = 2\n}",
            ),
            (
                "abstract class A {\n    abstract fun m()\n}",
                "abstract class A {\n    abstract fun n()\n}",
            ),
        ] {
            assert_eq!(ch(old, new, "kotlin"), Some(false), "{old:?}");
        }
        for (old, new) in [
            (
                "companion object {\n    fun m() {}\n}",
                "companion object {\n    fun m() { a() }\n}",
            ),
            (
                "constructor(x: Int) : this() {\n    a()\n}",
                "constructor(x: Int) : this() {\n    b()\n}",
            ),
            (
                "val x: Int\n    get() {\n        return 1\n    }",
                "val x: Int\n    get() {\n        return 2\n    }",
            ),
            ("val x = run {\n    1\n}", "val x = run {\n    2\n}"),
        ] {
            assert_eq!(ch(old, new, "kotlin"), None, "{old:?}");
        }
    }

    #[test]
    fn text_encoding_shapes() {
        // A BOM before the declaration, on both sides, with a body edit.
        assert_eq!(
            ch("\u{feff}fn a() { 1 }", "\u{feff}fn a() { 2 }", "rust"),
            Some(false)
        );
        assert_eq!(
            ch("fn\ta(x:\tu8)\t{}", "fn a(x: u8) {}", "rust"),
            Some(false)
        );
        assert_eq!(
            ch("fn café(x: u8) { 1 }", "fn café(x: u8) { 2 }", "rust"),
            Some(false)
        );
        assert_eq!(
            ch("fn café(x: u8) { 1 }", "fn cafe(x: u8) { 1 }", "rust"),
            Some(true)
        );
        assert_eq!(ch("fn a() {}", "struct a {}", "rust"), Some(true));
    }

    // ------------------------------------------------ retired text scanner corpus
    //
    // The shapes below are the corpus of the text scanner that `declaration_changed`
    // replaced (the `declaration_changed` tests that lived in `symbol/edit.rs`). Each is asserted with
    // the value the reader returns. The invariant of every pair that edits only the
    // body: never `Some(true)`. Where the scanner answered `None` for a body-only
    // edit and the reader can now tell, the value is `Some(false)`. Where a pair
    // changes the header as well (a parameter, a name, a decorator argument) the
    // value is `Some(true)`, which is what the edit does.

    type Case<'a> = (&'a str, &'a str, &'a str, Option<bool>);

    /// Runs every case and reports all mismatches at once.
    fn check_all(cases: &[Case]) {
        let bad: Vec<String> = cases
            .iter()
            .filter_map(|(lang, old, new, want)| {
                let got = ch(old, new, lang);
                (got != *want)
                    .then(|| format!("{lang}: {old:?} -> {new:?}: want {want:?}, got {got:?}"))
            })
            .collect();
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    #[test]
    fn corpus_basic_rust_edits() {
        check_all(&[
            (
                "rust",
                "fn a() -> u8 {\n    1\n}",
                "fn a() -> u8 {\n    3\n}",
                Some(false),
            ),
            (
                "rust",
                "fn a() -> u8 {\n    1\n}",
                "fn a(x: u8) -> u8 {\n    1\n}",
                Some(true),
            ),
            ("rust", "fn a() {}", "pub fn a() {}", Some(true)),
            (
                "rust",
                "fn a(x: u8) -> u8 {}",
                "fn  a(x: u8)\n    -> u8 {}",
                Some(false),
            ),
            (
                "rust",
                "fn a() -> u8 {\r\n    1\r\n}",
                "fn a() -> u8 {\n    3\n}",
                Some(false),
            ),
            (
                "rust",
                "fn a() { // one\n    1\n}",
                "fn a() { // two\n    1\n}",
                Some(false),
            ),
        ]);
    }

    #[test]
    fn corpus_leading_attributes_comments_and_decorators_are_skipped() {
        check_all(&[
            (
                "rust",
                "/// doc\n#[inline]\nfn a() {}",
                "fn a() {}",
                Some(false),
            ),
            (
                "python",
                "@d\ndef f(a):\n    pass",
                "def f(a):\n    return 1",
                Some(false),
            ),
            (
                "python",
                "@d\ndef f(a):\n    pass",
                "def f(a, b):\n    pass",
                Some(true),
            ),
        ]);
    }

    #[test]
    fn corpus_no_header_is_unknown() {
        check_all(&[
            ("rust", "", "fn a() {}", None),
            ("rust", "fn a() {}", "", None),
            ("rust", "// only a comment", "fn a() {}", None),
            ("rust", "fn a() {}", "// only a comment", None),
        ]);
    }

    #[test]
    fn corpus_python_trailing_comment_on_the_header_line() {
        check_all(&[
            (
                "python",
                "def f(a):  # note\n    pass",
                "def f(a):  # note\n    return 1",
                Some(false),
            ),
            (
                "python",
                "def f(a):  # note\n    pass",
                "def f(a, b):  # note\n    pass",
                Some(true),
            ),
            (
                "python",
                "def f(a):  # one\n    pass",
                "def f(a):  # two\n    pass",
                Some(false),
            ),
        ]);
    }

    #[test]
    fn corpus_crlf_reads_like_lf() {
        check_all(&[(
            "rust",
            "fn a() -> u8 {\r\n    1\r\n}",
            "fn a() -> u8 {\n    3\n}",
            Some(false),
        )]);
    }

    #[test]
    fn corpus_url_in_a_string_default_does_not_cut_the_header() {
        let old = "def f(\n    url=\"http://x\",\n    a=1,\n):\n    pass";
        check_all(&[
            (
                "python",
                old,
                "def f(\n    url=\"http://x\",\n    a=2,\n):\n    pass",
                Some(true),
            ),
            (
                "python",
                old,
                "def f(\n    url=\"http://x\",\n    a=1,\n):\n    return 1",
                Some(false),
            ),
        ]);
    }

    #[test]
    fn corpus_hash_in_a_string_default_does_not_cut_the_header() {
        check_all(&[(
            "python",
            "def f(sep=\" #\"):\n    pass",
            "def f(sep=\" #\"):\n    return 1",
            Some(false),
        )]);
    }

    #[test]
    fn corpus_bodyless_rust_header_with_a_trailing_comment_next_to_a_function() {
        // The scanner read the first header and answered `Some(false)`. Two
        // declarations are not one declaration, so the reader cannot tell.
        check_all(&[(
            "rust",
            "fn a();  // one\nfn b() {}",
            "fn a();  // one\nfn b() { 1 }",
            None,
        )]);
    }

    #[test]
    fn corpus_one_line_declarations() {
        check_all(&[
            (
                "rust",
                "pub fn len(&self) -> usize { self.0.len() }",
                "pub fn len(&self) -> usize { self.items.len() }",
                Some(false),
            ),
            (
                "python",
                "def f(a): return 1",
                "def f(a): return 2",
                Some(false),
            ),
            (
                "rust",
                "pub fn len(&self) -> usize { self.0.len() }",
                "pub fn len(&self, n: u8) -> usize { self.0.len() }",
                Some(true),
            ),
            (
                "python",
                "def f(a): return 1",
                "def f(a, b): return 1",
                Some(true),
            ),
        ]);
    }

    #[test]
    fn corpus_header_without_a_terminator_is_unknown() {
        check_all(&[
            ("kotlin", "val x = 5", "val x = 6", None),
            ("rust", "fn a(x: u8)", "fn a(x: u8) {}", None),
            ("rust", "fn a() {}", "fn a(x: u8)", None),
        ]);
    }

    #[test]
    fn corpus_shapes_the_scanner_could_not_read() {
        check_all(&[
            // Python one-liner whose body holds ` = `: a body edit, now readable.
            ("python", "def f(a): x = 1", "def f(a): y = 1", Some(false)),
            // A call value is not a function: unknown.
            (
                "typescript",
                "const save = debounce((x) => {\n  a();\n}, 100);",
                "const save = debounce((x) => {\n  b();\n}, 100);",
                None,
            ),
            (
                "typescript",
                "export const M = React.memo((p) => {\n  return 1;\n});",
                "export const M = React.memo((p) => {\n  return 2;\n});",
                None,
            ),
            (
                "rust",
                "static R: Lazy<u8> = Lazy::new(|| {\n    1\n});",
                "static R: Lazy<u8> = Lazy::new(|| {\n    2\n});",
                None,
            ),
            // Kotlin expression bodies.
            (
                "kotlin",
                "fun f(x: Int) = x + 1",
                "fun f(x: Int) = x + 2",
                Some(false),
            ),
            (
                "kotlin",
                "fun f(x: Int) = x + 1",
                "fun f(x: Int, y: Int) = x + 1",
                Some(true),
            ),
            // `=>` expression bodies.
            (
                "typescript",
                "const f = (x: number) => x + 1;",
                "const f = (x: number) => x + 2;",
                Some(false),
            ),
            (
                "typescript",
                "const f = (x: number) => x + 1;",
                "const f = (x: number, y: number) => x + 1;",
                Some(true),
            ),
            // Scala has no grammar here.
            (
                "scala",
                "def f(x: Int): Int = x + 1",
                "def f(x: Int): Int = x + 2",
                None,
            ),
            // Bodyless declarations.
            ("rust", "fn a(x: u8);", "fn a(x: u8, y: u8);", None),
            ("rust", "fn a(x: u8);  // one", "fn a(x: u8);  // two", None),
        ]);
    }

    #[test]
    fn corpus_brace_directly_after_an_arrow_or_an_assignment() {
        check_all(&[
            (
                "typescript",
                "const f = (x) => {\n  a();\n}",
                "const f = (x) => {\n  b();\n}",
                Some(false),
            ),
            // An object literal is not a function: unknown (the scanner read it).
            (
                "typescript",
                "const x = {\n  a: 1,\n};",
                "const x = {\n  a: 2,\n};",
                None,
            ),
            (
                "typescript",
                "const x =\n{\n  a: 1,\n};",
                "const x =\n{\n  a: 2,\n};",
                None,
            ),
            (
                "typescript",
                "const f = (x) => {\n  a();\n}",
                "const f = (x, y) => {\n  a();\n}",
                Some(true),
            ),
            (
                "rust",
                "fn a() -> u8 {\n    1\n}",
                "fn a() -> u8 {\n    1\n}",
                Some(false),
            ),
            (
                "rust",
                "fn a<T>(x: T) -> u8 where T: Copy {\n    1\n}",
                "fn a<T>(x: T) -> u8 where T: Copy {\n    1\n}",
                Some(false),
            ),
            (
                "typescript",
                "function f(a: {x: number}) {\n  1;\n}",
                "function f(a: {x: number}) {\n  1;\n}",
                Some(false),
            ),
            (
                "typescript",
                "class A extends B {\n  m() {}\n}",
                "class A extends B {\n  m() {}\n}",
                Some(false),
            ),
        ]);
    }

    #[test]
    fn corpus_brace_inside_an_expression_body() {
        check_all(&[
            (
                "typescript",
                "const f = (x: number) => x ? {a: 1} : {b: 2};",
                "const f = (x: number) => !x ? {a: 1} : {b: 2};",
                Some(false),
            ),
            (
                "kotlin",
                "fun f(x: Int) =\n    when (x) {\n        1 -> 2\n        else -> 3\n    }",
                "fun f(x: Int) =\n    when (x + 1) {\n        1 -> 2\n        else -> 3\n    }",
                Some(false),
            ),
            (
                "kotlin",
                "fun f() = run {\n    1\n}",
                "fun f() = run {\n    2\n}",
                Some(false),
            ),
            (
                "kotlin",
                "fun f() = run {\n    val y = {\n        1\n    }\n}",
                "fun f() = run {\n    val y = {\n        2\n    }\n}",
                Some(false),
            ),
            (
                "typescript",
                "const f = function(x) {\n  a();\n}",
                "const f = function(x) {\n  b();\n}",
                Some(false),
            ),
            (
                "rust",
                "static S: Foo = Foo {\n    a: 1,\n};",
                "static S: Foo = Foo {\n    a: 2,\n};",
                None,
            ),
            (
                "rust",
                "fn a() -> impl Iterator<Item = u8> {\n    1\n}",
                "fn a() -> impl Iterator<Item = u8> {\n    2\n}",
                Some(false),
            ),
        ]);
    }

    #[test]
    fn corpus_python_class_header_ignores_its_body() {
        check_all(&[
            (
                "python",
                "class A:\n    def __init__(self):\n        self.cache = {}\n",
                "class A:\n    def __init__(self):\n        self.items = {}\n",
                Some(false),
            ),
            (
                "python",
                "class A:\n    def f(self):\n        return {1: 2}\n",
                "class A:\n    def g(self):\n        return {1: 2}\n",
                Some(false),
            ),
            (
                "python",
                "class A:\n    \"\"\"Old.\"\"\"\n    def f(self):\n        return {}\n",
                "class A:\n    \"\"\"New.\"\"\"\n    def f(self):\n        return {}\n",
                Some(false),
            ),
            (
                "python",
                "class A(B):\n    def f(self):\n        print({1})\n        return {}",
                "class A(B):\n    def g(self):\n        print({1})\n        return {}",
                Some(false),
            ),
            (
                "python",
                "class A:\n    x = 1\n",
                "class A:\n    x = 2\n",
                Some(false),
            ),
            (
                "python",
                "class A:\n    x = 1\n",
                "class B:\n    x = 1\n",
                Some(true),
            ),
            (
                "python",
                "class A(B):\n    x = 1\n",
                "class A(C):\n    x = 1\n",
                Some(true),
            ),
            (
                "typescript",
                "async function f(a: number): Promise<void> {\n  a;\n}",
                "async function f(a: number): Promise<number> {\n  a;\n}",
                Some(true),
            ),
            (
                "python",
                "async def f(a):\n    return 1",
                "async def f(a, b):\n    return 1",
                Some(true),
            ),
        ]);
    }

    #[test]
    fn corpus_text_that_did_not_read_as_a_header() {
        check_all(&[
            (
                "python",
                "def reset(self): self.cache = {}",
                "def reset(self): self.items = {}",
                Some(false),
            ),
            (
                "python",
                "def reset(self) -> None: self.cache = {}",
                "def reset(self) -> None: self.items = {}",
                Some(false),
            ),
            (
                "typescript",
                "const f = (x) => x ? (y) => {\n  a();\n} : null;",
                "const f = (x) => !x ? (y) => {\n  a();\n} : null;",
                Some(false),
            ),
            (
                "typescript",
                "const f = (a) => (b) => {\n  a();\n}",
                "const f = (a) => (b) => {\n  b();\n}",
                Some(false),
            ),
            // The parameter is renamed too (`o` -> `p`): a header change.
            (
                "typescript",
                "const f = (o) => o !== {\n};",
                "const f = (p) => p !== {\n};",
                Some(true),
            ),
            // Ruby has no grammar here.
            (
                "ruby",
                "def total\n  items.each { |i| p i }\nend",
                "def total\n  items.each { |i| puts i }\nend",
                None,
            ),
            (
                "ruby",
                "def show\n  render json: @user\nend",
                "def show\n  render json: @users\nend",
                None,
            ),
            (
                "ruby",
                "def foo(a)\n  x = {a: 1}\nend",
                "def foo(a)\n  x = {a: 2}\nend",
                None,
            ),
        ]);
    }

    #[test]
    fn corpus_multi_line_decorator_and_attribute() {
        check_all(&[
            (
                "python",
                "@pytest.mark.parametrize(\n    \"a\", [1, 2],\n)\ndef test_x(a):\n    x()\n    assert {a} == {a}",
                "@pytest.mark.parametrize(\n    \"a\", [1, 2],\n)\ndef test_x(a):\n    y()\n    assert {a} == {a}",
                Some(false),
            ),
            (
                "python",
                "@app.route(\n    \"/x\",\n    methods=[\"GET\"],\n)\ndef view(self):\n    self.cache = {}",
                "@app.route(\n    \"/x\",\n    methods=[\"GET\"],\n)\ndef view(self):\n    self.items = {}",
                Some(false),
            ),
            (
                "python",
                "@dataclass(\n    frozen=True,\n)\nclass P:\n    def to_dict(self):\n        return {1: 2}",
                "@dataclass(\n    frozen=True,\n)\nclass P:\n    def as_dict(self):\n        return {1: 2}",
                Some(false),
            ),
            (
                "python",
                "@d(\n    a=1,\n    b=2,\n)\ndef f(x):\n    y = {}",
                "@d(\n    a=1,\n    b=2,\n)\ndef f(x):\n    z = {}",
                Some(false),
            ),
            (
                "python",
                "@d(\n    1,\n)\ndef f(self):\n    return {}",
                "@d(\n    1,\n)\ndef f(self):\n    log()\n    return {}",
                Some(false),
            ),
            (
                "rust",
                "#[derive(\n    Debug,\n)]\nfn a() -> u8 {\n    1\n}",
                "#[derive(\n    Debug,\n)]\nfn a() -> u8 {\n    2\n}",
                Some(false),
            ),
            (
                "python",
                "@d(\n    1,\n)\ndef f(self):\n    return {}",
                "@d(\n    1,\n)\ndef f(self, x):\n    return {}",
                Some(true),
            ),
            // A decorator whose bracket never closes does not parse.
            (
                "python",
                "@d(\n    1,\ndef f(self):\n    return {}",
                "def f(self):",
                None,
            ),
        ]);
    }

    #[test]
    fn corpus_class_with_a_colon_that_is_not_last_on_its_line() {
        check_all(&[
            // The header changes in the first two pairs (a constructor parameter, a
            // supertype); the scanner could not read them.
            (
                "kotlin",
                "class A<T : Any>(x: T) {\n    val y = 1\n}",
                "class A<T : Any>(x: T, z: T) {\n    val y = 1\n}",
                Some(true),
            ),
            (
                "kotlin",
                "class A : B {\n    val y = 1\n}",
                "class A : C {\n    val y = 1\n}",
                Some(true),
            ),
            ("python", "class A: x = {}", "class A: y = {}", Some(false)),
            (
                "python",
                "class A(\n    B,\n):\n    x = 1\n",
                "class A(\n    B,\n):\n    x = 2\n",
                Some(false),
            ),
        ]);
    }

    #[test]
    fn corpus_code_sharing_a_line_with_an_annotation() {
        check_all(&[
            (
                "kotlin",
                "@Suppress(\n    \"x\") fun f(): Int {\n    log()\n    return run {\n        1\n    }\n}",
                "@Suppress(\n    \"x\") fun f(): Int {\n    warn()\n    return run {\n        1\n    }\n}",
                Some(false),
            ),
            (
                "java",
                "@SuppressWarnings({\n  \"a\"}) public int f() {\n  log();\n  int[] a = {1};\n}",
                "@SuppressWarnings({\n  \"a\"}) public int f() {\n  warn();\n  int[] a = {1};\n}",
                Some(false),
            ),
            // The decorator argument changes (`x` -> `y`): a header change.
            (
                "typescript",
                "@Component({\n  selector: \"x\",\n}) export class A {\n  m() {}\n}",
                "@Component({\n  selector: \"y\",\n}) export class A {\n  m() {}\n}",
                Some(true),
            ),
            (
                "rust",
                "#[cfg(\n    test)] fn f() {\n    1\n}",
                "#[cfg(\n    test)] fn f() {\n    2\n}",
                Some(false),
            ),
            (
                "kotlin",
                "@JvmStatic fun f(): Int {\n    log()\n    return run {\n        1\n    }\n}",
                "@JvmStatic fun f(): Int {\n    warn()\n    return run {\n        1\n    }\n}",
                Some(false),
            ),
            (
                "typescript",
                "@HostListener(\"click\") onClick(e) {\n  log();\n  return {a: 1};\n}",
                "@HostListener(\"click\") onClick(e) {\n  warn();\n  return {a: 1};\n}",
                Some(false),
            ),
            (
                "rust",
                "#[inline] fn f() -> S {\n    log();\n    S {\n        a: 1,\n    }\n}",
                "#[inline] fn f() -> S {\n    warn();\n    S {\n        a: 1,\n    }\n}",
                Some(false),
            ),
        ]);
    }

    #[test]
    fn corpus_annotation_alone_on_its_line() {
        for (lang, old, same, changed) in [
            (
                "java",
                "@Override\npublic String toString() {\n  return a;\n}",
                "@Override\npublic String toString() {\n  return b;\n}",
                "@Override\npublic String toString(int x) {\n  return a;\n}",
            ),
            (
                "python",
                "@app.route(\"/x\")\ndef view(self):\n    return {}",
                "@app.route(\"/x\")\ndef view(self):\n    return {1}",
                "@app.route(\"/x\")\ndef view(self, x):\n    return {}",
            ),
            (
                "rust",
                "#[derive(Debug)]\nstruct S {\n    a: u8,\n}",
                "#[derive(Debug)]\nstruct S {\n    a: u16,\n}",
                "#[derive(Debug)]\nstruct T {\n    a: u8,\n}",
            ),
            (
                "python",
                "@d  # note\ndef f(a):\n    pass",
                "@d  # other\ndef f(a):\n    return 1",
                "@d  # note\ndef f(a, b):\n    pass",
            ),
        ] {
            check_all(&[
                (lang, old, same, Some(false)),
                (lang, old, changed, Some(true)),
            ]);
        }
    }
}
