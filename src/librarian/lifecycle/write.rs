//! Pure write functions for lifecycle entries: no file, catalog or clock access. This module holds
//! the shared problem type and `entry_template`, which reads the template section of a collection
//! readme.

use super::entry::EntryProblem;
use super::naming::StemError;
use crate::util::markdown_fence::FenceState;

/// Why a write function refused to produce a document.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WriteProblem {
    #[error("entry_kind {0:?} is not declared in the collection readme")]
    KindUndeclared(String),
    #[error("status {0:?} is not declared in the collection readme")]
    StatusUndeclared(String),
    #[error("title {0:?} is not a one-line title with a slug")]
    Title(String),
    #[error("no entry stem can be built: {0:?}")]
    Stem(StemError),
    #[error("frontmatter is not valid YAML: {0}")]
    BadYaml(String),
    #[error("the document has no frontmatter")]
    NoFrontmatter,
    #[error("field {0:?} is not a writable field of this collection")]
    FieldUndeclared(String),
    #[error("status {0:?} takes no revisit_by date")]
    RevisitNotNeeded(String),
    #[error("the result is not a valid entry: {0:?}")]
    Invalid(Vec<EntryProblem>),
}

const TEMPLATE_HEADING: &str = "## Entry template";

/// The body of the `## Entry template` section of a collection readme, trimmed: the lines after
/// that exact heading up to the next line starting `## ` outside a code fence. `None` when the
/// heading is absent or the section is empty. A heading-like line inside a fence neither starts
/// nor ends the section.
pub fn entry_template(readme_body: &str) -> Option<String> {
    let mut fence = FenceState::new();
    let mut in_section = false;
    let mut collected: Vec<&str> = Vec::new();
    for line in readme_body.lines() {
        let is_delim = fence.feed(line.trim_start());
        let fenced = is_delim || fence.in_fence();
        if !fenced {
            if in_section && line.starts_with("## ") {
                break;
            }
            if !in_section && line == TEMPLATE_HEADING {
                in_section = true;
                continue;
            }
        }
        if in_section {
            collected.push(line);
        }
    }
    let text = collected.join("\n");
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

#[cfg(test)]
mod tests;
