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
/// `docs/issues/archive/2026-07-13-artifact-update-frontmatter-null-churn.md`.
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
    /// doc(find); readable on disk and surfaced by doc(get) as
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
/// See `docs/issues/archive/2026-08-08-artifact-extra-key-collision-unclassifies-silently.md`.
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

/// Number of lines that sit ABOVE `body` in `doc` — the frontmatter block's
/// height, and the amount every line number derived from `body` alone is short
/// by.
///
/// **Pass the pair a single [`parse`] call returned.** `parse`'s body is a
/// suffix subslice of the document it parsed, which is the whole derivation:
/// the prefix is everything before it. Handing in a body from one parse and a
/// document from another is the mistake this exists to make hard to commit — so
/// a pair that is NOT a suffix match returns `0` rather than a number computed
/// from unrelated text. That direction is deliberate: `0` reproduces the
/// pre-fix behaviour (ranges short by the frontmatter), whereas a wrong
/// non-zero offset points at a line that exists and is wrong, which reads as
/// correct.
pub fn body_line_offset(doc: &str, body: &str) -> usize {
    if !doc.ends_with(body) {
        return 0;
    }
    // `ends_with` guarantees the split is on a char boundary.
    doc[..doc.len() - body.len()].lines().count()
}

#[cfg(test)]
mod body_line_offset_tests {
    use super::*;

    #[test]
    fn a_document_without_frontmatter_has_no_offset() {
        let doc = "# Title\n\nbody\n";
        let (_, body) = parse(doc).unwrap();
        assert_eq!(body_line_offset(doc, body), 0);
    }

    #[test]
    fn the_offset_is_the_frontmatter_blocks_own_line_count() {
        // LOAD-BEARING: this block is exactly 4 lines (`---`, two keys, `---`),
        // so 4 is the only correct answer. Add or remove a key here and the
        // expected value below must move with it — that coupling is the point.
        // A hard-coded 4 against a block of some other height is what gives
        // this test the ability to fail at all.
        let doc = "---\nkind: tracker\nstatus: active\n---\n# Title\n\nbody\n";
        let (fm, body) = parse(doc).unwrap();
        assert!(
            fm.is_some(),
            "the fixture's frontmatter must actually parse"
        );
        assert_eq!(body, "# Title\n\nbody\n");
        assert_eq!(body_line_offset(doc, body), 4);
    }

    #[test]
    fn a_body_that_is_not_this_documents_body_yields_zero_not_a_wrong_number() {
        // The safe-direction guarantee: a mismatched pair degrades to "no
        // offset", never to an offset computed from a length coincidence.
        // `"ope\n"` is 4 bytes, so a naive `doc.len() - body.len()` would have
        // produced a confident, wrong prefix here instead of refusing.
        let doc = "---\nkind: tracker\n---\n# Title\n";
        assert_eq!(body_line_offset(doc, "ope\n"), 0);
    }

    #[test]
    fn crlf_frontmatter_counts_the_same_lines() {
        // `---\r\n` + one key + `---\r\n` = 3 lines. Pins that `str::lines()`
        // strips the `\r` rather than counting a phantom line.
        let doc = "---\r\nkind: tracker\r\n---\r\n# Title\r\n";
        let (_, body) = parse(doc).unwrap();
        assert_eq!(body_line_offset(doc, body), 3);
    }
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
    // See docs/issues/archive/2026-07-17-artifact-extra-quotes-frontmatter-scalars.md.
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
        // Callers are refused at the input boundary (`doc(create|update)`); this is
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

pub fn rewrite_frontmatter_normalizing(
    doc: &str,
    edit: impl FnOnce(&mut Frontmatter),
) -> Result<String> {
    let (fm_opt, body) = parse(doc)?;
    let mut fm = fm_opt.unwrap_or_default();
    edit(&mut fm);
    Ok(write(&fm, body))
}

/// Swap the frontmatter's top-level `<key>:` line for `value`, leaving every
/// other byte of the document untouched.
///
/// [`rewrite_frontmatter_normalizing`] is the wrong primitive for a one-field
/// write: it re-emits the whole block from the parsed struct, so a hand-authored
/// file comes back requoted, reordered, with flow sequences rendered as block
/// ones and null keys dropped. Worse, a `{Placeholder}` is valid YAML for a
/// *flow mapping*, so an ADR template's `created: {YYYY-MM-DD}` round-trips into
/// `created:\n  YYYY-MM-DD: null` — semantically identical, and no longer a
/// placeholder.
///
/// Measured twice through real callers. `move`: 3.5 changed lines per file on a
/// librarian-written corpus, **30** on a hand-authored one (BL-34). Then
/// `doc(update, patch={status})` on a hostile fixture: one field requested,
/// **seven** lines changed, six unrequested (BL-36). Value-preserving and
/// form-preserving are different properties; a targeted write owes both.
///
/// Returns `None` when there is nothing to splice — no frontmatter, or no
/// top-level `<key>:` line in it. That is a *decline*, not a failure: the caller
/// decides whether to fall back to the normalizing writer or leave the file
/// alone. Both callers do so deliberately, and for opposite reasons — `move`
/// leaves the file alone (a stale id is a broken citation; a reformat is data
/// loss), `update` falls back (the field must be written, and an absent key has
/// no line to replace).
pub fn replace_scalar_line(doc: &str, key: &str, value: &str) -> Option<String> {
    // Same delimiter scan `parse` performs, so the two agree on where the
    // frontmatter ends — a splice that disagreed would edit body text.
    let after_open = if doc.starts_with("---\r\n") {
        5
    } else if doc.starts_with("---\n") {
        4
    } else {
        return None;
    };

    // A value carrying a newline cannot BE a single line, so there is no splice
    // to make. Decline and let the caller choose — same contract as a missing key.
    if value.contains('\n') {
        return None;
    }

    let prefix = format!("{key}:");
    let rest = &doc[after_open..];
    let mut idx = 0usize;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "---" {
            // Closing delimiter with no top-level `<key>:` above it. Any match
            // below is body text.
            return None;
        }
        // `trimmed`, not `line`, and no trim_start: an indented `<key>:` belongs
        // to a nested mapping, not to the artifact.
        if trimmed.starts_with(&prefix) {
            let start = after_open + idx;
            // `trimmed.len()` stops before the line ending, so the original `\n`
            // or `\r\n` is carried through untouched by the tail splice.
            let end = start + trimmed.len();
            let rendered = if scalar_can_be_bare(value) {
                format!("{key}: {value}")
            } else {
                // YAML single-quoted style escapes an apostrophe by doubling it.
                // The id-only ancestor of this function never hit that — ids are
                // hex — but a title or topic can carry one, and emitting it raw
                // would close the quote early and corrupt the whole block.
                format!("{key}: '{}'", value.replace('\'', "''"))
            };
            let mut out = String::with_capacity(doc.len() + rendered.len());
            out.push_str(&doc[..start]);
            out.push_str(&rendered);
            out.push_str(&doc[end..]);
            return Some(out);
        }
        idx += line.len();
    }
    None
}

/// Set a top-level integer `<key>:` line — splicing it in place when the key is
/// already present, inserting it just above the closing delimiter when it is not.
///
/// The insert half is why this is not simply [`replace_scalar_line`]. That function
/// *declines* on a missing key, and its two callers either leave the file alone or
/// fall back to the normalizing writer. Neither option is open to a caller that MUST
/// persist a value into a hand-authored block: falling back reformats it (30 changed
/// lines on a hand-authored corpus, BL-34), and leaving it alone silently drops the
/// write — which for a committed high-water mark is the very regression it exists to
/// prevent.
///
/// Inserting above the closing `---`, rather than adjacent to a sibling key, is
/// deliberate. The natural anchor would be the ledger's `entry_prefix` line, but that
/// key's value may be a BLOCK SEQUENCE — `entry_prefix:` followed by indented `- F`
/// items — and a line spliced directly after it would land inside the sequence and
/// change its meaning.
///
/// Integers need no quoting decision: YAML's core schema resolves a bare integer to a
/// number. So this bypasses [`scalar_can_be_bare`], whose round-trip check would quote
/// `33` on the correct-but-unhelpful grounds that it is not a string.
///
/// Returns `None` only when there is no frontmatter block to write into.
pub fn upsert_int_line(doc: &str, key: &str, value: u64) -> Option<String> {
    // Same delimiter scan `parse` and `replace_scalar_line` perform, so all three
    // agree on where the frontmatter ends.
    let after_open = if doc.starts_with("---\r\n") {
        5
    } else if doc.starts_with("---\n") {
        4
    } else {
        return None;
    };

    let rendered = format!("{key}: {value}");
    let prefix = format!("{key}:");
    let rest = &doc[after_open..];
    let mut idx = 0usize;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "---" {
            // Closing delimiter with no top-level `<key>:` above it — insert here,
            // borrowing this line's own ending so a CRLF file stays CRLF.
            let start = after_open + idx;
            let eol = if line.ends_with("\r\n") { "\r\n" } else { "\n" };
            let mut out = String::with_capacity(doc.len() + rendered.len() + eol.len());
            out.push_str(&doc[..start]);
            out.push_str(&rendered);
            out.push_str(eol);
            out.push_str(&doc[start..]);
            return Some(out);
        }
        // `trimmed`, not `line`, and no trim_start: an indented `<key>:` belongs to a
        // nested mapping or a sequence item, not to the artifact.
        if trimmed.starts_with(&prefix) {
            let start = after_open + idx;
            let end = start + trimmed.len();
            let mut out = String::with_capacity(doc.len() + rendered.len());
            out.push_str(&doc[..start]);
            out.push_str(&rendered);
            out.push_str(&doc[end..]);
            return Some(out);
        }
        idx += line.len();
    }
    None
}

/// Swap the document's body, leaving the frontmatter block byte-identical.
///
/// The twin of [`replace_scalar_line`] for the other half of the file. A body
/// overwrite has no business re-emitting the frontmatter, but
/// `write(&fm, new_body)` does exactly that — measured on a hostile fixture, a
/// `patch={body}` call reformatted six frontmatter lines it was never asked to
/// touch, including expanding `created: {YYYY-MM-DD}` into a nested null. BL-36.
///
/// `new_body` is written verbatim after the closing delimiter's line ending, so
/// the caller owns the leading blank line exactly as it does with [`write`].
/// Returns `None` when the document has no frontmatter block to preserve — there
/// is nothing to protect, and [`write`] is then the right call.
pub fn replace_body(doc: &str, new_body: &str) -> Option<String> {
    let after_open = if doc.starts_with("---\r\n") {
        5
    } else if doc.starts_with("---\n") {
        4
    } else {
        return None;
    };

    let rest = &doc[after_open..];
    let mut idx = 0usize;
    for line in rest.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            // Past the closing delimiter's own line ending: everything before this
            // point is the author's frontmatter, byte for byte.
            let body_start = after_open + idx + line.len();
            let mut out = String::with_capacity(body_start + new_body.len());
            out.push_str(&doc[..body_start]);
            out.push_str(new_body);
            return Some(out);
        }
        idx += line.len();
    }
    None
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
        // (`docs/issues/archive/2026-08-08-edit-file-out-of-project-ack-handle-unresolvable.md`),
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
        // BUG (docs/issues/archive/2026-07-17-artifact-extra-quotes-frontmatter-scalars.md):
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
    fn rewrite_frontmatter_normalizing_preserves_untouched_fields() {
        let doc = "---\nkind: spec\nstatus: draft\ntitle: Original\n---\n\nbody\n";
        let updated = rewrite_frontmatter_normalizing(doc, |fm| {
            fm.status = Some("active".into());
        })
        .unwrap();
        assert!(updated.contains("status: active"));
        assert!(updated.contains("title: Original"));
        assert!(updated.ends_with("\nbody\n"));
    }

    #[test]
    fn rewrite_frontmatter_normalizing_inserts_frontmatter_if_absent() {
        let doc = "# Heading\n\nbody\n";
        let updated = rewrite_frontmatter_normalizing(doc, |fm| {
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
        // See docs/issues/archive/2026-07-13-artifact-update-frontmatter-null-churn.md.
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
    fn rewrite_frontmatter_normalizing_does_not_introduce_null_keys() {
        // A real bug file's shape: no id/title/topic/time_scope keys at all.
        // Flipping `status` must not conjure those keys into existence.
        let doc = "---\nstatus: open\nseverity: medium\nkind: bug\n---\n\nbody\n";
        let updated = rewrite_frontmatter_normalizing(doc, |fm| {
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
    fn rewrite_frontmatter_normalizing_is_idempotent_after_first_pass() {
        // Re-serializing through serde cannot preserve the source's key order or
        // inline-vs-block scalar style, so the FIRST update of a hand-written file
        // still normalizes its shape. What must hold is that the normalization is
        // one-time: updating an already-normalized file changes nothing but the
        // field being set. Otherwise every update would re-churn the same file.
        let doc = "---\nstatus: open\nseverity: medium\nkind: bug\ntags: [a, b]\n---\n\nbody\n";
        let once =
            rewrite_frontmatter_normalizing(doc, |fm| fm.status = Some("investigating".into()))
                .unwrap();
        let twice =
            rewrite_frontmatter_normalizing(&once, |fm| fm.status = Some("investigating".into()))
                .unwrap();
        assert_eq!(
            once, twice,
            "a second identical update must be a no-op; churn is not allowed to recur"
        );
    }

    /// BL-34 / `docs/issues/archive/2026-08-16-frontmatter-id-repair-reserializes-the-whole-block.md`.
    ///
    /// The fixture is deliberately hostile to normalization — a flow sequence, a
    /// double-quoted title, a `{Placeholder}` (valid YAML for a flow *mapping*),
    /// and two null/empty keys. Every one of those is something
    /// `rewrite_frontmatter_normalizing` rewrites, and the reason this needs a
    /// different primitive.
    ///
    /// The assertion is byte equality against the document with only that one line
    /// substituted. Asserting merely that the id changed is what let the
    /// re-serialization through in the first place.
    #[test]
    fn replace_scalar_line_touches_only_the_targeted_line() {
        let doc = concat!(
            "---\n",
            "kind: adr\n",
            "title: \"Unified Error Dialog System\"\n",
            "id: aaaaaaaaaaaaaaaa\n",
            "tags: [solver, iel, prod]\n",
            "created: {YYYY-MM-DD}\n",
            "topic: null\n",
            "owners: []\n",
            "---\n",
            "\n# Body\n\nSome prose with an id: like token.\n",
        );

        let out = replace_scalar_line(doc, "id", "bbbbbbbbbbbbbbbb")
            .expect("a top-level id: line exists");

        assert_eq!(
            out,
            doc.replace("id: aaaaaaaaaaaaaaaa", "id: bbbbbbbbbbbbbbbb"),
            "only the id line may change — the flow sequence, the double-quoted title, \
             the {{YYYY-MM-DD}} placeholder, the null keys and the body must all survive \
             verbatim"
        );
    }

    /// The one thing a line splice must not lose relative to `write`: an id YAML
    /// would resolve to a non-string still has to come back quoted.
    #[test]
    fn replace_scalar_line_quotes_a_value_yaml_would_misread() {
        let out = replace_scalar_line("---\nid: abc\nkind: bug\n---\n", "id", "1234567890123456")
            .expect("an id line exists");
        assert_eq!(
            out, "---\nid: '1234567890123456'\nkind: bug\n---\n",
            "an all-digit id must be quoted or YAML reads it back as a number"
        );
    }

    #[test]
    fn replace_scalar_line_declines_when_there_is_nothing_to_replace() {
        for (label, doc) in [
            (
                "no frontmatter at all",
                "# Just a doc\nid: aaaaaaaaaaaaaaaa\n",
            ),
            (
                "frontmatter without an id",
                "---\nkind: tracker\n---\n# x\n",
            ),
            (
                "indented id is a nested key, not the artifact's",
                "---\nmeta:\n  id: aaaaaaaaaaaaaaaa\n---\n",
            ),
            (
                "id after the closing delimiter is body text",
                "---\nkind: bug\n---\nid: aaaaaaaaaaaaaaaa\n",
            ),
        ] {
            assert!(
                replace_scalar_line(doc, "id", "bbbbbbbbbbbbbbbb").is_none(),
                "{label}: must decline rather than rewrite"
            );
        }
    }

    /// CRLF documents exist in this corpus (`handles_trailing_crlf` above), and a
    /// splice that assumed `\n` would eat the `\r` and corrupt every following line.
    #[test]
    fn replace_scalar_line_preserves_crlf_line_endings() {
        let out = replace_scalar_line(
            "---\r\nid: abc\r\nkind: bug\r\n---\r\n",
            "id",
            "ffffffffffffffff",
        )
        .expect("an id line exists");
        assert_eq!(out, "---\r\nid: ffffffffffffffff\r\nkind: bug\r\n---\r\n");
    }

    /// Generalizing from `id` to any key opened a hazard the ancestor could not
    /// reach: ids are hex, but a title or topic can carry an apostrophe. Emitted
    /// raw inside single quotes it closes the quote early and corrupts every
    /// following line of the block — a one-field write costing the whole document.
    #[test]
    fn replace_scalar_line_escapes_an_apostrophe_in_the_value() {
        let out = replace_scalar_line(
            "---\nid: abc\ntitle: old\n---\n# b\n",
            "title",
            "It's a #hash: colon",
        )
        .expect("a title line exists");

        let (fm, _) = parse(&out).expect("the block must still parse after the splice");
        assert_eq!(
            fm.expect("frontmatter").title.as_deref(),
            Some("It's a #hash: colon"),
            "the value must round-trip through YAML exactly; emitted: {out:?}"
        );
    }

    #[test]
    fn replace_scalar_line_declines_a_value_that_cannot_be_one_line() {
        assert!(
            replace_scalar_line("---\nid: abc\ntitle: old\n---\n", "title", "two\nlines").is_none(),
            "a newline cannot live on a spliced single line — decline so the caller re-serializes"
        );
    }

    #[test]
    fn upsert_int_line_splices_an_existing_key_in_place() {
        let doc = "---\nkind: tracker\nentry_prefix: HY\nentry_high_water_HY: 10\n---\n\n# Body\n";
        let out = upsert_int_line(doc, "entry_high_water_HY", 11).unwrap();
        assert_eq!(
            out,
            "---\nkind: tracker\nentry_prefix: HY\nentry_high_water_HY: 11\n---\n\n# Body\n"
        );
    }

    #[test]
    fn upsert_int_line_inserts_above_the_closing_delimiter_when_the_key_is_absent() {
        let doc = "---\nkind: tracker\nentry_prefix: HY\n---\n\n# Body\n";
        let out = upsert_int_line(doc, "entry_high_water_HY", 1).unwrap();
        assert_eq!(
            out,
            "---\nkind: tracker\nentry_prefix: HY\nentry_high_water_HY: 1\n---\n\n# Body\n"
        );
    }

    /// The reason this exists instead of reusing `replace_scalar_line`: that path
    /// runs the value through `scalar_can_be_bare`, whose round-trip check quotes
    /// `11` because `"11"` does not parse back as the STRING `"11"`. A quoted
    /// integer reads back as a string, and the allocator compares numbers.
    #[test]
    fn upsert_int_line_renders_the_value_bare_not_quoted() {
        let doc = "---\nkind: tracker\n---\n";
        let out = upsert_int_line(doc, "entry_high_water_R", 41).unwrap();
        assert!(
            out.contains("entry_high_water_R: 41"),
            "expected a bare integer, got: {out}"
        );
        assert!(!out.contains('\''), "must not quote an integer: {out}");
    }

    /// The insert must not land inside a block sequence. `entry_prefix` is the
    /// natural anchor for this key and is exactly the one that can be a sequence —
    /// a session log owns both F-N and W-N — so anchoring on the closing delimiter
    /// is what keeps the write valid YAML.
    #[test]
    fn upsert_int_line_does_not_insert_inside_a_block_sequence() {
        let doc = "---\nkind: tracker\nentry_prefix:\n  - F\n  - W\n---\n\n# Log\n";
        let out = upsert_int_line(doc, "entry_high_water_F", 33).unwrap();
        assert_eq!(
            out,
            "---\nkind: tracker\nentry_prefix:\n  - F\n  - W\nentry_high_water_F: 33\n---\n\n# Log\n"
        );
        // And it must still parse, with the sequence intact.
        let (fm, _) = parse(&out).unwrap();
        let fm = fm.unwrap();
        assert_eq!(
            fm.extra.get("entry_prefix"),
            Some(&serde_json::json!(["F", "W"])),
            "the sequence must survive the insert"
        );
        assert_eq!(
            fm.extra.get("entry_high_water_F"),
            Some(&serde_json::json!(33))
        );
    }

    #[test]
    fn upsert_int_line_preserves_crlf_line_endings_on_insert() {
        let doc = "---\r\nkind: tracker\r\n---\r\n\r\n# Body\r\n";
        let out = upsert_int_line(doc, "entry_high_water_HY", 7).unwrap();
        assert_eq!(
            out,
            "---\r\nkind: tracker\r\nentry_high_water_HY: 7\r\n---\r\n\r\n# Body\r\n"
        );
    }

    #[test]
    fn upsert_int_line_declines_when_there_is_no_frontmatter() {
        assert!(upsert_int_line("# Just a body\n", "entry_high_water_HY", 3).is_none());
    }

    /// `replace_body` is the mirror image of `replace_scalar_line`: the frontmatter
    /// block belongs to the author byte-for-byte, and only what follows the closing
    /// delimiter may change. Measured need — `patch={body}` re-emitted six
    /// frontmatter lines it was never asked to touch (BL-36).
    #[test]
    fn replace_body_preserves_the_frontmatter_block_verbatim() {
        let doc = concat!(
            "---\n",
            "title: \"Kept\"\n",
            "tags: [a, b]\n",
            "created: {YYYY-MM-DD}\n",
            "---\n",
            "\nold body\n",
        );

        assert_eq!(
            replace_body(doc, "\nnew body\n").expect("a frontmatter block exists"),
            concat!(
                "---\n",
                "title: \"Kept\"\n",
                "tags: [a, b]\n",
                "created: {YYYY-MM-DD}\n",
                "---\n",
                "\nnew body\n",
            ),
            "the flow sequence, the quoted title and the placeholder must all survive"
        );
        assert!(
            replace_body("# no frontmatter\n", "x").is_none(),
            "nothing to preserve means `write` is the right call, not a splice"
        );
    }

    #[test]
    fn replace_body_preserves_crlf_frontmatter() {
        assert_eq!(
            replace_body("---\r\nid: abc\r\n---\r\nold\n", "new\n").expect("block exists"),
            "---\r\nid: abc\r\n---\r\nnew\n"
        );
    }
}
