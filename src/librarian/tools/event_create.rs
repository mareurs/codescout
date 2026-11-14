use anyhow::Result;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{LibrarianRecoverableError, ToolContext};
use crate::librarian::catalog::dual_write::SectionRollback;
use crate::librarian::catalog::event_text::{splice_event, EventText};
use crate::librarian::catalog::{event_edges, events, sources};

fn any_value_schema(_g: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({})
}

fn source_schema(_g: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": ["object", "null"],
        "properties": {
            "uri": {"type": "string"},
            "kind": {"type": "string"},
            "payload": {}
        },
        "required": ["uri", "kind"],
        "additionalProperties": false
    })
}

/// **`deny_unknown_fields` is safe HERE while being unavailable on the shared `doc`
/// schema — the difference is what reaches this type.** `crate::tools::param_probe`'s
/// module doc records the attribute being tried once and breaking every `doc(update)`
/// call, because that dispatcher passes the shared argument blob straight down, so
/// `action` and every sibling action's key arrive as unknown fields. `flatten_event_args`
/// (`crate::librarian::tools::artifact`) does not: it builds a fresh map from the `event`
/// object's own keys plus `artifact_id`, so a sibling's key can never reach here.
/// `augment::Args` is reached the same way, through `flatten_augment_args`, and has
/// carried the attribute all along — this closes the asymmetry, it does not open a
/// new one.
///
/// Without it the `"additionalProperties": false` that `event` declares in that schema is
/// a promise serde never reads: `event={kind, payload, athor: "me"}` recorded the event
/// and discarded the typo, against a schema that told the caller it would be refused
/// (`IC-15`, accepted-parameter-silently-dropped).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Args {
    pub artifact_id: String,
    pub kind: String,
    #[schemars(schema_with = "any_value_schema")]
    pub payload: Value,
    #[serde(default)]
    pub anchor_commit: Option<String>,
    #[serde(default)]
    pub head_commit: Option<String>,
    #[serde(default)]
    pub parent_event_id: Option<String>,
    #[serde(default)]
    pub also_mutates: Option<Vec<String>>,
    #[serde(default)]
    pub resolves_intent_event_id: Option<String>,
    #[serde(default)]
    #[schemars(schema_with = "source_schema")]
    pub source: Option<SourceArg>,
    #[serde(default)]
    pub author: Option<String>,
}

/// Carries the attribute for the same reason as [`Args`], and is a SECOND guarded site
/// rather than a consequence of that one: `source` deserialises through this type's own
/// impl, which ignores unknown keys however strict its parent is.
///
/// The LIVE schema for this object is the hand-written `event.source` block in
/// `artifact.rs`, which says `"additionalProperties": false` to match — and
/// `every_schema_object_promising_to_refuse_unknown_keys_actually_refuses` pins that pair
/// together, in both directions. `source_schema` above is the schemars mirror feeding
/// `Args`'s derived `JsonSchema`, and is **INERT**: no `schema_for` call site exists in
/// this crate, so nothing materialises it, no test covers it, and it is kept in step by
/// hand. Do not read its agreement as coverage — do read it as the copy to update if that
/// derive is ever wired to a published surface.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SourceArg {
    pub uri: String,
    pub kind: String,
    #[serde(default)]
    #[schemars(schema_with = "any_value_schema")]
    pub payload: Option<Value>,
}

pub(crate) const ALLOWED_KINDS: &[&str] = &[
    "note",
    "reviewed",
    "status_change",
    "field_patch",
    "superseded_by",
    "external_signal",
    "intent",
    "verdict",
];

/// Per-artifact write lock registry.
///
/// `event_create::call` acquires a per-artifact-id lock that spans the
/// frontmatter mutation, parent-event lookup, and the row + edges
/// transaction. Two concurrent calls on the same artifact serialise
/// (preventing interleaved frontmatter writes and ensuring the
/// parent_event_id chain forms a line, not a fan). Calls on different
/// artifacts do not contend.
///
/// Memory grows linearly with the number of distinct artifact ids ever
/// seen in this process. Acceptable at v1 catalog sizes; revisit if the
/// catalog grows past tens of thousands of artifacts in long-lived
/// servers.
#[derive(Default)]
struct WriteLockRegistry {
    inner: parking_lot::Mutex<
        std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>,
    >,
}

impl WriteLockRegistry {
    fn lock_for(&self, artifact_id: &str) -> std::sync::Arc<tokio::sync::Mutex<()>> {
        let mut g = self.inner.lock();
        g.entry(artifact_id.to_string())
            .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }
}

static WRITE_LOCKS: std::sync::OnceLock<WriteLockRegistry> = std::sync::OnceLock::new();

fn write_locks() -> &'static WriteLockRegistry {
    WRITE_LOCKS.get_or_init(WriteLockRegistry::default)
}

/// Payload fields each event `kind` requires, as [`validate_payload`] enforces them.
///
/// Exists so `artifact_event`'s `payload` schema can *say* what it requires. That
/// description read only "create: event payload (a JSON object)" and named none of the
/// nine fields below, which is the same defect as `edit_code.body` and `artifact.patch`
/// — a param required only for certain values of another param, advertised as though it
/// were free-form. Here it is nested one level deeper: the requirement is on
/// `payload.<field>` conditional on `kind`, which flat JSON Schema `required` cannot
/// express at all.
///
/// Measured: `artifact_event` ran a 50% error rate over the 2026-07 window, and every
/// failure was a missing per-kind payload field. Those errors carry no `err_family`, so
/// the family-based sweep that found the sibling instances could not see them — TU-9 in
/// `docs/trackers/2026-08-15-tool-usage-investigation.md` found them by a different route
/// and asked for exactly this.
///
/// `validate_payload` is deliberately NOT rewritten to read this table: `field_patch.to`
/// accepts any JSON value while every other field must be a string, and collapsing that
/// distinction would be a silent behaviour change. Agreement between the two is proved by
/// `every_required_payload_field_is_enforced_and_advertised`, which executes the validator
/// rather than trusting the table.
///
/// See `docs/issues/archive/2026-08-15-conditionally-required-params-advertised-optional.md`.
pub const REQUIRED_PAYLOAD_FIELDS: &[(&str, &[&str])] = &[
    ("note", &["text"]),
    ("reviewed", &[]),
    ("status_change", &["to"]),
    ("field_patch", &["field", "to"]),
    ("superseded_by", &["target_artifact_id"]),
    ("external_signal", &["source_id", "summary"]),
    ("intent", &["hypothesis"]),
    ("verdict", &["outcome"]),
];

/// Render [`REQUIRED_PAYLOAD_FIELDS`] as the requirement clause for the `payload` schema.
///
/// Generated rather than hand-written so a new event kind cannot be added to the validator
/// and silently left out of what the tool advertises.
pub fn payload_requirements_sentence() -> String {
    let with_required = REQUIRED_PAYLOAD_FIELDS
        .iter()
        .filter(|(_, fields)| !fields.is_empty())
        .map(|(kind, fields)| format!("{kind}→{}", fields.join("+")))
        .collect::<Vec<_>>()
        .join("; ");
    let no_required = REQUIRED_PAYLOAD_FIELDS
        .iter()
        .filter(|(_, fields)| fields.is_empty())
        .map(|(kind, _)| *kind)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "REQUIRED keys depend on `kind`: {with_required}. \
         No required keys: {no_required}. \
         Omitting one is refused with `<kind>.<field> required`."
    )
}

pub(crate) fn validate_payload(kind: &str, p: &Value) -> Result<()> {
    let obj = p
        .as_object()
        .ok_or_else(|| LibrarianRecoverableError::new("payload must be object"))?;
    match kind {
        "note" => {
            obj.get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| LibrarianRecoverableError::new("note.text required"))?;
        }
        "reviewed" => { /* both fields optional */ }
        "status_change" => {
            obj.get("to")
                .and_then(|v| v.as_str())
                .ok_or_else(|| LibrarianRecoverableError::new("status_change.to required"))?;
        }
        "field_patch" => {
            obj.get("field")
                .and_then(|v| v.as_str())
                .ok_or_else(|| LibrarianRecoverableError::new("field_patch.field required"))?;
            obj.get("to")
                .ok_or_else(|| LibrarianRecoverableError::new("field_patch.to required"))?;
        }
        "superseded_by" => {
            obj.get("target_artifact_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    LibrarianRecoverableError::new("superseded_by.target_artifact_id required")
                })?;
        }
        "external_signal" => {
            obj.get("source_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    LibrarianRecoverableError::new("external_signal.source_id required")
                })?;
            obj.get("summary").and_then(|v| v.as_str()).ok_or_else(|| {
                LibrarianRecoverableError::new("external_signal.summary required")
            })?;
        }
        "intent" => {
            obj.get("hypothesis")
                .and_then(|v| v.as_str())
                .ok_or_else(|| LibrarianRecoverableError::new("intent.hypothesis required"))?;
        }
        "verdict" => {
            let outcome = obj
                .get("outcome")
                .and_then(|v| v.as_str())
                .ok_or_else(|| LibrarianRecoverableError::new("verdict.outcome required"))?;
            if !matches!(outcome, "confirmed" | "refuted" | "partial" | "abandoned") {
                return Err(LibrarianRecoverableError::new(
                    "verdict.outcome must be confirmed|refuted|partial|abandoned",
                ));
            }
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn apply_payload_to_frontmatter(
    ctx: &ToolContext,
    artifact_id: &str,
    kind: &str,
    payload: &Value,
) -> Result<()> {
    match kind {
        "status_change" => {
            let to = payload.get("to").and_then(|v| v.as_str()).unwrap(); // already validated above
            crate::librarian::tools::update::write_field_to_frontmatter(
                ctx,
                artifact_id,
                "status",
                &Value::String(to.into()),
            )?;
        }
        "field_patch" => {
            let field = payload.get("field").and_then(|v| v.as_str()).unwrap(); // already validated above
            let to = payload.get("to").unwrap(); // already validated above
            crate::librarian::tools::update::write_field_to_frontmatter(
                ctx,
                artifact_id,
                field,
                to,
            )?;
        }
        _ => {}
    }
    Ok(())
}
// clippy 1.96 (Windows toolchain) false-positives `missing_const_for_thread_local`
// here even though the initializer already uses `const { ... }`. Scoped allow.
#[allow(clippy::missing_const_for_thread_local)]
/// Generate a monotonic ULID — guarantees strict-monotonic ordering even when
/// multiple events are created within the same millisecond. Without this,
/// `ulid::Ulid::new()` uses fresh random bits per ID, and two IDs sharing a
/// timestamp millisecond would have an unpredictable relative sort order.
/// The `Generator` increments the random portion when a same-ms collision
/// would occur, so `id_n > id_{n-1}` always holds.
fn next_monotonic_id() -> String {
    use std::cell::RefCell;
    thread_local! {
        static GENERATOR: RefCell<ulid::Generator> = const { RefCell::new(ulid::Generator::new()) };
    }
    GENERATOR.with(|g| {
        g.borrow_mut()
            .generate()
            .expect("ulid monotonic overflow within single ms — astronomically unlikely")
            .to_string()
    })
}

/// Argument parse for `event_create` (on the flattened args): pure, no catalog or filesystem
/// access — deserialisation plus the kind and payload checks. `call` runs it first, and
/// [`preflight`] runs it alone so the worktree write guard can sit behind it.
fn parse_args(args: Value) -> Result<Args> {
    let a: Args = serde_json::from_value(args).map_err(|e| {
        super::deser_error(
            e,
            "event_create",
            "doc(action=\"event_create\") requires 'id', 'event.kind' and 'event.payload'",
            "e.g. doc(action=\"event_create\", id=\"<16-hex>\", event={kind: \"note\", payload: {\"text\": \"...\"}}). Required payload keys depend on kind: note->text, status_change->to, field_patch->field+to, verdict->outcome.",
        )
    })?;

    if !ALLOWED_KINDS.contains(&a.kind.as_str()) {
        return Err(LibrarianRecoverableError::with_hint(
            format!("unknown event kind: {}", a.kind),
            format!("allowed: {}", ALLOWED_KINDS.join(", ")),
        ));
    }
    validate_payload(&a.kind, &a.payload)?;
    Ok(a)
}

/// The part of `call` that cannot write: refuses a malformed call with the tool's own error.
/// See `artifact::preflight`.
pub(crate) fn preflight(args: &Value) -> Result<()> {
    parse_args(args.clone()).map(|_| ())
}

/// The event kinds whose fact has a text twin (an `## Events` section of a single-writer document).
const TWIN_KINDS: &[&str] = &["note", "verdict", "intent", "reviewed", "external_signal"];

/// The document an event's text twin goes into: its path and the bytes read from it.
struct TwinDocument {
    path: std::path::PathBuf,
    text: String,
}

/// Bytes this call wrote into a document, with the bytes it replaced.
struct TwinWrite {
    path: std::path::PathBuf,
    original: String,
    written: String,
}

impl TwinWrite {
    /// The catalog write failed after this file write: put the file back if it still holds what
    /// this call wrote, and return an error that says what became of the file.
    fn undo(&self, cause: impl std::fmt::Display) -> anyhow::Error {
        use crate::librarian::catalog::dual_write::{
            document_label, restore_section_after_failed_commit, without_path,
        };
        let label = document_label(&self.path);
        // Every lower-layer error that enters the text is stripped of the document's path first
        // (`without_path`), as `dual_write` does: the catalog error, and the io error a failed
        // restore carries.
        let cause = without_path(&cause.to_string(), &self.path, &label);
        let rollback =
            match restore_section_after_failed_commit(&self.path, &self.original, &self.written) {
                SectionRollback::Failed(io) => SectionRollback::Failed(std::io::Error::other(
                    without_path(&io.to_string(), &self.path, &label),
                )),
                other => other,
            };
        anyhow::anyhow!(
            "event_create: the catalog write failed after `{label}` was written, so no event was \
             recorded: {cause}. {}",
            twin_rollback_sentence(&rollback)
        )
    }
}

/// The document at the write target when it is single-writer and takes this event kind, or the
/// reason it takes no text twin. A document whose file cannot be read or whose frontmatter does
/// not parse has no flag to read, so it is not single-writer.
fn single_writer_target(
    ctx: &ToolContext,
    cat: &crate::librarian::catalog::Catalog,
    artifact_id: &str,
    kind: &str,
) -> std::result::Result<TwinDocument, &'static str> {
    const NOT_SINGLE_WRITER: &str = "not a single-writer document";
    if !TWIN_KINDS.contains(&kind) {
        return Err("kind has no text twin");
    }
    let Ok(Some(row)) = crate::librarian::catalog::artifact::get(cat, artifact_id) else {
        return Err(NOT_SINGLE_WRITER);
    };
    let path = row.abs_path;
    let Ok(text) =
        crate::librarian::read_door::ReadDoor::new(super::managed_roots(ctx)).text(&path)
    else {
        return Err(NOT_SINGLE_WRITER);
    };
    let Ok((Some(fm), _)) = crate::librarian::frontmatter::parse(&text) else {
        return Err(NOT_SINGLE_WRITER);
    };
    if fm.extra.get("single_writer") != Some(&Value::Bool(true)) {
        return Err(NOT_SINGLE_WRITER);
    }
    // A ledger or an entry document ignores the flag: a section appended to it would be read as
    // an entry, or as a declared-field problem.
    if !crate::librarian::catalog::augmentation::raw_declared_prefixes(Some(&fm)).is_empty()
        || fm.extra.contains_key("entry_kind")
    {
        return Err("ledger or entry document");
    }
    Ok(TwinDocument { path, text })
}

/// The grammar form of each `also_mutates` id, or the reason the twin is skipped: a target that
/// the grammar cannot name (outside the key space, shared key, no git repository) skips the whole
/// twin, and the row stays.
fn twin_mutates_forms(
    cat: &crate::librarian::catalog::Catalog,
    doc_path: &std::path::Path,
    ids: &[String],
) -> std::result::Result<Vec<String>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let Some(root) = crate::librarian::current_project::lookup_git_root(doc_path) else {
        return Err("target not nameable: the document lies in no git repository".to_string());
    };
    ids.iter()
        .map(|id| {
            crate::librarian::catalog::cite_text::grammar_form_of(&cat.conn, &root, id)
                .map_err(|e| format!("target not nameable: {e:#}"))
        })
        .collect()
}

/// What a caller is told about the document after the catalog write failed: one sentence that
/// claims only what the rollback did.
fn twin_rollback_sentence(rollback: &SectionRollback) -> String {
    match rollback {
        SectionRollback::Restored => "The document was put back as it was.".to_string(),
        SectionRollback::ForeignChange => "The document was changed by another writer after \
            this call wrote it, so it was left as found and still holds the new section."
            .to_string(),
        SectionRollback::Failed(e) => {
            format!("Putting the document back failed ({e}); the new section may still be in it.")
        }
    }
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a = parse_args(args)?;

    let a = {
        let mut cat = ctx.catalog.lock();
        let artifact_id = super::worktree::resolve_write_target(&mut cat, ctx, &a.artifact_id)?;
        Args { artifact_id, ..a }
    };

    let _write_guard = write_locks().lock_for(&a.artifact_id).lock_owned().await;

    if let Some(ref target) = a.resolves_intent_event_id {
        if a.kind != "verdict" {
            return Err(LibrarianRecoverableError::new(
                "resolves_intent_event_id only valid on verdict events",
            ));
        }
        let cat = ctx.catalog.lock();
        let target_kind: Option<String> = cat
            .conn
            .query_row(
                "SELECT kind FROM events WHERE id=?1",
                rusqlite::params![target],
                |r| r.get(0),
            )
            .ok();
        match target_kind.as_deref() {
            Some("intent") => {}
            Some(k) => {
                return Err(LibrarianRecoverableError::new(format!(
                    "target event {target} is kind={k}, not intent"
                )))
            }
            None => {
                return Err(LibrarianRecoverableError::new(format!(
                    "target event {target} not found"
                )))
            }
        }
        if !event_edges::incoming_by_rel(&cat, target, "resolves")?.is_empty() {
            return Err(LibrarianRecoverableError::new(format!(
                "intent {target} already resolved"
            )));
        }
    }

    let now = chrono::Utc::now().timestamp_millis();
    let id = next_monotonic_id();

    let parent_id = match &a.parent_event_id {
        Some(p) => Some(p.clone()),
        None => {
            let cat = ctx.catalog.lock();
            events::latest_for_artifact(&cat, &a.artifact_id)?.map(|e| e.id)
        }
    };

    if a.kind == "status_change" || a.kind == "field_patch" {
        apply_payload_to_frontmatter(ctx, &a.artifact_id, &a.kind, &a.payload)?;
    }

    let payload_str = serde_json::to_string(&a.payload)?;
    let event_row = events::EventRow {
        id: id.clone(),
        artifact_id: a.artifact_id.clone(),
        kind: a.kind.clone(),
        payload: payload_str,
        anchor_commit: a.anchor_commit.clone(),
        head_commit: a.head_commit.clone(),
        author: a.author.clone(),
        created_at: now,
    };

    let supersedes_link = if a.kind == "superseded_by" {
        let target_id = a
            .payload
            .get("target_artifact_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                LibrarianRecoverableError::new("superseded_by.target_artifact_id required")
            })?;
        Some(crate::librarian::catalog::links::LinkRow {
            src_id: a.artifact_id.clone(),
            dst_id: target_id.into(),
            rel: "supersedes".into(),
            created_at: now,
        })
    } else {
        None
    };

    let mut edges: Vec<event_edges::EdgeRow> = Vec::new();

    if let Some(p) = parent_id.clone() {
        edges.push(event_edges::EdgeRow {
            src_event_id: id.clone(),
            dst_event_id: Some(p),
            dst_artifact_id: None,
            dst_source_id: None,
            rel: "parent".into(),
        });
    }

    let source_row = a.source.as_ref().map(|s| {
        let src_id = format!("{}:{}", s.kind, s.uri);
        edges.push(event_edges::EdgeRow {
            src_event_id: id.clone(),
            dst_event_id: None,
            dst_artifact_id: None,
            dst_source_id: Some(src_id.clone()),
            rel: "triggered_by".into(),
        });
        sources::SourceRow {
            id: src_id,
            uri: s.uri.clone(),
            kind: s.kind.clone(),
            payload: s.payload.as_ref().map(|p| p.to_string()),
            ingested_at: now,
        }
    });

    let mutates_ids: Vec<String> = a.also_mutates.clone().unwrap_or_default();
    for art in mutates_ids.iter().cloned() {
        edges.push(event_edges::EdgeRow {
            src_event_id: id.clone(),
            dst_event_id: None,
            dst_artifact_id: Some(art),
            dst_source_id: None,
            rel: "mutates".into(),
        });
    }

    if let Some(target) = a.resolves_intent_event_id.clone() {
        edges.push(event_edges::EdgeRow {
            src_event_id: id.clone(),
            dst_event_id: Some(target),
            dst_artifact_id: None,
            dst_source_id: None,
            rel: "resolves".into(),
        });
    }

    let text_twin: String;
    {
        let cat = ctx.catalog.lock();
        let tx = cat.conn.unchecked_transaction()?;
        if let Some(s) = &source_row {
            sources::upsert_with(&tx, s)?;
        }
        if let Some(l) = &supersedes_link {
            crate::librarian::catalog::links::insert_with(&tx, l)?;
        }
        events::insert_with(&tx, &event_row)?;
        #[cfg(test)]
        if tests::INJECT_FAIL_AFTER_EVENT_INSERT.with(|c| {
            let v = c.get();
            c.set(false);
            v
        }) {
            anyhow::bail!("test injection: forced failure after event insert");
        }
        event_edges::insert_many_in_tx(&tx, &edges)?;

        // The text twin: the file is written before the commit, inside the same lock, and put
        // back when the commit fails. A rule that does not apply skips the twin and keeps the
        // row; a write that is refused or fails is an error, and the transaction drops with no
        // row recorded.
        let mut twin_file: Option<TwinWrite> = None;
        match single_writer_target(ctx, &cat, &a.artifact_id, &a.kind) {
            Err(reason) => text_twin = format!("skipped: {reason}"),
            Ok(doc) => match twin_mutates_forms(&cat, &doc.path, &mutates_ids) {
                Err(reason) => text_twin = format!("skipped: {reason}"),
                Ok(mutates) => {
                    let event_text = EventText {
                        id: id.clone(),
                        kind: a.kind.clone(),
                        created_at_ms: now,
                        author: a.author.clone(),
                        anchor_commit: a.anchor_commit.clone(),
                        head_commit: a.head_commit.clone(),
                        parent: parent_id.clone(),
                        resolves: a.resolves_intent_event_id.clone(),
                        source: a.source.as_ref().map(|s| (s.kind.clone(), s.uri.clone())),
                        mutates,
                        payload: a.payload.clone(),
                    };
                    let written = splice_event(&doc.text, &event_text).map_err(|e| {
                        LibrarianRecoverableError::new(format!(
                            "event_create: the event could not be added to the document, so \
                             nothing was recorded: {e:#}"
                        ))
                    })?;
                    let label = crate::librarian::catalog::dual_write::document_label(&doc.path);
                    crate::librarian::util::write_catalogued_file(
                        &super::managed_roots(ctx),
                        &doc.path,
                        &written,
                    )
                    .map_err(|e| {
                        // The refusal and the io error write `path.display()` (and a link's
                        // target) into their text; a response names the document by its label.
                        let e = crate::librarian::catalog::dual_write::without_path(
                            &format!("{e:#}"),
                            &doc.path,
                            &label,
                        );
                        LibrarianRecoverableError::new(format!(
                            "event_create: cannot write {label}, so no event was recorded: {e}"
                        ))
                    })?;
                    let file = TwinWrite {
                        path: doc.path,
                        original: doc.text,
                        written,
                    };
                    if let Err(e) = crate::librarian::catalog::dual_write::record_written_file(
                        &tx,
                        &a.artifact_id,
                        &file.path.to_string_lossy(),
                        &file.written,
                    ) {
                        return Err(file.undo(e));
                    }
                    text_twin = "written".to_string();
                    twin_file = Some(file);
                }
            },
        }

        if let Err(e) = tx.commit() {
            return Err(match twin_file {
                Some(file) => file.undo(e),
                None => e.into(),
            });
        }

        if a.kind == "note" {
            if let Some(text) = a.payload.get("text").and_then(|v| v.as_str()) {
                let obs = crate::librarian::catalog::observations::ObservationRow {
                    id: None,
                    artifact_id: a.artifact_id.clone(),
                    text: text.to_string(),
                    source: a.author.clone(),
                    created_at: now,
                };
                let _ = crate::librarian::catalog::observations::insert(&cat, &obs);
            }
        }
    }

    Ok(json!({
        "event_id": id,
        "parent_event_id": parent_id,
        "anchor_commit": a.anchor_commit,
        "head_commit": a.head_commit,
        "text_twin": text_twin,
    }))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{
        upsert as art_insert, ArtifactRow, TestArtifactRowBuilder,
    };
    use crate::librarian::tools::{TestToolContextBuilder, ToolContext};
    use tempfile::TempDir;

    pub(crate) fn mk_ctx(tmp_root: std::path::PathBuf) -> ToolContext {
        use crate::librarian::workspace::Root;
        TestToolContextBuilder::new(crate::librarian::catalog::Catalog::open_in_memory().unwrap())
            .with_root(Root {
                name: "r".into(),
                path: tmp_root,
            })
            .build()
    }

    thread_local! {
        /// When set true, `call()` aborts with an error after inserting the
        /// event row but before inserting edges. Used by the transaction-rollback
        /// test below to verify atomicity. Thread-local so parallel tests
        /// running on different threads do not race on a shared flag.
        //
        // clippy 1.96+ (Windows toolchain) false-positives `missing_const_for_thread_local`
        // here even though the initializer already uses `const { ... }`. Scoped allow,
        // same false-positive as `next_monotonic_id`'s GENERATOR above.
        #[allow(clippy::missing_const_for_thread_local)]
        pub(super) static INJECT_FAIL_AFTER_EVENT_INSERT: std::cell::Cell<bool> =
            const { std::cell::Cell::new(false) };
    }

    fn art(id: &str) -> ArtifactRow {
        TestArtifactRowBuilder::new(id).build()
    }

    #[tokio::test]
    async fn note_event_round_trip() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("a1")).unwrap();
        }
        let result = call(
            &ctx,
            json!({
                "artifact_id": "a1",
                "kind": "note",
                "payload": {"text": "hi"}
            }),
        )
        .await
        .unwrap();
        let event_id = result["event_id"].as_str().unwrap();
        assert!(!event_id.is_empty());
    }

    #[tokio::test]
    async fn note_event_also_writes_observation_row() {
        use crate::librarian::catalog::observations;
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("obs-art")).unwrap();
        }

        call(
            &ctx,
            json!({
                "artifact_id": "obs-art",
                "kind": "note",
                "payload": {"text": "hello observation"}
            }),
        )
        .await
        .unwrap();

        let cat = ctx.catalog.lock();
        let obs = observations::list_for_artifact(&cat, "obs-art").unwrap();
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].text, "hello observation");
    }

    #[tokio::test]
    async fn rejects_unknown_kind() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let err = call(
            &ctx,
            json!({
                "artifact_id": "a1",
                "kind": "bogus",
                "payload": {}
            }),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("unknown event kind"));
    }

    #[tokio::test]
    async fn verdict_resolves_intent_emits_edge() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("a2")).unwrap();
        }

        // Write intent event
        let intent_result = call(
            &ctx,
            json!({
                "artifact_id": "a2",
                "kind": "intent",
                "payload": {"hypothesis": "X causes Y"}
            }),
        )
        .await
        .unwrap();
        let intent_id = intent_result["event_id"].as_str().unwrap().to_string();

        // Write verdict resolving the intent
        let verdict_result = call(
            &ctx,
            json!({
                "artifact_id": "a2",
                "kind": "verdict",
                "payload": {"outcome": "confirmed"},
                "resolves_intent_event_id": intent_id
            }),
        )
        .await
        .unwrap();
        let verdict_id = verdict_result["event_id"].as_str().unwrap().to_string();

        let cat = ctx.catalog.lock();
        let edges = event_edges::outgoing(&cat, &verdict_id).unwrap();
        let resolves_edge = edges.iter().find(|e| e.rel == "resolves");
        assert!(resolves_edge.is_some());
        assert_eq!(
            resolves_edge.unwrap().dst_event_id.as_deref(),
            Some(intent_id.as_str())
        );
    }

    #[tokio::test]
    async fn cannot_resolve_intent_twice() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("a3")).unwrap();
        }

        // Write intent event
        let intent_result = call(
            &ctx,
            json!({
                "artifact_id": "a3",
                "kind": "intent",
                "payload": {"hypothesis": "P implies Q"}
            }),
        )
        .await
        .unwrap();
        let intent_id = intent_result["event_id"].as_str().unwrap().to_string();

        // First verdict — should succeed
        call(
            &ctx,
            json!({
                "artifact_id": "a3",
                "kind": "verdict",
                "payload": {"outcome": "refuted"},
                "resolves_intent_event_id": intent_id
            }),
        )
        .await
        .unwrap();

        // Second verdict against the same intent — should fail
        let err = call(
            &ctx,
            json!({
                "artifact_id": "a3",
                "kind": "verdict",
                "payload": {"outcome": "confirmed"},
                "resolves_intent_event_id": intent_id
            }),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("already resolved"));
    }

    #[tokio::test]
    async fn event_create_superseded_by_creates_link() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("src-art")).unwrap();
            art_insert(&cat, &art("dst-art")).unwrap();
        }

        call(
            &ctx,
            serde_json::json!({
                "artifact_id": "src-art",
                "kind": "superseded_by",
                "payload": {"target_artifact_id": "dst-art"}
            }),
        )
        .await
        .unwrap();

        // Expect an artifact_link with rel=supersedes from src-art → dst-art.
        let count: i64 = ctx
            .catalog
            .lock()
            .conn
            .query_row(
                "SELECT count(*) FROM artifact_link WHERE src_id='src-art' AND dst_id='dst-art' AND rel='supersedes'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 1,
            "superseded_by event must create an artifact_link rel=supersedes"
        );
    }

    #[tokio::test]
    async fn unknown_kind_is_recoverable_error() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let err = call(
            &ctx,
            json!({
                "artifact_id": "a1",
                "kind": "bogus",
                "payload": {}
            }),
        )
        .await
        .unwrap_err();
        assert!(
            err.downcast_ref::<LibrarianRecoverableError>().is_some(),
            "expected RecoverableError, got: {err:#}"
        );
    }

    #[tokio::test]
    async fn rollback_on_failure_after_event_insert_leaves_no_orphan_row() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("a-tx")).unwrap();
        }
        INJECT_FAIL_AFTER_EVENT_INSERT.with(|c| c.set(true));
        let err = call(
            &ctx,
            json!({
                "artifact_id": "a-tx",
                "kind": "note",
                "payload": {"text": "should roll back"}
            }),
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains("test injection"),
            "expected injected failure, got: {err:#}"
        );
        // Event row must be rolled back — count must be zero for this artifact.
        let cat = ctx.catalog.lock();
        let count: i64 = cat
            .conn
            .query_row(
                "SELECT count(*) FROM events WHERE artifact_id='a-tx'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 0,
            "expected event-row rollback after failure between event insert and edges insert"
        );
        // No edges either.
        let edge_count: i64 = cat
            .conn
            .query_row("SELECT count(*) FROM event_edges", [], |r| r.get(0))
            .unwrap();
        assert_eq!(edge_count, 0);
    }

    #[tokio::test]
    async fn intent_inputs_payload_passthrough() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("a-inp")).unwrap();
        }
        let inputs = json!([
            {"kind": "doc", "ref": "spec/foo.md"},
            {"kind": "issue", "ref": "linear://abc-123"}
        ]);
        let result = call(
            &ctx,
            json!({
                "artifact_id": "a-inp",
                "kind": "intent",
                "payload": {"hypothesis": "X works", "inputs": inputs.clone()}
            }),
        )
        .await
        .unwrap();
        let event_id = result["event_id"].as_str().unwrap().to_string();

        let cat = ctx.catalog.lock();
        let payload_str: String = cat
            .conn
            .query_row(
                "SELECT payload FROM events WHERE id=?1",
                rusqlite::params![event_id],
                |r| r.get(0),
            )
            .unwrap();
        let payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap();
        assert_eq!(
            payload["inputs"], inputs,
            "intent.inputs must round-trip unchanged through events.payload"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn concurrent_calls_on_same_artifact_chain_not_fan() {
        let tmp = TempDir::new().unwrap();
        let ctx = std::sync::Arc::new(mk_ctx(tmp.path().to_path_buf()));
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("conc")).unwrap();
        }
        let c1 = ctx.clone();
        let c2 = ctx.clone();
        let h1 = tokio::spawn(async move {
            call(
                &c1,
                json!({
                    "artifact_id": "conc",
                    "kind": "note",
                    "payload": {"text": "first"}
                }),
            )
            .await
            .unwrap()
        });
        let h2 = tokio::spawn(async move {
            call(
                &c2,
                json!({
                    "artifact_id": "conc",
                    "kind": "note",
                    "payload": {"text": "second"}
                }),
            )
            .await
            .unwrap()
        });
        let r1 = h1.await.unwrap();
        let r2 = h2.await.unwrap();

        let id1 = r1["event_id"].as_str().unwrap().to_string();
        let id2 = r2["event_id"].as_str().unwrap().to_string();
        let p1 = r1["parent_event_id"].as_str().map(|s| s.to_string());
        let p2 = r2["parent_event_id"].as_str().map(|s| s.to_string());

        // Exactly one event must have no parent (the lock-winner) and the
        // other must point to it. Without the lock, both would race
        // `latest_for_artifact` and both would return parent_event_id=None.
        match (p1.as_deref(), p2.as_deref()) {
            (None, Some(p)) => assert_eq!(p, id1, "second event must chain off first"),
            (Some(p), None) => assert_eq!(p, id2, "second event must chain off first"),
            other => panic!(
                "expected one None and one Some(other_id); got {other:?} (id1={id1}, id2={id2})"
            ),
        }

        let cat = ctx.catalog.lock();
        let count: i64 = cat
            .conn
            .query_row(
                "SELECT count(*) FROM events WHERE artifact_id='conc'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn field_patch_unwritable_field_errors_and_writes_no_event() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        // Write a real artifact file on disk so frontmatter ops have a target.
        let rel_path = "a-fp.md";
        std::fs::write(
            tmp.path().join(rel_path),
            "---\nid: a-fp\nrepo: r\nrel_path: a-fp.md\nkind: spec\nstatus: active\n---\n# body\n",
        )
        .unwrap();
        {
            let cat = ctx.catalog.lock();
            art_insert(
                &cat,
                &ArtifactRow {
                    abs_path: std::path::PathBuf::from(format!("/test/r/{rel_path}")),
                    ..art("a-fp")
                },
            )
            .unwrap();
        }
        let err = call(
            &ctx,
            json!({
                "artifact_id": "a-fp",
                "kind": "field_patch",
                "payload": {"field": "owners", "to": ["alice"]}
            }),
        )
        .await
        .unwrap_err();
        assert!(
            err.downcast_ref::<LibrarianRecoverableError>().is_some(),
            "expected RecoverableError, got: {err:#}"
        );
        assert!(
            err.to_string().contains("not writable"),
            "expected 'not writable' in error, got: {err:#}"
        );
        // No event row may be written when the disk write would have been a no-op.
        let cat = ctx.catalog.lock();
        let count: i64 = cat
            .conn
            .query_row(
                "SELECT count(*) FROM events WHERE artifact_id='a-fp'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 0,
            "field_patch on unwritable field must not insert an event row"
        );
    }
    // ---- the text twin of a note, verdict, intent, reviewed or external_signal event ----

    use crate::librarian::catalog::event_text::parse_event_sections;

    const FLAGGED: &str = "---\nsingle_writer: true\n---\n\n# Notes\n\nbody\n";

    /// A real file under a fresh temp dir and an artifact row `doc1` whose `abs_path` is that file.
    fn twin_fixture(text: &str) -> (TempDir, ToolContext, std::path::PathBuf) {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let path = tmp.path().join("notes.md");
        std::fs::write(&path, text).unwrap();
        {
            let cat = ctx.catalog.lock();
            art_insert(
                &cat,
                &TestArtifactRowBuilder::new("doc1")
                    .with_abs_path(path.clone())
                    .build(),
            )
            .unwrap();
        }
        (tmp, ctx, path)
    }

    fn event_rows(ctx: &ToolContext, artifact_id: &str) -> i64 {
        let cat = ctx.catalog.lock();
        cat.conn
            .query_row(
                "SELECT count(*) FROM events WHERE artifact_id = ?1",
                rusqlite::params![artifact_id],
                |r| r.get(0),
            )
            .unwrap()
    }

    fn row_sha(ctx: &ToolContext, artifact_id: &str) -> String {
        let cat = ctx.catalog.lock();
        crate::librarian::catalog::artifact::get(&cat, artifact_id)
            .unwrap()
            .unwrap()
            .file_sha256
    }

    async fn note(ctx: &ToolContext, artifact_id: &str, text: &str) -> Result<Value> {
        call(
            ctx,
            json!({"artifact_id": artifact_id, "kind": "note", "payload": {"text": text}}),
        )
        .await
    }

    #[tokio::test]
    async fn a_note_on_a_single_writer_document_gets_a_section_and_a_row() {
        let (_tmp, ctx, path) = twin_fixture(FLAGGED);
        let r = note(&ctx, "doc1", "first thought").await.unwrap();
        assert_eq!(r["text_twin"], "written");
        let id = r["event_id"].as_str().unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.starts_with("---\nsingle_writer: true\n---\n"),
            "the frontmatter is untouched: {text}"
        );
        let sections = parse_event_sections(&text);
        assert_eq!(sections.len(), 1, "{text}");
        assert_eq!(sections[0].id, id);
        assert_eq!(sections[0].kind, "note");
        assert_eq!(sections[0].payload, json!({"text": "first thought"}));
        assert_eq!(event_rows(&ctx, "doc1"), 1);
        assert_eq!(
            row_sha(&ctx, "doc1"),
            crate::librarian::util::sha_of_bytes(text.as_bytes()),
            "the row describes the bytes the call wrote"
        );
    }

    #[tokio::test]
    async fn the_flag_is_set_through_doc_update_extra() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            json!({
                "repo": "r", "rel_path": "docs/specs/notes.md", "kind": "spec",
                "title": "Notes", "body": "# Notes\n\nbody\n"
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();
        let path = tmp.path().join("docs/specs/notes.md");

        let before = note(&ctx, &id, "before the flag").await.unwrap();
        assert!(
            before["text_twin"]
                .as_str()
                .unwrap()
                .starts_with("skipped:"),
            "{before}"
        );

        crate::librarian::tools::update::call(
            &ctx,
            json!({"id": id, "patch": {"extra": {"single_writer": true}}}),
        )
        .await
        .unwrap();
        let after = note(&ctx, &id, "after the flag").await.unwrap();
        assert_eq!(after["text_twin"], "written");

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("after the flag"), "{text}");
        assert!(!text.contains("before the flag"), "{text}");
        assert_eq!(event_rows(&ctx, &id), 2, "both events keep their row");
    }

    #[tokio::test]
    async fn a_note_on_an_unflagged_document_keeps_only_the_row() {
        for (label, text) in [
            ("no flag", "---\nkind: spec\n---\n\n# Plain\n"),
            ("flag false", "---\nsingle_writer: false\n---\n\n# Plain\n"),
            (
                "flag a string",
                "---\nsingle_writer: \"true\"\n---\n\n# Plain\n",
            ),
            ("no frontmatter", "# Plain\n"),
        ] {
            let (_tmp, ctx, path) = twin_fixture(text);
            let r = note(&ctx, "doc1", "a thought").await.unwrap();
            assert_eq!(
                r["text_twin"], "skipped: not a single-writer document",
                "{label}"
            );
            assert_eq!(std::fs::read_to_string(&path).unwrap(), text, "{label}");
            assert_eq!(event_rows(&ctx, "doc1"), 1, "{label}");
        }
    }

    #[tokio::test]
    async fn a_flagged_ledger_or_entry_document_gets_no_text() {
        for (label, text) in [
            (
                "declared entry_prefix",
                "---\nsingle_writer: true\nentry_prefix: F\n---\n\n# Ledger\n",
            ),
            (
                "entry_kind",
                "---\nsingle_writer: true\nentry_kind: friction\n---\n\n# Entry\n",
            ),
        ] {
            let (_tmp, ctx, path) = twin_fixture(text);
            let r = note(&ctx, "doc1", "a thought").await.unwrap();
            assert_eq!(
                r["text_twin"], "skipped: ledger or entry document",
                "{label}"
            );
            assert_eq!(std::fs::read_to_string(&path).unwrap(), text, "{label}");
            assert_eq!(event_rows(&ctx, "doc1"), 1, "{label}");
        }
    }

    #[tokio::test]
    async fn a_kind_without_a_text_twin_keeps_only_the_row() {
        let (_tmp, ctx, path) = twin_fixture(FLAGGED);
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("other")).unwrap();
        }
        let r = call(
            &ctx,
            json!({
                "artifact_id": "doc1", "kind": "superseded_by",
                "payload": {"target_artifact_id": "other"}
            }),
        )
        .await
        .unwrap();
        assert_eq!(r["text_twin"], "skipped: kind has no text twin");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), FLAGGED);
        assert_eq!(event_rows(&ctx, "doc1"), 1);
    }

    #[tokio::test]
    async fn a_verdict_writes_a_fenced_payload_and_a_resolves_id() {
        let (_tmp, ctx, path) = twin_fixture(FLAGGED);
        let intent = call(
            &ctx,
            json!({
                "artifact_id": "doc1", "kind": "intent",
                "payload": {"hypothesis": "X causes Y"}
            }),
        )
        .await
        .unwrap();
        assert_eq!(intent["text_twin"], "written");
        let intent_id = intent["event_id"].as_str().unwrap().to_string();
        let verdict = call(
            &ctx,
            json!({
                "artifact_id": "doc1", "kind": "verdict",
                "payload": {"outcome": "confirmed"},
                "resolves_intent_event_id": intent_id
            }),
        )
        .await
        .unwrap();
        assert_eq!(verdict["text_twin"], "written");
        let verdict_id = verdict["event_id"].as_str().unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("```event-payload\n"), "{text}");
        let sections = parse_event_sections(&text);
        assert_eq!(sections.len(), 2, "{text}");
        assert_eq!(sections[0].id, intent_id);
        assert_eq!(sections[0].kind, "intent");
        assert_eq!(sections[1].id, verdict_id);
        assert_eq!(sections[1].kind, "verdict");
        assert_eq!(sections[1].payload, json!({"outcome": "confirmed"}));
        assert_eq!(sections[1].resolves.as_deref(), Some(intent_id.as_str()));
        assert_eq!(sections[1].parent.as_deref(), Some(intent_id.as_str()));
    }

    #[tokio::test]
    async fn two_notes_keep_one_events_heading_and_call_order() {
        let (_tmp, ctx, path) = twin_fixture(FLAGGED);
        let first = note(&ctx, "doc1", "first").await.unwrap();
        let second = note(&ctx, "doc1", "second").await.unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.matches("## Events").count(), 1, "{text}");
        let sections = parse_event_sections(&text);
        let ids: Vec<&str> = sections.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                first["event_id"].as_str().unwrap(),
                second["event_id"].as_str().unwrap()
            ],
            "{text}"
        );
        assert_eq!(sections[1].payload, json!({"text": "second"}));
        assert_eq!(
            sections[1].parent.as_deref(),
            first["event_id"].as_str(),
            "the second note chains off the first"
        );
    }

    #[tokio::test]
    async fn a_failed_commit_restores_the_file() {
        let (_tmp, ctx, path) = twin_fixture(FLAGGED);
        let sha_before = row_sha(&ctx, "doc1");
        // A deferred foreign-key violation raised by the `events` INSERT: every statement and the
        // file write succeed, and only the COMMIT fails.
        ctx.catalog
            .lock()
            .conn
            .execute_batch(
                "CREATE TABLE commit_tripwire(
                     missing TEXT REFERENCES artifact(id) DEFERRABLE INITIALLY DEFERRED
                 );
                 CREATE TRIGGER commit_tripwire_trg AFTER INSERT ON events
                 BEGIN
                     INSERT INTO commit_tripwire(missing) VALUES ('no-such-artifact');
                 END;",
            )
            .unwrap();

        let err = note(&ctx, "doc1", "never recorded").await.unwrap_err();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            FLAGGED,
            "the file is back to its old bytes"
        );
        assert_eq!(event_rows(&ctx, "doc1"), 0, "no row remains");
        assert_eq!(row_sha(&ctx, "doc1"), sha_before, "the row is untouched");
        let msg = format!("{err:#}");
        assert!(msg.contains("put back as it was"), "{msg}");
        assert!(
            !msg.contains(&path.display().to_string()),
            "no absolute path in the error: {msg}"
        );
    }

    #[test]
    fn the_rollback_sentence_claims_only_what_happened() {
        let restored = twin_rollback_sentence(&SectionRollback::Restored);
        assert!(restored.contains("put back as it was"), "{restored}");
        let foreign = twin_rollback_sentence(&SectionRollback::ForeignChange);
        assert!(foreign.contains("left as found"), "{foreign}");
        assert!(!foreign.contains("put back as it was"), "{foreign}");
        let failed =
            twin_rollback_sentence(&SectionRollback::Failed(std::io::Error::other("boom")));
        assert!(failed.contains("boom"), "{failed}");
        assert!(failed.contains("may still be in it"), "{failed}");
        assert!(!failed.contains("put back as it was"), "{failed}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_failed_file_write_leaves_no_row() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        // The link's target lies in a directory of its own, so the target path is not covered by
        // stripping the link's own directory.
        std::fs::create_dir(tmp.path().join("sub")).unwrap();
        let real = tmp.path().join("sub/real.md");
        std::fs::write(&real, FLAGGED).unwrap();
        let link = tmp.path().join("link.md");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        {
            let cat = ctx.catalog.lock();
            art_insert(
                &cat,
                &TestArtifactRowBuilder::new("doc1")
                    .with_abs_path(link.clone())
                    .build(),
            )
            .unwrap();
        }

        let err = note(&ctx, "doc1", "never recorded").await.unwrap_err();

        assert_eq!(event_rows(&ctx, "doc1"), 0, "no row remains: {err:#}");
        assert_eq!(
            std::fs::read_to_string(&real).unwrap(),
            FLAGGED,
            "nothing was written through the link"
        );
        let hint = err
            .downcast_ref::<LibrarianRecoverableError>()
            .and_then(|e| e.hint.clone())
            .unwrap_or_default();
        let shown = format!("{err:#} {hint}");
        assert!(
            shown.contains("link.md"),
            "the error names the document: {shown}"
        );
        for (what, path) in [
            ("the link path", link.display().to_string()),
            ("the link's target path", real.display().to_string()),
            ("the temp-dir root", tmp.path().display().to_string()),
        ] {
            assert!(!shown.contains(&path), "the error holds {what}: {shown}");
        }
    }

    /// Both lower-layer errors a failed restore can carry (a missing file, a link) write the
    /// path into their text, and so can the cause; none may reach the caller.
    #[cfg(unix)]
    #[test]
    fn a_failed_restore_names_the_document_and_holds_no_path() {
        let tmp = TempDir::new().unwrap();
        let gone = tmp.path().join("gone.md");
        let real = tmp.path().join("sub");
        std::fs::create_dir(&real).unwrap();
        let real = real.join("real.md");
        std::fs::write(&real, "x").unwrap();
        let link = tmp.path().join("link.md");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        for (label, path) in [("a missing file", gone), ("a link", link)] {
            let file = TwinWrite {
                path: path.clone(),
                original: "old".into(),
                written: "new".into(),
            };
            let err = file.undo(format!("boom at {}", path.display()));
            let text = format!("{err:#}");
            assert!(text.contains("may still be in it"), "{label}: {text}");
            for (what, p) in [
                ("the document path", path.display().to_string()),
                ("the link's target", real.display().to_string()),
                ("the temp-dir root", tmp.path().display().to_string()),
            ] {
                assert!(!text.contains(&p), "{label}: holds {what}: {text}");
            }
        }
    }

    #[tokio::test]
    async fn an_unreadable_insertion_point_refuses_the_call_and_records_nothing() {
        // An unclosed code fence swallows the section: the event would not read back.
        let text = format!("{FLAGGED}\n```\nunclosed\n");
        let (_tmp, ctx, path) = twin_fixture(&text);
        let err = note(&ctx, "doc1", "never recorded").await.unwrap_err();
        assert!(format!("{err:#}").contains("nothing was added"), "{err:#}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        assert_eq!(event_rows(&ctx, "doc1"), 0);
    }

    /// A git root `repo/` that excludes `contrib/**` from the key space, with the flagged
    /// `docs/notes.md` (`doc1`), a nameable `docs/target.md` (`target`) and an excluded
    /// `contrib/x.md` (`outside`).
    fn repo_fixture() -> (TempDir, ToolContext, std::path::PathBuf) {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(repo.join(".codescout")).unwrap();
        std::fs::write(
            repo.join(".codescout/key-space.toml"),
            "exclude = [\"contrib/**\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(repo.join("docs")).unwrap();
        std::fs::create_dir_all(repo.join("contrib")).unwrap();
        let notes = repo.join("docs/notes.md");
        std::fs::write(&notes, FLAGGED).unwrap();
        std::fs::write(repo.join("docs/target.md"), "# T\n").unwrap();
        std::fs::write(repo.join("contrib/x.md"), "# X\n").unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        {
            let cat = ctx.catalog.lock();
            for (id, rel) in [
                ("doc1", "docs/notes.md"),
                ("target", "docs/target.md"),
                ("outside", "contrib/x.md"),
            ] {
                art_insert(
                    &cat,
                    &TestArtifactRowBuilder::new(id)
                        .with_abs_path(repo.join(rel))
                        .build(),
                )
                .unwrap();
            }
        }
        (tmp, ctx, notes)
    }

    async fn note_mutating(ctx: &ToolContext, targets: &[&str]) -> Result<Value> {
        call(
            ctx,
            json!({
                "artifact_id": "doc1", "kind": "note",
                "payload": {"text": "changed things"}, "also_mutates": targets
            }),
        )
        .await
    }

    #[tokio::test]
    async fn a_mutates_target_inside_the_key_space_is_written_as_its_key() {
        let (_tmp, ctx, notes) = repo_fixture();
        let r = note_mutating(&ctx, &["target"]).await.unwrap();
        assert_eq!(r["text_twin"], "written");
        let text = std::fs::read_to_string(&notes).unwrap();
        assert!(text.contains("**Mutates:** [[target]]"), "{text}");
        assert!(
            !text.contains("target.md") && !text.contains("/repo/"),
            "{text}"
        );
        let sections = parse_event_sections(&text);
        assert_eq!(sections[0].mutates, ["[[target]]"]);
    }

    #[tokio::test]
    async fn a_mutates_target_outside_the_key_space_skips_the_twin_and_keeps_the_row() {
        let (_tmp, ctx, notes) = repo_fixture();
        for targets in [&["outside"][..], &["target", "outside"][..]] {
            let r = note_mutating(&ctx, targets).await.unwrap();
            let twin = r["text_twin"].as_str().unwrap();
            assert!(
                twin.starts_with("skipped: target not nameable: "),
                "{targets:?}: {twin}"
            );
            assert_eq!(
                std::fs::read_to_string(&notes).unwrap(),
                FLAGGED,
                "{targets:?}: the whole twin is skipped, not half of it"
            );
            let id = r["event_id"].as_str().unwrap();
            let cat = ctx.catalog.lock();
            let edges = event_edges::outgoing(&cat, id).unwrap();
            assert_eq!(
                edges.iter().filter(|e| e.rel == "mutates").count(),
                targets.len(),
                "{targets:?}: the mutates edges stay"
            );
        }
        assert_eq!(event_rows(&ctx, "doc1"), 2, "the rows stay");
    }
}
