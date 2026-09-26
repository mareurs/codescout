//! Which post-phase engine attached what to one tool response — record-only.
//!
//! `usage.db`'s `deliveries_json` column is written from here, and it has three
//! states. The column exists for the difference between the first two:
//!
//! - **NULL** — the fan-out never ran for this call: the tool returned `Err`
//!   before `Tool::call_content` reached `coordinator::run_post`, or the tool
//!   overrides `call_content` without calling it (`onboarding` does), or the row
//!   predates the column.
//! - **`[]`** — the fan-out ran and no engine contributed anything.
//! - **`[{…}, …]`** — one [`DeliveryRecord`] per engine that contributed, in
//!   registry order.
//!
//! Nothing reads this back into behaviour, and it must not change a byte of the
//! response. `call_tool_inner_records_opener_then_empty_deliveries`
//! (`src/server.rs`) checks every recorded digest against the blocks the call
//! actually returned.

use rmcp::model::Content;
use std::cell::RefCell;

/// One engine's contribution to one response.
///
/// Written only for an engine that CLAIMED and contributed something — a block,
/// a ledger key, or the kept hint. A claim that shipped nothing is not recorded:
/// `operator-rules` claims on every call that has a selector, so recording empty
/// claims would make `[]` unreachable and erase the one distinction this column
/// draws. See `coordinator::run_post_in`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(crate) struct DeliveryRecord {
    /// `EngineDecl::id`.
    pub engine: &'static str,
    /// Ledger keys this engine's `emit` ADDED: the set difference of
    /// `GuideLedger::key_set` across the call. A repeat insert that only
    /// refreshes an existing key's stamp is not counted.
    pub ledger_keys: Vec<String>,
    /// One digest per block this engine appended, in order.
    pub blocks: Vec<BlockDigest>,
    /// True only when this engine's hint is the one `run_post_in` kept for the
    /// primary block's single `_guide_hint` field. A hint that lost to an
    /// earlier engine's is `false`.
    pub hint: bool,
}

/// A block's identity without its bytes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(crate) struct BlockDigest {
    /// Lowercase hex SHA-256 over the block's text bytes.
    pub sha256: String,
    /// The length of those bytes.
    pub bytes: usize,
}

impl BlockDigest {
    /// Digest one appended block.
    ///
    /// Every emitter builds `Content::text`, so the text arm is the only live one.
    /// A non-text block digests its JSON serialisation instead, which keeps it
    /// identifiable rather than hashing an empty string under the same field name.
    pub(crate) fn of(block: &Content) -> Self {
        match block.as_text() {
            Some(t) => Self::of_bytes(t.text.as_bytes()),
            None => Self::of_bytes(&serde_json::to_vec(block).unwrap_or_default()),
        }
    }

    fn of_bytes(bytes: &[u8]) -> Self {
        use sha2::Digest as _;
        Self {
            sha256: hex::encode(sha2::Sha256::digest(bytes)),
            bytes: bytes.len(),
        }
    }
}

tokio::task_local! {
    /// The sink [`record`] writes and `UsageRecorder::record_content` drains,
    /// scoped around exactly one tool call's future.
    ///
    /// Inside a scope, `None` means nothing was recorded, i.e. the fan-out never
    /// ran; it becomes the column's NULL. `Some(vec![])` means the fan-out ran and
    /// delivered nothing, and becomes `[]`.
    ///
    /// Deliberately NOT a `ToolContext` field, for the reason `PEER_SERVE_DISPATCH`
    /// (`src/tools/core/types.rs`) records: `ToolContext` is built by bare struct
    /// literal, with no `..Default::default()`, at well over a hundred sites, so a
    /// new required field ripples into every tool test in the tree. Re-derive the
    /// population with `git grep -nw 'ToolContext {' HEAD -- '*.rs'`, which gave
    /// 248 lines in 69 files at `decfd71f`. That is an upper bound, not a count of
    /// literals: the pattern also matches the `struct`/`impl` headers and
    /// `librarian::tools::ToolContext`, a different type.
    ///
    /// Like that precedent, it holds only while no `tokio::spawn` separates
    /// `record_content` from `call_content`'s fan-out. A spawn there would make
    /// [`record`] a silent no-op and every row NULL.
    pub(crate) static DELIVERY_SINK: RefCell<Option<Vec<DeliveryRecord>>>;
}

/// Replace the sink's value with `Some(records)`.
///
/// Replaces, never appends: if a tool's call nests another `call_content`, the
/// inner fan-out records first and the outer one last, and the outer one is the
/// response the caller receives. Outside a scope it does nothing, because a test
/// calling `call_content` directly must not panic.
pub(crate) fn record(records: Vec<DeliveryRecord>) {
    let _ = DELIVERY_SINK.try_with(|sink| *sink.borrow_mut() = Some(records));
}

/// The sink's value as the `deliveries_json` column: `None` gives NULL, and
/// `Some(vec![])` gives `"[]"`.
pub(crate) fn to_column(v: &Option<Vec<DeliveryRecord>>) -> Option<String> {
    // `.ok()` cannot turn a delivery into a NULL: these types hold only strings,
    // integers and bools, with no maps, so serde_json has no error to return.
    v.as_ref()
        .and_then(|records| serde_json::to_string(records).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(engine: &'static str) -> DeliveryRecord {
        DeliveryRecord {
            engine,
            ledger_keys: vec![format!("{engine}-key")],
            blocks: vec![BlockDigest {
                sha256: "00".to_string(),
                bytes: 1,
            }],
            hint: false,
        }
    }

    fn drain() -> Option<Vec<DeliveryRecord>> {
        DELIVERY_SINK.with(|s| s.borrow_mut().take())
    }

    /// "Does nothing" has two halves, and each assertion here holds one. It must
    /// not PANIC: a bare `DELIVERY_SINK.with` would, and every test that calls
    /// `call_content` without `record_content` around it runs this path. And it
    /// must not reach a LATER scope, which a process-global sink would.
    #[test]
    fn record_outside_a_scope_is_a_no_op() {
        record(vec![fixture("outside")]);
        let later = DELIVERY_SINK.sync_scope(RefCell::new(None), drain);
        assert_eq!(later, None, "a record made outside every scope leaked");

        // POSITIVE CONTROL. Without it the `None` above is also what a `record`
        // that never writes anything would produce.
        let inside = DELIVERY_SINK.sync_scope(RefCell::new(None), || {
            record(vec![fixture("inside")]);
            drain()
        });
        assert_eq!(inside, Some(vec![fixture("inside")]));
    }

    /// Replace, not append and not first-wins: the outer call is the response the
    /// caller receives. The second case is the one an "only replace with a
    /// non-empty value" variant would survive, because an outer fan-out that
    /// delivered nothing must still overwrite what an inner one recorded.
    #[test]
    fn the_last_record_in_a_scope_wins() {
        let got = DELIVERY_SINK.sync_scope(RefCell::new(None), || {
            record(vec![fixture("inner")]);
            record(vec![fixture("outer")]);
            drain()
        });
        assert_eq!(got, Some(vec![fixture("outer")]));

        let got = DELIVERY_SINK.sync_scope(RefCell::new(None), || {
            record(vec![fixture("inner")]);
            record(Vec::new());
            drain()
        });
        assert_eq!(got, Some(Vec::new()));
    }

    #[test]
    fn to_column_distinguishes_never_ran_from_delivered_nothing() {
        assert_eq!(to_column(&None), None, "never ran must be NULL");
        assert_eq!(
            to_column(&Some(Vec::new())).as_deref(),
            Some("[]"),
            "ran and delivered nothing must be \"[]\", never NULL"
        );
        // The field names are the contract a query reads, so pin the whole shape.
        let one = DeliveryRecord {
            engine: "e",
            ledger_keys: vec!["k1".to_string()],
            blocks: vec![BlockDigest {
                sha256: "ab".to_string(),
                bytes: 2,
            }],
            hint: true,
        };
        assert_eq!(
            to_column(&Some(vec![one])).as_deref(),
            Some(
                r#"[{"engine":"e","ledger_keys":["k1"],"blocks":[{"sha256":"ab","bytes":2}],"hint":true}]"#
            )
        );
    }
}
