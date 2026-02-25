//! Workflow and onboarding tools.

use super::{Tool, ToolContext};
use anyhow::anyhow;
use serde_json::{json, Value};

pub struct Onboarding;
pub struct CheckOnboardingPerformed;
pub struct ExecuteShellCommand;

#[async_trait::async_trait]
impl Tool for Onboarding {
    fn name(&self) -> &str {
        "onboarding"
    }
    fn description(&self) -> &str {
        "Perform initial project discovery: detect languages, list top-level structure, \
         create config. Requires an active project."
    }
    fn input_schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }
    async fn call(&self, _input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let root = ctx.agent.require_project_root().await?;

        // Detect languages by walking files
        let mut languages = std::collections::BTreeSet::new();
        let walker = ignore::WalkBuilder::new(&root)
            .hidden(true)
            .git_ignore(true)
            .build();
        for entry in walker.flatten() {
            if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                if let Some(lang) = crate::ast::detect_language(entry.path()) {
                    languages.insert(lang.to_string());
                }
            }
        }

        // List top-level entries
        let mut top_level = vec![];
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let suffix = if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    "/"
                } else {
                    ""
                };
                top_level.push(format!("{}{}", name, suffix));
            }
        }
        top_level.sort();

        // Create .code-explorer/project.toml if it doesn't exist
        let config_dir = root.join(".code-explorer");
        let config_path = config_dir.join("project.toml");
        let created_config = if !config_path.exists() {
            std::fs::create_dir_all(&config_dir)?;
            let name = root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unnamed")
                .to_string();
            let langs: Vec<String> = languages.iter().cloned().collect();
            let config = crate::config::project::ProjectConfig {
                project: crate::config::project::ProjectSection {
                    name,
                    languages: langs,
                    encoding: "utf-8".into(),
                    tool_timeout_secs: 60,
                },
                embeddings: Default::default(),
                ignored_paths: Default::default(),
                security: Default::default(),
            };
            let toml_str = toml::to_string_pretty(&config)?;
            std::fs::write(&config_path, &toml_str)?;
            true
        } else {
            false
        };

        // Store onboarding result in memory
        ctx.agent
            .with_project(|p| {
                let summary = format!(
                    "Languages: {}\nTop-level: {}\nConfig created: {}",
                    languages.iter().cloned().collect::<Vec<_>>().join(", "),
                    top_level.join(", "),
                    created_config
                );
                p.memory.write("onboarding", &summary)?;
                Ok(())
            })
            .await?;

        // Build the onboarding instruction prompt
        let lang_list: Vec<String> = languages.iter().cloned().collect();
        let prompt = crate::prompts::build_onboarding_prompt(&lang_list, &top_level);

        Ok(json!({
            "languages": lang_list,
            "top_level": top_level,
            "config_created": created_config,
            "instructions": prompt,
        }))
    }
}

#[async_trait::async_trait]
impl Tool for CheckOnboardingPerformed {
    fn name(&self) -> &str {
        "check_onboarding_performed"
    }
    fn description(&self) -> &str {
        "Check whether project onboarding has been performed for the active project."
    }
    fn input_schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }
    async fn call(&self, _input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        ctx.agent
            .with_project(|p| {
                let has_config = p.root.join(".code-explorer").join("project.toml").exists();
                let memories = p.memory.list()?;
                let has_onboarding_memory = memories.iter().any(|m| m == "onboarding");
                let onboarded = has_config && has_onboarding_memory;

                let message = if onboarded {
                    format!(
                        "Onboarding already performed. Available memories: {}. \
                         Use `read_memory(topic)` to read relevant ones as needed for your current task. \
                         Do not read all memories at once — only read those relevant to what you're working on.",
                        memories.join(", ")
                    )
                } else {
                    "Onboarding not performed yet. Call the `onboarding` tool to discover the project \
                     and create memories that will help you work effectively.".to_string()
                };

                Ok(json!({
                    "onboarded": onboarded,
                    "has_config": has_config,
                    "has_onboarding_memory": has_onboarding_memory,
                    "memories": memories,
                    "message": message,
                }))
            })
            .await
    }
}

#[async_trait::async_trait]
impl Tool for ExecuteShellCommand {
    fn name(&self) -> &str {
        "execute_shell_command"
    }
    fn description(&self) -> &str {
        "Run a shell command in the active project root and return stdout/stderr."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["command"],
            "properties": {
                "command": { "type": "string" },
                "timeout_secs": { "type": "integer", "default": 30 }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let command = input["command"]
            .as_str()
            .ok_or_else(|| anyhow!("missing 'command' parameter"))?;
        let timeout_secs = input["timeout_secs"].as_u64().unwrap_or(30);
        let root = ctx.agent.require_project_root().await?;
        let security = ctx.agent.security_config().await;

        // Check shell command mode
        match security.shell_command_mode.as_str() {
            "disabled" => {
                anyhow::bail!(
                    "shell commands are disabled. Set security.shell_command_mode = \"warn\" or \"unrestricted\" in .code-explorer/project.toml"
                );
            }
            "unrestricted" | "warn" | "" => {} // allowed
            other => {
                anyhow::bail!("unknown shell_command_mode: '{}'. Use \"warn\", \"unrestricted\", or \"disabled\"", other);
            }
        }

        #[cfg(unix)]
        let child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&root)
            .output();

        #[cfg(windows)]
        let child = tokio::process::Command::new("cmd")
            .arg("/C")
            .arg(command)
            .current_dir(&root)
            .output();

        let output_limit = if security.shell_output_limit_bytes > 0 {
            security.shell_output_limit_bytes
        } else {
            100 * 1024
        };

        match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), child).await {
            Ok(Ok(output)) => {
                let raw_stdout = String::from_utf8_lossy(&output.stdout);
                let raw_stderr = String::from_utf8_lossy(&output.stderr);

                let (stdout, stdout_truncated) = truncate_output(&raw_stdout, output_limit);
                let (stderr, stderr_truncated) = truncate_output(&raw_stderr, output_limit);

                let mut result = json!({
                    "stdout": stdout,
                    "stderr": stderr,
                    "exit_code": output.status.code()
                });

                if stdout_truncated {
                    result["stdout_truncated"] = json!(true);
                    result["stdout_total_bytes"] = json!(raw_stdout.len());
                }
                if stderr_truncated {
                    result["stderr_truncated"] = json!(true);
                    result["stderr_total_bytes"] = json!(raw_stderr.len());
                }

                // Add warning in warn mode
                if security.shell_command_mode == "warn" || security.shell_command_mode.is_empty() {
                    result["warning"] = json!(
                        "Shell commands execute with full user permissions. Only use for build/test commands."
                    );
                }

                Ok(result)
            }
            Ok(Err(e)) => Err(anyhow!("command execution error: {}", e)),
            Err(_) => Ok(json!({
                "timed_out": true,
                "stdout": "",
                "stderr": format!("Command timed out after {} seconds", timeout_secs),
                "exit_code": null
            })),
        }
    }
}

fn truncate_output(output: &str, limit: usize) -> (String, bool) {
    if output.len() > limit {
        let truncated = &output[..limit];
        // Find a safe UTF-8 boundary
        let safe_end = truncated
            .char_indices()
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        (
            format!(
                "{}\n... (truncated, showing first {} of {} bytes)",
                &output[..safe_end],
                safe_end,
                output.len()
            ),
            true,
        )
    } else {
        (output.to_string(), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use crate::lsp::LspManager;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn lsp() -> Arc<LspManager> {
        Arc::new(LspManager::new())
    }

    async fn project_ctx() -> (tempfile::TempDir, ToolContext) {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".code-explorer")).unwrap();
        // Create some source files for language detection
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.path().join("lib.py"), "def hello(): pass").unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        (dir, ToolContext { agent, lsp: lsp() })
    }

    #[tokio::test]
    async fn onboarding_detects_languages() {
        let (_dir, ctx) = project_ctx().await;
        let result = Onboarding.call(json!({}), &ctx).await.unwrap();
        let langs: Vec<&str> = result["languages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(langs.contains(&"rust"));
        assert!(langs.contains(&"python"));
    }

    #[tokio::test]
    async fn onboarding_creates_config() {
        let (dir, ctx) = project_ctx().await;
        // Remove the config if it exists
        let _ = std::fs::remove_file(dir.path().join(".code-explorer/project.toml"));

        let result = Onboarding.call(json!({}), &ctx).await.unwrap();
        assert_eq!(result["config_created"], true);
        assert!(dir.path().join(".code-explorer/project.toml").exists());
    }

    #[tokio::test]
    async fn check_onboarding_before_and_after() {
        let (dir, ctx) = project_ctx().await;
        let _ = std::fs::remove_file(dir.path().join(".code-explorer/project.toml"));

        // Before onboarding
        let result = CheckOnboardingPerformed
            .call(json!({}), &ctx)
            .await
            .unwrap();
        assert_eq!(result["onboarded"], false);

        // Run onboarding
        Onboarding.call(json!({}), &ctx).await.unwrap();

        // After onboarding
        let result = CheckOnboardingPerformed
            .call(json!({}), &ctx)
            .await
            .unwrap();
        assert_eq!(result["onboarded"], true);
        assert_eq!(result["has_config"], true);
        assert_eq!(result["has_onboarding_memory"], true);
    }

    #[tokio::test]
    async fn onboarding_returns_instruction_prompt() {
        let (_dir, ctx) = project_ctx().await;
        let result = Onboarding.call(json!({}), &ctx).await.unwrap();
        let instructions = result["instructions"].as_str().unwrap();
        assert!(instructions.contains("## What to Explore"));
        assert!(instructions.contains("## Memories to Create"));
        assert!(instructions.contains("rust")); // detected language
    }

    #[tokio::test]
    async fn onboarding_errors_without_project() {
        let ctx = ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: lsp(),
        };
        assert!(Onboarding.call(json!({}), &ctx).await.is_err());
    }

    #[tokio::test]
    async fn check_onboarding_returns_guidance_message() {
        let (_dir, ctx) = project_ctx().await;

        // Before onboarding
        let result = CheckOnboardingPerformed
            .call(json!({}), &ctx)
            .await
            .unwrap();
        assert!(result["message"]
            .as_str()
            .unwrap()
            .contains("not performed yet"));

        // Run onboarding
        Onboarding.call(json!({}), &ctx).await.unwrap();

        // After onboarding
        let result = CheckOnboardingPerformed
            .call(json!({}), &ctx)
            .await
            .unwrap();
        let msg = result["message"].as_str().unwrap();
        assert!(msg.contains("already performed"));
        assert!(result["memories"].as_array().unwrap().len() > 0);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn execute_shell_command_timeout_is_enforced() {
        let (_dir, ctx) = project_ctx().await;
        let result = ExecuteShellCommand
            .call(json!({ "command": "sleep 10", "timeout_secs": 1 }), &ctx)
            .await
            .unwrap();
        assert_eq!(result["timed_out"], true, "command should have timed out");
        assert!(result["stderr"]
            .as_str()
            .unwrap()
            .contains("timed out after 1 seconds"));
    }

    #[tokio::test]
    async fn execute_shell_command_fast_command_succeeds() {
        let (_dir, ctx) = project_ctx().await;
        let result = ExecuteShellCommand
            .call(json!({ "command": "echo hello", "timeout_secs": 5 }), &ctx)
            .await
            .unwrap();
        assert_eq!(result["timed_out"], serde_json::Value::Null);
        assert!(result["stdout"].as_str().unwrap().contains("hello"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn execute_shell_command_output_truncated() {
        let (_dir, ctx) = project_ctx().await;
        // Generate output larger than default limit
        let result = ExecuteShellCommand
            .call(
                json!({ "command": "seq 1 100000", "timeout_secs": 10 }),
                &ctx,
            )
            .await
            .unwrap();
        // Output should be truncated (seq 1 100000 produces ~588KB)
        assert_eq!(
            result["stdout_truncated"], true,
            "large output should be truncated"
        );
        assert!(result["stdout_total_bytes"].as_u64().unwrap() > 100 * 1024);
        // The truncated output should still be valid
        assert!(result["stdout"].as_str().unwrap().contains("truncated"));
    }

    #[tokio::test]
    async fn execute_shell_command_small_output_not_truncated() {
        let (_dir, ctx) = project_ctx().await;
        let result = ExecuteShellCommand
            .call(json!({ "command": "echo hello", "timeout_secs": 5 }), &ctx)
            .await
            .unwrap();
        assert_eq!(result["stdout_truncated"], serde_json::Value::Null);
        assert!(result["stdout"].as_str().unwrap().contains("hello"));
    }

    #[tokio::test]
    async fn execute_shell_command_warn_mode_includes_warning() {
        let (_dir, ctx) = project_ctx().await;
        let result = ExecuteShellCommand
            .call(json!({ "command": "echo test", "timeout_secs": 5 }), &ctx)
            .await
            .unwrap();
        assert!(
            result["warning"].as_str().is_some(),
            "warn mode should include warning"
        );
    }

    #[tokio::test]
    async fn execute_shell_command_exit_code_preserved() {
        let (_dir, ctx) = project_ctx().await;
        let result = ExecuteShellCommand
            .call(json!({ "command": "exit 42", "timeout_secs": 5 }), &ctx)
            .await
            .unwrap();
        assert_eq!(result["exit_code"], 42);
    }

    #[tokio::test]
    async fn execute_shell_command_echo_cross_platform() {
        let (_dir, ctx) = project_ctx().await;
        // "echo hello" works on both sh and cmd.exe
        let result = ExecuteShellCommand
            .call(json!({ "command": "echo hello", "timeout_secs": 5 }), &ctx)
            .await
            .unwrap();
        let stdout = result["stdout"].as_str().unwrap();
        assert!(
            stdout.contains("hello"),
            "stdout should contain 'hello': {}",
            stdout
        );
    }
}
