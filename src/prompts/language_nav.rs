//! Per-language navigation hints rendered into `server_instructions.md`
//! at session start. Pure data + pure functions — no I/O.

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
}
