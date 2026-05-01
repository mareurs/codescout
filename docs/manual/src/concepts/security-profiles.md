# Security Profiles


codescout now supports a `profile` field in the `[security]` section of
`.codescout/project.toml`. The profile controls how strictly path validation
and shell command safety checks are enforced.

## Profiles

### `default` (standard sandbox)

This is the profile used when no `profile` key is present.

- **Read deny-list active** — system paths (`.ssh/`, `/etc/passwd`, etc.) are
  blocked regardless of the calling tool.
- **Writes restricted** to the project root and the system temp directory.
  Additional directories can be added via `extra_write_roots`.
- **Dangerous shell commands** (e.g. `rm -rf`, `dd`, `mkfs`) require
  `acknowledge_risk: true` in `run_command`.

### `root` (unrestricted)

For system-administration projects that legitimately need full filesystem access.

- **No read deny-list** — any path readable by the OS user can be read.
- **Writes allowed anywhere** the OS user has permission.
- **Dangerous command check bypassed** — `run_command` executes without a
  speed bump.

Source-file shell access guidance (prefer `read_file`/`symbols` over `cat`)
remains active in both profiles. It improves tool output quality, not security.

## Configuration

Add a `[security]` section to `.codescout/project.toml`:

```toml
[security]
profile = "root"
```

The default value is `"default"` — omitting the field is equivalent to
`profile = "default"`.

## When to use `root` mode

Only switch to `root` if your project genuinely needs it:

- System administration scripts that read `/etc`, `/var`, or write outside the
  project tree.
- Dotfile managers, backup tools, or package managers where restricting paths
  would prevent the tool from functioning.

For regular application development, keep `profile = "default"`. The sandbox
prevents accidental reads of SSH keys or credential files and catches
destructive shell commands before they run.

## Limitations

- The profile is per-project, not per-tool. There is no way to lift the sandbox
  for a single tool call.
- `profile = "root"` does not bypass OS-level permissions — codescout can still
  only access paths the running user is allowed to read or write.
