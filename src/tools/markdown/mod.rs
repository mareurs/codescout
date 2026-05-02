//! Markdown-specific tools: `read_markdown` and `edit_markdown`.

mod edit_markdown;
mod read_markdown;

pub use edit_markdown::EditMarkdown;
pub use read_markdown::ReadMarkdown;

#[cfg(test)]
mod tests;
