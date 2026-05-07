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

pub(crate) fn nav_block(lang: &str) -> Option<&'static NavBlock> {
    match lang {
        "rust" => Some(&RUST),
        "python" => Some(&PYTHON),
        _ => None,
    }
}

pub(crate) fn supported_languages() -> &'static [&'static str] {
    &["rust", "python"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nav_block_returns_some_for_rust_and_python() {
        assert!(nav_block("rust").is_some());
        assert!(nav_block("python").is_some());
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
            assert!(nav_block(lang).is_some(), "supported but no nav_block: {lang}");
        }
    }
}
