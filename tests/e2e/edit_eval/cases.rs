use crate::e2e::edit_eval::types::{
    CompilerExpected, ContentInvariant, EditAction, EditCase, Expected, ReturnExpected,
};
use serde_json::json;
use std::sync::OnceLock;

static CASES: OnceLock<Vec<EditCase>> = OnceLock::new();

pub fn all() -> &'static [EditCase] {
    CASES.get_or_init(|| vec![
        EditCase {
            id: "R-01",
            action: EditAction::Replace,
            input: json!({
                "action": "replace",
                "symbol": "compute",
                "path": "src/replace_plain.rs",
                "body": "pub fn compute(x: i32) -> i32 {\n    x * 2\n}",
            }),
            target_file: "replace_plain.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::Contains { file: "replace_plain.rs", needle: "x * 2", count: 1 },
                    ContentInvariant::NotContains { file: "replace_plain.rs", needle: "x + 1" },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "plain function body replace — happy-path baseline",
            h1_exempt: None,
        },
        EditCase {
            id: "R-02",
            action: EditAction::Replace,
            input: json!({
                "action": "replace",
                "symbol": "impl Greeter for Foo/greet",
                "path": "src/replace_trait_impl.rs",
                "body": "    fn greet(&self) -> String {\n        String::from(\"hi\")\n    }",
            }),
            target_file: "replace_trait_impl.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::Contains { file: "replace_trait_impl.rs", needle: "String::from(\"hi\")", count: 1 },
                    // BUG-054 watchdog: a stray `}` would produce three closing braces in a row.
                    ContentInvariant::NotContains { file: "replace_trait_impl.rs", needle: "}\n}\n}" },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "trait-method body — BUG-054 stray-brace watchdog",
            h1_exempt: Some("BUG-054"),
        },
        EditCase {
            id: "R-03",
            action: EditAction::Replace,
            input: json!({
                "action": "replace",
                "symbol": "parse",
                "path": "src/replace_generic.rs",
                "body": "pub fn parse<T>(s: &str) -> Option<T>\nwhere\n    T: FromStr + Display + 'static,\n{\n    let trimmed = s.trim();\n    trimmed.parse().ok()\n}",
            }),
            target_file: "replace_generic.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::Contains { file: "replace_generic.rs", needle: "s.trim()", count: 1 },
                    ContentInvariant::Contains { file: "replace_generic.rs", needle: "where", count: 1 },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "generic fn with where bounds — preserve signature shape",
            h1_exempt: None,
        },
        EditCase {
            id: "R-04",
            action: EditAction::Replace,
            input: json!({
                "action": "replace",
                "symbol": "impl Counter/b",
                "path": "src/replace_tight_impl.rs",
                "body": "    pub fn b(&self) -> u32 { self.0 * 4 }",
            }),
            target_file: "replace_tight_impl.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::Contains { file: "replace_tight_impl.rs", needle: "self.0 * 4", count: 1 },
                    ContentInvariant::Contains { file: "replace_tight_impl.rs", needle: "pub fn a(&self) -> u32 { self.0 }", count: 1 },
                    ContentInvariant::Contains { file: "replace_tight_impl.rs", needle: "pub fn c(&self) -> u32 { self.0 * 3 }", count: 1 },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "tight impl block — sibling methods must remain intact",
            h1_exempt: None,
        },
        EditCase {
            id: "R-05",
            action: EditAction::Replace,
            input: json!({
                "action": "replace",
                "symbol": "missing_sig",
                "path": "src/replace_no_sig.rs",
                "body": "    99",
            }),
            target_file: "replace_no_sig.rs",
            expected: Expected {
                return_: ReturnExpected::CleanError,
                disk: vec![
                    ContentInvariant::Contains { file: "replace_no_sig.rs", needle: "42", count: 1 },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "body omits signature — must produce CleanError",
            h1_exempt: None,
        },
        EditCase {
            id: "R-06",
            action: EditAction::Replace,
            input: json!({
                "action": "replace",
                "symbol": "foo",
                "path": "src/replace_wrong_sig.rs",
                "body": "pub fn bar() -> u32 {\n    2\n}",
            }),
            target_file: "replace_wrong_sig.rs",
            expected: Expected {
                return_: ReturnExpected::CleanError,
                disk: vec![
                    ContentInvariant::Contains { file: "replace_wrong_sig.rs", needle: "pub fn foo", count: 1 },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "body has different signature than target — must refuse",
            h1_exempt: None,
        },
        EditCase {
            id: "R-07",
            action: EditAction::Replace,
            input: json!({
                "action": "replace",
                "symbol": "outer/inner",
                "path": "src/replace_nested.rs",
                "body": "    fn inner() -> i32 {\n        9\n    }",
            }),
            target_file: "replace_nested.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::Contains { file: "replace_nested.rs", needle: "9", count: 1 },
                    ContentInvariant::NotContains { file: "replace_nested.rs", needle: "7" },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "nested function — symbol path resolves inside outer fn",
            h1_exempt: None,
        },
        EditCase {
            id: "R-08",
            action: EditAction::Replace,
            input: json!({
                "action": "replace",
                "symbol": "documented",
                "path": "src/replace_doc_adj.rs",
                "body": "pub fn documented() -> &'static str {\n    \"after\"\n}",
            }),
            target_file: "replace_doc_adj.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::Contains { file: "replace_doc_adj.rs", needle: "\"after\"", count: 1 },
                    ContentInvariant::Contains { file: "replace_doc_adj.rs", needle: "/// Doc that lives immediately above", count: 1 },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "doc-comment-adjacent function — doc must survive replace",
            h1_exempt: None,
        },
        EditCase {
            id: "I-01",
            action: EditAction::Insert,
            input: json!({
                "action": "insert",
                "symbol": "impl Foo/method_a",
                "path": "src/insert_before_first.rs",
                "position": "before",
                "body": "    pub fn method_zero(&self) -> u32 { 0 }",
            }),
            target_file: "insert_before_first.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::Contains { file: "insert_before_first.rs", needle: "pub fn method_zero", count: 1 },
                    ContentInvariant::Contains { file: "insert_before_first.rs", needle: "pub fn method_a(&self) -> u32 { 1 }", count: 1 },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "insert before first method of impl — sibling method_a intact",
            h1_exempt: None,
        },
        EditCase {
            id: "I-02",
            action: EditAction::Insert,
            input: json!({
                "action": "insert",
                "symbol": "impl Bar/method_z",
                "path": "src/insert_after_last.rs",
                "position": "after",
                "body": "    pub fn method_zz(&self) -> u32 { 26 }",
            }),
            target_file: "insert_after_last.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::Contains { file: "insert_after_last.rs", needle: "method_zz", count: 1 },
                    ContentInvariant::Contains { file: "insert_after_last.rs", needle: "}\n", count: 4 },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "insert after last method at EOF — impl close brace preserved",
            h1_exempt: None,
        },
        EditCase {
            id: "I-03",
            action: EditAction::Insert,
            input: json!({
                "action": "insert",
                "symbol": "target",
                "path": "src/insert_bad_syntax.rs",
                "position": "after",
                "body": "this is not rust",
            }),
            target_file: "insert_bad_syntax.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::Contains { file: "insert_bad_syntax.rs", needle: "this is not rust", count: 1 },
                ],
                compiler: CompilerExpected::Breaks,
            },
            rationale: "tool-faithful contract — disk gets exactly what was asked; compiler legitimately breaks",
            h1_exempt: None,
        },
        EditCase {
            id: "M-01",
            action: EditAction::Remove,
            input: json!({
                "action": "remove",
                "symbol": "orphan",
                "path": "src/remove_clean.rs",
            }),
            target_file: "remove_clean.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::NotContains { file: "remove_clean.rs", needle: "pub fn orphan" },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "remove function with no callers — clean removal",
            h1_exempt: None,
        },
        EditCase {
            id: "M-02",
            action: EditAction::Remove,
            input: json!({
                "action": "remove",
                "symbol": "referenced",
                "path": "src/remove_referenced.rs",
            }),
            target_file: "remove_referenced.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::NotContains { file: "remove_referenced.rs", needle: "pub fn referenced" },
                    ContentInvariant::Contains { file: "remove_referenced.rs", needle: "pub fn caller", count: 1 },
                ],
                compiler: CompilerExpected::Breaks,
            },
            rationale: "remove with same-file caller — tool faithful, compile legitimately breaks",
            h1_exempt: None,
        },
        EditCase {
            id: "N-01",
            action: EditAction::Rename,
            input: json!({
                "action": "rename",
                "symbol": "target_fn",
                "path": "src/rename_target.rs",
                "new_name": "renamed_fn",
            }),
            target_file: "rename_target.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::Contains { file: "rename_target.rs", needle: "pub fn renamed_fn", count: 1 },
                    ContentInvariant::NotContains { file: "rename_target.rs", needle: "target_fn" },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "cross-file rename — LSP updates all callsites; project still builds",
            h1_exempt: None,
        },
    ])
}
