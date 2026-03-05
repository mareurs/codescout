# Source File Access Check — Quote-Aware Splitting Design

**Date:** 2026-03-05
**Status:** Approved

## Problem

`check_source_file_access` in `src/util/path_security.rs` applies two regexes to the
raw command string:

- `SOURCE_ACCESS_COMMANDS`: `\b(cat|head|tail|sed|awk|less|more|wc)\b`
- `SOURCE_EXTENSIONS`: `\.(rs|py|ts|...)\b`

If both match in the same pipe-segment, the command is blocked. The only split is on
`|` (pipes). This causes false positives when a command string contains those tokens
inside a **quoted argument** — not as an actual command or filename.

### Reproducer

```
git reset --soft abc123 && git commit -m "feat: tail-50 of log, output_buffer.rs"
```

- No pipes → one segment
- `tail` matches `SOURCE_ACCESS_COMMANDS`
- `.rs` matches `SOURCE_EXTENSIONS`
- → Blocked, even though no source file is being read

### Root cause

The check has no concept of quoted strings. Text inside `"..."` or `'...'` is never
a command being executed or a filename being passed to a blocked command, but the regex
sees it the same as unquoted text.

A secondary gap: only `|` is used as a segment separator. `&&` and `;` also delimit
independent commands — without splitting on them, `./build.sh && cat src/main.rs`
is one segment whose first token is `./build.sh`, not `cat`.

## Design

### Single change: `src/util/path_security.rs`

Replace the `check_source_file_access` implementation. The constants
(`SOURCE_ACCESS_COMMANDS`, `SOURCE_EXTENSIONS`) are unchanged.

### New helper: `split_outside_quotes`

```rust
fn split_outside_quotes(s: &str, seps: &[&str]) -> Vec<String>
```

A character-level state machine that tracks whether each position is inside
`"..."` or `'...'` (with `\\` escape handling). When a separator from `seps`
is encountered **outside** quotes, it splits there. Returns a `Vec<String>` of
sub-segments.

Separators checked in this order: `"&&"`, `"||"`, `";"`, `"|"`. `&&` and `||`
before `|` prevents `||` from being mis-split as two `|` separators.

Edge cases:
- **Unclosed quote**: treat end-of-string as implicit close. Return whatever was parsed.
- **`||`**: treated as a command separator (same as `&&`) — both sides are independent commands.
- **Empty sub-segments**: skipped silently.
- **Escaped quotes** (`\"`): state machine skips the next char after `\\`, so `\"` does not close the string.

### Updated `check_source_file_access`

```
1. split_outside_quotes(command, ["&&", "||", ";", "|"])
2. For each sub-segment:
   a. Skip if contains "<<" (heredoc guard — unchanged)
   b. Shell-tokenize; take first non-empty token
   c. If first token ∉ SOURCE_ACCESS_COMMANDS → skip (not a source-reading command)
   d. If first token ∈ SOURCE_ACCESS_COMMANDS AND full sub-segment matches SOURCE_EXTENSIONS → block
```

Step (c) is the key change from the current implementation: only the **first token**
of a sub-segment determines whether a source-reading command is being executed.
Content inside quoted arguments (commit messages, echo strings, etc.) cannot be the
first token of a sub-segment and is therefore never matched as a command.

Step (d) still checks the full sub-segment string for `SOURCE_EXTENSIONS` so that
`cat "src/main.rs"` (quoted path) is still caught — the `.rs` extension appears in
the sub-segment even though it's inside quotes.

## Known Remaining Limitations (pre-existing, not regressions)

| Pattern | Blocked? | Note |
|---|---|---|
| `sudo cat src/main.rs` | ✗ | First token `sudo` |
| `env X=1 cat src/main.rs` | ✗ | First token `env X=1` |
| `bash -c "cat src/main.rs"` | ✗ | First token `bash` |
| `git show HEAD:src/main.rs` | ✗ | `git` not in blocklist |

None of these are caught by the current implementation either.

## Tests

All in `src/util/path_security.rs` tests module:

| Test | Command | Expected |
|---|---|---|
| `git_commit_with_tail_in_message_not_blocked` | `git commit -m "feat: tail-50 output_buffer.rs"` | not blocked |
| `git_commit_with_ampersand_and_source_in_message_not_blocked` | `git commit -m "fix && cat src/main.rs"` | not blocked |
| `cat_source_file_blocked` | `cat src/main.rs` | blocked |
| `compound_and_then_cat_blocked` | `./build.sh && cat src/main.rs` | blocked |
| `semicolon_then_cat_blocked` | `echo done; cat src/main.rs` | blocked |
| `pipe_chain_with_source_blocked` | `tail src/main.rs \| grep foo` | blocked |
| `git_diff_pipe_head_not_blocked` | `git diff src/server.rs \| head -80` | not blocked |
| `heredoc_not_blocked` | `cat <<'EOF'` | not blocked |
