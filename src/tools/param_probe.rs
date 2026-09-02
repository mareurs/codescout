//! The shared half of the action-labelled schema-key probe (`IC-15`).
//!
//! **Why this lives under `src/tools/` rather than beside its first caller.** It was written in
//! `src/librarian/tools/mod.rs`, which is `#[cfg(feature = "librarian")]`. Every consolidated tool
//! outside that tree — `workspace`, `index`, `library`, `edit_markdown` — dispatches on a string
//! `action` over a shared schema and is therefore exposed to `IC-15`, but none of them depends on
//! the librarian feature. Reaching the probe from there would have meant gating a `workspace` test
//! on `feature = "librarian"`, which makes the guard vanish from the lean lane
//! (`cargo test --workspace --no-default-features`) for tools that have nothing to do with the
//! librarian — a feature-gated test reads as "filtered out", never as a failure. So the probe is
//! feature-independent and both lanes compile it.
//!
//! **Why this exists at all.** A consolidated tool takes one schema for many actions and
//! dispatches on `action`. Each sub-action deserialises the *same* args blob into its own
//! `Args`, so a key the schema advertises for action X, but which X's `Args` does not carry,
//! is dropped by serde in silence — the call succeeds, at defaults. That is `IC-15`'s
//! signature, and `docs/issues/archive/2026-08-17-find-silently-drops-top-level-rel-path.md`
//! is the instance that motivated the first probe.
//!
//! **Why the probe compares two calls rather than asserting one fails.** For each key
//! labelled `<action>:` (or `<a>/<b>: `, a key shared by several actions), that action is
//! called twice: once with its required params alone, once with the same plus the key set to
//! a value ill-typed for its *declared schema type*. A key the sub-tool honours is
//! type-checked, so the second call fails differently. A key it discards never reaches a type
//! check, so both calls behave identically — **and identical is the defect.** Asserting "the
//! second call errors" would be vacuous: most actions have required params and error either
//! way.
//!
//! **The label convention is `<action>: …`, and a key that misses it is skipped SILENTLY.**
//! `sweep` reads `desc.split(':').next()`, so a description written `For action='activate': …`
//! yields the label `For action='activate'`, matches no action, and contributes nothing — the
//! probe then reports success having checked zero keys. Measured 2026-09-02: `workspace`,
//! `index`, `library` and `edit_markdown` all used that prose form, so wiring the probe to them
//! before relabelling would have been vacuous in the one direction `floor` cannot see per-key.
//! `floor` catches the convention breaking wholesale; it cannot catch one key losing its label.
//!
//! **`deny_unknown_fields` is not available as an alternative** — measured, not assumed. It
//! was tried and broke every `doc(update)` call, because the dispatcher passes `action`
//! down and the shared schema holds sibling actions' keys, so every `Args` sees keys that are
//! not its own. See the note on `find::Args`.
//!
//! **Known blindness, and it must be declared per call site.** A param read through an
//! untyped accessor (`args.get(k).and_then(Value::as_str)`) has no ill-typed value — every
//! wrong type reads as absent — so the probe cannot speak for it. Those keys go in
//! `accepts_any_json`, which is an admission, never a pass.
//!
//! **One deliberate difference from the two hand-written probes this replaces:** they built a
//! single `ToolContext` per key and shared it between the base and probe calls; `call` here
//! is invoked twice and may build a fresh one each time. That is strictly more isolation —
//! every base call is designed to fail before it mutates anything, so there was no state to
//! carry, and a fresh context removes the possibility that a base call contaminates its own
//! probe.

use serde_json::{json, Map, Value};

/// The per-tool half: what the generic sweep cannot know.
pub(crate) struct Spec<'a> {
    /// Every action the tool dispatches. A label naming anything else is skipped.
    pub actions: &'a [&'a str],
    /// Keys the probe is structurally blind to. An admission, not a pass.
    pub accepts_any_json: &'a [&'a str],
    /// Minimum type-valid args for an action, chosen to fail resolution *after*
    /// deserialisation so a deser error is visibly different from the baseline.
    pub required: fn(&str) -> Map<String, Value>,
}

fn ill_typed(declared: &str) -> Value {
    if declared == "array" {
        json!(0)
    } else {
        json!([])
    }
}

fn outcome(r: &anyhow::Result<Value>) -> String {
    match r {
        Ok(v) => format!("ok:{v}"),
        Err(e) => format!("err:{e}"),
    }
}

/// Sweep every action-labelled key. Returns `(checked, unhonored)`.
pub(crate) async fn sweep<F, Fut>(schema: &Value, spec: &Spec<'_>, call: F) -> (usize, Vec<String>)
where
    F: Fn(Value) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<Value>>,
{
    let props = schema["properties"]
        .as_object()
        .expect("schema has properties")
        .clone();

    let mut checked = 0usize;
    let mut unhonored = Vec::new();

    for (name, spec_v) in &props {
        if name == "action" || spec.accepts_any_json.contains(&name.as_str()) {
            continue;
        }
        let Some(desc) = spec_v["description"].as_str() else {
            continue;
        };
        // Label convention: `<action>: …`, or `<a>/<b>/<c>: …` for a shared key. The
        // slash split is load-bearing — without it a shared key matches no action and is
        // skipped SILENTLY, which is how `librarian`'s `scope` sat unprobed while an
        // archived IC-15 member was that exact key on that exact tool.
        let Some(action) = desc.split(':').next().and_then(|l| l.split('/').next()) else {
            continue;
        };
        if !spec.actions.contains(&action) {
            continue;
        }

        let declared = spec_v["type"].as_str().unwrap_or("string");
        let mut base_args = (spec.required)(action);
        base_args.insert("action".into(), json!(action));

        let base = call(Value::Object(base_args.clone())).await;
        let mut probe_args = base_args;
        probe_args.insert(name.clone(), ill_typed(declared));
        let probed = call(Value::Object(probe_args)).await;

        if outcome(&base) == outcome(&probed) {
            unhonored.push(format!("{action}:{name} (declared {declared})"));
        }
        checked += 1;
    }

    (checked, unhonored)
}

/// `sweep` plus the two assertions every call site owes.
///
/// The `floor` is not a target. It exists because `unhonored.is_empty()` is **monotone
/// under the label convention breaking**: if `<action>:` prefixes were renamed away, every
/// key would be skipped, `unhonored` would be empty, and the test would pass while
/// checking nothing — the exact failure mode it is here to prevent.
pub(crate) async fn assert_all_honored<F, Fut>(
    tool: &str,
    schema: &Value,
    spec: &Spec<'_>,
    floor: usize,
    call: F,
) where
    F: Fn(Value) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<Value>>,
{
    let (checked, unhonored) = sweep(schema, spec, call).await;
    assert!(
        unhonored.is_empty(),
        "{tool}: these schema keys are labelled for an action whose Args has no such \
         field, so serde discards them silently — the shape of IC-15. Either add the \
         field or move the guidance off the key: {unhonored:?}"
    );
    assert!(
        checked >= floor,
        "{tool}: expected the sweep to cover at least {floor} labelled keys, covered \
         {checked} — the `<action>:` label convention may have changed, which would make \
         this test silently stop checking"
    );
}

/// The **reverse** direction of `sweep`, and the one nothing checked.
///
/// `sweep` walks `schema["properties"]` and asks whether each advertised key is
/// honored — schema→action. It cannot see a key the schema never advertises, so an
/// action whose *required* params are absent from the schema passes it silently. That
/// is how `doc(action="graft")` shipped advertised-but-unusable: `graft::Args`
/// requires `from_id` and `into_id`, neither appeared among the 53 advertised
/// properties, and the single real attempt in `usage.db` failed with
/// `missing_required_param`.
///
/// The check needs no new table. `Spec::required` is **already** a per-action
/// statement of what an action requires, and `sweep` depends on it being complete —
/// its base call must survive deserialisation to be a valid baseline. So the two
/// representations already exist; this asserts they agree, which is the seam itself
/// rather than a third copy of it.
///
/// **What it cannot see, stated because the gap is monotone under omission:** an
/// action missing from `Spec::required` altogether contributes no keys and is passed
/// over in silence. `sweep` catches part of that case indirectly — an action needing
/// fields that `required` omits produces a base call that dies at deserialisation, so
/// base and probe outcomes match and its labelled keys report as unhonored — but only
/// for keys carrying an `<action>:` label. Adding an action means adding it to
/// `required`; neither this assertion nor `sweep` will remind you.
pub(crate) fn assert_required_are_advertised(tool: &str, schema: &Value, spec: &Spec<'_>) {
    let props = schema["properties"]
        .as_object()
        .expect("schema has properties");

    let mut missing = Vec::new();
    let mut checked = 0usize;
    for action in spec.actions {
        for key in (spec.required)(action).keys() {
            if key == "action" {
                continue;
            }
            checked += 1;
            if !props.contains_key(key) {
                missing.push(format!("{action}:{key}"));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "{tool}: these params are REQUIRED by an action's Args but are not advertised \
         in the schema, so a caller cannot discover them and the action is unusable as \
         advertised — the inverse of IC-15, and invisible to the forward sweep. Add \
         them to input_schema: {missing:?}"
    );
    assert!(
        checked > 0,
        "{tool}: the required-param table supplied no keys for any action, so this \
         assertion checked nothing — `Spec::required` or `Spec::actions` has drifted"
    );
}
