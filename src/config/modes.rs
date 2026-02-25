//! Operational modes and contexts (mirrors Serena's context/mode system).

use serde::{Deserialize, Serialize};

/// A named operational mode that adjusts which tools are enabled and
/// how the agent presents itself to the LLM.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Read-only planning pass — no editing tools exposed
    Planning,
    /// Full editing capabilities
    Editing,
    /// Interactive session with all tools
    Interactive,
    /// Single-shot one-time task
    OneShot,
}

/// Deployment context that configures tool visibility and prompts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Context {
    /// Running as a standalone MCP server agent
    Agent,
    /// Embedded in a desktop IDE-like application
    DesktopApp,
    /// Assisting an IDE extension
    IdeAssistant,
}

impl Default for Context {
    fn default() -> Self {
        Self::Agent
    }
}

impl Default for Mode {
    fn default() -> Self {
        Self::Interactive
    }
}
