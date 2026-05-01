# codescout-companion Plugin

The `codescout-companion` plugin steers Claude Code — and every subagent it spawns — toward
codescout's symbol-aware tools instead of falling back to `grep`, `cat`, and `Read`. It injects
tool guidance at session start, propagates it to subagents, and hard-blocks native file-reading
tools on source files before they execute.

## Install

```
/plugin marketplace add mareurs/sdd-misc-plugins
/plugin install codescout-companion@sdd-misc-plugins
```

Or add to `~/.claude/settings.json`:

```json
{
  "enabledPlugins": {
    "codescout-companion@sdd-misc-plugins": true
  }
}
```

Start a new Claude Code session after installing — the plugin activates automatically.

## Verify

```bash
claude /plugin list
# should show: codescout-companion@sdd-misc-plugins
```

Then start a session and ask Claude which tool it would use to search for a function by name.
It should cite `symbols`, not `grep`.

## Full Documentation

For configuration options, hook details, Ollama setup, and troubleshooting, see the
[codescout-companion README](https://github.com/mareurs/sdd-misc-plugins/tree/main/codescout-companion/README.md).
