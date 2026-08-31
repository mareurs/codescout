//! Tool trait and registry.
//!
//! Each tool is a struct that implements the `Tool` trait. Tools are
//! registered in the MCP server at startup.

mod core;
pub use core::*;

pub mod approve_write;
pub mod command_summary;
pub mod config;
pub mod create_file;
pub mod edit_file;
pub(crate) mod edit_repair;
pub mod file_group;
pub mod file_summary;
pub(crate) mod format;
pub mod grep;
pub mod library;
pub mod markdown;
pub mod memory;
pub mod onboarding;
pub mod output;
pub mod output_buffer;
pub mod probe;
pub mod progress;
pub mod read_file;
pub mod run_command;
pub mod section_coverage;
pub mod semantic;
pub mod session_key;
pub mod symbol;
pub mod tree;
pub use onboarding::Onboarding;
pub use run_command::RunCommand;
pub mod guide;
pub mod guide_ledger;
#[cfg(unix)]
pub mod peer;
pub mod rendezvous;
