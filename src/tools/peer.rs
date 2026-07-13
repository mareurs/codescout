//! The requester-facing `peer` MCP tool. Resolves a peer id from the registry,
//! connects, and runs one of the peer's read tools / reads a peer buffer.
//! Phase 1 is read-only delegation — the peer enforces its own read-only
//! allow-list, so even a mistaken write request is refused at the peer.

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use super::{Tool, ToolContext};
use crate::peer::registry::Registry;

pub struct PeerTool;

#[async_trait::async_trait]
impl Tool for PeerTool {
    fn name(&self) -> &str {
        "peer"
    }

    fn description(&self) -> &str {
        "Delegate read-only exploration to a peer codescout instance that owns another project. \
         action=status lists configured peers; action=query (alias explore) runs one of the peer's \
         read tools via {peer, tool, args}; action=knowledge fetches a peer buffer handle."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["status", "query", "explore", "knowledge"] },
                "peer": { "type": "string", "description": "Registry id of the target peer" },
                "tool": { "type": "string", "description": "For query/explore: the peer tool name" },
                "args": { "type": "object", "description": "For query/explore: the peer tool args" },
                "handle": { "type": "string", "description": "For knowledge: a peer buffer handle" }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        let action = input
            .get("action")
            .and_then(|a| a.as_str())
            .unwrap_or("status");
        let registry_path = ctx
            .agent
            .project_root_for(ctx.workspace_override.as_deref())
            .await
            .map(|r| r.join(".codescout").join("peers.toml"))
            .ok_or_else(|| anyhow!("no active project to resolve the peer registry"))?;
        let registry = Registry::load(&registry_path)?;

        match action {
            "status" => {
                let peers: Vec<Value> = registry
                    .entries()
                    .iter()
                    .map(|p| {
                        json!({
                            "id": p.id,
                            "description": p.description,
                            "read_only": p.default_access.is_read_only(),
                        })
                    })
                    .collect();
                Ok(json!({ "peers": peers }))
            }
            "query" | "explore" => {
                let id = input
                    .get("peer")
                    .and_then(|p| p.as_str())
                    .ok_or_else(|| anyhow!("action={action} requires 'peer'"))?;
                let entry = registry
                    .get(id)
                    .ok_or_else(|| anyhow!("unknown peer: {id}"))?;
                let tool = input
                    .get("tool")
                    .and_then(|t| t.as_str())
                    .ok_or_else(|| anyhow!("action={action} requires 'tool'"))?;
                let tool_args = input.get("args").cloned().unwrap_or_else(|| json!({}));
                let mut client = crate::peer::launch::ensure_peer_serve(
                    &entry.target,
                    entry.default_access.is_read_only(),
                )
                .await?;
                let _caps = client.hello().await?;
                client.call_tool(tool, tool_args).await
            }
            "knowledge" => {
                let id = input
                    .get("peer")
                    .and_then(|p| p.as_str())
                    .ok_or_else(|| anyhow!("action=knowledge requires 'peer'"))?;
                let entry = registry
                    .get(id)
                    .ok_or_else(|| anyhow!("unknown peer: {id}"))?;
                let handle = input
                    .get("handle")
                    .and_then(|h| h.as_str())
                    .ok_or_else(|| anyhow!("action=knowledge requires 'handle'"))?;
                let mut client = crate::peer::launch::ensure_peer_serve(
                    &entry.target,
                    entry.default_access.is_read_only(),
                )
                .await?;
                client.read_buffer(handle).await
            }
            other => Err(anyhow!("unknown peer action: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_tool_advertises_name_and_actions() {
        let t = PeerTool;
        assert_eq!(t.name(), "peer");
        assert!(
            !t.is_write(&serde_json::json!({})),
            "peer tool is read-only"
        );
        let schema = t.input_schema();
        let actions = schema["properties"]["action"]["enum"].as_array().unwrap();
        for a in ["status", "query", "explore", "knowledge"] {
            assert!(actions.iter().any(|v| v == a), "missing action {a}");
        }
    }

    #[tokio::test]
    async fn peer_tool_honors_workspace_override_pin() {
        // BUG (docs/issues/2026-07-09-residual-workspace-pin-gaps-post-edit-code-fix.md,
        // finding 1): PeerTool never overrides Tool::pinnable(), so its schema
        // advertises a `workspace` param and ctx.workspace_override IS populated —
        // but the body resolved `.codescout/peers.toml` via the plain, unpinned
        // project_root(). A pinned call read the session-default project's registry.
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir_a.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir_b.path().join(".codescout")).unwrap();

        // Only workspace A has a peer registry. If the pin is honored, `status`
        // lists A's peer; if it is ignored, it reads B's (absent) registry and
        // Registry::load returns an empty registry — an empty peer list.
        std::fs::write(
            dir_a.path().join(".codescout").join("peers.toml"),
            "[[peer]]\nid = \"alpha\"\ntarget = \"/tmp/alpha\"\ndescription = \"the pinned peer\"\ndefault_access = \"ro\"\n",
        )
        .unwrap();
        let canon_a = std::fs::canonicalize(dir_a.path()).unwrap();

        // Default (unpinned) project is B; pin THIS call to A.
        let agent = crate::agent::Agent::new(Some(dir_b.path().to_path_buf()))
            .await
            .unwrap();
        let ctx = ToolContext {
            agent,
            lsp: crate::lsp::manager::LspManager::new_arc(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
            workspace_override: Some(canon_a),
        };

        let out = PeerTool
            .call(serde_json::json!({"action": "status"}), &ctx)
            .await
            .unwrap();
        let peers = out["peers"].as_array().unwrap();
        assert_eq!(
            peers.len(),
            1,
            "must read the PINNED workspace A's peers.toml, not the default B's; got: {out}"
        );
        assert_eq!(peers[0]["id"], "alpha");
    }
}
