---
specialist: debugging-yeti
scope: project
slug: lsp-install-recipe
created: 2026-05-30
updated: 2026-05-30
tags: [lsp, environment, kotlin, path, codescout]
---

**Lesson:** "Failed to start LSP server: <lang>" from codescout almost always means the language-server binary isn't on the MCP server's PATH — it is NOT a codescout bug. The surface message swallows the underlying ENOENT. Diagnose with `command -v <binary>` (canonical names live in `src/lsp/servers/mod.rs::default_config`). A `kotlin-mux-*.lock` with NO matching `.sock` in `/run/user/$UID/` is the smoking gun: the mux acquired its lock, the spawn failed, the socket never got created.

**Why:** 2026-05-30, backend-kotlin — `references` failed with that message while `symbols`/`grep` (tree-sitter, no LSP) worked. Root cause: `kotlin-lsp` simply wasn't installed. The clean tree-sitter-vs-LSP split in the symptom table localizes it instantly to "the LSP *server* process," not codescout.

**How to apply:** Install LSP binaries into a dir ALREADY on the MCP server's PATH (here `~/.local/bin` and `/usr/bin`) — installing to a default that isn't on PATH (`go install`→`~/go/bin`, npm-global→`/usr` needing sudo) recreates the exact bug. Recipe on this Arch box:
- bash → `npm i -g --prefix ~/.local bash-language-server`
- html/css → `npm i -g --prefix ~/.local vscode-langservers-extracted`
- go → `GOBIN=~/.local/bin go install golang.org/x/tools/gopls@latest`
- kotlin → `yay -S kotlin-lsp-bin` (JetBrains `intellij-server`; accepts `--stdio --system-path=`; NOT the fwcd `kotlin-language-server`, which has a different binary name and arg set)
- java → `yay -S jdtls`
- rust-analyzer / pyright-langserver / typescript-language-server / clangd were already present (rustup + pacman).
After installing, `/mcp` restart in the affected project to clear any tripped LSP circuit-breaker / stale mux lock. `~/.local/bin` + `/usr/bin` are already on the server PATH, so no shell-config change is needed. See [[dont-fabricate-commit-rationale]] for the sibling memory channel.
