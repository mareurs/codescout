//! Tool trait and registry.
//!
//! Each tool is a struct that implements the `Tool` trait. Tools are
//! registered in the MCP server at startup.

mod core;
pub use core::*;

pub mod approve_write;
pub mod ast;
pub mod command_summary;
pub mod config;
pub mod create_file;
pub mod edit_file;
pub mod file_group;
pub mod file_summary;
pub(crate) mod format;
pub mod grep;
pub mod library;
pub mod memory;
pub mod output;
pub mod output_buffer;
pub mod progress;
pub mod semantic;
pub mod symbol;
pub mod usage;
pub use usage::GetUsageStats;
pub mod markdown;
pub mod onboarding;
pub mod probe;
pub mod read_file;
pub mod run_command;
pub mod section_coverage;
pub mod tree;
pub use onboarding::Onboarding;
pub use run_command::RunCommand;
pub mod guide;
pub mod peer;
