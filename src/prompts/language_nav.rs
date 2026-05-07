//! Per-language navigation hints rendered into `server_instructions.md`
//! at session start. Pure data + pure functions — no I/O.

use std::collections::BTreeMap;


/// One language's symbol-navigation hint block.
pub(crate) struct NavBlock {
    pub language: &'static str,
    pub display_name: &'static str,
    pub markdown: &'static str,
}

const RUST: NavBlock = NavBlock {
    language: "rust",
    display_name: "Rust",
    markdown: "### Rust — Symbol Navigation\n\
        - **`name_path` form:** `Type/method`, `impl Trait for Type/method`\n\
        - **Find a method:** `symbols(name_path=\"Service/handle\", include_body=true)`\n\
        - **List by kind:** `symbols(path=\"src/\", kind=\"struct\")` (also `\"interface\"` for traits)\n\
        - **Language note:** trait impls use `impl Trait for Type/method`; rust-analyzer reports traits as `kind=\"interface\"`\n\
        - **Before refactor:** `call_graph(symbol=\"Service/handle\", path=\"src/service.rs\", direction=\"callers\", max_depth=3)`\n",
};

const PYTHON: NavBlock = NavBlock {
    language: "python",
    display_name: "Python",
    markdown: "### Python — Symbol Navigation\n\
        - **`name_path` form:** `Class/method`, `module_func`\n\
        - **Find a method:** `symbols(name_path=\"Service/handle\", include_body=true)`\n\
        - **List by kind:** `symbols(path=\"src/\", kind=\"class\")`\n\
        - **Language note:** decorators are not part of the symbol — search by the decorated function's name\n\
        - **Before refactor:** `call_graph(symbol=\"Service/handle\", path=\"src/service.py\", direction=\"callers\", max_depth=3)`\n",
};

const TYPESCRIPT: NavBlock = NavBlock {
    language: "typescript",
    display_name: "TypeScript / JavaScript",
    markdown: "### TypeScript / JavaScript — Symbol Navigation\n\
        - **`name_path` form:** `Class/method`, `exportedFunction`\n\
        - **Find a method:** `symbols(name_path=\"Service/handle\", include_body=true)`\n\
        - **List by kind:** `symbols(path=\"src/\", kind=\"class\")` for classes; `kind=\"function\"` for arrow fns\n\
        - **Language note:** React function components are `kind=\"function\"`, not `kind=\"class\"`\n\
        - **Before refactor:** `call_graph(symbol=\"Service/handle\", path=\"src/service.ts\", direction=\"callers\", max_depth=3)`\n",
};

const KOTLIN: NavBlock = NavBlock {
    language: "kotlin",
    display_name: "Kotlin / Java",
    markdown: "### Kotlin / Java — Symbol Navigation\n\
        - **`name_path` form:** `Class/method`, `Object.companion/method`\n\
        - **Find a method:** `symbols(name_path=\"Service/handle\", include_body=true)`\n\
        - **List by kind:** `symbols(path=\"src/\", kind=\"class\")` (covers classes, objects, annotations)\n\
        - **Language note:** annotations are not part of the symbol — search by method name\n\
        - **Before refactor:** `call_graph(symbol=\"Service/handle\", path=\"src/Service.kt\", direction=\"callers\", max_depth=3)`\n",
};

const GO: NavBlock = NavBlock {
    language: "go",
    display_name: "Go",
    markdown: "### Go — Symbol Navigation\n\
        - **`name_path` form:** `Type/Method`, `PackageFunc`\n\
        - **Find a method:** `symbols(name_path=\"Service/Handle\", include_body=true)`\n\
        - **List by kind:** `symbols(path=\"./\", kind=\"function\")` (covers funcs and methods)\n\
        - **Language note:** interfaces use `kind=\"interface\"`; receiver methods stay in `Type/Method` form\n\
        - **Before refactor:** `call_graph(symbol=\"Service/Handle\", path=\"service.go\", direction=\"callers\", max_depth=3)`\n",
};

const CSHARP: NavBlock = NavBlock {
    language: "csharp",
    display_name: "C#",
    markdown: "### C# — Symbol Navigation\n\
        - **`name_path` form:** `Class/Method`, `Namespace.Class/Method` for nested\n\
        - **Find a method:** `symbols(name_path=\"Service/Handle\", include_body=true)`\n\
        - **List by kind:** `symbols(path=\"src/\", kind=\"class\")` (also `\"interface\"`)\n\
        - **Language note:** properties surface as `kind=\"function\"` getters/setters in some LSPs\n\
        - **Before refactor:** `call_graph(symbol=\"Service/Handle\", path=\"src/Service.cs\", direction=\"callers\", max_depth=3)`\n",
};

pub(crate) fn nav_block(lang: &str) -> Option<&'static NavBlock> {
    match lang {
        "rust" => Some(&RUST),
        "python" => Some(&PYTHON),
        "typescript" | "javascript" | "tsx" | "jsx" => Some(&TYPESCRIPT),
        "kotlin" | "java" => Some(&KOTLIN),
        "go" => Some(&GO),
        "csharp" => Some(&CSHARP),
        _ => None,
    }
}

pub(crate) fn supported_languages() -> &'static [&'static str] {
    &["rust", "python", "typescript", "kotlin", "go", "csharp"]
}



/// Map an arbitrary language string to its canonical key (the one that
/// appears in `supported_languages()`). Returns `None` for unsupported.
fn canonical_key(lang: &str) -> Option<&'static str> {
    match lang {
        "rust" => Some("rust"),
        "python" => Some("python"),
        "typescript" | "javascript" | "tsx" | "jsx" => Some("typescript"),
        "kotlin" | "java" => Some("kotlin"),
        "go" => Some("go"),
        "csharp" => Some("csharp"),
        _ => None,
    }
}

/// Rank workspace languages by occurrence count and return the top `max`
/// supported canonical keys. Ties broken alphabetically for determinism.
pub(crate) fn rank_workspace_languages(
    project_languages: &[Vec<String>],
    max: usize,
) -> Vec<&'static str> {
    let mut counts: BTreeMap<&'static str, u32> = BTreeMap::new();
    for langs in project_languages {
        for lang in langs {
            if let Some(key) = canonical_key(lang) {
                *counts.entry(key).or_insert(0) += 1;
            }
        }
    }
    let mut ranked: Vec<(&'static str, u32)> = counts.into_iter().collect();
    // Sort by count descending; BTreeMap iter is already alphabetical for ties.
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    ranked.into_iter().take(max).map(|(k, _)| k).collect()
}

const LEAD_IN: &str = "### Symbol Navigation Patterns\n\
\n\
- **Hierarchical nav** — impl/class methods, all languages:\n\
  `symbols(name_path=\"MyStruct/my_method\", include_body=true)`\n\
- **Kind filter + path scope:**\n\
  `symbols(path=\"src/tools/\", kind=\"struct\")`\n\
- **Find across project then read body:**\n\
  `symbols(name=\"edit_code\")` → `symbols(name_path=\"ToolName/edit_code\", include_body=true)`\n\
\n";

const GENERIC: &str = "### Generic Patterns (any language)\n\
\n\
- `name_path` syntax: `Container/member` for methods on classes/structs/objects;\n\
  bare name for top-level functions or types.\n\
- `kind` filter values vary by language: `function`, `class`, `struct`, `interface`,\n\
  `type`, `enum`, `module`, `constant`. Run `symbols(path)` once on a representative\n\
  file to see what kinds your LSP emits.\n\
- For impact analysis before any structural change:\n\
  `call_graph(symbol, path, direction=\"callers\")` traces blast radius;\n\
  `direction=\"callees\"` traces outbound flow.\n\
- When the symbol's exact name is unknown, start with\n\
  `semantic_search(\"what it does\")` then drill down with `symbols(name_path=...)`.\n";

pub(crate) fn render_symbol_navigation_block(
    project_languages: &[Vec<String>],
) -> String {
    let ranked = rank_workspace_languages(project_languages, 2);
    let mut out = String::with_capacity(2048);
    out.push_str(LEAD_IN);
    for key in ranked {
        if let Some(block) = nav_block(key) {
            out.push_str(block.markdown);
            out.push('\n');
        }
    }
    out.push_str(GENERIC);
    out
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nav_block_returns_some_for_all_supported_languages() {
        for lang in [
            "rust",
            "python",
            "typescript",
            "javascript",
            "tsx",
            "jsx",
            "kotlin",
            "java",
            "go",
            "csharp",
        ] {
            assert!(nav_block(lang).is_some(), "missing nav_block for {lang}");
        }
    }

    #[test]
    fn every_nav_block_has_required_bullets() {
        for lang in supported_languages() {
            let block = nav_block(lang).unwrap();
            let md = block.markdown;
            for marker in [
                "**`name_path` form:**",
                "**Find a method:**",
                "**List by kind:**",
                "**Language note:**",
                "**Before refactor:**",
            ] {
                assert!(md.contains(marker), "{} missing bullet: {marker}", lang);
            }
        }
    }

    #[test]
    fn every_nav_block_uses_only_generic_example_names() {
        let allowed_caps = ["Service", "Repository", "Order", "Account"];
        let allowed_lower = ["find", "handle", "process", "create", "core", "worker"];
        let banned = [
            "MyStruct",
            "UserService",
            "AuthProvider",
            "UserRepository",
            "Server/handle_request",
            "UserService/create",
            "AuthProvider/login",
            "UserRepository/findById",
        ];
        for lang in supported_languages() {
            let block = nav_block(lang).unwrap();
            let md = block.markdown;
            for b in banned {
                assert!(
                    !md.contains(b),
                    "{} uses banned example name {b} (drift risk)",
                    lang
                );
            }
            let _ = (allowed_caps, allowed_lower); // documentation; not asserted positively
        }
    }

    #[test]
    fn nav_block_returns_none_for_unsupported() {
        assert!(nav_block("bash").is_none());
        assert!(nav_block("markdown").is_none());
        assert!(nav_block("unknown_lang").is_none());
    }

    #[test]
    fn supported_languages_lists_all_with_nav_blocks() {
        for lang in supported_languages() {
            assert!(
                nav_block(lang).is_some(),
                "supported but no nav_block: {lang}"
            );
        }
    }
    #[test]
    fn rank_workspace_languages_picks_top_2_by_weight() {
        // Three projects: rust appears 3x, python 2x, kotlin 1x
        let lists: Vec<Vec<String>> = vec![
            vec!["rust".into()],
            vec!["rust".into(), "python".into()],
            vec!["rust".into(), "python".into(), "kotlin".into()],
        ];
        let ranked = rank_workspace_languages(&lists, 2);
        assert_eq!(ranked, vec!["rust", "python"]);
    }

    #[test]
    fn rank_workspace_languages_filters_unsupported() {
        let lists: Vec<Vec<String>> = vec![vec!["bash".into(), "rust".into()]];
        let ranked = rank_workspace_languages(&lists, 2);
        assert_eq!(ranked, vec!["rust"]);
    }

    #[test]
    fn rank_workspace_languages_deterministic_on_ties() {
        // rust and python both appear once → alphabetical order
        let lists: Vec<Vec<String>> = vec![vec!["rust".into(), "python".into()]];
        let ranked = rank_workspace_languages(&lists, 2);
        assert_eq!(ranked, vec!["python", "rust"]);
    }

    #[test]
    fn rank_workspace_languages_caps_at_max() {
        let lists: Vec<Vec<String>> = vec![vec![
            "rust".into(), "python".into(), "kotlin".into(),
            "go".into(), "csharp".into(),
        ]];
        let ranked = rank_workspace_languages(&lists, 2);
        assert_eq!(ranked.len(), 2);
    }

    #[test]
    fn rank_workspace_languages_handles_empty() {
        let lists: Vec<Vec<String>> = vec![];
        let ranked = rank_workspace_languages(&lists, 2);
        assert!(ranked.is_empty());
    }

    #[test]
    fn rank_workspace_languages_normalizes_aliases() {
        // "javascript" should bucket into "typescript" canonical key
        let lists: Vec<Vec<String>> = vec![
            vec!["javascript".into()],
            vec!["typescript".into()],
            vec!["jsx".into()],
        ];
        let ranked = rank_workspace_languages(&lists, 2);
        assert_eq!(ranked, vec!["typescript"]);
    }

    #[test]
    fn render_with_no_languages_emits_lead_in_and_generic() {
        let lists: Vec<Vec<String>> = vec![];
        let out = render_symbol_navigation_block(&lists);
        assert!(out.contains("### Symbol Navigation Patterns"));
        assert!(out.contains("### Generic Patterns (any language)"));
        // No per-language sections
        assert!(!out.contains("### Rust — Symbol Navigation"));
        assert!(!out.contains("### Python — Symbol Navigation"));
    }

    #[test]
    fn render_with_one_language_emits_one_block() {
        let lists: Vec<Vec<String>> = vec![vec!["rust".into()]];
        let out = render_symbol_navigation_block(&lists);
        assert!(out.contains("### Rust — Symbol Navigation"));
        assert!(!out.contains("### Python — Symbol Navigation"));
        assert!(out.contains("### Generic Patterns (any language)"));
    }

    #[test]
    fn render_with_many_languages_caps_at_two() {
        let lists: Vec<Vec<String>> = vec![vec![
            "rust".into(), "python".into(), "kotlin".into(),
            "go".into(), "csharp".into(),
        ]];
        let out = render_symbol_navigation_block(&lists);
        let n_blocks = ["### Rust — Symbol Navigation",
                         "### Python — Symbol Navigation",
                         "### Kotlin / Java — Symbol Navigation",
                         "### Go — Symbol Navigation",
                         "### C# — Symbol Navigation"]
            .iter()
            .filter(|h| out.contains(*h))
            .count();
        assert_eq!(n_blocks, 2);
    }

    #[test]
    fn render_contains_no_deprecated_tool_names() {
        let lists: Vec<Vec<String>> = vec![vec![
            "rust".into(), "python".into(), "typescript".into(),
            "kotlin".into(), "go".into(),
        ]];
        let out = render_symbol_navigation_block(&lists);
        for dead in ["find_symbol", "list_symbols", "replace_symbol",
                     "insert_code", "rename_symbol", "search_pattern"] {
            assert!(!out.contains(dead),
                "rendered block contains deprecated tool name: {dead}");
        }
    }

}
