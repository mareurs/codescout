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
/// The `IC-15` action-labelled schema-key probe, shared by every consolidated tool.
///
/// Deliberately NOT under `src/librarian/`, which is `#[cfg(feature = "librarian")]`: the tools
/// that need it most (`workspace`, `index`, `library`, `edit_file`) do not depend on that
/// feature, and gating their guard on it would delete the guard from the lean lane silently.
#[cfg(test)]
pub(crate) mod param_probe;
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
