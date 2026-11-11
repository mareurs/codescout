# librarian and librarian-runtime — the wording the guides no longer carry

`src/prompts/guides/librarian.md` and `librarian-runtime.md` are served to every repo. They keep
the contract of each tool, action and parameter. This file keeps what they shed on 2026-11-11.
The removed text is verbatim from commit `4f444dd51`
(`git show 4f444dd51:src/prompts/guides/librarian.md`, same for `librarian-runtime.md`).

## Editing the guides: what pins them

- **Scope: repo-neutral.** `no_guide_names_a_path_or_field_only_this_repo_has`
  (`src/prompts/mod.rs`) fails on a codescout-only path or field. `docs/trackers/` is not one:
  the server classifies `**/docs/trackers/**/*.md` as `kind=tracker` by default
  (`DEFAULT_RULES_TOML` in `src/librarian/classify.rs`).
- **Headings.** Rename none. Tests and probes key on them (`src/server.rs`,
  `src/prompts/guide_index.rs`, `scripts/probe_guide_section_use.py`), and
  `librarian-runtime` § *Trackers as cross-session behavior* is cited by `tracker-conventions`.
- **`serves:` lines.** `librarian.md` declares fourteen sections. Each must stay within
  `MAX_DECLARED_SECTION_BYTES` (2,500 B). `Reference` and `Filter Syntax` were 14 B and 71 B
  under the cap on 2026-11-11, so neither can grow. `librarian-runtime.md` declares none.
- **Pinned strings.** `Every artifact is a markdown file with YAML frontmatter`,
  `don't hand-maintain the table`, `Avoid this anti-pattern`,
  `**Refresh cycle** (run by the agent, not automatic)`, and the staging paragraph that starts
  `**Stage both halves: `git add -- <old> <new>`` (it must stay under 2,000 B and contain
  `column 1`, `similarit` and `stale`). `librarian-runtime` must still contain
  `expects_augmentation` and `sidecar`. No guide may say a move "preserves `id`".

## librarian § doctor repairs — what each `fix=` mode does

**Kept in the guide:** the dry-run rule, the scope rule and the table.

**Pre-cut text — three phrases and a paragraph:**

> Every mode WRITES and is a **dry run until `confirm=true`** — so reading this
> after your first call has cost you nothing, which is why it lives here rather
> than in the tool schema.

> Takes the same `scope` as the report, via one shared `DoctorScope`.

> Which params each mode needs is on `root` / `old_root` / `new_root` in the schema,
> because that is what you need to *form* the call.

**Pre-cut text — the `mint_slugs` row:**

> | `mint_slugs` | Backfills `artifact.slug` where NULL. **Corpus-wide — the one mode `scope` and `root` do not narrow**, deliberately (see its arm in `src/librarian/tools/doctor.rs`): it writes only machine-local catalog rows, never files, so one deterministic pass keeps slug assignment a function of the corpus rather than of whichever repo swept first. Expect its dry run to name OTHER repos' artifacts — that is the mode working, not a scope leak — and note a slug is immutable once minted. |

## librarian-runtime § Where catalog state lives

**Pre-cut text:** "(the test suite does this per-test for isolation)" after the `LIBRARIAN_DB`
override, and "(`sidecar_write_through`, `augment.rs`)" after the sidecar write-through.

## librarian-runtime § Per-project classifier overrides

**Kept in the guide:** the example, written for a neutral family (`embedded-docs`).

**Pre-cut text:**

> Example — flag everything codescout produces under its source tree, and
> rescue embedded template files that would otherwise classify as
> `unknown`:
>
> ```toml
> [[rule]]
> glob = "src/**/*.md"
> kind = "doc"
> tags = ["codescout"]
> ```
>
> Query the family back with array-membership (`contains`, not `in`):
>
> ```
> doc(action="find", filter={"tags": {"contains": "codescout"}})
> ```

## librarian-runtime § Trackers as cross-session behavior

**Kept in the guide:** one paragraph on the augmented tracker and the reflective tracker, and the
pointer to `get_guide("tracker-conventions")`. No Rust code handles the passover tracker (`grep -rli
passover src/` found only that guide's own pointer): it is a convention of the codescout-companion
plugin. The pointer in `tracker-conventions` § *Querying with the librarian* read "the session-passover pattern:" and
now reads "trackers that carry cross-session behavior:".

**Pre-cut text — the opening:**

> A skill tells an agent how to *act*. An augmented tracker — a standing
> `prompt` + `params` that travel with the artifact — tells an agent how to
> *maintain durable state*. The two are complementary: a skill shapes a
> single session's behavior; an augmented tracker shapes every session's
> behavior toward one artifact.
>
> A **reflective tracker** takes this further: its body *is* a behavioral
> script. The body is not prose about what to do — it is the executable
> specification the next session reads and follows. The tracker carries
> cross-session behavior the way a skill carries per-session behavior.

**Pre-cut text — `### Session-passover tracker`, the whole subsection:**

> The session-passover tracker is the canonical worked example. It is a
> reflective tracker tagged `passover` whose body encodes the handoff
> protocol: what state was in flight, what the next session must verify,
> and what to do first.
>
> **Discovery** — at the start of a new session, find any active passover
> tracker with:
>
> ```
> doc(action="find", kind="tracker", filter={"and":[
>   {"tags": {"in": ["passover"]}},
>   {"status": {"eq": "active"}}
> ]})
> ```
>
> **Resume protocol** — read the full body
> (`doc(action="get", id="<id>", full=true)`), work through its
> `## Next actions` checklist, verify the state claims, and resume the
> prior thread. The tracker *is* the cross-session behavior: following it
> is what continuity means.
>
> **Maintenance** — update the body at session end to reflect the current
> state and the next handoff. The `passover` tag + `active` status keeps
> it discoverable; archive it when the work-stream completes.
