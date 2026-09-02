//! Markdown-specific tools: heading-addressed edits (folded into `edit_file` — see
//! `edit_markdown::edit`) and heading-addressed reads (folded into `read_file` — see
//! `read_markdown::read`).

pub(crate) mod edit_markdown;
mod frontmatter;
pub(crate) mod read_markdown;

pub(crate) use edit_markdown::{edit, LONG_DOCS};
pub(crate) use read_markdown::{format_read, is_markdown_target, read};

#[cfg(test)]
mod tests;
