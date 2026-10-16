//! Startup report of the embedder's effective per-request ceiling.
//!
//! Segmentation (`26feb1aa`) sizes every oversized memory and artifact to
//! `chunk_size_for_model(model)` chars. That table holds the *model's* published context;
//! the server may accept less — `--ctx-size 8192 --parallel 8` is 1024 tokens a slot, half
//! of CodeRankEmbed's 2048. Until this report, such a misconfiguration was found at the
//! first oversized write rather than at connect
//! (`docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md`,
//! Fix step 3).
//!
//! **Advisory and fail-soft.** The probe ([`codescout_embed::remote::probe_embed_ceiling`])
//! has no error arm, the report runs in a detached task, and its only output is a log line.
//! A server that answers nothing — OpenAI, Ollama, an unreachable host — logs at debug
//! level and nothing else changes. Nothing here can fail or delay startup.

use codescout_embed::ceiling::{judge, CeilingVerdict, EmbedCeiling};

/// How loudly a report should be logged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportLevel {
    /// The server answered nothing usable; nothing to act on.
    Debug,
    /// The budget fits; recorded so the measured ceiling is on file.
    Info,
    /// The budget overshoots the ceiling; the operator should act.
    Warn,
}

/// One rendered report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeilingReport {
    pub level: ReportLevel,
    pub message: String,
}

/// Judge `ceiling` against the segment budget `model` is sized to, and say so.
///
/// Pure over its arguments so every arm is testable without a server.
pub fn assess(url: &str, model: &str, ceiling: &EmbedCeiling) -> CeilingReport {
    let budget_chars = codescout_embed::chunk_size_for_model(model);
    let limits = format!(
        "per-slot context {}, n_ctx_train {}",
        ceiling
            .per_slot_ctx
            .map_or("unreported".to_string(), |n| format!("{n} tokens")),
        ceiling
            .n_ctx_train
            .map_or("unreported".to_string(), |n| format!("{n} tokens")),
    );
    match judge(ceiling, budget_chars) {
        CeilingVerdict::Unknown => CeilingReport {
            level: ReportLevel::Debug,
            message: format!(
                "embed ceiling: {url} reported neither /slots nor /v1/models n_ctx_train \
                 (not a llama.cpp server, or unreachable); skipping the ceiling check"
            ),
        },
        CeilingVerdict::Fits { ceiling_tokens } => CeilingReport {
            level: ReportLevel::Info,
            message: format!(
                "embed ceiling: {url} accepts {ceiling_tokens} tokens per request ({limits}); \
                 the {budget_chars}-char segment budget for '{model}' fits"
            ),
        },
        CeilingVerdict::BudgetExceedsCeiling {
            ceiling_tokens,
            max_chars,
            ..
        } => {
            // Which limit binds decides which remedy exists: a slot can be enlarged, the
            // model's training context cannot.
            let slot_binds = matches!(
                (ceiling.per_slot_ctx, ceiling.n_ctx_train),
                (Some(s), Some(t)) if s < t
            ) || (ceiling.per_slot_ctx.is_some() && ceiling.n_ctx_train.is_none());
            let remedy = if slot_binds {
                "raise the server's --ctx-size or lower --parallel (per-slot context is \
                 ctx-size / parallel)"
            } else {
                "the model's own training context binds and no server flag lifts it; \
                 point [embeddings].model at a model whose segment budget fits, or \
                 correct the model name so the budget table matches the served model"
            };
            CeilingReport {
                level: ReportLevel::Warn,
                message: format!(
                    "embed ceiling: {url} accepts only {ceiling_tokens} tokens per request \
                     ({limits}), but the segment budget for '{model}' is {budget_chars} chars \
                     and the ceiling admits {max_chars}. Oversized memory and artifact embeds \
                     will be refused (HTTP 4xx/5xx) or silently truncated: {remedy}."
                ),
            }
        }
    }
}

/// Emit `report` at its own level.
pub fn log_report(report: &CeilingReport) {
    match report.level {
        ReportLevel::Debug => tracing::debug!("{}", report.message),
        ReportLevel::Info => tracing::info!("{}", report.message),
        ReportLevel::Warn => tracing::warn!("{}", report.message),
    }
}

/// Run `probe` for `url` and assess the result against `model`'s budget.
///
/// The probe is a parameter so a test supplies a canned answer and never opens a socket;
/// production passes [`codescout_embed::remote::probe_embed_ceiling`].
pub async fn probe_and_assess<F, Fut>(url: &str, model: &str, probe: F) -> CeilingReport
where
    F: FnOnce(String) -> Fut,
    Fut: std::future::Future<Output = EmbedCeiling>,
{
    let ceiling = probe(url.to_string()).await;
    assess(url, model, &ceiling)
}

/// Resolve the embedder config for `root` and, when it names an HTTP embedder, spawn a
/// detached task that probes it and logs the verdict.
///
/// Returns immediately; never errors. No url means the model names an in-process or
/// provider backend (`local:`, `ollama:`, `openai:`) with no llama.cpp server to ask — and
/// probing a paid endpoint is exactly what this must not do — so nothing is spawned.
#[cfg(feature = "remote-embed")]
pub fn spawn_startup_report(root: Option<&std::path::Path>) {
    let config = match crate::retrieval::config::RetrievalConfig::from_env_and_project(root) {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("embed ceiling: config unresolved, skipping the check: {e}");
            return;
        }
    };
    let Some(url) = config.embedder_url else {
        return;
    };
    let model = config.model;
    tokio::spawn(async move {
        let report = probe_and_assess(&url, &model, |u| async move {
            codescout_embed::remote::probe_embed_ceiling(&u).await
        })
        .await;
        log_report(&report);
    });
}

/// Without `remote-embed` there is no HTTP embedder to probe.
#[cfg(not(feature = "remote-embed"))]
pub fn spawn_startup_report(_root: Option<&std::path::Path>) {}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "http://127.0.0.1:48081";

    fn ceiling(slot: Option<usize>, train: Option<usize>) -> EmbedCeiling {
        EmbedCeiling {
            per_slot_ctx: slot,
            n_ctx_train: train,
        }
    }

    #[test]
    fn the_live_hosts_numbers_report_fits_at_info() {
        let r = assess(URL, "CodeRankEmbed", &ceiling(Some(4096), Some(2048)));
        assert_eq!(r.level, ReportLevel::Info);
        assert!(r.message.contains("accepts 2048 tokens"), "{}", r.message);
        assert!(r.message.contains("5222-char"), "{}", r.message);
    }

    #[test]
    fn a_small_slot_warns_and_names_the_slot_remedy() {
        // The reporter's `--ctx-size 8192 --parallel 8`: 1024 a slot under a 2048 model.
        let r = assess(URL, "CodeRankEmbed", &ceiling(Some(1024), Some(2048)));
        assert_eq!(r.level, ReportLevel::Warn);
        assert!(r.message.contains("1024 tokens"), "{}", r.message);
        assert!(r.message.contains("2611"), "{}", r.message);
        assert!(r.message.contains("--parallel"), "{}", r.message);
        assert!(!r.message.contains("no server flag"), "{}", r.message);
    }

    #[test]
    fn a_model_limit_below_the_table_warns_without_offering_a_flag_that_cannot_help() {
        // Slot is roomy, the model itself is smaller than the table believes: raising
        // --ctx-size would be a remedy that does nothing.
        let r = assess(URL, "CodeRankEmbed", &ceiling(Some(8192), Some(512)));
        assert_eq!(r.level, ReportLevel::Warn);
        assert!(r.message.contains("no server flag"), "{}", r.message);
        assert!(!r.message.contains("--parallel"), "{}", r.message);
    }

    #[test]
    fn a_silent_server_is_debug_only() {
        let r = assess(URL, "CodeRankEmbed", &EmbedCeiling::default());
        assert_eq!(r.level, ReportLevel::Debug);
    }

    #[tokio::test]
    async fn the_probe_is_called_with_the_configured_url_and_its_answer_is_judged() {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
        let seen_in = std::sync::Arc::clone(&seen);
        let r = probe_and_assess(URL, "CodeRankEmbed", move |u| async move {
            *seen_in.lock().unwrap() = Some(u);
            ceiling(Some(1024), Some(2048))
        })
        .await;
        assert_eq!(seen.lock().unwrap().as_deref(), Some(URL));
        assert_eq!(r.level, ReportLevel::Warn);
    }
}
