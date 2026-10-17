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
        let Some(inner) = t.strip_prefix("<!--").and_then(|r| r.strip_suffix("-->")) else {
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
    /// is reachable by the call that needs it, and by no call that does not.
    ///
    /// The routing half is proved separately, against real `doc` calls
    /// (`server::guide_hint_tests::a_write_to_a_tracker_or_bug_file_is_routed_to_its_section`);
    /// this table is the declaration half: for each section, the calls that MUST serve it and
    /// at least one that must NOT. Both directions, because each alone is monotone — "every
    /// section is served by something" is satisfied by a `serves:` widened to every write
    /// (over-reporting), and "no section leaks" by an emptied declaration. The `doc.move`
    /// archive sections have their own cases in
    /// `tracker_conventions_ships_sections_to_each_shape_that_routes_to_it`; here `move` is
    /// only ever a negative control.
    ///
    /// The table is closed over the guide, compared as sets: a `serves:` on one of these
    /// write shapes with no row here fails, rather than passing untested.
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
        let upd_i = ("doc.update", &i);
        let mov_t = ("doc.move", &t);
        let mov_i = ("doc.move", &i);
        let app_t = ("doc.append_entry", &t);
        let app_i = ("doc.append_entry", &i);
        let ue_t = ("doc.update_entry", &t);
        // A write whose result names no path routes nowhere near this topic.
        let app_none = ("doc.append_entry", &none);

        type Call<'a> = (&'a str, &'a Value);
        // (heading, calls that must serve it, calls that must not)
        let table: Vec<(&str, Vec<Call>, Vec<Call>)> = vec![
            (
                "Settled caveats — STANDING and TRACKED",
                vec![upd_i],
                vec![upd_t, mov_i, app_t],
            ),
            (
                "Closing a bug — patch-id and fix provenance",
                vec![upd_i],
                vec![mov_i, upd_t],
            ),
            (
                "Fix provenance — plural, cross-repo, and no commit",
                vec![upd_i],
                vec![mov_i, upd_t],
            ),
            (
                "Fix provenance — a merge commit's patch-id",
                vec![upd_i],
                vec![mov_i, upd_t],
            ),
            (
                "Fix provenance — nothing is owed to master",
                vec![upd_i],
                vec![mov_i, upd_t],
            ),
            (
                "Archiving a tracker",
                vec![upd_t],
                vec![upd_i, mov_t, app_t],
            ),
            (
                "Entry-level standard — the shape INSIDE a tracker",
                vec![app_t, ue_t],
                vec![app_i, app_none, mov_t, upd_t],
            ),
            ("Entry ids", vec![app_t], vec![ue_t, app_i, upd_t, mov_t]),
            (
                "Entry ids — refusals: main checkout only, and unpushed ledgers",
                vec![app_t],
                vec![ue_t, app_i, upd_t, mov_t],
            ),
            (
                "Entry ids — where the counter lives",
                vec![app_t],
                vec![ue_t, app_i, upd_t, mov_t],
            ),
            (
                "Entry headings — the definition rule",
                vec![app_t],
                vec![ue_t, app_i, upd_t, mov_t],
            ),
            (
                "One entry format, never two",
                vec![app_t],
                vec![ue_t, app_i, upd_t, mov_t],
            ),
            (
                "Required fields",
                vec![app_t, ue_t],
                vec![app_i, upd_t, mov_t],
            ),
            (
                "Required fields — Rests on, and detecting the lines",
                vec![app_t, ue_t],
                vec![app_i, upd_t, mov_t],
            ),
            (
                "Detecting these fields",
                vec![app_t, ue_t],
                vec![app_i, upd_t, mov_t],
            ),
            (
                "Compaction and archival",
                vec![app_t],
                vec![ue_t, upd_t, mov_t],
            ),
            (
                "Citing an entry — bare, or qualified",
                vec![ue_t],
                vec![app_t, upd_t, mov_t],
            ),
            (
                "Citing an entry — a third shape, and mentioning without citing",
                vec![ue_t],
                vec![app_t, upd_t, mov_t],
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
        let writes = ["update", "append_entry", "update_entry"];
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

        // Every call the table names ships a bounded set, not the guide.
        let mut calls: Vec<Call> = table
            .iter()
            .flat_map(|(_, m, n)| m.iter().chain(n.iter()).copied())
            .collect();
        calls.sort_by(|a, b| (a.0, a.1.to_string()).cmp(&(b.0, b.1.to_string())));
        calls.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
        for call in calls {
            let bytes: usize = idx
                .match_sections("tracker-conventions", Some(call.0), call.1)
                .iter()
                .map(|s| s.body.len())
                .sum();
            assert!(
                bytes <= crate::tools::guide_emit::MAX_AUTO_INJECT_GUIDE_BYTES,
                "`{}` on {} ships {bytes} B — over the auto-inject bound",
                call.0,
                call.1
            );
            assert!(
                bytes * 4 < whole,
                "`{}` on {} ships {bytes} B of a {whole} B guide; that is not section grain",
                call.0,
                call.1
            );
        }

        // A write whose result names no tracker or bug path serves nothing from this topic.
        for sel in ["doc.update", "doc.append_entry", "doc.update_entry"] {
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
