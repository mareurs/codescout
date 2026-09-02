//! Markdown-specific tools: `edit_markdown`, and `read_markdown`'s heading-addressed
//! reads (folded into `read_file` — see `read_markdown::read`).

pub(crate) mod edit_markdown;
mod frontmatter;
pub(crate) mod read_markdown;

pub use edit_markdown::EditMarkdown;
pub(crate) use read_markdown::{format_read, is_markdown_target, read};

#[cfg(test)]
mod tests;
