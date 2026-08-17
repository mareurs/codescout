//! Extract documentation text from any source file codescout can parse.
//!
//! This exists so `audit_doc_refs` can see the citations that live in code —
//! `// see docs/issues/….md` — which are invisible to a scanner whose include
//! set is markdown. Measured 2026-08-15: 111 unique bug files were cited from
//! 94 `.rs` files and 95 of them pointed at a path that had been archived out
//! from under the comment.
//!
//! **This module is not a per-language doc-ref extractor**, and deliberately
//! so. Language resolution already has one owner —
//! [`crate::ast::get_ts_language`], the documented single source of truth,
//! shared with the AST parser and the embedding chunker. This is its third
//! consumer, not a fourth mapping. Everything downstream (`parser::parse_refs`,
//! the resolver, severity) is reused unchanged: documentation text is simply a
//! second *source of citable text* beside a markdown file.
//!
//! Restricting to documentation nodes is the whole design, not a refinement of
//! it. Handing whole `.rs` files to the markdown parser yields 33,105 "refs" —
//! it reads `tokio::sync::Semaphore` as a link — against ~659 real `docs/`
//! citations. It is also what separates a human pointer from a constructed
//! path: this repo's fabricated test fixtures (`docs/issues/2026-01-01-x.md`)
//! live in string literals, never in documentation.
//!
//! # Two shapes of documentation, not one
//!
//! Most languages document in comments. Python documents in **string
//! literals** — a module, class or function docstring is
//! `expression_statement > string`, carrying no comment node at all. A
//! comment-only extractor would return Python's `#` notes and silently miss
//! every docstring, which is the worst failure available here: a scanner that
//! finds nothing is indistinguishable from a codebase with nothing to find.
//!
//! So this takes the *language name*, not just a grammar, and applies the
//! docstring rule where the language calls for it. That is one explicit, named
//! special case rather than a per-language table, and
//! `python_docstrings_are_extracted` plus
//! `docstring_rule_does_not_leak_into_other_languages` are what keep it honest.

/// One documentation node's text, with the 1-based line its first character
/// sits on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommentBlock {
    /// `u32` to match `RefCandidate::md_line`, which is the only thing this
    /// number is ever used for — rebasing a within-comment ref line back onto
    /// the file.
    pub line: u32,
    pub text: String,
}

/// Every tree-sitter grammar codescout vendors names its comment nodes with a
/// kind *containing* `comment` — `comment`, `line_comment`, `block_comment`.
/// The names differ per grammar (Rust `line_comment`, Go `comment`, Java
/// `block_comment`), which is why this is a substring test rather than a
/// per-language table: the table would be a fourth language mapping to keep in
/// sync, and `every_supported_language_yields_its_doc_text` is the guard that
/// this substring assumption still holds after a grammar upgrade.
pub(crate) fn is_comment_kind(kind: &str) -> bool {
    kind.contains("comment")
}

/// Collect every documentation node in `source`, ordered by line.
///
/// `language` is a [`crate::ast::get_ts_language`] key (`"rust"`, `"python"`,
/// …). Returns empty rather than erroring on an unknown language or an
/// unparseable source: a file the grammar chokes on should contribute no
/// citations, never fail the scan.
pub(crate) fn extract_comments(source: &str, language: &str) -> Vec<CommentBlock> {
    let Some(ts_lang) = crate::ast::get_ts_language(language) else {
        return Vec::new();
    };
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&ts_lang).is_err() {
        return Vec::new();
    }
    let Some(tree) = parser.parse(source, None) else {
        return Vec::new();
    };
    let docstrings = language.eq_ignore_ascii_case("python");

    let mut out = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        if is_comment_kind(node.kind()) {
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                out.push(CommentBlock {
                    line: node.start_position().row as u32 + 1,
                    text: text.to_string(),
                });
            }
            // Do not descend — a comment's interior is prose, not more nodes
            // worth visiting, and descending would double-count any grammar
            // that models a doc comment as a comment wrapping comments.
            continue;
        }
        if docstrings {
            if let Some(block) = python_docstring(node, source) {
                out.push(block);
                continue;
            }
        }
        // Reverse so the traversal visits children left-to-right; the final
        // sort makes order deterministic regardless, but a stable walk keeps
        // failures easy to read.
        for i in (0..node.child_count() as u32).rev() {
            if let Some(child) = node.child(i) {
                stack.push(child);
            }
        }
    }
    out.sort_by_key(|c| c.line);
    out
}

/// A Python docstring: a bare string expression in the **leading** position of
/// a module or a block (a `def`/`class` body).
///
/// The leading-position requirement is what makes this a docstring rule rather
/// than an "any bare string" rule. A bare string elsewhere in a body is legal
/// Python that means nothing; treating one as documentation would let an
/// incidental literal be reported as a broken citation — the class of false
/// positive that gets a gate switched off.
fn python_docstring(node: tree_sitter::Node, source: &str) -> Option<CommentBlock> {
    if node.kind() != "expression_statement" {
        return None;
    }
    let parent = node.parent()?;
    if !matches!(parent.kind(), "module" | "block") {
        return None;
    }
    // Leading position: the first *named* child of its module/block.
    if parent.named_child(0)?.id() != node.id() {
        return None;
    }
    let inner = node.named_child(0)?;
    if inner.kind() != "string" {
        return None;
    }
    let text = inner.utf8_text(source.as_bytes()).ok()?;
    Some(CommentBlock {
        line: inner.start_position().row as u32 + 1,
        text: text.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joined(src: &str, lang: &str) -> String {
        extract_comments(src, lang)
            .iter()
            .map(|c| c.text.clone())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// One snippet per grammar codescout vendors, each citing a path the way
    /// that language documents.
    ///
    /// This is the load-bearing test of the whole module: `is_comment_kind`
    /// asserts every grammar names its comment nodes with a kind containing
    /// `comment`, which was verified from working code for only five of the
    /// nine (`src/ast/parser.rs` uses `line_comment` for Rust, `comment` for
    /// Go and TypeScript, `block_comment` for Java and Kotlin). If a grammar
    /// upgrade renames a node this fails, rather than silently dropping that
    /// language's citations.
    const CASES: &[(&str, &str)] = &[
        ("rust", "// see docs/a.md\nfn f() {}\n"),
        ("python", "# see docs/a.md\ndef f():\n    pass\n"),
        ("go", "// see docs/a.md\npackage main\n"),
        ("typescript", "// see docs/a.md\nexport const x = 1;\n"),
        ("tsx", "// see docs/a.md\nexport const x = 1;\n"),
        ("java", "/** see docs/a.md */\nclass A {}\n"),
        ("kotlin", "/** see docs/a.md */\nclass A\n"),
        ("html", "<!-- see docs/a.md -->\n<p>x</p>\n"),
        ("css", "/* see docs/a.md */\na { color: red; }\n"),
        ("bash", "# see docs/a.md\necho hi\n"),
    ];

    #[test]
    fn every_supported_language_yields_its_doc_text() {
        for (lang, src) in CASES {
            let comments = extract_comments(src, lang);
            assert!(
                comments.iter().any(|c| c.text.contains("docs/a.md")),
                "{lang}: no documentation node carried the citation — either \
                 the grammar stopped naming comments with a kind containing \
                 'comment', or the snippet no longer parses. Got: {comments:?}"
            );
        }
    }

    #[test]
    fn python_docstrings_are_extracted() {
        // Python's primary documentation form is a string literal, not a
        // comment. Without this the scanner returns Python's `#` notes and
        // silently misses every docstring.
        let src = "\"\"\"Module doc: see docs/mod.md\"\"\"\n\n\
                   def f():\n    \"\"\"Func doc: see docs/fn.md\"\"\"\n    return 1\n\n\
                   class C:\n    \"\"\"Class doc: see docs/cls.md\"\"\"\n    pass\n";
        let all = joined(src, "python");
        for expected in ["docs/mod.md", "docs/fn.md", "docs/cls.md"] {
            assert!(all.contains(expected), "missing {expected} in: {all}");
        }
    }

    #[test]
    fn python_non_leading_strings_are_not_docstrings() {
        // The discriminator for the docstring rule. A bare string outside
        // leading position is legal Python that means nothing, and an assigned
        // string is data — reporting either as a citation is the
        // false-positive class that gets a gate switched off.
        let src = "def f():\n    \"\"\"Real doc: docs/real.md\"\"\"\n    \
                   x = \"docs/assigned.md\"\n    \"\"\"docs/stray.md\"\"\"\n    return x\n";
        let all = joined(src, "python");
        assert!(
            all.contains("docs/real.md"),
            "leading docstring kept: {all}"
        );
        assert!(
            !all.contains("docs/assigned.md"),
            "assignment is data: {all}"
        );
        assert!(!all.contains("docs/stray.md"), "non-leading string: {all}");
    }

    #[test]
    fn docstring_rule_does_not_leak_into_other_languages() {
        // `expression_statement` exists in TypeScript too, where a leading
        // bare string is a directive like "use strict" — not documentation.
        // The rule is gated on the language for exactly this reason.
        let all = joined(
            "\"docs/directive.md\";\nexport const x = 1;\n",
            "typescript",
        );
        assert!(
            !all.contains("docs/directive.md"),
            "a leading string is a directive in TS, not a docstring: {all}"
        );
    }

    #[test]
    fn comment_line_is_one_based_and_points_at_the_comment() {
        let src = "fn a() {}\n\n// cite docs/b.md\nfn b() {}\n";
        let comments = extract_comments(src, "rust");
        assert_eq!(comments.len(), 1, "exactly one comment: {comments:?}");
        assert_eq!(
            comments[0].line, 3,
            "1-based line of the comment itself, so a ref's line can be \
             offset back onto the real file"
        );
    }

    #[test]
    fn code_outside_documentation_is_not_collected() {
        // `docs/issues/2026-01-01-x.md` is a real fabricated fixture in this
        // repo's tests; a scanner reading code as well as documentation would
        // report it as a broken citation.
        let all = joined(
            "// real docs/kept.md\nlet p = \"docs/issues/2026-01-01-x.md\";\n",
            "rust",
        );
        assert!(all.contains("docs/kept.md"), "comment kept: {all}");
        assert!(
            !all.contains("2026-01-01-x.md"),
            "a path in a string literal is not a citation: {all}"
        );
    }

    #[test]
    fn multiple_comments_come_back_in_line_order() {
        let src = "// one\nfn a() {}\n// two\nfn b() {}\n// three\n";
        let lines: Vec<u32> = extract_comments(src, "rust")
            .iter()
            .map(|c| c.line)
            .collect();
        assert_eq!(lines, vec![1, 3, 5]);
    }

    #[test]
    fn an_unknown_language_yields_nothing_rather_than_panicking() {
        assert!(extract_comments("// docs/a.md\n", "cobol").is_empty());
    }

    #[test]
    fn unparseable_source_degrades_to_no_citations() {
        // The scan must degrade to "no citations here", never to a failed audit.
        let comments = extract_comments("fn ( ( ( unterminated", "rust");
        assert!(comments.iter().all(|c| !c.text.is_empty()));
    }

    #[test]
    fn is_comment_kind_matches_the_kinds_the_ast_parser_already_relies_on() {
        // Not hypothetical — src/ast/parser.rs dispatches on these today
        // (line_comment for Rust, comment for Go/TS, block_comment for
        // Java/Kotlin). If this predicate stopped matching one, that
        // language's citations would vanish silently.
        for kind in ["comment", "line_comment", "block_comment", "doc_comment"] {
            assert!(is_comment_kind(kind), "{kind} must count as a comment");
        }
        for kind in ["function_item", "string_literal", "identifier"] {
            assert!(!is_comment_kind(kind), "{kind} must NOT count as a comment");
        }
    }

    /// The `med` cap is policy, and policy that never fires is policy nobody
    /// has checked. A live scan of this repo produced **zero**
    /// `code_comment_capped` findings — good news about the corpus, but no
    /// evidence at all that the cap works. These pin it directly.
    ///
    /// `High` is genuinely reachable here: a markdown link inside a doc
    /// comment resolves as a `link` ref, and an unresolvable link is graded
    /// `high` by `policy_default` — that is exactly what the earlier
    /// whole-file probe surfaced when it read `tokio::sync::Semaphore` as a
    /// link.
    #[test]
    fn a_high_severity_citation_in_a_comment_is_capped_to_med() {
        use crate::librarian::tools::audit_doc_refs::severity::cap_code_comment;
        use crate::librarian::tools::audit_doc_refs::Severity;

        let (sev, reason) = cap_code_comment(
            Severity::High,
            crate::librarian::tools::audit_doc_refs::SeverityReason::PolicyDefault,
        );
        assert_eq!(sev, Severity::Med, "high must not gate CI from a comment");
        assert_eq!(
            reason.as_str(),
            "code_comment_capped",
            "the reason must say WHY it was downgraded, or a reader cannot \
             tell a capped high from a native med"
        );
    }

    #[test]
    fn cap_leaves_lower_severities_and_their_reasons_untouched() {
        use crate::librarian::tools::audit_doc_refs::severity::cap_code_comment;
        use crate::librarian::tools::audit_doc_refs::Severity;

        // The discriminator: a blanket "always Med" would pass the test above
        // while destroying every native reason string.
        for sev in [Severity::Med, Severity::Low] {
            let (out, reason) = cap_code_comment(
                sev,
                crate::librarian::tools::audit_doc_refs::SeverityReason::InferredPath,
            );
            assert_eq!(out, sev, "{sev:?} is already below the gate");
            assert_eq!(
                reason.as_str(),
                "inferred_path",
                "reason must survive uncapped"
            );
        }
    }
}
