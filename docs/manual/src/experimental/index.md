# Experimental Features

> Features on the `experiments` branch, not yet released to `master` or
> crates.io. APIs and behaviour may change without notice. When a feature
> graduates to stable, its page moves into the main manual and its entry here is
> removed.

## Seeing what is actually in flight

This page was hand-maintained and went stale by seven weeks and 387 commits at
least once, so treat the summary below as a pointer, not the source of truth.
The mechanical answer is the commit range itself:

```bash
git log master..experiments --oneline
git diff master...experiments --stat
```

If that range is empty, nothing is in flight regardless of what this page says.

## Awaiting promotion

The cohort accumulated since v0.15.0 (2026-06-16) is documented in
[`CHANGELOG.md`](https://github.com/mareurs/codescout/blob/experiments/CHANGELOG.md)
under `[Unreleased]`, and each subsystem already has a permanent home in the
main manual rather than a temporary page here — follow the SUMMARY.md table of
contents. The headline additions:

| Feature | Manual page |
|---|---|
| Worktree overlay for the artifact catalog | [Worktree Overlay](../concepts/worktree-overlay.md) |
| Catalog GC, `rehome`, `prune_missing` (schema v10) | [Catalog GC & Repair](../concepts/catalog-gc.md) |
| `append_entry` + `entry_cite` (schema v9) | [Entry Citations](../concepts/entry-citations.md) |
| `librarian(action="link_scan")` | [link_scan](../concepts/link-scan.md) |
| `doc(action="graft")` | [artifact (action="graft")](../concepts/artifact-graft.md) |
| Constitution trackers + `codescout constitution-check` | [Constitution Trackers](../concepts/constitution-trackers.md) |
| `edit_file` miss diagnostics | [Miss Diagnostics](../tools/edit-markdown-miss-diagnostics.md) |
| `**Valid:**` decay classes + four `doctor` checks | [Statement Validity](../concepts/statement-validity.md) |

Writing the page into the main manual directly — rather than staging it here and
moving it on graduation — is deliberate: the move step is what got skipped, and a
page that documents a shipped-on-`experiments` feature is more useful in the
place readers already look.

## Previously graduated

- **v0.15.0 cohort** (shipped straight to stable): two-stack retrieval
  (daemon-free lite default plus opt-in server), the `peer` tool,
  `librarian(action="legibility_scan")`, Windows/EDR support.
- **2026-05-18 to 2026-05-24**: ripgrep-style text output, the goal-tracker
  archetype, `codescout artifact*` CLI subcommands,
  `librarian(action="audit_doc_refs")`, the `edit_file` frontmatter/`at`
  additions.
