//! The shared half of the action-labelled schema-key probe (`IC-15`).
//!
//! **Why this lives under `src/tools/` rather than beside its first caller.** It was written in
//! `src/librarian/tools/mod.rs`, which is `#[cfg(feature = "librarian")]`. Every consolidated tool
//! outside that tree — `workspace`, `index`, `library`, `edit_file` — dispatches on a string
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
//! `index`, `library` and `edit_markdown` (since folded into `edit_file`, Task 8) all used that
//! prose form, so wiring the probe to them
//! before relabelling would have been vacuous in the one direction `floor` cannot see per-key.
//! `floor` catches the convention breaking wholesale; it cannot catch one key losing its label.
//!
//! **`deny_unknown_fields` is not available as an alternative** — measured, not assumed. It
//! was tried and broke every `doc(update)` call, because the dispatcher passes `action`
//! down and the shared schema holds sibling actions' keys, so every `Args` sees keys that are
//! not its own. See the note on `find::Args`.
//!
//! **The call sites are deliberately not numbered.** They carried `Site N of M` until
//! 2026-09-11 and the M decayed twice — once when `artifact_event.rs` was deleted, once when
//! `artifact_refresh.rs` folded into `artifact.rs` — leaving three live sites labelled 1, 2
//! and 4 "of 4", which is a reader's problem in both directions: two totals wrong and a gap
//! implying a site that no longer exists. Numbering also conflates the two directions, which
//! are separate call sites of separate functions. The membership is one call away —
//! `references(symbol="assert_all_honored", path="src/tools/param_probe.rs")`, and the same
//! for `assert_required_are_advertised` — so each site names its tool and its direction and
//! stores no count.
//!
//! **Known blindness, and it must be declared per call site.** A param read through an
//! untyped accessor (`args.get(k).and_then(Value::as_str)`), or deserialised into a bare
//! `Value`, has no ill-typed value — every wrong type reads as absent, or parses — so the
//! probe cannot speak for it. Those keys go in `accepts_any_json`, which is an admission,
//! never a pass.
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
    ///
    /// A nested key is named by its dotted path (`"augment.params"`); a bare name skips the
    /// top-level key and, with it, any recursion into it.
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

/// What one sweep found.
///
/// `checked` counts action/key **pairs**, nested keys included — see `assert_all_honored`'s
/// note on why a per-key figure cannot back a floor.
///
/// **`unprobeable` is the field this struct exists for.** The sweep used to return
/// `(checked, unhonored)` and walk one level of `properties`, so a key moved inside a nested
/// object left the probe's reach with no error, no warning and no change in its own pass/fail
/// — nine keys did exactly that in one commit
/// (`docs/issues/archive/2026-09-02-param-probe-does-not-recurse-so-nesting-a-key-removes-it-from-guard-reach.md`).
/// Recursion closes that hole where it can reach; this field is how the sweep says where it
/// still cannot, rather than reporting success over a population it quietly shrank.
pub(crate) struct Sweep {
    pub checked: usize,
    pub unhonored: Vec<String>,
    pub unprobeable: Vec<String>,
    /// Keys the sweep passed over because the call site declared them in `accepts_any_json`
    /// (a top-level name or a nested dotted path). An admission made on purpose, so it is
    /// recorded to be compared against the call site's own list rather than to be acted on:
    /// a name here that the schema no longer carries is a stale admission, and one the
    /// schema carries that this list lacks is not reachable by any other route.
    pub skipped_any_json: Vec<String>,
    /// Top-level keys that received **no probe** because no `<action>:` label token names a
    /// dispatched action — either the description is missing, or its label (the text before
    /// the first `:`) matches nothing in `Spec::actions`. This is the half `floor` cannot see
    /// per key: a key that loses its label drops out of `checked` and `unhonored` together,
    /// so the sweep reads as clean over a population it quietly shrank
    /// (`docs/issues/archive/2026-09-02-param-probe-reads-only-the-first-slash-token-so-later-actions-are-unswept.md`,
    /// whose residue this field closes).
    pub unlabelled: Vec<String>,
}

/// Sweep every action-labelled key, one level of nesting included.
///
/// **Nested keys inherit their parent's label, and that is the load-bearing half.** Recursing
/// alone would have been vacuous: every child of `doc.event` carries a bare description
/// (`"event author"`, `"git commit to anchor the event to"`) with no `<action>:` prefix, so a
/// label scan of the children matches nothing. The action a child belongs to is the one its
/// parent object is labelled for — which is exact here, since an object like `event` exists
/// for one action only.
///
/// **The nested probe varies a parent the call site already supplies, and never invents one.**
/// `required(action)` hands back a type-valid parent object (`{"kind":"note","payload":{…}}`
/// for `event_create`); the probe clones it and ill-types one child. A parent the site does
/// not supply cannot be probed — adding the object *and* the bogus key varies two things at
/// once, so base and probe would differ for the wrong reason — and that case goes to
/// `unprobeable` rather than being skipped in silence.
pub(crate) async fn sweep<F, Fut>(schema: &Value, spec: &Spec<'_>, call: F) -> Sweep
where
    F: Fn(Value) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<Value>>,
{
    let props = schema["properties"]
        .as_object()
        .expect("schema has properties")
        .clone();

    let mut out = Sweep {
        checked: 0,
        unhonored: Vec::new(),
        unprobeable: Vec::new(),
        skipped_any_json: Vec::new(),
        unlabelled: Vec::new(),
    };

    for (name, spec_v) in &props {
        // `action` is the dispatch key, not a param any action reads; it is not a skip.
        if name == "action" {
            continue;
        }
        if spec.accepts_any_json.contains(&name.as_str()) {
            out.skipped_any_json.push(name.clone());
            continue;
        }
        let Some(desc) = spec_v["description"].as_str() else {
            out.unlabelled.push(name.clone());
            continue;
        };
        // Label convention: `<action>: …`, or `<a>/<b>/<c>: …` for a key shared by several
        // actions. The slash split is load-bearing — without it a shared key matches no
        // action and is skipped SILENTLY, which is how `librarian`'s `scope` sat unprobed
        // while an archived IC-15 member was that exact key on that exact tool.
        //
        // **Every token, and `checked` counts action/key PAIRS.** Taking only the first
        // token left every later action unswept while `checked` still rose once per key, so
        // the coverage loss read as coverage: deleting `doc`'s whole `"gather" =>` dispatch
        // arm left the lib suite at 5007 passed, 0 failed
        // (docs/issues/archive/2026-09-09-param-probe-checks-one-action-per-shared-key.md
        // and docs/issues/archive/
        // 2026-09-02-param-probe-reads-only-the-first-slash-token-so-later-actions-are-unswept.md,
        // the same defect filed twice a week apart). A floor compared against a per-key
        // count cannot see a lost label, which is the one thing the floor exists for.
        let label = desc.split(':').next().unwrap_or_default();

        let declared = spec_v["type"].as_str().unwrap_or("string");

        let mut matched = false;
        for action in label.split('/') {
            if !spec.actions.contains(&action) {
                continue;
            }
            matched = true;

            let mut base_args = (spec.required)(action);
            base_args.insert("action".into(), json!(action));

            let base = call(Value::Object(base_args.clone())).await;
            let mut probe_args = base_args.clone();
            probe_args.insert(name.clone(), ill_typed(declared));
            let probed = call(Value::Object(probe_args)).await;

            if outcome(&base) == outcome(&probed) {
                out.unhonored
                    .push(format!("{action}:{name} (declared {declared})"));
            }
            out.checked += 1;

            // A `"type": "object"` param with no `properties` map of its own is free-form
            // (`filter`, `patch`, `entry`, `extra`, …) — there is nothing to descend into
            // and its contents are not schema-declared, so there is no coverage to lose.
            let Some(children) = spec_v["properties"].as_object() else {
                continue;
            };

            let Some(parent_base) = base_args.get(name).and_then(Value::as_object).cloned() else {
                out.unprobeable.push(format!(
                    "{action}:{name}.* ({} nested key(s)) — required(\"{action}\") supplies no \
                     `{name}` object for the probe to vary",
                    children.len()
                ));
                continue;
            };

            for (child, child_v) in children {
                if child_v["description"].as_str().is_none() {
                    continue;
                }
                // `accepts_any_json` addresses a nested key by its dotted path. Widening the
                // sweep without widening its escape leaves a site unable to declare a
                // blindness it has — and the first two keys recursion reached, `doc`'s
                // `augment.params` and `augment.params_schema` (both `Option<Value>`, so no
                // value is ill-typed for them), would have read as IC-15 defects the probe
                // simply cannot speak for.
                let path = format!("{name}.{child}");
                if spec.accepts_any_json.contains(&path.as_str()) {
                    // A key labelled for several actions reaches here once per action.
                    if !out.skipped_any_json.contains(&path) {
                        out.skipped_any_json.push(path);
                    }
                    continue;
                }
                // One level, declared rather than assumed: a child that declares its own
                // `properties` is a floor this sweep does not reach. Saying so is the whole
                // point — an undeclared depth limit is the defect that produced `unprobeable`.
                //
                // **Counted by describable grandchildren, not by the object's existence.** A
                // grandchild with no description is skipped at every level and belongs to
                // `every_property_has_a_description`, a different guard — reporting it here
                // would fire on `doc`'s real `event.source` (three bare `{"type": …}` entries)
                // where no coverage is at stake. An alarm that cries on a case with nothing to
                // act on is one somebody eventually silences.
                let deep = child_v["properties"]
                    .as_object()
                    .map(|g| g.values().filter(|v| v["description"].is_string()).count())
                    .unwrap_or(0);
                if deep > 0 {
                    out.unprobeable.push(format!(
                        "{action}:{name}.{child}.* ({deep} described key(s)) — nested deeper \
                         than one level"
                    ));
                }

                let child_declared = child_v["type"].as_str().unwrap_or("string");
                let mut parent = parent_base.clone();
                parent.insert(child.clone(), ill_typed(child_declared));

                let mut probe_args = base_args.clone();
                probe_args.insert(name.clone(), Value::Object(parent));
                let probed = call(Value::Object(probe_args)).await;

                if outcome(&base) == outcome(&probed) {
                    out.unhonored.push(format!(
                        "{action}:{name}.{child} (declared {child_declared})"
                    ));
                }
                out.checked += 1;
            }
        }
        if !matched {
            out.unlabelled.push(name.clone());
        }
    }

    out
}

/// `sweep` plus the three assertions every call site owes.
///
/// The `floor` is not a target. It exists because `unhonored.is_empty()` is **monotone
/// under the label convention breaking**: if `<action>:` prefixes were renamed away, every
/// key would be skipped, `unhonored` would be empty, and the test would pass while
/// checking nothing — the exact failure mode it is here to prevent.
///
/// **`floor` counts action/key PAIRS, not keys**, and a call site's value is measured
/// rather than chosen — see each site's comment for the reading and its date. A floor
/// carrying a per-key figure is satisfied by a sweep that lost every shared key's later
/// actions, which is the defect
/// `docs/issues/archive/2026-09-09-param-probe-checks-one-action-per-shared-key.md`
/// records: the
/// margin between floor and count is exactly the number of labels that can go missing in
/// silence, so these are set at the measurement and a schema shrink must move them
/// deliberately.
///
/// **The `unprobeable` assertion is not a duplicate of the floor.** The floor catches the
/// sweep covering *fewer pairs than it used to*; `unprobeable` catches it covering fewer
/// than it *claims to right now* — a nested object it descended into and could not vary.
/// A floor cannot see that, because a sweep that skips a nested object silently never had
/// those pairs in its count to begin with, which is how nine keys left guard reach without
/// moving a single number.
///
/// **`unlabelled_pin` is the third assertion, and it is a ledger rather than a floor.** A key
/// whose label matches no dispatched action is probed by nothing, and drops out of `checked`
/// and `unhonored` together — so neither assertion above can see it go, and `floor` can only
/// notice once enough of them have. The pin lists those keys by name and is compared for
/// exact set equality: a key that newly loses its label reds, and so does a pinned key that
/// has since been labelled, so the list can neither grow nor rot in silence. A non-empty pin
/// is an honest record of unguarded keys, not an approval of them — clearing it is the
/// work. `accepts_any_json` is reconciled the same way, against the keys the sweep actually
/// skipped for it, so an admission naming a key the schema no longer carries is caught.
pub(crate) async fn assert_all_honored<F, Fut>(
    tool: &str,
    schema: &Value,
    spec: &Spec<'_>,
    floor: usize,
    unlabelled_pin: &[&str],
    call: F,
) where
    F: Fn(Value) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<Value>>,
{
    let Sweep {
        checked,
        unhonored,
        unprobeable,
        skipped_any_json,
        unlabelled,
    } = sweep(schema, spec, call).await;
    let coverage = format!(
        "coverage: {checked} pair(s) probed, {} key(s) skipped as accepts_any_json, {} key(s) \
         skipped as unlabelled",
        skipped_any_json.len(),
        unlabelled.len()
    );
    assert!(
        unhonored.is_empty(),
        "{tool}: these schema keys are labelled for an action whose Args has no such \
         field, so serde discards them silently — the shape of IC-15. Either add the \
         field or move the guidance off the key: {unhonored:?} ({coverage})"
    );
    assert!(
        unprobeable.is_empty(),
        "{tool}: the sweep reached these nested schema keys and could not probe them, so \
         they are advertised but unguarded. Fix it in this file, not in param_probe: give \
         `required(<action>)` a type-valid parent object for each, or flatten the key out \
         of the nested object. Leaving them here is the IC-14 shape — a guard narrower \
         than its name: {unprobeable:?}"
    );
    let sorted = |mut v: Vec<String>| {
        v.sort();
        v
    };
    let (got, want) = (
        sorted(unlabelled),
        sorted(unlabelled_pin.iter().map(|s| s.to_string()).collect()),
    );
    assert!(
        got == want,
        "{tool}: the keys the sweep left UNPROBED for want of an `<action>:` label changed. \
         Newly unlabelled (lost or never had a label — relabel, or pin them here if the \
         key is genuinely not per-action): {:?}. Pinned but now labelled (drop from the \
         pin): {:?}. A key with no matching label is dropped from `checked` and \
         `unhonored` alike, so this pin is the only place its absence is visible ({coverage})",
        got.iter().filter(|k| !want.contains(k)).collect::<Vec<_>>(),
        want.iter().filter(|k| !got.contains(k)).collect::<Vec<_>>(),
    );
    let (got_any, want_any) = (
        sorted(skipped_any_json),
        sorted(
            spec.accepts_any_json
                .iter()
                .map(|s| s.to_string())
                .collect(),
        ),
    );
    assert!(
        got_any == want_any,
        "{tool}: `accepts_any_json` and the keys the sweep actually skipped for it disagree — \
         an admission that names no key the sweep reached is stale. Declared: {want_any:?}; \
         skipped: {got_any:?} ({coverage})"
    );
    assert!(
        checked >= floor,
        "{tool}: expected the sweep to cover at least {floor} labelled keys, covered \
         {checked} — the `<action>:` label convention may have changed, which would make \
         this test silently stop checking ({coverage})"
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in tool: every action fails *after* deserialisation (so a base call is a
    /// valid baseline, per `Spec::required`'s contract), and every action type-checks
    /// `id` **except `beta`**, which discards it the way serde discards a field its
    /// `Args` does not carry.
    async fn honours_id_except_beta(args: Value) -> anyhow::Result<Value> {
        let action = args["action"].as_str().unwrap_or_default().to_string();
        if action != "beta" {
            if let Some(v) = args.get("id") {
                if !v.is_string() {
                    anyhow::bail!("{action}: id must be a string");
                }
            }
        }
        anyhow::bail!("{action}: resolution failed")
    }

    /// The nested counterpart of `honours_id_except_beta`: the `event` object and its `kind`
    /// child are type-checked, and `bogus` is **discarded** — serde's behaviour for a schema
    /// key the `Args` does not carry, one level down.
    ///
    /// The parent's own type check is load-bearing, not scenery: without it the top-level
    /// `event` probe (ill-typed to `[]`) would fall through to the same baseline error and be
    /// reported unhonored, so the fixture would red on a key that is fine.
    async fn honours_event_kind_only(args: Value) -> anyhow::Result<Value> {
        if let Some(ev) = args.get("event") {
            if !ev.is_object() {
                anyhow::bail!("event must be an object");
            }
            if let Some(v) = ev.get("kind") {
                if !v.is_string() {
                    anyhow::bail!("event.kind must be a string");
                }
            }
        }
        anyhow::bail!("resolution failed")
    }

    fn spec(actions: &'static [&'static str]) -> Spec<'static> {
        Spec {
            actions,
            accepts_any_json: &[],
            required: |_| Map::new(),
        }
    }

    /// As `spec`, but `required` hands back a type-valid `event` parent — the shape a real
    /// call site's `probe_required` supplies and the one the nested probe varies.
    fn spec_with_event_parent() -> Spec<'static> {
        Spec {
            actions: &["event_create"],
            accepts_any_json: &[],
            required: |_| {
                let mut m = Map::new();
                m.insert("event".into(), json!({"kind": "note"}));
                m
            },
        }
    }

    fn event_schema(children: Value) -> Value {
        json!({
            "properties": {
                "event": {
                    "type": "object",
                    "description": "event_create: the event",
                    "properties": children
                }
            }
        })
    }

    /// `beta` is the SECOND slash token, and that placement is the whole test: a fixture
    /// whose *first* action drops the key passes against the bug this guards. Reorder the
    /// label to `beta/alpha` and this test still passes while discriminating nothing.
    #[tokio::test]
    async fn a_key_labelled_for_several_actions_is_probed_for_every_one() {
        let schema = json!({
            "properties": {
                "id": {"type": "string", "description": "alpha/beta: the id"}
            }
        });

        let Sweep { unhonored, .. } =
            sweep(&schema, &spec(&["alpha", "beta"]), honours_id_except_beta).await;

        assert_eq!(
            unhonored,
            vec!["beta:id (declared string)".to_string()],
            "a key labelled for two actions must be probed for both; only the action \
             named first in the label was swept"
        );
    }

    /// `checked` is what every call site's `floor` is compared against, so a count of
    /// keys rather than action/key pairs leaves the floor unable to see a lost label.
    #[tokio::test]
    async fn checked_counts_action_key_pairs_not_keys() {
        let schema = json!({
            "properties": {
                "id": {"type": "string", "description": "alpha/gamma/delta: the id"}
            }
        });

        let Sweep {
            checked, unhonored, ..
        } = sweep(
            &schema,
            &spec(&["alpha", "gamma", "delta"]),
            honours_id_except_beta,
        )
        .await;

        assert!(unhonored.is_empty(), "all three honour `id`: {unhonored:?}");
        assert_eq!(
            checked, 3,
            "one key labelled for three actions is three action/key pairs, not one"
        );
    }
    /// A schema with one skipped key of every kind the sweep has a way to skip, plus one
    /// probed key and the `action` dispatch key (which is neither probed nor a skip).
    fn schema_with_one_skip_of_each_kind() -> Value {
        json!({
            "properties": {
                "action": {"type": "string", "enum": ["alpha"]},
                "id": {"type": "string", "description": "alpha/zeta: one token matches"},
                "blob": {"type": "string", "description": "alpha: an opaque value"},
                "nodesc": {"type": "string"},
                "prose": {"type": "string", "description": "For action='alpha': prose label"},
                "ghost": {"type": "string", "description": "zeta: names no dispatched action"}
            }
        })
    }

    fn spec_admitting_blob() -> Spec<'static> {
        Spec {
            actions: &["alpha"],
            accepts_any_json: &["blob"],
            required: |_| Map::new(),
        }
    }

    /// The residue `IC-15`'s parent bug named: a key skipped for `accepts_any_json` or for
    /// carrying no `<action>:` label dropped out of `checked` and `unhonored` alike, so the
    /// sweep read as clean over a population it had quietly shrunk. Each kind is reported
    /// under its own name, and `action` plus a key with ONE matching label token are not
    /// skips at all.
    #[tokio::test]
    async fn skipped_keys_are_reported_by_kind() {
        let Sweep {
            checked,
            unhonored,
            mut unlabelled,
            skipped_any_json,
            ..
        } = sweep(
            &schema_with_one_skip_of_each_kind(),
            &spec_admitting_blob(),
            honours_id_except_beta,
        )
        .await;

        unlabelled.sort();
        assert_eq!(
            unlabelled,
            ["ghost", "nodesc", "prose"],
            "a key with no description, a prose label and a label naming no dispatched action \
             are each probed by nothing and must each be reported"
        );
        assert_eq!(skipped_any_json, ["blob"]);
        assert!(unhonored.is_empty(), "{unhonored:?}");
        assert_eq!(
            checked, 1,
            "`id` is the only probed pair: its `alpha` token matches although `zeta` does not"
        );
    }

    /// Positive twin of `skipped_keys_are_reported_by_kind`: a fully labelled schema with no
    /// admissions reports no skips, so the fields are not simply always non-empty.
    #[tokio::test]
    async fn a_fully_labelled_schema_reports_no_skipped_keys() {
        let schema = json!({
            "properties": {
                "action": {"type": "string"},
                "id": {"type": "string", "description": "alpha/zeta: one token matches"}
            }
        });

        let Sweep {
            checked,
            unlabelled,
            skipped_any_json,
            ..
        } = sweep(&schema, &spec(&["alpha"]), honours_id_except_beta).await;

        assert!(unlabelled.is_empty(), "{unlabelled:?}");
        assert!(skipped_any_json.is_empty(), "{skipped_any_json:?}");
        assert_eq!(checked, 1);
    }

    /// A nested `accepts_any_json` path is a skip too, and is reported once even though the
    /// key is labelled for two actions and so reaches the check twice.
    #[tokio::test]
    async fn a_nested_any_json_skip_is_reported_once() {
        let schema = json!({
            "properties": {
                "event": {
                    "type": "object",
                    "description": "alpha/beta: the event",
                    "properties": {
                        "kind": {"type": "string", "description": "event kind"},
                        "payload": {"type": "object", "description": "opaque payload"}
                    }
                }
            }
        });
        let spec = Spec {
            actions: &["alpha", "beta"],
            accepts_any_json: &["event.payload"],
            required: |_| {
                let mut m = Map::new();
                m.insert("event".into(), json!({"kind": "note"}));
                m
            },
        };

        let Sweep {
            skipped_any_json, ..
        } = sweep(&schema, &spec, honours_event_kind_only).await;

        assert_eq!(skipped_any_json, ["event.payload"]);
    }

    /// The entry point every call site uses, with a pin that matches the sweep: passes.
    #[tokio::test]
    async fn assert_all_honored_accepts_a_matching_pin() {
        assert_all_honored(
            "fixture",
            &schema_with_one_skip_of_each_kind(),
            &spec_admitting_blob(),
            1,
            &["nodesc", "prose", "ghost"],
            honours_id_except_beta,
        )
        .await;
    }

    /// Negative control: a key that loses its label while the pin still says it had one must
    /// red. Without this, `unlabelled` is a number that is printed and never compared.
    #[tokio::test]
    #[should_panic(expected = "per-action): [\"ghost\"]. Pinned")]
    async fn assert_all_honored_rejects_a_newly_unlabelled_key() {
        assert_all_honored(
            "fixture",
            &schema_with_one_skip_of_each_kind(),
            &spec_admitting_blob(),
            1,
            &["nodesc", "prose"], // `ghost` is skipped but not pinned
            honours_id_except_beta,
        )
        .await;
    }

    /// The other direction: a pin naming a key the sweep now probes is a stale pin, and a
    /// stale pin is how a ledger stops being one.
    #[tokio::test]
    #[should_panic(expected = "the pin): [\"id\"]. A key")]
    async fn assert_all_honored_rejects_a_stale_pin() {
        assert_all_honored(
            "fixture",
            &schema_with_one_skip_of_each_kind(),
            &spec_admitting_blob(),
            1,
            &["nodesc", "prose", "ghost", "id"], // `id` is labelled and probed
            honours_id_except_beta,
        )
        .await;
    }

    /// An `accepts_any_json` entry naming a key the sweep never skipped is a stale admission.
    #[tokio::test]
    #[should_panic(expected = "`accepts_any_json` and the keys the sweep actually skipped")]
    async fn assert_all_honored_rejects_a_stale_admission() {
        let spec = Spec {
            actions: &["alpha"],
            accepts_any_json: &["blob", "removed_long_ago"],
            required: |_| Map::new(),
        };
        assert_all_honored(
            "fixture",
            &schema_with_one_skip_of_each_kind(),
            &spec,
            1,
            &["nodesc", "prose", "ghost"],
            honours_id_except_beta,
        )
        .await;
    }

    /// The reproduction from
    /// `docs/issues/archive/2026-09-02-param-probe-does-not-recurse-so-nesting-a-key-removes-it-from-guard-reach.md`,
    /// inverted into a guard. `bogus` is unhonored and nested; a sweep that walks one level
    /// reports nothing and passes, which is how nine real keys left guard reach in silence.
    ///
    /// **`kind` is in the fixture to pin the inheritance rule, not for symmetry.** Neither
    /// child carries an `<action>:` label — the real ones do not either (`"event author"`,
    /// `"event kind"`) — so a recursion that scanned children for their own labels would
    /// match nothing and stay green on `bogus`. Both children being probed is what shows the
    /// action came from the parent.
    #[tokio::test]
    async fn a_nested_key_is_probed_under_its_parents_action() {
        let schema = event_schema(json!({
            "kind": {"type": "string", "description": "event kind"},
            "bogus": {"type": "string", "description": "declared but absent from Args"}
        }));

        let Sweep {
            checked,
            unhonored,
            unprobeable,
            ..
        } = sweep(&schema, &spec_with_event_parent(), honours_event_kind_only).await;

        assert_eq!(
            unhonored,
            vec!["event_create:event.bogus (declared string)".to_string()],
            "a nested key the Args does not carry must be reported; `event.kind` is \
             honoured and `event` itself is type-checked"
        );
        assert_eq!(
            checked, 3,
            "the parent plus both children are three action/key pairs"
        );
        assert!(unprobeable.is_empty(), "{unprobeable:?}");
    }

    /// The sweep's own edge, said out loud. A nested object the call site's `required` does
    /// not supply cannot be varied — adding the object *and* the bogus key changes two things
    /// at once — so the pairs are lost either way. Reporting them is the difference between
    /// this bug and its fix: the count is the same, the silence is not.
    #[tokio::test]
    async fn a_nested_object_absent_from_required_is_reported_unprobeable() {
        let schema = event_schema(json!({
            "kind": {"type": "string", "description": "event kind"},
            "bogus": {"type": "string", "description": "declared but absent from Args"}
        }));

        let Sweep {
            unprobeable,
            unhonored,
            ..
        } = sweep(&schema, &spec(&["event_create"]), honours_event_kind_only).await;

        assert_eq!(
            unprobeable,
            vec![
                "event_create:event.* (2 nested key(s)) — required(\"event_create\") supplies \
                 no `event` object for the probe to vary"
                    .to_string()
            ],
            "a parent the site does not supply must be named, never skipped"
        );
        assert!(
            unhonored.is_empty(),
            "unreachable is not the same claim as unhonored: {unhonored:?}"
        );
    }

    #[tokio::test]
    /// The recursion is one level, and the limit is declared rather than assumed. `doc`'s
    /// real schema has exactly this shape — `event.source` is an object with its own
    /// `properties` — so this is the live case, not a hypothetical.
    ///
    /// **`uri` carrying a description is the load-bearing detail.** The report is counted by
    /// *describable* grandchildren, because a bare `{"type": …}` grandchild is skipped at
    /// every level and loses no coverage — which is why the real `event.source` is silent
    /// here. Drop the description from this fixture and the test passes while asserting
    /// nothing.
    async fn a_child_object_deeper_than_one_level_is_reported_unprobeable() {
        let schema = event_schema(json!({
            "kind": {"type": "string", "description": "event kind"},
            "source": {
                "type": "object",
                "description": "external signal source",
                "properties": {"uri": {"type": "string", "description": "the uri"}}
            }
        }));

        let Sweep { unprobeable, .. } =
            sweep(&schema, &spec_with_event_parent(), honours_event_kind_only).await;

        assert_eq!(
            unprobeable,
            vec![
                "event_create:event.source.* (1 described key(s)) — nested deeper than one \
                 level"
                    .to_string()
            ],
            "a grandchild the sweep does not reach must say so; an undeclared depth limit \
             is the defect this field exists for"
        );
    }

    /// The other half of the rule above, and the one that keeps the alarm worth reading: a
    /// grandchild-bearing object whose grandchildren are all bare `{"type": …}` reports
    /// nothing, because nothing was probeable there to begin with. This is `doc`'s real
    /// `event.source`, copied shape-for-shape — the case that made the first cut of this
    /// report fire where no coverage was at stake.
    #[tokio::test]
    async fn a_child_object_with_no_described_grandchildren_is_silent() {
        let schema = event_schema(json!({
            "kind": {"type": "string", "description": "event kind"},
            "source": {
                "type": "object",
                "description": "external signal source",
                "properties": {
                    "uri": {"type": "string"},
                    "kind": {"type": "string"},
                    "payload": {}
                }
            }
        }));

        let Sweep { unprobeable, .. } =
            sweep(&schema, &spec_with_event_parent(), honours_event_kind_only).await;

        assert!(
            unprobeable.is_empty(),
            "no described grandchild means no lost coverage, so the depth limit has \
             nothing to report: {unprobeable:?}"
        );
    }
}
