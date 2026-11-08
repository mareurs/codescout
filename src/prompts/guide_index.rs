//! Section-grain index over the compiled-in guide corpus.
//!
//! Delivery used to be all-or-nothing: `relevant_guide_topic` returned a topic
//! NAME and `topic_body` a whole file. Measured 2026-08-27 over 81 injections,
//! 66.7% went unused and 94% of `librarian`'s bytes were never touched. This
//! module makes the section the unit.

use serde_json::Value;

/// One `##` or `###` section of a guide, before declarations are parsed.
#[derive(Debug, Clone, PartialEq)]
pub struct RawSection {
    pub heading: String,
    pub level: u8,
    /// Heading line through to the very next heading of ANY level, so a parent
    /// excludes its `###` children and sections partition the file exactly.
    pub body: &'static str,
}

/// Split a guide into `(preamble, sections)`.
///
/// Fence-aware: a `##` line inside a fenced block is content, not a heading.
/// This is not defensive coding — the guides teach this very syntax by example,
/// and three separate measurements taken while designing this feature were wrong
/// because a naive `^## ` split matched a fence line in `tracker-conventions`,
/// inflating its section count and mis-splitting a 17,378 B section.
///
/// The fence tracker remembers which delimiter opened it: per CommonMark, a
/// closing fence must reuse the opener's character (``` ` ``` or `~`) and be at
/// least as long. A plain toggle-on-any-fence-line bool desyncs on a nested
/// 4-backtick block wrapping a ``` example, or a `~~~` line appearing as content
/// inside a ``` block — do not simplify this back to a `bool`.
pub fn split_sections(src: &'static str) -> (&'static str, Vec<RawSection>) {
    /// Leading run of a single fence character (`` ` `` or `~`) at the start of
    /// a trimmed line, if it is at least 3 long — i.e. a valid CommonMark fence
    /// delimiter, open or close.
    fn fence_run(trimmed: &str) -> Option<(u8, usize)> {
        let byte = trimmed.as_bytes().first().copied()?;
        if byte != b'`' && byte != b'~' {
            return None;
        }
        let run = trimmed.bytes().take_while(|&b| b == byte).count();
        (run >= 3).then_some((byte, run))
    }

    let mut fence: Option<(u8, usize)> = None;
    let mut starts: Vec<(usize, u8, String)> = Vec::new();
    let mut offset = 0usize;

    for line in src.split_inclusive('\n') {
        let trimmed = line.trim_start();
        match fence {
            None => {
                if let Some((byte, run)) = fence_run(trimmed) {
                    fence = Some((byte, run));
                } else {
                    let level = if line.starts_with("### ") {
                        3
                    } else if line.starts_with("## ") {
                        2
                    } else {
                        0
                    };
                    if level > 0 {
                        let heading = line[level as usize + 1..].trim().to_string();
                        starts.push((offset, level, heading));
                    }
                }
            }
            Some((open_byte, open_run)) => {
                if let Some((byte, run)) = fence_run(trimmed) {
                    if byte == open_byte && run >= open_run {
                        fence = None;
                    }
                }
            }
        }
        offset += line.len();
    }

    let preamble_end = starts.first().map(|(s, _, _)| *s).unwrap_or(src.len());
    let preamble = &src[..preamble_end];

    let mut sections = Vec::with_capacity(starts.len());
    for (i, (start, level, heading)) in starts.iter().enumerate() {
        // End at the VERY NEXT heading of any level, so a parent EXCLUDES its
        // `###` children and `preamble + every section` partitions the file
        // exactly. "Parent swallows its children" was considered and is wrong:
        // declaring a child would then re-send bytes the parent already carries,
        // and `###` decomposition — the entire remedy for an over-cap section —
        // would buy nothing.
        let end = starts.get(i + 1).map(|(s, _, _)| *s).unwrap_or(src.len());
        sections.push(RawSection {
            heading: heading.clone(),
            level: *level,
            body: &src[*start..end],
        });
    }
    (preamble, sections)
}

/// A call shape a section declares itself relevant to.
///
/// Grammar, deliberately minimal — widening it requires amending the spec:
/// ```text
/// shape := tool ["." action] ["(" "path~" substring ")"]
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    pub tool: String,
    pub action: Option<String>,
    pub path_contains: Option<String>,
}

pub fn parse_shape(s: &str) -> Result<Shape, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty shape".to_string());
    }
    let (head, path_contains) = match s.find('(') {
        Some(open) => {
            if !s.ends_with(')') {
                return Err(format!("unterminated predicate in `{s}`"));
            }
            let inner = &s[open + 1..s.len() - 1];
            let needle = inner
                .strip_prefix("path~")
                .ok_or_else(|| format!("only `path~<substring>` is supported, got `{inner}`"))?;
            if needle.is_empty() {
                return Err(format!("empty path predicate in `{s}`"));
            }
            (&s[..open], Some(needle.to_string()))
        }
        None => (s, None),
    };
    let head = head.trim();
    if head.is_empty() {
        return Err(format!("missing tool name in `{s}`"));
    }
    let (tool, action) = match head.split_once('.') {
        Some((t, a)) => {
            if !is_ident(t) {
                return Err(format!("malformed tool `{t}` in `{s}`"));
            }
            if !is_ident(a) {
                return Err(format!("malformed action `{a}` in `{s}`"));
            }
            (t.to_string(), Some(a.to_string()))
        }
        None => {
            if !is_ident(head) {
                return Err(format!("malformed tool `{head}` in `{s}`"));
            }
            (head.to_string(), None)
        }
    };
    Ok(Shape {
        tool,
        action,
        path_contains,
    })
}

impl Shape {
    /// Whether this declared shape matches a call.
    ///
    /// `sel` is the selector-key `Agent::selector_key` returns for the call —
    /// `None` means the tool opted out of `selector_key` entirely, so nothing
    /// should match it. That is deliberate, not a gap: do not turn it into a
    /// wildcard.
    ///
    /// A tool-only shape (`action: None`) matches any action of that tool, and
    /// also a bare tool key with no action at all — `selector_key`
    /// returns e.g. `Some("edit_markdown")` for a call with no `action`
    /// field.
    ///
    /// The `path~` predicate reads the RESULT, not the selector: a shape
    /// carrying one must not match on tool+action alone.
    pub fn matches(&self, sel: Option<&str>, result: &Value) -> bool {
        let Some(sel) = sel else { return false };
        let (tool, action) = match sel.split_once('.') {
            Some((t, a)) => (t, Some(a)),
            None => (sel, None),
        };
        if tool != self.tool {
            return false;
        }
        if let Some(want) = &self.action {
            if action != Some(want.as_str()) {
                return false;
            }
        }
        if let Some(needle) = &self.path_contains {
            if !crate::util::librarian_response::names_path_containing(result, needle) {
                return false;
            }
        }
        true
    }
}

/// `tool` and `action` identifiers: non-empty, `[A-Za-z0-9_]+` only.
///
/// Rejecting anything else — rather than trimming stray whitespace or a
/// doubled separator — matters because of how this class of bug fails: a
/// component like `" append_entry"` or `".append_entry"` parses without
/// error into a `Shape` that can never match a real call, so the section it
/// guards silently stops being delivered. That is the exact failure this
/// feature exists to prevent, so it must not be reintroducible here.
fn is_ident(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Parse the `serves:` / `requires:` comment block directly under a heading.
///
/// Only comments before the first blank line count. Everything after is prose,
/// including worked examples — a guide that teaches this syntax must not
/// declare itself by accident.
pub fn parse_declarations(body: &str) -> Result<(Vec<Shape>, Vec<String>), String> {
    let mut serves = Vec::new();
    let mut requires = Vec::new();
    for line in body.split_inclusive('\n').skip(1) {
        let t = line.trim();
        if t.is_empty() {
            break;
        }
        let Some(inner) = t.strip_circumfix("<!--", "-->") else {
            // A non-comment, non-blank line ends the declaration block.
            break;
        };
        let inner = inner.trim();
        if let Some(rest) = inner.strip_prefix("serves:") {
            for part in rest.split(',') {
                serves.push(parse_shape(part)?);
            }
        } else if let Some(rest) = inner.strip_prefix("requires:") {
            // NOT comma-split: a heading is prose and commonly contains its own
            // commas (e.g. "docs/trackers/ — Backing Store, Not a Docs Folder"),
            // so one `requires:` line names exactly one heading. Multiple
            // requirements are multiple `<!-- requires: ... -->` lines, each
            // appended by this same loop — `GuideSection::requires` is already a
            // `Vec` for that reason.
            let h = rest.trim();
            if h.is_empty() {
                return Err("empty heading in `requires:`".to_string());
            }
            requires.push(h.to_string());
        } else if inner.starts_with("serve") || inner.starts_with("require") {
            // A near-miss on the declaration keyword (missing `s`, singular
            // `require:` instead of `requires:`, etc.) must not silently parse
            // to nothing — that would make the section stop being delivered
            // with no signal anywhere, exactly the failure Gate 1's "malformed
            // declarations are loud" contract exists to rule out. Gate 5 was
            // meant to be the backstop for an orphaned section, but it did not
            // actually discriminate before the `child_declares` fix, so a
            // typo here had no gate catching it at all.
            return Err(format!(
                "unrecognised declaration comment `{inner}` — did you mean `serves:` \
                 or `requires:`?"
            ));
        }
    }
    Ok((serves, requires))
}

/// Maximum size of a section carrying a `serves:` declaration.
///
/// Bytes, not characters — these guides are dense with multi-byte em-dashes and
/// arrows, so `chars().count()` would silently under-report. 2,500 B sits just
/// above `librarian`'s natural p90 section, so most of the corpus already
/// complies: only 6 of 67 sections corpus-wide exceed it.
// cap-class: NOT_A_CAP — authoring ceiling asserted by a test; no runtime path truncates a section that exceeds it
pub const MAX_DECLARED_SECTION_BYTES: usize = 2500;

/// Measured delivered bytes (wrapped blocks, as injected) of the FIRST call of each write shape
/// on a tracker, plus a fixed 128 B (the margin the emission-budget fixtures use): 2026-10-17,
/// `append_entry` 12,928 B, `update_entry` 9,077 B, `augment` 5,261 B, `link` 2,591 B. `link` is
/// also bounded on a bug file. Raise one only with a new measurement. They sit under
/// `MAX_AUTO_INJECT_GUIDE_BYTES` (16,384 B, wrapper included), which the first cut of
/// `append_entry` met with 40 B to spare (16,344 B) while a body-summing test read 13.5 KB.
/// `append_entry`'s is also read by the end-to-end test, so there is one number.
///
/// RE-MEASURED 2026-10-18 (throwaway `eprintln!` in `tracker_write_shapes_ship_their_sections_and_only_theirs`
/// and the end-to-end test): `append_entry` 12,936 B (end-to-end: 12,936 B), `update_entry`
/// 9,083 B, `augment` 5,263 B, `link` 2,592 B on a tracker and on a bug file, so the margins
/// left under the ceilings below are 120, 122, 126 and 127 B. The block-seam fix (every section
/// block now opens with `\n`, `trailing_block`) adds 1 B per section block; how the drift above
/// splits between that and earlier edits was not measured for these shapes. The constants below
/// still read the 2026-10-17 figures on purpose: the ceilings were not raised.
#[cfg(test)]
pub(crate) const WRITE_CEILING_APPEND_ENTRY: usize = 12_928 + 128;
#[cfg(test)]
pub(crate) const WRITE_CEILING_UPDATE_ENTRY: usize = 9_077 + 128;
#[cfg(test)]
pub(crate) const WRITE_CEILING_AUGMENT: usize = 5_261 + 128;
#[cfg(test)]
pub(crate) const WRITE_CEILING_LINK: usize = 2_591 + 128;
/// A floor for `append_entry`, between its section BODIES and what it delivers wrapped: it fails
/// a delivered-bytes measure that forgot the wrappers. On 2026-10-17 bodies were 10,641 B and the
/// wrapped delivery 12,928 B (12,936 B on 2026-10-18), a 2,287 B wrapper share. On 2026-11-08 the
/// stale "not while the ledger's own commits are unpushed" bullet left tracker-conventions §Entry
/// ids (the refusal was removed 2026-09-11), and the wrapped delivery measured 11,631 B; the same
/// wrapper share puts the bodies near 9,344 B. The floor moved from 12,000 to 10,500 B to stay
/// between the two; the ceilings above were not raised.
#[cfg(test)]
pub(crate) const WRITE_FLOOR_APPEND_ENTRY: usize = 10_500;

/// A guide section with its declarations resolved.
#[derive(Debug, Clone)]
pub struct GuideSection {
    pub topic: &'static str,
    pub heading: String,
    pub level: u8,
    pub body: &'static str,
    pub serves: Vec<Shape>,
    pub requires: Vec<String>,
}

impl GuideSection {
    /// Ledger key. Opaque to `GuideLedger`, which stores `String` keys, so no
    /// on-disk format change is needed: a stale topic-only key simply fails to
    /// match, which re-delivers rather than suppresses.
    pub fn ledger_key(&self) -> String {
        format!("{}#{}", self.topic, self.heading)
    }
}

/// One guide's preamble plus its section index.
#[derive(Debug)]
pub struct TopicEntry {
    pub preamble: &'static str,
    pub sections: Vec<GuideSection>,
}

impl TopicEntry {
    /// Sections with a non-empty `serves` — the ones section-grain delivery can
    /// route on.
    pub fn declared(&self) -> impl Iterator<Item = &GuideSection> {
        self.sections.iter().filter(|s| !s.serves.is_empty())
    }
}

/// Split and parse one topic's compiled-in body into a `TopicEntry`.
///
/// Shared by `GuideIndex::try_build` (the live corpus) and, in tests,
/// `GuideIndex::from_str_for_test` (a synthetic one-topic corpus) — one code
/// path, so a test fixture cannot drift from production parsing behaviour.
fn build_topic_entry(topic: &'static str, src: &'static str) -> Result<TopicEntry, String> {
    let (preamble, raws) = split_sections(src);
    let mut sections = Vec::with_capacity(raws.len());
    for raw in raws {
        let (serves, requires) =
            parse_declarations(raw.body).map_err(|e| format!("{topic} § {}: {e}", raw.heading))?;
        sections.push(GuideSection {
            topic,
            heading: raw.heading,
            level: raw.level,
            body: raw.body,
            serves,
            requires,
        });
    }
    Ok(TopicEntry { preamble, sections })
}

/// Section index over the whole compiled-in guide corpus.
#[derive(Debug)]
pub struct GuideIndex {
    topics: std::collections::BTreeMap<&'static str, TopicEntry>,
}

impl GuideIndex {
    /// Build the index, or fail loudly if any guide's declarations are
    /// malformed. Never silently drops a topic or a section.
    pub fn try_build() -> Result<Self, String> {
        let mut topics = std::collections::BTreeMap::new();
        for &topic in crate::prompts::GUIDE_TOPICS {
            let src = crate::prompts::topic_body(topic)
                .ok_or_else(|| format!("topic `{topic}` has no body"))?;
            topics.insert(topic, build_topic_entry(topic, src)?);
        }
        Ok(Self { topics })
    }

    pub fn topic(&self, t: &str) -> Option<&TopicEntry> {
        self.topics.get(t)
    }

    /// Whether this topic has opted into section-grain delivery. Topics with no
    /// declarations keep the whole-topic path — this is the phase switch.
    pub fn declares(&self, t: &str) -> bool {
        self.topic(t).is_some_and(|e| e.declared().next().is_some())
    }

    /// The topic owning a section that DECLARES this call's shape, if any.
    ///
    /// Corpus-wide, unlike `match_sections`, which is *told* its topic. That
    /// difference is the point: it lets a `serves:` declaration compete with a
    /// tool's own result-based topic heuristic instead of sitting unreachable
    /// behind it. `librarian.md` § *doctor repairs* declares `librarian.doctor`,
    /// but every `doctor` scan of a real catalog names tracker paths, so
    /// `LibrarianAdapter::relevant_guide_topic` sent `tracker-conventions` —
    /// whole, 26x the size of the displaced section — and the section could not
    /// be delivered at all. See
    /// `docs/issues/archive/2026-08-31-a-served-section-can-be-unreachable-via-topic-routing.md`.
    ///
    /// Returning a topic here does NOT mean it wins. The caller tries the
    /// tool's own result-based topic FIRST and only falls through to this one
    /// when that ships nothing — because the result heuristic encodes what the
    /// call touched, which an unqualified `serves:` shape cannot express. See
    /// `Tool::call_content` for the two fixes an outright victory would revert.
    ///
    /// **Only tools implementing `selector_key` are reachable here at all.** The
    /// trait default is `None` and `Shape::matches` refuses to match on `None`
    /// — deliberately, per its own doc comment — so this cannot hijack the
    /// topic of a tool that never opted into selectors.
    ///
    /// Deterministic by `BTreeMap` order, but that order is never allowed to
    /// decide anything: `no_two_topics_declare_an_overlapping_shape` fails if
    /// two topics could both answer one call. Ambiguity is a corpus defect to
    /// fix at authoring time, not a runtime coin-flip.
    ///
    /// Guarantee relied on by the caller: if this returns `Some(t)`, then
    /// `match_sections(t, sel, result)` is non-empty — it selects `t` *by* a
    /// section matching, and `match_sections` only ever adds the `requires:`
    /// closure on top. So a declaring candidate can never take the preamble
    /// path — which spares it that path's `insert`-then-return-empty, but does
    /// NOT make trying it side-effect free. See `Tool::call_content`, which
    /// carries the correction: `GuideLedger::insert` refreshes and persists on
    /// repeats, so a fallthrough onto a topic whose sections are all spent still
    /// pays stamp refreshes and disk writes for zero delivered bytes.
    pub fn topic_declaring(&self, sel: Option<&str>, result: &Value) -> Option<&'static str> {
        self.topics.iter().find_map(|(topic, entry)| {
            entry
                .declared()
                .any(|s| s.serves.iter().any(|sh| sh.matches(sel, result)))
                .then_some(*topic)
        })
    }

    /// Sections serving this call, plus their transitive `requires:` closure,
    /// in document order with no duplicates.
    ///
    /// The closure matters because sections are not independent: e.g.
    /// `tracker-conventions` § *Entry ids* states entry-id law whose
    /// precondition lives in § *Declaring a ledger*. Delivered alone it is
    /// true and misleading.
    pub fn match_sections(
        &self,
        topic: &str,
        sel: Option<&str>,
        result: &Value,
    ) -> Vec<&GuideSection> {
        let Some(entry) = self.topic(topic) else {
            return Vec::new();
        };
        let mut wanted: std::collections::BTreeSet<&str> = Default::default();
        for sec in entry.declared() {
            if sec.serves.iter().any(|s| s.matches(sel, result)) {
                wanted.insert(sec.heading.as_str());
            }
        }
        // Transitive closure over `requires:`. Bounded by section count, so a
        // `requires:` cycle (including a section requiring itself) terminates
        // rather than hanging: each pass either adds at least one new heading
        // to `wanted` or the loop stops, and `wanted` is capped at
        // `entry.sections.len()`.
        loop {
            let mut added = false;
            let pending: Vec<&str> = entry
                .sections
                .iter()
                .filter(|s| wanted.contains(s.heading.as_str()))
                .flat_map(|s| s.requires.iter().map(|r| r.as_str()))
                .collect();
            for req in pending {
                if entry.sections.iter().any(|s| s.heading == req) && wanted.insert(req) {
                    added = true;
                }
            }
            if !added {
                break;
            }
        }
        entry
            .sections
            .iter()
            .filter(|s| wanted.contains(s.heading.as_str()))
            .collect()
    }

    /// Every STATICALLY KNOWN ledger key the guide side can stamp — topic keys
    /// and section keys. Does NOT include the two forms synthesized at call time:
    /// `<topic>#<preamble>` (`guide_emit::guide_blocks_for`, when a call's shape
    /// matches no section) and `<topic>#<pointer>` (`guide_emit::whole_topic_blocks`,
    /// when a non-declaring body is over the auto-inject bound). Neither can be
    /// enumerated from `topics` because each exists only as a runtime string, not
    /// a stored field.
    ///
    /// Exists for Gate 5 in `operator_rules::route`, which must assert the `op:`
    /// namespace collides with none of them. Returning the keys rather than
    /// exposing `topics` keeps the map private.
    pub fn ledger_keys(&self) -> Vec<String> {
        self.topics
            .iter()
            .flat_map(|(t, e)| {
                std::iter::once((*t).to_string()).chain(e.sections.iter().map(|s| s.ledger_key()))
            })
            .collect()
    }
}

/// Build a one-topic index from a literal guide body, for tests that need
/// declarations the real corpus does not carry yet (nothing declares before
/// Task 6).
#[cfg(test)]
impl GuideIndex {
    pub fn from_str_for_test(topic: &'static str, src: &'static str) -> Self {
        let mut topics = std::collections::BTreeMap::new();
        topics.insert(
            topic,
            build_topic_entry(topic, src).expect("test fixture guide must parse"),
        );
        Self { topics }
    }
}

/// Lazily built, process-wide guide index.
///
/// `.expect()`s so a malformed corpus fails loudly at first use;
/// `index_builds_for_every_registered_topic` turns that into a build-time gate.
pub static GUIDE_INDEX: std::sync::LazyLock<GuideIndex> = std::sync::LazyLock::new(|| {
    GuideIndex::try_build()
        .expect("guide index failed to build; see gate `index_builds_for_every_registered_topic`")
});

/// A `serves: librarian.doctor` declaration in `librarian.md` matches the
/// selector a doctor call carries.
///
/// **Read the name literally: this pins the DECLARATION, not delivery.** It was
/// first written as `..._is_reachable_from_a_doctor_call`, to verify that moving
/// the `fix=` repair modes out of the tool schema left them reachable. It passed
/// — while the modes were, in fact, unreachable in production.
///
/// The gap is one layer up: `LibrarianAdapter::relevant_guide_topic` picks the
/// topic from the RESULT's content, and `names_tracker_path` scans `path` inside
/// the `violations` array, so a real doctor scan routes to `tracker-conventions`
/// and never consulted `librarian`'s sections at all. Measured 2026-08-31: 128 of
/// 138 violations named `docs/trackers/` or `docs/issues/`.
///
/// **Partly closed the same day** by the fallthrough in `Tool::call_content`:
/// once `tracker-conventions` has been spent for the session, a later librarian
/// call falls through to the topic whose section declares its shape, so
/// § *doctor repairs* is reachable — pinned end-to-end by
/// `guide_hint_tests::a_declared_section_still_arrives_once_the_content_topic_is_spent`.
/// Only partly: the FIRST tracker-path-naming call of a session still routes
/// away, deliberately (that route closed `32736ca0`). So this test still does not
/// measure delivery, and the `fix=` modes still belong in the schema.
///
/// So it answered "does the shape match?" when the question that mattered was
/// "does the router send a doctor result to this topic?" — an adjacent
/// proposition, faithfully measured (`reconnaissance-patterns:R-136`). The
/// schema text was restored rather than left resting on this; see
/// `docs/issues/archive/2026-08-31-a-served-section-can-be-unreachable-via-topic-routing.md`.
///
/// Kept, renamed to what it actually proves, because the declaration is still
/// worth pinning: a blank line between the heading and its `<!-- serves: -->`
/// yields a section that parses fine, declares nothing, and is never delivered,
/// with every other test green.
///
/// Mutation that must kill this: change the declaration to `librarian.reindex`,
/// or move the comment below the first blank line.
#[test]
fn doctor_repairs_section_declares_a_shape_a_doctor_selector_matches() {
    let empty = Value::Object(Default::default());
    let matched = GUIDE_INDEX.match_sections("librarian", Some("librarian.doctor"), &empty);
    let headings: Vec<&str> = matched.iter().map(|s| s.heading.as_str()).collect();
    assert!(
        headings.iter().any(|h| h.contains("doctor repairs")),
        "the section must at least declare a shape a doctor selector matches. \
         matched: {headings:?}"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const FENCED: &str = "\
intro text
## Real Heading
body one
```markdown
## Not A Heading
```
more body one
## Second Real
body two
";

    #[test]
    fn split_ignores_headings_inside_fences() {
        let (preamble, secs) = split_sections(FENCED);
        assert_eq!(preamble, "intro text\n");
        let names: Vec<&str> = secs.iter().map(|s| s.heading.as_str()).collect();
        assert_eq!(names, vec!["Real Heading", "Second Real"]);
        assert!(secs[0].body.contains("## Not A Heading"));
        assert!(secs[0].body.contains("more body one"));
    }

    #[test]
    fn split_captures_h3_as_its_own_section() {
        let src = "\
## Parent
p body
### Child
c body
## Next
n body
";
        let (_, secs) = split_sections(src);
        let got: Vec<(u8, &str)> = secs.iter().map(|s| (s.level, s.heading.as_str())).collect();
        assert_eq!(got, vec![(2, "Parent"), (3, "Child"), (2, "Next")]);
        // A parent's body stops at its child, so bytes are never double-counted.
        assert!(!secs[0].body.contains("c body"));
    }

    #[test]
    fn tilde_fences_toggle_too() {
        let src = "## A\n~~~\n## Fake\n~~~\n";
        let (_, secs) = split_sections(src);
        assert_eq!(secs.len(), 1);
    }

    #[test]
    fn nested_fences_do_not_desync() {
        // A 4-backtick block wrapping a ``` example: the outer opener must
        // stay open across the inner ```rust / ``` pair, so the `## ` line
        // inside — still within the outer fence — is never a heading.
        let src = "\
## Real
````markdown
```rust
## Not A Heading
```
````
## Next
";
        let (_, secs) = split_sections(src);
        let names: Vec<&str> = secs.iter().map(|s| s.heading.as_str()).collect();
        assert_eq!(names, vec!["Real", "Next"]);
    }

    #[test]
    fn mixed_delimiter_inside_fence_does_not_close_it() {
        // A `~~~` line appearing as *content* inside a ``` block must not
        // close the fence — CommonMark requires the closer to reuse the
        // opener's delimiter character.
        let src = "\
## Real
```
~~~
## Not A Heading
```
## Next
";
        let (_, secs) = split_sections(src);
        let names: Vec<&str> = secs.iter().map(|s| s.heading.as_str()).collect();
        assert_eq!(names, vec!["Real", "Next"]);
    }

    #[test]
    fn corpus_partitions_exactly() {
        for topic in crate::prompts::GUIDE_TOPICS {
            let src = crate::prompts::topic_body(topic).unwrap();
            let (pre, secs) = split_sections(src);
            assert_eq!(
                pre.len() + secs.iter().map(|s| s.body.len()).sum::<usize>(),
                src.len(),
                "sections of `{topic}` do not partition the file"
            );
            assert!(
                secs.iter().all(|s| !s.heading.contains("<ID>")),
                "`{topic}`: a fenced example line was parsed as a heading"
            );
        }
    }
    #[test]
    fn no_topic_has_duplicate_section_headings() {
        // `guide_blocks_for`'s `if emitted.insert(key)` (types.rs) means a second
        // section sharing an earlier one's `ledger_key` (heading-derived) is
        // dropped permanently for the session — silent under-delivery, not the
        // "fail-safe over-delivery" an earlier ruling assumed. Zero duplicates
        // in the corpus today; this pins that down.
        let idx = GuideIndex::try_build().unwrap();
        for topic in crate::prompts::GUIDE_TOPICS {
            let Some(entry) = idx.topic(topic) else {
                continue;
            };
            let mut seen = std::collections::HashSet::new();
            for sec in &entry.sections {
                assert!(
                    seen.insert(sec.heading.as_str()),
                    "{topic} has a duplicate section heading `{}`",
                    sec.heading
                );
            }
        }
    }

    /// Whether two shapes declared by DIFFERENT topics could leave `topic_declaring`
    /// choosing between them by `BTreeMap` order, i.e. by accident.
    ///
    /// Overlap is over-approximated on `(tool, action)` with `None` as a wildcard.
    /// Beyond that one refinement is allowed, and only one: **exactly one** of the
    /// pair carrying a `path~` predicate is a deliberate narrowing, not an ambiguity.
    /// The narrower shape can match only a call whose RESULT names that path, and for
    /// such a call the tool's own result-based topic is consulted before
    /// `topic_declaring` (`emit_guide_sections`), so which topic answers is decided
    /// by the content, not by the alphabet. `tracker-conventions` declares
    /// `doc.create(path~docs/trackers/)` beside `librarian`'s unqualified `doc.create`
    /// on exactly that basis.
    ///
    /// Two shapes that BOTH carry a predicate stay ambiguous: their needles may
    /// nest, and nothing here compares needles. Two that BOTH omit one stay
    /// ambiguous too — that is the case this test was written for.
    fn ambiguous_across_topics(a: &Shape, b: &Shape) -> bool {
        let overlaps =
            a.tool == b.tool && (a.action.is_none() || b.action.is_none() || a.action == b.action);
        overlaps && a.path_contains.is_some() == b.path_contains.is_some()
    }

    #[test]
    fn no_two_topics_declare_an_overlapping_shape() {
        // `topic_declaring` scans topics in `BTreeMap` order and takes the first
        // match, so two topics able to answer one call would make delivery hinge
        // on alphabetical accident — and silently, since the loser is simply
        // never consulted. That is the exact failure mode section-grain delivery
        // exists to prevent, so it must not be reintroducible one level up.
        //
        // Not vacuous since `tracker-conventions` adopted `serves:`: it is the
        // second declaring topic, and the only pairs this test now tolerates are the
        // path-qualified narrowings `ambiguous_across_topics` documents. See
        // `docs/issues/archive/2026-08-31-a-served-section-can-be-unreachable-via-topic-routing.md`.
        let idx = GuideIndex::try_build().unwrap();
        let mut seen: Vec<(&str, &Shape)> = Vec::new();
        for topic in crate::prompts::GUIDE_TOPICS {
            let Some(entry) = idx.topic(topic) else {
                continue;
            };
            for sec in entry.declared() {
                for shape in &sec.serves {
                    for (other_topic, other) in &seen {
                        // Same-topic overlap is legal and present: `librarian.md`
                        // declares `doc.update` from both § *Choosing a
                        // mode* and § *The shrink guard*, and `match_sections`
                        // delivers both. Only CROSS-topic overlap is ambiguous.
                        if *other_topic == *topic {
                            continue;
                        }
                        assert!(
                            !ambiguous_across_topics(other, shape),
                            "`{topic}` and `{other_topic}` both declare a shape matching \
                             {}.{} — topic_declaring would resolve that by BTreeMap \
                             order, i.e. by accident. A path-qualified shape in one topic \
                             beside an unqualified one in the other is allowed; two of the \
                             same kind are not.",
                            shape.tool,
                            shape.action.as_deref().unwrap_or("*")
                        );
                    }
                    seen.push((topic, shape));
                }
            }
        }
    }

    #[test]
    fn a_path_qualified_shape_beside_an_unqualified_one_is_a_narrowing_not_an_ambiguity() {
        let plain = parse_shape("doc.create").unwrap();
        let narrow = parse_shape("doc.create(path~docs/trackers/)").unwrap();
        let other_narrow = parse_shape("doc.create(path~docs/issues/)").unwrap();
        let other_action = parse_shape("doc.get").unwrap();
        let whole_tool = parse_shape("doc").unwrap();

        // The one tolerated pairing, in both argument orders.
        assert!(!ambiguous_across_topics(&plain, &narrow));
        assert!(!ambiguous_across_topics(&narrow, &plain));
        // The case the test exists for: two unqualified shapes on one call.
        assert!(ambiguous_across_topics(
            &plain,
            &parse_shape("doc.create").unwrap()
        ));
        assert!(ambiguous_across_topics(&plain, &whole_tool));
        // Two qualified shapes: needles are not compared, so they stay ambiguous even
        // when these two happen to be disjoint.
        assert!(ambiguous_across_topics(&narrow, &other_narrow));
        // Different actions never collide, qualified or not.
        assert!(!ambiguous_across_topics(&plain, &other_action));
        assert!(!ambiguous_across_topics(&narrow, &other_action));
    }

    #[test]
    fn parse_shape_forms() {
        assert_eq!(
            parse_shape("artifact.append_entry").unwrap(),
            Shape {
                tool: "artifact".into(),
                action: Some("append_entry".into()),
                path_contains: None
            }
        );
        assert_eq!(
            parse_shape("grep").unwrap(),
            Shape {
                tool: "grep".into(),
                action: None,
                path_contains: None
            }
        );
        assert_eq!(
            parse_shape("artifact.update(path~docs/issues/)").unwrap(),
            Shape {
                tool: "artifact".into(),
                action: Some("update".into()),
                path_contains: Some("docs/issues/".into()),
            }
        );
    }

    #[test]
    fn malformed_shape_is_an_error_not_a_skip() {
        // Gate 1: a typo must fail loudly. A silently-skipped declaration is
        // indistinguishable from a section nobody declared.
        assert!(parse_shape("artifact.update(path=docs/)").is_err());
        assert!(parse_shape("artifact.update(").is_err());
        assert!(parse_shape("").is_err());
        assert!(parse_shape("artifact.get(mode~x)").is_err());
    }
    #[test]
    fn a_near_miss_declaration_keyword_is_an_error_not_a_silent_skip() {
        // Gate 1 extended to the keyword itself, not just a well-formed shape's
        // internals: `serve:` (missing the `s`) or `require:` (singular) used to
        // parse to nothing with no error — the section just silently stopped
        // being delivered, with no gate catching it (fix 1's removal of the
        // bogus `child_declares` clause is what made Gate 5 able to catch a
        // resulting orphan at all, but a typo that still leaves the section
        // reachable some other way would sail through even then).
        assert!(parse_declarations("\n<!-- serve: artifact.get -->\n").is_err());
        assert!(parse_declarations("\n<!-- require: Some Heading -->\n").is_err());
        // A genuinely unrelated comment is not a near miss and must not error.
        assert!(parse_declarations("\n<!-- some other note -->\n").unwrap() == (vec![], vec![]));
    }

    #[test]
    fn stray_characters_around_the_separator_are_an_error_not_an_inert_shape() {
        // Fix round 1: `head.split_once('.')` used to accept anything on
        // either side as long as it was non-empty, so a stray space or a
        // doubled `.` parsed into a `Shape` that could never match a real
        // call — a permanently inert declaration, not a loud error. That is
        // the exact failure mode this feature exists to prevent, reintroduced
        // inside the machinery.
        assert!(parse_shape("artifact. append_entry").is_err()); // space after separator
        assert!(parse_shape("artifact .append_entry").is_err()); // space before separator
        assert!(parse_shape("artifact..append_entry").is_err()); // doubled separator
        assert!(parse_shape("artifact.append entry").is_err()); // space inside action
        assert!(parse_shape("tool.a.b").is_err()); // dot inside action

        // Underscores and digits remain legal — Task 6's real declarations
        // depend on this.
        assert_eq!(
            parse_shape("edit_markdown").unwrap(),
            Shape {
                tool: "edit_markdown".into(),
                action: None,
                path_contains: None,
            }
        );
        assert_eq!(
            parse_shape("artifact.append_entry").unwrap(),
            Shape {
                tool: "artifact".into(),
                action: Some("append_entry".into()),
                path_contains: None,
            }
        );
    }

    #[test]
    fn declarations_are_read_from_the_comment_block_under_the_heading() {
        let body = "## Entry ids\n<!-- serves: artifact.append_entry, artifact.update_entry -->\n<!-- requires: Declaring a ledger -->\n\nprose\n<!-- serves: not.parsed -->\n";
        let (serves, requires) = parse_declarations(body).unwrap();
        assert_eq!(serves.len(), 2);
        assert_eq!(serves[0].action.as_deref(), Some("append_entry"));
        assert_eq!(requires, vec!["Declaring a ledger".to_string()]);
    }

    #[test]
    fn a_fenced_declaration_is_documentation_not_a_declaration() {
        let body = "## Teaching\n```markdown\n<!-- serves: artifact.get -->\n```\n";
        let (serves, requires) = parse_declarations(body).unwrap();
        assert!(serves.is_empty());
        assert!(requires.is_empty());
    }

    #[test]
    fn index_builds_for_every_registered_topic() {
        // Gate 1 at corpus scale: a malformed declaration anywhere fails the build.
        let idx = GuideIndex::try_build().expect("guide index must build");
        for topic in crate::prompts::GUIDE_TOPICS {
            assert!(
                idx.topic(topic).is_some(),
                "topic `{topic}` missing from index"
            );
        }
    }

    #[test]
    fn section_bodies_partition_the_guide_exactly() {
        // No byte is lost between sections and none is counted twice; otherwise a
        // slice can silently omit a rule that lived on a boundary.
        let idx = GuideIndex::try_build().unwrap();
        for topic in crate::prompts::GUIDE_TOPICS {
            let entry = idx.topic(topic).unwrap();
            let body = crate::prompts::topic_body(topic).unwrap();
            let all: usize = entry.sections.iter().map(|s| s.body.len()).sum();
            assert_eq!(
                entry.preamble.len() + all,
                body.len(),
                "sections of `{topic}` do not partition the file"
            );
        }
    }

    #[test]
    fn a_topic_without_declarations_reports_false() {
        let idx = GuideIndex::try_build().unwrap();
        assert!(!idx.declares("progressive-disclosure"));
    }

    #[test]
    fn shape_matching_rules() {
        let empty = serde_json::json!({});
        let s = parse_shape("artifact.get").unwrap();
        assert!(s.matches(Some("artifact.get"), &empty));
        assert!(!s.matches(Some("artifact.find"), &empty));
        assert!(!s.matches(None, &empty));
        // An action-carrying shape must not match a bare-tool (actionless) selector —
        // the mirror of the tool-only rule below.
        assert!(!s.matches(Some("artifact"), &empty));

        // Tool-only shape matches any action of that tool, and a keyless call.
        let t = parse_shape("artifact").unwrap();
        assert!(t.matches(Some("artifact.get"), &empty));
        assert!(t.matches(Some("artifact"), &empty));
        // ...but not a different tool entirely.
        assert!(!t.matches(Some("grep.search"), &empty));

        // Path predicate reads the RESULT.
        let p = parse_shape("artifact.get(path~docs/issues/)").unwrap();
        assert!(!p.matches(Some("artifact.get"), &empty));
        assert!(p.matches(
            Some("artifact.get"),
            &serde_json::json!({"abs_path": "docs/issues/x.md"})
        ));
    }

    /// A hand-built guide with a real `requires:` chain: Zeta serves the call and
    /// requires Alpha; Alpha requires Mu; Mu is declared by nothing. No guide in the
    /// live corpus carries any declaration until Task 6 lands, so a closure test
    /// must build its own fixture rather than assert on `GuideIndex::try_build()` —
    /// asserting on the live corpus today would pass on an empty vec and keep
    /// passing even if the closure were completely broken.
    ///
    /// Headings are deliberately NOT in alphabetical order (Zeta, Alpha, Mu):
    /// `match_sections`'s working set is a `BTreeSet<&str>`, which iterates
    /// lexicographically, so an alphabetically-ordered fixture cannot distinguish
    /// "returns in document order" from "returns in set order" — the single most
    /// plausible ordering regression would pass unchanged against one.
    const CHAIN_GUIDE: &str = "\
# Synthetic

Preamble.

## Zeta
<!-- serves: artifact.append_entry -->
<!-- requires: Alpha -->

Body Zeta.

## Alpha
<!-- requires: Mu -->

Body Alpha.

## Mu

Body Mu.
";

    #[test]
    fn match_sections_closes_requires_transitively_and_orders_by_document() {
        let idx = GuideIndex::from_str_for_test("chain-topic", CHAIN_GUIDE);
        let got = idx.match_sections(
            "chain-topic",
            Some("artifact.append_entry"),
            &serde_json::json!({}),
        );
        let headings: Vec<&str> = got.iter().map(|s| s.heading.as_str()).collect();
        // Document order (Zeta, Alpha, Mu), not alphabetical (Alpha, Mu, Zeta) — a
        // set-ordered implementation fails this exact assertion.
        assert_eq!(headings, vec!["Zeta", "Alpha", "Mu"]);
    }

    #[test]
    fn unknown_topic_matches_nothing() {
        let idx = GuideIndex::try_build().unwrap();
        assert!(idx
            .match_sections(
                "no-such-topic",
                Some("artifact.get"),
                &serde_json::json!({})
            )
            .is_empty());
    }

    #[test]
    fn live_corpus_librarian_declares_and_matches() {
        let idx = GuideIndex::try_build().unwrap();
        let entry = idx.topic("librarian").expect("librarian in index");
        assert!(
            entry.declared().next().is_some(),
            "librarian has no declared sections"
        );

        let got = idx.match_sections("librarian", Some("doc.find"), &serde_json::json!({}));
        assert!(
            !got.is_empty(),
            "expected `doc.find` to match at least one librarian section"
        );
        assert!(
            got.iter().any(|s| s.heading == "Filter Syntax"),
            "expected § Filter Syntax to serve `artifact.find`, got {:?}",
            got.iter().map(|s| s.heading.as_str()).collect::<Vec<_>>()
        );
    }
    /// The result shapes that REALLY route a call to `tracker-conventions`, each with
    /// the guide sections that call needs, so the first tracker-path call of a session
    /// ships those and not the whole topic.
    ///
    /// The five shapes are the ones verified at runtime on 2026-10-16 by making the
    /// calls and reading which results name a tracker or bug path (the only thing
    /// `LibrarianAdapter::relevant_guide_topic` looks at): `doc.create` and `doc.get`
    /// name `abs_path`, `doc.find` names `rel_path` per item, `doc.move` names
    /// `old_abs_path`/`new_abs_path` (read since 2026-10-17), and `librarian.doctor` names
    /// `path` per violation. `doc.update`, `append_entry`, `delete`, `rekey_prefix` and
    /// the rest name no scanned path, so they route to `librarian` and no
    /// tracker-conventions declaration could ever reach them — see SECTION_WAIVERS.
    ///
    /// **A bound on bytes as well as on headings.** `MAX_AUTO_INJECT_GUIDE_BYTES` is
    /// the size a harness may save to disk unread; a "section" set larger than that
    /// would be the old whole-topic problem in several blocks.
    #[test]
    fn tracker_conventions_ships_sections_to_each_shape_that_routes_to_it() {
        let idx = GuideIndex::try_build().unwrap();
        assert!(
            idx.declares("tracker-conventions"),
            "tracker-conventions declares no `serves:` section, so its first call ships the \
             whole topic (or a pointer, over the auto-inject bound)"
        );
        let whole = crate::prompts::topic_body("tracker-conventions")
            .unwrap()
            .len();
        let tracker = serde_json::json!({"abs_path": "docs/trackers/x.md"});
        let issue = serde_json::json!({"abs_path": "docs/issues/x.md"});
        let find_tracker = serde_json::json!({"items": [{"rel_path": "docs/trackers/x.md"}]});
        let find_issue = serde_json::json!({"items": [{"rel_path": "docs/issues/x.md"}]});
        // `doc.move` is the archive flow; its result names the file only as
        // `old_abs_path`/`new_abs_path`.
        let move_issue = serde_json::json!({
            "old_abs_path": "/r/docs/issues/x.md",
            "new_abs_path": "/r/docs/issues/archive/x.md"
        });
        let move_tracker = serde_json::json!({
            "old_abs_path": "/r/docs/trackers/x.md",
            "new_abs_path": "/r/docs/trackers/archive/x.md"
        });
        let cases: [(&str, &Value, &[&str]); 8] = [
            (
                "doc.create",
                &tracker,
                &[
                    "Tracker artifacts (docs/trackers/)",
                    "Declaring a ledger",
                    "A prefix has one owner per repository",
                    "Make the tracker guarded",
                ],
            ),
            (
                "doc.create",
                &issue,
                &[
                    "Bug files (docs/issues/)",
                    "Bug frontmatter and status vocabulary",
                    "Trigger rules and capture discipline",
                ],
            ),
            (
                "doc.get",
                &tracker,
                &["A `## Resume` states its own currency"],
            ),
            (
                "doc.get",
                &issue,
                &[
                    "Bug frontmatter and status vocabulary",
                    "Claiming and releasing a bug",
                    "The unverified caveat, made queryable",
                ],
            ),
            ("doc.find", &find_tracker, &["Querying with the librarian"]),
            (
                "doc.move",
                &move_issue,
                &[
                    "Archiving a bug or tracker \u{2014} through the catalog, never git mv",
                    "Archiving a bug or tracker \u{2014} stage both halves of the move",
                    "Archiving a bug or tracker \u{2014} re-point the citations",
                    "Verifying the citation sweep",
                    "Reading move's inbound_path_citations",
                    "Why a green gate proves nothing about the sweep",
                ],
            ),
            (
                "doc.move",
                &move_tracker,
                &[
                    "Archiving a bug or tracker \u{2014} through the catalog, never git mv",
                    "Archiving a bug or tracker \u{2014} stage both halves of the move",
                    "Archiving a bug or tracker \u{2014} re-point the citations",
                ],
            ),
            (
                "doc.find",
                &find_issue,
                &[
                    "Bug frontmatter and status vocabulary",
                    "Claiming and releasing a bug",
                    "Querying with the librarian",
                ],
            ),
        ];
        for (sel, result, want) in cases {
            let got = idx.match_sections("tracker-conventions", Some(sel), result);
            let headings: Vec<&str> = got.iter().map(|s| s.heading.as_str()).collect();
            for w in want {
                assert!(
                    headings.contains(w),
                    "`{sel}` on {result} must be served § {w}, got {headings:?}"
                );
            }
            let bytes: usize = got.iter().map(|s| s.body.len()).sum();
            assert!(
                bytes <= crate::tools::guide_emit::MAX_AUTO_INJECT_GUIDE_BYTES,
                "`{sel}` on {result} ships {bytes} B of sections — over the auto-inject bound"
            );
            assert!(
                bytes * 4 < whole,
                "`{sel}` on {result} ships {bytes} B of a {whole} B guide; that is not section grain"
            );
        }
        // `Archiving a tracker` leads with `doc(action="update", patch={status: archived})`,
        // the call that needs it, and `update` still routes to `librarian`: serving it
        // after a MOVE has run is the late, wrong-call delivery that its waiver records.
        let moved = idx.match_sections("tracker-conventions", Some("doc.move"), &move_tracker);
        assert!(
            !moved.iter().any(|s| s.heading == "Archiving a tracker"),
            "a tracker move must not be served the update-first § Archiving a tracker"
        );
    }
    /// Every write-shape section of `tracker-conventions` that a write other than `move` needs
    /// and that can be routed safely is reachable by the call that needs it, and by no call that
    /// does not.
    ///
    /// The routing half is proved separately, against real `doc` calls
    /// (`server::guide_hint_tests::a_write_to_a_tracker_or_bug_file_is_routed_to_its_section`);
    /// this table is the declaration half: for each section, the calls that MUST serve it and
    /// at least one that must NOT. Both directions, because each alone is monotone — "every
    /// section is served by something" is satisfied by a `serves:` widened to every write
    /// (over-reporting), and "no section leaks" by an emptied declaration. The `doc.move`
    /// archive sections have their own cases in
    /// `tracker_conventions_ships_sections_to_each_shape_that_routes_to_it`; here `move` is not
    /// a row at all. **`doc.update` is never served from this topic** (asserted at the foot):
    /// routing it here delays the anti-wipe sections, see
    /// `prompts::TRACKER_SECTION_NEEDS_A_STATUS_UPDATE`.
    ///
    /// The table is closed over the guide, compared as sets: a `serves:` on one of these
    /// write shapes with no row here fails, rather than passing untested.
    ///
    /// **Bytes are measured on what is INJECTED**, the wrapped block (`section_block`), not on
    /// the section bodies: `MAX_AUTO_INJECT_GUIDE_BYTES` says "wrapper included", and the first
    /// cut of this test summed bodies, so a call that delivered 16,344 B read as 13.5 KB against
    /// a 16,384 B bound. The per-call ceilings below are measured delivered bytes plus a margin.
    /// docs/issues/2026-10-16-tracker-conventions-write-sections-are-declared-but-unreachable.md
    #[test]
    fn tracker_write_shapes_ship_their_sections_and_only_theirs() {
        let idx = GuideIndex::try_build().unwrap();
        let whole = crate::prompts::topic_body("tracker-conventions")
            .unwrap()
            .len();
        let t = serde_json::json!({"abs_path": "docs/trackers/x.md"});
        let i = serde_json::json!({"abs_path": "docs/issues/x.md"});
        let none = serde_json::json!({});
        let upd_t = ("doc.update", &t);
        let app_t = ("doc.append_entry", &t);
        let app_i = ("doc.append_entry", &i);
        let ue_t = ("doc.update_entry", &t);
        let aug_t = ("doc.augment", &t);
        let aug_i = ("doc.augment", &i);
        let lnk_t = ("doc.link", &t);
        let lnk_i = ("doc.link", &i);
        // A write whose result names no path routes nowhere near this topic.
        let app_none = ("doc.append_entry", &none);

        type Call<'a> = (&'a str, &'a Value);
        // (heading, calls that must serve it, calls that must not)
        let table: Vec<(&str, Vec<Call>, Vec<Call>)> = vec![
            (
                "Declaring an augmentation",
                vec![aug_t],
                vec![aug_i, app_t, ue_t, lnk_t, upd_t],
            ),
            (
                "Declaring an augmentation — why the sidecar exists, and the check's remedy",
                vec![aug_t],
                vec![aug_i, app_t, ue_t, lnk_t, upd_t],
            ),
            (
                "Entry-level standard — the shape INSIDE a tracker",
                vec![app_t, ue_t],
                vec![app_i, app_none, aug_t, lnk_t, upd_t],
            ),
            (
                "Entry ids",
                vec![app_t],
                vec![ue_t, app_i, aug_t, lnk_t, upd_t],
            ),
            (
                "Entry ids — refusals: main checkout only (prose ledgers)",
                vec![app_t],
                vec![ue_t, app_i, aug_t, lnk_t, upd_t],
            ),
            (
                "Entry ids — where the counter lives",
                vec![app_t],
                vec![ue_t, app_i, aug_t, lnk_t, upd_t],
            ),
            (
                "Entry headings — the definition rule",
                vec![app_t],
                vec![ue_t, app_i, aug_t, lnk_t, upd_t],
            ),
            (
                "Required fields",
                vec![app_t, ue_t],
                vec![app_i, aug_t, lnk_t, upd_t],
            ),
            (
                "Required fields — Rests on, and detecting the lines",
                vec![app_t, ue_t],
                vec![app_i, aug_t, lnk_t, upd_t],
            ),
            (
                "Detecting these fields",
                vec![app_t, ue_t],
                vec![app_i, aug_t, lnk_t, upd_t],
            ),
            (
                "Citing an entry — bare, or qualified",
                vec![ue_t],
                vec![app_t, aug_t, lnk_t, upd_t],
            ),
            (
                "Citing an entry — a third shape, and mentioning without citing",
                vec![ue_t],
                vec![app_t, aug_t, lnk_t, upd_t],
            ),
            (
                "Cross-linking (edges are derived — cite in prose)",
                vec![lnk_t, lnk_i],
                vec![app_t, ue_t, aug_t, upd_t],
            ),
        ];

        let served = |call: Call| -> Vec<String> {
            idx.match_sections("tracker-conventions", Some(call.0), call.1)
                .iter()
                .map(|s| s.heading.clone())
                .collect()
        };
        for (heading, must, must_not) in &table {
            for call in must {
                assert!(
                    served(*call).iter().any(|h| h == heading),
                    "`{}` on {} must be served § {heading}",
                    call.0,
                    call.1
                );
            }
            for call in must_not {
                assert!(
                    !served(*call).iter().any(|h| h == heading),
                    "`{}` on {} must NOT be served § {heading}",
                    call.0,
                    call.1
                );
            }
        }

        // The table is closed over the guide: a section declared for one of these write
        // shapes that has no row above is untested, and a row naming a heading that is not
        // declared for one proves nothing. Compared as sets, both ways, so neither a new
        // section nor a rename can pass silently.
        let writes = ["update", "append_entry", "update_entry", "augment", "link"];
        let mut declared: Vec<&str> = idx
            .topic("tracker-conventions")
            .unwrap()
            .declared()
            .filter(|s| {
                s.serves
                    .iter()
                    .any(|sh| sh.action.as_deref().is_some_and(|a| writes.contains(&a)))
            })
            .map(|s| s.heading.as_str())
            .collect();
        declared.sort_unstable();
        let mut tabled: Vec<&str> = table.iter().map(|(h, _, _)| *h).collect();
        tabled.sort_unstable();
        assert_eq!(
            declared, tabled,
            "the table and the guide's write-shape sections must be the same set"
        );

        // `doc.update` is never served from this topic, on a tracker, a bug file or no path.
        for (what, probe) in [("tracker", &t), ("bug file", &i), ("no path", &none)] {
            assert!(
                served(("doc.update", probe)).is_empty(),
                "`doc.update` on a {what} result must serve nothing from tracker-conventions: \
                 its anti-wipe sections live in librarian"
            );
        }

        // What each call DELIVERS: the wrapped blocks, bounded by the auto-inject bound, by the
        // section-grain ratio, and by a measured ceiling (delivered bytes + 128 B; raise it
        // deliberately, with the new measurement, never to make this pass).
        let delivered = |call: Call| -> usize {
            idx.match_sections("tracker-conventions", Some(call.0), call.1)
                .iter()
                .map(|s| {
                    crate::tools::guide_emit::section_block("tracker-conventions", s)
                        .as_text()
                        .map_or(0, |b| b.text.len())
                })
                .sum()
        };
        let mut calls: Vec<Call> = table
            .iter()
            .flat_map(|(_, m, n)| m.iter().chain(n.iter()).copied())
            .collect();
        calls.sort_by(|a, b| (a.0, a.1.to_string()).cmp(&(b.0, b.1.to_string())));
        calls.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
        for call in calls {
            let bytes = delivered(call);
            assert!(
                bytes * 4 < whole,
                "`{}` on {} delivers {bytes} B of a {whole} B guide; that is not section grain",
                call.0,
                call.1
            );
        }
        // The ceilings themselves sit under the auto-inject bound (16,384 B, wrapper included).
        // A per-call `<= MAX` check would be dominated by the tighter ceilings (the first guard
        // to refuse owns the input, so a mutation of it survives); bounding the ceilings is the
        // assertion that fails when one is RAISED past the bound.
        for ceiling in [
            WRITE_CEILING_APPEND_ENTRY,
            WRITE_CEILING_UPDATE_ENTRY,
            WRITE_CEILING_AUGMENT,
            WRITE_CEILING_LINK,
        ] {
            assert!(
                ceiling <= crate::tools::guide_emit::MAX_AUTO_INJECT_GUIDE_BYTES,
                "a measured ceiling of {ceiling} B is over the auto-inject bound"
            );
        }
        // The instrument's own floor: delivered bytes must INCLUDE the wrappers. A measure
        // that sums section bodies reads ~2.3 KB low for append_entry, which is exactly how
        // a 16,344 B delivery read as 13.5 KB against a 16,384 B bound.
        let floor_bytes = delivered(app_t);
        assert!(
            floor_bytes >= WRITE_FLOOR_APPEND_ENTRY,
            "`doc.append_entry` measures {floor_bytes} B, under the {WRITE_FLOOR_APPEND_ENTRY} B \
             floor: the delivered-bytes instrument has stopped counting what is injected"
        );
        for (call, measured_ceiling) in [
            (app_t, WRITE_CEILING_APPEND_ENTRY),
            (ue_t, WRITE_CEILING_UPDATE_ENTRY),
            (aug_t, WRITE_CEILING_AUGMENT),
            (lnk_t, WRITE_CEILING_LINK),
            (lnk_i, WRITE_CEILING_LINK),
        ] {
            let bytes = delivered(call);
            assert!(
                bytes <= measured_ceiling,
                "`{}` on {} delivers {bytes} B wrapped, over its measured ceiling \
                 {measured_ceiling} B",
                call.0,
                call.1
            );
        }

        // A write whose result names no tracker or bug path serves nothing from this topic.
        for sel in [
            "doc.update",
            "doc.append_entry",
            "doc.update_entry",
            "doc.augment",
            "doc.link",
        ] {
            assert!(
                idx.match_sections("tracker-conventions", Some(sel), &none)
                    .is_empty(),
                "`{sel}` with a result naming no path must serve no tracker-conventions section"
            );
        }
    }

    /// A `path~` shape can never match a result-less call, which is what keeps the
    /// pre-call first-use gate (`first_use_guide_gate`) from ever holding a call for a
    /// tracker-conventions section: the gate asks `topic_declaring` about the selector
    /// alone, with a null result. If this fails, a path-qualified declaration has become
    /// reachable before the call runs and the gate would stall a call to serve it.
    #[test]
    fn no_tracker_conventions_section_is_reachable_without_a_result() {
        let idx = GuideIndex::try_build().unwrap();
        let entry = idx.topic("tracker-conventions").expect("topic indexed");
        for sec in entry.declared() {
            for shape in &sec.serves {
                assert!(
                    shape.path_contains.is_some(),
                    "tracker-conventions § {} declares an unqualified shape `{}.{}`: it would \
                     overlap librarian's declaration and be reachable from the selector alone",
                    sec.heading,
                    shape.tool,
                    shape.action.as_deref().unwrap_or("*")
                );
            }
        }
        for action in [
            "create",
            "get",
            "find",
            "update",
            "move",
            "delete",
            "graft",
            "rekey_prefix",
            "append_entry",
            "update_entry",
            "link",
            "graph",
            "state_at",
            "augment",
        ] {
            let sel = format!("doc.{action}");
            assert_ne!(
                idx.topic_declaring(Some(&sel), &Value::Null),
                Some("tracker-conventions"),
                "`{sel}` with no result must not resolve to tracker-conventions"
            );
        }
    }

    #[test]
    fn declared_sections_are_within_the_size_cap() {
        // Gate 3, scoped to topics that have opted in. Widens automatically as
        // more topics land declarations. `str::len()` is bytes — these guides are
        // full of multi-byte em-dashes, so a char count would silently under-report.
        let idx = GuideIndex::try_build().unwrap();
        for topic in crate::prompts::GUIDE_TOPICS {
            let Some(entry) = idx.topic(topic) else {
                continue;
            };
            for sec in entry.declared() {
                assert!(
                    sec.body.len() <= MAX_DECLARED_SECTION_BYTES,
                    "{topic} § {} is {} B, over the {MAX_DECLARED_SECTION_BYTES} B cap. \
                 Decompose it at `###` and move the declaration onto the child \
                 sections. A slice this large is the failure this feature exists \
                 to fix.",
                    sec.heading,
                    sec.body.len()
                );
            }
        }
    }

    /// Every `get_guide("topic") § heading` citation in `body`, as `(topic, heading)`.
    ///
    /// A citation is `get_guide("t")` followed by ` § ` and a heading: `*emphasised*` (which may
    /// itself contain commas) or plain text up to the next comma. Lines inside a fenced block are
    /// skipped, because the guides teach call syntax by example and an example may be fictional;
    /// the cost is that a real citation placed inside a fence is not checked.
    fn citations(body: &str) -> Vec<(String, String)> {
        let mut found = Vec::new();
        let mut in_fence = false;
        for line in body.lines() {
            if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
                in_fence = !in_fence;
                continue;
            }
            if in_fence {
                continue;
            }
            let mut rest = line;
            while let Some(at) = rest.find("get_guide(") {
                rest = &rest[at + "get_guide(".len()..];
                let after_open = rest.trim_start_matches(['\\', '"']);
                let topic: String = after_open
                    .chars()
                    .take_while(|c| c.is_ascii_lowercase() || *c == '-')
                    .collect();
                if topic.is_empty() {
                    continue;
                }
                let after_topic = after_open[topic.len()..].trim_start_matches(['\\', '"']);
                let Some(after_paren) = after_topic.strip_prefix(')') else {
                    continue;
                };
                let Some(cited) = after_paren
                    .strip_prefix('`')
                    .unwrap_or(after_paren)
                    .strip_prefix(" § ")
                else {
                    continue;
                };
                let heading = match cited.strip_prefix('*') {
                    Some(emphasised) => emphasised.split('*').next().unwrap_or("").to_string(),
                    None => cited
                        .split(',')
                        .next()
                        .unwrap_or("")
                        .trim_end_matches('.')
                        .to_string(),
                };
                found.push((topic, heading.trim().to_string()));
            }
        }
        found
    }

    /// Whether `cited` names a heading in `headings`: an exact match, case-insensitively, or,
    /// failing that, a prefix of exactly one heading. The prefix arm exists because the guides
    /// cite `*Bug files*` for the heading `Bug files (docs/issues/)`; an exact match wins so a
    /// short heading is not made ambiguous by a longer one that starts with it.
    fn heading_resolves(cited: &str, headings: &[String]) -> bool {
        let want = cited.trim().to_lowercase();
        let lowered: Vec<String> = headings.iter().map(|h| h.to_lowercase()).collect();
        lowered.contains(&want) || lowered.iter().filter(|h| h.starts_with(&want)).count() == 1
    }

    /// `(citations checked, one message per citation that names no real heading)` over a
    /// `(topic, body)` corpus.
    fn check_citations(corpus: &[(&'static str, &'static str)]) -> (usize, Vec<String>) {
        let headings_of = |topic: &str| -> Option<Vec<String>> {
            let (_, body) = corpus.iter().find(|(t, _)| *t == topic)?;
            Some(
                split_sections(body)
                    .1
                    .into_iter()
                    .map(|s| s.heading)
                    .collect(),
            )
        };
        let mut checked = 0;
        let mut dangling = Vec::new();
        for (from, body) in corpus {
            for (topic, heading) in citations(body) {
                checked += 1;
                match headings_of(&topic) {
                    None => dangling.push(format!(
                        "{from} cites get_guide(\"{topic}\") § {heading}, but there is no topic `{topic}`"
                    )),
                    Some(headings) if !heading_resolves(&heading, &headings) => {
                        dangling.push(format!(
                            "{from} cites get_guide(\"{topic}\") § {heading}, which is not a heading of `{topic}`"
                        ))
                    }
                    Some(_) => {}
                }
            }
        }
        (checked, dangling)
    }

    #[test]
    fn every_section_citation_in_the_guides_resolves_to_a_heading() {
        // `get_guide` takes a topic and nothing narrower, so a caller cannot follow a
        // section-qualified citation with the tool: it must read the heading in the body it
        // already holds. A renamed heading turns the citation into a pointer at nothing.
        let corpus: Vec<(&'static str, &'static str)> = crate::prompts::GUIDE_TOPICS
            .iter()
            .map(|t| (*t, crate::prompts::topic_body(t).unwrap()))
            .collect();
        let (checked, dangling) = check_citations(&corpus);
        assert!(
            checked >= 5,
            "the citation scan checked {checked} citations; the guides held 5 when this test was \
             written (4 of them wrapped in backticks, which a first version of the scanner missed). \
             A drop means the scanner lost a citation form, or citations were removed on purpose: \
             lower this floor only for the second reason."
        );
        assert!(
            dangling.is_empty(),
            "dangling guide citations:\n{}",
            dangling.join("\n")
        );
    }

    #[test]
    fn a_citation_of_a_renamed_heading_is_reported() {
        let corpus = [
            ("a", "see get_guide(\"b\") § *Old name*.\n"),
            ("b", "## New name\nbody\n"),
        ];
        let (checked, dangling) = check_citations(&corpus);
        assert_eq!(checked, 1);
        assert_eq!(dangling.len(), 1, "{dangling:?}");
        assert!(dangling[0].contains("Old name"), "{dangling:?}");
    }

    #[test]
    fn a_citation_of_an_unknown_topic_is_reported() {
        let corpus = [("a", "see get_guide(\"nope\") § *Anything*.\n")];
        let (_, dangling) = check_citations(&corpus);
        assert_eq!(dangling.len(), 1, "{dangling:?}");
        assert!(dangling[0].contains("no topic `nope`"), "{dangling:?}");
    }

    #[test]
    fn a_citation_whose_call_is_wrapped_in_backticks_is_checked() {
        // Four of the five citations in the guides are written `get_guide("t")` § *H*, with the
        // closing backtick between the parenthesis and the section sign. A scanner that wants
        // " § " straight after ")" checks only the unwrapped one and passes with the rest broken.
        let corpus = [
            ("a", "see `get_guide(\"t\")` § *Gone*.\n"),
            ("t", "## Present\nx\n"),
        ];
        let (checked, dangling) = check_citations(&corpus);
        assert_eq!(checked, 1);
        assert_eq!(dangling.len(), 1, "{dangling:?}");
    }

    #[test]
    fn a_unique_prefix_of_a_heading_resolves_and_an_ambiguous_one_does_not() {
        let target = "## Bug files (docs/issues/)\nx\n## Declaring an augmentation — why\ny\n## Declaring an augmentation — how\nz\n";
        let unique = [("a", "get_guide(\"t\") § *Bug files*.\n"), ("t", target)];
        assert!(check_citations(&unique).1.is_empty());
        let ambiguous = [
            ("a", "get_guide(\"t\") § *Declaring an augmentation*.\n"),
            ("t", target),
        ];
        assert_eq!(check_citations(&ambiguous).1.len(), 1);
    }

    #[test]
    fn an_exact_heading_beats_a_longer_heading_that_starts_with_it() {
        let target = "## Declaring an augmentation\nx\n### Declaring an augmentation — why\ny\n";
        let corpus = [
            ("a", "get_guide(\"t\") § *Declaring an augmentation*.\n"),
            ("t", target),
        ];
        assert!(check_citations(&corpus).1.is_empty());
    }

    #[test]
    fn a_plain_heading_ends_at_the_comma_and_an_emphasised_one_may_contain_it() {
        let target = "## Path-relative annotation\nx\n### One entry format, never two\ny\n";
        let corpus = [
            (
                "a",
                "is get_guide(\"t\") § Path-relative annotation, which owns it.\n",
            ),
            (
                "b",
                "get_guide(\"t\") § *One entry format, never two* has the numbers.\n",
            ),
            ("t", target),
        ];
        let (checked, dangling) = check_citations(&corpus);
        assert_eq!(checked, 2);
        assert!(dangling.is_empty(), "{dangling:?}");
    }

    #[test]
    fn a_citation_inside_a_fenced_example_is_not_checked() {
        let corpus = [
            ("a", "```\nget_guide(\"t\") § *Fictional*\n```\n"),
            ("t", "## Real\nx\n"),
        ];
        let (checked, dangling) = check_citations(&corpus);
        assert_eq!(checked, 0);
        assert!(dangling.is_empty());
    }
    /// Every bare `§ heading` citation in `body` — one naming a section of the SAME guide, with no
    /// `get_guide(...)` in front of it — as the cited heading text.
    ///
    /// The grammar, fitted to the corpus on 2026-11-04:
    /// - A `§` preceded (ignoring spaces) by `)` or a backtick is not bare: `get_guide("t")` § H is
    ///   [`citations`]' form, and `` `CLAUDE.md` `` § H names a file. So does a preceding word ending
    ///   in `.md` or written in capitals (`CLAUDE.md § H`, `README § H`).
    /// - An `*emphasised*` heading runs to the closing `*`, and may wrap onto the next line
    ///   (`librarian.md` cites `§ *doc — Event` / `Log*`).
    /// - A plain heading runs to the first `,` `:` `;` `(` `)`, ` — `, `. ` or next `§`; a trailing
    ///   `.` and a trailing ` below` / ` above` are dropped. A heading containing one of those must
    ///   be cited emphasised.
    /// - Lines inside a fenced block are skipped, as in [`citations`], and so are lines indented
    ///   four or more spaces — an indented code block: `tracker-conventions.md` quotes a `## Resume`
    ///   banner that way, and its `§ *Integration*` names a section of the quoted tracker, not of
    ///   the guide. The cost is the same as for fences: a real citation placed there is unchecked.
    fn bare_citations(body: &str) -> Vec<String> {
        let mut prose: Vec<&str> = Vec::new();
        let mut in_fence = false;
        for line in body.lines() {
            if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
                in_fence = !in_fence;
                continue;
            }
            if in_fence || line.starts_with("    ") || line.starts_with('\t') {
                continue;
            }
            prose.push(line);
        }
        let names_a_document = |word: &str| {
            word.ends_with(".md")
                || (word.chars().count() >= 2
                    && word.chars().any(|c| c.is_ascii_uppercase())
                    && word.chars().all(|c| {
                        c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_' || c == '-'
                    }))
        };
        let mut found = Vec::new();
        for (i, line) in prose.iter().enumerate() {
            for (at, _) in line.match_indices('§') {
                let before = line[..at].trim_end();
                if before.ends_with(')') || before.ends_with('`') {
                    continue;
                }
                if before
                    .split_whitespace()
                    .last()
                    .is_some_and(names_a_document)
                {
                    continue;
                }
                let after = line[at + '§'.len_utf8()..].trim_start();
                let heading = match after.strip_prefix('*') {
                    Some(emphasised) => match emphasised.find('*') {
                        Some(end) => emphasised[..end].to_string(),
                        None => {
                            let next = prose.get(i + 1).map(|l| l.trim_start()).unwrap_or("");
                            let tail = next.split('*').next().unwrap_or("");
                            format!("{} {}", emphasised.trim_end(), tail)
                        }
                    },
                    None => {
                        let mut end = after.len();
                        for stop in [",", ":", ";", "(", ")", " — ", ". ", "§"] {
                            if let Some(p) = after.find(stop) {
                                end = end.min(p);
                            }
                        }
                        let plain = after[..end].trim().trim_end_matches('.');
                        plain
                            .strip_suffix(" below")
                            .or_else(|| plain.strip_suffix(" above"))
                            .unwrap_or(plain)
                            .to_string()
                    }
                };
                found.push(heading.trim().to_string());
            }
        }
        found
    }

    /// `(bare citations checked, one message per bare citation naming no heading of its own guide)`.
    fn check_bare_citations(corpus: &[(&'static str, &'static str)]) -> (usize, Vec<String>) {
        let mut checked = 0;
        let mut dangling = Vec::new();
        for (from, body) in corpus {
            let headings: Vec<String> = split_sections(body)
                .1
                .into_iter()
                .map(|s| s.heading)
                .collect();
            for heading in bare_citations(body) {
                checked += 1;
                if !heading_resolves(&heading, &headings) {
                    dangling.push(format!(
                        "{from} cites § {heading}, which is not a heading of `{from}`"
                    ));
                }
            }
        }
        (checked, dangling)
    }

    #[test]
    fn every_bare_section_citation_in_a_guide_resolves_to_a_heading_of_that_guide() {
        // The intra-guide twin of the `get_guide("t") § H` lint above. A bare `§ H` is followed by
        // reading the guide already in hand, so a renamed heading leaves it pointing at nothing —
        // `tracker-conventions.md`'s "see § Status vocabulary" did exactly that until it was fixed
        // by hand on 2026-11-04.
        let corpus: Vec<(&'static str, &'static str)> = crate::prompts::GUIDE_TOPICS
            .iter()
            .map(|t| (*t, crate::prompts::topic_body(t).unwrap()))
            .collect();
        let (checked, dangling) = check_bare_citations(&corpus);
        assert!(
            checked >= 5,
            "the bare-citation scan checked {checked}; the guides held 5 when this test was written \
         (librarian ×2, tracker-conventions ×1, workspace-state ×2). A drop means the scanner lost \
         a form, or citations were removed on purpose: lower this floor only for the second reason."
        );
        assert!(
            dangling.is_empty(),
            "dangling bare § citations:\n{}",
            dangling.join("\n")
        );
    }

    #[test]
    fn a_bare_citation_of_a_missing_heading_is_reported() {
        let corpus = [(
            "t",
            "## Bug frontmatter and status vocabulary\nx\n## Other\nsee § Status vocabulary. Also \
         see § *Bug frontmatter*.\n",
        )];
        let (checked, dangling) = check_bare_citations(&corpus);
        assert_eq!(checked, 2, "{dangling:?}");
        assert_eq!(dangling.len(), 1, "{dangling:?}");
        assert!(dangling[0].contains("Status vocabulary"), "{dangling:?}");
    }

    #[test]
    fn a_plain_bare_heading_ends_at_its_terminator() {
        // The five shapes the real corpus uses, each against its own heading.
        let body = "## Per-session state reset\nx\n## Worktree overlay\ny\n\
                ## Bug frontmatter and status vocabulary\nz\n\
                the predicate in § Per-session state reset: same-project keeps it\n\
                depends on § Per-session state reset — don't assume it\n\
                See § Worktree overlay below.\n\
                (see § Bug frontmatter and status vocabulary). Filter\n\
                ends the line § Worktree overlay\n";
        assert_eq!(
            bare_citations(body),
            vec![
                "Per-session state reset",
                "Per-session state reset",
                "Worktree overlay",
                "Bug frontmatter and status vocabulary",
                "Worktree overlay",
            ]
        );
        assert!(check_bare_citations(&[("t", body)]).1.is_empty());
    }

    #[test]
    fn an_emphasised_bare_heading_may_wrap_onto_the_next_line() {
        let body = "## doc — Event Log\nx\n## Other\n**read it.** (After the fact, § *doc — Event\nLog*.) To add\n";
        assert_eq!(bare_citations(body), vec!["doc — Event Log"]);
        assert!(check_bare_citations(&[("t", body)]).1.is_empty());
    }

    #[test]
    fn citations_of_other_documents_and_of_code_are_not_bare() {
        let body = "## Real\nx\n\
                the four commands in `CLAUDE.md` § *Development Commands*\n\
                see CLAUDE.md § Git Workflow and README § Install\n\
                `get_guide(\"t\")` § *Real* and get_guide(\"t\") § Real\n\
                ```\nsee § Fictional\n```\n\
                \x20   > historical record. Read § *Integration* for the current state.\n";
        assert_eq!(bare_citations(body), Vec::<String>::new());
    }

    #[test]
    fn librarian_declares_the_six_highest_volume_artifact_shapes() {
        // These six are 9,695 of 10,920 observed artifact/librarian calls (89%).
        let idx = GuideIndex::try_build().unwrap();
        let entry = idx.topic("librarian").expect("librarian in index");
        for shape in [
            "doc.update",
            "doc.get",
            "doc.find",
            "doc.append_entry",
            "doc.create",
            "doc.move",
        ] {
            let sel = Some(shape);
            let hits = idx.match_sections("librarian", sel, &serde_json::json!({}));
            assert!(!hits.is_empty(), "no librarian section serves `{shape}`");
        }
        assert!(entry.declared().next().is_some());
    }

    #[test]
    fn no_dangling_requires() {
        // Gate 4.
        let idx = GuideIndex::try_build().unwrap();
        for topic in crate::prompts::GUIDE_TOPICS {
            let Some(entry) = idx.topic(topic) else {
                continue;
            };
            for sec in &entry.sections {
                for req in &sec.requires {
                    assert!(
                        entry.sections.iter().any(|s| &s.heading == req),
                        "{topic} § {} requires `{req}`, which no heading in that topic defines",
                        sec.heading
                    );
                }
            }
        }
    }

    #[test]
    fn every_section_of_a_declaring_topic_is_reachable() {
        // Gate 5, scoped to opted-in topics. A section that is neither declared,
        // transitively required, nor waived is unreachable — the section-grain form
        // of "authoring a guide nothing fires is the same as deleting it".
        let idx = GuideIndex::try_build().unwrap();
        for topic in crate::prompts::GUIDE_TOPICS {
            if !idx.declares(topic) {
                continue;
            }
            let entry = idx.topic(topic).unwrap();
            let required: std::collections::BTreeSet<&str> = entry
                .sections
                .iter()
                .flat_map(|s| s.requires.iter().map(|r| r.as_str()))
                .collect();
            for sec in &entry.sections {
                let waived = crate::prompts::SECTION_WAIVERS
                    .iter()
                    .any(|(t, h, _)| *t == *topic && *h == sec.heading);
                assert!(
                    !sec.serves.is_empty() || required.contains(sec.heading.as_str()) || waived,
                    "{topic} § {} is unreachable: declare a `serves:`, have a section \
                 `requires:` it, or add it to prompts::SECTION_WAIVERS with a reason",
                    sec.heading
                );
            }
        }
    }
}
