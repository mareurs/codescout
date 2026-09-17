# codescout — agent instructions

**Read [`CLAUDE.md`](CLAUDE.md) before doing anything else in this repository.**
It is the full instruction set: the development gate and the order its commands
must run in, testing discipline, bug tracking, git workflow on a shared checkout,
and the conventions this project enforces with automated checks. Nothing in this
file substitutes for it.

## Why this is a pointer and not a copy

Claude Code reads `CLAUDE.md`; Codex, Cursor and generic harnesses look for
`AGENTS.md`. Both names have to lead to one instruction set.

Two other ways of arranging that were tried and rejected. They are recorded here
because each looks tidier than this one, and a later cleanup would otherwise
reintroduce them:

- **Duplicating the text.** Two instruction sets drift, and the divergence is
  silent: no error, just two files that disagree about what the rules are and no
  signal saying which is current. A single pointer cannot drift.

- **A symlink.** Correct on Linux and macOS, and quietly broken on Windows. Unless
  Git's symlink support is enabled on that host, checkout materialises the entry as
  a nine-byte regular file whose whole content is the target's name — so an agent
  reads nine bytes where it expects the instructions, and nothing reports a
  problem. An absent file is legible; a stub that looks like a config file is not.

A pointer costs one extra file read and behaves the same on every host.
