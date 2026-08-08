use anyhow::Result;
use serde::{Deserialize, Serialize};

/// A document's YAML frontmatter.
///
/// Every field carries BOTH `#[serde(default)]` (absent key → empty on read)
/// AND a `skip_serializing_if` (empty → absent key on write). The pair is what
/// makes a parse→edit→write round-trip idempotent: with `default` alone, an
/// unset `Option` came back out as an explicit `null` and an empty `Vec` as
/// `[]`, so any update churned the frontmatter of every file it touched with
/// keys the author never wrote. See
/// `docs/issues/2026-07-13-artifact-update-frontmatter-null-churn.md`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Frontmatter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owners: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_scope: Option<String>,
    /// Custom / unrecognized frontmatter keys, captured verbatim so they
    /// survive a parse→edit→write round-trip (otherwise an update would
    /// silently drop them). Not catalog-indexed — not filterable via
    /// artifact(find); readable on disk and surfaced by artifact(get) as
    /// `extra`.
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// The frontmatter keys [`Frontmatter`] models as typed fields.
///
/// Kept beside the struct so the two cannot drift: adding a field above without
/// adding it here re-opens the hole below.
///
/// **Why this list has to exist.** `extra` is `#[serde(flatten)]`, so on the PARSE
/// side serde routes any of these keys to its typed field — `extra` can never
/// legitimately hold one. Nothing enforced the same on the WRITE side, where
/// [`write`] emits the typed fields via `serde_yml` and then appends each `extra`
/// pair as a raw line. An `extra` entry named `kind` therefore emitted a **second**
/// `kind:` line into the same mapping, and a duplicate key makes the whole block
/// unparseable — at which point the artifact loses its kind, status, title, owners
/// and tags in one step and silently drops out of its own ledger.
///
/// See `docs/issues/2026-08-08-artifact-extra-key-collision-unclassifies-silently.md`.
pub const RESERVED_KEYS: &[&str] = &[
    "id",
    "kind",
    "status",
    "title",
    "owners",
    "tags",
    "topic",
    "time_scope",
];

/// Reserved keys present in an `extra` map, in [`RESERVED_KEYS`] order.
///
/// Empty is the healthy case. A non-empty result is always a caller error — see
/// [`RESERVED_KEYS`] for why parsing can never produce one.
pub fn reserved_keys_in_extra(
    extra: &std::collections::BTreeMap<String, serde_json::Value>,
) -> Vec<&'static str> {
    RESERVED_KEYS
        .iter()
        .copied()
        .filter(|k| extra.contains_key(*k))
        .collect()
}

pub fn parse(doc: &str) -> Result<(Option<Frontmatter>, &str)> {
    let looks_like_fm = doc.starts_with("---\n") || doc.starts_with("---\r\n");
    if !looks_like_fm {
        return Ok((None, doc));
    }
    let after_open = if doc.starts_with("---\r\n") { 5 } else { 4 };
    let rest = &doc[after_open..];
    let mut idx = 0usize;
    let mut close = None;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "---" {
            close = Some((idx, idx + line.len()));
            break;
        }
        idx += line.len();
    }
    let (yaml_end, body_start) =
        close.ok_or_else(|| anyhow::anyhow!("frontmatter missing closing `---`"))?;
    let yaml_src = &rest[..yaml_end];
    let fm: Frontmatter = serde_yml::from_str(yaml_src)
        .map_err(|e| anyhow::anyhow!("malformed frontmatter YAML: {e}"))?;
    Ok((Some(fm), &rest[body_start..]))
}

pub fn write(fm: &Frontmatter, body: &str) -> String {
    // Serialize the first-class fields with serde_yml, but emit the `extra` map
    // ourselves. serde_yml conservatively quotes any string YAML 1.1 could
    // reinterpret — including ISO dates like `2026-07-17` → `'2026-07-17'`. The
    // `extra` values are consumed as raw text by downstream regex readers (e.g.
    // the SessionStart next-sweep-due nudge tests `/^\d{4}-\d{2}-\d{2}$/`), which
    // a quoted scalar silently fails. Emit each extra scalar bare when — and only
    // when — doing so round-trips as the identical string, so we shed spurious
    // quotes without ever turning a `"30"`/`"true"` string into a number/bool.
    // See docs/issues/2026-07-17-artifact-extra-quotes-frontmatter-scalars.md.
    let mut first_class = fm.clone();
    let extra = std::mem::take(&mut first_class.extra);
    let mut yaml = serde_yml::to_string(&first_class).expect("frontmatter serializes");
    // An all-unset struct serializes to the empty-map token; drop it so only the
    // extra keys follow (matches the flatten behaviour for extra-only frontmatter).
    if yaml.trim() == "{}" {
        yaml.clear();
    }
    for (k, v) in &extra {
        // A reserved key here would emit a SECOND line for a key `serde_yml` already
        // wrote above, and a duplicate key makes the entire block unparseable — which
        // costs every field, not just this one. Dropping it is the safe direction: the
        // typed field above already carries that key with the catalog-indexed value.
        // Callers are refused at the input boundary (`artifact(create|update)`); this is
        // the backstop for internal ones, and it is why `write` can stay infallible.
        if RESERVED_KEYS.contains(&k.as_str()) {
            continue;
        }
        match v {
            serde_json::Value::String(s) if scalar_can_be_bare(s) => {
                yaml.push_str(&format!("{k}: {s}\n"));
            }
            _ => {
                // Delegate this single pair to serde_yml — its quoting for
                // type-ambiguous scalars and its block style for arrays / nested
                // maps are preserved exactly as before.
                let mut pair = serde_json::Map::new();
                pair.insert(k.clone(), v.clone());
                let rendered = serde_yml::to_string(&serde_json::Value::Object(pair))
                    .expect("frontmatter pair serializes");
                yaml.push_str(&rendered);
            }
        }
    }
    format!("---\n{yaml}---\n{body}")
}

/// Whether a string scalar can be emitted WITHOUT quotes on a `key: value`
/// frontmatter line. Two conditions must hold: it is structurally safe on a
/// single plain-scalar line (no newline, no YAML-significant punctuation, no
/// leading indicator char), AND it round-trips through the YAML parser as the
/// identical string. The round-trip check is what keeps type-ambiguous strings
/// (`"30"`, `"true"`, `"~"`) quoted — bare, YAML would resolve them to a
/// number / bool / null — while letting genuinely string-valued scalars such as
/// ISO dates (`2026-07-17`) stay bare, since the core schema resolves them to
/// strings anyway.
fn scalar_can_be_bare(s: &str) -> bool {
    if s.is_empty()
        || s != s.trim()
        || s.contains('\n')
        || s.contains(':')
        || s.contains('#')
        || s.contains('"')
        || s.contains('\'')
        || s.starts_with([
            '[', '{', '*', '?', '&', '!', '|', '>', '@', '`', '-', ',', ' ',
        ])
    {
        return false;
    }
    serde_yml::from_str::<serde_json::Value>(s)
        .map(|parsed| parsed == serde_json::Value::String(s.to_string()))
        .unwrap_or(false)
}

pub fn update_in_place(doc: &str, edit: impl FnOnce(&mut Frontmatter)) -> Result<String> {
    let (fm_opt, body) = parse(doc)?;
    let mut fm = fm_opt.unwrap_or_default();
    edit(&mut fm);
    Ok(write(&fm, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_frontmatter() {
        let doc = "---\nkind: spec\nstatus: active\ntitle: Example\n---\n\nBody here\n";
        let (fm, body) = parse(doc).unwrap();
        let fm = fm.expect("frontmatter present");
        assert_eq!(fm.kind.as_deref(), Some("spec"));
        assert_eq!(fm.status.as_deref(), Some("active"));
        assert_eq!(fm.title.as_deref(), Some("Example"));
        assert_eq!(body, "\nBody here\n");
    }

    #[test]
    fn returns_none_for_no_frontmatter() {
        let doc = "# just a heading\n\nbody\n";
        let (fm, body) = parse(doc).unwrap();
        assert!(fm.is_none());
        assert_eq!(body, doc);
    }

    #[test]
    fn handles_trailing_crlf() {
        let doc = "---\r\nkind: plan\r\n---\r\n\r\nbody\r\n";
        let (fm, _) = parse(doc).unwrap();
        assert_eq!(fm.unwrap().kind.as_deref(), Some("plan"));
    }

    #[test]
    fn rejects_missing_closing_delimiter() {
        let doc = "---\nkind: spec\n\nbody without close\n";
        let err = parse(doc).unwrap_err();
        assert!(err.to_string().to_lowercase().contains("closing"));
    }

    #[test]
    fn rejects_malformed_yaml() {
        let doc = "---\nkind: [unclosed\n---\nbody\n";
        assert!(parse(doc).is_err());
    }

    #[test]
    fn round_trip_preserves_body() {
        let fm = Frontmatter {
            kind: Some("spec".into()),
            status: Some("active".into()),
            title: Some("X".into()),
            ..Default::default()
        };
        let body = "\nBody text\n";
        let doc = write(&fm, body);
        let (parsed, parsed_body) = parse(&doc).unwrap();
        assert_eq!(parsed.unwrap(), fm);
        assert_eq!(parsed_body, body);
    }

    #[test]
    fn captures_unknown_fields_into_extra() {
        // Custom keys must be captured (not dropped) so updates can't silently
        // wipe them; first-class fields still deserialize normally.
        let doc = "---\nkind: spec\norigin_session_id: abc123\nbranch: feature/x\n---\nbody\n";
        let (fm, _) = parse(doc).unwrap();
        let fm = fm.unwrap();
        assert_eq!(fm.kind.as_deref(), Some("spec"));
        assert_eq!(
            fm.extra.get("origin_session_id"),
            Some(&serde_json::json!("abc123"))
        );
        assert_eq!(
            fm.extra.get("branch"),
            Some(&serde_json::json!("feature/x"))
        );
    }

    #[test]
    fn round_trip_preserves_extra() {
        let mut extra = std::collections::BTreeMap::new();
        extra.insert("origin_session_id".to_string(), serde_json::json!("abc123"));
        extra.insert("branch".to_string(), serde_json::json!("feature/x"));
        let fm = Frontmatter {
            kind: Some("tracker".into()),
            extra,
            ..Default::default()
        };
        let doc = write(&fm, "\nbody\n");
        let (parsed, _) = parse(&doc).unwrap();
        let parsed = parsed.unwrap();
        assert_eq!(
            parsed.extra.get("origin_session_id"),
            fm.extra.get("origin_session_id")
        );
        assert_eq!(parsed.extra.get("branch"), fm.extra.get("branch"));
        // And the first-class fields survive alongside the flattened map.
        assert_eq!(parsed.kind.as_deref(), Some("tracker"));
    }

    #[test]
    fn a_reserved_key_in_extra_is_not_emitted_twice() {
        // The defect: `extra` carrying a key the struct already models emitted that key
        // a SECOND time into the same mapping. A duplicate key makes the whole block
        // unparseable, so the artifact lost kind, status, title, owners and tags
        // together and silently left its own ledger — one live casualty
        // (`docs/issues/2026-08-08-edit-file-out-of-project-ack-handle-unresolvable.md`),
        // invisible for a working day because the file reads fine to a human.
        let mut extra = std::collections::BTreeMap::new();
        extra.insert("kind".to_string(), serde_json::json!("bug"));
        extra.insert("severity".to_string(), serde_json::json!("low"));
        let fm = Frontmatter {
            kind: Some("bug".into()),
            status: Some("open".into()),
            extra,
            ..Default::default()
        };

        let doc = write(&fm, "\nbody\n");
        assert_eq!(
            doc.matches("kind:").count(),
            1,
            "`kind:` emitted more than once:\n{doc}"
        );

        // The assertion that actually matters. The count above only says the symptom is
        // gone; this says the document still means something. A duplicate key is not a
        // cosmetic blemish — it costs every field in the block at once.
        let (parsed, body) = parse(&doc).expect("emitted frontmatter must re-parse");
        let parsed = parsed.expect("frontmatter present");
        assert_eq!(parsed.kind.as_deref(), Some("bug"));
        assert_eq!(parsed.status.as_deref(), Some("open"));
        assert_eq!(
            parsed.extra.get("severity"),
            Some(&serde_json::json!("low")),
            "a non-reserved extra key must still survive the round trip"
        );
        assert!(
            !parsed.extra.contains_key("kind"),
            "parse routes a modelled key to its typed field, never to `extra`"
        );
        assert_eq!(body, "\nbody\n");
    }

    #[test]
    fn reserved_keys_in_extra_reports_every_clash_and_nothing_else() {
        let mut extra = std::collections::BTreeMap::new();
        extra.insert("severity".to_string(), serde_json::json!("low"));
        assert!(
            reserved_keys_in_extra(&extra).is_empty(),
            "a custom key is what `extra` is for"
        );

        extra.insert("kind".to_string(), serde_json::json!("bug"));
        extra.insert("owners".to_string(), serde_json::json!(["marius"]));
        assert_eq!(reserved_keys_in_extra(&extra), vec!["kind", "owners"]);
    }

    #[test]
    fn extra_scalar_date_is_emitted_bare_not_quoted() {
        // BUG (docs/issues/2026-07-17-artifact-extra-quotes-frontmatter-scalars.md):
        // serde_yml quoted date-like extra scalars ('2026-07-17'), breaking raw-text
        // regex consumers (the SessionStart next-sweep-due nudge tests
        // /^\d{4}-\d{2}-\d{2}$/). Date-like and plain strings must serialize BARE;
        // genuinely type-ambiguous strings (number/bool-like) stay quoted so their
        // string type still round-trips.
        let mut extra = std::collections::BTreeMap::new();
        extra.insert(
            "next-sweep-due".to_string(),
            serde_json::json!("2026-07-17"),
        );
        extra.insert("sweep-interval-days".to_string(), serde_json::json!(30));
        extra.insert("origin_session_id".to_string(), serde_json::json!("abc123"));
        extra.insert("numish".to_string(), serde_json::json!("30")); // string, looks numeric
        extra.insert("boolish".to_string(), serde_json::json!("true")); // string, looks bool
        let fm = Frontmatter {
            kind: Some("tracker".into()),
            extra,
            ..Default::default()
        };
        let doc = write(&fm, "\nbody\n");

        // Date-like + plain strings emit bare (the fix):
        assert!(
            doc.contains("next-sweep-due: 2026-07-17\n"),
            "date-like extra scalar must be bare, not quoted: {doc}"
        );
        assert!(
            doc.contains("origin_session_id: abc123\n"),
            "plain string extra scalar must be bare: {doc}"
        );
        // A real number is unchanged (still a bare number):
        assert!(
            doc.contains("sweep-interval-days: 30\n"),
            "numeric value must stay a bare number: {doc}"
        );
        // Type-ambiguous STRINGS stay quoted so they never round-trip as number/bool:
        assert!(
            doc.contains("numish: '30'") || doc.contains("numish: \"30\""),
            "numeric-looking string must stay quoted: {doc}"
        );
        assert!(
            doc.contains("boolish: 'true'") || doc.contains("boolish: \"true\""),
            "bool-looking string must stay quoted: {doc}"
        );

        // Idempotent round-trip: parse back yields the identical Frontmatter —
        // every extra value keeps its original type.
        let (parsed, _) = parse(&doc).unwrap();
        assert_eq!(
            parsed.unwrap(),
            fm,
            "round-trip must preserve every extra value's type"
        );
    }

    #[test]
    fn update_in_place_preserves_untouched_fields() {
        let doc = "---\nkind: spec\nstatus: draft\ntitle: Original\n---\n\nbody\n";
        let updated = update_in_place(doc, |fm| {
            fm.status = Some("active".into());
        })
        .unwrap();
        assert!(updated.contains("status: active"));
        assert!(updated.contains("title: Original"));
        assert!(updated.ends_with("\nbody\n"));
    }

    #[test]
    fn update_in_place_inserts_frontmatter_if_absent() {
        let doc = "# Heading\n\nbody\n";
        let updated = update_in_place(doc, |fm| {
            fm.kind = Some("doc".into());
        })
        .unwrap();
        assert!(updated.starts_with("---\n"));
        assert!(updated.contains("kind: doc"));
        assert!(updated.ends_with("# Heading\n\nbody\n"));
    }

    #[test]
    fn write_omits_unset_optional_fields_instead_of_emitting_null() {
        // Regression: `#[serde(default)]` governs deserialization only, so an
        // unset Option round-tripped back out as an explicit `null`, churning
        // the frontmatter of every file an update touched.
        // See docs/issues/2026-07-13-artifact-update-frontmatter-null-churn.md.
        let fm = Frontmatter {
            kind: Some("bug".into()),
            status: Some("open".into()),
            ..Default::default()
        };
        let doc = write(&fm, "\nbody\n");
        for absent in ["id", "title", "topic", "time_scope"] {
            assert!(
                !doc.contains(&format!("{absent}: null")),
                "unset `{absent}` must be omitted, not emitted as null; got:\n{doc}"
            );
        }
        assert!(doc.contains("kind: bug"));
        assert!(doc.contains("status: open"));
    }

    #[test]
    fn update_in_place_does_not_introduce_null_keys() {
        // A real bug file's shape: no id/title/topic/time_scope keys at all.
        // Flipping `status` must not conjure those keys into existence.
        let doc = "---\nstatus: open\nseverity: medium\nkind: bug\n---\n\nbody\n";
        let updated = update_in_place(doc, |fm| {
            fm.status = Some("fixed".into());
        })
        .unwrap();
        assert!(updated.contains("status: fixed"));
        assert!(
            !updated.contains("null"),
            "update must not introduce explicit nulls; got:\n{updated}"
        );
    }

    #[test]
    fn update_in_place_is_idempotent_after_first_normalization() {
        // Re-serializing through serde cannot preserve the source's key order or
        // inline-vs-block scalar style, so the FIRST update of a hand-written file
        // still normalizes its shape. What must hold is that the normalization is
        // one-time: updating an already-normalized file changes nothing but the
        // field being set. Otherwise every update would re-churn the same file.
        let doc = "---\nstatus: open\nseverity: medium\nkind: bug\ntags: [a, b]\n---\n\nbody\n";
        let once = update_in_place(doc, |fm| fm.status = Some("investigating".into())).unwrap();
        let twice = update_in_place(&once, |fm| fm.status = Some("investigating".into())).unwrap();
        assert_eq!(
            once, twice,
            "a second identical update must be a no-op; churn is not allowed to recur"
        );
    }
}
