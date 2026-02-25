//! Central orchestrator: manages projects, tool registry, and shared state.

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::project::ProjectConfig;
use crate::memory::MemoryStore;

/// Shared agent state — cloned into each tool invocation.
#[derive(Clone)]
pub struct Agent {
    pub inner: Arc<RwLock<AgentInner>>,
}

pub struct AgentInner {
    pub active_project: Option<ActiveProject>,
}

pub struct ActiveProject {
    pub root: PathBuf,
    pub config: ProjectConfig,
    pub memory: MemoryStore,
}

impl Agent {
    pub async fn new(project: Option<PathBuf>) -> Result<Self> {
        let active_project = if let Some(root) = project {
            let config = ProjectConfig::load_or_default(&root)?;
            let memory = MemoryStore::open(&root)?;
            Some(ActiveProject { root, config, memory })
        } else {
            None
        };

        Ok(Self {
            inner: Arc::new(RwLock::new(AgentInner { active_project })),
        })
    }

    /// Activate a project by path, replacing the current active project.
    pub async fn activate(&self, root: PathBuf) -> Result<()> {
        let config = ProjectConfig::load_or_default(&root)?;
        let memory = MemoryStore::open(&root)?;
        let mut inner = self.inner.write().await;
        inner.active_project = Some(ActiveProject { root, config, memory });
        Ok(())
    }

    /// Get the active project root, or error if none is set.
    pub async fn require_project_root(&self) -> Result<PathBuf> {
        let inner = self.inner.read().await;
        inner
            .active_project
            .as_ref()
            .map(|p| p.root.clone())
            .ok_or_else(|| anyhow::anyhow!("No active project. Use activate_project first."))
    }
}
