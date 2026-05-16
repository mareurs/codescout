//! CLI dispatch layer for `codescout artifact*` subcommands.
//!
//! Each verb translates clap-parsed args into a `serde_json::Value` shaped
//! like the corresponding librarian-mcp tool's input, calls the tool, and
//! routes the response through `format::print`.

pub mod format;
