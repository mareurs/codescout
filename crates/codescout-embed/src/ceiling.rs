//! The effective per-request input ceiling of an embedding server, and the check that
//! a segment budget fits under it.
//!
//! Two limits bind a llama.cpp-style `/v1/embeddings` server, and the smaller one wins:
//!
//! - the **per-slot context** — `--ctx-size / --parallel`, reported as `n_ctx` per entry of
//!   `GET /slots`;
//! - the **model's training context** — `n_ctx_train`, reported under `meta` of
//!   `GET /v1/models`. No server flag lifts it
//!   (`docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md`
//!   § *The ceiling is the model's, not the config's*).
//!
//! `/props` is deliberately not used: on the embedding-only llama-server this was measured
//! against (2026-10-16) it answers HTTP 500 `vector::_M_range_check`, while `/slots` and
//! `/v1/models` both answer with the numbers needed.
//!
//! This module is pure — no I/O — so the parsing and the verdict are testable without a
//! server. The probe that fetches the two documents is
//! [`crate::remote::probe_embed_ceiling`].

use serde_json::Value;

/// What a server reported about the largest input it will accept, in tokens.
///
/// Each field is `None` when its endpoint was absent, unreachable, or answered in a shape
/// this module does not recognise — an OpenAI or Ollama endpoint answers neither, and that is
/// "unknown", not "no ceiling".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EmbedCeiling {
    /// `n_ctx` of one slot: the most tokens a single request can occupy.
    pub per_slot_ctx: Option<usize>,
    /// The model's own training context; a hard limit whatever the slot size.
    pub n_ctx_train: Option<usize>,
}

impl EmbedCeiling {
    /// The binding limit: the smaller of whichever of the two is known.
    pub fn effective_tokens(&self) -> Option<usize> {
        match (self.per_slot_ctx, self.n_ctx_train) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}

/// The verdict of [`judge`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CeilingVerdict {
    /// The server reported nothing usable, so nothing can be said either way.
    Unknown,
    /// Every segment of `budget_chars` fits under the reported ceiling.
    Fits { ceiling_tokens: usize },
    /// A segment of `budget_chars` can exceed the ceiling: the server will refuse (or a
    /// truncating one will silently clip) the largest segments this budget produces.
    BudgetExceedsCeiling {
        budget_chars: usize,
        ceiling_tokens: usize,
        /// The largest segment, in chars, that the ceiling accepts at the budget's own
        /// chars-per-token rule.
        max_chars: usize,
    },
}

/// Chars a request of `tokens` tokens may carry — the one rule [`crate::chunk_size_for_model`]
/// sizes its budgets by (85 % of the context, 3 chars per token), shared so the check below
/// cannot disagree with the budget it checks.
pub fn budget_chars_for_tokens(tokens: usize) -> usize {
    (tokens as f64 * 0.85 * 3.0) as usize
}

/// Whether a segment budget of `budget_chars` fits under `ceiling`.
pub fn judge(ceiling: &EmbedCeiling, budget_chars: usize) -> CeilingVerdict {
    let Some(ceiling_tokens) = ceiling.effective_tokens() else {
        return CeilingVerdict::Unknown;
    };
    let max_chars = budget_chars_for_tokens(ceiling_tokens);
    if budget_chars > max_chars {
        CeilingVerdict::BudgetExceedsCeiling {
            budget_chars,
            ceiling_tokens,
            max_chars,
        }
    } else {
        CeilingVerdict::Fits { ceiling_tokens }
    }
}

/// The per-slot context from a `GET /slots` document: an array with one object per slot,
/// each carrying `n_ctx`. The slots of one server are configured alike, so the minimum is
/// reported — the conservative reading if they ever differ. `None` for any other shape, an
/// empty array, or a non-positive `n_ctx`.
pub fn parse_slots_n_ctx(doc: &Value) -> Option<usize> {
    doc.as_array()?
        .iter()
        .map(|slot| slot.get("n_ctx").and_then(Value::as_u64))
        .collect::<Option<Vec<u64>>>()?
        .into_iter()
        .filter(|&n| n > 0)
        .min()
        .map(|n| n as usize)
}

/// The model's training context from a `GET /v1/models` document:
/// `data[*].meta.n_ctx_train`. OpenAI's own `/v1/models` has no `meta`, so it yields `None`
/// rather than a made-up number. The minimum across entries, for the same reason as
/// [`parse_slots_n_ctx`].
pub fn parse_models_n_ctx_train(doc: &Value) -> Option<usize> {
    doc.get("data")?
        .as_array()?
        .iter()
        .filter_map(|m| m.get("meta")?.get("n_ctx_train")?.as_u64())
        .filter(|&n| n > 0)
        .min()
        .map(|n| n as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The shape `GET /slots` returned from the live llama-server on this host,
    /// 2026-10-16, trimmed to the field read (`n_ctx`) plus neighbours that must be ignored.
    fn live_slots() -> Value {
        json!([{"id": 0, "id_task": 429058, "n_ctx": 4096, "speculative": false,
                "is_processing": false, "params": {"n_predict": -1}}])
    }

    /// The shape `GET /v1/models` returned from the same server, trimmed likewise.
    fn live_models() -> Value {
        json!({"models": [{"name": "/models/CodeRankEmbed-Q4_K_M.gguf"}],
               "object": "list",
               "data": [{"id": "/models/CodeRankEmbed-Q4_K_M.gguf", "object": "model",
                         "meta": {"n_vocab": 30528, "n_ctx_train": 2048, "n_embd": 768}}]})
    }

    #[test]
    fn slots_document_yields_the_per_slot_context() {
        assert_eq!(parse_slots_n_ctx(&live_slots()), Some(4096));
    }

    #[test]
    fn slots_takes_the_minimum_when_slots_differ() {
        let doc = json!([{"n_ctx": 4096}, {"n_ctx": 1024}, {"n_ctx": 4096}]);
        assert_eq!(parse_slots_n_ctx(&doc), Some(1024));
    }

    #[test]
    fn slots_with_an_unreadable_member_is_unknown_not_partial() {
        // One slot without `n_ctx` means the document is not the shape this reads; reporting
        // the other slots' number would present a guess as the server's answer.
        let doc = json!([{"n_ctx": 4096}, {"id": 1}]);
        assert_eq!(parse_slots_n_ctx(&doc), None);
    }

    #[test]
    fn slots_rejects_every_other_shape() {
        for doc in [
            json!([]),
            json!({"error": {"code": 501}}),
            json!("nope"),
            json!([{"n_ctx": 0}]),
            json!([{"n_ctx": "4096"}]),
        ] {
            assert_eq!(parse_slots_n_ctx(&doc), None, "{doc}");
        }
    }

    #[test]
    fn models_document_yields_the_training_context() {
        assert_eq!(parse_models_n_ctx_train(&live_models()), Some(2048));
    }

    #[test]
    fn an_openai_style_models_list_has_no_ceiling_to_report() {
        let doc = json!({"object": "list",
                         "data": [{"id": "text-embedding-3-small", "object": "model",
                                   "owned_by": "openai"}]});
        assert_eq!(parse_models_n_ctx_train(&doc), None);
    }

    #[test]
    fn effective_ceiling_is_the_smaller_known_limit() {
        let both = EmbedCeiling {
            per_slot_ctx: Some(4096),
            n_ctx_train: Some(2048),
        };
        assert_eq!(both.effective_tokens(), Some(2048));
        let slot_binds = EmbedCeiling {
            per_slot_ctx: Some(1024),
            n_ctx_train: Some(2048),
        };
        assert_eq!(slot_binds.effective_tokens(), Some(1024));
        let only_train = EmbedCeiling {
            per_slot_ctx: None,
            n_ctx_train: Some(2048),
        };
        assert_eq!(only_train.effective_tokens(), Some(2048));
        assert_eq!(EmbedCeiling::default().effective_tokens(), None);
    }

    /// The CodeRankEmbed budget `chunk_size_for_model` hands out (2048 tokens).
    fn coderank_budget() -> usize {
        crate::chunk_size_for_model("CodeRankEmbed")
    }

    #[test]
    fn the_live_host_passes_its_own_budget() {
        // 4096 per slot, 2048 trained: the model table's 2048 is exactly right.
        let ceiling = EmbedCeiling {
            per_slot_ctx: parse_slots_n_ctx(&live_slots()),
            n_ctx_train: parse_models_n_ctx_train(&live_models()),
        };
        assert_eq!(
            judge(&ceiling, coderank_budget()),
            CeilingVerdict::Fits {
                ceiling_tokens: 2048
            }
        );
    }

    #[test]
    fn the_reporters_misconfiguration_is_caught_before_the_first_write() {
        // `--ctx-size 8192 --parallel 8` is 1024 tokens a slot, under the model's 2048: the
        // table's budget (5222 chars) overshoots what the slot accepts (2611).
        let ceiling = EmbedCeiling {
            per_slot_ctx: Some(1024),
            n_ctx_train: Some(2048),
        };
        assert_eq!(
            judge(&ceiling, coderank_budget()),
            CeilingVerdict::BudgetExceedsCeiling {
                budget_chars: coderank_budget(),
                ceiling_tokens: 1024,
                max_chars: 2611,
            }
        );
    }

    #[test]
    fn a_budget_exactly_at_the_ceiling_fits() {
        let ceiling = EmbedCeiling {
            per_slot_ctx: Some(2048),
            n_ctx_train: None,
        };
        let at = budget_chars_for_tokens(2048);
        assert_eq!(
            judge(&ceiling, at),
            CeilingVerdict::Fits {
                ceiling_tokens: 2048
            }
        );
        assert!(matches!(
            judge(&ceiling, at + 1),
            CeilingVerdict::BudgetExceedsCeiling { .. }
        ));
    }

    #[test]
    fn nothing_reported_is_unknown_never_fits() {
        assert_eq!(
            judge(&EmbedCeiling::default(), coderank_budget()),
            CeilingVerdict::Unknown
        );
    }

    #[test]
    fn the_budget_rule_is_the_one_chunk_size_for_model_uses() {
        // A model the table maps to 2048 tokens must come out at exactly the shared rule's
        // value; if the two formulas drift, `judge` would flag (or pass) budgets the sizer
        // never produced.
        assert_eq!(coderank_budget(), budget_chars_for_tokens(2048));
        assert_eq!(
            crate::chunk_size_for_model("nomic-embed-text"),
            budget_chars_for_tokens(8192)
        );
    }
}
