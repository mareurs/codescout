# docs/media

A personal reference library for external media (videos, podcasts, articles) relevant to
codescout or its adjacent work — kept in this repo for convenience, but **explicitly not part of
codescout's tracked documentation system**: it doesn't carry `cluster/` bug tags, isn't scoped by
the librarian's `doc()` tracker machinery, and isn't swept by `audit_doc_refs` or the tracker
hygiene sweep. It's a curated notebook, not project documentation.

## Layout

- **`input/`** — notes on external material we found interesting or relevant: one file per
  source, named `YYYY-MM-DD-short-slug-notes.md`. Each file is a **summary + a small number of
  short, attributed quotes with timestamps** — deliberately *not* a full transcript. Full
  verbatim transcripts of commercial video/audio are a substantial reproduction of someone else's
  copyrighted work; a structured summary with a handful of illustrative quotes covers the same
  "build context / cite this later" purpose without that exposure, especially in a repo that could
  end up on a public remote.
- **`output/`** — drafts of things we're producing that *reference* the material in `input/`
  (tutorial sections, blog drafts, doc excerpts). Empty until something lands here.
- **`contacts.md`** — people encountered through this material who might be worth citing or
  linking to in docs/tutorials later.

## Adding a new source

1. Fetch metadata + auto-captions (see any `input/*.md` file's header for the yt-dlp-based
   approach used so far — no dependency added to the project itself; done ad hoc from a
   session's scratch directory).
2. Read the source, then write **only** a summary + short quotes to `input/`, not the raw
   transcript. Keep any raw/full-text intermediate scratch files outside this repo (session
   scratchpad, not committed).
3. Add a `### Name` entry to `contacts.md` for any person worth remembering.
