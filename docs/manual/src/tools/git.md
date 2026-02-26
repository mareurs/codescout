# Git Tools

The three git tools give you read access to the repository history of the
active project. They use the `git2` library and operate directly on the local
repository — no `git` binary needs to be installed.

All three tools require an active project that is inside a git repository. They
respect the project's path security settings: passing paths outside the project
root is rejected.

---

## `git_blame`

**Purpose:** Return line-level blame for a file: who last changed each line,
the commit SHA, and the commit timestamp.

**Parameters:**

| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `path` | string | yes | — | File path relative to the project root |
| `start_line` | integer | no | — | First line to include (1-indexed, inclusive) |
| `end_line` | integer | no | — | Last line to include (1-indexed, inclusive) |
| `detail_level` | string | no | compact | `"full"` returns all lines without the default 50-line cap |
| `offset` | integer | no | `0` | Skip this many lines (pagination) |
| `limit` | integer | no | `50` | Maximum lines per page |

**Example (blame a specific range):**

```json
{
  "path": "src/auth.rs",
  "start_line": 100,
  "end_line": 140
}
```

**Output:**

```json
{
  "lines": [
    {
      "line": 100,
      "content": "pub fn authenticate_user(token: &str) -> Result<Session> {",
      "sha": "a3f8c120",
      "author": "Alice",
      "timestamp": 1706745600
    },
    {
      "line": 101,
      "content": "    let claims = decode_jwt(token)?;",
      "sha": "a3f8c120",
      "author": "Alice",
      "timestamp": 1706745600
    }
  ],
  "total": 41
}
```

Each entry has:
- `line` — 1-indexed line number in the file
- `content` — the line text (from the last committed version, not the working copy)
- `sha` — short commit SHA (8 characters) of the last commit that touched this line
- `author` — commit author name
- `timestamp` — Unix timestamp of the commit

When the result exceeds the cap, an `overflow` object is added with a hint on
how to retrieve more lines.

**Tips:**

- Use `start_line`/`end_line` to scope blame to the function you care about.
  Getting blame for an entire large file is rarely useful.
- `timestamp` is a Unix timestamp. Divide by 86400 to get days since epoch, or
  compare two values to see which change is more recent.
- Blame operates on the last committed version of the file. Uncommitted changes
  to the working directory are not reflected. Use `git_diff` to see what has
  changed since the last commit.
- To understand the full context of a change, take the `sha` from a blame line
  and use it with `git_log` or check it in your git client.

---

## `git_log`

**Purpose:** Show commit history for a specific file, or for the entire project
when no path is given.

**Parameters:**

| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `path` | string | no | — | File path relative to the project root. Omit for project-wide history |
| `limit` | integer | no | `20` | Maximum number of commits to return |

**Example (file history):**

```json
{
  "path": "src/embed/index.rs",
  "limit": 10
}
```

**Output (file history):**

```json
{
  "file": "src/embed/index.rs",
  "commits": [
    {
      "sha": "300e9ee4",
      "message": "ci: add macOS and Windows to test matrix",
      "author": "Marius",
      "timestamp": 1740524400
    },
    {
      "sha": "9665961a",
      "message": "fix: replace hardcoded Unix paths in tests with portable alternatives",
      "author": "Marius",
      "timestamp": 1740438000
    }
  ]
}
```

**Example (project-wide history):**

```json
{
  "limit": 5
}
```

**Output (project-wide):**

```json
{
  "commits": [
    {
      "sha": "300e9ee4",
      "message": "ci: add macOS and Windows to test matrix",
      "author": "Marius",
      "timestamp": 1740524400
    }
  ]
}
```

When a `path` is provided, the response includes a `"file"` key alongside
`"commits"`. Project-wide results omit the `"file"` key.

Each commit entry has:
- `sha` — short commit SHA (8 characters)
- `message` — the first line of the commit message
- `author` — commit author name
- `timestamp` — Unix timestamp of the commit

**Tips:**

- File history uses git's path-filtering walk, so it only returns commits that
  actually touched the given file. This is the right tool for "when was this
  file last changed and by whom."
- Project-wide history walks from HEAD in time order and is equivalent to
  `git log --oneline -N`. Use it for a quick orientation to recent activity.
- `limit` defaults to 20. Increase it if you need to look further back, but
  large values may produce more output than is useful in a single tool call.

---

## `git_diff`

**Purpose:** Show uncommitted changes in the working directory, optionally
restricted to a single file or compared against a specific commit instead of
HEAD.

**Parameters:**

| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `path` | string | no | — | Restrict the diff to this file (relative to project root) |
| `commit` | string | no | `HEAD` | Commit SHA or ref to diff the working directory against |
| `detail_level` | string | no | compact | `"full"` returns the complete diff without the 50 KB truncation |

**Example (all uncommitted changes):**

```json
{}
```

**Example (changes to one file):**

```json
{
  "path": "src/tools/git.rs"
}
```

**Example (diff against a specific commit):**

```json
{
  "commit": "HEAD~3",
  "detail_level": "full"
}
```

**Output:**

```json
{
  "diff": "diff --git a/src/tools/git.rs b/src/tools/git.rs\nindex 3f2a1b0..7c8d4e2 100644\n--- a/src/tools/git.rs\n+++ b/src/tools/git.rs\n@@ -128,6 +128,10 @@ impl Tool for GitDiff {\n ..."
}
```

When the diff is large and `detail_level` is not `"full"`, the output is
truncated at approximately 50 KB and an `overflow` object explains how to
retrieve the rest:

```json
{
  "diff": "...(truncated)...",
  "overflow": {
    "shown_bytes": 49987,
    "total_bytes": 134210,
    "hint": "Diff truncated. Use detail_level='full' for complete output, or restrict to a specific file with 'path'."
  }
}
```

**Tips:**

- Call `git_diff` with no arguments to get a quick overview of what is
  currently changed before deciding which files to inspect more closely.
- For large diffs, start without `detail_level: "full"` to see the scope of
  changes, then re-call with `path` to drill into specific files.
- The `commit` parameter accepts any git ref: a full or short SHA, a branch
  name, a tag, or an expression like `HEAD~2`. Use this to compare the working
  directory against a known good commit when bisecting a regression.
- `git_diff` shows the diff of the working directory (including staged and
  unstaged changes) against the specified commit. It does not distinguish
  between staged and unstaged changes — that distinction is not exposed.
- If the diff is empty, the working directory is clean relative to the
  specified commit.
