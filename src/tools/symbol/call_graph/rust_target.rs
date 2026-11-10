//! Resolve ordinary declared Rust modules without treating arbitrary qualifiers as filenames.
use crate::tools::symbol::call_edges::resolver::Definition;
use std::path::{Path, PathBuf};

fn module_body<'a>(
    tree: &'a tree_sitter::Tree,
    source: &str,
    modules: &[String],
) -> Option<tree_sitter::Node<'a>> {
    let mut body = tree.root_node();
    for name in modules {
        let mut cursor = body.walk();
        let declaration = body.named_children(&mut cursor).find(|n| {
            n.kind() == "mod_item"
                && n.child_by_field_name("name")
                    .and_then(|v| v.utf8_text(source.as_bytes()).ok())
                    == Some(name.as_str())
        })?;
        body = declaration.child_by_field_name("body")?;
    }
    Some(body)
}

pub(super) fn resolve(
    root: &Path,
    source_file: &Path,
    scopes: &[String],
    expression: &str,
) -> Result<Option<Definition>, String> {
    let mut parts = expression.split("::").collect::<Vec<_>>();
    if parts.first() == Some(&"self") {
        parts.remove(0);
    }
    if parts
        .iter()
        .any(|p| matches!(*p, "crate" | "super") || p.is_empty())
    {
        return Err(format!(
            "qualified Rust target `{expression}` needs semantic module resolution"
        ));
    }
    let Some(last) = parts.pop() else {
        return Ok(None);
    };
    let mut file = source_file.to_path_buf();
    let mut inline = scopes.to_vec();
    while let Some(part) = parts.first().copied() {
        let source =
            std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&crate::ast::get_ts_language("rust").unwrap())
            .map_err(|e| e.to_string())?;
        let tree = parser
            .parse(&source, None)
            .ok_or("could not parse Rust module")?;
        let body =
            module_body(&tree, &source, &inline).ok_or("could not locate enclosing Rust module")?;
        let mut cursor = body.walk();
        let declarations = body
            .named_children(&mut cursor)
            .filter(|n| {
                n.kind() == "mod_item"
                    && n.child_by_field_name("name")
                        .and_then(|v| v.utf8_text(source.as_bytes()).ok())
                        == Some(part)
            })
            .collect::<Vec<_>>();
        let [declaration] = declarations.as_slice() else {
            // A type-qualified local method may have an exact name_path; do not infer a file.
            let path = inline
                .iter()
                .map(String::as_str)
                .chain(parts.iter().copied())
                .chain([last])
                .collect::<Vec<_>>()
                .join("/");
            return super::identity::exact_in_file(&file, &path)
                .map_err(|e| e.to_string())?
                .map(Some)
                .ok_or_else(|| format!("unresolved qualified Rust target `{expression}`"));
        };
        if declaration
            .prev_named_sibling()
            .is_some_and(|n| n.kind() == "attribute_item")
        {
            return Err(format!(
                "attributed module in target `{expression}` needs semantic module resolution"
            ));
        }
        parts.remove(0);
        if declaration.child_by_field_name("body").is_some() {
            inline.push(part.to_owned());
            continue;
        }
        let parent = file.parent().ok_or("Rust module has no parent directory")?;
        let stem = file
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("invalid module filename")?;
        let mut directory = if matches!(stem, "lib" | "main" | "mod") {
            parent.to_path_buf()
        } else {
            parent.join(stem)
        };
        for name in &inline {
            directory.push(name);
        }
        let candidates = [
            directory.join(format!("{part}.rs")),
            directory.join(part).join("mod.rs"),
        ];
        let files = candidates
            .into_iter()
            .filter(|p| p.is_file())
            .collect::<Vec<PathBuf>>();
        let [candidate] = files.as_slice() else {
            return Err(format!(
                "unresolved or ambiguous module for Rust target `{expression}`"
            ));
        };
        let canonical = candidate.canonicalize().map_err(|e| e.to_string())?;
        let canonical_root = root.canonicalize().map_err(|e| e.to_string())?;
        if !canonical.starts_with(canonical_root) {
            return Err(format!(
                "Rust module for `{expression}` leaves the project scope"
            ));
        }
        file = canonical;
        inline.clear();
    }
    let path = inline
        .iter()
        .map(String::as_str)
        .chain([last])
        .collect::<Vec<_>>()
        .join("/");
    let found = super::identity::exact_in_file(&file, &path).map_err(|e| e.to_string())?;
    if found.is_none() && expression.contains("::") {
        Err(format!("unresolved qualified Rust target `{expression}`"))
    } else {
        Ok(found)
    }
}
