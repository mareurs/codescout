//! Output formatting and buffering helpers for run_command.

use serde_json::{json, Value};

use super::super::ToolContext;
use super::inner::TmpfileGuard;

/// Reassemble a buffered command summary with a stable, reader-friendly field order.
///
/// Dynamic field appending (`obj["key"] = val`) always places fields last, which
/// caused `output_id` (the buffer reference) to land after `stdout`/`failures`/
/// `first_error` (the bulk content). Correct order:
///   type → exit_code → output_id → [counts] → [content]
pub(crate) fn rebuild_buffered_summary(raw: Value, output_id: &str) -> Value {
    // These are large text fields — always go last.
    const CONTENT_FIELDS: &[&str] = &["stdout", "failures", "first_error"];

    let mut map = serde_json::Map::new();

    // 1. Status identity
    if let Some(v) = raw.get("type") {
        map.insert("type".into(), v.clone());
    }
    if let Some(v) = raw.get("exit_code") {
        map.insert("exit_code".into(), v.clone());
    }

    // 2. Buffer reference — most action-relevant, agent needs this to query results
    map.insert("output_id".into(), json!(output_id));

    // 3. Type-specific compact fields (counts, not content)
    let raw_obj = raw.as_object().expect("summary is always an object");
    for (k, v) in raw_obj {
        if !["type", "exit_code"].contains(&k.as_str()) && !CONTENT_FIELDS.contains(&k.as_str()) {
            map.insert(k.clone(), v.clone());
        }
    }

    // 4. Content fields last — bulk payload
    for field in CONTENT_FIELDS {
        if let Some(v) = raw_obj.get(*field) {
            map.insert((*field).into(), v.clone());
        }
    }

    Value::Object(map)
}

pub(crate) async fn handle_successful_output(
    original_command: &str,
    raw_stdout: String,
    raw_stderr: String,
    exit_code: i32,
    buffer_only: bool,
    unfiltered_tmpfile: Option<TmpfileGuard>,
    ctx: &ToolContext,
) -> anyhow::Result<Value> {
    use super::super::command_summary::{
        count_lines, detect_command_type, needs_summary, strip_ansi_codes, summarize_build_output,
        summarize_generic, summarize_test_output, truncate_lines, truncate_lines_and_bytes,
        CommandType, BUFFER_QUERY_INLINE_CAP,
    };

    // Buffer-only queries strip ANSI codes — they are opaque to LLMs and bloat byte counts.
    let raw_stdout = if buffer_only {
        strip_ansi_codes(&raw_stdout)
    } else {
        raw_stdout
    };
    let raw_stderr = if buffer_only {
        strip_ansi_codes(&raw_stderr)
    } else {
        raw_stderr
    };

    // --- Step 6.5: Read tee capture and store as unfiltered_output ref ---
    let unfiltered_ref: Option<(String, bool)> = if let Some(ref tmpfile) = unfiltered_tmpfile {
        let capture = std::fs::read_to_string(&tmpfile.0).ok();
        // tmpfile drops at function exit — TmpfileGuard::drop() removes the file.
        // Skip empty captures: when the terminal filter matched nothing, both
        // raw_stdout and the tee file are empty — surfacing a handle is misleading.
        capture.and_then(|content| {
            if content.is_empty() {
                return None;
            }
            let (stored, truncated) = if crate::tools::exceeds_inline_limit(&content) {
                let mut byte_budget = crate::tools::MAX_INLINE_TOKENS * 4;
                let capped: String = content
                    .lines()
                    .take_while(|line| {
                        if byte_budget == 0 {
                            return false;
                        }
                        byte_budget = byte_budget.saturating_sub(line.len() + 1);
                        true
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                (capped, true)
            } else {
                (content, false)
            };
            let ref_id = ctx.output_buffer.store(
                original_command.to_string(),
                stored,
                String::new(), // unfiltered capture is stdout-only
                exit_code,
            );
            Some((ref_id, truncated))
        })
    } else {
        None
    };

    // --- Step 6: Decide whether to buffer + summarize ---
    let mut result = if needs_summary(&raw_stdout, &raw_stderr) {
        if buffer_only {
            // Buffer-only: return inline, never create a new buffer ref (avoids infinite loop).
            const STDERR_BUDGET: usize = 20;
            let buffer_stderr: String = if raw_stderr.is_empty() {
                original_command
                    .find("@cmd_")
                    .or_else(|| original_command.find("@file_"))
                    .and_then(|pos| {
                        original_command[pos..]
                            .split_whitespace()
                            .next()
                            .and_then(|tok| ctx.output_buffer.get(tok))
                    })
                    .map(|e| e.stderr)
                    .unwrap_or_default()
            } else {
                raw_stderr.clone()
            };
            let stderr_budget = STDERR_BUDGET.min(count_lines(&buffer_stderr));
            let stdout_budget = BUFFER_QUERY_INLINE_CAP - stderr_budget;

            let (stderr_out, stderr_shown, stderr_total) =
                truncate_lines(&buffer_stderr, STDERR_BUDGET);

            // Byte budget: keep final JSON under TOOL_OUTPUT_BUFFER_THRESHOLD to avoid re-buffering loop.
            const JSON_OVERHEAD: usize = 300;
            let stdout_byte_budget = crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD
                .saturating_sub(JSON_OVERHEAD)
                .saturating_sub(stderr_out.len());

            let (stdout_out, stdout_shown, stdout_total) =
                truncate_lines_and_bytes(&raw_stdout, stdout_budget, stdout_byte_budget);

            let was_truncated = stdout_shown < stdout_total || stderr_shown < stderr_total;

            let mut result = json!({"exit_code": exit_code});
            if !stdout_out.is_empty() {
                result["stdout"] = json!(stdout_out);
            }
            if !stderr_out.is_empty() {
                result["stderr"] = json!(stderr_out);
            }
            if was_truncated {
                result["truncated"] = json!(true);
                result["stdout_shown"] = json!(stdout_shown);
                result["stdout_total"] = json!(stdout_total);
                if stderr_total > 0 {
                    result["stderr_shown"] = json!(stderr_shown);
                    result["stderr_total"] = json!(stderr_total);
                }
                let stderr_note = if stderr_total > 0 {
                    format!(", stderr {stderr_shown}/{stderr_total}")
                } else {
                    String::new()
                };
                let next_start = stdout_shown + 1;
                let next_end = stdout_shown + BUFFER_QUERY_INLINE_CAP;
                result["hint"] = json!(format!(
                    "Output capped at {BUFFER_QUERY_INLINE_CAP} lines \
                     (stdout {stdout_shown}/{stdout_total}{stderr_note}). \
                     Next page: sed -n '{next_start},{next_end}p' @ref. \
                     Or grep 'keyword' @ref for targeted search.",
                ));
            }
            // buffer_only => tee injection was skipped (unfiltered_tmpfile is None).
            return Ok(result);
        }

        let output_id = ctx.output_buffer.store(
            original_command.to_string(),
            raw_stdout.clone(),
            raw_stderr.clone(),
            exit_code,
        );

        let cmd_type = detect_command_type(original_command);
        let cmd_summary = match cmd_type {
            CommandType::Test => summarize_test_output(&raw_stdout, &raw_stderr, exit_code),
            CommandType::Build => summarize_build_output(&raw_stdout, &raw_stderr, exit_code),
            CommandType::Generic => summarize_generic(&raw_stdout, &raw_stderr, exit_code),
        };

        // Rebuild with correct field order so output_id appears before content fields.
        rebuild_buffered_summary(cmd_summary, &output_id)
    } else {
        // Short output — apply byte budget for buffer-only to prevent re-buffering loop.
        if buffer_only
            && raw_stdout.len() + raw_stderr.len()
                > crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD.saturating_sub(300)
        {
            const JSON_OVERHEAD: usize = 300;
            let byte_budget = crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD
                .saturating_sub(JSON_OVERHEAD)
                .saturating_sub(raw_stderr.len());
            let (stdout_out, stdout_shown, stdout_total) =
                truncate_lines_and_bytes(&raw_stdout, BUFFER_QUERY_INLINE_CAP, byte_budget);
            let mut r = json!({"exit_code": exit_code});
            if !stdout_out.is_empty() {
                r["stdout"] = json!(stdout_out);
            }
            if !raw_stderr.is_empty() {
                r["stderr"] = json!(raw_stderr);
            }
            if stdout_shown < stdout_total {
                r["truncated"] = json!(true);
                r["hint"] = json!(
                    "Match truncated: a single grep match inside a @tool_* ref \
                     contains compact JSON (one very long line). \
                     Use read_file(@tool_abc, json_path=\"$.field\") to extract \
                     a specific field, or read_file(@tool_abc, start_line=N, \
                     end_line=M) to browse sections of the pretty-printed result."
                );
            }
            r
        } else {
            let mut r = json!({"exit_code": exit_code});
            if !raw_stdout.is_empty() {
                r["stdout"] = json!(raw_stdout);
            }
            if !raw_stderr.is_empty() {
                r["stderr"] = json!(raw_stderr);
            }
            r
        }
    };

    // Attach unfiltered_output ref if we captured via tee.
    if let Some((ref ref_id, truncated)) = unfiltered_ref {
        result["unfiltered_output"] = json!(ref_id);
        if truncated {
            result["unfiltered_truncated"] = json!(true);
        }
    }

    Ok(result)
}

/// Format a compact one-liner summary of a run_command result for `format_compact`.
pub(crate) fn format_run_command(result: &Value) -> String {
    let mut s = if result["output_id"].is_string() {
        let exit = result["exit_code"].as_i64().unwrap_or(0);
        let check = if exit == 0 { "✓" } else { "✗" };
        let output_id = result["output_id"].as_str().unwrap_or("");
        match result["type"].as_str() {
            Some("test") => {
                let passed = result["passed"].as_u64().unwrap_or(0);
                let failed = result["failed"].as_u64().unwrap_or(0);
                let ignored = result["ignored"].as_u64().unwrap_or(0);
                let mut s = format!("{check} exit {exit} · {passed} passed");
                if failed > 0 {
                    s.push_str(&format!(" · {failed} FAILED"));
                }
                if ignored > 0 {
                    s.push_str(&format!(" · {ignored} ignored"));
                }
                s.push_str(&format!("  (query {output_id})"));
                s
            }
            Some("build") => {
                let errors = result["errors"].as_u64().unwrap_or(0);
                if errors > 0 {
                    format!("{check} exit {exit} · {errors} errors  (query {output_id})")
                } else {
                    format!("{check} exit {exit}  (query {output_id})")
                }
            }
            _ => format!("{check} exit {exit}  (query {output_id})"),
        }
    } else if result["timed_out"].as_bool().unwrap_or(false) {
        "✗ timed out".to_string()
    } else {
        let exit = result["exit_code"].as_i64().unwrap_or(0);
        let stdout_lines = result["stdout"]
            .as_str()
            .map(|s| s.lines().count())
            .unwrap_or(0);
        let check = if exit == 0 { "✓" } else { "✗" };
        format!("{check} exit {exit} · {stdout_lines} lines")
    };

    // Append timeout hint after all branch logic so it covers every output shape.
    if let Some(hint) = result["timeout_hint"].as_str() {
        s.push_str(&format!("\n⚠ timeout: {hint}"));
    }

    s
}
