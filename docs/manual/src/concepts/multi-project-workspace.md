# Multi-Project Workspace Support

codescout can manage multiple related projects from a single server instance.
This is useful for monorepos or closely related repositories where you want
cross-project navigation without running separate MCP servers.

## Registering projects

Projects are registered in `.codescout/workspace.toml` under a `[[project]]` table:

```toml
[[project]]
id = "backend"
root = "services/backend"

[[project]]
id = "frontend"
root = "apps/frontend"
```

Each entry requires `id` (unique name) and `root` (path relative to the workspace root).
The `languages` and `depends_on` fields are optional: `languages` restricts which LSP
servers are started for the project; `depends_on` lists project IDs whose symbols are
visible during cross-project navigation.

Each project gets its own LSP servers, memory store, and semantic index.

### Declared projects and discovered projects

codescout finds projects by walking the workspace for build manifests (`Cargo.toml`,
`pyproject.toml`, `package.json` with scripts, and so on). A `[[project]]` entry adds a
member to that list. This is how a directory with no build manifest, or one that the walk
cannot see (for example a repository hidden with `.git/info/exclude`), becomes a project.
If a declared entry has no `languages`, codescout takes them from the files in the directory.

- A project that the walk finds keeps its discovered `id`. A declared entry for the same
  root changes nothing. This is the normal case: onboarding writes every discovered project
  into `workspace.toml`.
- A declared entry is skipped, with a warning in the log, when its `root` is empty, is `.`,
  is absolute, contains `..`, is not an existing directory, leaves the workspace through a
  link, or is named by `exclude_projects`. It is also skipped when its `id` is empty,
  contains a path separator, or is already used by another project.
- A member that is its own git repository (it has a `.git` directory or file) keeps its
  memory in its own `.codescout/memories`, not in the parent's
  `.codescout/projects/<id>/memories`.

## Using project scope

Most tools accept a `project` parameter to scope the operation:

```
symbols("MyStruct", project: "backend")
semantic_search("authentication flow", project: "frontend")
memory(action: "read", project: "backend", topic: "architecture")
```

Omitting `project` uses the workspace-level context.

## Onboarding

Run onboarding once after registering projects:

```
Run codescout onboarding
```

codescout generates a per-project Navigation Strategy section in the system prompt so the
agent knows which files and entry points belong to each project. It also generates
cross-project semantic search scope guidance.

## Cross-project semantic search

The system prompt includes guidance on which `scope=` values to use for semantic search
across projects, so the agent does not need to guess project boundaries.
