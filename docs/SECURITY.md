# Security Policy

## Reporting a Vulnerability

Report security vulnerabilities privately via [GitHub Security
Advisories](https://github.com/mareurs/codescout/security/advisories/new)
rather than a public issue. Include a proof-of-concept and the affected
version if you have one.

codescout is a small-team, pre-1.0 project. There's no formal SLA, but
security reports are prioritized over other work.

## Supported Versions

Only the latest published release (currently `0.15.x`) receives security
fixes. codescout is pre-1.0 (`0.y.z`) — minor bumps may carry breaking
changes, and there is no long-term-support branch.

## Threat Model

codescout is an MCP server that gives an LLM agent read/search/edit access
to a codebase. The design goal: **an agent can explore freely but cannot
write outside explicit boundaries or read known credential locations.** Full
model and configuration: [Security &
Permissions](docs/manual/src/concepts/security.md).

| Surface | Protection | Caveat |
|---|---|---|
| File reads | Built-in, non-configurable deny-list (SSH/cloud/git/package-registry credentials, shell history, OS secret stores) | `profile = "root"` disables this entirely for absolute-path reads — an opt-out, not a stronger sandbox |
| File writes | Restricted to project root + `extra_write_roots`, or a session-approved root via `approve_write`; deny-list still applies and wins even over an approved root | Hard boundary by default, not just a warning |
| Shell (`run_command`) | `shell_command_mode` gate (`warn` / `disabled`); `cwd` is sandboxed to the project root | The command string itself is not filtered when shell is enabled; dangerous commands (`rm -rf`, `dd`, `mkfs`, ...) require `acknowledge_risk: true` |
| HTTP transport | Bearer token (OS-random, `/dev/urandom`), constant-time compare, validated on every request | Token is printed to stderr or passed via `--auth-token`; there's no TLS termination built in — put a reverse proxy in front for anything beyond localhost |
| Dashboard (`codescout dashboard`, `dashboard` feature) | Binds to `127.0.0.1` by default | **No authentication of its own.** If you pass `--host 0.0.0.0` or another non-loopback address, anyone who can reach that address gets full read/write/delete access to project memory and config over HTTP. Don't expose it beyond localhost without your own reverse-proxy auth. |

### Known limitations (tracked, not hidden)

- The `default` security profile's read protection is a deny-list, not a
  containment sandbox — it blocks a fixed set of credential paths, not reads
  outside the project tree in general.
- The dashboard has no auth layer of its own (see above); safety currently
  depends entirely on the loopback-only default.
- HTTP bearer auth validates a single static token per server instance —
  there's no key rotation or per-client scoping.

We'd rather document these plainly than have them discovered in a report. If
you find a way to defeat any protection above in a way not already described
here, that's exactly what we want reported.
