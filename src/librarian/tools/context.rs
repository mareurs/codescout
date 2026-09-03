use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

use crate::librarian::catalog::{artifact, augmentation, links};
use crate::librarian::filter::FilterNode;

use super::scope::{apply_scope, resolve_scope, Scope, UmbrellaPolicy};
use super::ToolContext;

use super::HIDDEN_STATUSES;

#[derive(Deserialize)]

struct Args {
    #[serde(default)]
    topic: Option<String>,
    #[serde(default)]
    anchor_id: Option<String>,
    #[serde(default)]
    max_tokens: Option<usize>,
    #[serde(default)]
    scope: Option<Scope>,
    #[serde(default)]
    include_archived: bool,
}

const DEFAULT_MAX_TOKENS: usize = 4000;

/// Per-neighbour byte cap used ONLY when an entry-grain neighbourhood does not fit whole.
///
/// Chosen by measurement, not by roundness (2026-08-21, 931 anchors / 1598 sections):
/// 1000 bytes takes the fully-served share from 76% to **98%** at the default budget, and
/// an excerpt that size still carries the entry's heading, its status line and the opening
/// of its claim — which is what a neighbour is for. Neighbours answer *what rests on this
/// and how widely*, a shape question; the anchor is the thing you came to read.
///
/// BYTES rather than lines is also measured: these ledgers run 40-200+ bytes per line, so
/// the file-grain path's 30-line preview would leave 1074 of 1598 sections untouched.
/// Re-tune from `docs/trackers/context-performance.md` if the corpus shape moves.
const NEIGHBOUR_EXCERPT_BYTES: usize = 1000;

/// Distinct citing Statements a claim needs before *reading* it incurs a proof obligation.
///
/// **Measured on the live entry graph 2026-08-21, not guessed.** At 5 the tap arms **5**
/// Statements corpus-wide — `SI-7` (9 citers), `R-19` and `R-3` (7), `R-77` and `R-79` (5)
/// — which are the load-bearing lessons, and a population small enough that an obligation
/// still means something. At 4 it would arm 13; at 3, 41; at 2, 144. The design called this
/// constant "a guess with no measurement behind it"; this is the measurement.
///
/// **Those are the second figures, and the first ones are worth recording.** The original
/// query counted every distinct `src_slug` per destination and reported 8 armed at this
/// threshold. It did not exclude same-ledger citations — which the code below does — so it
/// measured a different predicate from the one it was cited to justify. `R-50`, `R-95` and
/// `R-49` reached 5 only by counting their own ledger's citations of them, and do not arm.
/// Caught by probing the shipped tap and seeing it report exposure **7** for `R-3` where
/// the query had said 8: the code was right and the measurement was wrong. Running a query
/// is not the same as running the right one.
///
/// A **floor, not a period**: a Statement appraised at 5 arms again at 10.
///
/// Deliberately NOT `doctor`'s `EXPOSURE_THRESHOLD`, despite sharing a value today. That
/// one prices a different population (every citing FILE, recomputed from disk) for a
/// different purpose (a gate). This reads the materialized graph, because `entry_indegree`
/// takes ~5s over 4113 files and an interactive read cannot pay that. The cost is that this
/// UNDERCOUNTS — only ledger entries produce `entry_cite` rows, so a spec or bug file
/// citing the Statement is invisible here — and undercounting is the conservative
/// direction for an obligation.
const ATTESTATION_EXPOSURE_THRESHOLD: usize = 5;

const ANCHOR_MARKER: &str = "\n\n… [anchor truncated — reserved half the budget for its \
                             neighbours; use `doc(get, id=…, heading=…)` for the full \
                             entry]";
const EXCERPT_MARKER: &str = "\n\n… [excerpted — the neighbourhood exceeds the budget; use \
                              `doc(get, id=…, heading=…)` for the full entry]";

fn scope_summary(
    scope: Scope,
    current: Option<&crate::librarian::current_project::CurrentProject>,
    fallback: bool,
) -> Value {
    json!({
        "applied": match scope {
            Scope::Project => "project",
            Scope::Repo => "repo",
            Scope::Umbrella => "umbrella",
            Scope::All => "all",
        },
        "root": current.map(|c| c.git_root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
        "subdir": current.map(|_| String::new()),
        "umbrella": current.and_then(|c| c.umbrella.clone()),
        "scope_fallback": fallback,
    })
}
/// One Statement packed into an entry-grain context bundle.
struct StatementNode {
    reference: String,
    display_path: String,
    validity: String,
    rests_on: Option<String>,
    text: String,
    /// `anchor`, `cites` (the anchor points at it) or `cited-by` (it points at the anchor).
    direction: &'static str,
}

/// A slug's file, parsed into entry sections once and reused across neighbours.
type SlugEntry = Option<(
    String,
    Vec<crate::librarian::tools::link_scan::extract::EntrySection>,
)>;

fn load_slug(cat: &crate::librarian::catalog::Catalog, slug: &str) -> Result<SlugEntry> {
    use rusqlite::OptionalExtension;
    let abs: Option<String> = cat
        .conn
        .query_row(
            "SELECT abs_path FROM artifact WHERE slug = ?1",
            rusqlite::params![slug],
            |r| r.get(0),
        )
        .optional()?;
    let Some(abs) = abs else {
        return Ok(None);
    };
    let Ok(text) = std::fs::read_to_string(&abs) else {
        // A catalogued row whose file is gone is `doctor`'s `missing_file`, not this
        // surface's problem — pack what resolves and let the count show the shortfall.
        return Ok(None);
    };
    Ok(Some((
        abs,
        crate::librarian::tools::link_scan::extract::entry_sections(&text),
    )))
}

/// Render one section into a packed node, or `None` when the slug or entry does not resolve.
fn statement_node(
    cat: &crate::librarian::catalog::Catalog,
    cache: &mut std::collections::HashMap<String, SlugEntry>,
    root: Option<&std::path::Path>,
    slug: &str,
    local: &str,
    direction: &'static str,
) -> Result<Option<StatementNode>> {
    use crate::librarian::statements::{
        declared_section_text, parse_rests_on, parse_validity, Validity,
    };

    if !cache.contains_key(slug) {
        let loaded = load_slug(cat, slug)?;
        cache.insert(slug.to_string(), loaded);
    }
    let Some(Some((abs, sections))) = cache.get(slug) else {
        return Ok(None);
    };
    let Some(section) = sections.iter().find(|s| s.id == local) else {
        return Ok(None);
    };

    // Never `section.text` — a nested child's declaration would be read as this
    // entry's. See `declared_section_text`.
    let declared = declared_section_text(section, sections);
    let validity = match parse_validity(&declared) {
        Ok(Some(Validity::Invariant)) => "invariant".to_string(),
        Ok(Some(Validity::Dated(d))) => format!("dated {d}"),
        Ok(Some(Validity::Conditional { condition })) => format!("conditional — {condition}"),
        // Absence is not an exemption: an undeclared entry already means decay. Say so
        // rather than printing nothing, which reads as "no decay concern".
        Ok(None) => "undeclared (defaults to decay)".to_string(),
        // A malformed declaration is `doctor`'s `validity_unparseable` finding. Surfacing
        // it here as if it were absent would hide a defect behind a plausible default.
        Err(_) => "unparseable — see doctor(validity_unparseable)".to_string(),
    };

    let display_path = root
        .and_then(|r| std::path::Path::new(abs).strip_prefix(r).ok())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| abs.clone());

    Ok(Some(StatementNode {
        reference: format!("{slug}:{local}"),
        display_path,
        validity,
        rests_on: parse_rests_on(&declared),
        text: section.text.clone(),
        direction,
    }))
}

/// The declared decay class of a Statement, or `None` when it has not declared one.
///
/// Reported in the `verification` fact block so a reader can tell an undeclared Statement
/// from a dated one. What each class requires to discharge is deliberately NOT here: those
/// four strings were imperative-voiced, and an imperative arriving inside relayed tool
/// output is quarantined as an injection. Measured 2026-08-21, prompt-engineering
/// `scenarios/attestation-register` rounds 3-5, n=10 per arm — the same information as a
/// directive surfaced 0-4/10, as data 10/10. The class-to-obligation table now lives in
/// `get_guide("tracker-conventions")`, which is pulled rather than pushed.
fn validity_class(validity: &str) -> Option<&'static str> {
    ["invariant", "dated", "conditional"]
        .into_iter()
        .find(|class| validity.starts_with(class))
}

/// How many *evidence-carrying* appraisals one Statement has, and the most recent verdict.
///
/// **Proof-carrying, and this check is the load-bearing half of the mechanism.** A
/// `reviewed` event counts only when its payload names this entry AND carries
/// `instrument`, `observed` and `verdict`. Without that, the tap is a laundering machine:
/// the claim acquires a verification stamp it did not earn and reads as *more* trustworthy
/// than before it was ever checked — strictly worse than no mechanism at all. Storing the
/// raw invocation and its raw output is what lets a later, independent reader re-check
/// cheaply; a verdict alone is an assurance, not evidence.
///
/// An event missing any of the three is skipped rather than rejected — it stays in the log
/// as a note, and simply does not move anything, nor does it count.
///
/// The verdict is returned rather than a bool because only `held` discharges. `refuted`
/// closes a validity interval and belongs on a worklist, not in silence; `inconclusive`
/// records that someone looked and could not tell, which is information and leaves the tap
/// armed. Neither resets the counter.
///
/// **The count is not short-circuited.** Returning on the first hit would be cheaper and
/// would still give the right verdict, but `appraisals_recorded: 0` and
/// `appraisals_recorded: 4` say different things to a reader about a Statement that is
/// still unverified — the second means people have looked and could not settle it. The
/// timeline is already capped at 200 rows, so walking it costs nothing extra.
fn last_appraisal(
    cat: &crate::librarian::catalog::Catalog,
    artifact_id: &str,
    reference: &str,
) -> Result<(usize, Option<String>)> {
    let mut count = 0usize;
    let mut latest: Option<String> = None;
    for ev in crate::librarian::catalog::events::timeline_for_artifact(
        cat,
        artifact_id,
        Some(&["reviewed"]),
        None,
        200,
    )? {
        let Ok(payload) = serde_json::from_str::<Value>(&ev.payload) else {
            continue;
        };
        if payload.get("entry").and_then(Value::as_str) != Some(reference) {
            continue;
        }
        let carries = |k: &str| {
            payload
                .get(k)
                .and_then(Value::as_str)
                .is_some_and(|s| !s.trim().is_empty())
        };
        if !(carries("instrument") && carries("observed") && carries("verdict")) {
            continue;
        }
        count += 1;
        // The timeline is newest-first, so the first qualifying row is the latest verdict
        // and every later one is older. Set once.
        if latest.is_none() {
            latest = payload
                .get("verdict")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
    }
    Ok((count, latest))
}

/// Entry-grain anchor: pack the Statement itself plus the Statements on either side of it.
///
/// Returns `Ok(None)` when `anchor_id` does not name an entry, so the caller falls through
/// to the unchanged file-grain path. **That fall-through is why this is a separate function
/// rather than a branch threaded through `call`:** the spec requires a file-grain anchor's
/// behaviour to be unchanged, and an early return makes that true by construction instead
/// of by regression test.
///
/// Entry-ness is decided by RESOLUTION, not by shape. No second copy of the
/// `[A-Z]{1,3}-\d+` grammar lives here, so this cannot drift from `link_scan::extract`'s
/// idea of what an entry id is — and the condition it tests ("a slug owns this, and a
/// section with this id exists") is exactly the condition under which packing can work.
#[allow(clippy::too_many_arguments)]
fn pack_entry_anchor(
    ctx: &ToolContext,
    anchor_id: &str,
    char_cap: usize,
    effective_scope: Scope,
    current: Option<&crate::librarian::current_project::CurrentProject>,
    scope_fallback: bool,
) -> Result<Option<Value>> {
    let Some((slug, local)) = anchor_id.rsplit_once(':') else {
        return Ok(None);
    };
    if slug.is_empty() || local.is_empty() {
        return Ok(None);
    }

    let cat = ctx.catalog.lock();
    let root = current.map(|c| c.git_root.as_path());
    let mut cache: std::collections::HashMap<String, SlugEntry> = Default::default();

    let Some(anchor) = statement_node(&cat, &mut cache, root, slug, local, "anchor")? else {
        return Ok(None);
    };

    // Outward needs the entry-grain accessor; `outgoing` filters on src_slug alone and
    // would hand back every entry in the ledger. Inward needs no twin — `dst_ref` IS
    // `<slug>:<local>`, so exact match is already entry grain.
    let mut nodes: Vec<StatementNode> = Vec::new();
    let mut unresolved = 0usize;
    for row in crate::librarian::catalog::entry_cite::outgoing_from_entry(&cat, slug, local)? {
        match row.dst_ref.rsplit_once(':') {
            Some((s, l)) => match statement_node(&cat, &mut cache, root, s, l, "cites")? {
                Some(n) => nodes.push(n),
                None => unresolved += 1,
            },
            // A bare artifact id names a FILE, not a Statement. Counted, never packed:
            // inventing a section for it would fabricate provenance the citation never
            // claimed.
            None => unresolved += 1,
        }
    }
    // Exposure for the tap, gathered from rows this function already needs — so the whole
    // mechanism costs no extra query. Distinct SOURCE LEDGERS, not edges: one ledger citing
    // the anchor from eight of its own entries is one voice, not eight.
    let mut citers: std::collections::BTreeSet<String> = Default::default();
    for row in crate::librarian::catalog::entry_cite::incoming(&cat, &anchor.reference)? {
        // Same-ledger citations are excluded, mirroring `entry_indegree`'s same-file rule:
        // an entry citing a sibling inside its own ledger is the ledger talking to itself,
        // not independent corroboration — and a hand-maintained `## Index` row is exactly
        // that shape.
        if row.src_slug != slug {
            citers.insert(row.src_slug.clone());
        }
        match statement_node(
            &cat,
            &mut cache,
            root,
            &row.src_slug,
            &row.src_local,
            "cited-by",
        )? {
            Some(n) => nodes.push(n),
            None => unresolved += 1,
        }
    }

    // The tap. RFC 5861 `stale-while-revalidate`, and the correspondence is exact: serve
    // the possibly-stale value immediately and in full, never block the reader, and
    // predicate revalidation on an incoming REQUEST rather than on a timer.
    let exposure = citers.len();
    let appraisal = if exposure >= ATTESTATION_EXPOSURE_THRESHOLD {
        use rusqlite::OptionalExtension;
        let aid: Option<String> = cat
            .conn
            .query_row(
                "SELECT id FROM artifact WHERE slug = ?1",
                rusqlite::params![slug],
                |r| r.get(0),
            )
            .optional()?;
        match aid {
            Some(aid) => last_appraisal(&cat, &aid, &anchor.reference)?,
            None => (0, None),
        }
    } else {
        (0, None)
    };
    let (appraisal_count, appraisal) = appraisal;
    // Armed unless an evidence-carrying appraisal came back `held`. `refuted` and
    // `inconclusive` deliberately leave it armed — see `last_appraisal`.
    let armed =
        exposure >= ATTESTATION_EXPOSURE_THRESHOLD && !matches!(appraisal.as_deref(), Some("held"));
    drop(cat);

    // Dedup by REFERENCE, never by (reference, direction). An entry that cites the anchor
    // AND is cited by it is ONE node — packing it under both labels duplicates its whole
    // section. Measured 2026-08-21: 182 bidirectional rows (91 mutual pairs) corpus-wide,
    // and for `reconnaissance-patterns:R-3` that was ~4.8KB of repeat inside a 16KB budget.
    // `mutual` is also the more informative label: a reciprocal citation is a stronger tie
    // than either direction alone.
    let mut merged: BTreeMap<String, StatementNode> = BTreeMap::new();
    for n in nodes {
        match merged.get_mut(&n.reference) {
            Some(existing) => {
                if existing.direction != n.direction {
                    existing.direction = "mutual";
                }
            }
            None => {
                merged.insert(n.reference.clone(), n);
            }
        }
    }
    let mut nodes: Vec<StatementNode> = merged.into_values().collect();
    // `mutual` first, then the one-way classes, each alphabetical.
    //
    // This is not a relevance heuristic pulled from air, and the two one-way classes are
    // DELIBERATELY not ranked against each other: `cites` and `cited-by` answer different
    // questions (proof route vs blast radius) and choosing between them is the caller's,
    // not the packer's. Reciprocity is different — it is the one tie both authors asserted
    // independently, and it is 12.4% of edges (`docs/trackers/context-performance.md`
    // CTX-2).
    //
    // Ordering only decides anything for anchors that overflow, which is why
    // `CTX-1` deferred it. Measured on the FIRST live call after shipping: plain
    // alphabetical `direction` sorts `cited-by` < `cites` < `mutual`, so
    // `reconnaissance-patterns:R-3` served eleven one-way citations and buried BOTH its
    // mutual partners — which are its own documented chain. The deferral's own
    // revisit-when fired immediately.
    let rank = |d: &str| match d {
        "mutual" => 0u8,
        "cited-by" => 1,
        _ => 2,
    };
    nodes.sort_by(|a, b| {
        rank(a.direction)
            .cmp(&rank(b.direction))
            .then(a.reference.cmp(&b.reference))
    });

    let render = |n: &StatementNode, budget: Option<usize>, marker: &str| -> String {
        let rests = n
            .rests_on
            .as_deref()
            .map(|r| format!("\n**Rests on:** {r}"))
            .unwrap_or_default();
        let mut body = n.text.clone();
        if let Some(cap) = budget {
            if body.len() > cap {
                let mut cut = cap;
                while !body.is_char_boundary(cut) {
                    cut -= 1;
                }
                body.truncate(cut);
                body.push_str(marker);
            }
        }
        format!(
            "## {} · {}\n*{} · valid: {}*{}\n\n{}\n\n",
            n.reference, n.direction, n.display_path, n.validity, rests, body
        )
    };

    // Same reserve rule as the file-grain path, and for the same reason: a long anchor
    // otherwise consumes the whole budget before a neighbour is considered.
    // docs/issues/archive/2026-07-05-context-anchor-starves-neighbors.md
    let anchor_reserve = (!nodes.is_empty()).then_some(char_cap / 2);
    let anchor_section = render(&anchor, anchor_reserve, ANCHOR_MARKER);

    // Two passes, because neither fixed policy dominates. Measured 2026-08-21 over 931
    // anchors (mean 3.0 neighbours, mean section 2.6KB): 76% of neighbourhoods fit WHOLE at
    // the default budget, so excerpting unconditionally would degrade three anchors in four
    // for nothing. The other 24% overflow badly — `R-3`'s neighbourhood is 80KB against a
    // 16KB budget — and a 1000-byte excerpt takes the fully-served share to 98%.
    //
    // The cap is in BYTES, not lines, and that is measured too: these ledgers run 40-200+
    // bytes per line, so a 30-line cap — what the file-grain path uses, correctly, for prose
    // bodies — leaves 1074 of 1598 sections completely untouched. Copying the sibling's
    // NUMBER without checking that its UNIT transfers was the trap here.
    let whole: Vec<String> = nodes
        .iter()
        .map(|n| render(n, None, EXCERPT_MARKER))
        .collect();
    let whole_total = anchor_section.len() + whole.iter().map(String::len).sum::<usize>();
    let excerpted = whole_total > char_cap;
    let sections: Vec<String> = if excerpted {
        nodes
            .iter()
            .map(|n| render(n, Some(NEIGHBOUR_EXCERPT_BYTES), EXCERPT_MARKER))
            .collect()
    } else {
        whole
    };

    // NO BANNER. The markdown carries the Statement and nothing addressed to the reader.
    //
    // It used to open with `> ⚠ ... Proof owed before this is relied on: <obligation>`,
    // on the reasoning that a structured field is easy to skip. The reasoning was sound
    // and the conclusion was backwards: measured 2026-08-21 (prompt-engineering
    // `scenarios/attestation-register`, n=10 per arm), a directive delivered inside
    // relayed tool output is not skipped, it is QUARANTINED — read, identified as an
    // embedded instruction, and explicitly declined. Runs said so in as many words: "it
    // didn't come from you, so I'm ignoring it", and then wrote the blurb with no caveat
    // at all. The same information stated as data in `verification` below surfaced 10/10
    // against 0-4/10 for every directive-shaped arm.
    //
    // So the budget the banner cost is returned to the Statements, and the register that
    // makes a reader act now arrives through `server_instructions` — a channel the same
    // measurement shows is read, obeyed and cited by name rather than quarantined.
    let mut markdown = String::new();
    markdown.push_str(&anchor_section);
    let mut included_ids: Vec<String> = vec![anchor.reference.clone()];

    for (n, section) in nodes.iter().zip(sections.iter()) {
        if (markdown.len() + section.len()) > char_cap {
            break;
        }
        markdown.push_str(section);
        included_ids.push(n.reference.clone());
        if markdown.len() >= char_cap {
            break;
        }
    }

    let candidates = 1 + nodes.len();
    let included = included_ids.len();
    let omitted = candidates.saturating_sub(included);
    let mut overflow = json!({
        "candidates": candidates,
        "included": included,
        "omitted": omitted,
        "candidates_capped": false,
        "grain": "entry",
        // Which of the two packing modes ran. Reported rather than inferable: a reader
        // cannot tell a whole section from a 1000-byte excerpt that happened to end at a
        // paragraph, and "is this the entry or the top of it" changes what they do next.
        "packing": if excerpted { "excerpted" } else { "whole" },
        // Edges whose endpoint is a file rather than a Statement, or whose entry no
        // longer exists. Reported rather than folded into `omitted`, which means
        // "did not fit": these would not have been packed at any budget.
        "unresolved_edges": unresolved,
    });
    if omitted > 0 {
        overflow["hint"] = json!(format!(
            "{omitted} neighbouring Statement(s) omitted (token budget) — raise `max_tokens`."
        ));
    }

    let mut out = json!({
        "markdown": markdown,
        "included_ids": included_ids,
        "overflow": overflow,
        "scope": scope_summary(effective_scope, current, scope_fallback),
    });
    if armed {
        // **A fact block. No verbs, no addressee, nothing to perform.**
        //
        // This replaces a `must_follow` sentence and a `pending_attestations` array whose
        // `discharge` field spelled out an `artifact_event` call. Both were directives, and
        // the register was chosen on a real finding applied to the wrong channel: `Guidance`
        // in `src/tools/core/types.rs` says agents "react to the key, not the prose", which
        // is true — A-7 measured provenance envelope keys shipping at 6/6. It is a finding
        // about how a TRUSTED instruction is noticed, not about whether relayed tool output
        // can carry an instruction at all.
        //
        // Measured 2026-08-21, prompt-engineering `scenarios/attestation-register`, n=10
        // per arm, 10 distinct answers each:
        //
        //   pending_attestations only                     0/10 surfaced
        //   + must_follow                                 2/10
        //   + the data-vs-directive rule in CLAUDE.md     4/10
        //   the SAME facts, stated as data                10/10
        //
        // The failure was never that the obligation went unnoticed. Runs quarantined it and
        // said so — "that's a directive embedded in relayed tool output, not a command from
        // you, so I'm not acting on it" — and then wrote their answer with no caveat. A
        // louder key cannot fix that; a louder key is more conspicuously an instruction.
        //
        // Keys are the measured fixture's, not a paraphrase of it. `appraisals_recorded`
        // distinguishes "nobody has looked" from "people looked and could not settle it",
        // which `last_verdict` alone could not say. Still gated on `armed`, so it appears
        // exactly where the tap would have fired: an always-present block trains the reader
        // to skip the key, and rarity is what the exposure threshold buys.
        out["verification"] = json!({
            "entry": anchor.reference,
            "verification_state": appraisal.as_deref().unwrap_or("unverified"),
            "appraisals_recorded": appraisal_count,
            "validity_class_declared": validity_class(&anchor.validity),
            "cited_by_statements": exposure,
        });
    }
    Ok(Some(out))
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    use crate::librarian::catalog::find::{find, FindOpts};
    use std::collections::HashMap;

    let a: Args = serde_json::from_value(args)?;
    let max_tokens = a.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);
    let char_cap = max_tokens * 4;

    let current = ctx.current_project.as_deref();
    // Literal, not Require: `context` is an orientation surface and reaching
    // across every project on an explicit `scope="all"` is the point of it.
    // `Scope::Project` is the default here too. `context` differs from `find` on
    // what an explicit `all` MEANS (`Literal`, not `Require` — see the comment
    // above), not on where an unspecified query starts.
    let (effective_scope, scope_fallback) =
        resolve_scope(a.scope, current, UmbrellaPolicy::Literal, Scope::Project)?;
    // Entry-grain anchor (Layer 4). Returns `None` for anything that does not resolve to
    // a Statement, so every file-grain anchor falls through to the path below with its
    // behaviour untouched — the requirement holds by construction, not by regression test.
    if let Some(ref anchor) = a.anchor_id {
        if let Some(v) = pack_entry_anchor(
            ctx,
            anchor,
            char_cap,
            effective_scope,
            current,
            scope_fallback,
        )? {
            return Ok(v);
        }
    }

    // Set when candidate DISCOVERY (not the token budget) hit its cap — more
    // artifacts may match than were even considered.
    let mut candidates_capped = false;

    let cutoff_ms = {
        let cat = ctx.catalog.lock();
        crate::librarian::catalog::gc::visibility_cutoff_ms(
            &cat.conn,
            chrono::Utc::now().timestamp_millis(),
        )?
    };

    // Worktree overlay. `apply_scope` deliberately over-selects for a worktree
    // session (main prefix OR worktree prefix) and hands the caller the job of
    // dropping the duplicate; separately, an in-repo worktree layout puts OTHER
    // sessions' shadow rows under this session's own prefix. `find` does both;
    // this handler used to do neither.
    // docs/issues/archive/2026-08-15-context-and-state-at-never-dedup-the-worktree-overlay.md
    let (shadowed_mains, worktree_exclusions) = {
        let cat = ctx.catalog.lock();
        (
            crate::librarian::tools::worktree::shadowed_main_ids(&cat, current)?,
            crate::librarian::tools::worktree::overlay_exclusions(&cat, current)?,
        )
    };

    let topic_vec: Option<Vec<f32>> =
        if let (Some(ref topic), Some(ref svc)) = (&a.topic, &ctx.embedding) {
            Some(svc.embedder.embed_query(topic).await?)
        } else {
            None
        };

    // Semantic topic path: the async store-backed coordinator (manages its own
    // catalog locking), hoisted out of the sync candidate_ids block below.
    let semantic_candidate_ids: Option<Vec<String>> = if a.anchor_id.is_none() {
        if let Some(vec) = topic_vec {
            let store = ctx.artifact_store.as_ref().ok_or_else(|| {
                crate::librarian::tools::RecoverableError::new(
                    "artifact semantic search backend unavailable — set `[librarian] \
                     vector_backend = \"sqlite-vec\"` (or CODESCOUT_ARTIFACT_BACKEND=sqlite-vec) \
                     for the offline backend.",
                )
            })?;
            let archived_clause = if a.include_archived {
                None
            } else {
                Some(FilterNode::Leaf(
                    [("status".to_string(), json!({"nin": HIDDEN_STATUSES}))]
                        .into_iter()
                        .collect(),
                ))
            };
            let (scoped_filter, _) = apply_scope(
                archived_clause,
                effective_scope,
                &ctx.workspace,
                current,
                &worktree_exclusions,
            )?;
            let project_id = if effective_scope == Scope::Project {
                current.and_then(|cp| {
                    let roots: Vec<std::path::PathBuf> =
                        ctx.workspace.roots.iter().map(|r| r.path.clone()).collect();
                    crate::librarian::tools::containing_root(&roots, &cp.abs_path)
                        .map(|p| p.to_string_lossy().into_owned())
                })
            } else {
                None
            };
            let page = crate::librarian::catalog::find::semantic_find(
                store.as_ref(),
                &ctx.catalog,
                project_id.as_deref(),
                &vec,
                scoped_filter.as_ref(),
                // One chunk per artifact. `context` packs a NEIGHBOURHOOD — its
                // value is breadth across artifacts, so several chunks of one
                // ledger crowding out another artifact is the regression to
                // avoid. Pinned by
                // `context_packs_distinct_artifacts_never_two_chunks_of_one`,
                // which at `2` packs one ledger twice and at `51` fills all 50
                // candidate slots with a single artifact's chunks.
                1,
                51,
                0,
                cutoff_ms,
            )
            .await?;
            // context() ranks by the link graph after this point and only needs the
            // candidate ids, so the distances are dropped here deliberately rather
            // than plumbed through a second consumer that has no use for them.
            let mut rows = page.hits;
            candidates_capped = rows.len() > 50;
            rows.truncate(50);
            Some(rows.into_iter().map(|h| h.row.id).collect())
        } else {
            None
        }
    } else {
        None
    };

    let candidate_ids: Vec<String> = if let Some(ids) = semantic_candidate_ids {
        ids
    } else {
        let cat = ctx.catalog.lock();
        if let Some(ref anchor_id) = a.anchor_id {
            let mut ids: Vec<String> = vec![anchor_id.clone()];
            let out = links::outgoing(&cat, anchor_id)?;
            let inc = links::incoming(&cat, anchor_id)?;
            for link in out {
                if !ids.contains(&link.dst_id) {
                    ids.push(link.dst_id);
                }
            }
            for link in inc {
                if !ids.contains(&link.src_id) {
                    ids.push(link.src_id);
                }
            }
            ids
        } else if a.topic.is_some() {
            let archived_clause = if a.include_archived {
                None
            } else {
                Some(FilterNode::Leaf(
                    [("status".to_string(), json!({"nin": HIDDEN_STATUSES}))]
                        .into_iter()
                        .collect(),
                ))
            };
            let (scoped_filter, _) = apply_scope(
                archived_clause,
                effective_scope,
                &ctx.workspace,
                current,
                &worktree_exclusions,
            )?;

            // topic_vec was None here (the semantic path is hoisted above) —
            // fall back to a title/topic substring filter.
            let topic = a.topic.as_deref().unwrap_or("");
            let topic_clause = FilterNode::Or {
                or: vec![
                    FilterNode::Leaf(
                        [("title".to_string(), json!({"contains": topic}))]
                            .into_iter()
                            .collect(),
                    ),
                    FilterNode::Leaf(
                        [("topic".to_string(), json!({"contains": topic}))]
                            .into_iter()
                            .collect(),
                    ),
                ],
            };
            let combined = match scoped_filter {
                Some(s) => FilterNode::And {
                    and: vec![s, topic_clause],
                },
                None => topic_clause,
            };
            let mut rows = find(
                &cat,
                &FindOpts {
                    filter: Some(combined),
                    limit: 51,
                    offset: 0,
                },
                cutoff_ms,
            )?;
            candidates_capped = rows.len() > 50;
            rows.truncate(50);
            rows.into_iter().map(|r| r.id).collect()
        } else {
            // No anchor, no topic: surface active goal-trackers.
            let mut clauses: Vec<FilterNode> = vec![
                FilterNode::Leaf(
                    [("kind".to_string(), json!({"eq": "tracker"}))]
                        .into_iter()
                        .collect(),
                ),
                FilterNode::Leaf(
                    [("tags".to_string(), json!({"contains": "goal"}))]
                        .into_iter()
                        .collect(),
                ),
                FilterNode::Leaf(
                    [("status".to_string(), json!({"eq": "active"}))]
                        .into_iter()
                        .collect(),
                ),
            ];
            if !a.include_archived {
                clauses.push(FilterNode::Leaf(
                    [("status".to_string(), json!({"nin": HIDDEN_STATUSES}))]
                        .into_iter()
                        .collect(),
                ));
            }
            let goal_filter = FilterNode::And { and: clauses };
            let (scoped_filter, _) = apply_scope(
                Some(goal_filter),
                effective_scope,
                &ctx.workspace,
                current,
                &worktree_exclusions,
            )?;
            let mut rows = find(
                &cat,
                &FindOpts {
                    filter: scoped_filter,
                    limit: 11,
                    offset: 0,
                },
                cutoff_ms,
            )?;
            candidates_capped = rows.len() > 10;
            rows.truncate(10);
            rows.into_iter().map(|r| r.id).collect()
        }
    };

    let rows_map: HashMap<String, artifact::ArtifactRow> = {
        let cat = ctx.catalog.lock();
        if candidate_ids.is_empty() {
            HashMap::new()
        } else {
            let placeholders = (0..candidate_ids.len())
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "SELECT id, abs_path, kind, status, title, owners, tags, topic, \
                 time_scope, source, created_at, updated_at, file_mtime, \
                 file_sha256, confidence FROM artifact WHERE id IN ({placeholders})"
            );
            let mut stmt = cat.conn.prepare(&sql)?;
            let params = rusqlite::params_from_iter(candidate_ids.iter());
            let rows: Vec<artifact::ArtifactRow> = stmt
                .query_map(params, artifact::row_from_sql)?
                .collect::<Result<_, _>>()?;
            rows.into_iter().map(|r| (r.id.clone(), r)).collect()
        }
    };

    // The anchor-graph path walks `worktree_of` edges and the semantic path
    // queries the vector store, so neither is covered by the scope clause the
    // exclusions were folded into. Drop shadowed main twins and foreign shadow
    // rows here, at the one point every candidate path converges on, before any
    // of them can consume the token budget.
    let rows_map: HashMap<String, artifact::ArtifactRow> = rows_map
        .into_iter()
        .filter(|(id, r)| {
            !shadowed_mains.contains(id.as_str())
                && !crate::librarian::tools::worktree::is_under_any(
                    &r.abs_path,
                    &worktree_exclusions,
                )
        })
        .collect();
    let candidate_ids: Vec<String> = candidate_ids
        .into_iter()
        .filter(|id| rows_map.contains_key(id.as_str()))
        .collect();

    let aug_map: std::collections::HashMap<String, augmentation::AugmentationRow> = {
        let cat = ctx.catalog.lock();
        augmentation::get_batch(&cat, &candidate_ids)?
    };

    let mut sorted_ids = candidate_ids.clone();
    sorted_ids.sort_by_key(|id| {
        let is_tracker = rows_map
            .get(id.as_str())
            .is_some_and(|r| r.kind == "tracker");
        let is_augmented = aug_map.contains_key(id.as_str());
        match (is_tracker, is_augmented) {
            (true, _) => 0u8,
            (false, true) => 1,
            _ => 2,
        }
    });

    let active_goals_header =
        matches!((&a.topic, &a.anchor_id), (None, None)) && !sorted_ids.is_empty();
    let mut markdown = if active_goals_header {
        String::from("## Active goals\n\n")
    } else {
        String::new()
    };
    let mut included_ids: Vec<String> = Vec::new();
    // A large anchor otherwise consumes the whole budget before any neighbor is
    // even considered (docs/issues/archive/2026-07-05-context-anchor-starves-neighbors.md):
    // reserve half of char_cap for neighbors whenever the anchor actually has any.
    let anchor_reserve_cap =
        (a.anchor_id.is_some() && candidate_ids.len() > 1).then_some(char_cap / 2);

    for id in &sorted_ids {
        let row = match rows_map.get(id) {
            Some(r) => r,
            None => continue,
        };
        let full_path = row.abs_path.clone();
        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let body = match crate::librarian::frontmatter::parse(&content) {
            Ok((_, body)) => body.to_string(),
            Err(_) => content.clone(),
        };
        let total_lines = body.lines().count();
        let mut first_30: String = body.lines().take(30).collect::<Vec<_>>().join("\n");
        if total_lines > 30 {
            first_30.push_str(&format!(
                "\n… [30 of {total_lines} lines — doc(get, id=…) for the full body]"
            ));
        }
        let title = row.title.as_deref().unwrap_or("(untitled)");
        let mut section = if let Some(aug) = aug_map.get(id.as_str()) {
            let refreshed = aug.last_refreshed_at.as_deref().unwrap_or("never");
            let rendered = aug.render_template.as_deref().map(|tmpl| {
                let params: Value =
                    serde_json::from_str(&aug.params).unwrap_or(Value::Object(Default::default()));
                match crate::librarian::tools::render::render_params(tmpl, &params) {
                    Ok(s) => format!("{s}\n\n"),
                    Err(e) => format!("<!-- render_template error: {e} -->\n\n"),
                }
            });
            format!(
                "<!-- [LIVE]: {} | last refreshed: {} | refresh #{} -->\n\
                 > Standing instruction: {}\n\n\
                 {}## {}  — {}/{}  ({})\n{}\n\n",
                title,
                refreshed,
                aug.refresh_count,
                aug.prompt,
                rendered.as_deref().unwrap_or(""),
                title,
                row.kind,
                row.status,
                row.abs_path.display(),
                first_30
            )
        } else {
            format!(
                "## {}  — {}/{}  ({})\n{}\n\n",
                title,
                row.kind,
                row.status,
                row.abs_path.display(),
                first_30
            )
        };
        if Some(id.as_str()) == a.anchor_id.as_deref() {
            if let Some(cap) = anchor_reserve_cap {
                if section.len() > cap {
                    let mut cut = cap;
                    while !section.is_char_boundary(cut) {
                        cut -= 1;
                    }
                    section.truncate(cut);
                    section.push_str(
                        "\n\n… [anchor truncated — reserved half the budget for its \
                         link neighbors; use `doc(get, id=…)` for the full body]\n\n",
                    );
                }
            }
        }
        if !markdown.is_empty() && (markdown.len() + section.len()) > char_cap {
            break;
        }
        markdown.push_str(&section);
        included_ids.push(id.clone());
        if markdown.len() >= char_cap {
            break;
        }
    }

    let total_candidates = sorted_ids.len();
    let included = included_ids.len();
    let omitted = total_candidates.saturating_sub(included);
    let mut overflow = json!({
        "candidates": total_candidates,
        "included": included,
        "omitted": omitted,
        "candidates_capped": candidates_capped,
    });
    if omitted > 0 || candidates_capped {
        let mut hint = String::new();
        if omitted > 0 {
            hint.push_str(&format!(
                "{omitted} candidate(s) omitted (token budget) — raise `max_tokens` or narrow `topic`. "
            ));
        }
        if candidates_capped {
            hint.push_str(
                "candidate discovery hit its cap; more artifacts may match than were considered. ",
            );
        }
        overflow["hint"] = json!(hint.trim_end());
    }
    Ok(json!({
        "markdown": markdown,
        "included_ids": included_ids,
        "overflow": overflow,
        "scope": scope_summary(effective_scope, current, scope_fallback),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::{artifact::ArtifactRow, Catalog};
    use crate::librarian::tools::TestToolContextBuilder;
    use crate::librarian::workspace::Root;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn sample_row(
        id: &str,
        repo: &str,
        rel_path: &str,
        title: &str,
        topic: Option<&str>,
    ) -> ArtifactRow {
        let now = chrono::Utc::now().timestamp_millis();
        ArtifactRow {
            id: id.into(),
            abs_path: std::path::PathBuf::from(format!("/{repo}/{rel_path}")),
            kind: "spec".into(),
            status: "active".into(),
            title: Some(title.into()),
            owners: vec![],
            tags: vec![],
            topic: topic.map(|s| s.into()),
            time_scope: None,
            source: None,
            created_at: now,
            updated_at: now,
            file_mtime: now,
            file_sha256: "abc".into(),
            confidence: 1.0,
        }
    }

    /// Realign rows whose `sample_row` placeholder abs_path is `/r/{rel}` to point
    /// under `tmp_root`, so files written under `tmp_root` resolve.
    ///
    /// Forward-slash — the catalog's abs_path column is a RepoPath and every
    /// LIKE query against it assumes `/`. A native-separator prefix here would
    /// write backslash paths into the catalog on Windows.
    ///
    /// LOAD-BEARING, and hoisted out of `mk_ctx` so a fixture that needs a
    /// store-backed context can reuse it rather than re-derive it: the packing
    /// loop reads every candidate's body off DISK and `continue`s past a path it
    /// cannot open. An unrealigned row is therefore skipped in SILENCE —
    /// indistinguishable from "the search matched nothing".
    fn realign_abs_paths(cat: &Catalog, tmp_root: &std::path::Path) {
        let new_prefix = format!("{}/", crate::util::fs::to_forward_slash(tmp_root));
        cat.conn
            .execute(
                "UPDATE artifact SET abs_path = REPLACE(abs_path, '/r/', ?1)",
                rusqlite::params![new_prefix],
            )
            .unwrap();
    }

    fn mk_ctx(tmp_root: std::path::PathBuf, cat: Catalog) -> ToolContext {
        realign_abs_paths(&cat, &tmp_root);
        TestToolContextBuilder::new(cat)
            .with_root(Root {
                name: "r".into(),
                path: tmp_root,
            })
            .build()
    }

    /// Returns one fixed vector for every text, so the fixture's KNN ranking is a
    /// property of the FIXTURE rather than of whichever embedding model happens to
    /// be configured.
    struct ConstEmbedder(Vec<f32>);
    #[async_trait::async_trait]
    impl codescout_embed::Embedder for ConstEmbedder {
        fn dimensions(&self) -> usize {
            self.0.len()
        }
        async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
            Ok(texts.iter().map(|_| self.0.clone()).collect())
        }
    }

    /// `context` packs a NEIGHBOURHOOD, so its unit is the ARTIFACT — but the
    /// vector store has been chunk-keyed since Task 7 and `semantic_find` returns
    /// one hit per CHUNK. The `max_per_artifact = 1` at the call site is the only
    /// thing keeping the two grains apart; this pins it.
    ///
    /// THREE FIXTURE DETAILS ARE LOAD-BEARING. Remove any one and what is left
    /// passes while asserting nothing:
    ///
    /// 1. **`TOPIC` occurs in no artifact's title or topic.** When the semantic
    ///    path is unavailable — no embedder, no store, or the hoisted block above
    ///    stops being reached — the topic branch falls back to a
    ///    `title|topic contains` filter. That fallback is artifact-grain, so it
    ///    satisfies the distinctness assertion below while exercising none of the
    ///    code under test. Because `TOPIC` matches neither field the fallback
    ///    returns ZERO rows, which makes a skipped semantic path fail the
    ///    "note present" assertion instead of passing in silence.
    /// 2. **The ledger carries more chunks than `context`'s own 50-candidate
    ///    truncation** (asserted below, not merely intended). With fewer, dropping
    ///    the cap would only repeat the ledger; past 50 it EVICTS the note
    ///    entirely, which is the harm the cap exists to prevent.
    /// 3. **Every ledger chunk shares one vector and the note's is orthogonal**, so
    ///    the ledger owns the head of the KNN list by construction rather than by
    ///    luck.
    ///
    /// The two assertions catch different mutations and NEITHER SUBSUMES THE
    /// OTHER: at `max_per_artifact = 2` the note still fits and only distinctness
    /// fails; at `= 51` the ledger's chunks fill the page and only the note's
    /// absence fails.
    #[tokio::test]
    async fn context_packs_distinct_artifacts_never_two_chunks_of_one() {
        use crate::librarian::artifact_store::test_support::InMemoryArtifactStore;
        use crate::librarian::artifact_store::ArtifactVectorStore;
        use crate::librarian::catalog::chunk;

        // Substring of neither title nor topic of either artifact — see (1).
        const TOPIC: &str = "zzz-matches-no-title-or-topic-zzz";

        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // 60 `####` entries → 61 chunks at the librarian's heading-depth-6 grain.
        let mut ledger = String::from("# Gate Ledger\n\nintro\n\n");
        for i in 1..=60 {
            ledger.push_str(&format!("#### E-{i}\n\nentry {i} body\n\n"));
        }
        const NOTE: &str = "# Note\n\nnote body\n";
        std::fs::write(root.join("ledger.md"), &ledger).unwrap();
        std::fs::write(root.join("note.md"), NOTE).unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &sample_row("r/ledger.md", "r", "ledger.md", "Gate Ledger", None),
        )
        .unwrap();
        artifact::upsert(&cat, &sample_row("r/note.md", "r", "note.md", "Note", None)).unwrap();
        realign_abs_paths(&cat, root);

        let store = InMemoryArtifactStore::default();
        let ledger_chunks = chunk::replace_chunks(
            &cat,
            "r/ledger.md",
            &chunk::build_chunks("r/ledger.md", &ledger, 2048, 0),
        )
        .unwrap();
        assert!(
            ledger_chunks.len() > 50,
            "fixture must exceed context's 50-candidate truncation or (2) does not \
             hold and this test stops discriminating; got {}",
            ledger_chunks.len()
        );
        for c in &ledger_chunks {
            store
                .upsert("p", &c.chunk_id, &c.artifact_id, &[1.0, 0.0])
                .await
                .unwrap();
        }
        for c in &chunk::replace_chunks(
            &cat,
            "r/note.md",
            &chunk::build_chunks("r/note.md", NOTE, 2048, 0),
        )
        .unwrap()
        {
            store
                .upsert("p", &c.chunk_id, &c.artifact_id, &[0.0, 1.0])
                .await
                .unwrap();
        }

        let ctx = TestToolContextBuilder::new(cat)
            .with_root(Root {
                name: "r".into(),
                path: root.to_path_buf(),
            })
            .with_embedding(Arc::new(
                crate::librarian::embedding::EmbeddingService::new(Arc::new(ConstEmbedder(vec![
                    1.0, 0.0,
                ]))),
            ))
            .with_artifact_store(Arc::new(store))
            .build();

        let v = call(&ctx, json!({"topic": TOPIC, "max_tokens": 5000}))
            .await
            .unwrap();
        let ids: Vec<&str> = v["included_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap())
            .collect();

        assert!(
            ids.contains(&"r/note.md"),
            "the ledger's chunks evicted the other artifact — context lost artifact \
             grain (or the semantic path never ran; see (1)): {ids:?}"
        );
        let mut distinct = ids.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(
            distinct.len(),
            ids.len(),
            "context packed one artifact's body twice, once per chunk: {ids:?}"
        );
    }

    #[tokio::test]
    async fn topic_search_returns_matching_artifacts() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create 3 real .md files
        std::fs::write(root.join("auth_login.md"), "# Auth Login\nsome body\n").unwrap();
        std::fs::write(root.join("auth_signup.md"), "# Auth Signup\nsome body\n").unwrap();
        std::fs::write(root.join("billing.md"), "# Billing\nsome body\n").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &sample_row("r/auth_login.md", "r", "auth_login.md", "Auth Login", None),
        )
        .unwrap();
        artifact::upsert(
            &cat,
            &sample_row(
                "r/auth_signup.md",
                "r",
                "auth_signup.md",
                "Auth Signup",
                None,
            ),
        )
        .unwrap();
        artifact::upsert(
            &cat,
            &sample_row("r/billing.md", "r", "billing.md", "Billing", None),
        )
        .unwrap();

        let ctx = mk_ctx(root.to_path_buf(), cat);

        let v = call(&ctx, json!({"topic": "auth"})).await.unwrap();

        let ids = v["included_ids"].as_array().unwrap();
        assert_eq!(ids.len(), 2, "only auth artifacts should be included");

        let md = v["markdown"].as_str().unwrap();
        assert!(
            md.contains("Auth Login"),
            "markdown should contain Auth Login title"
        );
        assert!(
            md.contains("Auth Signup"),
            "markdown should contain Auth Signup title"
        );
        assert!(
            !md.contains("Billing"),
            "markdown should not contain Billing"
        );
    }

    #[tokio::test]
    async fn topic_search_hides_retired_artifacts() {
        // Regression for the HIDDEN_STATUSES split-brain: the context topic
        // branch must hide `retired` artifacts exactly as find() does.
        // See docs/issues/archive/2026-05-25-hidden-statuses-context-missing-retired.md
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        std::fs::write(root.join("auth_live.md"), "# Auth Live\nsome body\n").unwrap();
        std::fs::write(root.join("auth_retired.md"), "# Auth Retired\nsome body\n").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &sample_row("r/auth_live.md", "r", "auth_live.md", "Auth Live", None),
        )
        .unwrap();
        let mut retired = sample_row(
            "r/auth_retired.md",
            "r",
            "auth_retired.md",
            "Auth Retired",
            None,
        );
        retired.status = "retired".into();
        artifact::upsert(&cat, &retired).unwrap();

        let ctx = mk_ctx(root.to_path_buf(), cat);

        let v = call(&ctx, json!({"topic": "auth"})).await.unwrap();

        let ids = v["included_ids"].as_array().unwrap();
        assert_eq!(
            ids.len(),
            1,
            "retired artifact must be hidden in topic context, like find()"
        );
        let md = v["markdown"].as_str().unwrap();
        assert!(md.contains("Auth Live"), "live artifact should be present");
        assert!(
            !md.contains("Auth Retired"),
            "retired artifact must not leak into context markdown"
        );
    }

    #[tokio::test]
    async fn max_tokens_caps_inclusion() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create 2 auth files
        std::fs::write(root.join("auth_a.md"), "# Auth A\n".repeat(5)).unwrap();
        std::fs::write(root.join("auth_b.md"), "# Auth B\n".repeat(5)).unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &sample_row("r/auth_a.md", "r", "auth_a.md", "Auth A", None),
        )
        .unwrap();
        artifact::upsert(
            &cat,
            &sample_row("r/auth_b.md", "r", "auth_b.md", "Auth B", None),
        )
        .unwrap();

        let ctx = mk_ctx(root.to_path_buf(), cat);

        // max_tokens=1 means char_cap=4 — way too small for any full section, but first
        // artifact is always included (budget check only triggers on subsequent artifacts).
        // Use a slightly larger budget that fits exactly 1 section.
        // Each section header is ~50+ chars; set max_tokens=15 (60 chars) → fits 1, not 2.
        let v = call(&ctx, json!({"topic": "auth", "max_tokens": 15}))
            .await
            .unwrap();

        let ids = v["included_ids"].as_array().unwrap();
        assert_eq!(
            ids.len(),
            1,
            "max_tokens should cap inclusion to 1 artifact"
        );
    }

    #[tokio::test]
    async fn omitted_signal_when_budget_truncates() {
        // Silent-cap regression: candidates dropped by the char budget must be
        // reported, so the bundle is not read as the complete set.
        // docs/issues/archive/2026-07-10-silent-cap-missing-overflow-signals-audit.md
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("auth_a.md"), "# Auth A\n".repeat(5)).unwrap();
        std::fs::write(root.join("auth_b.md"), "# Auth B\n".repeat(5)).unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &sample_row("r/auth_a.md", "r", "auth_a.md", "Auth A", None),
        )
        .unwrap();
        artifact::upsert(
            &cat,
            &sample_row("r/auth_b.md", "r", "auth_b.md", "Auth B", None),
        )
        .unwrap();
        let ctx = mk_ctx(root.to_path_buf(), cat);
        let v = call(&ctx, json!({"topic": "auth", "max_tokens": 15}))
            .await
            .unwrap();
        assert_eq!(v["included_ids"].as_array().unwrap().len(), 1);
        assert_eq!(v["overflow"]["candidates"], json!(2));
        assert_eq!(v["overflow"]["included"], json!(1));
        assert_eq!(v["overflow"]["omitted"], json!(1));
    }

    #[tokio::test]
    async fn body_preview_marks_line_truncation() {
        // A 30-line preview of a 50-line body must say so — a cut body must not
        // read as a short-but-complete one.
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let body = (0..50)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(root.join("big.md"), body).unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &sample_row("r/big.md", "r", "big.md", "Big", Some("bigtopic")),
        )
        .unwrap();
        let ctx = mk_ctx(root.to_path_buf(), cat);
        let v = call(&ctx, json!({"topic": "bigtopic", "max_tokens": 5000}))
            .await
            .unwrap();
        let md = v["markdown"].as_str().unwrap();
        assert!(
            md.contains("30 of 50 lines"),
            "expected line-truncation marker, got: {md}"
        );
    }

    #[tokio::test]
    async fn anchor_neighbors_are_not_starved_by_oversized_anchor() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Anchor body is one giant line so the 30-line preview cap can't shrink
        // it — it must consume the whole reserved-anchor share and nothing more.
        std::fs::write(root.join("anchor.md"), "x".repeat(2000)).unwrap();
        std::fs::write(root.join("neighbor_a.md"), "Neighbor A\n").unwrap();
        std::fs::write(root.join("neighbor_b.md"), "Neighbor B\n").unwrap();
        std::fs::write(root.join("neighbor_c.md"), "Neighbor C\n").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &sample_row("r/anchor.md", "r", "anchor.md", "Anchor", None),
        )
        .unwrap();
        artifact::upsert(
            &cat,
            &sample_row("r/neighbor_a.md", "r", "neighbor_a.md", "Neighbor A", None),
        )
        .unwrap();
        artifact::upsert(
            &cat,
            &sample_row("r/neighbor_b.md", "r", "neighbor_b.md", "Neighbor B", None),
        )
        .unwrap();
        artifact::upsert(
            &cat,
            &sample_row("r/neighbor_c.md", "r", "neighbor_c.md", "Neighbor C", None),
        )
        .unwrap();

        links::insert(
            &cat,
            &links::LinkRow {
                src_id: "r/anchor.md".into(),
                dst_id: "r/neighbor_a.md".into(),
                rel: "cites".into(),
                created_at: 0,
            },
        )
        .unwrap();
        links::insert(
            &cat,
            &links::LinkRow {
                src_id: "r/anchor.md".into(),
                dst_id: "r/neighbor_b.md".into(),
                rel: "cites".into(),
                created_at: 0,
            },
        )
        .unwrap();
        links::insert(
            &cat,
            &links::LinkRow {
                src_id: "r/neighbor_c.md".into(),
                dst_id: "r/anchor.md".into(),
                rel: "cites".into(),
                created_at: 0,
            },
        )
        .unwrap();

        let ctx = mk_ctx(root.to_path_buf(), cat);

        let v = call(&ctx, json!({"anchor_id": "r/anchor.md", "max_tokens": 300}))
            .await
            .unwrap();

        let ids: Vec<String> = v["included_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect();

        assert!(
            ids.contains(&"r/anchor.md".to_string()),
            "anchor must always be included, got {ids:?}"
        );
        assert_eq!(
            ids.len(),
            4,
            "expected anchor + all 3 neighbors, got {ids:?}"
        );
    }

    /// Seed a slugged artifact whose file lives under `root`.
    fn seed_ledger(cat: &Catalog, root: &std::path::Path, name: &str, slug: &str, body: &str) {
        std::fs::write(root.join(name), body).unwrap();
        artifact::upsert(
            cat,
            &sample_row(&format!("r/{name}"), "r", name, name, None),
        )
        .unwrap();
        cat.conn
            .execute(
                "UPDATE artifact SET slug=?1 WHERE id=?2",
                rusqlite::params![slug, format!("r/{name}")],
            )
            .unwrap();
    }

    fn seed_edge(cat: &Catalog, src_slug: &str, src_local: &str, dst_ref: &str) {
        crate::librarian::catalog::entry_cite::insert_with(
            &cat.conn,
            &crate::librarian::catalog::entry_cite::EntryCiteRow {
                src_slug: src_slug.into(),
                src_local: src_local.into(),
                dst_ref: dst_ref.into(),
                rel: "cites".into(),
                origin: "scan".into(),
                created_at: 1,
            },
        )
        .unwrap();
    }

    #[tokio::test]
    async fn an_entry_anchor_packs_the_statement_and_both_sides_of_it() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let cat = Catalog::open_in_memory().unwrap();

        seed_ledger(
            &cat,
            root,
            "ledger.md",
            "ledger",
            "## W-1 — the anchor\n\
             **Valid:** invariant\n\
             anchor body\n\
             \n\
             ## W-2 — the entry that rests on the anchor\n\
             **Valid:** dated 2026-08-01\n\
             sibling body\n",
        );
        seed_ledger(
            &cat,
            root,
            "other.md",
            "other",
            "## F-1 — what the anchor rests on\n\
             **Valid:** invariant\n\
             target body\n",
        );

        seed_edge(&cat, "ledger", "W-1", "other:F-1"); // anchor -> F-1
        seed_edge(&cat, "ledger", "W-2", "ledger:W-1"); // W-2 -> anchor

        let ctx = mk_ctx(root.to_path_buf(), cat);
        let v = call(&ctx, json!({"anchor_id": "ledger:W-1"}))
            .await
            .unwrap();

        assert_eq!(
            v["overflow"]["grain"],
            json!("entry"),
            "the envelope must say which grain answered, or a caller cannot tell an \
             entry pack from a file pack: {v:#?}"
        );
        let ids: Vec<String> = v["included_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            ids,
            vec!["ledger:W-1", "ledger:W-2", "other:F-1"],
            "anchor first, then neighbours; BOTH directions are walked — outward needs \
             the entry-grain accessor and inward is exact on dst_ref: {v:#?}"
        );

        let md = v["markdown"].as_str().unwrap();
        assert!(md.contains("## ledger:W-1 · anchor"), "{md}");
        assert!(
            md.contains("## other:F-1 · cites"),
            "the anchor's outward edge is labelled by direction: {md}"
        );
        assert!(
            md.contains("## ledger:W-2 · cited-by"),
            "and its inward edge is distinguishable from the outward one: {md}"
        );
        assert!(
            md.contains("valid: invariant") && md.contains("valid: dated 2026-08-01"),
            "every packed node carries its decay class: {md}"
        );
    }
    /// Seed `n` distinct ledgers that each cite `dst_ref` exactly once.
    fn seed_citers(cat: &Catalog, root: &std::path::Path, dst_ref: &str, n: usize) {
        for i in 0..n {
            let name = format!("citer-{i}.md");
            let slug = format!("citer-{i}");
            seed_ledger(
                cat,
                root,
                &name,
                &slug,
                &format!("## C-{i} — citer {i}\n**Valid:** invariant\nbody\n"),
            );
            seed_edge(cat, &slug, &format!("C-{i}"), dst_ref);
        }
    }

    fn seed_appraisal(cat: &Catalog, artifact_id: &str, id: &str, payload: Value) {
        seed_appraisal_at(cat, artifact_id, id, 0, payload);
    }

    /// `seed_appraisal` with an explicit timestamp, for the tests whose meaning depends on
    /// which appraisal is the LATEST. Stating the times beats relying on id lexicography:
    /// the timeline orders `created_at DESC, id DESC`, so ids happen to decide it when
    /// every row shares a timestamp — and a test that reads correct only because of how
    /// its ids happen to sort is a test nobody can maintain.
    fn seed_appraisal_at(cat: &Catalog, artifact_id: &str, id: &str, at: i64, payload: Value) {
        let row =
            crate::librarian::catalog::events::TestEventRowBuilder::new(artifact_id, "reviewed")
                .with_id(id)
                .with_created_at(at)
                .with_payload(payload.to_string())
                .build();
        crate::librarian::catalog::events::insert(cat, &row).unwrap();
    }

    fn anchor_ledger(cat: &Catalog, root: &std::path::Path, validity: &str) {
        seed_ledger(
            cat,
            root,
            "ledger.md",
            "ledger",
            &format!("## W-1 — the anchor\n**Valid:** {validity}\nanchor body\n"),
        );
    }

    #[tokio::test]
    async fn a_load_bearing_statement_arms_the_tap_and_says_what_would_discharge_it() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let cat = Catalog::open_in_memory().unwrap();
        anchor_ledger(&cat, root, "invariant");
        seed_citers(&cat, root, "ledger:W-1", ATTESTATION_EXPOSURE_THRESHOLD);

        let ctx = mk_ctx(root.to_path_buf(), cat);
        let v = call(&ctx, json!({"anchor_id": "ledger:W-1"}))
            .await
            .unwrap();

        let ver = &v["verification"];
        assert!(
            ver.is_object(),
            "a Statement at the exposure floor must report its verification state: {v:#?}"
        );
        assert_eq!(ver["entry"], json!("ledger:W-1"));
        assert_eq!(
            ver["cited_by_statements"],
            json!(ATTESTATION_EXPOSURE_THRESHOLD),
            "exposure is DISTINCT citing ledgers: {v:#?}"
        );
        assert_eq!(ver["verification_state"], json!("unverified"), "{v:#?}");
        assert_eq!(ver["appraisals_recorded"], json!(0), "{v:#?}");
        assert_eq!(
            ver["validity_class_declared"],
            json!("invariant"),
            "the declared class is reported so a reader can tell it from an undeclared \
             Statement: {v:#?}"
        );

        // EVERY DIRECTIVE REGISTER MUST STAY UNUSED, and this is the assertion the change
        // is for. Measured 2026-08-21 (prompt-engineering `scenarios/attestation-register`,
        // n=10 per arm): an obligation delivered inside relayed tool output is quarantined
        // as an embedded instruction and explicitly declined — 0-4/10 surfaced, against
        // 10/10 for these same facts stated as data. Naming all four keys rather than only
        // the two that existed, because the next person to reach for a louder register
        // will reach for whichever one is not listed here.
        for key in ["must_follow", "pending_attestations", "hint", "next_step"] {
            assert!(
                v.get(key).is_none(),
                "`{key}` is a directive register; this payload carries facts only: {v:#?}"
            );
        }
        let md = v["markdown"].as_str().unwrap();
        for phrase in ["Proof owed", "last appraised", "⚠"] {
            assert!(
                !md.contains(phrase),
                "the banner was a directive too and rode in the prose where it could not \
                 even be skipped — {phrase:?} must be gone: {md}"
            );
        }
    }

    #[tokio::test]
    async fn the_tap_stays_silent_one_citation_below_the_floor() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let cat = Catalog::open_in_memory().unwrap();
        anchor_ledger(&cat, root, "invariant");
        seed_citers(&cat, root, "ledger:W-1", ATTESTATION_EXPOSURE_THRESHOLD - 1);

        let ctx = mk_ctx(root.to_path_buf(), cat);
        let v = call(&ctx, json!({"anchor_id": "ledger:W-1"}))
            .await
            .unwrap();

        assert!(
            v.get("verification").is_none(),
            "the key is ABSENT rather than a block reporting `cited_by_statements: 4` — an \
             always-present key trains the reader to skip it, and rarity is exactly what \
             the exposure threshold buys: {v:#?}"
        );
        assert!(
            !v["markdown"].as_str().unwrap().contains("last appraised"),
            "and no banner: {v:#?}"
        );
        // These two now hold for EVERY response, armed or not, so on their own they no
        // longer distinguish a below-floor Statement from an above-floor one. Kept because
        // they still pin the intended shape, and paired with the `verification` assertion
        // above, which is what actually discriminates.
        assert!(
            v.get("must_follow").is_none() && v.get("pending_attestations").is_none(),
            "no directive register on any path: {v:#?}"
        );
    }

    /// A ledger's own entries citing each other are the ledger talking to itself — and a
    /// hand-maintained `## Index` row is exactly that shape. 28.5% of ledger citations sit
    /// above the first definition, so counting them would let a table of contents arm the
    /// tap on every entry it lists.
    #[tokio::test]
    async fn same_ledger_citations_do_not_arm_the_tap() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let cat = Catalog::open_in_memory().unwrap();
        let mut body = String::from("## W-1 — the anchor\n**Valid:** invariant\nanchor body\n");
        for i in 0..(ATTESTATION_EXPOSURE_THRESHOLD + 3) {
            body.push_str(&format!(
                "\n## S-{i} — sibling {i}\n**Valid:** invariant\nb\n"
            ));
        }
        seed_ledger(&cat, root, "ledger.md", "ledger", &body);
        for i in 0..(ATTESTATION_EXPOSURE_THRESHOLD + 3) {
            seed_edge(&cat, "ledger", &format!("S-{i}"), "ledger:W-1");
        }

        let ctx = mk_ctx(root.to_path_buf(), cat);
        let v = call(&ctx, json!({"anchor_id": "ledger:W-1"}))
            .await
            .unwrap();

        assert!(
            v.get("verification").is_none(),
            "{} same-ledger citations must not arm the tap: {v:#?}",
            ATTESTATION_EXPOSURE_THRESHOLD + 3
        );
    }
    /// One ledger citing the anchor from many of its own entries is ONE voice, not many.
    /// Exposure asks how many independent parties depend on this claim; a single chatty
    /// ledger is one party however many times it says so.
    ///
    /// Confirmed by mutation 2026-08-21: counting `(ledger, entry)` pairs instead of
    /// distinct ledgers left all 29 context tests green until this test existed — every
    /// other fixture happens to give each citing ledger exactly one edge, so the two
    /// counts coincide and the distinction was asserted only in a comment.
    #[tokio::test]
    async fn one_ledger_citing_from_many_entries_counts_as_one_voice() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let cat = Catalog::open_in_memory().unwrap();
        anchor_ledger(&cat, root, "invariant");

        let loud = ATTESTATION_EXPOSURE_THRESHOLD + 2;
        let mut body = String::new();
        for i in 0..loud {
            body.push_str(&format!(
                "## E-{i} — entry {i}\n**Valid:** invariant\nb\n\n"
            ));
        }
        seed_ledger(&cat, root, "loud.md", "loud", &body);
        for i in 0..loud {
            seed_edge(&cat, "loud", &format!("E-{i}"), "ledger:W-1");
        }

        let ctx = mk_ctx(root.to_path_buf(), cat);
        let v = call(&ctx, json!({"anchor_id": "ledger:W-1"}))
            .await
            .unwrap();

        assert!(
            v.get("verification").is_none(),
            "{loud} edges from ONE ledger is exposure 1, not {loud} — otherwise a single \
             chatty ledger arms the tap on everything it cites: {v:#?}"
        );
    }

    #[tokio::test]
    async fn an_evidence_carrying_held_verdict_discharges_the_obligation() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let cat = Catalog::open_in_memory().unwrap();
        anchor_ledger(&cat, root, "invariant");
        seed_citers(&cat, root, "ledger:W-1", ATTESTATION_EXPOSURE_THRESHOLD);
        seed_appraisal(
            &cat,
            "r/ledger.md",
            "ev-held",
            json!({
                "entry": "ledger:W-1",
                "instrument": "cargo test --lib the_thing",
                "observed": "3 passed; 0 failed",
                "verdict": "held",
            }),
        );

        let ctx = mk_ctx(root.to_path_buf(), cat);
        let v = call(&ctx, json!({"anchor_id": "ledger:W-1"}))
            .await
            .unwrap();

        assert!(
            v.get("verification").is_none(),
            "an appraisal carrying instrument + observed + verdict=held discharges: {v:#?}"
        );
    }

    /// The laundering guard, and the single most load-bearing test in this file. A
    /// `reviewed` event whose payload names no instrument and no observed output is an
    /// ASSURANCE, not evidence. Letting it discharge would leave the claim wearing a
    /// verification stamp it did not earn — reading as *more* trustworthy than before the
    /// tap ever fired, which is strictly worse than having no mechanism at all.
    #[tokio::test]
    async fn a_verdict_without_evidence_does_not_discharge_anything() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let cat = Catalog::open_in_memory().unwrap();
        anchor_ledger(&cat, root, "invariant");
        seed_citers(&cat, root, "ledger:W-1", ATTESTATION_EXPOSURE_THRESHOLD);
        seed_appraisal(
            &cat,
            "r/ledger.md",
            "ev-bare",
            json!({"entry": "ledger:W-1", "verdict": "held"}),
        );
        // Present but empty is the same as absent — a blank instrument is not an
        // instrument, and whitespace must not buy a discharge either.
        seed_appraisal(
            &cat,
            "r/ledger.md",
            "ev-blank",
            json!({
                "entry": "ledger:W-1",
                "instrument": "   ",
                "observed": "",
                "verdict": "held",
            }),
        );

        let ctx = mk_ctx(root.to_path_buf(), cat);
        let v = call(&ctx, json!({"anchor_id": "ledger:W-1"}))
            .await
            .unwrap();

        let ver = &v["verification"];
        assert!(
            ver.is_object(),
            "a stamp with no evidence must leave the obligation standing: {v:#?}"
        );
        assert_eq!(ver["verification_state"], json!("unverified"), "{v:#?}");
        assert_eq!(
            ver["appraisals_recorded"],
            json!(0),
            "and it must not be COUNTED either — a reader who sees `appraisals_recorded: 2` \
             next to `unverified` concludes people looked and could not settle it, which is \
             the opposite of what two evidence-free stamps mean: {v:#?}"
        );
    }

    /// `refuted` is the highest-value output the mechanism produces and must never be
    /// recorded identically to a pass. It closes a validity interval and belongs on a
    /// worklist — so the tap stays armed, and `verification_state` says what the last look
    /// concluded rather than flattening it back to "unverified".
    #[tokio::test]
    async fn a_refuted_verdict_leaves_the_tap_armed_and_names_itself() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let cat = Catalog::open_in_memory().unwrap();
        anchor_ledger(&cat, root, "dated 2026-01-01");
        seed_citers(&cat, root, "ledger:W-1", ATTESTATION_EXPOSURE_THRESHOLD);
        seed_appraisal(
            &cat,
            "r/ledger.md",
            "ev-refuted",
            json!({
                "entry": "ledger:W-1",
                "instrument": "re-ran the sweep",
                "observed": "12, not 4",
                "verdict": "refuted",
            }),
        );

        let ctx = mk_ctx(root.to_path_buf(), cat);
        let v = call(&ctx, json!({"anchor_id": "ledger:W-1"}))
            .await
            .unwrap();

        let ver = &v["verification"];
        assert!(ver.is_object(), "a refutation does not discharge: {v:#?}");
        assert_eq!(
            ver["verification_state"],
            json!("refuted"),
            "the state must not read the same as one that was never looked at — a \
             refutation flattened back to `unverified` discards the highest-value output \
             this mechanism produces: {v:#?}"
        );
        assert_eq!(
            ver["appraisals_recorded"],
            json!(1),
            "someone looked, and the count says so even though nothing discharged: {v:#?}"
        );
        assert_eq!(
            ver["validity_class_declared"],
            json!("dated"),
            "the class is reported from the same parser the obligation was chosen from: \
             {v:#?}"
        );
    }

    /// Two appraisals, and the LATER one governs.
    ///
    /// Written from an observed surviving mutation, not from a coverage argument. Dropping
    /// the `if latest.is_none()` guard in `last_appraisal` — so the loop keeps overwriting
    /// and ends holding the OLDEST verdict — left all 30 context tests green on 2026-08-21.
    /// Every other fixture seeds at most one appraisal, so "most recent" was asserted
    /// nowhere but in a comment, and the walk-the-whole-timeline change that added
    /// `appraisals_recorded` is exactly what made the ordering reachable.
    ///
    /// The fixture is chosen so the mutation flips an OBSERVABLE, not just a string: older
    /// `held` would discharge and remove the block entirely, later `refuted` leaves it
    /// armed. Reading the wrong end of the timeline therefore silently marks a refuted
    /// Statement as verified — the laundering outcome `a_verdict_without_evidence_...`
    /// guards against from the other side.
    #[tokio::test]
    async fn the_most_recent_appraisal_governs_not_the_first_one_found() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let cat = Catalog::open_in_memory().unwrap();
        anchor_ledger(&cat, root, "dated 2026-01-01");
        seed_citers(&cat, root, "ledger:W-1", ATTESTATION_EXPOSURE_THRESHOLD);
        seed_appraisal_at(
            &cat,
            "r/ledger.md",
            "ev-old-held",
            100,
            json!({
                "entry": "ledger:W-1",
                "instrument": "re-ran the sweep",
                "observed": "4, as recorded",
                "verdict": "held",
            }),
        );
        seed_appraisal_at(
            &cat,
            "r/ledger.md",
            "ev-new-refuted",
            200,
            json!({
                "entry": "ledger:W-1",
                "instrument": "re-ran the sweep",
                "observed": "12, not 4",
                "verdict": "refuted",
            }),
        );

        let ctx = mk_ctx(root.to_path_buf(), cat);
        let v = call(&ctx, json!({"anchor_id": "ledger:W-1"}))
            .await
            .unwrap();

        let ver = &v["verification"];
        assert!(
            ver.is_object(),
            "the LATER appraisal refuted, so the tap stays armed — reading the earlier \
             `held` would discharge it and hand the reader a Statement that was refuted \
             the last time anyone looked: {v:#?}"
        );
        assert_eq!(ver["verification_state"], json!("refuted"), "{v:#?}");
        assert_eq!(
            ver["appraisals_recorded"],
            json!(2),
            "both carried evidence, so both count: {v:#?}"
        );
    }

    #[tokio::test]
    async fn a_colon_anchor_that_names_no_entry_falls_through_to_the_file_grain_path() {
        // Entry-ness is decided by RESOLUTION, not by shape, so an unresolvable
        // colon-bearing anchor must not be swallowed by the entry path — it has to reach
        // the file-grain code with its behaviour unchanged. `grain` is absent there, which
        // is the observable difference.
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let cat = Catalog::open_in_memory().unwrap();
        seed_ledger(&cat, root, "ledger.md", "ledger", "## W-1 — real\nbody\n");

        let ctx = mk_ctx(root.to_path_buf(), cat);

        for anchor in ["nosuchslug:W-1", "ledger:W-9", "ledger:"] {
            let v = call(&ctx, json!({"anchor_id": anchor})).await.unwrap();
            assert!(
                v["overflow"].get("grain").is_none(),
                "`{anchor}` names no Statement and must fall through: {v:#?}"
            );
        }
    }

    #[tokio::test]
    async fn a_nested_childs_declaration_is_not_read_as_the_anchors() {
        // `entry_sections` bounds a section at the next SAME-OR-HIGHER heading, so a
        // deeper child's text sits wholly inside its parent's. Parsing the parent's raw
        // `section.text` would let `parse_validity`'s first-wins rule report the CHILD's
        // `invariant` as the PARENT's — asserting a law nobody declared.
        //
        // `statement_node` calls `declared_section_text` first, exactly as all four
        // `doctor` scans do. This test exists because that copied discipline is otherwise
        // untested HERE: the tests pinning it are named for `doctor`'s call sites, so a
        // reviewer asking "is this consistent with its siblings?" gets a correct yes and
        // never reaches "and is the consistency pinned?".
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let cat = Catalog::open_in_memory().unwrap();

        seed_ledger(
            &cat,
            root,
            "ledger.md",
            "ledger",
            "## W-1 — parent that declares nothing of its own\n\
             parent body\n\
             \n\
             ### W-2 — nested child\n\
             **Valid:** invariant\n\
             child body\n",
        );
        seed_edge(&cat, "ledger", "W-1", "ledger:W-2");

        let ctx = mk_ctx(root.to_path_buf(), cat);
        let v = call(&ctx, json!({"anchor_id": "ledger:W-1"}))
            .await
            .unwrap();
        let md = v["markdown"].as_str().unwrap();

        let anchor_line = md
            .lines()
            .find(|l| l.contains("valid:") && md.find(l) < md.find("## ledger:W-2"))
            .unwrap_or_default();
        assert!(
            anchor_line.contains("undeclared (defaults to decay)"),
            "the parent declares nothing, so it must read as the decay default, never as \
             its child's `invariant`. Got: {anchor_line:?}\n{md}"
        );
        assert!(
            md.contains("## ledger:W-2 · cites"),
            "and the child is still packed, with its own class: {md}"
        );
    }

    #[tokio::test]
    async fn an_oversized_entry_anchor_does_not_starve_its_neighbours() {
        // The entry-grain twin of `anchor_neighbors_are_not_starved_by_oversized_anchor`.
        // It exists because the half-budget reserve was COPIED from the file-grain path,
        // and a copy inherits the sibling's discipline but not its tests — the sibling's
        // test is named for the sibling. Disabling the reserve here left all 19 context
        // tests green (confirmed by applying the mutation), which is the hole this closes.
        //
        // Ledgers are exactly where this bites: an entry can run for hundreds of lines
        // while the Statements that rest on it are short, so the anchor is routinely the
        // largest node in its own pack.
        // docs/issues/archive/2026-07-05-context-anchor-starves-neighbors.md
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let cat = Catalog::open_in_memory().unwrap();

        // One giant line, so no line-based preview cap can shrink it instead.
        seed_ledger(
            &cat,
            root,
            "ledger.md",
            "ledger",
            &format!("## W-1 — huge anchor\n{}\n", "x".repeat(2000)),
        );
        seed_ledger(
            &cat,
            root,
            "other.md",
            "other",
            "## F-1 — small\na\n\n## F-2 — small\nb\n\n## F-3 — small\nc\n",
        );
        for t in ["F-1", "F-2", "F-3"] {
            seed_edge(&cat, "ledger", "W-1", &format!("other:{t}"));
        }

        let ctx = mk_ctx(root.to_path_buf(), cat);
        let v = call(&ctx, json!({"anchor_id": "ledger:W-1", "max_tokens": 400}))
            .await
            .unwrap();

        let ids: Vec<String> = v["included_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            ids.len(),
            4,
            "anchor + all three neighbours must fit; without the reserve the anchor eats \
             the whole budget and every neighbour is dropped: {v:#?}"
        );

        let md = v["markdown"].as_str().unwrap();
        assert!(
            md.contains("… [anchor truncated — reserved half the budget"),
            "and the truncation must be VISIBLE — a silently shortened Statement is \
             indistinguishable from a short one, which is the lie this whole layer \
             exists to avoid: {md}"
        );
        assert_eq!(
            v["overflow"]["packing"],
            json!("whole"),
            "the neighbours themselves fit whole here; only the ANCHOR was capped, and the \
             two are different mechanisms: {v:#?}"
        );
    }

    #[tokio::test]
    async fn a_mutual_neighbour_is_packed_once_and_labelled_mutual() {
        // An entry that cites the anchor AND is cited by it is ONE node. Keying the dedup
        // on (reference, direction) — which is what this did first — packs its whole
        // section twice. Measured 2026-08-21: 182 bidirectional rows corpus-wide, and for
        // `reconnaissance-patterns:R-3` the repeat was ~4.8KB inside a 16KB budget.
        //
        // `mutual` is also the more informative label. A reciprocal citation is the
        // strongest tie in the graph: both authors independently asserted the relationship.
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let cat = Catalog::open_in_memory().unwrap();

        seed_ledger(&cat, root, "ledger.md", "ledger", "## W-1 — anchor\nbody\n");
        seed_ledger(
            &cat,
            root,
            "other.md",
            "other",
            "## F-1 — the mutual peer\nbody\n",
        );
        seed_edge(&cat, "ledger", "W-1", "other:F-1"); // anchor -> peer
        seed_edge(&cat, "other", "F-1", "ledger:W-1"); // peer -> anchor

        let ctx = mk_ctx(root.to_path_buf(), cat);
        let v = call(&ctx, json!({"anchor_id": "ledger:W-1"}))
            .await
            .unwrap();

        let ids: Vec<String> = v["included_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            ids,
            vec!["ledger:W-1", "other:F-1"],
            "the peer appears ONCE, not once per direction: {v:#?}"
        );
        assert_eq!(
            v["overflow"]["candidates"],
            json!(2),
            "and it is counted once, so `omitted` cannot be computed against a phantom: \
             {v:#?}"
        );

        let md = v["markdown"].as_str().unwrap();
        assert!(
            md.contains("## other:F-1 · mutual"),
            "reciprocity is reported, not collapsed to one arbitrary direction: {md}"
        );
        assert!(
            !md.contains("· cites") && !md.contains("· cited-by"),
            "and neither one-way label survives for a mutual pair: {md}"
        );
    }

    #[tokio::test]
    async fn mutual_neighbours_survive_the_budget_before_one_way_ones() {
        // Ordering only decides anything for anchors that overflow — which is why CTX-1
        // deferred it — but there it decides everything, and plain alphabetical `direction`
        // sorts `cited-by` < `cites` < `mutual`, burying the strongest class LAST.
        //
        // Measured on the first live call after shipping: `reconnaissance-patterns:R-3`
        // served 11 one-way citations and dropped BOTH its mutual partners (`R-77`, `R-79`)
        // — its own documented chain. The deferral's revisit-when fired immediately.
        //
        // Note what is NOT ranked here: `cites` against `cited-by`. They answer different
        // questions (proof route vs blast radius); that choice is the caller's.
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let cat = Catalog::open_in_memory().unwrap();

        seed_ledger(&cat, root, "ledger.md", "ledger", "## W-1 — anchor\nbody\n");
        // Eight fat one-way citers, alphabetically ahead of the mutual peer's ledger, plus
        // one mutual peer. Only a few fit; the mutual one must be among them.
        let mut aaa = String::new();
        for i in 1..=8 {
            aaa.push_str(&format!(
                "## F-{i} — one-way citer {i}\n{}\n\n",
                "z".repeat(3000)
            ));
        }
        seed_ledger(&cat, root, "aaa.md", "aaa", &aaa);
        seed_ledger(
            &cat,
            root,
            "zzz.md",
            "zzz",
            "## M-1 — the mutual peer\nbody\n",
        );

        for i in 1..=8 {
            seed_edge(&cat, "aaa", &format!("F-{i}"), "ledger:W-1");
        }
        seed_edge(&cat, "ledger", "W-1", "zzz:M-1");
        seed_edge(&cat, "zzz", "M-1", "ledger:W-1");

        let ctx = mk_ctx(root.to_path_buf(), cat);
        let v = call(&ctx, json!({"anchor_id": "ledger:W-1", "max_tokens": 900}))
            .await
            .unwrap();

        assert!(
            v["overflow"]["omitted"].as_u64().unwrap() > 0,
            "the fixture must actually overflow, or this pins nothing: {v:#?}"
        );
        let ids: Vec<String> = v["included_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect();
        assert!(
            ids.contains(&"zzz:M-1".to_string()),
            "the mutual peer must survive the cut despite sorting last alphabetically by \
             BOTH direction and reference: {ids:?}"
        );
        assert_eq!(
            ids[1], "zzz:M-1",
            "and it must come first among neighbours, not merely survive: {ids:?}"
        );
    }

    #[tokio::test]
    async fn a_neighbourhood_that_does_not_fit_whole_is_excerpted_rather_than_dropped() {
        // The two-pass policy, and the measurement behind it (2026-08-21, 931 anchors):
        // 76% of neighbourhoods fit WHOLE at the default budget, so excerpting always would
        // degrade three anchors in four for nothing. The other 24% overflow badly, and a
        // 1000-byte excerpt takes the fully-served share to 98%.
        //
        // Before the two-pass, the overflow half simply lost neighbours off the end of the
        // budget — `reconnaissance-patterns:R-3` served 2 of 24, and which 2 was decided by
        // ledger spelling.
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let cat = Catalog::open_in_memory().unwrap();

        seed_ledger(&cat, root, "ledger.md", "ledger", "## W-1 — anchor\nbody\n");
        // Six neighbours of ~3KB each: 18KB of neighbourhood against a 16KB budget.
        let mut other = String::new();
        for i in 1..=6 {
            other.push_str(&format!(
                "## F-{i} — neighbour {i}\n{}\n\n",
                "y".repeat(3000)
            ));
        }
        seed_ledger(&cat, root, "other.md", "other", &other);
        for i in 1..=6 {
            seed_edge(&cat, "ledger", "W-1", &format!("other:F-{i}"));
        }

        let ctx = mk_ctx(root.to_path_buf(), cat);
        let v = call(&ctx, json!({"anchor_id": "ledger:W-1"}))
            .await
            .unwrap();

        assert_eq!(
            v["overflow"]["packing"],
            json!("excerpted"),
            "the mode must be REPORTED — a reader cannot tell a whole section from a \
             1000-byte excerpt that happened to end at a paragraph, and 'is this the entry \
             or the top of it' changes what they do next: {v:#?}"
        );
        assert_eq!(
            v["overflow"]["omitted"],
            json!(0),
            "excerpting is what buys completeness: all six neighbours are present, where \
             full-text packing would have dropped most of them: {v:#?}"
        );

        let md = v["markdown"].as_str().unwrap();
        assert!(
            md.contains("… [excerpted — the neighbourhood exceeds the budget"),
            "and every shortened neighbour says so: {md}"
        );
        for i in 1..=6 {
            assert!(
                md.contains(&format!("## other:F-{i} ·")),
                "F-{i} missing: {md}"
            );
        }
    }

    /// C1 of docs/issues/archive/2026-08-15-context-and-state-at-never-dedup-the-worktree-overlay.md.
    ///
    /// `apply_scope` ORs the worktree prefix with the main prefix for a worktree
    /// session, so both twins are in the candidate pool by construction. `find`
    /// drops the main twin; `context` used to render both — two `## <title>`
    /// sections for one document, each charged against the same token budget.
    #[tokio::test]
    async fn a_worktree_session_drops_the_main_twin_its_shadow_supersedes() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let cat = Catalog::open_in_memory().unwrap();

        let main_dir = root.join("codescout");
        let wt_dir = main_dir.join(".worktrees/feat");
        std::fs::create_dir_all(&main_dir).unwrap();
        std::fs::create_dir_all(&wt_dir).unwrap();
        std::fs::write(main_dir.join("auth.md"), "# auth\nmain body").unwrap();
        std::fs::write(wt_dir.join("auth.md"), "# auth\nshadow body").unwrap();

        let mut main_row = sample_row("main", "x", "auth.md", "auth notes", Some("auth"));
        main_row.abs_path = main_dir.join("auth.md");
        let mut shadow_row = sample_row("shadow", "x", "auth.md", "auth notes", Some("auth"));
        shadow_row.abs_path = wt_dir.join("auth.md");
        artifact::upsert(&cat, &main_row).unwrap();
        artifact::upsert(&cat, &shadow_row).unwrap();

        // The lineage edge is what pairs them; without it there is no way to
        // know the two rows are one document.
        links::insert(
            &cat,
            &links::LinkRow {
                src_id: "shadow".into(),
                dst_id: "main".into(),
                rel: crate::librarian::tools::worktree::LINEAGE_REL.into(),
                created_at: 0,
            },
        )
        .unwrap();

        let ctx = TestToolContextBuilder::new(cat)
            .with_root(Root {
                name: "x".into(),
                path: root.clone(),
            })
            .with_current_project(Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: wt_dir.clone(),
                    git_root: wt_dir.clone(),
                    main_root: Some(main_dir.clone()),
                    umbrella: None,
                },
            ))
            .build();

        let v = call(&ctx, json!({"topic": "auth"})).await.unwrap();
        let ids: Vec<String> = v["included_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect();

        assert_eq!(
            ids,
            vec!["shadow".to_string()],
            "the shadow supersedes its main twin; rendering both packs one \
             document into the bundle twice, unlabelled, got {ids:?}"
        );
    }

    /// C2 of the same bug — the half its report never reached.
    ///
    /// `exclude_worktrees` is computed for EVERY session, not only worktree
    /// ones: an in-repo layout (`<main>/.worktrees/<n>`) puts a foreign
    /// session's shadow rows underneath the main checkout's own path prefix,
    /// so a plain main-checkout query matches them unless they are excluded.
    /// This needs no worktree session at all, which is why it is the more
    /// reachable of the two.
    #[tokio::test]
    async fn a_main_checkout_never_pulls_in_another_worktrees_shadow() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let cat = Catalog::open_in_memory().unwrap();

        let main_dir = root.join("codescout");
        let wt_dir = main_dir.join(".worktrees/feat");
        std::fs::create_dir_all(&main_dir).unwrap();
        std::fs::create_dir_all(&wt_dir).unwrap();
        std::fs::write(main_dir.join("auth.md"), "# auth\nmain body").unwrap();
        std::fs::write(wt_dir.join("other.md"), "# auth\nforeign shadow").unwrap();

        let mut mine = sample_row("mine", "x", "auth.md", "auth notes", Some("auth"));
        mine.abs_path = main_dir.join("auth.md");
        let mut foreign = sample_row("foreign", "x", "other.md", "auth elsewhere", Some("auth"));
        foreign.abs_path = wt_dir.join("other.md");
        artifact::upsert(&cat, &mine).unwrap();
        artifact::upsert(&cat, &foreign).unwrap();

        crate::librarian::catalog::worktree::upsert_active(
            &cat,
            &crate::util::fs::RepoPath::from(wt_dir.as_path()).into_string(),
            &crate::util::fs::RepoPath::from(main_dir.as_path()).into_string(),
            None,
            1,
        )
        .unwrap();

        let ctx = TestToolContextBuilder::new(cat)
            .with_root(Root {
                name: "x".into(),
                path: root.clone(),
            })
            .with_current_project(Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: main_dir.clone(),
                    git_root: main_dir.clone(),
                    // Not a worktree session — this is the plain main checkout.
                    main_root: None,
                    umbrella: None,
                },
            ))
            .build();

        let v = call(&ctx, json!({"topic": "auth"})).await.unwrap();
        let ids: Vec<String> = v["included_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect();

        assert_eq!(
            ids,
            vec!["mine".to_string()],
            "a registered worktree's rows belong to that session's overlay; the \
             main checkout must not see them, got {ids:?}"
        );
    }

    #[tokio::test]
    async fn no_args_with_no_active_goals_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf(), cat);

        let v = call(&ctx, json!({})).await.unwrap();

        assert_eq!(v["markdown"].as_str().unwrap(), "");
        assert_eq!(v["included_ids"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn no_args_returns_active_goals_header() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create the real .md file the no-anchor branch will read.
        let goal_dir = root.join("docs/trackers");
        std::fs::create_dir_all(&goal_dir).unwrap();
        std::fs::write(goal_dir.join("goal-a.md"), "# Ship Feature X\nsome body\n").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        let mut goal_row = sample_row(
            "r/docs/trackers/goal-a.md",
            "r",
            "docs/trackers/goal-a.md",
            "Ship Feature X",
            None,
        );
        goal_row.kind = "tracker".into();
        goal_row.tags = vec!["goal".into()];
        artifact::upsert(&cat, &goal_row).unwrap();

        let ctx = mk_ctx(root.to_path_buf(), cat);

        let v = call(&ctx, json!({})).await.unwrap();

        let md = v["markdown"].as_str().unwrap();
        assert!(
            md.contains("## Active goals"),
            "expected '## Active goals' header in markdown; got: {md}"
        );
        assert!(
            md.contains("Ship Feature X"),
            "expected goal title in active-goals section; got: {md}"
        );
    }

    #[tokio::test]
    async fn default_scope_is_project_and_excludes_other_repos() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let cat = Catalog::open_in_memory().unwrap();

        // Active project lives at root/codescout with file inside.
        let proj_dir = root.join("codescout");
        std::fs::create_dir_all(&proj_dir).unwrap();
        std::fs::write(proj_dir.join("auth.md"), "# auth\nbody").unwrap();

        let mut in_proj = sample_row(
            "in",
            "claude",
            "codescout/auth.md",
            "auth notes",
            Some("auth"),
        );
        in_proj.abs_path = proj_dir.join("auth.md");
        let mut out_proj = sample_row("out", "agents", "x/auth.md", "auth elsewhere", Some("auth"));
        // Place the other repo's row outside the active project AND outside the
        // git_root, so it is excluded under either scope — what this test pins is
        // the DEFAULT, and a fixture that only one scope excluded could not tell
        // a correct default from a wrong one.
        let other_root = std::path::PathBuf::from("/some/other/repo");
        out_proj.abs_path = other_root.join("x/auth.md");
        artifact::upsert(&cat, &in_proj).unwrap();
        artifact::upsert(&cat, &out_proj).unwrap();

        let ctx = TestToolContextBuilder::new(cat)
            .with_root(Root {
                name: "claude".into(),
                path: root.clone(),
            })
            .with_current_project(Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: proj_dir.clone(),
                    git_root: root.clone(),
                    main_root: None,
                    umbrella: None,
                },
            ))
            .build();

        let v = call(&ctx, json!({"topic": "auth"})).await.unwrap();
        let included = v["included_ids"].as_array().unwrap();
        assert_eq!(included.len(), 1);
        assert_eq!(included[0], "in");
        assert_eq!(v["scope"]["applied"], "project");
    }

    #[tokio::test]
    async fn scope_all_stays_literal_and_reaches_outside_the_umbrella() {
        // Pins the behaviour `librarian(action="context")` deliberately has and
        // `doc(action="find")` deliberately does not: an explicit `scope="all"`
        // is NOT aliased to `umbrella`, so orientation can reach a project the
        // umbrella does not contain. Confirmed by a live A/B against the running
        // server, then ruled intentional by the owner — see
        // docs/issues/archive/2026-08-15-context-scope-all-crosses-umbrella-boundary.md.
        //
        // Before `UmbrellaPolicy` this behaviour was defended by nothing: it existed
        // only as the ABSENCE of a block two sibling handlers carried, which is why
        // it was reported as a bug before it was recognised as a choice.
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let cat = Catalog::open_in_memory().unwrap();

        let proj_dir = root.join("codescout");
        std::fs::create_dir_all(&proj_dir).unwrap();
        std::fs::write(proj_dir.join("auth.md"), "# auth\nbody").unwrap();
        let outside_dir = root.join("unrelated");
        std::fs::create_dir_all(&outside_dir).unwrap();
        std::fs::write(outside_dir.join("auth.md"), "# auth\nbody").unwrap();

        let mut inside = sample_row(
            "in",
            "claude",
            "codescout/auth.md",
            "auth notes",
            Some("auth"),
        );
        inside.abs_path = proj_dir.join("auth.md");
        let mut outside = sample_row(
            "out",
            "claude",
            "unrelated/auth.md",
            "auth elsewhere",
            Some("auth"),
        );
        outside.abs_path = outside_dir.join("auth.md");
        artifact::upsert(&cat, &inside).unwrap();
        artifact::upsert(&cat, &outside).unwrap();

        let ctx = TestToolContextBuilder::new(cat)
            .with_root(Root {
                name: "claude".into(),
                path: root.clone(),
            })
            // The umbrella contains the active project ONLY, so `out` is a
            // non-member — precisely the row `find` would narrow away.
            .with_umbrellas(vec![crate::librarian::workspace::Umbrella {
                name: "main".into(),
                members: vec![proj_dir.clone()],
            }])
            .with_current_project(Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: proj_dir.clone(),
                    git_root: proj_dir.clone(),
                    main_root: None,
                    umbrella: Some("main".into()),
                },
            ))
            .build();

        let v = call(&ctx, json!({"topic": "auth", "scope": "all"}))
            .await
            .unwrap();
        assert_eq!(
            v["scope"]["applied"], "all",
            "an explicit scope=all must NOT be aliased to umbrella on context"
        );
        let included: Vec<&str> = v["included_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap())
            .collect();
        assert!(
            included.contains(&"out"),
            "context must reach the non-member project under scope=all; got {included:?}"
        );
    }

    #[tokio::test]
    async fn live_header_present_for_augmented_artifact() {
        use crate::librarian::catalog::augmentation::{self, AugmentationRow};
        use std::io::Write;

        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();

        // Write the artifact file to disk.
        let mut f = std::fs::File::create(root.join("tracker.md")).unwrap();
        writeln!(f, "# My Tracker\n\nsome content").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        let mut row = sample_row(
            "ctx-aug",
            "r",
            "tracker.md",
            "My Tracker",
            Some("live-test"),
        );
        row.kind = "tracker".into();
        artifact::upsert(&cat, &row).unwrap();
        augmentation::upsert(
            &cat,
            &AugmentationRow {
                artifact_id: "ctx-aug".to_string(),
                prompt: "Maintain state".to_string(),
                params: "{}".to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: None,
                refreshed_at_commit: None,
            },
        )
        .unwrap();

        let ctx = mk_ctx(root, cat);
        let result = call(&ctx, json!({"topic": "live-test"})).await.unwrap();

        let md = result["markdown"].as_str().unwrap();
        assert!(md.contains("[LIVE]"), "expected [LIVE] in:\n{md}");
        assert!(md.contains("Maintain state"), "expected prompt in:\n{md}");
    }

    #[tokio::test]
    async fn render_template_projects_params_into_context() {
        use crate::librarian::catalog::augmentation::{self, AugmentationRow};
        use std::io::Write;

        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();

        let mut f = std::fs::File::create(root.join("tracker.md")).unwrap();
        writeln!(f, "# Eval Tracker\n\nProse-only body.").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        let mut row = sample_row(
            "ctx-tmpl",
            "r",
            "tracker.md",
            "Eval Tracker",
            Some("render-test"),
        );
        row.kind = "tracker".into();
        artifact::upsert(&cat, &row).unwrap();

        let template = "**Status:** {{ status }} ({{ failures|length }} failing)";
        let params = r#"{"status":"red","failures":[{"id":"F-1"},{"id":"F-2"}]}"#;
        augmentation::upsert(
            &cat,
            &AugmentationRow {
                artifact_id: "ctx-tmpl".to_string(),
                prompt: "Maintain F-N table".to_string(),
                params: params.to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: Some(template.to_string()),
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: None,
                refreshed_at_commit: None,
            },
        )
        .unwrap();

        let ctx = mk_ctx(root, cat);
        let result = call(&ctx, json!({"topic": "render-test"})).await.unwrap();

        let md = result["markdown"].as_str().unwrap();
        assert!(md.contains("[LIVE]"), "expected [LIVE] in:\n{md}");
        assert!(
            md.contains("**Status:** red (2 failing)"),
            "expected rendered template line in:\n{md}"
        );
    }

    #[tokio::test]
    async fn render_template_error_surfaces_in_context() {
        use crate::librarian::catalog::augmentation::{self, AugmentationRow};
        use std::io::Write;

        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let mut f = std::fs::File::create(root.join("t.md")).unwrap();
        writeln!(f, "# T\n\nbody").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        let mut row = sample_row("ctx-bad", "r", "t.md", "T", Some("bad-tmpl"));
        row.kind = "tracker".into();
        artifact::upsert(&cat, &row).unwrap();

        // Intentionally malformed template
        augmentation::upsert(
            &cat,
            &AugmentationRow {
                artifact_id: "ctx-bad".to_string(),
                prompt: "p".to_string(),
                params: "{}".to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: Some("{% for x in %}".to_string()),
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: None,
                refreshed_at_commit: None,
            },
        )
        .unwrap();

        let ctx = mk_ctx(root, cat);
        let result = call(&ctx, json!({"topic": "bad-tmpl"})).await.unwrap();

        let md = result["markdown"].as_str().unwrap();
        assert!(
            md.contains("render_template error"),
            "expected error comment in:\n{md}"
        );
    }

    #[tokio::test]
    async fn augmented_artifacts_sorted_before_plain() {
        use crate::librarian::catalog::augmentation::{self, AugmentationRow};

        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();

        // Write files for both artifacts.
        std::fs::write(root.join("plain.md"), "# Plain\nbody").unwrap();
        std::fs::write(root.join("aug.md"), "# Augmented\nbody").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        // Insert plain first so it would appear first without sorting.
        artifact::upsert(
            &cat,
            &sample_row("plain", "r", "plain.md", "Plain", Some("sort-test")),
        )
        .unwrap();
        artifact::upsert(
            &cat,
            &sample_row("aug", "r", "aug.md", "Augmented", Some("sort-test")),
        )
        .unwrap();
        augmentation::upsert(
            &cat,
            &AugmentationRow {
                artifact_id: "aug".to_string(),
                prompt: "keep fresh".to_string(),
                params: "{}".to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: None,
                refreshed_at_commit: None,
            },
        )
        .unwrap();

        let ctx = mk_ctx(root, cat);
        let result = call(&ctx, json!({"topic": "sort-test"})).await.unwrap();

        let included = result["included_ids"].as_array().unwrap();
        assert_eq!(included.len(), 2);
        // Augmented artifact should appear before plain.
        assert_eq!(included[0], "aug");
        assert_eq!(included[1], "plain");
    }
}
