# Artifact Cutover: `codescout cutover`

> ⚠ **Unreleased — on the `experiments` branch only.** Not in v0.15.0 and not on
> crates.io; the API may change without notice. The full cohort is listed under
> `[Unreleased]` in
> [CHANGELOG.md](https://github.com/mareurs/codescout/blob/experiments/CHANGELOG.md).

```text
codescout cutover plan    # dry run: print what would move, write nothing
codescout cutover export  # write the plan into the working tree
codescout cutover build   # build the new catalog from HEAD
```

The old catalog (`catalog.db`) holds facts that no committed file holds: events,
observations, manual links, augmentation shapes, and params. The new catalog
(`.codescout/derived.db`) reads committed text only. `codescout cutover` moves each of these
facts into a committed file. When it cannot move a fact, it names the fact in a
report. Then it builds the new catalog.

## The three subcommands, in order

1. **`cutover plan`** reads the old catalog and the committed files of `HEAD`. It
   prints the report. It writes nothing: not to the old catalog, not to the
   repository, not to a report file.
2. **`cutover export`** does the same, then writes the plan into the working tree.
   It makes one atomic write per file. It keeps the report beside the old catalog.
   You commit the changed files yourself.
3. **`cutover build`** builds `.codescout/derived.db` in this checkout from `HEAD`.
   It builds only when a plan of `HEAD` writes nothing. Run it after the commit.
   Git must ignore the file (see Build).

The usual sequence is:

```bash
codescout cutover plan                # read the report
codescout cutover export              # write the files, read the report again
git status --short                    # the files the report lists, no .codescout/write.lock*
git add <the files>; git commit       # you commit; the tool never does
codescout cutover plan                # the plan writes nothing
codescout cutover build               # .codescout/derived.db appears in this checkout
```

All three take the same options:

| Option | Meaning |
|---|---|
| `--old-db <file>` | The old catalog. Default: `LIBRARIAN_DB`, else `catalog.db` in the `librarian` directory of the local data directory. |
| `--old-root <path>` | Another spelling of the repository root that old rows carry. Absolute. Repeat it for each spelling. |
| `--project <path>` | The project directory. Default: the working directory. The git root at or above it is used. |
| `--json` | Print one JSON object instead of markdown. |

The plan is a function of `HEAD` and the old catalog. It does not read the working
tree. Uncommitted edits are invisible to it, so commit or stash first.

## Before the first run: extend the key space

Some subtrees hold files whose names another tool dictates: `AGENTS.md` and `SKILL.md`
for agents, `.github` templates, a book source, recorded evidence. The cutover renames
a file when its name gives no valid key, such as `My_Notes.md`. It must not rename a
file that another tool reads by name.

The report refuses such a rename, or it freezes the file (see below). Other names
cause trouble without a rename: `AGENTS.md` gives the valid key `agents`, and several
folders that hold one make the key collide.

In these cases the report tells you to extend `.codescout/key-space.toml`. Do it
before the run. List the subtree under `exclude`, commit the file, and run `cutover
plan` again. The plan reads `HEAD`, so an uncommitted change to the key space has no
effect.

```toml
exclude = [
    "contrib",
    "tests/fixtures",
]
```

A document in an excluded subtree has no key. The cutover leaves it alone.

## The migration window

The tool needs the old catalog to stay unchanged while it runs. Stop every codescout
process first: every MCP server, every hook, every other CLI run.

The tool refuses when a `-wal` or `-shm` file lies beside the old catalog. It also
refuses when a `-journal` file that is not empty lies there. One of these files
means another process holds the catalog open, or one stopped without closing it.
Run this command, then run the cutover again:

```bash
sqlite3 <old catalog file> 'PRAGMA wal_checkpoint(TRUNCATE)'
```

The tool checks four times:

1. when it opens the old catalog, for side files;
2. after it reads the rows, for side files;
3. after it reads the rows, for a change in the size or the modification time of the
   file;
4. for `export`, once more before the first write, for both.

A change that check 2, 3 or 4 finds gives the refusal `the old catalog changed while
it was read`. Stop the process that wrote and run the command again.

`export` also refuses in these cases:

- `.codescout/derived.db` already exists in this checkout. `export` runs before the
  build. Delete the file (git can rebuild it), then run `export` again.
- The project write lock is not free. Another codescout process writes to the
  project. The lock file is `.codescout/write.lock`, in the nearest directory (from
  the project up to the git root) that holds `.codescout/project.toml`. `export`
  creates this file and `.codescout/write.lock.holder` when they are not there. Both
  hold runtime state. Keep both out of git: add `.codescout/write.lock*` to
  `.gitignore`, and check `git status --short` before you commit.
- The lock file cannot be opened. A link or a directory at that path does this.

`export` tries the lock for 250 milliseconds. When the lock is not free in that time,
`export` refuses. It exits with code 2 and writes nothing.

## What the tool never does

- It never commits, stages, or moves anything in git. You commit.
- It never writes a row into the old catalog. It opens the file read-only.
- It never renames or edits a frozen file (see below).
- It never changes a file that you changed since `HEAD`. `export` refuses that file
  with `changed since HEAD: commit or revert it, then run again`. It still writes
  the other files.
- It never replaces a file with a rename. A rename onto an existing name is refused
  (`target exists`).
- It never overwrites a report file.
- It never prints an absolute path. Paths in every text are relative to the
  repository. The old catalog is called "the old catalog".

## Reading the report

The report has one line per class of old fact. Each class ends in one of four
statuses. The markdown report prints `already exported`. The JSON report spells it
`already_exported`.

| Status | Meaning |
|---|---|
| `moved` | A committed file changes for this fact. `export` writes the change. |
| `already exported` | The committed text already holds the fact. Nothing to do. |
| `skipped` | The fact stays in the old catalog on purpose. The reason says why. |
| `unrecoverable` | The tool cannot move the fact. The reason says why. You decide. |

The classes are:

| Class | Fact |
|---|---|
| `params_entry` | An entry of an augmentation's entry collection. `moved` writes a section for it. |
| `params_other` | A params key outside the entry collection. It has no committed home. |
| `augmentation_shape` | The prompt and the shape of an augmentation. They go to a sidecar file. |
| `event` | An event. It goes into the `## Events` section of a single-writer document. |
| `observation` | An observation. It goes the same way as an event. |
| `manual_link` | A link made by hand. It becomes a relation line under `## Relations`. |
| `write_cite` | A citation that a tool call wrote. It stays a catalog row. |
| `rename` | A file whose name gives no valid key. |
| `citation` | An old citation form in prose. `moved` rewrites it into the grammar. |

A `skipped` event has one of these reasons:

- `no committed home yet: kind has no text twin`
- `no committed home yet: not a single-writer document`
- `no committed home yet: ledger or entry document`
- `git history of the file holds it`
- `the frontmatter holds it`
- `the worktree overlay is removed`

A `skipped` observation has one of these reasons:

- `carried by its note event`: the note event holds the same text, so its section
  holds the fact.
- `no committed home yet: not a single-writer document`
- `no committed home yet: ledger or entry document`

An observation is `unrecoverable` with `another section holds id observation-<n>`
when the text holds a section with its id and other content. Another machine wrote
that section for its own observation.

A skipped row stays in the old catalog. The section `## Rows with no committed home`
lists these rows by file.

Read these sections before you run `export`:

- `## Not moved` lists every `skipped` and `unrecoverable` item.
- `## Frozen files and blocked renames` lists each rename that the tool did not make.
- `## Rewrites` lists every prose change as `path:line`, the old text, and the new
  text. Read this section. It is your last look at the changes before they are
  written.
- `## Files` lists every file in the plan.

After the report, `export` adds `## Export result`. It counts the files `written`,
`renamed`, `unchanged`, and `refused`, and it lists each file that was written,
renamed, or refused. The last line names the next step: `Commit the changed files,
then run: codescout cutover build`. A second `export` finds every file `unchanged`.

`export` keeps the report beside the old catalog, as
`cutover-report-<name>-<commit12>.md` and `.json`. A later run never overwrites
them. It uses `-2`, then `-3`, and so on.

## Renames

A rename gives a file the name its key needs. The new name is the old name in ASCII
lowercase. Each run of other characters becomes one `-`. `My_Notes.md` becomes
`my-notes.md`. The file stays in its folder. The tool then rewrites every citation
of the old name, so no `[[My_Notes]]` is left.

The tool refuses a rename and reports it with one of these reasons:

| Reason | Meaning |
|---|---|
| `folder index: rename the folder by hand` | The folder name is the problem, not the file. |
| `no canonical name` | No valid name remains after the rule. |
| `hex-shaped name` | The new name is 16 hex digits. It would read as an artifact id. |
| `target exists` | The new name is already a file, or the old catalog holds a live row for it. |
| `key collision` | Another document holds the new key. The report suggests `<parent folder>-<stem>`. |
| `named by code` | A code file holds the old name or the old path (see [Frozen files](#frozen-files)). The detail lists `path:line`. |
| `frozen: <why>` | A tool reads the file by path. See below. |

## Frozen files

A markdown file is frozen when another tool reads it by path. The tool never
renames a frozen file and never edits one. Each change that the tool would make to
it becomes an `unrecoverable` item with the reason `frozen: <why>`.

A file is frozen in these cases:

- Code names its path. The reason reads `code names its path at <path>:<line>`.
- Code in the same folder names the file. The reason reads `code in its folder names
  the file at <path>:<line>`. A Rust `include_str!("./prompt.md")` does this.
- It lies under a directory whose name starts with `.`. The reason reads `it lies
  under the directory <name>, whose files other tools read by name`.
- It is a page of an mdBook. The reason reads `mdBook reads it: <book.toml> names
  its folder as the book source`.

To read a `frozen:` reason, open the `path:line` it names. That line holds the name
of the markdown file.

**What counts as code.** The code scan reads the text files of `HEAD` that are not
markdown. It skips these files, because they name documents as data:

- data and log files: `.jsonl`, `.json`, `.txt`, `.csv` and `.log`;
- the audit trail under `.codescout/audit/`;
- every subtree that the key space excludes.

Every other text file counts as code: source files, scripts and configuration, such
as `.rs`, `.py`, `.sh`, `.toml` and `.yaml`. A comment counts too. A doc comment that
holds `docs/issues/x.md` freezes that file. A repository whose code cites documents
in comments can report many `frozen:` reasons.

For each `frozen:` reason, decide:

1. Open the line that the reason names.
2. If code reads the file at that line, leave the file as it is. The tool is right.
3. If the line only mentions the file, nothing reads the file by path. A comment is
   an example. Change the file by hand, commit, and run `cutover plan` again. The
   plan reads the new `HEAD`.

A data tree with another extension can freeze files too. Exclude that tree in
`.codescout/key-space.toml`, commit, and run `cutover plan` again. The markdown
files of an excluded tree have no key.

## Build

`cutover build` plans `HEAD` and refuses while the plan would still change a file:

```text
the export is not committed: <n> files would change. Run codescout cutover export, commit, then run this again
```

When the plan writes nothing, `build` creates `.codescout/derived.db` in the git root
of this checkout. It builds from the committed files of `HEAD`, not from the working
tree. It prints the number of documents, entries, edges, chunks, skipped files, and
problems it stored.

Each checkout has one `derived.db`: its own. A linked worktree and a clone each build
their own file, and a build in one checkout does not change the file of another. Run
`build` in each checkout that needs one.

A second `build` rebuilds the same file in place. A `-wal`, `-shm` or `-journal` file
that a killed build left is not an error: SQLite recovers it. A `derived.db` that
holds no build is built over too.

`build` refuses with code 2 in these cases:

- `HEAD` moved after the plan.
- A `-wal`, `-shm` or non-empty `-journal` file lies beside the old catalog. Stop every
  codescout process, then use the `sqlite3` command above.
- Git does not ignore `.codescout/derived.db` and its `-wal`, `-shm` and `-journal`
  files. `build` writes them in the working tree, and the next `git add -A` can then
  stage them. Add this line to `.gitignore`, or to the `info/exclude` file of the
  repository (`git rev-parse --git-path info/exclude` prints its path), then run
  `build` again:

  ```text
  /.codescout/derived.db*
  ```

  A rule from any source that git reads, such as `core.excludesFile`, also counts.
- `.codescout` in this checkout is a link, is not a directory, or resolves to another
  directory. `build` writes only inside the checkout. Replace it with a directory.
- `.codescout/derived.db` or one of its side files is a link. On Unix, a file with more
  than one hard link is refused too. A write through it would also change the file
  under its other name. Remove it.
- Git tracks `.codescout/derived.db` or one of its side files. `build` would
  overwrite a committed file. Run `git rm --cached <file>`, commit, then run `build`
  again.
- `.codescout/derived.db` is not a new catalog. It is a file of another program.
- A newer codescout made `.codescout/derived.db`.
- `.codescout/derived.db` cannot be created or opened. Check that the `.codescout`
  directory of this checkout is writable.
- The tool cannot inspect `.codescout` or one of the files, cannot read the git
  index, or cannot tell whether git ignores the file. The refusal says which.

The old catalog is never written.

A `derived.db` beside the old catalog is unused. Delete it.

The vector cache is `vector-cache.db` in the librarian directory of the machine: the
directory of `LIBRARIAN_DB` when it is set, else the `librarian` directory of the
local data directory. Every checkout shares it. An old codescout binary never opens
`.codescout/derived.db`.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | The run did what it was asked, whatever it found. |
| `1` | `export` refused some files, or could not keep a report file. |
| `2` | The run did not run. The output gives the reason on one line. |

`plan` and `build` use `0` and `2`. A refusal before the first write always gives
`2`. These cases refuse:

- no old catalog;
- a side file beside the old catalog;
- a changed old catalog;
- a `repo.toml` or a `key-space.toml` in `HEAD` that does not parse;
- no git repository;
- a held project write lock (`export`);
- an existing `.codescout/derived.db` (`export`);
- an export that is not committed (`build`);
- a `HEAD` that moved after the plan (`build`);
- a repository that does not ignore `.codescout/derived.db` and its side files
  (`build`);
- a link at `.codescout`, a `.codescout` that is not a directory of this checkout,
  or a link at `.codescout/derived.db` or a side file, or, on Unix, a hard link there
  (`build`);
- a `.codescout/derived.db` or a side file that git tracks (`build`);
- a `.codescout/derived.db` that is not a new catalog, that a newer codescout made,
  or that cannot be created or opened (`build`);
- a file or git state that the tool cannot inspect (`build`).
