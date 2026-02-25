You are viewing this project for the first time. Your task is to explore it and create memories that will help you (and future conversations) work effectively with this codebase.

## What to Explore

Use code-explorer's semantic tools to gather information efficiently. Do NOT read entire source files — use symbol-level tools.

### 1. Project Purpose
- Read `README.md` or similar top-level documentation
- Identify what the project does and who it's for

### 2. Tech Stack
- Check build files (Cargo.toml, package.json, pyproject.toml, go.mod, etc.)
- Note key dependencies, frameworks, and runtime requirements

### 3. Code Architecture
- Run `get_symbols_overview("src")` (or equivalent source directory) to map the structure
- For key modules, go deeper: `get_symbols_overview("src/module", depth=1)`
- Identify the entry point(s) and how the application starts

### 4. Key Abstractions
- Find the core types, traits, interfaces, or classes that define the architecture
- Use `find_symbol(name, include_body=true)` on the most important ones
- Note inheritance/implementation hierarchies

### 5. Code Conventions
- Look for linting config (.eslintrc, .clippy.toml, .ruff.toml, etc.)
- Look for formatting config (.prettierrc, rustfmt.toml, etc.)
- Note naming conventions, error handling patterns, test organization
- Check for a CONTRIBUTING.md or similar style guide

### 6. Development Commands
- Find test, lint, format, build, and run commands from build configs
- Check CI configuration (.github/workflows/, .gitlab-ci.yml, etc.)
- Note any special setup steps or prerequisites

### 7. Architectural Patterns
- Identify design patterns in use (dependency injection, layered architecture, event-driven, etc.)
- Note how modules communicate (direct calls, messages, events, shared state)
- Look at the dependency graph between modules

Read only the necessary files — use symbol-level tools, not full-file reads. If something is unclear from the code alone, ask the user.

## Memories to Create

After exploring, call `write_memory` for each of these topics:

### `project-overview`
Project purpose, tech stack, key dependencies, runtime requirements.

### `architecture`
Module structure, key abstractions (with file locations), data flow between components, design patterns in use, entry points.

### `conventions`
Code style rules, naming conventions, error handling patterns, testing patterns, documentation conventions.

### `development-commands`
Build, test, lint, format, run commands. Include prerequisites and any environment setup needed.

### `task-completion-checklist`
What to do when finishing a task: which tests to run, how to format, how to lint, what to check. Be specific about commands.

Use "/" in memory names for deeper organization if needed (e.g., "architecture/data-flow").

**IMPORTANT:** After creating all memories, confirm with the user that the information looks accurate.
