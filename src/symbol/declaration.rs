//! Whether an edit changed a declaration's header, read from the tree-sitter parse.

use std::borrow::Cow;

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

    /// Whether a text that is not a single top-level owner is retried as the sole
    /// member of a wrapper class (a method alone).
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

/// The comment-free leaf tokens from the start of the single top-level declaration
/// in `text` to the start of its body, or `None` when there is no such declaration.
fn declaration_tokens(text: &str, language_id: &str) -> Option<Vec<String>> {
    let family = Family::of(language_id)?;
    let language = crate::ast::get_ts_language(language_id)?;
    let source: Cow<'_, str> = if family == Family::Python {
        Cow::Owned(dedent(text))
    } else {
        Cow::Borrowed(text)
    };

    if let Some(tokens) = parse_clean(&source, &language).and_then(|tree| {
        let owner = sole_member(tree.root_node(), &[])?;
        header_tokens(owner, family, &source)
    }) {
        return Some(tokens);
    }
    if !family.retries_in_wrapper() {
        return None;
    }

    let wrapped = format!("class W {{\n{source}\n}}");
    let tree = parse_clean(&wrapped, &language)?;
    let class = sole_member(tree.root_node(), &[])?;
    if class.kind() != "class_declaration" {
        return None;
    }
    // A member's decorator is a sibling of the member in a TS class body, outside the
    // owner node, so it is skipped like a comment.
    let member = sole_member(class.child_by_field_name("body")?, &["decorator"])?;
    header_tokens(member, family, &wrapped)
}

/// Parse `source`; `None` when the parse fails or the tree holds an ERROR or MISSING node.
fn parse_clean(source: &str, language: &Language) -> Option<Tree> {
    let mut parser = Parser::new();
    parser.set_language(language).ok()?;
    let tree = parser.parse(source, None)?;
    (!tree.root_node().has_error()).then_some(tree)
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

/// The source text of the non-comment leaves under `root` whose byte range lies in
/// `[start, end)`, in document order. A string node without named children counts as
/// one leaf. The walk is iterative so its depth never follows the input's nesting.
fn collect_tokens(root: Node<'_>, start: usize, end: usize, source: &str) -> Option<Vec<String>> {
    let mut tokens = Vec::new();
    let mut cursor = root.walk();
    let mut depth = 0usize;
    loop {
        let node = cursor.node();
        let atom = node.child_count() == 0
            || (node.kind().contains("string") && node.named_child_count() == 0);
        let in_range = node.start_byte() >= start && node.end_byte() <= end;
        if atom {
            if in_range && !is_comment(node.kind()) && node.end_byte() > node.start_byte() {
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

/// Strip the whitespace prefix common to every non-blank line, so an indented Python
/// method parses as a top-level definition.
fn dedent(text: &str) -> String {
    let indent_of =
        |line: &str| -> usize { line.len() - line.trim_start_matches([' ', '\t']).len() };
    let mut common: Option<&str> = None;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let indent = &line[..indent_of(line)];
        common = Some(match common {
            None => indent,
            Some(prev) => {
                let shared = prev
                    .bytes()
                    .zip(indent.bytes())
                    .take_while(|(a, b)| a == b)
                    .count();
                &prev[..shared]
            }
        });
    }
    let common = common.unwrap_or("");
    if common.is_empty() {
        return text.to_owned();
    }
    text.split_inclusive('\n')
        .map(|line| {
            line.strip_prefix(common)
                .unwrap_or_else(|| line.trim_start_matches([' ', '\t']))
        })
        .collect()
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
    fn python_indented_method_is_dedented() {
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
        // of the wrapper class body. The retry fires on "not exactly one owner", so it
        // would also fire if the as-is parse were clean.
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
}
