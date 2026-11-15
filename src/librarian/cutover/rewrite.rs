//! The citation rewrite: old citation forms to the closed grammar, one document at a time.
//!
//! Spans come from the two real scanners ([`extract_spanned`] and [`citation_spans`]) and
//! edits apply to the original bytes, right to left. A citation is rewritten only when its
//! target resolves through the tool's own [`KeyIndex`] to exactly one document (D-R5, D-R30).
//! Anything else is reported and left byte for byte as it was.

use std::collections::{BTreeMap, HashSet};
use std::ops::Range;
use std::sync::OnceLock;

use pulldown_cmark::{Event, LinkType, Options, Parser, Tag};
use regex::Regex;
use serde::Serialize;

use crate::librarian::identity::grammar::{extract_spanned, in_url_word, Reference};
use crate::librarian::identity::index::{defined_tokens, Definer, KeyIndex, Resolution};
use crate::librarian::identity::key::{folder_of, key_of, Key};
use crate::librarian::identity::repo_name::RepoName;
use crate::librarian::tools::link_scan::extract::{citation_spans, CitationKind, CitationSpan};

/// The reason of a 16-hex id that neither the old catalog nor git history names. A shallow clone
/// replaces it, because history is then incomplete.
pub const REASON_UNKNOWN_ID: &str = "no document of the old catalog or of git history has this id";

/// Where an id's target was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdSource {
    Catalog,
    History,
}

/// The file an old artifact id names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdTarget {
    pub rel_path: String,
    pub source: IdSource,
}

/// What the rewrite resolves against.
pub struct Resolver<'a> {
    pub index: &'a KeyIndex,
    pub this_repo: &'a RepoName,
    /// Old key (the lowercase stem) of a renamed file to the key it has now.
    pub renamed_keys: &'a BTreeMap<String, Key>,
    /// Old path of a renamed file to its path now: a link written with the old path follows it.
    pub renamed_paths: &'a BTreeMap<String, String>,
    /// Old key to the HEAD documents that could be what the key named: a renamed file and another
    /// document with the same lowercase stem whose rename is not applied. Such a key is never
    /// followed (D-R7: ambiguous targets are reported, never guessed).
    pub ambiguous_keys: &'a BTreeMap<String, Vec<String>>,
    /// Old file stem (as written in the file name) to the final paths of the files that carried it.
    pub stems: &'a BTreeMap<String, Vec<String>>,
    /// A 16-hex id to the file it names.
    pub ids: &'a BTreeMap<String, IdTarget>,
    /// A 16-hex id to the historical path it hashes, reported with a suggestion.
    pub id_hints: &'a BTreeMap<String, String>,
}

/// The shape of a citation as written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Form {
    RenamedKey,
    StemToken,
    RepoToken,
    PathLink,
    HexId,
    BareToken,
}

impl Form {
    /// The name the report uses.
    pub const fn label(self) -> &'static str {
        match self {
            Form::RenamedKey => "renamed key",
            Form::StemToken => "stem token",
            Form::RepoToken => "repo token",
            Form::PathLink => "path link",
            Form::HexId => "hex id",
            Form::BareToken => "bare token",
        }
    }
}

/// What happened to one citation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Rewritten {
        to: String,
    },
    Unresolved {
        reason: String,
        suggestion: Option<String>,
    },
    Ambiguous {
        candidates: Vec<String>,
    },
    InCode,
}

/// One citation the rewrite looked at, and what it did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub form: Form,
    /// 1-based line in the text the rewrite read.
    pub line: u32,
    /// The bytes of the citation as written.
    pub raw: String,
    pub outcome: Outcome,
}

/// One replacement of a byte range of the original text.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Edit {
    range: Range<usize>,
    to: String,
}

/// Apply `edits` to `text`, right to left on the original bytes. `Err` when two edits overlap
/// or a range is not on char boundaries of `text`: nothing is applied then.
fn apply_edits(text: &str, mut edits: Vec<Edit>) -> Result<String, ()> {
    edits.sort_by_key(|e| (e.range.start, e.range.end));
    if edits.windows(2).any(|w| w[0].range.end > w[1].range.start) {
        return Err(());
    }
    let on_boundaries = |e: &Edit| {
        e.range.start <= e.range.end
            && e.range.end <= text.len()
            && text.is_char_boundary(e.range.start)
            && text.is_char_boundary(e.range.end)
    };
    if !edits.iter().all(on_boundaries) {
        return Err(());
    }
    let mut out = text.to_owned();
    for edit in edits.iter().rev() {
        out.replace_range(edit.range.clone(), &edit.to);
    }
    Ok(out)
}

/// Rewrite the old citation forms of the document at `rel_path`. Returns the new text and one
/// finding per citation looked at.
pub fn rewrite_document(rel_path: &str, text: &str, r: &Resolver) -> (String, Vec<Finding>) {
    let mut pass = Pass::new(rel_path, text, r);
    pass.scan();
    pass.finish()
}

const REASON_URL: &str = "inside a URL";
const REASON_NO_RANGE: &str = "escape or entity in the text run";
const REASON_BACKSLASH: &str = "escape";
const REASON_ENTITY: &str = "entity";
const REASON_MARKUP: &str = "markup or a path next to the token";
const REASON_LINK_MARKUP: &str = "markup next to the link";
const REASON_CROSS_REPO: &str = "cross repo";
const REASON_NO_DOCUMENT: &str = "the target is not a document with a unique key";
const REASON_NOT_RENDERABLE: &str = "the rendered form does not resolve";

/// The name `Form` takes for a scanner kind.
fn form_of_kind(kind: CitationKind) -> Form {
    match kind {
        CitationKind::EntryToken => Form::BareToken,
        CitationKind::ArtifactId => Form::HexId,
        CitationKind::RelPathLink => Form::PathLink,
        CitationKind::CrossRepoToken | CitationKind::MalformedQualifier => Form::RepoToken,
    }
}

/// A region of the document in which a citation is part of other markup.
struct Zone {
    range: Range<usize>,
    reason: &'static str,
}

/// One document's rewrite: the scanners' spans, the edits chosen and the findings made.
struct Pass<'a> {
    rel_path: &'a str,
    text: &'a str,
    r: &'a Resolver<'a>,
    line_starts: Vec<usize>,
    zones: Vec<Zone>,
    /// The byte ranges of the table cells the parser finds: a wiki-link label separator inside
    /// one is written `\|`.
    cells: Vec<Range<usize>>,
    /// Entry ids the document defines itself: a bare token that names one is local.
    local: HashSet<String>,
    edits: Vec<Edit>,
    /// Findings with the byte offset they sort by.
    findings: Vec<(usize, Finding)>,
}

impl<'a> Pass<'a> {
    fn new(rel_path: &'a str, text: &'a str, r: &'a Resolver<'a>) -> Pass<'a> {
        let line_starts = std::iter::once(0)
            .chain(text.match_indices('\n').map(|(i, _)| i + 1))
            .collect();
        let (zones, cells) = zones_of(text);
        Pass {
            rel_path,
            text,
            r,
            line_starts,
            zones,
            cells,
            local: defined_tokens(text).into_iter().collect(),
            edits: Vec::new(),
            findings: Vec::new(),
        }
    }

    /// 1-based line of a byte offset.
    fn line_at(&self, offset: usize) -> u32 {
        self.line_starts.partition_point(|&s| s <= offset) as u32
    }

    fn zone_of(&self, range: &Range<usize>) -> Option<&'static str> {
        self.zones
            .iter()
            .find(|z| z.range != *range && z.range.start <= range.start && range.end <= z.range.end)
            .map(|z| z.reason)
    }

    fn note(&mut self, form: Form, range: &Range<usize>, outcome: Outcome) {
        let line = self.line_at(range.start);
        let raw = self.text[range.clone()].to_owned();
        self.findings.push((
            range.start,
            Finding {
                form,
                line,
                raw,
                outcome,
            },
        ));
    }

    /// A finding with no range: it sorts at the start of its line.
    fn note_on_line(&mut self, form: Form, line: u32, raw: String, outcome: Outcome) {
        let pos = self.line_starts[(line as usize)
            .saturating_sub(1)
            .min(self.line_starts.len() - 1)];
        self.findings.push((
            pos,
            Finding {
                form,
                line,
                raw,
                outcome,
            },
        ));
    }

    fn unresolved(
        &mut self,
        form: Form,
        range: &Range<usize>,
        reason: impl Into<String>,
        suggestion: Option<String>,
    ) {
        let outcome = Outcome::Unresolved {
            reason: reason.into(),
            suggestion,
        };
        self.note(form, range, outcome);
    }

    fn ambiguous(&mut self, form: Form, range: &Range<usize>, candidates: Vec<String>) {
        self.note(form, range, Outcome::Ambiguous { candidates });
    }

    fn rewrite(&mut self, form: Form, range: Range<usize>, to: String) {
        self.note(form, &range, Outcome::Rewritten { to: to.clone() });
        self.edits.push(Edit { range, to });
    }

    /// `[[key:TOKEN]]`, when it resolves to an entry from this document.
    fn entry_form(&self, key: &Key, token: &str) -> Option<String> {
        self.render(None, key, Some(token))
    }

    /// `[[key]]`, when it resolves to a file from this document.
    fn file_form(&self, key: &Key) -> Option<String> {
        self.render(None, key, None)
    }

    /// The wiki link to `key` (to its entry `token`), written with `repo` when the old link was.
    /// `None` unless the index resolves exactly that form to a file (to an entry).
    fn render(&self, repo: Option<&RepoName>, key: &Key, token: Option<&str>) -> Option<String> {
        let prefix = repo.map(|r| format!("{}/", r.as_str())).unwrap_or_default();
        match token {
            Some(token) => {
                let reference = Reference::Entry {
                    repo: repo.cloned(),
                    key: key.clone(),
                    token: token.to_owned(),
                };
                matches!(
                    self.r.index.resolve(self.rel_path, &reference),
                    Resolution::Entry { .. }
                )
                .then(|| format!("[[{prefix}{key}:{token}]]"))
            }
            None => {
                let reference = Reference::File {
                    repo: repo.cloned(),
                    key: key.clone(),
                };
                matches!(
                    self.r.index.resolve(self.rel_path, &reference),
                    Resolution::File { .. }
                )
                .then(|| format!("[[{prefix}{key}]]"))
            }
        }
    }

    /// The rewrite of a token that one or several documents define, after every guard has
    /// passed: one definer gives `[[key:TOKEN]]`, several are reported and left alone.
    fn qualify(&mut self, form: Form, range: &Range<usize>, token: &str, definers: Vec<Definer>) {
        if definers.len() > 1 {
            let candidates = definers
                .iter()
                .map(|d| {
                    d.key
                        .as_ref()
                        .map_or_else(|| d.rel_path.clone(), Key::to_string)
                })
                .collect();
            self.ambiguous(form, range, candidates);
            return;
        }
        let Some(key) = definers.first().and_then(|d| d.key.as_ref()) else {
            let why = "the one document that defines the token has no unique key";
            self.unresolved(form, range, why, None);
            return;
        };
        match self.entry_form(key, token) {
            Some(to) => self.rewrite(form, range.clone(), to),
            None => self.unresolved(form, range, REASON_NOT_RENDERABLE, None),
        }
    }

    fn scan(&mut self) {
        let text = self.text;
        let grammar = extract_spanned(text);
        let spans = citation_spans(text);
        // Wiki links and the hits of the old scanner that hold other spans: a span inside one
        // of them is part of it, and an edit of both would overlap.
        let mut outer: Vec<Range<usize>> = grammar
            .iter()
            .filter(|g| !matches!(g.citation.reference, Reference::Bare { .. }))
            .map(|g| g.range.clone())
            .collect();
        outer.extend(
            spans
                .iter()
                .filter(|s| !s.in_code && s.kind != CitationKind::EntryToken)
                .filter_map(|s| s.range.clone()),
        );
        let inside_other = |range: &Range<usize>| {
            outer
                .iter()
                .any(|o| o != range && o.start <= range.start && range.end <= o.end)
        };

        for g in &grammar {
            match &g.citation.reference {
                Reference::Malformed { raw } => self.renamed_key(&g.range, raw),
                Reference::Bare { token } if !inside_other(&g.range) => {
                    self.bare_token(&g.range, token)
                }
                _ => {}
            }
        }
        for s in &spans {
            // Bare tokens come from the grammar alone.
            if s.kind == CitationKind::EntryToken && !s.in_code {
                continue;
            }
            if s.in_code {
                self.in_code(s);
                continue;
            }
            match &s.range {
                None => self.without_range(s),
                Some(range) if !inside_other(range) => self.hit(s, range),
                Some(_) => {}
            }
        }
    }

    /// A malformed wiki link whose target is a file the cutover renamed: `[[Old_Name]]` follows
    /// the rename, label and all (D-R15 keeps the label's bytes, not the target's). A malformed
    /// link with no rename on record is not this tool's to judge.
    ///
    /// The old key is the lowercase stem, so the lookup folds ASCII case. A `repo/` qualifier is
    /// kept when it names this repository. A qualifier that does not, and a `#fragment`, make the
    /// target unsafe to guess: they are reported with the new form as the suggestion. A key that
    /// another document of `HEAD` also has (its rename refused) is ambiguous and never followed.
    fn renamed_key(&mut self, range: &Range<usize>, raw: &str) {
        let (target, token) = match raw.rsplit_once(':') {
            Some((target, token)) if is_entry_token(token) => (target, Some(token)),
            _ => (raw, None),
        };
        let (target, fragment) = match target.split_once('#') {
            Some((target, fragment)) => (target, Some(fragment)),
            None => (target, None),
        };
        let (qualifier, stem) = match target.split_once('/') {
            Some((_, stem)) if stem.contains('/') => return,
            Some((qualifier, stem)) => (Some(qualifier), stem),
            None => (None, target),
        };
        let old_key = stem.to_ascii_lowercase();
        let ambiguous = self.r.ambiguous_keys.get(&old_key);
        let Some(new_key) = self.r.renamed_keys.get(&old_key) else {
            return;
        };
        let new_key = new_key.clone();
        if self.text.get(range.clone()).is_none() || in_url_word(self.text, range) {
            self.unresolved_or_skip(Form::RenamedKey, range, REASON_URL);
            return;
        }
        if let Some(candidates) = ambiguous {
            self.ambiguous(Form::RenamedKey, range, candidates.clone());
            return;
        }
        let this_repo = qualifier
            .and_then(|q| RepoName::parse(q).ok())
            .filter(|r| r == self.r.this_repo);
        if fragment.is_some() || (qualifier.is_some() && this_repo.is_none()) {
            let why = if fragment.is_some() {
                "a renamed file is named with a #fragment"
            } else {
                "a renamed file is named with a qualifier that is not this repository's"
            };
            let suggestion = self.render(this_repo.as_ref(), &new_key, token);
            self.unresolved(Form::RenamedKey, range, why, suggestion);
            return;
        }
        // What follows the target inside the brackets: nothing, or `|label` (`\|label` in a table
        // cell), copied as it is.
        let tail = self.text[range.clone()]
            .strip_prefix("[[")
            .and_then(|s| s.strip_suffix("]]"))
            .map(|inner| inner.trim_start_matches(|c: char| c.is_ascii_whitespace()))
            .and_then(|inner| inner.strip_prefix(raw))
            .map(|rest| rest.trim_start_matches(|c: char| c.is_ascii_whitespace()))
            .filter(|rest| rest.is_empty() || rest.starts_with('|') || rest.starts_with("\\|"))
            .map(str::to_owned);
        let Some(tail) = tail else {
            self.unresolved(
                Form::RenamedKey,
                range,
                "the wiki link to a renamed file is not written as [[target]] or [[target|label]]",
                None,
            );
            return;
        };
        let to = self
            .render(this_repo.as_ref(), &new_key, token)
            .and_then(|link| {
                link.strip_suffix("]]")
                    .map(|head| format!("{head}{tail}]]"))
            });
        match to {
            Some(to) => self.rewrite(Form::RenamedKey, range.clone(), to),
            None => self.unresolved(Form::RenamedKey, range, REASON_NOT_RENDERABLE, None),
        }
    }

    /// `unresolved` for a range that may not be on char boundaries: nothing is read from it then.
    fn unresolved_or_skip(&mut self, form: Form, range: &Range<usize>, reason: &str) {
        if self.text.get(range.clone()).is_some() {
            self.unresolved(form, range, reason, None);
        }
    }

    fn bare_token(&mut self, range: &Range<usize>, token: &str) {
        if self.local.contains(token)
            || matches!(
                self.r.index.resolve(
                    self.rel_path,
                    &Reference::Bare {
                        token: token.to_owned()
                    }
                ),
                Resolution::Local { .. }
            )
        {
            return;
        }
        let definers = self.r.index.definers_of(token);
        // A token no document defines is prose (an acronym), not a citation.
        if definers.is_empty() {
            return;
        }
        let form = Form::BareToken;
        if self.text.get(range.clone()).is_none() || in_url_word(self.text, range) {
            self.unresolved_or_skip(form, range, REASON_URL);
            return;
        }
        if let Some(word) = qualifier_word(self.text, range) {
            let why = format!("qualified by {word}, which is no file stem");
            self.unresolved(form, range, why, None);
            return;
        }
        if let Some(why) = self.zone_of(range) {
            self.unresolved(form, range, why, None);
            return;
        }
        if let Some(why) = token_neighbours(self.text, range) {
            self.unresolved(form, range, why, None);
            return;
        }
        self.qualify(form, range, token, definers);
    }

    /// A citation only inline code holds: counted, never edited. A bare token and an id are
    /// counted only when the tool would otherwise have acted on them.
    fn in_code(&mut self, s: &CitationSpan) {
        let counted = match s.kind {
            CitationKind::EntryToken => {
                !self.local.contains(&s.raw) && !self.r.index.definers_of(&s.raw).is_empty()
            }
            CitationKind::ArtifactId => {
                self.r.ids.contains_key(&s.raw) || self.r.id_hints.contains_key(&s.raw)
            }
            _ => true,
        };
        if counted {
            self.note_on_line(form_of_kind(s.kind), s.line, s.raw.clone(), Outcome::InCode);
        }
    }

    /// A hit with no range: the scanner could not say which bytes it came from.
    fn without_range(&mut self, s: &CitationSpan) {
        let form = form_of_kind(s.kind);
        let (reason, suggestion) = if s.kind == CitationKind::RelPathLink {
            let suggestion = self.target_key(&s.raw).and_then(|key| self.file_form(&key));
            ("reference-style link", suggestion)
        } else {
            (REASON_NO_RANGE, None)
        };
        let outcome = Outcome::Unresolved {
            reason: reason.to_owned(),
            suggestion,
        };
        self.note_on_line(form, s.line, s.raw.clone(), outcome);
    }

    /// The key of the document a link destination names, written in this document, after the renames.
    fn target_key(&self, dest: &str) -> Option<Key> {
        let target = join_path(&folder_of(self.rel_path), dest)?;
        self.r.index.key_at(self.current_path(&target)).cloned()
    }

    /// Where the document written at `path` in `HEAD` lies after the renames.
    fn current_path<'p>(&'p self, path: &'p str) -> &'p str {
        self.r.renamed_paths.get(path).map_or(path, String::as_str)
    }

    /// A hit of the old scanner with its bytes, not inside another span.
    fn hit(&mut self, s: &CitationSpan, range: &Range<usize>) {
        let form = form_of_kind(s.kind);
        if self.text.get(range.clone()).is_none() || in_url_word(self.text, range) {
            self.unresolved_or_skip(form, range, REASON_URL);
            return;
        }
        if s.kind == CitationKind::RelPathLink {
            self.path_link(s, range);
            return;
        }
        if let Some(why) = self.zone_of(range) {
            self.unresolved(form, range, why, None);
            return;
        }
        if let Some(why) = token_neighbours(self.text, range) {
            self.unresolved(form, range, why, None);
            return;
        }
        match s.kind {
            CitationKind::ArtifactId => self.hex_id(range, &s.raw),
            CitationKind::CrossRepoToken => self.qualified_token(range, &s.raw),
            CitationKind::MalformedQualifier => self.unresolved(
                form,
                range,
                "more than one qualifier before the token",
                None,
            ),
            CitationKind::EntryToken | CitationKind::RelPathLink => {}
        }
    }

    fn hex_id(&mut self, range: &Range<usize>, id: &str) {
        let form = Form::HexId;
        if let Some(target) = self.r.ids.get(id) {
            let key = self.r.index.key_at(&target.rel_path).cloned();
            match key.and_then(|key| self.file_form(&key)) {
                Some(to) => self.rewrite(form, range.clone(), to),
                None => self.unresolved(form, range, "the document has no unique key", None),
            }
        } else if let Some(historical) = self.r.id_hints.get(id) {
            let historical = self.current_path(historical);
            let suggestion = key_of(historical).ok().and_then(|key| self.file_form(&key));
            self.unresolved(
                form,
                range,
                "the id names a file that HEAD no longer holds",
                suggestion,
            );
        } else {
            self.unresolved(form, range, REASON_UNKNOWN_ID, None);
        }
    }

    /// `<qualifier>:TOKEN`: this repository's name reads as a bare token, a file stem names the
    /// one file of that name that defines the token, anything else is another repository.
    fn qualified_token(&mut self, range: &Range<usize>, raw: &str) {
        let Some((qualifier, token)) = raw.split_once(':') else {
            return;
        };
        if qualifier == self.r.this_repo.as_str() {
            let form = Form::RepoToken;
            if self.local.contains(token) {
                return;
            }
            let definers = self.r.index.definers_of(token);
            if definers.is_empty() {
                self.unresolved(form, range, "no document defines the token", None);
            } else {
                self.qualify(form, range, token, definers);
            }
            return;
        }
        let Some(paths) = self.r.stems.get(qualifier) else {
            self.unresolved(Form::RepoToken, range, REASON_CROSS_REPO, None);
            return;
        };
        let form = Form::StemToken;
        let definers = self.r.index.definers_of(token);
        let mut defining: Vec<Definer> = Vec::new();
        let mut without_key = 0usize;
        for path in paths {
            match self.r.index.key_at(path) {
                Some(key) => {
                    if let Some(found) = definers.iter().find(|d| d.key.as_ref() == Some(key)) {
                        defining.push(found.clone());
                    }
                }
                None => without_key += 1,
            }
        }
        match (defining.len(), without_key) {
            (0, 0) => self.unresolved(
                form,
                range,
                "the file of that name does not define the token",
                None,
            ),
            (0, _) => self.unresolved(form, range, "the file of that name has no unique key", None),
            (1, 0) => self.qualify(form, range, token, defining),
            // A file with no key cannot be asked whether it defines the token too.
            (1, _) => self.unresolved(
                form,
                range,
                "another file of that name has no unique key",
                None,
            ),
            _ => {
                defining.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
                self.qualify(form, range, token, defining)
            }
        }
    }

    /// A `[text](dest)` link to a markdown file becomes a wiki link.
    fn path_link(&mut self, s: &CitationSpan, range: &Range<usize>) {
        let form = Form::PathLink;
        if let Some(why) = self.zone_of(range).filter(|why| *why != "inside link text") {
            self.unresolved(form, range, why, None);
            return;
        }
        if link_neighbours_unsafe(self.text, range) {
            self.unresolved(form, range, REASON_LINK_MARKUP, None);
            return;
        }
        let link = &self.text[range.clone()];
        let Some((label, dest)) = split_inline_link(link) else {
            self.unresolved(
                form,
                range,
                "the link is not written as [text](destination)",
                None,
            );
            return;
        };
        let (path_part, fragment) = match dest.split_once('#') {
            Some((path, fragment)) => (path, Some(fragment)),
            None => (dest, None),
        };
        if !plain_destination(dest) || path_part != s.raw {
            self.unresolved(form, range, "the destination is not a plain path", None);
            return;
        }
        let Some(written) = join_path(&folder_of(self.rel_path), path_part) else {
            self.unresolved(
                form,
                range,
                "the link leaves the repository or names an absolute path",
                None,
            );
            return;
        };
        let Some(key) = self.r.index.key_at(self.current_path(&written)).cloned() else {
            self.unresolved(form, range, REASON_NO_DOCUMENT, None);
            return;
        };
        let Some(file_form) = self.file_form(&key) else {
            self.unresolved(form, range, REASON_NO_DOCUMENT, None);
            return;
        };
        if fragment.is_some() {
            self.unresolved(form, range, "link with a fragment", Some(file_form));
            return;
        }
        let file_name = written.rsplit('/').next().unwrap_or(&written);
        let to = if label == key.as_str() || label == file_name || label == path_part {
            file_form
        } else if label.contains('\n') {
            self.unresolved(form, range, "a line end in the link text", None);
            return;
        } else if label.is_empty() {
            self.unresolved(form, range, "the link text is empty", None);
            return;
        } else if !plain_label(label) {
            self.unresolved(form, range, "the link text is not plain", None);
            return;
        } else {
            let separator = if self.in_table_cell(range) {
                "\\|"
            } else {
                "|"
            };
            format!("[[{key}{separator}{label}]]")
        };
        self.rewrite(form, range.clone(), to);
    }

    /// The link lies inside a table cell the parser found (not merely on a line with a pipe).
    fn in_table_cell(&self, range: &Range<usize>) -> bool {
        self.cells
            .iter()
            .any(|cell| cell.start <= range.start && range.end <= cell.end)
    }

    /// Apply the edits; an overlap is a bug and rewrites nothing.
    fn finish(self) -> (String, Vec<Finding>) {
        let Pass {
            text,
            edits,
            mut findings,
            ..
        } = self;
        findings.sort_by_key(|(pos, _)| *pos);
        let mut findings: Vec<Finding> = findings.into_iter().map(|(_, f)| f).collect();
        match apply_edits(text, edits) {
            Ok(new_text) => (new_text, findings),
            Err(()) => {
                for finding in &mut findings {
                    if matches!(finding.outcome, Outcome::Rewritten { .. }) {
                        finding.outcome = Outcome::Unresolved {
                            reason: "not rewritten: two edits of this file overlap".to_owned(),
                            suggestion: None,
                        };
                    }
                }
                (text.to_owned(), findings)
            }
        }
    }
}

/// Headings, images and the text of links that are not wiki links, and separately the table cells,
/// with their byte ranges.
fn zones_of(text: &str) -> (Vec<Zone>, Vec<Range<usize>>) {
    let base = match crate::librarian::frontmatter::parse(text) {
        Ok((_, body)) => text.len() - body.len(),
        Err(_) => 0,
    };
    let options = Options::ENABLE_WIKILINKS | Options::ENABLE_TABLES;
    let mut zones = Vec::new();
    let mut cells = Vec::new();
    for (event, range) in Parser::new_ext(&text[base..], options).into_offset_iter() {
        let range = range.start + base..range.end + base;
        match event {
            Event::Start(Tag::Heading { .. }) => zones.push(Zone {
                range,
                reason: "inside a heading",
            }),
            Event::Start(Tag::Image { .. }) => zones.push(Zone {
                range,
                reason: "inside an image",
            }),
            Event::Start(Tag::Link { link_type, .. })
                if !matches!(link_type, LinkType::WikiLink { .. }) =>
            {
                zones.push(Zone {
                    range,
                    reason: "inside link text",
                })
            }
            Event::Start(Tag::TableCell) => cells.push(range),
            _ => {}
        }
    }
    (zones, cells)
}

fn entity_before() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"&(?:#[0-9]+|#[xX][0-9a-fA-F]+|[A-Za-z][A-Za-z0-9]*);$").expect("static regex")
    })
}

fn entity_after() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^&(?:#[0-9]+|#[xX][0-9a-fA-F]+|[A-Za-z][A-Za-z0-9]*);").expect("static regex")
    })
}

/// Why a `[[...]]` put in place of the bytes at `range` could change what the text reads: the
/// reason, or `None` when the neighbours are plain. The bytes next to the range must be blanks or
/// punctuation that ends or starts a word, with at most emphasis markers between. An escape, an
/// entity, a bracket, a quote of code or a path character beside it decides nothing safely.
/// `range` is on char boundaries of `text`.
fn token_neighbours(text: &str, range: &Range<usize>) -> Option<&'static str> {
    let before = &text[..range.start];
    let after = &text[range.end..];
    if before.ends_with('\\') || after.starts_with('\\') {
        return Some(REASON_BACKSLASH);
    }
    if entity_before().is_match(before) || entity_after().is_match(after) {
        return Some(REASON_ENTITY);
    }
    let before_ok = match before.trim_end_matches(['*', '~']).chars().next_back() {
        None => true,
        Some(c) => c.is_whitespace() || "(\"'>|{,—–“‘".contains(c),
    };
    let core = after.trim_start_matches(['*', '~']);
    let after_ok = match core.chars().next() {
        None => true,
        Some(c) if c.is_whitespace() => true,
        // A full stop ends the token only at the end of a sentence, not inside `F-1.md`.
        Some('.') => core[1..]
            .chars()
            .next()
            .is_none_or(|n| n.is_whitespace() || matches!(n, ')' | '"' | '*' | '\'')),
        Some(c) => ")\"'<|},;:?!—–”’".contains(c),
    };
    (!(before_ok && after_ok)).then_some(REASON_MARKUP)
}

/// A link followed by `(` or `[` would join the next bracket (`[[b]](x)` is a Markdown link),
/// and one that sits against a bracket, a `!`, an escape or an entity is not a link of its own.
fn link_neighbours_unsafe(text: &str, range: &Range<usize>) -> bool {
    matches!(
        text[..range.start].chars().next_back(),
        Some('\\' | '[' | ']' | '!' | '&')
    ) || matches!(
        text[range.end..].chars().next(),
        Some('(' | '[' | '\\' | '&')
    )
}

/// The word before a `:` directly in front of `range`: `My_Notes` for `My_Notes:F-2`.
fn qualifier_word<'t>(text: &'t str, range: &Range<usize>) -> Option<&'t str> {
    let before = text[..range.start].strip_suffix(':')?;
    let word_start = before
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_alphanumeric() || *c == '_' || *c == '-')
        .last()
        .map(|(i, _)| i)?;
    Some(&before[word_start..])
}

/// `F-3`: one to three capitals, a hyphen, digits.
fn is_entry_token(s: &str) -> bool {
    s.split_once('-').is_some_and(|(letters, digits)| {
        (1..=3).contains(&letters.len())
            && letters.bytes().all(|b| b.is_ascii_uppercase())
            && !digits.is_empty()
            && digits.bytes().all(|b| b.is_ascii_digit())
    })
}

/// `[text](dest)` as `(text, dest)`.
fn split_inline_link(link: &str) -> Option<(&str, &str)> {
    let inner = link.strip_prefix('[')?.strip_suffix(')')?;
    let at = inner.rfind("](")?;
    Some((&inner[..at], &inner[at + 2..]))
}

/// A destination that the scanner's decoded form equals byte for byte: no blank, bracket,
/// quote, escape, entity or percent escape.
fn plain_destination(dest: &str) -> bool {
    !dest.is_empty()
        && !dest
            .chars()
            .any(|c| c.is_whitespace() || "()<>\\&%\"'".contains(c))
}

/// Link text that a wiki-link label shows as written: no markup and no separator. An underscore
/// is plain only inside a word.
fn plain_label(label: &str) -> bool {
    if label != label.trim() {
        return false;
    }
    let chars: Vec<char> = label.chars().collect();
    chars.iter().enumerate().all(|(i, &c)| match c {
        '`' | '*' | '~' | '\\' | '&' | '<' | '>' | '!' | '[' | ']' | '|' | '\n' => false,
        '_' => {
            i > 0
                && chars.get(i + 1).is_some_and(|n| n.is_alphanumeric())
                && chars[i - 1].is_alphanumeric()
        }
        _ => true,
    })
}

/// `dest` joined to `folder` and normalised; `None` when it is absolute or leaves the root.
pub(crate) fn join_path(folder: &str, dest: &str) -> Option<String> {
    if dest.starts_with('/') {
        return None;
    }
    let mut stack: Vec<&str> = folder.split('/').filter(|p| !p.is_empty()).collect();
    for part in dest.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                stack.pop()?;
            }
            other => stack.push(other),
        }
    }
    Some(stack.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::identity::index::DocInput;

    const LEDGER: &str = "# Ledger\n\n## F-1 — first\n\n## F-2 — second\n";

    fn k(s: &str) -> Key {
        Key::parse(s).unwrap()
    }

    fn default_docs() -> Vec<(&'static str, &'static str)> {
        vec![
            ("docs/ledger.md", LEDGER),
            ("docs/b.md", "# B\n"),
            ("docs/src.md", "# Src\n"),
            ("docs/notes/a-doc.md", "# A\n\n## T-5 — five\n"),
            ("docs/my-notes.md", "# My\n"),
        ]
    }

    /// An index of a few documents, repository name `demo`, and empty side maps a test fills.
    struct Fx {
        index: KeyIndex,
        repo: RepoName,
        renamed: BTreeMap<String, Key>,
        paths: BTreeMap<String, String>,
        ambiguous: BTreeMap<String, Vec<String>>,
        stems: BTreeMap<String, Vec<String>>,
        ids: BTreeMap<String, IdTarget>,
        hints: BTreeMap<String, String>,
    }

    impl Fx {
        fn new(docs: &[(&str, &str)]) -> Fx {
            let repo = RepoName::parse("demo").unwrap();
            let index = KeyIndex::build(
                repo.clone(),
                docs.iter().map(|&(p, t)| DocInput {
                    rel_path: p.to_owned(),
                    text: t.to_owned(),
                }),
            );
            Fx {
                index,
                repo,
                renamed: BTreeMap::new(),
                paths: BTreeMap::new(),
                ambiguous: BTreeMap::new(),
                stems: BTreeMap::new(),
                ids: BTreeMap::new(),
                hints: BTreeMap::new(),
            }
        }
        fn standard() -> Fx {
            Fx::new(&default_docs())
        }
        /// The index holds the renamed file at its final path; the side maps record the rename of
        /// `docs/<old stem>.md`, as the plan builds them.
        fn after_rename(old_stem: &str) -> Fx {
            let mut docs = default_docs();
            docs.retain(|(p, _)| *p != "docs/my-notes.md");
            docs.push(("docs/my-notes.md", "# My\n\n## F-2 — two\n"));
            let mut fx = Fx::new(&docs);
            let old_key = old_stem.to_ascii_lowercase();
            fx.renamed.insert(old_key, k("my-notes"));
            fx.paths
                .insert(format!("docs/{old_stem}.md"), "docs/my-notes.md".to_owned());
            fx.stems
                .insert(old_stem.to_owned(), vec!["docs/my-notes.md".to_owned()]);
            fx
        }
        fn run(&self, src: &str, text: &str) -> (String, Vec<Finding>) {
            rewrite_document(
                src,
                text,
                &Resolver {
                    index: &self.index,
                    this_repo: &self.repo,
                    renamed_keys: &self.renamed,
                    renamed_paths: &self.paths,
                    ambiguous_keys: &self.ambiguous,
                    stems: &self.stems,
                    ids: &self.ids,
                    id_hints: &self.hints,
                },
            )
        }
    }

    fn unresolved(f: &Finding) -> (&str, Option<&str>) {
        match &f.outcome {
            Outcome::Unresolved { reason, suggestion } => (reason, suggestion.as_deref()),
            other => panic!("not unresolved: {other:?}"),
        }
    }

    fn rewritten(f: &Finding) -> &str {
        match &f.outcome {
            Outcome::Rewritten { to } => to,
            other => panic!("not rewritten: {other:?}"),
        }
    }

    #[test]
    fn a_stem_token_becomes_an_entry_form() {
        let mut fx = Fx::standard();
        fx.stems
            .insert("a-doc".into(), vec!["docs/notes/a-doc.md".into()]);
        let (out, f) = fx.run("docs/src.md", "# Src\n\nsee a-doc:T-5 now\n");
        assert_eq!(out, "# Src\n\nsee [[a-doc:T-5]] now\n");
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].form, Form::StemToken);
        assert_eq!(f[0].line, 3);
        assert_eq!(f[0].raw, "a-doc:T-5");
        assert_eq!(rewritten(&f[0]), "[[a-doc:T-5]]");
    }

    #[test]
    fn a_stem_two_files_share_is_ambiguous_and_unchanged() {
        let mut fx = Fx::new(&[
            ("docs/a/readme.md", "# A\n\n## R-7 — x\n"),
            ("docs/b/readme.md", "# B\n\n## R-7 — y\n"),
            ("docs/src.md", "# Src\n"),
        ]);
        fx.stems.insert(
            "readme".into(),
            vec!["docs/a/readme.md".into(), "docs/b/readme.md".into()],
        );
        let text = "# Src\n\nsee readme:R-7 now\n";
        let (out, f) = fx.run("docs/src.md", text);
        assert_eq!(out, text);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(
            f[0].outcome,
            Outcome::Ambiguous {
                candidates: vec!["a".into(), "b".into()]
            }
        );
    }

    #[test]
    fn this_repos_name_as_qualifier_reads_as_a_bare_token() {
        let fx = Fx::standard();
        let (out, f) = fx.run("docs/b.md", "# B\n\nsee demo:F-1 now\n");
        assert_eq!(out, "# B\n\nsee [[ledger:F-1]] now\n");
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].form, Form::RepoToken);
        assert_eq!(rewritten(&f[0]), "[[ledger:F-1]]");
    }

    #[test]
    fn another_qualifier_is_reported_cross_repo() {
        let fx = Fx::standard();
        let text = "# B\n\nsee other:F-1 here\n";
        let (out, f) = fx.run("docs/b.md", text);
        assert_eq!(out, text);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].form, Form::RepoToken);
        assert_eq!(unresolved(&f[0]).0, "cross repo");
    }

    #[test]
    fn a_path_link_becomes_a_wiki_link() {
        let fx = Fx::standard();
        // The text equals the key.
        let (out, f) = fx.run("docs/src.md", "# Src\n\nsee [b](b.md) now\n");
        assert_eq!(out, "# Src\n\nsee [[b]] now\n");
        assert_eq!(f[0].form, Form::PathLink);
        assert_eq!(rewritten(&f[0]), "[[b]]");
        // The text equals the file name, and the destination.
        let (out, _) = fx.run(
            "docs/src.md",
            "# Src\n\n[b.md](b.md) and [./b.md](./b.md)\n",
        );
        assert_eq!(out, "# Src\n\n[[b]] and [[b]]\n");
        // Other text becomes the label.
        let (out, f) = fx.run("docs/src.md", "# Src\n\nsee [the b doc](b.md) now\n");
        assert_eq!(out, "# Src\n\nsee [[b|the b doc]] now\n");
        assert_eq!(rewritten(&f[0]), "[[b|the b doc]]");
        // The destination is joined to the folder of the source.
        let (out, _) = fx.run("docs/notes/x.md", "# X\n\n[up](../b.md)\n");
        assert_eq!(out, "# X\n\n[[b|up]]\n");
        // In a table row the label separator is escaped.
        let (out, _) = fx.run(
            "docs/src.md",
            "| a | b |\n|---|---|\n| [the b doc](b.md) | x |\n",
        );
        assert_eq!(out, "| a | b |\n|---|---|\n| [[b\\|the b doc]] | x |\n");
    }

    #[test]
    fn a_path_link_with_a_fragment_is_reported_with_a_suggestion() {
        let fx = Fx::standard();
        let text = "# Src\n\nsee [b](b.md#part) now\n";
        let (out, f) = fx.run("docs/src.md", text);
        assert_eq!(out, text);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(unresolved(&f[0]), ("link with a fragment", Some("[[b]]")));
    }

    #[test]
    fn a_reference_style_link_is_reported_as_one_and_not_as_an_escape() {
        let fx = Fx::standard();
        let text = "# Src\n\nsee [t][x] now\n\n[x]: b.md\n";
        let (out, f) = fx.run("docs/src.md", text);
        assert_eq!(out, text);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].form, Form::PathLink);
        assert_eq!(unresolved(&f[0]), ("reference-style link", Some("[[b]]")));
    }

    #[test]
    fn a_path_link_whose_text_is_not_plain_is_reported() {
        let fx = Fx::standard();
        for link in [
            "[**b** doc](b.md)",
            "[`b`](b.md)",
            "[a | b](b.md)",
            "[a\nb](b.md)",
            "[x](b.md \"title\")",
            "[x](<b.md>)",
            "[](b.md)",
        ] {
            let text = format!("# Src\n\nsee {link} now\n");
            let (out, f) = fx.run("docs/src.md", &text);
            assert_eq!(out, text, "{link}");
            assert!(
                f.iter()
                    .all(|x| matches!(x.outcome, Outcome::Unresolved { .. })),
                "{link}: {f:?}"
            );
        }
    }

    #[test]
    fn a_target_outside_the_key_space_is_reported() {
        let fx = Fx::standard();
        for link in ["[x](outside.md)", "[x](../../../up.md)", "[x](/abs.md)"] {
            let text = format!("# Src\n\nsee {link} now\n");
            let (out, f) = fx.run("docs/src.md", &text);
            assert_eq!(out, text, "{link}");
            assert_eq!(f.len(), 1, "{link}: {f:?}");
            assert!(matches!(f[0].outcome, Outcome::Unresolved { .. }), "{link}");
        }
        let (_, f) = fx.run("docs/src.md", "# Src\n\n[x](outside.md)\n");
        assert_eq!(
            unresolved(&f[0]).0,
            "the target is not a document with a unique key"
        );
    }

    #[test]
    fn a_catalog_hex_id_becomes_a_file_form() {
        let mut fx = Fx::standard();
        fx.ids.insert(
            "0123456789abcdef".into(),
            IdTarget {
                rel_path: "docs/b.md".into(),
                source: IdSource::Catalog,
            },
        );
        let (out, f) = fx.run("docs/src.md", "# Src\n\nsee 0123456789abcdef.\n");
        assert_eq!(out, "# Src\n\nsee [[b]].\n");
        assert_eq!(f[0].form, Form::HexId);
        assert_eq!(rewritten(&f[0]), "[[b]]");
    }

    #[test]
    fn a_history_hint_is_reported_not_rewritten() {
        let mut fx = Fx::standard();
        fx.hints
            .insert("fedcba9876543210".into(), "docs/gone/b.md".into());
        let text = "# Src\n\nsee fedcba9876543210 ok\n";
        let (out, f) = fx.run("docs/src.md", text);
        assert_eq!(out, text);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].form, Form::HexId);
        assert_eq!(unresolved(&f[0]).1, Some("[[b]]"));
        // An id nothing knows is reported too.
        let text = "# Src\n\nsee 00112233aabbccdd ok\n";
        let (out, f) = fx.run("docs/src.md", text);
        assert_eq!(out, text);
        assert_eq!(unresolved(&f[0]), (REASON_UNKNOWN_ID, None));
    }

    #[test]
    fn a_bare_token_with_one_definer_is_qualified() {
        let fx = Fx::standard();
        let (out, f) = fx.run("docs/b.md", "# B\n\nsee F-1.\n");
        assert_eq!(out, "# B\n\nsee [[ledger:F-1]].\n");
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].form, Form::BareToken);
        assert_eq!(f[0].raw, "F-1");
        assert_eq!(f[0].line, 3);
        // Bold, a table cell and a list item are plain text too.
        let (out, _) = fx.run("docs/b.md", "- **F-1**\n- (F-2)\n\n| a |\n|---|\n| F-1 |\n");
        assert_eq!(
            out,
            "- **[[ledger:F-1]]**\n- ([[ledger:F-2]])\n\n| a |\n|---|\n| [[ledger:F-1]] |\n"
        );
    }

    #[test]
    fn a_bare_token_with_two_definers_is_ambiguous_and_unchanged() {
        let mut docs = default_docs();
        docs.push(("docs/ledger2.md", "# L2\n\n## F-1 — dup\n"));
        let fx = Fx::new(&docs);
        let text = "# B\n\nsee F-1 here\n";
        let (out, f) = fx.run("docs/b.md", text);
        assert_eq!(out, text);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(
            f[0].outcome,
            Outcome::Ambiguous {
                candidates: vec!["ledger".into(), "ledger2".into()]
            }
        );
    }

    #[test]
    fn a_unique_definer_beside_a_colliding_key_definer_is_reported_not_guessed() {
        let fx = Fx::new(&[
            ("docs/a-log.md", "# A\n\n## F-1 — a\n"),
            ("one/dup.md", "## F-1 — one\n"),
            ("two/dup.md", "## F-1 — two\n"),
            ("docs/b.md", "# B\n"),
        ]);
        let text = "# B\n\nsee F-1.\n";
        let (out, f) = fx.run("docs/b.md", text);
        assert_eq!(out, text, "three documents define it: no guess");
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(
            f[0].outcome,
            Outcome::Ambiguous {
                candidates: vec!["a-log".into(), "one/dup.md".into(), "two/dup.md".into()]
            }
        );
    }

    #[test]
    fn a_unique_definer_beside_an_invalid_key_definer_is_reported_not_guessed() {
        let fx = Fx::new(&[
            ("docs/a-log.md", "# A\n\n## F-1 — a\n"),
            ("docs/My_Notes.md", "# M\n\n## F-1 — m\n"),
            ("docs/b.md", "# B\n"),
        ]);
        let text = "# B\n\nsee F-1.\n";
        let (out, f) = fx.run("docs/b.md", text);
        assert_eq!(out, text);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(
            f[0].outcome,
            Outcome::Ambiguous {
                candidates: vec!["docs/My_Notes.md".into(), "a-log".into()]
            }
        );
    }

    #[test]
    fn a_token_whose_one_definer_has_no_unique_key_is_reported() {
        let fx = Fx::new(&[
            ("docs/My_Notes.md", "# M\n\n## F-2 — m\n"),
            ("docs/b.md", "# B\n"),
        ]);
        let text = "# B\n\nsee F-2.\n";
        let (out, f) = fx.run("docs/b.md", text);
        assert_eq!(out, text);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(
            unresolved(&f[0]).0,
            "the one document that defines the token has no unique key"
        );
    }

    #[test]
    fn a_table_cell_is_found_by_the_parser_not_by_a_leading_pipe() {
        let fx = Fx::standard();
        // A row with no leading pipe is still a row.
        let (out, _) = fx.run("docs/src.md", "a | b\n---|---\nc | [the b doc](b.md)\n");
        assert_eq!(out, "a | b\n---|---\nc | [[b\\|the b doc]]\n");
        // A table inside a block quote.
        let (out, _) = fx.run(
            "docs/src.md",
            "> | a | b |\n> |---|---|\n> | [the b doc](b.md) | x |\n",
        );
        assert_eq!(
            out,
            "> | a | b |\n> |---|---|\n> | [[b\\|the b doc]] | x |\n"
        );
        // A paragraph that starts with a pipe is no table: the separator stays unescaped.
        let (out, _) = fx.run("docs/src.md", "| not a table [the b doc](b.md)\n");
        assert_eq!(out, "| not a table [[b|the b doc]]\n");
    }

    #[test]
    fn a_bare_token_the_source_defines_is_left_alone() {
        let own = "# Src\n\n## F-1 — mine\n\nsee F-1 again\n";
        let mut docs = default_docs();
        docs.retain(|(p, _)| *p != "docs/src.md");
        docs.push(("docs/src.md", own));
        let fx = Fx::new(&docs);
        let (out, f) = fx.run("docs/src.md", own);
        assert_eq!(out, own);
        assert!(f.is_empty(), "{f:?}");
        // A source with no valid key is no index document, and still defines its own tokens.
        let (out, f) = fx.run("docs/My_Notes.md", own);
        assert_eq!(out, own);
        assert!(f.is_empty(), "{f:?}");
    }

    #[test]
    fn an_uppercase_qualifier_is_reported_and_never_half_rewritten() {
        let fx = Fx::standard();
        let text = "# B\n\nsee My_Notes:F-2 here\n";
        let (out, f) = fx.run("docs/b.md", text);
        assert_eq!(out, text, "byte for byte");
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].form, Form::BareToken);
        assert_eq!(
            unresolved(&f[0]).0,
            "qualified by My_Notes, which is no file stem"
        );
    }

    #[test]
    fn nothing_in_code_frontmatter_comments_labels_or_urls_is_rewritten() {
        let mut fx = Fx::standard();
        fx.stems
            .insert("a-doc".into(), vec!["docs/notes/a-doc.md".into()]);
        fx.ids.insert(
            "0123456789abcdef".into(),
            IdTarget {
                rel_path: "docs/b.md".into(),
                source: IdSource::Catalog,
            },
        );
        let text = "---\ntitle: x\nnote: |\n  see F-1 and a-doc:T-5\n\n---\n\n\
                    # Heading F-1\n\n\
                    ```\nfenced F-1 a-doc:T-5 0123456789abcdef\n```\n\n\
                    inline `F-1` code\n\n\
                    <!-- F-1 a-doc:T-5 -->\n\n\
                    [[ledger|F-1]]\n\n\
                    https://host/runs/F-1 and https://host/0123456789abcdef\n";
        let (out, f) = fx.run("docs/src.md", text);
        assert_eq!(out, text, "output equals input");
        let in_code = f.iter().filter(|x| x.outcome == Outcome::InCode).count();
        let in_url = f
            .iter()
            .filter(|x| matches!(&x.outcome, Outcome::Unresolved { reason, .. } if reason == "inside a URL"))
            .count();
        assert_eq!((in_code, in_url, f.len()), (1, 2, 3), "{f:?}");
    }

    #[test]
    fn a_token_split_by_an_escape_is_not_rewritten() {
        let mut fx = Fx::standard();
        fx.stems
            .insert("a-doc".into(), vec!["docs/notes/a-doc.md".into()]);
        let text = "# B\n\nsee F-1\\_x here\n";
        let (out, f) = fx.run("docs/b.md", text);
        assert_eq!(out, text);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(unresolved(&f[0]).0, "escape");
        // A qualified token has no range at an escape edge.
        let text = "# B\n\nsee a-doc:T-5\\_x here\n";
        let (out, f) = fx.run("docs/b.md", text);
        assert_eq!(out, text);
        assert!(
            f.iter()
                .all(|x| matches!(x.outcome, Outcome::Unresolved { .. })),
            "{f:?}"
        );
        assert!(
            f.iter()
                .any(|x| unresolved(x).0 == "escape or entity in the text run"),
            "{f:?}"
        );
    }

    #[test]
    fn an_entity_or_emphasis_neighbour_is_not_rewritten() {
        let fx = Fx::standard();
        for (text, reason) in [
            ("see &#65;F-1 now\n", "entity"),
            ("see F-1&#65; now\n", "entity"),
            ("see *a*F-1 now\n", "markup or a path next to the token"),
            ("see F-1*b* now\n", "markup or a path next to the token"),
            (
                "see docs/F-1.md now\n",
                "markup or a path next to the token",
            ),
            ("see !F-1 now\n", "markup or a path next to the token"),
            ("see F-1(x) now\n", "markup or a path next to the token"),
        ] {
            let text = format!("# B\n\n{text}");
            let (out, f) = fx.run("docs/b.md", &text);
            assert_eq!(out, text, "{text:?}");
            assert_eq!(f.len(), 1, "{text:?}: {f:?}");
            assert_eq!(unresolved(&f[0]).0, reason, "{text:?}");
        }
    }

    #[test]
    fn a_citation_in_a_heading_an_image_or_link_text_is_reported_not_rewritten() {
        let mut fx = Fx::standard();
        fx.stems
            .insert("a-doc".into(), vec!["docs/notes/a-doc.md".into()]);
        for (text, reason) in [
            ("## see a-doc:T-5\n", "inside a heading"),
            ("![alt a-doc:T-5](x.png)\n", "inside an image"),
            ("[a-doc:T-5 here](src/x.rs)\n", "inside link text"),
            ("[see F-1 here](src/x.rs)\n", "inside link text"),
        ] {
            let text = format!("# B\n\n{text}");
            let (out, f) = fx.run("docs/b.md", &text);
            assert_eq!(out, text, "{text:?}");
            assert_eq!(f.len(), 1, "{text:?}: {f:?}");
            assert_eq!(unresolved(&f[0]).0, reason, "{text:?}");
        }
    }

    #[test]
    fn a_token_in_a_rewritten_path_links_text_is_not_rewritten_twice() {
        let fx = Fx::standard();
        let (out, f) = fx.run("docs/src.md", "# Src\n\nsee [see F-1](b.md) now\n");
        assert_eq!(out, "# Src\n\nsee [[b|see F-1]] now\n");
        assert_eq!(
            f.len(),
            1,
            "one edit for the link, none for the token: {f:?}"
        );
        assert_eq!(f[0].form, Form::PathLink);
    }

    #[test]
    fn a_renamed_key_in_a_wiki_link_follows_the_rename() {
        let mut fx = Fx::standard();
        fx.renamed.insert("my_notes".into(), k("my-notes"));
        let (out, f) = fx.run(
            "docs/b.md",
            "# B\n\nsee [[My_Notes]] and [[My_Notes|label]]\n",
        );
        assert_eq!(out, "# B\n\nsee [[my-notes]] and [[my-notes|label]]\n");
        assert_eq!(f.len(), 2, "{f:?}");
        assert!(f.iter().all(|f| f.form == Form::RenamedKey), "{f:?}");
        assert_eq!(rewritten(&f[0]), "[[my-notes]]");
        assert_eq!(rewritten(&f[1]), "[[my-notes|label]]");
        // With no rename on record a malformed link is left alone and not reported.
        let fx = Fx::standard();
        let text = "# B\n\nsee [[My_Notes]]\n";
        let (out, f) = fx.run("docs/b.md", text);
        assert_eq!(out, text);
        assert!(f.is_empty(), "{f:?}");
    }

    #[test]
    fn a_label_on_a_link_to_a_renamed_file_is_kept_byte_for_byte() {
        let fx = Fx::after_rename("My_Notes");
        let text = "# B\n\n[[My_Notes|label]] [[my_notes:F-2|a b]] [[ My_Notes | spaced ]] [[demo/My_Notes|x]]\n\n\
                    | a | b |\n|---|---|\n| [[My_Notes\\|cell label]] | plain |\n";
        let (out, f) = fx.run("docs/b.md", text);
        assert_eq!(
            out,
            "# B\n\n[[my-notes|label]] [[my-notes:F-2|a b]] [[my-notes| spaced ]] [[demo/my-notes|x]]\n\n\
             | a | b |\n|---|---|\n| [[my-notes\\|cell label]] | plain |\n"
        );
        assert_eq!(f.len(), 5, "{f:?}");
        assert!(
            f.iter()
                .all(|f| matches!(f.outcome, Outcome::Rewritten { .. })),
            "{f:?}"
        );
    }

    #[test]
    fn a_label_form_that_does_not_resolve_is_reported_not_written() {
        let fx = Fx::after_rename("My_Notes");
        let text = "# B\n\n[[my_notes:F-9|x]]\n";
        let (out, f) = fx.run("docs/b.md", text);
        assert_eq!(out, text);
        assert_eq!(unresolved(&f[0]).0, REASON_NOT_RENDERABLE);
    }

    #[test]
    fn a_key_two_files_could_be_is_reported_ambiguous_and_left_alone() {
        let mut fx = Fx::after_rename("My_Notes");
        fx.ambiguous.insert(
            "my_notes".into(),
            vec![".github/My_Notes.md".into(), "docs/My_Notes.md".into()],
        );
        let text = "# B\n\n[[My_Notes]] [[my_notes:F-2]] [[My_Notes|x]] [[docs/My_Notes]] [[My_Notes#S]]\n";
        let (out, f) = fx.run("docs/b.md", text);
        assert_eq!(out, text);
        assert_eq!(f.len(), 5, "{f:?}");
        for finding in &f {
            assert_eq!(finding.form, Form::RenamedKey);
            match &finding.outcome {
                Outcome::Ambiguous { candidates } => {
                    assert_eq!(candidates, &[".github/My_Notes.md", "docs/My_Notes.md"])
                }
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn a_foreign_qualifier_or_a_fragment_on_a_renamed_key_is_reported_with_the_new_form() {
        let fx = Fx::after_rename("My_Notes");
        let text = "# B\n\n[[docs/My_Notes]] [[My_Notes#Sec]] [[demo/My_Notes#Sec]] [[docs/my_notes:F-2]]\n";
        let (out, f) = fx.run("docs/b.md", text);
        assert_eq!(out, text, "neither form is safe to guess");
        assert_eq!(f.len(), 4, "{f:?}");
        assert_eq!(unresolved(&f[0]).1, Some("[[my-notes]]"));
        assert_eq!(unresolved(&f[1]).1, Some("[[my-notes]]"));
        assert_eq!(unresolved(&f[2]).1, Some("[[demo/my-notes]]"));
        assert_eq!(unresolved(&f[3]).1, Some("[[my-notes:F-2]]"));
        // A qualifier on a stem that is not a renamed key is nobody's business here.
        let text = "# B\n\n[[docs/Other_Thing]] [[Other_Thing#Sec]] [[other/My_Notes]]\n";
        let (out, f) = fx.run("docs/b.md", text);
        assert_eq!(out, text);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(
            unresolved(&f[0]).1,
            Some("[[my-notes]]"),
            "another repository's My_Notes names a renamed key too"
        );
    }

    #[test]
    fn a_wiki_link_to_an_old_key_follows_the_rename() {
        let fx = Fx::after_rename("My_Notes");
        let text = "# B\n\n[[My_Notes]] [[my_notes:F-2]] [[demo/My_Notes]] [[demo/My_Notes:F-2]]\n";
        let (out, f) = fx.run("docs/b.md", text);
        assert_eq!(
            out,
            "# B\n\n[[my-notes]] [[my-notes:F-2]] [[demo/my-notes]] [[demo/my-notes:F-2]]\n"
        );
        assert_eq!(f.len(), 4, "{f:?}");
        assert!(f.iter().all(|f| f.form == Form::RenamedKey), "{f:?}");
        // The qualifier is this repository's to keep; another one on a stem nobody renamed is not ours.
        let text = "# B\n\n[[other/Plain]] [[a:b:c]]\n";
        let (out, f) = fx.run("docs/b.md", text);
        assert_eq!(out, text);
        assert!(f.is_empty(), "{f:?}");
    }

    #[test]
    fn a_rename_that_gives_no_renderable_form_is_reported_not_written() {
        // The renamed file does not define the token: the rendered form would not resolve.
        let fx = Fx::after_rename("My_Notes");
        let text = "# B\n\n[[my_notes:F-9]]\n";
        let (out, f) = fx.run("docs/b.md", text);
        assert_eq!(out, text);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(unresolved(&f[0]).0, REASON_NOT_RENDERABLE);
    }

    #[test]
    fn a_lowercase_stem_token_and_a_path_link_follow_the_rename() {
        let fx = Fx::after_rename("my_notes");
        let text = "# B\n\nsee my_notes:F-2 and [n](my_notes.md) here\n";
        let (out, f) = fx.run("docs/b.md", text);
        assert_eq!(out, "# B\n\nsee [[my-notes:F-2]] and [[my-notes|n]] here\n");
        assert_eq!(f.len(), 2, "{f:?}");
        assert_eq!(f[0].form, Form::StemToken);
        assert_eq!(f[1].form, Form::PathLink);
        // A path link keeps its spelling of the old name, whatever the case.
        let fx = Fx::after_rename("My_Notes");
        let (out, f) = fx.run("docs/b.md", "# B\n\n[My_Notes.md](My_Notes.md)\n");
        assert_eq!(out, "# B\n\n[[my-notes]]\n");
        assert_eq!(f.len(), 1, "{f:?}");
        // Another folder reaches the old path through `..`.
        let (out, _) = fx.run("docs/notes/a-doc.md", "# A\n\n[n](../My_Notes.md)\n");
        assert_eq!(out, "# A\n\n[[my-notes|n]]\n");
        // A label that is the old file name reads as the file itself.
        let (out, _) = fx.run(
            "docs/notes/a-doc.md",
            "# A\n\n[My_Notes.md](../My_Notes.md)\n",
        );
        assert_eq!(out, "# A\n\n[[my-notes]]\n");
    }

    #[test]
    fn a_reference_style_link_to_a_renamed_file_carries_the_new_form_as_its_suggestion() {
        let fx = Fx::after_rename("My_Notes");
        let (out, f) = fx.run("docs/b.md", "# B\n\n[n][ref]\n\n[ref]: My_Notes.md\n");
        assert_eq!(out, "# B\n\n[n][ref]\n\n[ref]: My_Notes.md\n");
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(
            unresolved(&f[0]),
            ("reference-style link", Some("[[my-notes]]"))
        );
    }

    #[test]
    fn an_id_hint_for_a_file_the_cutover_renamed_suggests_the_new_form() {
        let mut fx = Fx::after_rename("My_Notes");
        fx.hints
            .insert("0123456789abcdef".into(), "docs/My_Notes.md".into());
        let (out, f) = fx.run("docs/b.md", "# B\n\nsee 0123456789abcdef\n");
        assert_eq!(out, "# B\n\nsee 0123456789abcdef\n");
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(unresolved(&f[0]).1, Some("[[my-notes]]"));
    }

    #[test]
    fn a_second_pass_changes_nothing() {
        let mut fx = Fx::standard();
        fx.stems
            .insert("a-doc".into(), vec!["docs/notes/a-doc.md".into()]);
        fx.ids.insert(
            "0123456789abcdef".into(),
            IdTarget {
                rel_path: "docs/b.md".into(),
                source: IdSource::History,
            },
        );
        let text = "# Src\n\nsee F-1, demo:F-2, a-doc:T-5, [the b](b.md), 0123456789abcdef.\n\n\
                    | a |\n|---|\n| [x](b.md) F-1 |\n";
        let (once, f) = fx.run("docs/src.md", text);
        assert_ne!(once, text);
        assert_eq!(f.len(), 7, "{f:?}");
        let (twice, again) = fx.run("docs/src.md", &once);
        assert_eq!(twice, once);
        assert!(again.is_empty(), "{again:?}");
    }

    /// Every markdown file under `dir` (hidden directories skipped), as (path from `root`, text).
    fn corpus(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<(String, String)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            if kind.is_dir() {
                corpus(root, &path, out);
            } else if kind.is_file() && name.ends_with(".md") {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    let rel = path
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    out.push((rel, text));
                }
            }
        }
    }

    #[test]
    fn the_live_corpus_rewrites_idempotently_and_keeps_every_wiki_link() {
        use crate::librarian::identity::grammar::extract_spanned;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut docs = Vec::new();
        corpus(root, &root.join("docs"), &mut docs);
        assert!(docs.len() > 50, "the corpus is there: {}", docs.len());
        let fx = {
            let mut fx = Fx::new(
                &docs
                    .iter()
                    .map(|(p, t)| (p.as_str(), t.as_str()))
                    .collect::<Vec<_>>(),
            );
            for (path, _) in &docs {
                let name = path.rsplit('/').next().unwrap();
                if let Some(stem) = name.strip_suffix(".md") {
                    fx.stems
                        .entry(stem.to_owned())
                        .or_default()
                        .push(path.clone());
                }
            }
            fx
        };
        // Every tenth document keeps the test short; a full run over all of them was made by hand.
        let (mut rewrites, mut changed) = (0usize, 0usize);
        for (path, text) in docs.iter().step_by(10) {
            let (once, findings) = fx.run(path, text);
            rewrites += findings
                .iter()
                .filter(|f| matches!(f.outcome, Outcome::Rewritten { .. }))
                .count();
            if once != *text {
                changed += 1;
            }
            let (twice, again) = fx.run(path, &once);
            assert_eq!(twice, once, "{path}: a second pass changed the text");
            assert!(
                again
                    .iter()
                    .all(|f| !matches!(f.outcome, Outcome::Rewritten { .. })),
                "{path}: a second pass found more to rewrite: {again:?}"
            );
            // Nothing that was a citation before is lost.
            let kept = |t: &str| -> Vec<String> {
                extract_spanned(t)
                    .into_iter()
                    .filter(|s| {
                        !matches!(
                            s.citation.reference,
                            crate::librarian::identity::grammar::Reference::Bare { .. }
                        )
                    })
                    .map(|s| format!("{:?}", s.citation.reference))
                    .collect()
            };
            let after = kept(&once);
            for before in kept(text) {
                assert!(after.contains(&before), "{path}: lost {before}");
            }
        }
        // Counts are information only: once the cutover is applied here no old form remains.
        eprintln!("live corpus sample: {changed} changed, {rewrites} rewrites");
    }

    #[test]
    fn edits_that_overlap_are_never_applied() {
        let text = "0123456789";
        let edit = |s: usize, e: usize, to: &str| Edit {
            range: s..e,
            to: to.to_owned(),
        };
        assert_eq!(
            apply_edits(text, vec![edit(2, 5, "X"), edit(4, 6, "Y")]),
            Err(())
        );
        assert_eq!(
            apply_edits(text, vec![edit(2, 5, "X"), edit(2, 5, "Y")]),
            Err(())
        );
        assert_eq!(
            apply_edits(text, vec![edit(2, 20, "X")]),
            Err(()),
            "past the end"
        );
        // Touching ranges, given in any order, apply right to left on the original bytes.
        assert_eq!(
            apply_edits(
                text,
                vec![edit(5, 7, "B"), edit(2, 5, "A"), edit(9, 10, "")]
            ),
            Ok("01AB78".to_owned())
        );
        let multibyte = "é-F-1";
        assert_eq!(
            apply_edits(multibyte, vec![edit(1, 2, "x")]),
            Err(()),
            "inside a char"
        );
    }
}
