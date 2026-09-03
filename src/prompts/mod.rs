//! Prompt templates for LLM guidance.
//!
//! Templates are stored as markdown files and compiled into the binary
//! via `include_str!`. Dynamic sections are appended at runtime based
//! on project state.

pub mod builders;
pub mod guide_index;
pub mod source;

/// Static server instructions — tool reference, workflow patterns, steering rules.
pub const SERVER_INSTRUCTIONS: &str =
    include_str!(concat!(env!("OUT_DIR"), "/server_instructions.md"));

/// The MEASURED client limit for MCP `initialize.instructions`, in **characters**.
///
/// Claude Code cuts the field at exactly 2048 chars. Measured 2026-08-16 by locating a
/// live session's own truncation point inside the rendered slice: the delivered prefix
/// ended mid-token at `- "symbol-navigatio`, which is byte 2092 and **char 2048** of
/// `build_server_instructions(None)`. 2048 is 2^11, not a coincidence.
///
/// The unit matters and was previously wrong. The old constant was named
/// `MAX_INSTRUCTIONS_CHARS` and compared against `String::len()`, which counts **bytes**
/// — and this surface is dense with em-dashes and arrows, so the same slice measured
/// 2127 bytes and 2081 chars. A byte budget over-counts the surface *and* the old value
/// (2200) sat above the real cliff, so the gate was wrong twice over while staying green.
///
/// See `docs/issues/archive/2026-08-15-server-instructions-truncated-before-reaching-the-model.md`.
pub(crate) const CLIENT_INSTRUCTIONS_CHAR_LIMIT: usize = 2048;

/// Characters held back from the measured cliff. The limit was observed on one client
/// build; another may cut a little lower, and a surface that arrives whole everywhere is
/// worth 48 characters.
pub(crate) const CHANNEL_SAFETY_MARGIN: usize = 48;

/// Build the full server instructions string, optionally appending dynamic project
/// status — **guaranteeing** the result fits the client channel.
///
/// The static slice is never sacrificed. Whatever does not fit is taken from the tail of
/// the dynamic block, at a line boundary, with the loss named. That inverts the previous
/// behaviour, where the client cut a fixed char count mid-token with no signal at all and
/// the `get_guide` pointer list — the mechanism by which a model discovers deeper
/// guidance exists — was exactly what fell off the end.
pub fn build_server_instructions(project_status: Option<&ProjectStatus>) -> String {
    let mut instructions = SERVER_INSTRUCTIONS.to_string();

    if let Some(status) = project_status {
        // Persistent tiers only. The `Substitutable` segments now ride the first tool
        // response (`build_status_response_block`), which has no character ceiling —
        // see `persistent_status_segments` for why the split is by what-breaks-if-lost
        // rather than by size.
        let segments = persistent_status_segments(status);
        instructions.push_str(&fit_dynamic_block(&instructions, &segments));
    }

    instructions
}

/// Drop order for one `## Project Status` segment when the channel cannot carry it all.
///
/// Derived from the bug's own Workarounds list rather than invented: a segment another
/// surface reproduces on demand is cheap to lose, and one only this channel delivers is
/// not. `Ord` follows declaration order, so sorting ascending drops `Substitutable` first.
///
/// BL-37: the previous fitting cut from the tail, and the tail is where the user's own
/// text lives — so `## Custom Instructions` went first and the memories list, which
/// `memory(action="list")` reproduces, went last. Measured on a three-language project
/// with eight memories: the memories line is ~137 chars and did not fit the ~225 of room,
/// while the custom-instructions line is ~70 and would have. Ordering here is not only a
/// better choice of loss; it delivers content that was being dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum StatusPriority {
    /// Reachable on demand elsewhere: `memory(action="list")` for the memory names,
    /// `get_guide("workspace-state")` for the topology, `index(action="status")` for the
    /// index, memory `gotchas` for the Kotlin block.
    Substitutable,
    /// The user wrote it and nothing else surfaces it to the agent.
    UserAuthored,
    /// Never dropped. Losing the worktree banner sends commits to the wrong branch;
    /// losing the header or the active-project line leaves everything else unattributed.
    Anchor,
}

/// One line-or-block of the `## Project Status` suffix, with what it costs to lose.
///
/// `label` names the segment in the trim note. Naming the loss is the point — an agent
/// told *what* went can ask for it, where "something was trimmed" only tells it to
/// distrust the whole block.
struct StatusSegment {
    text: String,
    label: &'static str,
    priority: StatusPriority,
}

/// Render the dynamic suffix as droppable segments. See [`build_project_status_block`]
/// for the concatenated form, which is what the renderer's own tests assert on.
fn build_project_status_segments(status: &ProjectStatus) -> Vec<StatusSegment> {
    let mut segs: Vec<StatusSegment> = Vec::new();

    segs.push(StatusSegment {
        text: String::from("\n\n## Project Status\n\n"),
        label: "header",
        priority: StatusPriority::Anchor,
    });

    // "Active project" wording makes the implicit launch-time activation explicit —
    // agents see at a glance that activation happened without needing a separate tool
    // call signal. Pairs with the worktree line below so the activated root is never
    // ambiguous.
    segs.push(StatusSegment {
        text: format!(
            "- **Active project:** {} at `{}`\n",
            status.name, status.path
        ),
        label: "active project",
        priority: StatusPriority::Anchor,
    });

    if let Some(wt) = &status.worktree {
        let branch = wt.branch.as_deref().unwrap_or("<detached HEAD>");
        let main = wt
            .main_repo
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<unknown>".to_string());
        // Explicit worktree banner — when present, the agent must NOT assume the
        // activated root is the canonical checkout. Changes here flow into commits,
        // branches, and PRs on the worktree's branch, not the main repo's. Anchor for
        // that reason: it is the one segment whose absence causes a wrong WRITE.
        segs.push(StatusSegment {
            text: format!("- **Worktree:** branch `{branch}` of `{main}`\n"),
            label: "worktree",
            priority: StatusPriority::Anchor,
        });
    }

    if !status.languages.is_empty() {
        segs.push(StatusSegment {
            text: format!("- **Languages:** {}\n", status.languages.join(", ")),
            label: "languages",
            priority: StatusPriority::Substitutable,
        });
    }

    let memories = if !status.memories.is_empty() {
        // Bare list, ALL of it — the action verb is documented on the `memory` tool itself.
        //
        // This used to cap at 8 names and append "+N more". That cap's stated reason was
        // *"unbounded is exactly what a fixed channel cannot carry"* — true when written,
        // and falsified by the tier split that moved this segment to the tool response,
        // which has no character ceiling. A cap whose rationale has been removed is just
        // 14 hidden memory names: precisely the pointers an agent needs to decide what to
        // read, withheld to protect a budget this segment no longer spends.
        //
        // Nothing here bounds it now, and nothing needs to: `fit_dynamic_block` still
        // guarantees the persistent channel never overflows, so even re-tiering this
        // segment back to a persistent tier would degrade to a trim rather than a
        // truncation.
        format!("- **Memories:** {}\n", status.memories.join(", "))
    } else {
        "- **Memories:** None yet — run `onboarding` to create project memories\n".to_string()
    };
    segs.push(StatusSegment {
        text: memories,
        label: "memories",
        priority: StatusPriority::Substitutable,
    });

    segs.push(StatusSegment {
        text: if status.has_index {
            "- **Semantic index:** Built — `semantic_search` is ready to use\n".to_string()
        } else {
            "- **Semantic index:** Not built — run `index(action='build')` to enable `semantic_search`\n"
                .to_string()
        },
        label: "index status",
        priority: StatusPriority::Substitutable,
    });

    // Workspace topology — inject project table when there are sibling projects.
    if let Some(projects) = &status.workspace {
        if !projects.is_empty() {
            let mut table = String::from("\n## Workspace Projects\n\n");
            table.push_str("| Project | Root | Languages | Depends On |\n");
            table.push_str("|---------|------|-----------|------------|\n");
            for p in projects {
                let langs = if p.languages.is_empty() {
                    "—".to_string()
                } else {
                    p.languages.join(", ")
                };
                let deps = if p.depends_on.is_empty() {
                    "—".to_string()
                } else {
                    p.depends_on.join(", ")
                };
                table.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    p.id, p.root, langs, deps
                ));
            }
            table.push_str(
                "\nUse `project_id: \"<id>\"` in `semantic_search` / `memory` to scope to a specific project, or pass an absolute workspace path on a single call to pin it. `symbols` has no project param — scope it with `path` (its `scope` is a different axis: project/libraries/all).\n",
            );
            segs.push(StatusSegment {
                text: table,
                label: "workspace table",
                priority: StatusPriority::Substitutable,
            });
        }
    }

    // NO LANGUAGE-SPECIFIC WARNINGS. `KOTLIN_KNOWN_ISSUES` used to be injected here for any
    // project whose `languages` contained "kotlin", and it is removed rather than narrowed.
    //
    // The trigger was wrong: `languages` is what the repo CONTAINS, not what it is written
    // in. codescout is a Rust project with Kotlin fixtures, so it served itself ~600
    // characters about a JetBrains LSP limitation on every single session — and while this
    // segment lived in the 2048-char instructions channel, it was the largest droppable
    // thing in it, so it was also the first casualty of every trim. It cost the most and
    // was delivered least.
    //
    // But narrowing the trigger to Kotlin-only projects would still have been wrong,
    // because the block buys nothing at any trigger. `detect_fatal_stderr`
    // (`src/lsp/client.rs`) already returns a `RecoverableError` naming the exact condition
    // AND what to do — "Another codescout instance or editor is already serving this
    // project with kotlin-lsp ... Stop the other session and retry." That is more
    // actionable than the block, arrives only when it is true, and is pinned by
    // `detect_fatal_stderr_flags_kotlin_multi_session`. The block's own last line conceded
    // the point: "codescout detects this and fails fast with a clear error."
    //
    // Pre-loading an explanation of a self-announcing error is the anti-pattern. If a new
    // language issue is genuinely NOT self-announcing, it belongs in memory `gotchas` and
    // `get_guide` — pull channels, read when the topic is live — not pushed at every
    // session that happens to contain one file of that language.

    if let Some(prompt) = &status.system_prompt {
        segs.push(StatusSegment {
            text: format!("\n\n## Custom Instructions\n\n{prompt}\n"),
            label: "custom instructions",
            priority: StatusPriority::UserAuthored,
        });
    }

    segs
}

/// The segments that earn a place in the **persistent** channel.
///
/// `server_instructions` rides the MCP `initialize.instructions` field, which the client
/// puts in the system prompt — re-sent on every request, so it survives compaction and
/// context eviction. That persistence is the scarce thing here, not the bytes: the whole
/// channel is 2048 characters and there is no second push-and-persist surface, so what
/// stays is decided by *what breaks if it is lost mid-session*, not by what is useful.
///
/// `Anchor` stays because its absence causes a wrong WRITE — an agent that compacts and
/// then commits must still know it is on a worktree branch, and a response-carried banner
/// cannot help there because the first tool call can itself be the write.
///
/// `UserAuthored` stays because it is the user's own project rules and nothing else
/// surfaces them; losing those to a compaction is worse than losing a memories list. It
/// can still overflow to the response channel when it does not fit, which is strictly
/// better than today, where it is dropped outright.
fn persistent_status_segments(status: &ProjectStatus) -> Vec<StatusSegment> {
    build_project_status_segments(status)
        .into_iter()
        .filter(|s| s.priority != StatusPriority::Substitutable)
        .collect()
}

/// The `Substitutable` segments, rendered for the tool-response channel.
///
/// Returns `None` when there is nothing to say. These are the segments whose own tier
/// definition is *"reachable on demand elsewhere"*, which is exactly the property that
/// makes them safe to deliver in conversation content rather than in the system prompt:
/// if a compaction eats them, the cost is a `memory(action="list")` call, not a wrong
/// write. They are also the segments that today are dropped FIRST and therefore often
/// never arrive at all — on a Kotlin project with custom instructions, several never fit.
/// Ephemeral-and-complete beats absent.
///
/// No length cap and no fitting. That is the point of moving them: the response channel
/// has no 2048-character ceiling, so the memory list arrives in full — the 8-name cap that
/// used to truncate it was deleted once this move falsified its rationale — and the
/// workspace table arrives whole.
pub fn build_status_response_block(status: &ProjectStatus) -> Option<String> {
    let body: String = build_project_status_segments(status)
        .iter()
        .filter(|s| s.priority == StatusPriority::Substitutable)
        .map(|s| s.text.as_str())
        .collect();
    if body.trim().is_empty() {
        return None;
    }
    // Its own heading rather than a second `## Project Status`: two blocks under one
    // title read as a contradiction when they disagree, and they will disagree — this
    // one is re-rendered per emission while the persistent half is fixed at activation.
    Some(format!("\n## Project Status (details)\n{body}"))
}

/// Render the dynamic `## Project Status` suffix. Split out from
/// [`build_server_instructions`] so its length can be measured and trimmed before it is
/// appended, rather than discovered to be too long by a client that says nothing.
///
/// The concatenation of [`build_project_status_segments`] in display order — i.e. the
/// whole block, before any fitting. The renderer's own tests assert on this; what the
/// channel actually delivers is [`fit_dynamic_block`]'s output.
///
/// Test-only since BL-37: production renders from segments so the fitting can drop them
/// individually. It stays because three tests assert the RENDERER is correct independently
/// of what the channel can carry — that separation is BL-37's standing reproduction, and
/// concatenating segments by hand in each of them would put the same logic in three places.
#[cfg(test)]
fn build_project_status_block(status: &ProjectStatus) -> String {
    build_project_status_segments(status)
        .iter()
        .map(|s| s.text.as_str())
        .collect()
}

/// Trim the `## Project Status` segments so `static_part + status` fits the channel,
/// dropping whole segments **by priority** and naming what went.
///
/// The client cuts from the tail at a fixed char count, mid-token, and says nothing — so
/// anything not fitted here vanishes silently. Dropping producer-side is strictly better:
/// the same content goes, but the agent learns that it went, and now *which* went.
///
/// BL-37: the drop order used to be the tail, and the tail is where the user's own text
/// lives — `## Custom Instructions` was sacrificed first, the memories list last. Priority
/// now dominates; within one priority the LATER segment still goes first, so this change
/// only ever reorders *across* tiers and reproduces the old behaviour inside one.
fn fit_dynamic_block(static_part: &str, segments: &[StatusSegment]) -> String {
    let budget = CLIENT_INSTRUCTIONS_CHAR_LIMIT
        .saturating_sub(CHANNEL_SAFETY_MARGIN)
        .saturating_sub(static_part.chars().count());

    let len = |i: usize| segments[i].text.chars().count();
    let total: usize = (0..segments.len()).map(len).sum();
    if total <= budget {
        return segments.iter().map(|s| s.text.as_str()).collect();
    }

    let mut order: Vec<usize> = (0..segments.len()).collect();
    order.sort_by_key(|&i| (segments[i].priority, std::cmp::Reverse(i)));

    // The note costs room too, and it grows as it names more losses — so it is inside the
    // comparison from the first drop rather than subtracted once up front.
    let mut dropped = vec![false; segments.len()];
    let mut labels: Vec<&'static str> = Vec::new();
    let mut used = total;
    for &i in &order {
        if used + trim_note(&labels).chars().count() <= budget {
            break;
        }
        if segments[i].priority == StatusPriority::Anchor {
            continue;
        }
        dropped[i] = true;
        used -= len(i);
        labels.push(segments[i].label);
    }

    let kept: String = (0..segments.len())
        .filter(|&i| !dropped[i])
        .map(|i| segments[i].text.as_str())
        .collect();
    let note = trim_note(&labels);
    if kept.chars().count() + note.chars().count() <= budget {
        return kept + &note;
    }

    // Every droppable segment is gone and the anchors alone still overflow — a
    // pathological project path, or a static slice grown to the cap. The hard guarantee
    // (the total NEVER exceeds the channel, so the static slice is never sacrificed)
    // outranks segment integrity, so fall back to the pre-BL-37 line cut.
    // `production_render_fits_the_client_channel` is the invariant this branch keeps true.
    const SHORT_NOTE: &str = "- (status trimmed to fit the MCP instructions channel)\n";
    if SHORT_NOTE.chars().count() > budget {
        return String::new();
    }
    let room = budget - SHORT_NOTE.chars().count();
    let mut out = String::new();
    let mut used = 0usize;
    for line in kept.split_inclusive('\n') {
        let n = line.chars().count();
        if used + n > room {
            break;
        }
        out.push_str(line);
        used += n;
    }
    out.push_str(SHORT_NOTE);
    out
}

/// The trim note: keeps the phrase `status trimmed` — two channel invariants assert on
/// it — and adds *what* was lost.
///
/// Naming the loss is the point. An agent told which segment went can ask for it by its
/// own route (`memory(action="list")`, `get_guide("workspace-state")`); "something was
/// trimmed" only tells it to distrust the whole block. Capped, because a note that grows
/// with the losses it reports can consume the budget it is reporting on.
fn trim_note(dropped: &[&'static str]) -> String {
    const MAX_NAMED_DROPS: usize = 3;
    if dropped.is_empty() {
        return String::new();
    }
    let named = dropped.len().min(MAX_NAMED_DROPS);
    let mut list = dropped[..named].join(", ");
    if dropped.len() > named {
        list.push_str(&format!(", +{} more", dropped.len() - named));
    }
    format!("- (status trimmed: {list})\n")
}

/// Topic names registered as compiled-in `get_guide(topic)` content.
///
/// Single source of truth: `GetGuide` uses this for tool registration
/// and the input-schema enum; `Tool::call_content` uses [`topic_body`]
/// to inject the body when a `relevant_guide_topic()` hint fires.
pub const GUIDE_TOPICS: &[&str] = &[
    "librarian",
    "librarian-runtime",
    "tracker-conventions",
    "progressive-disclosure",
    "error-handling",
    "workspace-state",
    "iron-laws-detail",
    "symbol-navigation",
    "untrusted-content",
    "project-activation-bootstrap",
];

/// Guide topics reachable **only** by an explicit `get_guide(topic)` call, each with the
/// reason it has no `relevant_guide_topic()` trigger.
///
/// Authoring a guide and wiring its trigger are two separate edits, and nothing used to
/// prompt for the second. `src/prompts/README.md` rule 8 tells an author to move content
/// into a guide when `server_instructions` overflows its 2200-byte cap — so "move it to a
/// guide" read as *filed* when, absent a trigger, it is closer to *deleted from the
/// agent's view*. Measured 2026-08-16: 7 of 10 topics, 47,343 of 75,441 bytes — 63% of the
/// guide corpus — fired for nothing.
///
/// Being on this list is a **decision**, not a default. `every_guide_topic_is_triggered_or_declared_pull_only`
/// (`src/server.rs`) fails the build for any topic that is neither triggered nor listed
/// here, which is what stops the omission recurring silently. It also fails on a stale
/// entry — a topic listed here that later gains a trigger, or that no longer exists.
///
/// Entries marked `PENDING BL-25` are honest about the current state rather than
/// retrofitting a rationale: they are candidates for a trigger whose wiring is blocked on
/// a byte-budget decision, because `librarian` alone is 19.9 KB and already auto-injects on
/// a routine `artifact` call. Wiring more triggers before cutting the corpus trades one
/// problem for another.
///
/// See `docs/issues/archive/2026-08-16-cap-evicted-guidance-lands-in-guides-nothing-triggers.md`.
pub const PULL_ONLY_GUIDE_TOPICS: &[(&str, &str)] = &[
    (
        "librarian-runtime",
        "By design. It is the spill-out half of `librarian` (19.9 KB, auto-injected), and \
         the parent guide ends by naming it. Wiring it would add 9.4 KB to a call that \
         already receives 19.9 KB, which is the byte problem this whole bug is about.",
    ),
    (
        "error-handling",
        "Authoring guidance for contributors writing a new tool — RecoverableError vs \
         anyhow::bail — not runtime guidance a caller acts on. No tool call implies it.",
    ),
    (
        "iron-laws-detail",
        "The gate text and its exceptions, expanding rules the always-loaded \
         `server_instructions` slice already states. A caller who needs the detail has \
         already read the pointer.",
    ),
    (
        "untrusted-content",
        "PENDING BL-25: not yet classified. The candidate trigger is whichever surface \
         first admits third-party text, which has not been identified.",
    ),
];

/// Sections and shapes deliberately left unserved, each with its reason.
///
/// `(topic, heading, reason)`. A row waives one of two things, decided by which
/// gate consults it:
///
/// - **Gate 5** (`every_section_of_a_declaring_topic_is_reachable`,
///   `src/prompts/guide_index.rs`) matches on `(topic, heading)` — the section
///   itself is orientation prose that no single call shape owns.
/// - **Gate 2** (`every_observed_shape_of_a_declaring_topic_has_a_section`,
///   `src/server.rs`) matches on `topic` plus `reason.contains(shape)` — the
///   reason must name the shape verbatim for a shape-level waiver to resolve.
///
/// Either way the reason must exceed 40 characters — a placeholder turns this
/// gate back into the silent default it replaced, which is exactly how 7 of 10
/// topics came to fire for nothing before 2026-08-16 (see
/// `PULL_ONLY_GUIDE_TOPICS` above, the convention this mirrors).
pub const SECTION_WAIVERS: &[(&str, &str, &str)] = &[
    (
        "librarian",
        "Common Mistakes",
        "Troubleshooting prose spanning nearly every `artifact`/`librarian` shape, not \
         one shape in particular: a `requires:` edge from a single section would \
         deliver it to callers who never hit a mistake, and a `serves:` on the bare \
         `artifact` tool key (no `.action`) is a wildcard that the `Shape` matcher \
         treats as matching every `artifact.*` call — over-broad by construction, the \
         exact failure this gate exists to catch. Also resolves the census shapes \
         `artifact` (a call observed missing the required `action` field — no `Shape` \
         can match only the no-action form without matching every `artifact.*` call \
         too) and `librarian.find` (an invalid `librarian` action; the caller most \
         likely meant `doc(action=\"find\")`) — both are exactly the class of \
         mistake this table exists to catch.",
    ),
    (
        "librarian",
        "Runtime tips",
        "Pure cross-reference: a two-line pointer to the separate pull-only \
         `librarian-runtime` topic, not guidance a caller acts on for any one call \
         shape. A `requires:` edge from any declared section would deliver this \
         forwarding pointer to every caller of that shape, most of whom never need \
         `librarian-runtime`; the guide's own closing line already sends whoever does \
         need it there directly.",
    ),
    (
        "librarian",
        "Tracker Workflow",
        "Undeliverable under section grain right now: reachability would require a \
         `requires:` edge from a declaring section, and that edge would push the \
         6-shape p50 session over the 12,000 B ceiling — the corpus is already at \
         capacity (margin 54 B against this section's 346 B). This is not an \
         editorial call that the section doesn't matter; it is blocked purely by the \
         byte ceiling. The recorded remedy is decomposing § Body Editing Surfaces \
         (see `docs/superpowers/plans/2026-08-27-get-guide-section-grain.md` § Out of \
         scope for Phase 1), which would free enough room to make both orphans \
         reachable without raising the ceiling.",
    ),
    (
        "librarian",
        "Body Editing Surfaces",
        "Undeliverable under section grain right now: reachability would require a \
         `requires:` edge from a declaring section, and that edge would push the \
         6-shape p50 session over the 12,000 B ceiling — the corpus is already at \
         capacity (margin 54 B against this section's 1,456 B). This is not an \
         editorial call that the section doesn't matter; it is blocked purely by the \
         byte ceiling. The recorded remedy is decomposing this very section (see \
         `docs/superpowers/plans/2026-08-27-get-guide-section-grain.md` § Out of \
         scope for Phase 1) into smaller declaring sub-sections, which is the fix \
         that would make it reachable without raising the ceiling.",
    ),
];

/// The guide that opens a session.
///
/// Delivered on the first guide-eligible tool call of a session, whatever that
/// call is — see `Tool::call_content` in `src/tools/core/types.rs`.
///
/// Before 2026-08-16 its only trigger was the `workspace` tool, so a session
/// that opened with `symbols`/`grep`/`read_file` — the common case — never
/// received it. That is a *discoverability* failure, not an adherence one: the
/// wording itself measured 100% plausibility-verified as eval arm `s1`
/// (prompt-engineering `scenarios/conclude-last`), and audit-log A-10 found that
/// on-demand guidance is obeyed as reliably as always-visible guidance once
/// fetched — its failure mode is "never fetched", not "fetched then forgotten".
/// So the fix is the trigger, not the text.
pub const SESSION_OPENING_GUIDE: &str = "project-activation-bootstrap";

/// Return the compiled-in markdown body for a `get_guide(topic)` topic.
/// `None` for unknown topics — callers that need a hard-fail should match
/// `None` themselves; `GetGuide::call` wraps `None` into a
/// `RecoverableError`.
///
/// The matched cases must stay in sync with [`GUIDE_TOPICS`]; the
/// `prompts::tests::guide_topics_have_bodies` invariant enforces this.
pub fn topic_body(topic: &str) -> Option<&'static str> {
    match topic {
        "librarian" => Some(include_str!("guides/librarian.md")),
        "librarian-runtime" => Some(include_str!("guides/librarian-runtime.md")),
        "tracker-conventions" => Some(include_str!("guides/tracker-conventions.md")),
        "progressive-disclosure" => Some(include_str!("guides/progressive-disclosure.md")),
        "error-handling" => Some(include_str!("guides/error-handling.md")),
        "workspace-state" => Some(include_str!("guides/workspace-state.md")),
        "iron-laws-detail" => Some(include_str!("guides/iron-laws-detail.md")),
        "symbol-navigation" => Some(include_str!("guides/symbol-navigation.md")),
        "untrusted-content" => Some(include_str!("guides/untrusted-content.md")),
        "project-activation-bootstrap" => {
            Some(include_str!("guides/project-activation-bootstrap.md"))
        }
        _ => None,
    }
}

/// The gate condition behind a refusal, for the error families where knowing it
/// is what prevents the next one.
///
/// Not a guide body — a **predicate**. `iron-laws-detail` holds the full text and
/// is 9.9 KB; these are ~150 bytes each and state the rule *and its exceptions*,
/// which is the part a refused caller cannot infer from the refusal itself.
///
/// **Why this exists.** Measured 2026-08-16 over codescout's own `usage.db`
/// (2026-07-17..2026-08-16): agents comply with an Iron-Law refusal on the very
/// next call **96%** of the time, and **47%** of sessions re-offend on the same
/// law later. They are not ignoring the gate — they cannot predict it, because
/// what makes an input refusable lives in `path_security.rs` and no surface
/// exposes it. `iron-laws-detail` does, and was fetched **once in 30 days
/// against 557 Iron-Law violations**: A-10's "never fetched" failure mode, since
/// guide injection sits after the `?` in `Tool::call_content` and so cannot ride
/// a refusal at all.
///
/// Delivered once per family per session via [`GuideLedger::notice_once`], which
/// keeps the key out of the topic namespace — a sentinel in `emitted` would
/// only suppress [`SESSION_OPENING_GUIDE`] if it collided with that literal
/// topic string (the opener's trigger is the
/// `!emitted.contains(SESSION_OPENING_GUIDE)` check in `Tool::call_content`,
/// `src/tools/core/types.rs`); `notice_once` avoids that regardless, and
/// also keeps the key out of the persisted stamp shape.
///
/// (GF-4 / GF-5 in `docs/trackers/2026-08-16-iron-law-gate-firing-audit.md` —
/// a 2026-08-16 snapshot that predates the predicate change above and still
/// describes the `is_empty()` era; trust the mechanism described here, not
/// that tracker's wording, for current behavior.)
///
/// [`GuideLedger::notice_once`]: crate::tools::guide_ledger::GuideLedger::notice_once
pub fn refusal_predicate(err_family: &str) -> Option<&'static str> {
    Some(match err_family {
        "il1_read_overlaps_symbol" => {
            "IL-1 gate condition: a line-range read is refused only when the range OVERLAPS a \
             named symbol. Ranges over imports, `use` blocks, consts and other glue are allowed, \
             and `force=true` reads the range anyway. A whole-file read of source is always \
             refused — `symbols(path)` for the overview, \
             `symbols(name=..., include_body=true)` for one body."
        }
        "il2_structural_edit" => {
            "IL-2 gate condition: `edit_file` is refused when the edit spans a symbol DEFINITION \
             in a source file. Imports, string literals, comments and config are allowed. \
             Structural changes go through `edit_code` \
             (action=replace|insert|remove|rename)."
        }
        "il3_pipe_to_trimmer" => {
            "IL-3 gate condition: the check reads the LEFT side of the pipe. Unbounded producers \
                 are cargo/npm/pnpm/yarn/python/pytest/go/mvn/gradle/rg/fd, recursive grep, and \
                 `find` without -maxdepth. `git` is unbounded ONLY without an output limiter \
                 (-n, --max-count, -3, --show-current, --porcelain/--short, --stat) — `--oneline` is \
                 NOT a limiter, it bounds width rather than line count; single-line plumbing \
                 (rev-parse, patch-id, merge-base, describe) is always bounded. On the RIGHT, \
                 trimmers (head, tail, grep, sed, awk, sort) block — but `cut`/`tr` are 1:1 on \
                 records and never do, and a stage that COLLAPSES anywhere in the chain (wc, \
                 grep -c, sha256sum, git patch-id) allows the whole pipeline whatever follows it."
        }
        "il3_shell_on_source" => {
            "IL-3 source condition: refused when a CONTENT reader \
             (cat/head/tail/sed/awk/less/more/grep) names a source file INSIDE this project. \
             `wc` is allowed — it returns a count, not content. A path outside the project root \
             is allowed, because `symbols`/`read_file` resolve against the active project and \
             cannot serve it. `acknowledge_risk: true` bypasses."
        }
        _ => return None,
    })
}

/// One row in the workspace project table injected into server instructions.
#[derive(Debug)]
pub struct WorkspaceProjectSummary {
    pub id: String,
    pub root: String,
    pub languages: Vec<String>,
    pub depends_on: Vec<String>,
}

/// Worktree context for the active project, when it lives in a git worktree
/// (i.e. `.git` is a *file* pointing at `<main_repo>/.git/worktrees/<name>/`,
/// not a regular `.git/` directory).
///
/// Used by [`build_server_instructions`] to surface a "Worktree: branch X of
/// /main/repo" line in the Project Status block so the agent knows when it's
/// operating in an isolated worktree vs the main checkout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeInfo {
    /// Current branch name parsed from the worktree's `HEAD` file. `None` when
    /// HEAD is detached (a raw SHA rather than a `ref: refs/heads/...` line).
    pub branch: Option<String>,
    /// Filesystem path of the main repo this worktree belongs to. Parsed from
    /// the `gitdir:` pointer in `.git` (the worktree's `.git` file contains
    /// `gitdir: <main>/.git/worktrees/<name>`; we strip the `/worktrees/<name>`
    /// suffix and a trailing `/.git` to recover `<main>`).
    pub main_repo: Option<std::path::PathBuf>,
    /// The worktree's **git name** — the `<name>` in
    /// `gitdir: <main>/.git/worktrees/<name>`. `None` only when the pointer
    /// does not have that shape.
    ///
    /// Git guarantees this is unique per repository, which the worktree's
    /// directory basename is not: `/a/wt` and `/b/wt` of the same repo share a
    /// basename but never share a git name. That distinction is load-bearing
    /// for `crate::retrieval::sync::worktree_ids`, which keys the delta index
    /// on it — two worktrees collapsing onto one delta project id means one
    /// worktree's sync prunes the other's chunks and then serves them from the
    /// wrong branch, classified `Healthy` with no warning.
    pub name: Option<String>,
}

/// Detect whether `root` is a git worktree and return basic context if so.
///
/// Returns `None` when:
/// - `<root>/.git` does not exist (not a git repo at all).
/// - `<root>/.git` is a directory (regular checkout — not a worktree).
/// - Reading the `.git` pointer file fails.
///
/// Filesystem-only — no `git` subprocess. The detection is the standard
/// "linked worktree" shape: `git worktree add` writes a `.git` *file*
/// containing `gitdir: <abs path to main repo's .git/worktrees/<name>>`.
pub fn detect_worktree_info(root: &std::path::Path) -> Option<WorktreeInfo> {
    let dot_git = root.join(".git");
    let meta = std::fs::symlink_metadata(&dot_git).ok()?;
    if !meta.file_type().is_file() {
        return None;
    }
    let pointer = std::fs::read_to_string(&dot_git).ok()?;
    let gitdir_line = pointer
        .lines()
        .find_map(|l| l.strip_prefix("gitdir:").map(str::trim))?;
    let gitdir = std::path::PathBuf::from(gitdir_line);

    // The worktree's git name is the last segment of that same path. Git keeps
    // it unique per repo, so it is the only identifier here that two sibling
    // worktrees cannot share — see `WorktreeInfo::name`.
    let name = gitdir
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty());

    // Recover the main repo path: gitdir typically looks like
    // `<main_repo>/.git/worktrees/<name>`. Strip `<name>` then `worktrees`
    // then `.git`. Be tolerant — if the shape doesn't match, we still
    // return a WorktreeInfo with main_repo: None.
    let main_repo = gitdir
        .parent() // .../.git/worktrees
        .and_then(|p| p.parent()) // .../.git
        .and_then(|p| p.parent()) // .../<main_repo>
        .map(std::path::PathBuf::from);

    // Branch comes from <gitdir>/HEAD: either `ref: refs/heads/<name>` or
    // a raw SHA (detached HEAD).
    let branch = std::fs::read_to_string(gitdir.join("HEAD"))
        .ok()
        .and_then(|s| {
            s.trim()
                .strip_prefix("ref: refs/heads/")
                .map(|b| b.trim().to_string())
        });

    Some(WorktreeInfo {
        branch,
        main_repo,
        name,
    })
}

/// Dynamic project status used to build server instructions.
#[derive(Debug)]
pub struct ProjectStatus {
    pub name: String,
    pub path: String,
    pub languages: Vec<String>,
    pub memories: Vec<String>,
    pub has_index: bool,
    pub system_prompt: Option<String>,
    /// Other projects in the workspace, if this is a multi-project repo.
    /// None for single-project activations; Some([]) is never emitted.
    pub workspace: Option<Vec<WorkspaceProjectSummary>>,
    /// Git worktree context for the active project. `Some(...)` when the
    /// project root lives in a linked git worktree (a `.git` *file* pointing
    /// at the main repo's worktree dir). Surfaced in server_instructions so
    /// the agent can tell worktree from main-checkout — see
    /// [`detect_worktree_info`].
    pub worktree: Option<WorktreeInfo>,
}

pub const INCLUDE_MARKER: &str = "{{include: memory-templates.md}}";

pub(crate) const RAW_ONBOARDING_PROMPT: &str =
    include_str!(concat!(env!("OUT_DIR"), "/onboarding_prompt.md"));
const RAW_WORKSPACE_ONBOARDING_PROMPT: &str = include_str!("workspace_onboarding_prompt.md");
const MEMORY_TEMPLATES: &str = include_str!("memory-templates.md");

/// Load a prompt with `{{include: memory-templates.md}}` markers substituted.
pub fn load_prompt(name: &str) -> String {
    let raw = match name {
        "onboarding_prompt.md" => RAW_ONBOARDING_PROMPT,
        "workspace_onboarding_prompt.md" => RAW_WORKSPACE_ONBOARDING_PROMPT,
        other => panic!("unknown prompt: {other}"),
    };
    raw.replace(INCLUDE_MARKER, MEMORY_TEMPLATES)
}

/// Context for building the onboarding prompt.
pub struct OnboardingContext<'a> {
    pub languages: &'a [String],
    pub top_level: &'a [String],
    pub key_files: &'a [String],
    pub ci_files: &'a [String],
    pub entry_points: &'a [String],
    pub test_dirs: &'a [String],
    pub index_ready: bool,
    pub index_files: usize,
    pub index_chunks: usize,
    pub projects: &'a [crate::workspace::DiscoveredProject],
    pub is_workspace: bool,
}

/// Build the onboarding prompt, substituting detected project information.
///
/// In workspace mode (multiple projects discovered) the single-project
/// `ONBOARDING_PROMPT` is omitted entirely — keeping its Phase 1/Phase 2
/// instructions in the prompt caused orchestrators to spawn an extra "root"
/// subagent in addition to the per-project ones, duplicating exploration of
/// the dominant sub-project.
pub fn build_onboarding_prompt(ctx: &OnboardingContext) -> String {
    let workspace_mode = ctx.is_workspace && ctx.projects.len() > 1;

    let mut prompt = if workspace_mode {
        load_prompt("workspace_onboarding_prompt.md")
    } else {
        load_prompt("onboarding_prompt.md")
    };

    prompt.push_str("\n\n---\n\n");

    if !ctx.languages.is_empty() {
        prompt.push_str(&format!(
            "**Detected languages:** {}\n\n",
            ctx.languages.join(", ")
        ));
    }

    if !ctx.top_level.is_empty() {
        prompt.push_str(&format!(
            "**Top-level structure:**\n```\n{}\n```\n\n",
            ctx.top_level.join("\n")
        ));
    }

    if !ctx.entry_points.is_empty() {
        prompt.push_str(&format!(
            "**Entry points found:** {}\n\n",
            ctx.entry_points.join(", ")
        ));
    }

    if !ctx.test_dirs.is_empty() {
        prompt.push_str(&format!(
            "**Test directories:** {}\n\n",
            ctx.test_dirs.join(", ")
        ));
    }

    if !ctx.ci_files.is_empty() {
        prompt.push_str(&format!(
            "**CI config files:** {}\n\n",
            ctx.ci_files.join(", ")
        ));
    }

    if !ctx.key_files.is_empty() {
        prompt.push_str(&format!(
            "**Key files to read during Phase 1:**\n{}\n\n",
            ctx.key_files
                .iter()
                .map(|f| format!("- `{f}`"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    if ctx.index_ready {
        prompt.push_str(&format!(
            "**Semantic index:** ready ({} files, {} chunks)\n\n",
            ctx.index_files, ctx.index_chunks
        ));
    } else {
        prompt.push_str("**Semantic index:** not built\n\n");
    }

    if workspace_mode {
        prompt.push_str(&format!(
            "**Workspace mode:** {} projects detected\n\n",
            ctx.projects.len()
        ));
        prompt.push_str("**Discovered projects:**\n\n");
        prompt.push_str("| Project | Root | Languages | Build |\n");
        prompt.push_str("|---------|------|-----------|-------|\n");
        for p in ctx.projects {
            prompt.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                p.id,
                crate::util::fs::to_forward_slash(&p.relative_root),
                p.languages.join(", "),
                p.manifest.as_deref().unwrap_or("-"),
            ));
        }
        prompt.push('\n');
    }

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_with_project_appends_status() {
        let status = ProjectStatus {
            name: "my-project".into(),
            path: "/home/user/my-project".into(),
            languages: vec!["rust".into(), "python".into()],
            memories: vec!["architecture".into(), "conventions".into()],
            has_index: true,
            system_prompt: None,
            workspace: None,
            worktree: None,
        };
        let result = build_server_instructions(Some(&status));
        assert!(result.contains("## Project Status"));
        // D: explicit activation banner — "Active project" wording surfaces the
        // implicit launch-time activation so agents don't have to infer it from
        // path stripping in tool output.
        assert!(
            result.contains("**Active project:** my-project at `/home/user/my-project`"),
            "missing Active project banner, got:\n{result}"
        );
        // Without worktree info present, NO worktree line should appear.
        assert!(
            !result.contains("Worktree:"),
            "non-worktree project must not emit a Worktree line, got:\n{result}"
        );

        // THE SPLIT, asserted from both sides. Languages / memories / index are
        // `Substitutable` and now ride the tool response, so the persistent channel must
        // NOT carry them — and the response block must. Asserting only the second half
        // would pass with the segments duplicated in both, which is the failure mode this
        // change exists to remove: the same content spending the one persistent budget
        // twice.
        for absent in [
            "rust, python",
            "architecture, conventions",
            "Semantic index",
        ] {
            assert!(
                !result.contains(absent),
                "`{absent}` is Substitutable and must not spend instructions budget, got:\n{result}"
            );
        }
        let block = build_status_response_block(&status).expect("substitutable block");
        assert!(block.contains("rust, python"), "{block}");
        assert!(block.contains("architecture, conventions"), "{block}");
        assert!(block.contains("Semantic index:** Built"), "{block}");
    }

    /// The onboarding and index nudges are `Substitutable`, so they moved with their tier.
    /// Retargeted rather than deleted: the behaviour under test — an empty project is told
    /// what to run — is unchanged, only the carrier moved.
    #[test]
    fn build_with_no_memories_suggests_onboarding() {
        let status = ProjectStatus {
            name: "new-project".into(),
            path: "/tmp/new".into(),
            languages: vec![],
            memories: vec![],
            has_index: false,
            system_prompt: None,
            workspace: None,
            worktree: None,
        };
        let block = build_status_response_block(&status).expect("nudges are substitutable");
        assert!(block.contains("run `onboarding`"), "{block}");
        assert!(block.contains("run `index(action='build')`"), "{block}");

        // And the anchor still arrives on the persistent channel, so an agent knows which
        // project the nudges are about even after a compaction eats the block.
        let result = build_server_instructions(Some(&status));
        assert!(
            result.contains("**Active project:** new-project at `/tmp/new`"),
            "{result}"
        );
    }

    /// The response block carries no anchors — they are the persistent channel's job, and
    /// duplicating them would be the same double-spend from the other direction. A separate
    /// test because it is the half a future edit is likelier to break: adding a segment to
    /// `build_project_status_segments` and forgetting its priority silently files it under
    /// whichever variant is declared first, which is `Substitutable`.
    #[test]
    fn the_response_block_carries_no_anchor_or_user_authored_segments() {
        let status = ProjectStatus {
            name: "my-project".into(),
            path: "/home/user/my-project".into(),
            languages: vec!["rust".into()],
            memories: vec!["architecture".into()],
            has_index: true,
            system_prompt: Some("Always run the integration suite.".into()),
            workspace: None,
            worktree: Some(WorktreeInfo {
                branch: Some("feat/x".into()),
                main_repo: Some(std::path::PathBuf::from("/tmp/main")),
                name: Some("x".into()),
            }),
        };
        let block = build_status_response_block(&status).expect("substitutable block");
        assert!(
            !block.contains("Active project"),
            "the anchor must stay in the persistent channel: {block}"
        );
        assert!(
            !block.contains("Worktree:"),
            "the worktree banner is the one segment whose absence causes a wrong WRITE, and \
             a response-carried copy arrives too late for a first call that IS the write: \
             {block}"
        );
        assert!(
            !block.contains("Custom Instructions"),
            "UserAuthored stays persistent while it fits — the user's own rules are worth \
             more than a memories list when a compaction takes one of them: {block}"
        );

        // The persistent side carries exactly those three.
        let result = build_server_instructions(Some(&status));
        assert!(result.contains("**Worktree:** branch `feat/x`"), "{result}");
        assert!(result.contains("## Custom Instructions"), "{result}");
    }

    #[test]
    fn onboarding_prompt_contains_key_sections() {
        let prompt = load_prompt("onboarding_prompt.md");
        assert!(prompt.contains("## THE IRON LAW"));
        assert!(prompt.contains("## Phase 0: Embedding Model Selection"));
        assert!(prompt.contains("## Phase 1: Semantic Index Check"));
        assert!(prompt.contains("## Phase 2: Explore the Code"));
        assert!(prompt.contains("### project-scope: project-overview"));
        assert!(prompt.contains("### project-scope: architecture"));
        assert!(prompt.contains("## Coverage Verification"));
        assert!(prompt.contains("### Refresh CLAUDE.md"));
    }

    #[test]
    fn workspace_onboarding_prompt_contains_key_sections() {
        let prompt = load_prompt("workspace_onboarding_prompt.md");
        assert!(prompt.contains("# WORKSPACE MODE"));
        assert!(prompt.contains("## Phase 1 — Workspace Survey"));
        assert!(prompt.contains("## Phase 3 — Per-Project Deep Dives"));
        assert!(prompt.contains("## Phase 4 — Coverage Verification"));
        assert!(prompt.contains("## Phase 5 — Workspace Synthesis"));
        assert!(prompt.contains("## Phase 6 — CLAUDE.md Refresh"));
    }
    #[test]
    fn load_prompt_substitutes_include_marker() {
        let single = load_prompt("onboarding_prompt.md");
        let workspace = load_prompt("workspace_onboarding_prompt.md");
        assert!(
            !single.contains("{{include: memory-templates.md}}"),
            "include marker should be substituted in single-project prompt"
        );
        assert!(
            !workspace.contains("{{include: memory-templates.md}}"),
            "include marker should be substituted in workspace prompt"
        );
    }

    #[test]
    fn build_onboarding_includes_languages() {
        let result = build_onboarding_prompt(&OnboardingContext {
            languages: &["rust".into(), "python".into()],
            top_level: &["src/".into(), "tests/".into()],
            key_files: &[],
            ci_files: &[],
            entry_points: &[],
            test_dirs: &[],
            index_ready: false,
            index_files: 0,
            index_chunks: 0,
            projects: &[],
            is_workspace: false,
        });
        assert!(result.contains("rust, python"));
        assert!(result.contains("src/"));
    }

    #[test]
    fn build_onboarding_handles_empty() {
        let result = build_onboarding_prompt(&OnboardingContext {
            languages: &[],
            top_level: &[],
            key_files: &[],
            ci_files: &[],
            entry_points: &[],
            test_dirs: &[],
            index_ready: false,
            index_files: 0,
            index_chunks: 0,
            projects: &[],
            is_workspace: false,
        });
        assert!(result.contains("## Rules"));
        assert!(!result.contains("Detected languages"));
    }

    #[test]
    fn build_onboarding_includes_gathered_context() {
        let result = build_onboarding_prompt(&OnboardingContext {
            languages: &["rust".into(), "python".into()],
            top_level: &["src/".into(), "tests/".into()],
            key_files: &["README.md".into(), "Cargo.toml".into(), "CLAUDE.md".into()],
            ci_files: &[".github/workflows/ci.yml".into()],
            entry_points: &["src/main.rs".into()],
            test_dirs: &["tests".into()],
            index_ready: false,
            index_files: 0,
            index_chunks: 0,
            projects: &[],
            is_workspace: false,
        });
        assert!(result.contains("Cargo.toml"));
        assert!(result.contains("ci.yml"));
        assert!(result.contains("src/main.rs"));
        assert!(result.contains("Detected languages"));
    }

    #[test]
    /// Retargeted 2026-08-16 (BL-9). The renderer's job — emit `## Custom Instructions`
    /// after the status lines — is unchanged and still asserted here. What changed is the
    /// *channel*: the MCP `instructions` field is capped at 2048 chars and a custom prompt
    /// does not fit alongside the static slice, so asserting on
    /// `build_server_instructions` would now be asserting that the trim is broken.
    fn build_with_system_prompt_appends_custom_section() {
        let status = ProjectStatus {
            name: "my-project".into(),
            path: "/tmp/my-project".into(),
            languages: vec![],
            memories: vec![],
            has_index: false,
            system_prompt: Some("Always use pytest.".into()),
            workspace: None,
            worktree: None,
        };
        let block = build_project_status_block(&status);
        assert!(block.contains("## Custom Instructions"));
        assert!(block.contains("Always use pytest."));
        // Custom instructions come after project status.
        let status_pos = block.find("## Project Status").unwrap();
        let custom_pos = block.find("## Custom Instructions").unwrap();
        assert!(custom_pos > status_pos);
    }

    /// BL-37 established that the user's own text must outlive a memories list that
    /// `memory(action="list")` reproduces on demand. That decision is unchanged; what
    /// changed is how it is EXPRESSED.
    ///
    /// It used to be a drop ORDER inside one channel — both segments competed for the same
    /// 2048 characters and `UserAuthored` was dropped later than `Substitutable`. Since the
    /// tier split they no longer compete: `Substitutable` moved to the tool response and
    /// `UserAuthored` kept the persistent channel, so the same judgement is now a CHANNEL
    /// ASSIGNMENT. The old form is unreachable — `fit_dynamic_block` never sees a
    /// substitutable segment any more — so asserting it would assert nothing.
    ///
    /// Retargeted rather than deleted, because the decision it guards is still live and
    /// still the one a future edit could quietly reverse by re-tiering a segment.
    #[test]
    fn an_overflowing_status_keeps_the_user_s_own_text_over_a_substitutable_list() {
        let status = ProjectStatus {
            name: "my-project".into(),
            path: "/tmp/my-project".into(),
            languages: vec!["rust".into(), "python".into(), "typescript".into()],
            memories: vec![
                "architecture".into(),
                "conventions".into(),
                "development-commands".into(),
                "domain-glossary".into(),
                "gotchas".into(),
                "language-patterns".into(),
                "onboarding".into(),
                "project-overview".into(),
            ],
            has_index: false,
            system_prompt: Some("Always run the integration suite before pushing.".into()),
            workspace: None,
            worktree: None,
        };

        let rendered = build_server_instructions(Some(&status));
        assert!(
            rendered.contains("Always run the integration suite before pushing."),
            "the user's own instructions hold the persistent channel — it is the only \
             surface that survives a compaction, and nothing else shows them to the agent; \
             got:\n{rendered}"
        );
        assert!(
            !rendered.contains("architecture, conventions"),
            "and the memories list must NOT be there competing for it: \
             `memory(action=\"list\")` reproduces it on demand; got:\n{rendered}"
        );

        let block = build_status_response_block(&status).expect("substitutable block");
        assert!(
            block.contains("architecture, conventions"),
            "the list is not lost, only re-homed — and it arrives WHOLE here, where the \
             old channel truncated it: {block}"
        );
    }

    /// The worktree banner is the segment nothing else supplies, and its absence is not
    /// merely inconvenient: an agent that assumes the activated root is the canonical
    /// checkout commits to the wrong branch. It must outlive everything else in the
    /// persistent channel.
    ///
    /// The fixture changed with the tier split. Overflowing used to be easy — a memories
    /// list and a language list were enough. Now only `UserAuthored` competes with the
    /// anchors, so forcing the trimming path takes a genuinely large custom prompt. Same
    /// invariant, and the control below is what proves the fixture still reaches it.
    #[test]
    fn an_overflowing_status_keeps_the_worktree_banner() {
        let status = ProjectStatus {
            name: "my-project".into(),
            path: "/tmp/wt/my-project".into(),
            languages: vec!["rust".into(), "python".into(), "typescript".into()],
            memories: vec!["architecture".into(), "conventions".into()],
            has_index: true,
            system_prompt: Some("Always run the integration suite before pushing. ".repeat(20)),
            workspace: None,
            worktree: Some(WorktreeInfo {
                branch: Some("feat/x".into()),
                main_repo: Some(std::path::PathBuf::from("/tmp/main")),
                name: Some("x".into()),
            }),
        };

        let rendered = build_server_instructions(Some(&status));
        assert!(
            rendered.contains("trimmed"),
            "the fixture must overflow or this test proves nothing; got:\n{rendered}"
        );
        assert!(
            rendered.contains("**Worktree:**"),
            "a dropped worktree banner sends commits to the wrong branch; got:\n{rendered}"
        );
        assert!(
            rendered.chars().count() <= CLIENT_INSTRUCTIONS_CHAR_LIMIT - CHANNEL_SAFETY_MARGIN,
            "and the hard guarantee still holds under the new tiering; got {} chars",
            rendered.chars().count()
        );
    }

    /// A trim that says only "something went" tells the agent to distrust the whole block.
    /// One that names the segment tells it which route to take instead. The naming is the
    /// difference between a warning and an instruction.
    ///
    /// The fixture had to change and the reason is the point of the tier split. This test
    /// used to force a trim with 30 memories and a Kotlin project, and asserted the note
    /// named `kotlin known issues` — because the Kotlin block *"cannot fit at any position,
    /// so the agent's only chance of knowing it exists is being told it went."* That is no
    /// longer true: the block is `Substitutable`, so it now arrives whole on the response
    /// channel. Being told what you lost was always second best to not losing it, and the
    /// companion assertion below is the one that matters now.
    #[test]
    fn a_trim_names_what_it_dropped() {
        let status = ProjectStatus {
            name: "my-project".into(),
            path: "/tmp/my-project".into(),
            languages: vec!["rust".into(), "kotlin".into()],
            memories: (0..30).map(|i| format!("memory-topic-{i}")).collect(),
            has_index: true,
            // The only droppable segment left in the persistent channel, sized to force it.
            system_prompt: Some("Always run the integration suite before pushing. ".repeat(20)),
            workspace: None,
            worktree: None,
        };

        let rendered = build_server_instructions(Some(&status));
        assert!(
            rendered.contains("status trimmed: "),
            "the note must name the losses, not just announce one; got:\n{rendered}"
        );
        assert!(
            rendered.contains("custom instructions"),
            "with the substitutable tier re-homed, custom instructions is the only segment \
             a trim can still take — and the agent has no other route to it, so being told \
             is the whole remedy; got:\n{rendered}"
        );

        // The improvement the split bought, asserted so a regression is visible: the Kotlin
        // block used to be the first thing dropped and is now delivered in full.
        let block = build_status_response_block(&status).expect("substitutable block");
        assert!(
            block.contains("kotlin"),
            "the Kotlin known-issues block must ARRIVE, not be named in a trim note: {block}"
        );
        assert!(
            !block.contains("trimmed"),
            "and the response channel has no cap, so nothing there is trimmed: {block}"
        );
    }

    /// The note competes with the content it reports on, so its size is a budget
    /// decision rather than a wording preference. Pinned with REALISTIC labels, because
    /// the sibling bound uses one-character names and a 116-char note slipped under it.
    ///
    /// Provenance, and a correction worth keeping. A wire check against the release
    /// binary showed this repo's delivered status carrying only the active-project line,
    /// and I attributed the missing `Languages` line to the note's length. Rebuilding
    /// that case in-process refuted it: with the long note, `Languages` survives. The
    /// real status has six droppable segments — it carries a workspace table and a custom
    /// prompt, and its memory names are far longer — where the fixture had three. It was
    /// simply bigger. The note's cost is real and worth bounding; the eviction was never
    /// demonstrated, and this test pins only the half that was. See R-102.
    #[test]
    fn the_trim_note_stays_small_next_to_the_budget_it_reports_on() {
        // The labels this actually ships with, at the length they actually are.
        let note = trim_note(&[
            "kotlin known issues",
            "workspace table",
            "index status",
            "memories",
        ]);
        assert!(
            note.contains("kotlin known issues, workspace table, index status, +1 more"),
            "got: {note}"
        );

        // The dynamic budget is ~289 chars (2048 − 48 − a 1711-char static slice) and a
        // status line runs 40–85, so a note past ~90 spends a line's worth of room on
        // bookkeeping. The pre-2026-08-17 wording — "status trimmed to fit the MCP
        // instructions channel: …" — measures 116 here, and is what this bound refuses.
        assert!(
            note.chars().count() <= 90,
            "the note costs {} chars of a ~289-char budget, a status line's worth of \
             bookkeeping: {note}",
            note.chars().count()
        );
    }

    /// The note grows with the losses it reports, so an uncapped one can consume the
    /// budget it exists to explain. Three names carry the signal; a count carries the rest.
    #[test]
    fn the_trim_note_caps_the_names_it_lists() {
        let none = trim_note(&[]);
        assert!(none.is_empty(), "no drops, no note");

        let two = trim_note(&["memories", "languages"]);
        assert!(two.contains("memories, languages"));
        assert!(
            !two.contains("more"),
            "two names fit without a count: {two}"
        );

        let five = trim_note(&["a", "b", "c", "d", "e"]);
        assert!(five.contains("a, b, c, +2 more"), "got: {five}");
        assert!(
            five.chars().count() < 90,
            "the note must stay small next to the ~289-char budget it reports on: {five}"
        );
    }

    #[test]
    fn build_without_system_prompt_has_no_custom_section() {
        let status = ProjectStatus {
            name: "my-project".into(),
            path: "/tmp/my-project".into(),
            languages: vec![],
            memories: vec![],
            has_index: false,
            system_prompt: None,
            workspace: None,
            worktree: None,
        };
        let result = build_server_instructions(Some(&status));
        assert!(!result.contains("## Custom Instructions"));
    }

    #[test]
    fn build_with_workspace_appends_project_table() {
        let status = ProjectStatus {
            name: "backend-kotlin".into(),
            path: "/workspace/backend-kotlin".into(),
            languages: vec!["kotlin".into()],
            memories: vec![],
            has_index: false,
            system_prompt: None,
            workspace: Some(vec![
                WorkspaceProjectSummary {
                    id: "backend-kotlin".into(),
                    root: ".".into(),
                    languages: vec!["kotlin".into()],
                    depends_on: vec![],
                },
                WorkspaceProjectSummary {
                    id: "mcp-server".into(),
                    root: "mcp-server/".into(),
                    languages: vec!["typescript".into()],
                    depends_on: vec![],
                },
                WorkspaceProjectSummary {
                    id: "python-services".into(),
                    root: "python-services/".into(),
                    languages: vec!["python".into()],
                    depends_on: vec!["mcp-server".into()],
                },
            ]),
            worktree: None,
        };
        // Asserted on the RENDERER, not on `build_server_instructions`: a workspace
        // table cannot fit the 2048-char MCP instructions channel alongside the static
        // slice, so the shipped surface trims it with a note (BL-9). Testing the whole
        // render here would be asserting that the trim is broken.
        let block = build_project_status_block(&status);
        assert!(block.contains("## Workspace Projects"));
        assert!(block.contains("mcp-server"));
        assert!(block.contains("python-services"));
        assert!(block.contains("python-services/"));
        // depends_on rendered for python-services
        assert!(block.contains("mcp-server"));
        // Scoping hint names only params the tools actually advertise. This assertion
        // used to read `contains("project: \"<id>\"")` and was satisfied BY THE DEFECT:
        // the sentence named `project` on `symbols`, which advertises no such param and
        // silently ignored it, for three months.
        // docs/issues/archive/2026-09-02-activation-banner-names-a-project-param-symbols-does-not-have.md
        assert!(block.contains("project_id: \"<id>\""));
        assert!(
            !block.contains("` in `symbols`"),
            "`symbols` advertises no project param — naming one here is the defect this \
                 assertion now guards, and a substring check that both spellings satisfy is \
                 how it went unnoticed: {block}"
        );
    }

    #[test]
    fn build_with_single_project_no_workspace_table() {
        // workspace: None → no table emitted even if the field is absent
        let status = ProjectStatus {
            name: "solo".into(),
            path: "/solo".into(),
            languages: vec!["rust".into()],
            memories: vec![],
            has_index: false,
            system_prompt: None,
            workspace: None,
            worktree: None,
        };
        let result = build_server_instructions(Some(&status));
        assert!(!result.contains("## Workspace Projects"));
    }

    #[test]
    fn build_onboarding_shows_index_ready() {
        let result = build_onboarding_prompt(&OnboardingContext {
            languages: &["rust".into()],
            top_level: &[],
            key_files: &[],
            ci_files: &[],
            entry_points: &[],
            test_dirs: &[],
            index_ready: true,
            index_files: 42,
            index_chunks: 350,
            projects: &[],
            is_workspace: false,
        });
        assert!(result.contains("Semantic index:** ready (42 files, 350 chunks)"));
    }

    #[test]
    fn build_onboarding_shows_index_not_built() {
        let result = build_onboarding_prompt(&OnboardingContext {
            languages: &["rust".into()],
            top_level: &[],
            key_files: &[],
            ci_files: &[],
            entry_points: &[],
            test_dirs: &[],
            index_ready: false,
            index_files: 0,
            index_chunks: 0,
            projects: &[],
            is_workspace: false,
        });
        assert!(result.contains("Semantic index:** not built"));
    }

    #[test]
    fn onboarding_prompt_includes_workspace_projects() {
        use std::path::PathBuf;
        let projects = vec![
            crate::workspace::DiscoveredProject {
                id: "api".to_string(),
                relative_root: PathBuf::from("api"),
                languages: vec!["rust".to_string()],
                manifest: Some("Cargo.toml".to_string()),
            },
            crate::workspace::DiscoveredProject {
                id: "frontend".to_string(),
                relative_root: PathBuf::from("frontend"),
                languages: vec!["typescript".to_string()],
                manifest: Some("package.json".to_string()),
            },
        ];
        let ctx = OnboardingContext {
            languages: &["rust".to_string(), "typescript".to_string()],
            top_level: &["api/".to_string(), "frontend/".to_string()],
            key_files: &[],
            ci_files: &[],
            entry_points: &["api/src/main.rs".to_string()],
            test_dirs: &[],
            index_ready: false,
            index_files: 0,
            index_chunks: 0,
            projects: &projects,
            is_workspace: true,
        };
        let prompt = build_onboarding_prompt(&ctx);
        assert!(prompt.contains("Workspace"));
        assert!(prompt.contains("Workspace Survey"));
        assert!(prompt.contains("api"));
        assert!(prompt.contains("frontend"));
    }

    /// No project gets language-specific warnings pushed at it any more, and the fixture
    /// here is the one that used to trigger them.
    ///
    /// The inverse of what this test used to assert. It was
    /// `build_with_kotlin_project_includes_kotlin_warnings`, and it had already been
    /// retargeted once — from `build_server_instructions` to the renderer — so it could keep
    /// passing while the segment it described was trimmed away before reaching anyone. A
    /// test that survives by being pointed at a surface nobody receives is a signal.
    ///
    /// Removed rather than narrowed to Kotlin-only projects, because the block bought
    /// nothing at any trigger: `detect_fatal_stderr` (`src/lsp/client.rs`) raises a
    /// `RecoverableError` naming the condition and the fix at the moment it happens. The
    /// block conceded this in its own last line — "codescout detects this and fails fast
    /// with a clear error". Pre-loading an explanation of a self-announcing error is cost
    /// without benefit.
    #[test]
    fn no_language_specific_warnings_are_pushed_at_any_project() {
        let kotlin = ProjectStatus {
            name: "test".into(),
            path: "/tmp/test".into(),
            languages: vec!["kotlin".into(), "java".into()],
            memories: vec![],
            has_index: false,
            system_prompt: None,
            workspace: None,
            worktree: None,
        };
        assert!(
            !build_project_status_block(&kotlin).contains("kotlin-lsp"),
            "a Kotlin project must not be pushed an explanation of an error that already \
             explains itself"
        );
        assert!(
            !build_server_instructions(Some(&kotlin)).contains("kotlin-lsp"),
            "and it must not reach the persistent channel"
        );
        assert!(
            !build_status_response_block(&kotlin)
                .unwrap_or_default()
                .contains("kotlin-lsp"),
            "nor the response channel — removing the segment means removing it, not \
             re-homing it somewhere less visible"
        );

        // The TRIGGER was the real defect, and this pins it: `languages` is what a repo
        // CONTAINS, not what it is written in. codescout is a Rust project with Kotlin
        // fixtures, and on 2026-08-21 it was observed serving itself this block live.
        let rust_with_fixtures = ProjectStatus {
            languages: vec!["rust".into(), "kotlin".into(), "markdown".into()],
            ..kotlin
        };
        assert!(
            !build_project_status_block(&rust_with_fixtures).contains("kotlin-lsp"),
            "a Rust project that merely contains .kt fixtures is not a Kotlin project"
        );
    }

    #[test]
    fn build_with_worktree_emits_worktree_banner() {
        // C: when ProjectStatus carries WorktreeInfo, the Project Status block
        // must surface a "Worktree: branch X of /main/repo" line so the agent
        // knows it's in a linked worktree, not the main checkout.
        let status = ProjectStatus {
            name: "backend-kotlin".into(),
            path: "/home/user/repo/.worktrees/weekly-pattern".into(),
            languages: vec!["kotlin".into()],
            memories: vec![],
            has_index: false,
            system_prompt: None,
            workspace: None,
            worktree: Some(WorktreeInfo {
                branch: Some("weekly-pattern".into()),
                main_repo: Some(std::path::PathBuf::from("/home/user/repo")),
                name: Some("weekly-pattern".into()),
            }),
        };
        let result = build_server_instructions(Some(&status));
        assert!(
            result.contains("**Worktree:** branch `weekly-pattern` of `/home/user/repo`"),
            "missing worktree banner, got:\n{result}"
        );
    }

    #[test]
    fn build_with_detached_worktree_renders_placeholder() {
        // Edge case: HEAD is detached (raw SHA, not `ref: refs/heads/...`).
        // The banner should still emit with a clear "<detached HEAD>" marker
        // rather than silently dropping the worktree line.
        let status = ProjectStatus {
            name: "wt".into(),
            path: "/some/path".into(),
            languages: vec![],
            memories: vec![],
            has_index: false,
            system_prompt: None,
            workspace: None,
            worktree: Some(WorktreeInfo {
                branch: None,
                main_repo: Some(std::path::PathBuf::from("/main")),
                name: Some("wt".into()),
            }),
        };
        let result = build_server_instructions(Some(&status));
        assert!(
            result.contains("**Worktree:** branch `<detached HEAD>` of `/main`"),
            "detached HEAD placeholder missing, got:\n{result}"
        );
    }

    #[test]
    fn detect_worktree_info_identifies_linked_worktree() {
        // Build a fake worktree fixture on disk:
        //   <tmp>/main/.git/worktrees/feat/HEAD       — ref: refs/heads/feat
        //   <tmp>/wt/.git                              — gitdir: <tmp>/main/.git/worktrees/feat
        // detect_worktree_info(<tmp>/wt) must return Some with both branch
        // and main_repo populated correctly.
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("main");
        let wt = dir.path().join("wt");
        let worktree_meta = main.join(".git").join("worktrees").join("feat");
        std::fs::create_dir_all(&worktree_meta).unwrap();
        std::fs::write(worktree_meta.join("HEAD"), "ref: refs/heads/feat\n").unwrap();
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(
            wt.join(".git"),
            format!("gitdir: {}\n", worktree_meta.display()),
        )
        .unwrap();

        let info = detect_worktree_info(&wt).expect("worktree should be detected");
        assert_eq!(info.branch.as_deref(), Some("feat"));
        assert_eq!(info.main_repo.as_deref(), Some(main.as_path()));
        // The git worktree name is the last gitdir segment -- `feat` here,
        // deliberately DIFFERENT from the checkout directory's basename
        // (`wt`), so reading `name` off the wrong end of the path fails.
        // `retrieval::sync::worktree_key` keys the delta index on this.
        assert_eq!(info.name.as_deref(), Some("feat"));
    }

    #[test]
    fn detect_worktree_info_returns_none_for_regular_checkout() {
        // A real checkout has `.git` as a directory, not a file. Detector
        // must return None so the banner stays absent.
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        assert!(
            detect_worktree_info(dir.path()).is_none(),
            "regular checkout must not be classified as a worktree"
        );
    }

    #[test]
    fn detect_worktree_info_returns_none_when_no_git() {
        // Plain directory with no .git at all — defensive: returns None
        // rather than panicking on a missing path.
        let dir = tempfile::tempdir().unwrap();
        assert!(detect_worktree_info(dir.path()).is_none());
    }

    #[test]
    fn memory_templates_have_all_project_scope_sections() {
        let templates = include_str!("memory-templates.md");
        for topic in [
            "project-overview",
            "architecture",
            "conventions",
            "development-commands",
            "domain-glossary",
            "gotchas",
        ] {
            let heading = format!("### project-scope: {topic}");
            assert!(
                templates.contains(&heading),
                "memory-templates.md missing heading: {heading}"
            );
        }
    }

    #[test]
    fn memory_templates_define_empty_stub() {
        let templates = include_str!("memory-templates.md");
        assert!(
            templates.contains("EMPTY_STUB:"),
            "memory-templates.md must define the canonical empty stub"
        );
    }

    #[test]
    fn guide_topics_have_bodies() {
        for &topic in crate::prompts::GUIDE_TOPICS {
            let body = crate::prompts::topic_body(topic).unwrap_or_else(|| {
                panic!(
                    "GUIDE_TOPICS lists '{topic}' but topic_body returned None — \
                     add a match arm with include_str!(\"guides/{topic}.md\")"
                )
            });
            assert!(!body.is_empty(), "topic '{topic}' has an empty body");
        }
    }

    /// `PULL_ONLY_GUIDE_TOPICS` membership and reason-quality checks that used to
    /// live inside the deleted `every_guide_topic_is_triggered_or_declared_pull_only`
    /// (see `src/server.rs`, replaced by Gate 2 —
    /// `every_observed_shape_of_a_declaring_topic_has_a_section`). Gate 2 is scoped to
    /// topics that have opted into section grain (`GUIDE_INDEX.declares(topic)`) — only
    /// `librarian` today — so it says nothing about the other nine, still-whole-topic
    /// entries in `PULL_ONLY_GUIDE_TOPICS`. Deleting the old combined test would have
    /// silently dropped these two checks rather than moved them, so they are restored
    /// here rather than in `src/server.rs`, since neither needs a running `Server`.
    #[test]
    fn pull_only_guide_topics_are_registered_with_real_reasons() {
        for (topic, reason) in crate::prompts::PULL_ONLY_GUIDE_TOPICS {
            assert!(
                crate::prompts::GUIDE_TOPICS.contains(topic),
                "PULL_ONLY_GUIDE_TOPICS names `{topic}`, which is not a registered \
                     guide topic — a rename or removal left it behind."
            );
            assert!(
                reason.len() > 40,
                "the reason for `{topic}` must say why it is pull-only; a placeholder \
                     turns this check back into the silent default it replaced. Got: {reason:?}"
            );
        }
    }

    /// Mirrors the check above for `SECTION_WAIVERS`: every waiver must name a
    /// registered guide topic and carry a real, non-placeholder reason. A waiver with
    /// a placeholder reason is indistinguishable from no waiver at all — it would
    /// silence Gate 2/5 without saying why, which is the same failure mode the
    /// `> 40` convention on `PULL_ONLY_GUIDE_TOPICS` exists to catch.
    #[test]
    fn section_waivers_are_registered_with_real_reasons() {
        for (topic, heading, reason) in crate::prompts::SECTION_WAIVERS {
            assert!(
                crate::prompts::GUIDE_TOPICS.contains(topic),
                "SECTION_WAIVERS names topic `{topic}` for heading `{heading}`, which \
                     is not a registered guide topic — a rename or removal left it behind."
            );
            assert!(
                reason.len() > 40,
                "the reason for waiving `{topic}` § `{heading}` must say why; a \
                     placeholder turns this waiver back into a silent gap. Got: {reason:?}"
            );
        }
    }

    #[test]
    fn memory_templates_have_all_workspace_scope_sections() {
        let templates = include_str!("memory-templates.md");
        for topic in [
            "architecture",
            "conventions",
            "development-commands",
            "domain-glossary",
            "gotchas",
            "system-prompt",
        ] {
            let heading = format!("### workspace-scope: {topic}");
            assert!(
                templates.contains(&heading),
                "memory-templates.md missing heading: {heading}"
            );
        }
    }

    #[test]
    fn workspace_architecture_template_has_required_subsections() {
        let templates = include_str!("memory-templates.md");
        for sub in [
            "Project Map",
            "Cross-Project Dependencies",
            "Shared Infrastructure",
            "Top-Level Code Map",
            "Generic Navigation",
        ] {
            assert!(
                templates.contains(&format!("- `## {sub}`")),
                "workspace architecture template missing required subsection: {sub}"
            );
        }
    }

    #[test]
    fn workspace_prompt_has_six_phases() {
        let workspace = load_prompt("workspace_onboarding_prompt.md");
        for phase in [
            "## Phase 1 — Workspace Survey",
            "## Phase 2 — Stale-Project Cleanup",
            "## Phase 3 — Per-Project Deep Dives",
            "## Phase 4 — Coverage Verification",
            "## Phase 5 — Workspace Synthesis",
            "## Phase 6 — CLAUDE.md Refresh",
        ] {
            assert!(
                workspace.contains(phase),
                "workspace prompt missing phase: {phase}"
            );
        }
    }

    #[test]
    fn onboarding_prompts_write_system_prompt_to_root_not_memory() {
        // Regression (2026-06-12): onboarding must write the system prompt directly
        // to the root `.codescout/system-prompt.md` via `create_file` — that is the
        // always-on injection's read path (`Agent::project_status`). It must NOT route
        // through `memory(write, topic="system-prompt")`, which lands in
        // `.codescout/memories/` and never reaches `server_instructions`.
        // See docs/issues/archive/2026-06-12-onboarding-writes-system-prompt-to-memory-not-root.md.
        // The corrective prompts mention the prohibited call inside a "Do NOT" clause,
        // so we assert the POSITIVE `create_file` instruction plus absence of the
        // affirmative `topic: "system-prompt", content: ...)` form, not bare absence.
        for name in ["onboarding_prompt.md", "workspace_onboarding_prompt.md"] {
            let prompt = load_prompt(name);
            assert!(
                prompt.contains("create_file"),
                "{name} must instruct a direct `create_file` write of the system prompt"
            );
            assert!(
                !prompt.contains("topic: \"system-prompt\", content"),
                "{name} must NOT instruct memory(write, topic=\"system-prompt\", content=...) \
                 for the system prompt"
            );
        }
    }

    #[test]
    fn synthesis_prompt_writes_system_prompt_to_root_not_memory() {
        // Regression (2026-06-12): same root cause as the onboarding-prompt guard —
        // workspace synthesis writes the system prompt to the root file directly,
        // not via the memory store.
        let prompt =
            builders::build_synthesis_prompt(&[("proj-a".to_string(), vec!["rust".to_string()])]);
        assert!(
            prompt.contains(".codescout/system-prompt.md") && prompt.contains("create_file"),
            "synthesis prompt must instruct a direct create_file write to the root system-prompt.md"
        );
        assert!(
            !prompt.contains("topic=\"system-prompt\", content"),
            "synthesis prompt must NOT instruct memory(write, topic=\"system-prompt\", content=...)"
        );
    }

    #[test]
    fn workspace_prompt_requires_six_memories_per_project() {
        let workspace = load_prompt("workspace_onboarding_prompt.md");
        assert!(
            workspace.contains("6 memories"),
            "workspace subagent prompt must require 6 memories per project"
        );
        for topic in [
            "project-overview",
            "architecture",
            "conventions",
            "development-commands",
            "domain-glossary",
            "gotchas",
        ] {
            assert!(
                workspace.contains(topic),
                "workspace prompt missing topic name: {topic}"
            );
        }
    }

    #[test]
    fn onboarding_prompt_uses_include_marker() {
        // The raw file (pre-substitution) must have the marker
        let raw = RAW_ONBOARDING_PROMPT;
        assert!(
            raw.contains("{{include: memory-templates.md}}"),
            "onboarding_prompt.md must contain the include marker"
        );
        // After load_prompt, marker is replaced by template content
        let loaded = load_prompt("onboarding_prompt.md");
        assert!(!loaded.contains("{{include:"));
        assert!(loaded.contains("### project-scope: project-overview"));
    }

    #[test]
    fn onboarding_prompt_phase_0_has_stable_heading_marker() {
        let raw = RAW_ONBOARDING_PROMPT;
        assert!(
            raw.contains("STABLE-HEADING"),
            "Phase 0 must carry a STABLE-HEADING comment to prevent cross-prompt drift"
        );
    }
    #[test]
    fn workspace_phase_0_reference_resolves() {
        let single = load_prompt("onboarding_prompt.md");
        let workspace = load_prompt("workspace_onboarding_prompt.md");
        let referenced = "## Phase 0: Embedding Model Selection";
        if workspace.contains(referenced) {
            assert!(
                single.contains(referenced),
                "workspace prompt references heading missing from single-project prompt"
            );
        }
    }

    #[test]
    fn rendered_server_instructions_contains_no_deprecated_tool_names() {
        let status = ProjectStatus {
            name: "x".into(),
            path: "/tmp/x".into(),
            languages: vec![
                "rust".into(),
                "python".into(),
                "typescript".into(),
                "kotlin".into(),
                "go".into(),
            ],
            memories: vec![],
            has_index: false,
            system_prompt: None,
            workspace: None,
            worktree: None,
        };
        let rendered = build_server_instructions(Some(&status));
        for &dead in DEPRECATED_TOOL_NAMES {
            assert!(
                !rendered.contains(dead),
                "rendered server instructions contains deprecated tool name: {dead}"
            );
        }
    }

    /// Tool names that have been removed/renamed and must never appear in any
    /// prompt surface the model reads. Single source of truth, shared by the
    /// rendered-`server_instructions` gate above and the `CLAUDE.md` gate below.
    const DEPRECATED_TOOL_NAMES: &[&str] = &[
        "find_symbol",
        "list_symbols",
        "replace_symbol",
        "insert_code",
        "rename_symbol",
        "search_pattern",
        "read_markdown",
        // Retired by the 2026-09-02 tool-surface collapse. `read_markdown` above
        // went in Task 7; these five are Tasks 4-6 and 8.
        "edit_markdown",
        "artifact_augment",
        "artifact_event",
        "artifact_refresh",
        // `artifact(` WITH the paren, never the bare word. The tool was renamed to
        // `doc`, but "artifact" remains the correct noun for the thing it manages and
        // survives legitimately in prose ("a librarian-managed artifact"), in Rust
        // paths (`crate::librarian::catalog::artifact`), in SQL (the `artifact`
        // table) and in every `artifact_id` field name. A bare-word entry would make
        // this gate unsatisfiable without lying about the domain; the paren catches
        // exactly the call form and nothing else.
        "artifact(",
    ];

    /// `CLAUDE.md` is injected into every session as a `<system-reminder>` but is
    /// NOT one of the surfaces scanned by the rendered-instructions gate above
    /// or by `prompt_surfaces_reference_only_real_tools`. It is prose, so an
    /// allowlist guard is unusable here — denylist the known-dead names instead.
    /// (See refactor-log F-9 / F-10.)
    #[test]
    fn claude_md_contains_no_deprecated_tool_names() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/CLAUDE.md");
        let claude_md =
            std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
        for &dead in DEPRECATED_TOOL_NAMES {
            assert!(
                !claude_md.contains(dead),
                "CLAUDE.md references deprecated tool name: {dead}"
            );
        }
    }
    /// Retired tool names must not survive as CALL FORMS in the reader-facing docs.
    ///
    /// **This gate exists because the sweep that created it was unfalsifiable.** Task 10 of the
    /// 2026-09-02 tool-surface collapse rewrote 160 mentions across 35 files by hand, and
    /// nothing could have told anyone whether it finished: `tests/doc_tool_refs.rs` is blind
    /// here two independent ways — its `a_documented_call_names_a_live_tool` skips every token
    /// without an underscore (`artifact` has none, so `artifact(` was never checked), and
    /// `present_tense_surfaces()` walks neither `docs/architecture/`, `docs/conventions/` nor
    /// `docs/adrs/`, which between them held 60 of the 160. That is `IC-11` exactly: a
    /// hand-enumerated sweep reports the surfaces it changed, never the ones it missed.
    ///
    /// **Call forms only, and that restriction is the whole design.** A denylist over bare
    /// names is unusable on this corpus, because a doc legitimately NAMES a retired tool in
    /// four different ways that all have to keep working:
    ///
    /// - historical migration records — `docs/manual/src/concepts/librarian-tools-collapse.md`
    ///   documents a *previous* collapse, in `artifact_event {action: "create"}` brace form;
    /// - dated measurements — "Probed 2026-08-28: … returns *not found*";
    /// - Rust paths that still resolve — `src/tools/markdown/read_markdown.rs` still exists;
    ///   only its `impl Tool` went;
    /// - raw usage data — `src/prompts/shape_census.txt` counts calls made before the rename.
    ///
    /// A call form (`name(`) is none of those. It is a claim that this is how you invoke the
    /// tool today, which is exactly the claim the collapse falsified.
    ///
    /// **The memory store was added 2026-09-03, and its absence is the lesson this gate
    /// keeps re-learning.** The five original roots were chosen by asking "which doc
    /// directories did the sweep touch?". The right question is **how often does an agent
    /// read this without being asked to** — and by that measure `.codescout/memories/` beats
    /// every root above it: 42 tracked files, named in every session-start banner, with the
    /// model instructed to read them before exploring. `project-overview.md` was still
    /// enumerating the pre-collapse tool inventory a day after the rename shipped with five
    /// gates green, which is the worst possible file to be wrong: a tool inventory's entire
    /// purpose is to be believed about which tools exist. Nothing here was hard to find; it
    /// was outside the population, and a population is not falsified by the corpus it omits.
    ///
    /// `CHANGELOG.md` is excluded: its released sections are a historical record by
    /// construction, and its Unreleased section documents the rename by naming both sides.
    #[test]
    fn reader_docs_contain_no_retired_call_forms() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        // Every retired name, as the caller would write it. `artifact(` carries the paren for
        // the same reason DEPRECATED_TOOL_NAMES does: "artifact" is still the correct noun.
        const RETIRED_CALLS: &[&str] = &[
            "artifact(",
            "artifact_event(",
            "artifact_augment(",
            "artifact_refresh(",
            "read_markdown(",
            "edit_markdown(",
        ];
        const ROOTS: &[&str] = &[
            "docs/manual/src",
            "docs/architecture",
            "docs/conventions",
            "docs/adrs",
            "src/prompts/guides",
            // The memory store, added 2026-09-03 — see the "read frequency" note above.
            // One root covers all three tiers: `.codescout/memories/` (42 tracked files),
            // `.codescout/projects/*/memories/` (per-sub-project), and, on a developer's
            // machine only, the gitignored `.codescout/private-memories/`. That last tier
            // is why this is one root rather than three: it has ZERO tracked files, so a
            // per-root non-vacuity assertion naming it would fail on every clone and in CI.
            // Folded in here, it is scanned when present and costs nothing when absent.
            ".codescout",
        ];
        const FILES: &[&str] = &[
            "CLAUDE.md",
            "README.md",
            "CONTRIBUTING.md",
            "docs/TAXONOMY.md",
            "docs/PROGRESSIVE_DISCOVERABILITY.md",
            "docs/PROBES.md",
            "docs/RELEASE.md",
            "src/prompts/source.md",
            "src/prompts/README.md",
            // The bug-file template, and ONLY the template — never `docs/issues/` as a root.
            // A bug file quotes retired call forms as evidence (the 2026-09-03 filing that
            // produced this gate quotes `artifact(get)` six times), so the directory is a
            // historical record and rooting it here would red permanently. The template is
            // the opposite: it is prescriptive, copied verbatim into every new bug file, so
            // a stale call form in it propagates itself once per bug.
            "docs/issues/_TEMPLATE.md",
        ];

        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "md") {
                    out.push(p);
                }
            }
        }

        let mut paths: Vec<std::path::PathBuf> = Vec::new();
        for r in ROOTS {
            let before = paths.len();
            walk(&root.join(r), &mut paths);
            // PER-ROOT non-vacuity, and it is deliberately not a total.
            //
            // A single `paths.len() > N` floor decays: measured 2026-09-03, renaming ONE of the
            // five roots away left 39 files against a floor of 40, so it caught that loss by a
            // single file — and would stop catching it the moment the corpus grew. This asserts
            // the property that actually matters and does not drift: every root contributes.
            assert!(
                paths.len() > before,
                "ROOTS entry '{r}' matched no .md files — the scan is not reading what it claims. \
             A renamed or moved directory silently empties this gate, and both assertions \
             below are `is_empty()`, which is monotone under removal: an empty corpus \
             produces exactly the silence they assert."
            );
        }
        for f in FILES {
            let p = root.join(f);
            if p.exists() {
                paths.push(p);
            }
        }

        let mut bad: Vec<String> = Vec::new();
        for p in &paths {
            let Ok(text) = std::fs::read_to_string(p) else {
                continue;
            };
            let rel = p.strip_prefix(root).unwrap_or(p).display().to_string();
            for (n, line) in text.lines().enumerate() {
                for &call in RETIRED_CALLS {
                    if line.contains(call) {
                        bad.push(format!("  {rel}:{}  `{call}`  {}", n + 1, line.trim()));
                    }
                }
            }
        }

        assert!(
            bad.is_empty(),
            "{} retired call form(s) in reader-facing docs:\n{}\n\n\
         These say \"this is how you invoke the tool\", and the tool does not exist. \
         `artifact(` → `doc(`; `artifact_event(action=\"list\")` → \
         `doc(action=\"event_list\")`; `artifact_augment(id, params=…)` → \
         `doc(action=\"augment\", id, augment={{params: …}})`; \
         `artifact_refresh(action=\"gather\")` → `doc(action=\"gather\")`; \
         `read_markdown(` → `read_file(`; `edit_markdown(` → `edit_file(`.\n\n\
         To MENTION a retired form (a translation note, a migration record), write it \
         without the paren — `read_markdown`, not `read_markdown(`. That is the escape \
         hatch, and it is deliberate: this gate is about invocation claims, not vocabulary.",
            bad.len(),
            bad.join("\n")
        );
    }

    /// Retired tool names must not survive as CALL FORMS in RUNTIME strings — the text
    /// codescout hands an agent while it runs.
    ///
    /// **This is the surface the other gates cannot reach, and it is the one an agent reads
    /// most often.** `reader_docs_contain_no_retired_call_forms`,
    /// `claude_md_contains_no_deprecated_tool_names`,
    /// `guide_bodies_contain_no_deprecated_tool_names`,
    /// `prompt_surfaces_reference_only_real_tools` and
    /// `companion_surfaces_reference_only_real_tools` all walk PROSE. Not one walks a `hint`,
    /// an error message, or a `next_step` literal. Measured 2026-09-03, a day after the
    /// collapse landed with five gates green: `src/librarian/tools/find.rs` was still
    /// appending `[snippet truncated — read the span with artifact(get)]` to every truncated
    /// semantic snippet, and naming `artifact(action="get", id=…)` in `cap_suppressed_hint`.
    /// Both were seen in LIVE `doc(action="find")` output before they were found in source —
    /// the binary was telling agents to call a tool it had itself stopped registering.
    ///
    /// **Two cuts define the population, and each has a case that forced it.**
    ///
    /// - **From the first `#[cfg(test)]` onward is dropped.** A test literal is not a string
    ///   any agent receives. `src/cli/format.rs:506` is why the cut must exist: it holds
    ///   `"Call artifact_refresh(id) on each item …"` as an INERT fixture, annotated on the
    ///   line as pinning nothing about the runtime, while the real string at
    ///   `src/librarian/tools/refresh_stale.rs:91` already reads `doc(action="gather", …)`.
    ///   Without the cut this gate would demand a change that fixture's own comment forbids.
    ///   `tests.rs` files are skipped wholesale for the same reason — being included through
    ///   a `#[cfg(test)] mod`, they carry no marker of their own for the cut to find.
    /// - **Comment lines are skipped, and the cost is named here rather than hidden.**
    ///   Archaeology legitimately lives in comments: a note recording what a call USED to
    ///   look like is correct prose. The price is that a stale `///` naming `artifact(find,
    ///   …)` will NOT red this gate. Three such comments were corrected by hand when it was
    ///   written (`find.rs:762`, `find.rs:919`, `librarian/catalog/chunk.rs:36`); nothing
    ///   here stops a fourth appearing.
    ///
    /// **The three exclusions below are `artifact(`'s alone.** The other five retired names
    /// collide with nothing in this tree. `artifact` collides with three live things at once,
    /// and every exclusion was derived by RUNNING the scan and reading what it flagged rather
    /// than by predicting it — the unfiltered scan returned 27 hits, of which 25 were correct
    /// code:
    ///
    /// - **word boundary** — `embed_artifact(`, `list_for_artifact(`,
    ///   `timeline_for_artifact(`, `is_main_checkout_artifact(` are live functions. This one
    ///   exclusion removes 20 of the 27.
    /// - **`artifact(s)`** — English pluralisation, which four runtime messages in this crate
    ///   already use (`"{n} artifact(s) without a vector"`).
    /// - **`ON artifact(` / `REFERENCES artifact(`** — SQL DDL naming the `artifact` TABLE,
    ///   which the collapse did not rename and must not.
    ///
    /// The `find.rs` line floor below is a PER-MEMBER assertion on purpose. A total-lines
    /// floor is monotone under a `#[cfg(test)]` moving up in any one file: coverage of that
    /// file would vanish while the total stayed comfortable. `find.rs` is the member that
    /// actually held the defect, so it is the one whose coverage is asserted by name.
    #[test]
    fn runtime_strings_contain_no_retired_call_forms() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        const RETIRED_CALLS: &[&str] = &[
            "artifact(",
            "artifact_event(",
            "artifact_augment(",
            "artifact_refresh(",
            "read_markdown(",
            "edit_markdown(",
        ];
        // The file that held the two live defects. Its `#[cfg(test)]` sits at ~1107, so a
        // healthy scan reads ~1100 lines of it; 900 leaves room for ordinary edits while
        // still failing loudly if the cut migrates to the top of the file.
        const WITNESS: &str = "src/librarian/tools/find.rs";
        const WITNESS_MIN_LINES: usize = 900;

        fn is_word(c: char) -> bool {
            c.is_alphanumeric() || c == '_'
        }

        fn walk_rs(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk_rs(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs")
                    && p.file_name().is_some_and(|n| n != "tests.rs")
                {
                    out.push(p);
                }
            }
        }

        let mut paths: Vec<std::path::PathBuf> = Vec::new();
        walk_rs(&root.join("src"), &mut paths);
        assert!(
            paths.len() > 100,
            "the scan found only {} .rs file(s) under src/ — it is not reading what it \
             claims. The violation assertion below is `is_empty()`, which is monotone under \
             removal: an empty corpus produces exactly the silence it asserts.",
            paths.len()
        );

        let mut bad: Vec<String> = Vec::new();
        let mut witness_lines = 0usize;

        for p in &paths {
            let Ok(text) = std::fs::read_to_string(p) else {
                continue;
            };
            let rel = p
                .strip_prefix(root)
                .unwrap_or(p)
                .display()
                .to_string()
                .replace('\\', "/");
            let mut scanned = 0usize;
            for (n, line) in text.lines().enumerate() {
                if line.contains("#[cfg(test)]") {
                    break;
                }
                scanned += 1;
                let t = line.trim_start();
                if t.starts_with("//") || t.starts_with('*') {
                    continue;
                }
                for &call in RETIRED_CALLS {
                    let mut from = 0usize;
                    while let Some(hit) = line[from..].find(call) {
                        let idx = from + hit;
                        let boundary =
                            idx == 0 || !line[..idx].chars().next_back().is_some_and(is_word);
                        let pre = line[..idx].trim_end();
                        let post = &line[idx + call.len()..];
                        let plural = post.starts_with("s)");
                        let sql = pre.ends_with("ON") || pre.ends_with("REFERENCES");
                        if boundary && !plural && !sql {
                            bad.push(format!("  {rel}:{}  `{call}`  {}", n + 1, line.trim()));
                            break;
                        }
                        from = idx + 1;
                    }
                }
            }
            if rel == WITNESS {
                witness_lines = scanned;
            }
        }

        assert!(
            witness_lines >= WITNESS_MIN_LINES,
            "the witness file `{WITNESS}` contributed only {witness_lines} non-test line(s) \
             (expected >= {WITNESS_MIN_LINES}). Either it moved, or a `#[cfg(test)]` migrated \
             above its runtime strings — in which case this gate silently stopped covering \
             the exact file whose two live defects it was written for."
        );

        assert!(
            bad.is_empty(),
            "{} retired call form(s) in RUNTIME strings — text codescout hands an agent:\n{}\n\n\
             An agent that follows one of these emits a call to a tool absent from its own \
             tools/list, and gets a hard unknown-tool error. `artifact(` → `doc(`; \
             `artifact_event(` → `doc(action=\"event_create\"|\"event_list\")`; \
             `artifact_augment(` → `doc(action=\"augment\")`; `artifact_refresh(` → \
             `doc(action=\"gather\"|\"list_stale\")`; `read_markdown(` → `read_file(`; \
             `edit_markdown(` → `edit_file(`.\n\n\
             Three escapes, all deliberate: SQL naming the `artifact` TABLE is exempt when \
             written `ON artifact(` or `REFERENCES artifact(`; English pluralisation \
             `artifact(s)` is exempt; and a historical note belongs in a comment, which this \
             gate does not read.",
            bad.len(),
            bad.join("\n")
        );
    }

    /// The gate's four commands must stay in their documented order, because the
    /// order is load-bearing: the lean lane leaves a librarian-less binary in the
    /// shared `target/`, so ending on it arms a trap for the next session. See
    /// CLAUDE.md § Development Commands and
    /// `docs/issues/archive/2026-08-30-shared-target-dir-feature-clobber-reds-the-cli-tests.md`.
    ///
    /// Two traps this test is shaped around, both measured against CLAUDE.md on
    /// 2026-08-31 rather than reasoned about:
    ///
    /// 1. **Prefix collision.** `cargo test --workspace` is a prefix of
    ///    `cargo test --workspace --no-default-features`, so a bare-substring
    ///    `find()` for the default lane returns the LEAN lane's offset. An ordering
    ///    assertion built that way compares the lean lane against itself — `n < n`,
    ///    which fails on a correct file, or passes unconditionally if written the
    ///    other way round. Either way it never tests the order. The needles below
    ///    are backtick-DELIMITED; the closing backtick is what discriminates them.
    /// 2. **Repeated mentions.** File-wide, `cargo test --workspace` matches 4
    ///    times and the lean form 3, because the prose after the directive
    ///    discusses both at length. Any whole-file index read is therefore
    ///    arbitrary. So this scopes to the directive sentence FIRST, then asserts
    ///    order within that slice.
    ///
    /// Mutations it must die on, both demonstrated rather than assumed:
    /// swapping the last two commands (ordering assertion), and deleting the
    /// directive line outright (the `expect` on START) — two distinct failures,
    /// because "the gate line is missing" must never read as "the order is fine".
    #[test]
    fn claude_md_gate_lists_its_four_commands_in_the_load_bearing_order() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/CLAUDE.md");
        let claude_md =
            std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));

        // Scope first — see trap 2 above.
        const START: &str = "**Run `cargo fmt`";
        const END: &str = "before completing any task.**";

        let start = claude_md.find(START).unwrap_or_else(|| {
            panic!(
                "CLAUDE.md has no gate directive: expected a run beginning {START:?}. \
                 The gate is the contract every session pays on every task, so if it \
                 moved, move this test with it — do not delete it."
            )
        });
        let rest = &claude_md[start..];
        let end = rest.find(END).unwrap_or_else(|| {
            panic!("CLAUDE.md's gate directive begins with {START:?} but never reaches {END:?}")
        });
        let directive = &rest[..end];

        // Then order. Sequential cursor: each command must appear AFTER the
        // previous one, which is what makes this an ordering assertion rather than
        // a presence one. A presence check would survive the exact swap this exists
        // to catch.
        const GATE: [&str; 4] = [
            "`cargo fmt`",
            "`cargo clippy --workspace --all-targets --features local-embed -- -D warnings`",
            "`cargo test --workspace --no-default-features`",
            "`cargo test --workspace`",
        ];

        let mut cursor = 0usize;
        for needle in GATE {
            let offset = directive[cursor..].find(needle).unwrap_or_else(|| {
                panic!(
                    "gate directive does not list {needle} after byte {cursor}. \
                     The four commands must appear in GATE order, and the lean lane \
                     ({lean}) must come immediately before the default one. \
                     Directive as found: {directive:?}",
                    lean = GATE[2],
                )
            });
            cursor += offset + needle.len();
        }
    }

    /// The `get_guide` bodies are the fourth prose surface the model reads, and
    /// until now the only one with no drift gate at all:
    /// `prompt_surfaces_reference_only_real_tools` builds its `surfaces` list from
    /// exactly three entries — `server_instructions.md`, `onboarding_prompt.md`,
    /// and `build_system_prompt_draft` — and no guide body appears in it. A guide
    /// is auto-injected into the session on the first call that triggers its
    /// topic, so a stale tool name there reaches the model exactly like one in
    /// `server_instructions` would.
    ///
    /// Denylist, for the same reason as the `CLAUDE.md` gate above: these are
    /// prose. Measured 2026-08-16 — the ten bodies carry 179 distinct backticked
    /// snake_case tokens against ~30 real tools, so an allowlist would need ~150
    /// non-tool entries, and the two-way tripwire on that list would make every
    /// guide edit a maintenance event. That is the trade F-9 left undecided.
    ///
    /// Iterates `GUIDE_TOPICS` rather than a hand-written list, so an eleventh
    /// guide is covered the moment it is registered — the failure mode this whole
    /// gate exists to prevent.
    ///
    /// (I-7 in `docs/trackers/test-escape-hardening.md`; friction F-9 in
    /// `docs/trackers/archive/prompt-guide-refactor-session-log.md`.)
    #[test]
    fn guide_bodies_contain_no_deprecated_tool_names() {
        for &topic in crate::prompts::GUIDE_TOPICS {
            let body = crate::prompts::topic_body(topic).unwrap_or_else(|| {
                panic!("GUIDE_TOPICS lists '{topic}' but topic_body returned None")
            });
            for &dead in DEPRECATED_TOOL_NAMES {
                assert!(
                    !body.contains(dead),
                    "get_guide body '{topic}' references deprecated tool name: {dead}"
                );
            }
        }
    }

    // ---------- Y-C: surface roundtrip snapshots (gates I-01) ----------
    //
    // These tests pin the rendered output of the three prompt surfaces so the
    // I-01 refactor (consolidating into a single `source.md` template) can
    // prove zero content drift. Regenerate intentionally with:
    //   UPDATE_PROMPT_SNAPSHOTS=1 cargo test --lib prompt_surfaces

    fn fixture_path(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/prompt_surfaces")
            .join(name)
    }

    fn check_or_update_snapshot(name: &str, current: &str) {
        let path = fixture_path(name);
        if std::env::var("UPDATE_PROMPT_SNAPSHOTS").is_ok() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("create fixture dir");
            }
            std::fs::write(&path, current).expect("write fixture");
            eprintln!("updated snapshot: {}", path.display());
            return;
        }
        let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "missing snapshot `{}`: {e}\n\
                 Regenerate with: UPDATE_PROMPT_SNAPSHOTS=1 cargo test --lib prompt_surfaces",
                path.display()
            )
        });
        if expected != current {
            panic!(
                "prompt surface drift in `{name}`\n  \
                 expected: {} bytes\n  \
                 actual:   {} bytes\n\n\
                 If intentional, regenerate with:\n\
                 \x20 UPDATE_PROMPT_SNAPSHOTS=1 cargo test --lib prompt_surfaces\n\n\
                 Otherwise this is a regression — I-01 (and any later prompt-template\n\
                 refactor) must preserve rendered content byte-for-byte.",
                expected.len(),
                current.len()
            );
        }
    }

    #[test]
    fn prompt_surfaces_server_instructions_snapshot() {
        check_or_update_snapshot("server_instructions.md", SERVER_INSTRUCTIONS);
    }

    #[test]
    fn prompt_surfaces_onboarding_snapshot() {
        check_or_update_snapshot("onboarding_prompt.md", RAW_ONBOARDING_PROMPT);
    }

    #[test]
    fn prompt_surfaces_system_prompt_draft_empty_snapshot() {
        let draft = crate::prompts::builders::build_system_prompt_draft(&[], &[], None, None, &[]);
        check_or_update_snapshot("build_system_prompt_draft_empty.md", &draft);
    }
}

#[cfg(test)]
mod redesign_invariants {
    use super::*;

    /// Maximum **character** length of the static `server_instructions` slice
    /// (`build_server_instructions(None)`).
    ///
    /// Sits below `CLIENT_INSTRUCTIONS_CHAR_LIMIT - CHANNEL_SAFETY_MARGIN` (2000) so the
    /// dynamic `## Project Status` block still has room to arrive. `fit_dynamic_block`
    /// guarantees the *total* never exceeds the channel; this budget is what keeps the
    /// dynamic half from being trimmed to nothing on every session.
    ///
    /// Two things about this constant were wrong before 2026-08-16 and are worth stating
    /// so they are not re-introduced. It was **2200**, above the measured 2048-char
    /// cliff — a cap set past the edge it exists to protect. And it was compared against
    /// `String::len()`, which counts **bytes**: the same slice measures 2127 bytes and
    /// 2081 chars, because the surface is dense with em-dashes and arrows. The gate was
    /// green throughout and the surface shipped truncated the whole time.
    ///
    /// If you need to add content, author a `get_guide(topic)` entry and reference it
    /// from the slice — do not raise this number.
    const STATIC_SLICE_CHAR_BUDGET: usize = 1900;

    #[test]
    fn source_md_under_cap() {
        let rendered = build_server_instructions(None);
        let chars = rendered.chars().count();
        assert!(
            chars <= STATIC_SLICE_CHAR_BUDGET,
            "static server instructions are {chars} chars ({} bytes); budget is {}. \
             Cut content or move it to get_guide — do not raise the budget. \
             NOTE the unit: the client cuts at {} CHARACTERS, not bytes.",
            rendered.len(),
            STATIC_SLICE_CHAR_BUDGET,
            crate::prompts::CLIENT_INSTRUCTIONS_CHAR_LIMIT,
        );
    }

    /// The gate the old one was not.
    ///
    /// `source_md_under_cap` measures `build_server_instructions(None)` — but **every**
    /// production call passes `Some(&status)`, which appends the whole Project Status
    /// block. Measured 2026-08-16: the bare render was 2127 bytes and passed the old
    /// 2200-byte cap, while the render that actually ships was 2350. The green test was
    /// measuring a string nobody receives.
    ///
    /// This is R-86's shape — *name every deployment mode the component has and ask
    /// which one the test constructed and which one production runs* — reached
    /// independently, from a cap rather than an LSP transport.
    #[test]
    fn production_render_fits_the_client_channel() {
        let status = ProjectStatus {
            name: "codescout".into(),
            path: "/home/marius/work/claude/codescout".into(),
            // Deliberately hostile: a real repo carries ~18 memory topics, and the list
            // is the one part of this block that grows without bound.
            languages: vec!["rust".into(), "kotlin".into(), "python".into()],
            memories: (0..30)
                .map(|i| format!("memory-topic-number-{i}"))
                .collect(),
            has_index: true,
            system_prompt: Some("A long custom instruction block. ".repeat(20)),
            workspace: None,
            worktree: None,
        };

        let rendered = build_server_instructions(Some(&status));
        let chars = rendered.chars().count();
        let ceiling =
            crate::prompts::CLIENT_INSTRUCTIONS_CHAR_LIMIT - crate::prompts::CHANNEL_SAFETY_MARGIN;
        assert!(
            chars <= ceiling,
            "production render is {chars} chars; the client cuts at {}. \
             The dynamic block must be trimmed to fit, never allowed to push the \
             static slice over the cliff.",
            crate::prompts::CLIENT_INSTRUCTIONS_CHAR_LIMIT,
        );

        // The static slice must survive INTACT — its last line is precisely what the
        // client used to cut, and losing it costs the model every `get_guide` pointer.
        assert!(
            rendered.contains("\"symbol-navigation\""),
            "the final static line must arrive whole; it is the one that was being cut"
        );
        assert!(
            rendered.contains("status trimmed"),
            "a trim must announce itself — the whole defect was that the loss was silent"
        );
    }

    /// A status block that fits must not be touched: the trim exists for the overflow
    /// case, and a note on every session would be noise that teaches nothing.
    ///
    /// Retargeted for the tier split — the memories line it used to look for in the
    /// rendered instructions now rides the response channel. Both halves are asserted so
    /// the test still distinguishes "nothing was trimmed" from "nothing was rendered".
    #[test]
    fn a_status_block_that_fits_is_left_alone() {
        let status = ProjectStatus {
            name: "x".into(),
            path: "/tmp/x".into(),
            languages: vec!["rust".into()],
            memories: vec!["architecture".into(), "conventions".into()],
            has_index: true,
            system_prompt: None,
            workspace: None,
            worktree: None,
        };
        let rendered = build_server_instructions(Some(&status));
        assert!(
            rendered.contains("- **Active project:** x at `/tmp/x`\n"),
            "{rendered}"
        );
        assert!(
            !rendered.contains("status trimmed"),
            "no trim note when nothing was trimmed"
        );
        let block = build_status_response_block(&status).expect("substitutable block");
        assert!(
            block.contains("- **Memories:** architecture, conventions\n"),
            "{block}"
        );
    }

    /// What the tier split actually bought, pinned as a number rather than described.
    ///
    /// Before the split a realistic render consumed the whole 2000-char usable budget and
    /// trimmed; the `Substitutable` segments were the first casualties. With them re-homed
    /// to the tool response, only the anchors ride the persistent channel, and what is left
    /// over is the room any future addition to `server_instructions` has to work with.
    ///
    /// The floor is deliberately loose — it exists to catch a REGRESSION (someone growing
    /// the static slice back into this space), not to pin an exact figure that would churn
    /// on every wording change.
    #[test]
    fn the_tier_split_leaves_real_headroom_in_the_persistent_channel() {
        let base = ProjectStatus {
            name: "codescout".into(),
            path: "/home/marius/work/claude/codescout".into(),
            languages: vec!["rust".into(), "markdown".into()],
            memories: (0..18).map(|i| format!("memory-topic-{i}")).collect(),
            has_index: true,
            system_prompt: None,
            workspace: None,
            worktree: None,
        };
        let usable =
            crate::prompts::CLIENT_INSTRUCTIONS_CHAR_LIMIT - crate::prompts::CHANNEL_SAFETY_MARGIN;

        let plain = build_server_instructions(Some(&base));
        assert!(
            !plain.contains("trimmed"),
            "a perfectly ordinary project must no longer trim anything:\n{plain}"
        );
        let plain_free = usable - plain.chars().count();

        let wt = ProjectStatus {
            worktree: Some(WorktreeInfo {
                branch: Some("experiments".into()),
                main_repo: Some(std::path::PathBuf::from(
                    "/home/marius/work/claude/codescout",
                )),
                name: Some("bench".into()),
            }),
            ..base
        };
        let with_wt = build_server_instructions(Some(&wt));
        let wt_free = usable - with_wt.chars().count();

        assert!(
            wt_free >= 120,
            "the worst ordinary case (worktree banner present) must keep >=120 chars free; \
             got {wt_free} (plain: {plain_free}). If this fails because the static slice \
             grew, that is the regression — move the addition to a guide."
        );
    }

    /// B-9 regression
    /// (docs/issues/archive/2026-08-15-iron-laws-detail-guide-claims-cat-on-source-is-allowed.md):
    /// the guide once claimed `cat src/foo.rs` was "allowed on bounded files" —
    /// measured 0/10 unaided survival against that sentence
    /// (prompt-engineering scenarios/conclude-last, trap t2). The phantom
    /// carve-out must never regrow.
    ///
    /// The original fix over-corrected, and this test used to pin the
    /// over-correction: "by path, not by command". That was false the day it was
    /// written — `check_source_file_access` is a TWO-part predicate, a
    /// content-reader command name AND a source extension. The framing then
    /// outlived `wc`'s removal from the list (2026-08-16) and left the guide
    /// telling the agent that `wc` and `ls` were blocked when neither was.
    /// Guarding against both false sentences is this test's job; keeping the
    /// command list itself honest is
    /// `iron_laws_detail_gate_names_every_blocked_command`'s.
    #[test]
    fn iron_laws_detail_never_regrows_the_bounded_file_carve_out() {
        let body = crate::prompts::topic_body("iron-laws-detail").expect("guide registered");
        assert!(
            !body.contains("allowed on bounded files"),
            "B-9 false claim resurfaced: the shell gate has no bounded-file carve-out"
        );
        assert!(
            !body.contains("by path, not by command"),
            "the gate is a content-reader command name AND a source extension; \
             describing it as path-only is what kept `wc` and `ls` documented as \
             blocked after they stopped being"
        );
        assert!(body.contains("acknowledge_risk: true"));
    }

    /// The guide prints the blocked-command list to the agent, so that list and
    /// `SOURCE_ACCESS_COMMANDS` are one contract with two copies. Deriving the
    /// assertion from the constant is what makes the next edit to the gate fail
    /// the build until the guide follows: when `wc` came off the list on
    /// 2026-08-16 the guide went on naming it for a day, and nothing noticed
    /// because no test read both.
    #[test]
    fn iron_laws_detail_gate_names_every_blocked_command() {
        let body = crate::prompts::topic_body("iron-laws-detail").expect("guide registered");
        for cmd in crate::util::path_security::SOURCE_ACCESS_COMMANDS {
            assert!(
                body.contains(&format!("`{cmd}`")),
                "iron-laws-detail never names `{cmd}`, which the gate blocks"
            );
        }
        // The complement carries the same weight. These return a measurement OF
        // the content rather than the content, so they are deliberately absent —
        // and the guide documents them as usable. Re-adding one silently
        // falsifies that half.
        for cmd in ["wc", "ls", "stat", "du", "file"] {
            assert!(
                !crate::util::path_security::SOURCE_ACCESS_COMMANDS.contains(&cmd),
                "`{cmd}` is blocked again — the guide lists it as an allowed \
                 metadata command, so update that paragraph in the same commit"
            );
        }
    }

    /// BL-26 regression: `doc(action="move")` mints a NEW id — catalog
    /// identity is `sha256(abs_path)`, so a move cannot preserve it. What makes
    /// `move` the right call over delete+recreate is that it *grafts* the
    /// augmentation, events, links and observations onto the new id.
    ///
    /// The commit that made the graft work (`2d8c7f39`) corrected three of the
    /// four guide surfaces carrying the opposite claim and missed
    /// `librarian-runtime.md`. Scanning **every** registered guide, rather than the
    /// one that happened to drift, is the part that stops a fourth copy — the same
    /// reasoning as guarding every `edit_file` write path instead of the one that
    /// was reported.
    #[test]
    fn no_guide_claims_a_move_preserves_the_id() {
        for &topic in crate::prompts::GUIDE_TOPICS {
            let body = crate::prompts::topic_body(topic).expect("guide registered");
            for phrase in ["preserves `id`", "preserves the `id`", "preserves id"] {
                assert!(
                    !body.contains(phrase),
                    "guide '{topic}' says a move {phrase} — it mints a new one and grafts \
                     the history onto it. Say that instead; the response carries \
                     previous_id / id_changed."
                );
            }
        }
    }

    /// The `librarian-runtime` guide denied the augmentation sidecar for a full day
    /// after `e799f29d` shipped it, because the same-day corrective sweep
    /// (`e1b91221`) named *three* places and this was the fourth. That is the
    /// identical mechanism, in the identical guide section, that
    /// `no_guide_claims_a_move_preserves_the_id` above exists for — two separate
    /// three-place sweeps have now each missed this one file. Scanning every
    /// registered guide is what generalises past the copy that happened to drift.
    ///
    /// **The absence half alone would be monotone under removal:** deleting the
    /// section satisfies it exactly as a correct section does. The positive half is
    /// what lets this test tell the two apart — it fires if the mechanism stops
    /// being documented at all, which no `!contains` can detect.
    #[test]
    fn no_guide_denies_the_augmentation_sidecar() {
        for &topic in crate::prompts::GUIDE_TOPICS {
            let body = crate::prompts::topic_body(topic).expect("guide registered");
            for phrase in ["there is no on-disk representation", "local-only by design"] {
                assert!(
                    !body.contains(phrase),
                    "guide '{topic}' denies the augmentation sidecar ('{phrase}'). Shape has \
                     travelled since 2026-08-30 via docs/augmentations/ plus an \
                     `expects_augmentation:` declaration; only `params` stay catalog-only. \
                     A sentence saying a capability does not exist ends the search that \
                     would have found it."
                );
            }
        }

        let runtime = crate::prompts::topic_body("librarian-runtime").expect("guide registered");
        for token in ["expects_augmentation", "sidecar"] {
            assert!(
                runtime.contains(token),
                "librarian-runtime no longer mentions `{token}`, so the sidecar mechanism has \
                 gone undocumented — the state the absence assertions above are blind to."
            );
        }
    }

    #[test]
    fn every_iron_law_has_do_instead() {
        let rendered = build_server_instructions(None);
        // Iron Laws section uses "NEVER X → Y" format. Each NEVER line must
        // have an arrow on the same line or within the next 2 lines.
        for (i, line) in rendered.lines().enumerate() {
            if line.contains("NEVER ")
                || line.starts_with(|c: char| c.is_ascii_digit()) && line.contains("NEVER")
            {
                let next_two: String = rendered
                    .lines()
                    .skip(i)
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(" ");
                assert!(
                    next_two.contains("→")
                        || next_two.contains(" use ")
                        || next_two.contains(" do "),
                    "Iron Law without do-instead clause: '{}'",
                    line
                );
            }
        }
    }

    #[test]
    fn server_instructions_mentions_get_guide() {
        let rendered = build_server_instructions(None);
        assert!(
            rendered.contains("get_guide"),
            "system prompt must mention get_guide for discoverability"
        );
    }

    /// IL1's always-loaded text must NOT carry the overlap-condition clause. It was
    /// measured and **refuted** — hamsa **A-25**, `docs/trackers/prompt-hamsa-audit-log.md`.
    ///
    /// This guard used to assert the opposite. The history is the point, so read it before
    /// re-adding anything:
    ///
    /// - The gate refuses a source range whenever it overlaps a named symbol, and the
    ///   always-loaded wording "read_file is right for imports/glue" grants far more than
    ///   that. The deficit is real and large: 416 refusals across 89 sessions, 4.7 per
    ///   session, the largest single error class in the corpus.
    /// - So a 57-character clause was authored — "refused only when the range overlaps a
    ///   symbol; force=true reads it anyway" — deleted a day later by a budget refit
    ///   (`391fdcdc`), then restored and guarded here.
    /// - Then it was actually measured, 2026-08-18, 10 runs per arm on pinned sonnet with
    ///   the ship rule pre-registered before either arm ran. Base arm: **10/10** planned a
    ///   bare line-range read over a symbol — deficit confirmed, far past its 3/10 bar.
    ///   Clause arm: **8/10 still did**. The rule required <= 1/10; 0/10 -> 2/10 passing is
    ///   Fisher p~0.47, noise.
    ///
    /// **Why it failed, which is the useful part:** the clause is *informational*, not
    /// *directive*. It states the gate's condition but supplies no procedure, and an agent
    /// asked for lines 40-55 cannot know whether they overlap a symbol without checking —
    /// so it adds a fact that cannot be acted on. The one arm-B run that passed is the
    /// tell: it called `symbol_at` to resolve exactly that unknown.
    ///
    /// A *directive* wording ("on a mid-file range, pass force=true or fetch the symbol by
    /// name") is a DIFFERENT intervention. It needs its own base arm and its own A-N row;
    /// it must not be smuggled in as a revision of A-25. Until such a row exists and ships,
    /// this test stands — and the deficit is addressed by CODE instead: the
    /// `start == 1 && end <= 60` head-read exemption plus the extent-ordered refusal hint,
    /// which together exempt 102 of 103 `start == 1` refusals and were never subject to the
    /// prompt-eval gate.
    #[test]
    fn il1_does_not_carry_the_refuted_overlap_clause() {
        let rendered = build_server_instructions(None);
        // Everything before Iron Law 2's marker is the header plus IL1.
        let il1 = rendered
            .split("2. NEVER")
            .next()
            .expect("server instructions must contain Iron Law 2");
        assert!(
            !il1.contains("overlaps a symbol"),
            "Iron Law 1 carries the overlap-condition clause that hamsa A-25 measured and \
             REFUTED (base 10/10 vs clause 8/10 planning the refused read, ship rule was \
             <= 1/10). Re-adding it needs a new pre-registered A-N row, not a re-reading of \
             A-25 — and if the wording changed, make it DIRECTIVE rather than \
             informational, which is what A-25 found lacking. IL1 read:\n{il1}"
        );
        // The escape hatch itself is NOT what was refuted — only the condition clause was.
        // Naming `force=true` costs nothing extra and predates the clause.
        assert!(
            il1.contains("force=true"),
            "Iron Law 1 must still name the escape hatch; A-25 refuted the CONDITION \
             clause, not the mention of force. IL1 read:\n{il1}"
        );
    }

    /// The quickref must NOT route `call_graph` or `tree`. Measured and **refuted** —
    /// hamsa **A-26**, `docs/trackers/prompt-hamsa-audit-log.md`.
    ///
    /// Same shape as the A-25 guard above, one day later, and read it before re-adding:
    ///
    /// - The motive was an ABSENCE, not a failure: `call_graph` = 0 calls across 26,705
    ///   calls in four projects. A null cannot separate *unrouted* from *never tempted*
    ///   from *substituted*, and only the first was tested.
    /// - Two routing lines shipped anyway (`ba16b16a`) with no base arm, which P-3 makes
    ///   binding for a `source.md`-derived surface.
    /// - Then it was measured, 2026-08-18, 10 runs per arm on pinned sonnet, ship rule
    ///   pre-registered. Base (no lines) **0/10** named `call_graph`; treatment (lines)
    ///   **0/10**; positive control (a MANDATORY directive) **10/10**. All twenty
    ///   base+treatment runs answered `references(...)` byte-identically, on a fixture
    ///   built so one hop provably cannot answer the question.
    ///
    /// **Why it failed, which is the useful part:** the line is a routing entry competing
    /// with an adjacent, emphasised, semantically overlapping neighbour — `Who calls X →
    /// references(symbol, path) — NOT grep` sits directly above it and already claims the
    /// question. *Naming* a tool does not displace a strong competing prior; the control
    /// shows what does — *contrasting* the two and forbidding the wrong one. The evidence
    /// points at **substituted**, not unrouted: the model reaches for `references`, whose
    /// one hop is the wrong tool here but the habitual one.
    ///
    /// A CONTRASTIVE wording (`Who calls X → references | transitively → call_graph`) is a
    /// DIFFERENT intervention. It needs its own base arm and its own A-N row; it must not
    /// be smuggled in as a revision of A-26. Until such a row exists and ships, this test
    /// stands. Scenario: `prompt-engineering` `scenarios/call-graph-routing/`.
    #[test]
    fn quickref_does_not_carry_the_refuted_call_graph_routing() {
        let rendered = build_server_instructions(None);
        let quickref = rendered
            .split("## Search/Edit decision quickref")
            .nth(1)
            .and_then(|s| s.split("\n## ").next())
            .expect("server instructions must contain the Search/Edit quickref");
        for refuted in ["call_graph", "tree("] {
            assert!(
                !quickref.contains(refuted),
                "the quickref routes `{refuted}`, which hamsa A-26 measured and REFUTED \
                 (base 0/10 vs treatment 0/10 naming call_graph, positive control 10/10 — \
                 the line moved nothing). Re-adding needs a new pre-registered A-N row, not \
                 a re-reading of A-26 — and if the wording changed, make it CONTRASTIVE \
                 against `references` rather than a bare mention, which is what A-26 found \
                 lacking. Quickref read:\n{quickref}"
            );
        }
        // `references` itself is NOT what was refuted — it predates A-26 and is the line
        // the model already follows. Only the added routing entries were.
        assert!(
            quickref.contains("references(symbol, path)"),
            "the quickref must still route `references`; A-26 refuted the ADDED lines, not \
             the existing one. Quickref read:\n{quickref}"
        );
    }

    #[test]
    fn server_instructions_does_not_concat_librarian() {
        // After Task 14 lands, the librarian block must not be appended.
        //
        // The needle used to be `doc(action="event_create")` — that literal substring
        // occurs NOWHERE in the tree (`git grep -c 'doc(action="event_create")' -- src/`
        // returns zero; the guide writes `doc(action="event_create", id="...", event={...})`,
        // comma not paren), so this assertion could never fire. Verified by committing the
        // regression it guards — concatenating `librarian.md` into
        // `build_server_instructions` — and watching the old assertion pass anyway.
        //
        // `<!-- serves: doc.event_create` is the guide's own machine-readable declaration
        // (`src/prompts/guides/librarian.md`, just above the `## doc — Event Log` heading)
        // and is stable under prose edits to the section around it.
        let rendered = build_server_instructions(None);
        assert!(
            !rendered.contains("<!-- serves: doc.event_create"),
            "librarian guide content should not be in instructions; \
                 move it to get_guide(\"librarian\")"
        );
    }

    #[test]
    fn librarian_instructions_const_removed() {
        // Sentinel: any reintroduction of `crate::librarian::INSTRUCTIONS`
        // must remove this test or re-add the const. The presence of this
        // no-op test documents the deletion intent.
    }
}
