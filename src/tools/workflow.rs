//! Workflow and onboarding tools.

use std::path::Path;

use super::{parse_bool_param, Tool, ToolContext};
use serde_json::{json, Value};

// ── Hardware detection ────────────────────────────────────────────────────────

// ── Onboarding versioning ─────────────────────────────────────────────────────

/// Bump this when system prompt surfaces change significantly.
/// Missing or lower stored version triggers auto-refresh of the system prompt.
/// See CLAUDE.md § "Onboarding Version" for when to bump.
const ONBOARDING_VERSION: u32 = 3;

/// Returns true if the stored onboarding version is stale (needs refresh).
/// `None` means pre-versioning project — always stale.
/// Stored > compiled (downgrade) is treated as current to avoid churn.
fn onboarding_version_stale(stored: Option<u32>) -> bool {
    match stored {
        None => true,
        Some(v) => v < ONBOARDING_VERSION,
    }
}

/// System facts gathered at onboarding time for model selection.
#[derive(Debug, serde::Serialize)]
pub struct HardwareContext {
    pub ollama_available: bool,
    pub ollama_host: String,
    pub gpu: Option<GpuInfo>,
    pub ram_gb: u64,
    pub cpu_cores: u32,
}

/// GPU vendor and VRAM info (best-effort; None means no GPU detected).
#[derive(Debug, serde::Serialize)]
#[serde(tag = "vendor", rename_all = "lowercase")]
pub enum GpuInfo {
    Nvidia { name: String, vram_mb: u64 },
    Amd { name: String, vram_mb: Option<u64> },
}

/// One entry in the ranked model recommendation list.
#[derive(Debug, serde::Serialize)]
pub struct ModelOption {
    pub id: String,
    pub label: String,
    pub dims: u32,
    pub context_tokens: u32,
    pub reason: String,
    pub available: bool,
    pub recommended: bool,
}

/// Pure function: derive a ranked model list from hardware facts.
/// The first entry is always the recommended default (local:AllMiniLML6V2Q).
pub fn model_options_for_hardware(ctx: &HardwareContext) -> Vec<ModelOption> {
    let mut options = vec![ModelOption {
        id: "local:AllMiniLML6V2Q".into(),
        label: "AllMiniLML6V2Q".into(),
        dims: 384,
        context_tokens: 256,
        reason: "bundled ONNX, no server needed, lightweight default (22MB, quantized)".into(),
        available: true,
        recommended: true,
    }];

    if ctx.ollama_available {
        options.push(ModelOption {
            id: "url".into(),
            label: "Use running Ollama".into(),
            dims: 768,
            context_tokens: 8192,
            reason: format!(
                "set url = \"{}/v1\" in project.toml to use your running Ollama",
                ctx.ollama_host.trim_end_matches('/')
            ),
            available: true,
            recommended: false,
        });
    }

    options.push(ModelOption {
        id: "local:JinaEmbeddingsV2BaseCode".into(),
        label: "JinaEmbeddingsV2BaseCode".into(),
        dims: 768,
        context_tokens: 8192,
        reason: "code-specialized ONNX, no server needed (~300MB download)".into(),
        available: true,
        recommended: false,
    });

    if !ctx.ollama_available {
        options.push(ModelOption {
            id: "url".into(),
            label: "External server".into(),
            dims: 0,
            context_tokens: 0,
            reason: "set url in [embeddings] to use any OpenAI-compatible embedding server".into(),
            available: true,
            recommended: false,
        });
    }

    options
}

/// Extract a `host:port` string suitable for `TcpStream::connect` from an
/// Ollama host URL like `http://localhost:11434`.
pub(crate) fn ollama_tcp_addr(host: &str) -> String {
    let stripped = host
        .strip_prefix("https://")
        .or_else(|| host.strip_prefix("http://"))
        .unwrap_or(host);
    if stripped.contains(':') {
        stripped.to_string()
    } else {
        format!("{stripped}:11434")
    }
}

/// Returns true if a TCP connection to Ollama's port succeeds within 2s.
async fn probe_ollama(tcp_addr: &str) -> bool {
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        tokio::net::TcpStream::connect(tcp_addr),
    )
    .await
    .map(|r| r.is_ok())
    .unwrap_or(false)
}

/// Probe NVIDIA GPU via nvidia-smi. Returns None if not available.
async fn probe_nvidia() -> Option<GpuInfo> {
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        tokio::process::Command::new("nvidia-smi")
            .args([
                "--query-gpu=name,memory.total",
                "--format=csv,noheader,nounits",
            ])
            .output(),
    )
    .await
    .ok()?
    .ok()?;

    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next()?;
    let mut parts = line.splitn(2, ',');
    let name = parts.next()?.trim().to_string();
    let vram_mb: u64 = parts.next()?.trim().parse().ok()?;
    Some(GpuInfo::Nvidia { name, vram_mb })
}

/// Probe AMD GPU via rocm-smi. Returns None if not available.
async fn probe_amd() -> Option<GpuInfo> {
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        tokio::process::Command::new("rocm-smi")
            .arg("--showproductname")
            .output(),
    )
    .await
    .ok()?
    .ok()?;

    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    // rocm-smi output contains lines like "Card series:  AMD Radeon RX 7900 XTX"
    let name = stdout
        .lines()
        .find(|l| {
            let l = l.to_lowercase();
            l.contains("card series") || l.contains("card model") || l.contains("radeon")
        })
        .and_then(|l| l.split_once(':'))
        .map(|(_, v)| v.trim().to_string())
        .unwrap_or_else(|| "AMD GPU".into());
    Some(GpuInfo::Amd {
        name,
        vram_mb: None,
    })
}

/// Read total system RAM in GiB. Returns 0 on failure (non-fatal).
async fn probe_ram() -> u64 {
    // Linux: /proc/meminfo — use spawn_blocking to avoid blocking the async executor
    let meminfo = tokio::task::spawn_blocking(|| std::fs::read_to_string("/proc/meminfo"))
        .await
        .ok()
        .and_then(|r| r.ok());
    if let Some(content) = meminfo {
        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                let kb: u64 = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                return kb / 1024 / 1024;
            }
        }
    }
    // macOS
    if let Ok(output) = tokio::process::Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .await
    {
        if let Ok(s) = String::from_utf8(output.stdout) {
            if let Ok(bytes) = s.trim().parse::<u64>() {
                return bytes / 1024 / 1024 / 1024;
            }
        }
    }
    0
}

/// Probe the local system for hardware capabilities relevant to embedding
/// model selection. All probes run in parallel with a 2-second timeout;
/// any failure produces a safe zero/None default — never panics.
pub async fn detect_hardware_context() -> HardwareContext {
    let ollama_host =
        std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".into());
    let tcp_addr = ollama_tcp_addr(&ollama_host);

    let cpu_cores = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(4);

    let (ollama_available, nvidia, amd, ram_gb) = tokio::join!(
        probe_ollama(&tcp_addr),
        probe_nvidia(),
        probe_amd(),
        probe_ram(),
    );

    // NVIDIA wins if both somehow respond (shouldn't happen, but be defensive)
    let gpu = nvidia.or(amd);

    HardwareContext {
        ollama_available,
        ollama_host,
        gpu,
        ram_gb,
        cpu_cores,
    }
}

pub struct Onboarding;
pub struct RunCommand;

/// Context gathered from well-known project files during onboarding.
#[derive(Debug, Default)]
struct GatheredContext {
    readme_path: Option<String>,
    build_file_name: Option<String>,
    claude_md_exists: bool,
    ci_files: Vec<String>,
    entry_points: Vec<String>,
    test_dirs: Vec<String>,
    /// Path to FEATURES.md if found (relative to project root)
    features_md: Option<String>,
    /// Discovered sub-projects (populated by discover_projects)
    projects: Vec<crate::workspace::DiscoveredProject>,
}

/// Read key project files up-front so the onboarding prompt can include them.
/// Detect well-known project files during onboarding.
///
/// File *contents* are intentionally not read here — inlining README/CLAUDE.md
/// into the onboarding response causes "⚠ Large MCP response" warnings and
/// duplicates CLAUDE.md that may already be in the agent's context. The agent
/// reads these files via `read_file` during Phase 1 exploration.
///
/// `projects` is the already-discovered project list from the workspace, passed in
/// to avoid a redundant `discover_projects` walk (the agent runs it at activation).
fn gather_project_context(
    root: &std::path::Path,
    projects: Vec<crate::workspace::DiscoveredProject>,
) -> GatheredContext {
    let mut ctx = GatheredContext::default();

    // README (try common names — record path but don't read content)
    for name in &["README.md", "README.rst", "README.txt", "README"] {
        if root.join(name).exists() {
            ctx.readme_path = Some(name.to_string());
            break;
        }
    }

    // CLAUDE.md
    ctx.claude_md_exists = root.join("CLAUDE.md").exists();

    // Build file (first match wins, ordered by popularity — name only)
    let build_files = [
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "build.gradle.kts",
        "build.gradle",
        "go.mod",
        "pom.xml",
        "Makefile",
        "CMakeLists.txt",
        "setup.py",
        "mix.exs",
        "Gemfile",
    ];
    for name in &build_files {
        if root.join(name).exists() {
            ctx.build_file_name = Some(name.to_string());
            break;
        }
    }

    // CI config files (just names, not contents)
    for dir in &[".github/workflows", ".gitlab", ".circleci"] {
        let ci_path = root.join(dir);
        if ci_path.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&ci_path) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.ends_with(".yml") || name.ends_with(".yaml") {
                        ctx.ci_files.push(format!("{}/{}", dir, name));
                    }
                }
            }
        }
    }
    ctx.ci_files.sort();

    // Entry points (check common locations)
    let entry_candidates = [
        "src/main.rs",
        "src/lib.rs",
        "src/main.py",
        "src/index.ts",
        "src/index.js",
        "src/app.ts",
        "src/app.py",
        "main.go",
        "cmd/main.go",
        "lib/main.dart",
        "index.js",
        "index.ts",
        "app.py",
        "manage.py",
    ];
    for candidate in &entry_candidates {
        if root.join(candidate).exists() {
            ctx.entry_points.push(candidate.to_string());
        }
    }

    // Test directories
    for candidate in &[
        "tests",
        "test",
        "spec",
        "src/test",
        "src/tests",
        "__tests__",
    ] {
        if root.join(candidate).is_dir() {
            ctx.test_dirs.push(candidate.to_string());
        }
    }

    // FEATURES.md — documents implemented capabilities
    for candidate in &["docs/FEATURES.md", "FEATURES.md", "docs/features.md"] {
        if root.join(candidate).exists() {
            ctx.features_md = Some(candidate.to_string());
            break;
        }
    }

    // Use the already-discovered project list passed by the caller to avoid
    // a redundant filesystem walk (discover_projects is run at activation time).
    ctx.projects = projects;

    ctx
}

fn language_navigation_hints(lang: &str) -> Option<&'static str> {
    match lang {
        "rust" => Some(
            "- symbol: `StructName/method`, `impl Trait for Type/method`\n\
             - find_symbol(kind=\"struct\") for data types, kind=\"function\" for free fns\n\
             - impl blocks: `find_symbol(\"impl MyStruct\")` or list_symbols shows `impl Trait for Type`\n\
             - Example: `find_symbol(\"Server/handle_request\")` finds a method on Server",
        ),
        "python" => Some(
            "- symbol: `ClassName/method_name`, `module_func`\n\
             - find_symbol(kind=\"class\") for classes, kind=\"function\" for functions/methods\n\
             - Decorators aren't in symbol — search for the function name\n\
             - Example: `find_symbol(\"UserService/create\")` finds a method on UserService",
        ),
        "typescript" | "javascript" | "tsx" | "jsx" => Some(
            "- symbol: `ClassName/method`, `exportedFunction`\n\
             - find_symbol(kind=\"class\") for classes, kind=\"function\" for functions/arrow fns\n\
             - React components are functions — use kind=\"function\" not kind=\"class\"\n\
             - Example: `find_symbol(\"AuthProvider/login\")` finds a class method",
        ),
        "go" => Some(
            "- symbol: `TypeName/MethodName`, `PackageFunc`\n\
             - find_symbol(kind=\"function\") covers both functions and methods\n\
             - Receiver methods: `find_symbol(\"Server/ListenAndServe\")`\n\
             - Interfaces: find_symbol(kind=\"interface\") then list_symbols for signatures",
        ),
        "java" | "kotlin" => Some(
            "- symbol: `ClassName/methodName`, `InnerClass`\n\
             - find_symbol(kind=\"class\") for classes/interfaces, kind=\"function\" for methods\n\
             - Annotations aren't in symbol — search by method name\n\
             - Example: `find_symbol(\"UserRepository/findById\")`",
        ),
        "c" | "cpp" => Some(
            "- symbol: `ClassName/method`, `namespace_func`\n\
             - find_symbol(kind=\"struct\") or kind=\"class\" depending on codebase style\n\
             - Header vs implementation: find_symbol shows both — use path= to narrow",
        ),
        _ => None,
    }
}

/// Returns curated anti-patterns and correct patterns for a language.
/// Content sourced from docs/research/claude-language-patterns.md.
fn language_patterns(lang: &str) -> Option<&'static str> {
    match lang {
        "rust" => Some(
            "### Rust\n\
             \n\
             **Anti-patterns (Don't → Do):**\n\
             1. Gratuitous `.clone()` to silence borrow checker → borrow: `&str` over `&String`, `&[T]` over `&Vec<T>`\n\
             2. `.unwrap()` everywhere → `?` with `.context()` from anyhow, `.expect(\"invariant: ...\")` only for proven invariants\n\
             3. `Rc<RefCell<T>>` / interior mutability overuse → restructure data flow and ownership\n\
             4. `String` params where `&str` suffices → `fn greet(name: &str)`, use `Cow<'_, str>` when ownership is conditional\n\
             5. Catch-all `_ => {}` in match → handle all variants explicitly, let compiler check exhaustiveness\n\
             \n\
             **Correct patterns:**\n\
             1. `thiserror` for library errors, `anyhow` for application errors — propagate with `?`\n\
             2. Iterator chains over explicit loops — `.iter().map(f).collect()`, avoid unnecessary `.collect()`\n\
             3. `Vec::with_capacity()` when size is known\n\
             4. Derive common traits: `#[derive(Debug, Clone, PartialEq)]`, `#[derive(Default)]` when sensible\n\
             5. `if let`/`while let` for single-pattern matching instead of full match",
        ),
        "python" => Some(
            "### Python\n\
             \n\
             **Anti-patterns (Don't → Do):**\n\
             1. Mutable default arguments `def f(items=[])` → use `None` with `if items is None: items = []`\n\
             2. `typing.List`, `typing.Dict`, `typing.Optional` → built-in generics: `list[str]`, `str | None`\n\
             3. Bare/broad exception handling `except Exception: pass` → catch specific exceptions, log with context\n\
             4. `os.path.join()` → `pathlib.Path`: `Path(base) / \"data\" / \"file.csv\"`\n\
             5. `Any` type overuse → complete type annotations on all function signatures\n\
             \n\
             **Correct patterns:**\n\
             1. Modern type hints (3.10+): `list[int]`, `dict[str, Any]`, `str | None`\n\
             2. `uv` for packages, `ruff` for linting/formatting, `pyright` for types, `pytest` for testing\n\
             3. `pyproject.toml` over `setup.py`/`requirements.txt`\n\
             4. `dataclasses` for internal data, Pydantic for validation, TypedDict for dict shapes\n\
             5. `is` comparison for singletons: `if x is None:` not `if x == None:`",
        ),
        "typescript" => Some(
            "### TypeScript\n\
             \n\
             **Anti-patterns (Don't → Do):**\n\
             1. `any` type overuse → `unknown` when type is uncertain, Zod schemas for external data\n\
             2. Type assertion `as` abuse / `as unknown as T` → type guards, proper narrowing\n\
             3. Missing discriminated unions → model domain states with `'kind'`/`'type'` discriminant, `satisfies never` for exhaustiveness\n\
             4. Non-null assertion `!` abuse → handle null/undefined with narrowing, optional chaining, type guards\n\
             5. Enums → `as const` objects or string literal union types\n\
             \n\
             **Correct patterns:**\n\
             1. Strict tsconfig: `strict: true`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`\n\
             2. Explicit return types on exported functions\n\
             3. Zod schema validation for external data — derive types with `z.infer<typeof Schema>`\n\
             4. Discriminated unions with exhaustiveness: `default: throw new Error(\\`Unhandled: ${x satisfies never}\\`)`\n\
             5. `interface` for object shapes, `type` for unions/intersections/mapped types",
        ),
        "javascript" | "jsx" => Some(
            "### JavaScript\n\
             \n\
             **Anti-patterns (Don't → Do):**\n\
             1. Missing Promise error handling → every `.then()` needs `.catch()`, every `async/await` needs try/catch\n\
             2. Stale closures in React hooks → ensure exhaustive dependency arrays in useEffect/useCallback/useMemo\n\
             3. Event listener / timer memory leaks → cleanup with `removeEventListener`, `clearInterval`, `AbortController`\n\
             4. `var` declarations → `const` by default, `let` only for reassignment\n\
             5. Loose equality `==` → always `===` and `!==`\n\
             \n\
             **Correct patterns:**\n\
             1. Proper useEffect async: define async inside effect, call it, return cleanup with AbortController\n\
             2. `const` by default, destructuring at function boundaries\n\
             3. Named exports over default exports — aids tree-shaking and refactoring\n\
             4. Template literals over string concatenation\n\
             5. `jsconfig.json` with `checkJs: true` for type safety in JS projects",
        ),
        "go" => Some(
            "### Go\n\
             \n\
             **Anti-patterns (Don't → Do):**\n\
             1. `ioutil` package → `io.ReadAll`, `os.ReadFile`, `os.MkdirTemp` (deprecated since Go 1.16)\n\
             2. Pre-modern patterns → `slices.Contains()`, `min`/`max` builtins (1.21), `for range n` (1.22)\n\
             3. Java-style large interfaces at producer → accept interfaces at consumer, return structs, keep interfaces small (1-3 methods)\n\
             4. Error wrapping with `%v` → `fmt.Errorf(\"context: %w\", err)`, use `errors.Is`/`errors.As`\n\
             5. `context.Background()` deep in call chains → ctx as first param, pass through entire chain, never store in structs\n\
             \n\
             **Correct patterns:**\n\
             1. Table-driven tests with `t.Parallel()` and `t.Run()` subtests\n\
             2. `errgroup` for structured concurrency: `g, ctx := errgroup.WithContext(ctx)`\n\
             3. Functional options pattern: `WithPort(8080)`, `WithTimeout(30*time.Second)`\n\
             4. `slog` for structured logging (Go 1.21+), not `log.Println`\n\
             5. No name stuttering: `package kv; type Store` not `type KVStore`",
        ),
        "java" => Some(
            "### Java\n\
             \n\
             **Anti-patterns (Don't → Do):**\n\
             1. `@Autowired` field injection → constructor injection with `final` fields (Spring 4.3+ auto-infers)\n\
             2. `Optional.get()` without check → `orElseThrow(() -> new NotFoundException(id))`, Optional for return types only\n\
             3. `throws Exception` / bare catches → declare and catch specific exceptions, log with context\n\
             4. `Date`/`Calendar`/`SimpleDateFormat` → `java.time`: `LocalDate`, `ZonedDateTime`, `DateTimeFormatter`\n\
             5. Raw types `List items` → `List<String> items = new ArrayList<>()`\n\
             \n\
             **Correct patterns:**\n\
             1. Records for data carriers (Java 16+): `public record UserDto(String name, String email) {}`\n\
             2. Sealed classes + pattern matching (Java 17+/21+) with switch expressions\n\
             3. Text blocks `\"\"\"` for multi-line strings (Java 15+)\n\
             4. Pattern matching instanceof (Java 16+): `if (obj instanceof String s) { s.length(); }`\n\
             5. Immutable collections: `List.of()`, `Map.of()`, `Set.of()`",
        ),
        "kotlin" => Some(
            "### Kotlin\n\
             \n\
             **Anti-patterns (Don't → Do):**\n\
             1. `!!` (not-null assertion) overuse → `?.let`, `?:`, `?.` chaining, or redesign to eliminate nullability\n\
             2. `GlobalScope.launch`/`async` → lifecycle-bound scopes: `viewModelScope`, `lifecycleScope`, injected `CoroutineScope`\n\
             3. `runBlocking` in production code → only for `main()` and tests, use suspend functions\n\
             4. Mutable `var` in data classes → `val` + `List` (not `MutableList`), immutability by default\n\
             5. `enum` when sealed class is needed → `sealed class`/`sealed interface` for state with per-variant data\n\
             \n\
             **Correct patterns:**\n\
             1. `val` over `var`, `List` over `MutableList` — expose read-only interfaces\n\
             2. Structured concurrency: `coroutineScope { launch { a() }; launch { b() } }`\n\
             3. Sealed class/interface for all state and result types\n\
             4. `Sequence` for large collections with chained operations\n\
             5. `require`/`check`/`error` for preconditions: `require(age >= 0) { \"Age must be non-negative\" }`",
        ),
        _ => None,
    }
}

/// Assembles a language-patterns memory from detected project languages.
/// Returns None if no detected languages have pattern data.
fn build_language_patterns_memory(languages: &[String]) -> Option<String> {
    let sections: Vec<&str> = languages
        .iter()
        .filter_map(|lang| language_patterns(lang))
        .collect();

    if sections.is_empty() {
        return None;
    }

    let mut content = String::from(
        "# Language Patterns\n\n\
         Per-language anti-patterns and correct patterns for this project's languages.\n\
         Each section lists the top 5 mistakes LLMs make and the top 5 idiomatic patterns.\n\n",
    );

    for (i, section) in sections.iter().enumerate() {
        if i > 0 {
            content.push_str("\n---\n\n");
        }
        content.push_str(section);
        content.push('\n');
    }

    Some(content)
}

fn build_system_prompt_draft(
    languages: &[String],
    entry_points: &[String],
    project_root: Option<&Path>,
    projects: Option<&[crate::workspace::DiscoveredProject]>,
    libraries: &[crate::library::registry::LibraryEntry],
) -> String {
    let mut draft = String::new();
    draft.push_str("# Project — Code Explorer Guidance\n\n");

    let projects_slice = projects.unwrap_or(&[]);

    // Entry points section
    draft.push_str("## Entry Points\n");
    if entry_points.is_empty() {
        draft.push_str("- Explore with `list_dir(\".\")` then `list_symbols` on key files\n");
    } else {
        for ep in entry_points {
            draft.push_str(&format!("- `{}` — start here\n", ep));
        }
    }
    draft.push('\n');

    // Key abstractions — placeholder for the LLM to fill
    draft.push_str("## Key Abstractions\n");
    draft.push_str("- [Discover with `list_symbols` on main source directories]\n\n");

    // Search tips
    draft.push_str("## Search Tips\n");
    if !languages.is_empty() {
        draft.push_str(&format!("- This is a {} project\n", languages.join("/")));
    }
    draft.push_str("- Use specific terms over generic ones (e.g., avoid 'data', 'utils')\n");
    if projects_slice.len() > 1 {
        draft.push_str(
            "- **Workspace mode:** always scope `semantic_search` with `project: \"<id>\"` — \
             broad terms match all projects and return mixed results\n",
        );
        for p in projects_slice {
            let example_term = if p.languages.iter().any(|l| l == "rust") {
                "key type or trait name"
            } else if p
                .languages
                .iter()
                .any(|l| l == "typescript" || l == "javascript")
            {
                "handler or component name"
            } else if p.languages.iter().any(|l| l == "python") {
                "class or function name"
            } else {
                "concept specific to this project"
            };
            draft.push_str(&format!(
                "  - `{}`: `semantic_search(\"<{}>\", project: \"{}\")` \
                 — [fill in good query examples during onboarding]\n",
                p.id, example_term, p.id
            ));
        }
    }
    draft.push('\n');

    // Language-specific navigation hints — cap at 3 to keep the draft concise
    let hints: Vec<_> = languages
        .iter()
        .filter_map(|lang| language_navigation_hints(lang).map(|h| (lang.as_str(), h)))
        .take(3)
        .collect();
    if !hints.is_empty() {
        draft.push_str("## Language Navigation\n");
        for (lang, hint) in &hints {
            draft.push_str(&format!("**{}:**\n{}\n\n", lang, hint));
        }
    }

    // Language patterns reference — only if at least one language has patterns
    let has_patterns = languages.iter().any(|l| language_patterns(l).is_some());
    if has_patterns {
        let pattern_langs: Vec<&str> = languages
            .iter()
            .filter(|l| language_patterns(l).is_some())
            .map(|s| s.as_str())
            .collect();
        draft.push_str("## Language Patterns\n");
        draft.push_str(&format!(
            "This project uses {}. Read `memory(action=\"read\", topic=\"language-patterns\")` before writing, editing, or reviewing code.\n\n",
            pattern_langs.join(", ")
        ));
    }

    // Navigation strategy — per-project subsections for multi-project workspaces
    if projects_slice.len() > 1 {
        draft.push_str("## Navigation Strategy\n\n");
        draft.push_str("1. `memory(action=\"read\", topic=\"architecture\")` — orient yourself to the workspace\n");
        draft.push_str(
            "2. `semantic_search(\"your concept\")` — find relevant code across projects\n",
        );
        draft.push_str(
            "3. `memory(action=\"recall\", query=\"...\")` — search memories by meaning\n\n",
        );
        draft.push_str("**Per-project navigation:**\n\n");
        for p in projects_slice {
            let langs = if p.languages.is_empty() {
                String::new()
            } else {
                format!(" ({})", p.languages.join(", "))
            };
            draft.push_str(&format!("### {}{}\n", p.id, langs));
            draft.push_str(&format!(
                "1. `list_symbols(\"{}\")` — [fill in entry point during onboarding]\n",
                p.relative_root.display()
            ));
            draft.push_str(&format!(
                "2. `semantic_search(\"your concept\", scope=\"project:{}\")` — search within this project\n",
                p.id
            ));
            draft.push_str(&format!(
                "3. `memory(project: \"{}\", action=\"read\", topic=\"architecture\")` — project-specific knowledge\n\n",
                p.id
            ));
        }

        draft.push_str("**Cross-project navigation:**\n");
        draft.push_str("- **Quick lookups** (1–3 calls): pass `project: \"<id>\"` to scope the call — no state change.\n");
        draft.push_str("- **Sustained exploration** (reading memories, semantic search, many tool calls): \
                         use `activate_project(\"<id>\")`, but **always `activate_project` back to your original \
                         project when done.** Forgetting to return leaves all subsequent tool calls operating \
                         against the wrong project.\n");
        draft.push_str("- **Subagents:** the MCP server state is shared with the parent conversation. \
                         You **MUST** `activate_project` back to the original project before completing your task.\n\n");
    } else {
        draft.push_str("## Navigation Strategy\n");
        draft.push_str("1. `memory(action=\"read\", topic=\"architecture\")` — orient yourself\n");
        if !entry_points.is_empty() {
            draft.push_str(&format!(
                "2. `list_symbols(\"{}\")` — see main structure\n",
                entry_points[0]
            ));
        } else {
            draft.push_str("2. `list_symbols(\"src/\")` — see main structure\n");
        }
        draft.push_str("3. `semantic_search(\"your concept\")` — find relevant code\n");
        draft.push_str("4. `find_symbol(\"Name\", include_body=true)` — read implementation\n");
        draft.push_str("   - regex-like patterns belong in `grep`, not `find_symbol`\n");
        draft.push_str(
            "5. `memory(action=\"recall\", query=\"...\")` — search memories by meaning\n\n",
        );
    }

    // Project rules — empty section for the LLM to fill from exploration
    draft.push_str("## Project Rules\n");
    draft.push_str("- [Fill from Phase 1 exploration: linting, formatting, commit conventions]\n");

    // Workspace projects table — only for multi-project repos
    if projects_slice.len() > 1 {
        draft.push_str("\n## Workspace Projects\n\n");
        draft.push_str("| Project | Root | Languages | Build |\n");
        draft.push_str("|---------|------|-----------|-------|\n");
        for p in projects_slice {
            draft.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                p.id,
                p.relative_root.display(),
                p.languages.join(", "),
                p.manifest.as_deref().unwrap_or("-"),
            ));
        }
        draft.push('\n');

        // Load depends_on from workspace.toml if available
        if let Some(root) = project_root {
            let ws_path = crate::config::workspace::workspace_config_path(root);
            if let Ok(content) = std::fs::read_to_string(&ws_path) {
                if let Ok(ws) =
                    toml::from_str::<crate::config::workspace::WorkspaceConfig>(&content)
                {
                    let deps: Vec<_> = ws
                        .projects
                        .iter()
                        .filter(|p| !p.depends_on.is_empty())
                        .collect();
                    if !deps.is_empty() {
                        draft.push_str("**Cross-project dependencies:**\n");
                        for p in deps {
                            draft.push_str(&format!(
                                "- {} depends on {}\n",
                                p.id,
                                p.depends_on.join(", "),
                            ));
                        }
                        draft.push('\n');
                    }
                }
            }
        }

        draft.push_str(
            "Use `project: \"name\"` parameter to scope search/navigation to a specific project.\n\n",
        );

        draft.push_str(
            "**Per-project details:** Use `memory(project: \"<id>\", topic: \"architecture\")` \
             or `memory(project: \"<id>\", topic: \"conventions\")` for project-specific knowledge.\n\n",
        );
    }

    // Registered libraries — only included when at least one is registered
    if !libraries.is_empty() {
        draft.push_str("\n## Registered Libraries\n\n");
        for lib in libraries {
            let status = if lib.indexed {
                if lib.version.is_some()
                    && lib.version_indexed.is_some()
                    && lib.version != lib.version_indexed
                {
                    "indexed [stale]"
                } else {
                    "indexed"
                }
            } else {
                "not indexed"
            };
            draft.push_str(&format!(
                "- **{}** ({}) — {}\n",
                lib.name, lib.language, status
            ));
        }
        draft.push_str(
            "\nUse `scope=\"lib:<name>\"` with `find_symbol`, `list_symbols`, `grep`, \
             and `semantic_search` to navigate library code. \
             Run `index_project(scope=\"lib:<name>\")` to enable semantic search for a library.\n",
        );
    }

    // Auto-inject preferences from semantic memory (best-effort)
    if let Some(root) = project_root {
        if let Ok(conn) = crate::embed::index::open_db(root) {
            if crate::embed::index::ensure_vec_memories(&conn).is_ok() {
                let mut prefs = Vec::new();
                if let Ok(mut stmt) = conn.prepare(
                    "SELECT title, content FROM memories WHERE bucket = 'preferences' \
                     ORDER BY updated_at DESC LIMIT 10",
                ) {
                    if let Ok(rows) = stmt.query_map([], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    }) {
                        for row in rows.flatten() {
                            prefs.push(row);
                        }
                    }
                }
                if !prefs.is_empty() {
                    draft.push_str("\n## User Preferences\n\n");
                    for (title, content) in &prefs {
                        let summary = if content.len() > 200 {
                            let end = crate::tools::floor_char_boundary(content, 200);
                            format!("{}...", &content[..end])
                        } else {
                            content.clone()
                        };
                        draft.push_str(&format!("- **{}:** {}\n", title, summary));
                    }
                }
            }
        }
    }

    draft
}

/// Build the preamble prepended to the onboarding prompt for the subagent.
/// Instructs the subagent to activate the project before following the exploration steps.
fn build_subagent_preamble() -> String {
    let mut s = String::new();
    s.push_str("You are an onboarding subagent for codescout. ");
    s.push_str("Your job is to thoroughly explore this codebase and write project memories ");
    s.push_str("that will be used by every future session.\n\n");
    s.push_str("FIRST ACTION: Call activate_project(\".\", read_only: false) to initialize the ");
    s.push_str("project context. All subsequent tool calls depend on this.\n\n");
    s.push_str("Then follow the exploration and memory-writing instructions below exactly.\n\n");
    s.push_str("---\n\n");
    s
}

/// Build the epilogue appended to the onboarding prompt for the subagent.
/// Defines the return contract: what the subagent must include in its final response.
fn build_subagent_epilogue() -> String {
    let mut s = String::new();
    s.push_str("\n---\n\n");
    s.push_str("## Return Contract\n\n");
    s.push_str(
        "When you have completed ALL exploration steps and written ALL memories, end your \
response with this structured summary:\n\n",
    );
    s.push_str("**Exploration Summary:**\n");
    s.push_str("- What this system does (your own words, not the README's)\n");
    s.push_str("- The 5 most important types/modules (name, file, role)\n");
    s.push_str("- How a typical operation flows (concrete function names)\n");
    s.push_str("- What surprised you (things docs didn't mention)\n\n");
    s.push_str("**Memories Written:**\n");
    s.push_str(
        "- List each memory topic you wrote (e.g., \"architecture\", \"conventions\", etc.)\n\n",
    );
    s.push_str("**Warnings:**\n");
    s.push_str(
        "- Any issues encountered (index not built, LSP failures, files that couldn't be read)\n",
    );
    s.push_str("- Steps you couldn't fully complete and why\n\n");
    s.push_str(
        "This summary is returned to the main agent and shown to the user. Make it \
informative but concise — aim for 300-500 tokens total.\n\n",
    );
    s.push_str(
        "LAST ACTION: Call activate_project(\".\") before returning to ensure the parent's \
project state is unchanged.",
    );
    s
}

// ── Client detection ──────────────────────────────────────────────────────────

/// Extract the MCP client name from the peer info (set during initialize handshake).
fn client_name(ctx: &ToolContext) -> Option<String> {
    ctx.peer
        .as_ref()
        .and_then(|p| p.peer_info())
        .map(|info| info.client_info.name.clone())
}

/// Returns true if the client is known to support subagent spawning.
/// Conservative: only Claude Code for now. Add others as they gain support.
fn is_subagent_capable(name: Option<&str>) -> bool {
    name.is_some_and(|n| n.to_lowercase().contains("claude"))
}

/// Extract level-2 headings from markdown content with numbered line counts.
fn build_heading_map(prompt: &str) -> Vec<String> {
    let lines: Vec<&str> = prompt.lines().collect();
    let mut headings: Vec<(String, usize)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("## ") {
            headings.push((line.to_string(), i));
        }
    }
    headings
        .iter()
        .enumerate()
        .map(|(idx, (heading, start))| {
            let end = headings
                .get(idx + 1)
                .map(|(_, s)| *s)
                .unwrap_or(lines.len());
            format!("{}. {} ({} lines)", idx + 1, heading, end - start)
        })
        .collect()
}

/// Build dispatch instructions for full onboarding, adapted to client capability.
fn build_buffered_onboarding_instructions(prompt_path: &str, subagent_capable: bool) -> String {
    let p = prompt_path;
    let checklist = format!(
        "\
Read these phases in order — complete each before moving to the next:

  1. read_markdown(\"{p}\", heading=\"## THE IRON LAW\")
  2. read_markdown(\"{p}\", headings=[\"## Phase 0: Embedding Model Selection\", \"## Phase 1: Semantic Index Check\"])
  3. read_markdown(\"{p}\", heading=\"## Phase 2: Explore the Code\")
     → Execute ALL exploration steps before continuing
  4. read_markdown(\"{p}\", headings=[\"## Red Flags — STOP and Return to Phase 2\", \"## Common Rationalizations\"])
  5. read_markdown(\"{p}\", heading=\"## Phase 3: Write the Memories (Single-Project Mode)\")
     → Write all memories before continuing
  6. read_markdown(\"{p}\", headings=[\"## After Everything Is Created\", \"## Gathered Project Data\", \"## Return Contract\"])

For workspace mode, read these after step 2 (before step 3):
  read_markdown(\"{p}\", headings=[\"## Workspace Survey — Breadth-First\", \"## Workspace Deep Dives — Subagent Per-Project\"])
And after step 4 (before step 5):
  read_markdown(\"{p}\", heading=\"## Workspace Memory Synthesis\")"
    );

    if subagent_capable {
        format!(
            "\
Onboarding required — this project has not been explored yet.

Spawn a general-purpose subagent with model=sonnet to perform the exploration and \
memory writing. The subagent must read the onboarding prompt by phase — \
copy-paste these exact commands:

{checklist}

Do NOT read the onboarding prompt yourself — let the subagent handle it.

When the subagent completes, report its summary to the user. Then read whichever \
memories are relevant to the user's current task via memory(action=\"read\", topic=...).

Wait for the subagent to complete before continuing — onboarding is a prerequisite \
for all subsequent work.

If you cannot spawn subagents, follow the checklist yourself."
        )
    } else {
        format!(
            "\
Onboarding required — this project has not been explored yet.

{checklist}

Follow each phase's instructions to explore the codebase and write project memories."
        )
    }
}

/// Build dispatch instructions for version refresh, adapted to client capability.
fn build_buffered_refresh_instructions(
    prompt_path: &str,
    stored: Option<u32>,
    current: u32,
    subagent_capable: bool,
) -> String {
    let stored_str = stored
        .map(|v| format!("v{v}"))
        .unwrap_or_else(|| "pre-versioning".to_string());

    if subagent_capable {
        format!(
            "\
System prompt outdated ({stored_str} → v{current}) — a lightweight refresh is needed.

Spawn a general-purpose subagent with model=sonnet to regenerate the system prompt. \
The subagent must:
  read_markdown(\"{prompt_path}\")  — read the full refresh prompt (it's short)

The subagent will re-read memories and regenerate system-prompt.md without \
re-exploring the codebase.

When the subagent completes, continue with the user's original task."
        )
    } else {
        format!(
            "\
System prompt outdated ({stored_str} → v{current}) — a lightweight refresh is needed.

Read the refresh prompt:
  read_markdown(\"{prompt_path}\")

Follow it to re-read memories and regenerate system-prompt.md."
        )
    }
}

/// Build the subagent task prompt for refreshing the system prompt.
///
/// The subagent will re-read memories and regenerate system-prompt.md without
/// re-exploring the codebase — memory already captured all the relevant knowledge.
fn build_prompt_refresh_subagent_prompt(memory_topics: &[String]) -> String {
    let memory_reads = memory_topics
        .iter()
        .map(|t| format!("  - memory(action=\"read\", topic=\"{t}\")"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "\
System prompt refresh — the stored onboarding version is behind the current codescout version.

Steps:
1. activate_project(\".\", read_only: false) — enable writes
2. Read each project memory that contributes to the system prompt:
{memory_reads}
3. Read the current system-prompt.md (if it exists) to understand its structure
4. Regenerate system-prompt.md using the standard template sections:
   - ## Entry Points
   - ## Key Abstractions
   - ## Search Tips
   - ## Navigation Strategy
   - ## Project Rules
5. Write the updated content to .codescout/system-prompt.md
6. Do NOT re-explore the codebase — the memories already contain the relevant knowledge
7. activate_project(\".\") — restore normal state

When done, report: \"System prompt refreshed (vN → vM).\"",
        memory_reads = memory_reads,
    )
}

/// Build a self-contained onboarding prompt for one project in a workspace.
///
/// Each per-project subagent gets this prompt. It includes:
/// - Iron Law, exploration steps, red flags (from the shared template)
/// - Project-specific context (id, root, languages, siblings)
/// - Per-project memory writing instructions (scoped with project= param)
/// - Return contract
#[allow(dead_code)]
fn build_per_project_prompt(
    project: &crate::workspace::DiscoveredProject,
    siblings: &[(String, Vec<String>)],
) -> String {
    let mut prompt = String::new();

    // Iron Law (from shared template)
    prompt.push_str("## THE IRON LAW\n\n");
    prompt.push_str(
        "```\nNO MEMORIES WRITTEN WITHOUT COMPLETING ALL EXPLORATION STEPS FIRST\n```\n\n",
    );
    prompt.push_str("You may only call `memory(action: \"write\", ...)` after you have:\n");
    prompt.push_str("1. Completed ALL exploration steps below\n");
    prompt.push_str("2. Verified EVERY item in the Phase 2 Gate Checklist\n\n");
    prompt.push_str("---\n\n");

    // Project context
    prompt.push_str("## Your Project\n\n");
    prompt.push_str(&format!("- **ID:** {}\n", project.id));
    prompt.push_str(&format!(
        "- **Root:** {}\n",
        project.relative_root.display()
    ));
    prompt.push_str(&format!(
        "- **Languages:** {}\n",
        project.languages.join(", ")
    ));
    if let Some(ref manifest) = project.manifest {
        prompt.push_str(&format!("- **Manifest:** {}\n", manifest));
    }

    if !siblings.is_empty() {
        prompt.push_str("\n**Sibling projects** (for context — Do NOT deep-dive these):\n");
        for (id, langs) in siblings {
            prompt.push_str(&format!("- {} ({})\n", id, langs.join(", ")));
        }
    }
    prompt.push_str("\n---\n\n");

    // Phase 2: Explore (scoped to this project)
    prompt.push_str("## Phase 2: Explore the Code\n\n");
    prompt.push_str("Explore ONLY your project root. Do NOT explore sibling projects.\n\n");
    prompt.push_str(&format!(
        "### Step 1: Map the Codebase Structure\n\n\
         - `list_dir(\"{root}\")` — top-level structure\n\
         - `list_dir` on each subdirectory\n\
         - `read_file` on the build config\n\
         - `read_markdown(\"README.md\")` if present\n\n",
        root = project.relative_root.display()
    ));
    prompt.push_str(
        "### Step 2: Full Symbol Survey\n\n\
         - Run `list_symbols` on the main source directory\n\
         - Run `list_symbols` on EACH subdirectory individually\n\
         - Survey at least 5 distinct source files\n\n",
    );
    prompt.push_str(
        "### Step 3: Read Core Implementations\n\n\
         - Identify 5+ central types/functions from Step 2\n\
         - `find_symbol(name, include_body=true)` for each\n\
         - Read the FULL body, not just signatures\n\n",
    );
    prompt.push_str(
        "### Step 4: Read Architecture Documentation\n\n\
         - `read_markdown` on any docs found in the project\n\
         - Read completely — do not skim\n\n",
    );
    prompt.push_str(
        "### Step 5: Trace Two Data Flows\n\n\
         - Trace the most representative operation end-to-end\n\
         - Trace a second distinct path (error, write vs read, etc.)\n\n",
    );
    prompt.push_str(
        "### Step 6: Concept-Level Search (5+ queries)\n\n\
         - Error handling, data flow, testing, config, domain concept\n\
         - Use `semantic_search` or `grep` as fallback\n\n",
    );
    prompt.push_str(
        "### Step 7: Examine Tests\n\n\
         - `list_symbols` on test directory\n\
         - Read 2-3 test files for patterns\n\n",
    );

    // Phase 2 Gate Checklist
    prompt.push_str(
        "### Phase 2 Gate Checklist\n\n\
         Before writing ANY memory, verify ALL true:\n\
         - [ ] Listed structure AND ran list_dir on major subdirectories\n\
         - [ ] Symbol survey on 5+ source files\n\
         - [ ] Read full body of 5+ core implementations\n\
         - [ ] Read all architecture docs\n\
         - [ ] Traced two data flows\n\
         - [ ] Ran 5+ concept queries\n\
         - [ ] Read 2-3 test files\n\n\
         ---\n\n",
    );

    // Red Flags
    prompt.push_str(
        "## Red Flags — STOP and Return to Phase 2\n\n\
         If you notice any of these, STOP and go back:\n\
         - \"I have a good enough picture\" — No, read the code.\n\
         - \"The README covers this\" — READMEs lie. Verify in code.\n\
         - \"This is similar to...\" — Explore anyway. Differences matter.\n\n\
         ---\n\n",
    );

    // Phase 3: Write per-project memories
    prompt.push_str("## Phase 3: Write the Memories\n\n");
    prompt.push_str(&format!(
        "Write these memories using `memory(action=\"write\", project=\"{id}\", topic=\"...\", content=\"...\")`.\n\n",
        id = project.id
    ));
    prompt.push_str(
        "### 1. `project-overview`\n\
         Purpose, tech stack, key dependencies, runtime requirements. 15-30 lines.\n\n\
         ### 2. `architecture`\n\
         Module structure, key abstractions, data flow, design patterns. 20-40 lines.\n\
         Include 3-5 good `semantic_search(query, project=\"{id}\")` examples.\n\n\
         ### 3. `conventions`\n\
         Language/framework patterns, naming, testing approach. 15-30 lines.\n\n",
    );

    // Return contract
    prompt.push_str("---\n\n");
    prompt.push_str(
        "## Return Contract\n\n\
         Return a summary with:\n\
         - What this project does (your own words)\n\
         - 3-5 most important types/modules\n\
         - How a typical operation flows\n\
         - Memories written (list topics)\n\
         - Any issues encountered\n",
    );

    prompt
}

/// Build the workspace synthesis prompt that runs after all per-project subagents complete.
///
/// The main agent (or a synthesis subagent) reads this to:
/// 1. Read back all per-project memories
/// 2. Write 5 workspace-level memories
/// 3. Generate the system prompt
/// 4. Offer to refresh CLAUDE.md with memory references
#[allow(dead_code)]
fn build_synthesis_prompt(projects: &[(String, Vec<String>)]) -> String {
    let mut prompt = String::new();

    // Step 1: Read back per-project memories
    prompt.push_str("## Read Per-Project Memories\n\n");
    prompt.push_str("Read these memories to understand what each subagent discovered:\n\n");
    for (id, _langs) in projects {
        prompt.push_str(&format!(
            "- `memory(action=\"read\", project=\"{id}\", topic=\"project-overview\")`\n\
             - `memory(action=\"read\", project=\"{id}\", topic=\"architecture\")`\n\
             - `memory(action=\"read\", project=\"{id}\", topic=\"conventions\")`\n"
        ));
    }
    prompt.push_str("\n---\n\n");

    // Step 2: Write workspace-level memories
    prompt.push_str("## Write Workspace Memories\n\n");
    prompt.push_str(
        "Write these 5 workspace-level memories (no `project:` parameter = workspace-level):\n\n",
    );
    prompt.push_str(
        "### 1. `architecture`\n\
         Workspace-level architecture:\n\
         - Project map: each project's purpose (1 sentence each)\n\
         - Cross-project dependencies (which imports from which)\n\
         - Shared infrastructure (CI, deployment, tooling)\n\
         15-30 lines.\n\n\
         ### 2. `conventions`\n\
         Shared patterns across projects: commit style, PR process, CI rules.\n\
         Per-project: reference `memory(project=\"{id}\", topic=\"conventions\")`.\n\
         15-30 lines.\n\n\
         ### 3. `development-commands`\n\
         Workspace-level build/test/lint commands. Per-project commands go in per-project memories.\n\
         10-20 lines.\n\n\
         ### 4. `domain-glossary`\n\
         Terms used across multiple projects. Project-specific terms go in per-project memories.\n\
         10-20 lines.\n\n\
         ### 5. `gotchas`\n\
         Cross-project pitfalls, version mismatches, integration gotchas.\n\
         10-20 lines.\n\n",
    );

    // Step 3: System prompt
    prompt.push_str("---\n\n## Generate System Prompt\n\n");
    prompt.push_str(
        "Write `system-prompt.md` using `memory(action=\"write\", topic=\"system-prompt\", content=\"...\")`.\n\
         Include: entry points per project, key abstractions, search tips scoped by project,\n\
         navigation strategy for the workspace.\n\n",
    );

    // Step 4: CLAUDE.md refresh
    prompt.push_str("---\n\n## Refresh CLAUDE.md\n\n");
    prompt.push_str(
        "Read `read_markdown(\"CLAUDE.md\")` to see its heading structure.\n\n\
         Compare each section with the memories you just wrote. For sections that\n\
         overlap with memory content, offer to replace the body with a memory reference:\n\
         `See codescout memory 'architecture' (Key Patterns section).`\n\n\
         **preserve user-specific content:** personal preferences, code style rules,\n\
         iron rules, git workflow specifics, private notes — anything not derivable\n\
         from the codebase. Do NOT touch sections the user wrote for their own use.\n\n\
         **Add memory discovery hints** if CLAUDE.md doesn't already list available memories.\n\n\
         Present a summary of proposed changes and ask for approval before modifying.\n\n",
    );

    // Return contract
    prompt.push_str("---\n\n## Return Contract\n\n");
    prompt.push_str(
        "Return a summary with:\n\
         - Workspace-level memories written (list topics)\n\
         - Cross-project patterns discovered\n\
         - CLAUDE.md changes proposed/applied\n\
         - Any issues or gaps\n",
    );

    prompt
}

/// Build dispatch instructions for workspace onboarding.
#[allow(dead_code)]
fn build_workspace_instructions(
    main_prompt_path: &str,
    project_prompts: &[(String, String)],
    synthesis_path: &str,
    subagent_capable: bool,
) -> String {
    let p = main_prompt_path;

    if subagent_capable {
        let mut instructions = format!(
            "\
Onboarding required — this is a workspace with {} projects.

Step 1: Read prerequisites from the main prompt:
  read_markdown(\"{p}\", headings=[\"## Phase 0: Embedding Model Selection\", \"## Phase 1: Semantic Index Check\"])

Step 2: Spawn {} subagents IN PARALLEL — one per project:",
            project_prompts.len(),
            project_prompts.len(),
        );

        for (id, path) in project_prompts {
            instructions.push_str(&format!(
                "\n  - {id}: read_markdown(\"{path}\") and follow all instructions",
            ));
        }

        instructions.push_str(&format!(
            "\n\n\
Step 3: Wait for ALL subagents to complete.\n\n\
Step 4: Read the synthesis prompt and write workspace memories:\n\
  read_markdown(\"{synthesis_path}\")\n\n\
Follow the synthesis instructions to read back per-project memories,\n\
write workspace-level memories, generate the system prompt, and\n\
offer to refresh CLAUDE.md."
        ));

        instructions
    } else {
        let mut instructions = format!(
            "\
Onboarding required — this is a workspace with {} projects.

Step 1: Read prerequisites:
  read_markdown(\"{p}\", headings=[\"## Phase 0: Embedding Model Selection\", \"## Phase 1: Semantic Index Check\"])

Step 2: Explore each project one at a time:",
            project_prompts.len(),
        );

        for (id, path) in project_prompts {
            instructions.push_str(&format!(
                "\n  - {id}: read_markdown(\"{path}\") and follow all instructions",
            ));
        }

        instructions.push_str(&format!(
            "\n\n\
Step 3: Read the synthesis prompt and write workspace memories:\n\
  read_markdown(\"{synthesis_path}\")"
        ));

        instructions
    }
}

/// Gather staleness state for protected memory topics.
/// Returns a JSON object keyed by topic name, suitable for inclusion
/// in the onboarding result.
fn gather_protected_memory_state(
    memory: &crate::memory::MemoryStore,
    memories_dir: &std::path::Path,
    project_root: &std::path::Path,
    protected: &[String],
) -> Value {
    use crate::memory::anchors::{anchor_path_for_topic, check_path_staleness, read_anchor_file};

    // Programmatic topics are always machine-generated — exclude from protection
    const PROGRAMMATIC: &[&str] = &["onboarding", "language-patterns"];

    let mut result = serde_json::Map::new();

    for topic in protected {
        if PROGRAMMATIC.contains(&topic.as_str()) {
            continue;
        }

        let content = match memory.read(topic) {
            Ok(Some(c)) => c,
            _ => {
                // Topic doesn't exist — signal to create fresh
                result.insert(topic.clone(), json!({ "exists": false }));
                continue;
            }
        };

        let anchor_path = anchor_path_for_topic(memories_dir, topic);
        let staleness = if anchor_path.exists() {
            match read_anchor_file(&anchor_path)
                .and_then(|af| check_path_staleness(project_root, &af))
            {
                Ok(report) => json!({
                    "stale_files": report.stale_files,
                    "untracked": false,
                }),
                Err(_) => json!({
                    "stale_files": [],
                    "untracked": true,
                }),
            }
        } else {
            json!({
                "stale_files": [],
                "untracked": true,
            })
        };

        result.insert(
            topic.clone(),
            json!({
                "exists": true,
                "content": content,
                "staleness": staleness,
            }),
        );
    }

    Value::Object(result)
}

#[async_trait::async_trait]
impl Tool for Onboarding {
    fn name(&self) -> &str {
        "onboarding"
    }
    fn description(&self) -> &str {
        "Perform initial project discovery: detect languages, read key files \
         (README, build config, CLAUDE.md), and return instructions for creating \
         project memories and a system prompt draft. Requires an active project. \
         Returns status if already onboarded (use force=true to re-scan)."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "force": {
                    "type": "boolean",
                    "description": "Force full re-scan even if already onboarded (default: false)"
                },
                "refresh_prompt": {
                    "type": "boolean",
                    "description": "Regenerate system prompt from current templates without re-exploring (default: false)"
                }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let root = ctx.agent.require_project_root().await?;
        let force = parse_bool_param(&input["force"]);
        let refresh_prompt = parse_bool_param(&input["refresh_prompt"]);

        // Explicit prompt refresh: regenerate system-prompt.md from memories without re-exploring.
        // Takes effect only when not doing a full force re-scan.
        if refresh_prompt && !force {
            let status = ctx
                .agent
                .with_project(|p| {
                    let has_config = p.root.join(".codescout").join("project.toml").exists();
                    let memories = p.memory.list()?;
                    let has_onboarding_memory = memories.iter().any(|m| m == "onboarding");
                    Ok((has_config, has_onboarding_memory, memories))
                })
                .await?;
            let (has_config, has_onboarding_memory, memories) = status;
            if !has_config || !has_onboarding_memory {
                return Err(super::RecoverableError::with_hint(
                    "refresh_prompt requires a fully onboarded project",
                    "Run onboarding() without any flags first to perform the initial onboarding.",
                )
                .into());
            }

            let (stored_version, config_languages) = ctx
                .agent
                .with_project(|p| {
                    Ok((
                        p.config.project.onboarding_version,
                        p.config.project.languages.clone(),
                    ))
                })
                .await?;

            // Optimistic version write to disk so the refresh is not re-triggered
            let config_path = ctx
                .agent
                .with_project(|p| {
                    let config_path = p.root.join(".codescout").join("project.toml");
                    if config_path.exists() {
                        let mut config =
                            crate::config::project::ProjectConfig::load_or_default(&p.root)?;
                        config.project.onboarding_version = Some(ONBOARDING_VERSION);
                        let toml_str = toml::to_string_pretty(&config)?;
                        std::fs::write(&config_path, &toml_str)?;
                    }
                    Ok(config_path)
                })
                .await?;
            ctx.agent.reload_config_if_project_toml(&config_path).await;

            let subagent_prompt = build_prompt_refresh_subagent_prompt(&memories);

            return Ok(json!({
                "onboarded": true,
                "version_stale": false,
                "explicit_refresh": true,
                "stored_version": stored_version,
                "current_version": ONBOARDING_VERSION,
                "languages": config_languages,
                "config_created": false,
                "subagent_prompt": subagent_prompt,
            }));
        }

        // If already onboarded and not forced, return status instead of re-scanning
        if !force {
            let status = ctx
                .agent
                .with_project(|p| {
                    let has_config = p.root.join(".codescout").join("project.toml").exists();
                    let memories = p.memory.list()?;
                    let has_onboarding_memory = memories.iter().any(|m| m == "onboarding");
                    let private_memories = p.private_memory.list()?;
                    Ok((
                        has_config,
                        has_onboarding_memory,
                        memories,
                        private_memories,
                    ))
                })
                .await?;
            let (has_config, has_onboarding_memory, memories, private_memories) = status;
            if has_config && has_onboarding_memory {
                // --- Version check: refresh system prompt if stale ---
                let (stored_version, config_languages) = ctx
                    .agent
                    .with_project(|p| {
                        Ok((
                            p.config.project.onboarding_version,
                            p.config.project.languages.clone(),
                        ))
                    })
                    .await?;

                // Log downgrade (no action)
                if let Some(v) = stored_version {
                    if v > ONBOARDING_VERSION {
                        tracing::warn!(
                            "stored onboarding version ({}) is newer than compiled ({}) — skipping refresh",
                            v, ONBOARDING_VERSION
                        );
                    }
                }

                if onboarding_version_stale(stored_version) {
                    tracing::info!(
                        "onboarding version stale: stored={:?} current={}",
                        stored_version,
                        ONBOARDING_VERSION
                    );

                    // Optimistic version write to disk (prevents re-trigger across sessions)
                    let config_path = ctx
                        .agent
                        .with_project(|p| {
                            let config_path = p.root.join(".codescout").join("project.toml");
                            if config_path.exists() {
                                let mut config =
                                    crate::config::project::ProjectConfig::load_or_default(
                                        &p.root,
                                    )?;
                                config.project.onboarding_version = Some(ONBOARDING_VERSION);
                                let toml_str = toml::to_string_pretty(&config)?;
                                std::fs::write(&config_path, &toml_str)?;
                            }
                            Ok(config_path)
                        })
                        .await?;
                    // Reload in-memory config so subsequent calls in the same session
                    // see the updated version (prevents re-trigger within session)
                    ctx.agent.reload_config_if_project_toml(&config_path).await;

                    let subagent_prompt = build_prompt_refresh_subagent_prompt(&memories);

                    return Ok(json!({
                        "onboarded": true,
                        "version_stale": true,
                        "stored_version": stored_version,
                        "current_version": ONBOARDING_VERSION,
                        "languages": config_languages,
                        "config_created": false,
                        "subagent_prompt": subagent_prompt,
                    }));
                }

                let per_project_memories = ctx.agent.workspace_project_memories().await;

                let mut message = format!(
                    "Onboarding already performed. Available shared memories: {}. \
                     Use `memory(action=\"read\", topic=...)` to read relevant ones as needed for your current task. \
                     Do not read all memories at once — only read those relevant to what you're working on. \
                     Use `memory(action=\"recall\", query=\"...\")` to search memories by meaning when the topic name isn't known.",
                    memories.join(", ")
                );
                if !private_memories.is_empty() {
                    message.push_str(&format!(
                        " Private memories: {}. Read with `memory(action=\"read\", topic=..., private=true)`.",
                        private_memories.join(", ")
                    ));
                }
                if !per_project_memories.is_empty() {
                    message.push_str(" Per-project memories (use `project: \"<id>\"` parameter):");
                    for (id, topics) in &per_project_memories {
                        message.push_str(&format!(" {}: {};", id, topics.join(", ")));
                    }
                }
                let mut response = json!({
                    "onboarded": true,
                    "has_config": true,
                    "has_onboarding_memory": true,
                    "memories": memories,
                    "message": message,
                });
                if !private_memories.is_empty() {
                    response["private_memories"] = json!(private_memories);
                }
                if !per_project_memories.is_empty() {
                    let map: serde_json::Map<String, serde_json::Value> = per_project_memories
                        .into_iter()
                        .map(|(id, topics)| (id, json!(topics)))
                        .collect();
                    response["project_memories"] = serde_json::Value::Object(map);
                }
                return Ok(response);
            }
        }

        // Hardware detection runs after the file walk (Rust futures are lazy — this
        // just creates the future; it starts executing only when .await'd below).
        let hw_future = detect_hardware_context();

        // Detect languages by walking files
        let mut languages = std::collections::BTreeSet::new();
        let walker = ignore::WalkBuilder::new(&root)
            .hidden(true)
            .git_ignore(true)
            .build();
        for entry in walker.flatten() {
            if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                if let Some(lang) = crate::ast::detect_language(entry.path()) {
                    languages.insert(lang.to_string());
                }
            }
        }

        // List top-level entries
        let mut top_level = vec![];
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let suffix = if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    "/"
                } else {
                    ""
                };
                top_level.push(format!("{}{}", name, suffix));
            }
        }
        top_level.sort();

        // Resolve hardware detection and derive model options
        let hw = hw_future.await;
        let model_options = model_options_for_hardware(&hw);
        let recommended_model = model_options
            .first()
            .expect("model_options_for_hardware guarantees ≥1 entry")
            .id
            .clone();

        // Create .codescout/project.toml if it doesn't exist
        let config_dir = root.join(".codescout");
        let config_path = config_dir.join("project.toml");
        let created_config = if !config_path.exists() {
            std::fs::create_dir_all(&config_dir)?;
            let name = root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unnamed")
                .to_string();
            let langs: Vec<String> = languages.iter().cloned().collect();
            let config = crate::config::project::ProjectConfig {
                project: crate::config::project::ProjectSection {
                    name,
                    languages: langs,
                    encoding: "utf-8".into(),
                    system_prompt: None,
                    tool_timeout_secs: 60,
                    onboarding_version: Some(ONBOARDING_VERSION),
                },
                embeddings: crate::config::project::EmbeddingsSection {
                    model: recommended_model,
                    ..Default::default()
                },
                ignored_paths: Default::default(),
                security: Default::default(),
                memory: Default::default(),
                libraries: Default::default(),
            };
            let toml_str = toml::to_string_pretty(&config)?;
            std::fs::write(&config_path, &toml_str)?;
            // Reload in-memory config so the version is visible within this session
            ctx.agent.reload_config_if_project_toml(&config_path).await;
            true
        } else {
            false
        };

        // Gather rich context from well-known project files.
        // Pass the already-discovered project list from the workspace to avoid a
        // redundant discover_projects walk (the agent runs it at activation time).
        let discovered = ctx.agent.discovered_projects().await;
        let gathered = gather_project_context(&root, discovered);

        // Create workspace.toml for multi-project repos
        let workspace_config_path = crate::config::workspace::workspace_config_path(&root);
        if gathered.projects.len() > 1 && !workspace_config_path.exists() {
            let ws_config = crate::config::workspace::WorkspaceConfig {
                workspace: crate::config::workspace::WorkspaceSection {
                    name: root
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unnamed")
                        .to_string(),
                    discovery_max_depth: 3,
                },
                resources: Default::default(),
                exclude_projects: vec![],
                projects: gathered
                    .projects
                    .iter()
                    .map(|p| {
                        let project_abs = root.join(&p.relative_root);
                        let depends_on = crate::workspace::infer_depends_on(
                            &project_abs,
                            &root,
                            &gathered.projects,
                        );
                        crate::config::workspace::ProjectEntry {
                            id: p.id.clone(),
                            root: p.relative_root.to_string_lossy().to_string(),
                            languages: p.languages.clone(),
                            depends_on,
                        }
                    })
                    .collect(),
            };
            let toml_str = toml::to_string_pretty(&ws_config)?;
            std::fs::write(&workspace_config_path, &toml_str)?;
        }

        // Probe embedding index status (only opens existing DB, no network)
        let index_status = {
            let db_path = crate::embed::index::project_db_path(&root);
            if db_path.exists() {
                match crate::embed::index::open_db(&root)
                    .and_then(|conn| crate::embed::index::index_stats(&conn))
                {
                    Ok(stats) => json!({
                        "ready": stats.chunk_count > 0,
                        "files": stats.file_count,
                        "chunks": stats.chunk_count,
                    }),
                    Err(_) => json!({ "ready": false, "files": 0, "chunks": 0 }),
                }
            } else {
                json!({ "ready": false, "files": 0, "chunks": 0 })
            }
        };

        // Store onboarding result in memory
        let lang_list: Vec<String> = languages.iter().cloned().collect();
        ctx.agent
            .with_project(|p| {
                let summary = format!(
                    "Languages: {}\nHas README: {}\nHas CLAUDE.md: {}\nBuild file: {}\nEntry points: {}\nTest dirs: {}",
                    lang_list.join(", "),
                    gathered.readme_path.is_some(),
                    gathered.claude_md_exists,
                    gathered.build_file_name.as_deref().unwrap_or("none"),
                    if gathered.entry_points.is_empty() { "none".to_string() } else { gathered.entry_points.join(", ") },
                    if gathered.test_dirs.is_empty() { "none".to_string() } else { gathered.test_dirs.join(", ") },
                );
                p.memory.write("onboarding", &summary)?;

                // Write language-patterns memory (deterministic, from hardcoded content)
                if let Some(patterns) = build_language_patterns_memory(&lang_list) {
                    p.memory.write("language-patterns", &patterns)?;
                }

                Ok(())
            })
            .await?;

        // Write programmatic memories for each sub-project in workspace mode.
        if gathered.projects.len() > 1 {
            for project in &gathered.projects {
                let mem_dir = if project.relative_root == std::path::Path::new(".") {
                    root.join(".codescout").join("memories")
                } else {
                    root.join(".codescout")
                        .join("projects")
                        .join(&project.id)
                        .join("memories")
                };
                if let Ok(store) = crate::memory::MemoryStore::from_dir(mem_dir) {
                    let proj_summary = format!(
                        "Languages: {}\nRoot: {}\nManifest: {}",
                        project.languages.join(", "),
                        project.relative_root.display(),
                        project.manifest.as_deref().unwrap_or("none"),
                    );
                    let _ = store.write("onboarding", &proj_summary);
                    if let Some(patterns) = build_language_patterns_memory(&project.languages) {
                        let _ = store.write("language-patterns", &patterns);
                    }
                }
            }
        }

        // Gather protected memory state for the LLM merge flow
        let protected_memories = ctx
            .agent
            .with_project(|p| {
                let memories_dir = p.root.join(".codescout").join("memories");
                let protected = &p.config.memory.protected;
                Ok(gather_protected_memory_state(
                    &p.memory,
                    &memories_dir,
                    &p.root,
                    protected,
                ))
            })
            .await?;

        // Build the key-files manifest for the prompt (paths only, no content)
        let mut key_files: Vec<String> = Vec::new();
        if let Some(ref p) = gathered.readme_path {
            key_files.push(p.clone());
        }
        if gathered.claude_md_exists {
            key_files.push("CLAUDE.md".to_string());
        }
        if let Some(ref p) = gathered.build_file_name {
            key_files.push(p.clone());
        }

        // Build the onboarding instruction prompt
        let is_workspace = gathered.projects.len() > 1;
        let prompt = crate::prompts::build_onboarding_prompt(&crate::prompts::OnboardingContext {
            languages: &lang_list,
            top_level: &top_level,
            key_files: &key_files,
            ci_files: &gathered.ci_files,
            entry_points: &gathered.entry_points,
            test_dirs: &gathered.test_dirs,
            index_ready: index_status["ready"].as_bool().unwrap_or(false),
            index_files: index_status["files"].as_u64().unwrap_or(0) as usize,
            index_chunks: index_status["chunks"].as_u64().unwrap_or(0) as usize,
            projects: &gathered.projects,
            is_workspace,
        });

        // Build the system prompt draft scaffold
        let libraries: Vec<crate::library::registry::LibraryEntry> = ctx
            .agent
            .library_registry()
            .await
            .map(|r| r.all().to_vec())
            .unwrap_or_default();
        let system_prompt_draft = build_system_prompt_draft(
            &lang_list,
            &gathered.entry_points,
            Some(&root),
            Some(&gathered.projects),
            &libraries,
        );

        let discovered_projects: Vec<serde_json::Value> = gathered
            .projects
            .iter()
            .map(|p| {
                serde_json::json!({
                    "id": p.id,
                    "root": p.relative_root.to_string_lossy(),
                    "languages": p.languages,
                    "manifest": p.manifest,
                })
            })
            .collect();

        let features_suggestion = gathered.features_md.is_none().then_some(
            "No FEATURES.md found. Consider creating docs/FEATURES.md to document \
             implemented capabilities — helps agents understand what's already built \
             and avoid re-suggesting existing features.",
        );

        // Per-project protected memory state for workspace mode.
        let (workspace_mode, per_project_protected) = if gathered.projects.len() > 1 {
            let protected = ctx
                .agent
                .with_project(|p| Ok(p.config.memory.protected.clone()))
                .await
                .unwrap_or_default();
            let mut map = serde_json::Map::new();
            for project in &gathered.projects {
                let mem_dir = if project.relative_root == std::path::Path::new(".") {
                    root.join(".codescout").join("memories")
                } else {
                    root.join(".codescout")
                        .join("projects")
                        .join(&project.id)
                        .join("memories")
                };
                let project_root = root.join(&project.relative_root);
                if let Ok(store) = crate::memory::MemoryStore::from_dir(mem_dir.clone()) {
                    let state =
                        gather_protected_memory_state(&store, &mem_dir, &project_root, &protected);
                    map.insert(project.id.clone(), state);
                }
            }
            (true, Some(Value::Object(map)))
        } else {
            (false, None)
        };

        // Build the subagent prompt by concatenating preamble + onboarding prompt +
        // system prompt draft + gathered data + epilogue
        let subagent_prompt = {
            let mut sp = build_subagent_preamble();
            sp.push_str(&prompt);
            if !system_prompt_draft.is_empty() {
                sp.push_str("\n\n## System Prompt Draft\n\n");
                sp.push_str(&system_prompt_draft);
            }
            if let Some(suggestion) = features_suggestion {
                sp.push_str(&format!("\n\n> {suggestion}"));
            }
            // Append gathered data that the subagent needs
            sp.push_str("\n\n## Gathered Data\n\n");
            sp.push_str(&format!(
                "**Hardware:** {}\n\n",
                serde_json::to_string_pretty(&hw).unwrap_or_default()
            ));
            sp.push_str(&format!(
                "**Model options:** {}\n\n",
                serde_json::to_string_pretty(&model_options).unwrap_or_default()
            ));
            if !protected_memories.is_null() {
                sp.push_str(&format!(
                    "**Protected memories:** {}\n\n",
                    serde_json::to_string_pretty(&protected_memories).unwrap_or_default()
                ));
            }
            if workspace_mode {
                if let Some(ref ppm) = per_project_protected {
                    if !ppm.is_null() {
                        sp.push_str(&format!(
                            "**Per-project protected memories:** {}\n\n",
                            serde_json::to_string_pretty(ppm).unwrap_or_default()
                        ));
                    }
                }
            }
            sp.push_str(&build_subagent_epilogue());
            sp
        };

        // Optimistic version write for full onboarding (force=true on existing project)
        ctx.agent
            .with_project(|p| {
                let config_path = p.root.join(".codescout").join("project.toml");
                if config_path.exists() {
                    let mut config =
                        crate::config::project::ProjectConfig::load_or_default(&p.root)?;
                    config.project.onboarding_version = Some(ONBOARDING_VERSION);
                    let toml_str = toml::to_string_pretty(&config)?;
                    std::fs::write(&config_path, &toml_str)?;
                }
                Ok(())
            })
            .await?;

        Ok(json!({
            "languages": lang_list,
            "top_level": top_level,
            "config_created": created_config,
            "has_readme": gathered.readme_path.is_some(),
            "has_claude_md": gathered.claude_md_exists,
            "build_file": gathered.build_file_name,
            "entry_points": gathered.entry_points,
            "test_dirs": gathered.test_dirs,
            "ci_files": gathered.ci_files,
            "features_md": gathered.features_md,
            "index_status": index_status,
            "workspace_mode": workspace_mode,
            "projects": discovered_projects,
            "subagent_prompt": subagent_prompt,
        }))
    }

    async fn call_content(
        &self,
        input: Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<Vec<rmcp::model::Content>> {
        let val = self.call(input, ctx).await?;

        // If there's a subagent prompt, write it to a temp markdown file and return
        // compact instructions with heading navigation.
        if let Some(prompt) = val["subagent_prompt"].as_str() {
            let compact = format_onboarding(&val);

            let root = ctx.agent.require_project_root().await?;
            let tmp_dir = root.join(".codescout").join("tmp");
            std::fs::create_dir_all(&tmp_dir)?;
            let prompt_path = tmp_dir.join("onboarding-prompt.md");
            std::fs::write(&prompt_path, prompt)?;
            let rel_path = ".codescout/tmp/onboarding-prompt.md";
            let sections = build_heading_map(prompt);

            let name = client_name(ctx);
            let subagent = is_subagent_capable(name.as_deref());

            // Determine which instruction builder based on whether this is a
            // version refresh (has stored_version) or full onboarding.
            let instructions =
                if val.get("version_stale").is_some() || val.get("explicit_refresh").is_some() {
                    let stored = val["stored_version"].as_u64().map(|v| v as u32);
                    let current = val["current_version"].as_u64().unwrap_or(0) as u32;
                    build_buffered_refresh_instructions(rel_path, stored, current, subagent)
                } else {
                    build_buffered_onboarding_instructions(rel_path, subagent)
                };

            // For workspaces, also write per-project and synthesis prompt files.
            let workspace_fields = if val["workspace_mode"].as_bool().unwrap_or(false) {
                let projects_val = val["projects"].as_array();
                if let Some(projects) = projects_val {
                    let mut project_prompts = Vec::new();
                    let all_projects: Vec<(String, Vec<String>)> = projects
                        .iter()
                        .filter_map(|p| {
                            let id = p["id"].as_str()?.to_string();
                            let langs: Vec<String> = p["languages"]
                                .as_array()?
                                .iter()
                                .filter_map(|l| l.as_str().map(String::from))
                                .collect();
                            Some((id, langs))
                        })
                        .collect();

                    for p in projects {
                        let id = p["id"].as_str().unwrap_or("unknown");
                        let project = crate::workspace::DiscoveredProject {
                            id: id.to_string(),
                            relative_root: std::path::PathBuf::from(
                                p["root"].as_str().unwrap_or("."),
                            ),
                            languages: p["languages"]
                                .as_array()
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|l| l.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            manifest: p["manifest"].as_str().map(String::from),
                        };
                        let siblings: Vec<(String, Vec<String>)> = all_projects
                            .iter()
                            .filter(|(sid, _)| sid != id)
                            .cloned()
                            .collect();

                        let prompt_content = build_per_project_prompt(&project, &siblings);
                        let file_name = format!("onboarding-project-{}.md", id);
                        let file_path = tmp_dir.join(&file_name);
                        std::fs::write(&file_path, &prompt_content)?;

                        let rel = format!(".codescout/tmp/{}", file_name);
                        project_prompts.push((id.to_string(), rel));
                    }

                    // Write synthesis prompt
                    let synthesis_content = build_synthesis_prompt(&all_projects);
                    let synthesis_file = tmp_dir.join("onboarding-workspace-synthesis.md");
                    std::fs::write(&synthesis_file, &synthesis_content)?;
                    let synthesis_rel =
                        ".codescout/tmp/onboarding-workspace-synthesis.md".to_string();

                    // Build workspace-specific instructions (overrides the single-project ones)
                    let ws_instructions = build_workspace_instructions(
                        rel_path,
                        &project_prompts,
                        &synthesis_rel,
                        subagent,
                    );

                    Some((project_prompts, synthesis_rel, ws_instructions))
                } else {
                    None
                }
            } else {
                None
            };

            let response = if let Some((project_prompts, synthesis_path, ws_instructions)) =
                workspace_fields
            {
                let pp_json: Vec<Value> = project_prompts
                    .iter()
                    .map(|(id, path)| serde_json::json!({ "id": id, "path": path }))
                    .collect();

                serde_json::json!({
                    "prompt_path": rel_path,
                    "summary": compact,
                    "sections": sections,
                    "project_prompts": pp_json,
                    "synthesis_prompt_path": synthesis_path,
                    "instructions": ws_instructions,
                })
            } else {
                serde_json::json!({
                    "prompt_path": rel_path,
                    "summary": compact,
                    "sections": sections,
                    "instructions": instructions,
                })
            };

            return Ok(vec![rmcp::model::Content::text(
                serde_json::to_string_pretty(&response)
                    .unwrap_or_else(|_| format!("{{\"prompt_path\":\"{rel_path}\"}}")),
            )]);
        }

        // Single-block fast path: already-onboarded status.
        if val["onboarded"].as_bool().unwrap_or(false) {
            let msg = val["message"].as_str().unwrap_or("Already onboarded.");
            return Ok(vec![rmcp::model::Content::text(msg.to_string())]);
        }

        // Fallback
        let compact = format_onboarding(&val);
        Ok(vec![rmcp::model::Content::text(compact)])
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_onboarding(result))
    }
}

async fn handle_refresh_prompt(ctx: &ToolContext) -> anyhow::Result<Value> {
    let status = ctx
        .agent
        .with_project(|p| {
            let has_config = p.root.join(".codescout").join("project.toml").exists();
            let memories = p.memory.list()?;
            let has_onboarding_memory = memories.iter().any(|m| m == "onboarding");
            Ok((has_config, has_onboarding_memory, memories))
        })
        .await?;
    let (has_config, has_onboarding_memory, memories) = status;
    if !has_config || !has_onboarding_memory {
        return Err(super::RecoverableError::with_hint(
            "refresh_prompt requires a fully onboarded project",
            "Run onboarding() without any flags first to perform the initial onboarding.",
        )
        .into());
    }

    let (stored_version, config_languages) = ctx
        .agent
        .with_project(|p| {
            Ok((
                p.config.project.onboarding_version,
                p.config.project.languages.clone(),
            ))
        })
        .await?;

    let config_path = ctx
        .agent
        .with_project(|p| {
            let config_path = p.root.join(".codescout").join("project.toml");
            if config_path.exists() {
                let mut config = crate::config::project::ProjectConfig::load_or_default(&p.root)?;
                config.project.onboarding_version = Some(ONBOARDING_VERSION);
                let toml_str = toml::to_string_pretty(&config)?;
                std::fs::write(&config_path, &toml_str)?;
            }
            Ok(config_path)
        })
        .await?;
    ctx.agent.reload_config_if_project_toml(&config_path).await;

    let subagent_prompt = build_prompt_refresh_subagent_prompt(&memories);

    Ok(json!({
        "onboarded": true,
        "version_stale": false,
        "explicit_refresh": true,
        "stored_version": stored_version,
        "current_version": ONBOARDING_VERSION,
        "languages": config_languages,
        "config_created": false,
        "subagent_prompt": subagent_prompt,
    }))
}

/// Returns `Some(response)` if the project is already onboarded (caller should return it),
/// or `None` if onboarding hasn't been done yet (caller should proceed with full scan).
async fn handle_already_onboarded(ctx: &ToolContext) -> anyhow::Result<Option<Value>> {
    let status = ctx
        .agent
        .with_project(|p| {
            let has_config = p.root.join(".codescout").join("project.toml").exists();
            let memories = p.memory.list()?;
            let has_onboarding_memory = memories.iter().any(|m| m == "onboarding");
            let private_memories = p.private_memory.list()?;
            Ok((
                has_config,
                has_onboarding_memory,
                memories,
                private_memories,
            ))
        })
        .await?;
    let (has_config, has_onboarding_memory, memories, private_memories) = status;
    if !has_config || !has_onboarding_memory {
        return Ok(None);
    }

    // --- Version check: refresh system prompt if stale ---
    let (stored_version, config_languages) = ctx
        .agent
        .with_project(|p| {
            Ok((
                p.config.project.onboarding_version,
                p.config.project.languages.clone(),
            ))
        })
        .await?;

    // Log downgrade (no action)
    if let Some(v) = stored_version {
        if v > ONBOARDING_VERSION {
            tracing::warn!(
                "stored onboarding version ({}) is newer than compiled ({}) — skipping refresh",
                v,
                ONBOARDING_VERSION
            );
        }
    }

    if onboarding_version_stale(stored_version) {
        tracing::info!(
            "onboarding version stale: stored={:?} current={}",
            stored_version,
            ONBOARDING_VERSION
        );

        // Optimistic version write to disk (prevents re-trigger across sessions)
        let config_path = ctx
            .agent
            .with_project(|p| {
                let config_path = p.root.join(".codescout").join("project.toml");
                if config_path.exists() {
                    let mut config =
                        crate::config::project::ProjectConfig::load_or_default(&p.root)?;
                    config.project.onboarding_version = Some(ONBOARDING_VERSION);
                    let toml_str = toml::to_string_pretty(&config)?;
                    std::fs::write(&config_path, &toml_str)?;
                }
                Ok(config_path)
            })
            .await?;
        // Reload in-memory config so subsequent calls in the same session
        // see the updated version (prevents re-trigger within session)
        ctx.agent.reload_config_if_project_toml(&config_path).await;

        let subagent_prompt = build_prompt_refresh_subagent_prompt(&memories);

        return Ok(Some(json!({
            "onboarded": true,
            "version_stale": true,
            "stored_version": stored_version,
            "current_version": ONBOARDING_VERSION,
            "languages": config_languages,
            "config_created": false,
            "subagent_prompt": subagent_prompt,
        })));
    }

    let per_project_memories = ctx.agent.workspace_project_memories().await;

    let mut message = format!(
        "Onboarding already performed. Available shared memories: {}. \
         Use `memory(action=\"read\", topic=...)` to read relevant ones as needed for your current task. \
         Do not read all memories at once — only read those relevant to what you're working on. \
         Use `memory(action=\"recall\", query=\"...\")` to search memories by meaning when the topic name isn't known.",
        memories.join(", ")
    );
    if !private_memories.is_empty() {
        message.push_str(&format!(
            " Private memories: {}. Read with `memory(action=\"read\", topic=..., private=true)`.",
            private_memories.join(", ")
        ));
    }
    if !per_project_memories.is_empty() {
        message.push_str(" Per-project memories (use `project: \"<id>\"` parameter):");
        for (id, topics) in &per_project_memories {
            message.push_str(&format!(" {}: {};", id, topics.join(", ")));
        }
    }
    let mut response = json!({
        "onboarded": true,
        "has_config": true,
        "has_onboarding_memory": true,
        "memories": memories,
        "message": message,
    });
    if !private_memories.is_empty() {
        response["private_memories"] = json!(private_memories);
    }
    if !per_project_memories.is_empty() {
        let map: serde_json::Map<String, serde_json::Value> = per_project_memories
            .into_iter()
            .map(|(id, topics)| (id, json!(topics)))
            .collect();
        response["project_memories"] = serde_json::Value::Object(map);
    }
    Ok(Some(response))
}

async fn perform_full_onboarding(
    root: std::path::PathBuf,
    ctx: &ToolContext,
) -> anyhow::Result<Value> {
    // Hardware detection runs after the file walk (Rust futures are lazy — this
    // just creates the future; it starts executing only when .await'd below).
    let hw_future = detect_hardware_context();

    // Detect languages by walking files
    let mut languages = std::collections::BTreeSet::new();
    let walker = ignore::WalkBuilder::new(&root)
        .hidden(true)
        .git_ignore(true)
        .build();
    for entry in walker.flatten() {
        if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            if let Some(lang) = crate::ast::detect_language(entry.path()) {
                languages.insert(lang.to_string());
            }
        }
    }

    // List top-level entries
    let mut top_level = vec![];
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let suffix = if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                "/"
            } else {
                ""
            };
            top_level.push(format!("{}{}", name, suffix));
        }
    }
    top_level.sort();

    // Resolve hardware detection and derive model options
    let hw = hw_future.await;
    let model_options = model_options_for_hardware(&hw);
    let recommended_model = model_options
        .first()
        .expect("model_options_for_hardware guarantees ≥1 entry")
        .id
        .clone();

    // Create .codescout/project.toml if it doesn't exist
    let config_dir = root.join(".codescout");
    let config_path = config_dir.join("project.toml");
    let created_config = if !config_path.exists() {
        std::fs::create_dir_all(&config_dir)?;
        let name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed")
            .to_string();
        let langs: Vec<String> = languages.iter().cloned().collect();
        let config = crate::config::project::ProjectConfig {
            project: crate::config::project::ProjectSection {
                name,
                languages: langs,
                encoding: "utf-8".into(),
                system_prompt: None,
                tool_timeout_secs: 60,
                onboarding_version: Some(ONBOARDING_VERSION),
            },
            embeddings: crate::config::project::EmbeddingsSection {
                model: recommended_model,
                ..Default::default()
            },
            ignored_paths: Default::default(),
            security: Default::default(),
            memory: Default::default(),
            libraries: Default::default(),
            lsp: Default::default(),
        };
        let toml_str = toml::to_string_pretty(&config)?;
        std::fs::write(&config_path, &toml_str)?;
        // Reload in-memory config so the version is visible within this session
        ctx.agent.reload_config_if_project_toml(&config_path).await;
        true
    } else {
        false
    };

    // Gather rich context from well-known project files.
    // Pass the already-discovered project list from the workspace to avoid a
    // redundant discover_projects walk (the agent runs it at activation time).
    let discovered = ctx.agent.discovered_projects().await;
    let gathered = gather_project_context(&root, discovered);

    // Create workspace.toml for multi-project repos
    let workspace_config_path = crate::config::workspace::workspace_config_path(&root);
    if gathered.projects.len() > 1 && !workspace_config_path.exists() {
        let ws_config = crate::config::workspace::WorkspaceConfig {
            workspace: crate::config::workspace::WorkspaceSection {
                name: root
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unnamed")
                    .to_string(),
                discovery_max_depth: 3,
            },
            resources: Default::default(),
            exclude_projects: vec![],
            projects: gathered
                .projects
                .iter()
                .map(|p| {
                    let project_abs = root.join(&p.relative_root);
                    let depends_on =
                        crate::workspace::infer_depends_on(&project_abs, &root, &gathered.projects);
                    crate::config::workspace::ProjectEntry {
                        id: p.id.clone(),
                        root: p.relative_root.to_string_lossy().to_string(),
                        languages: p.languages.clone(),
                        depends_on,
                    }
                })
                .collect(),
        };
        let toml_str = toml::to_string_pretty(&ws_config)?;
        std::fs::write(&workspace_config_path, &toml_str)?;
    }

    // Probe embedding index status (only opens existing DB, no network)
    let index_status = {
        let db_path = crate::embed::index::project_db_path(&root);
        if db_path.exists() {
            match crate::embed::index::open_db(&root)
                .and_then(|conn| crate::embed::index::index_stats(&conn))
            {
                Ok(stats) => json!({
                    "ready": stats.chunk_count > 0,
                    "files": stats.file_count,
                    "chunks": stats.chunk_count,
                }),
                Err(_) => json!({ "ready": false, "files": 0, "chunks": 0 }),
            }
        } else {
            json!({ "ready": false, "files": 0, "chunks": 0 })
        }
    };

    // Store onboarding result in memory
    let lang_list: Vec<String> = languages.iter().cloned().collect();
    ctx.agent
        .with_project(|p| {
            let summary = format!(
                "Languages: {}\nHas README: {}\nHas CLAUDE.md: {}\nBuild file: {}\nEntry points: {}\nTest dirs: {}",
                lang_list.join(", "),
                gathered.readme_path.is_some(),
                gathered.claude_md_exists,
                gathered.build_file_name.as_deref().unwrap_or("none"),
                if gathered.entry_points.is_empty() {
                    "none".to_string()
                } else {
                    gathered.entry_points.join(", ")
                },
                if gathered.test_dirs.is_empty() {
                    "none".to_string()
                } else {
                    gathered.test_dirs.join(", ")
                },
            );
            p.memory.write("onboarding", &summary)?;

            // Write language-patterns memory (deterministic, from hardcoded content)
            if let Some(patterns) = build_language_patterns_memory(&lang_list) {
                p.memory.write("language-patterns", &patterns)?;
            }

            Ok(())
        })
        .await?;

    // Write programmatic memories for each sub-project in workspace mode.
    if gathered.projects.len() > 1 {
        for project in &gathered.projects {
            let mem_dir = if project.relative_root == std::path::Path::new(".") {
                root.join(".codescout").join("memories")
            } else {
                root.join(".codescout")
                    .join("projects")
                    .join(&project.id)
                    .join("memories")
            };
            if let Ok(store) = crate::memory::MemoryStore::from_dir(mem_dir) {
                let proj_summary = format!(
                    "Languages: {}\nRoot: {}\nManifest: {}",
                    project.languages.join(", "),
                    project.relative_root.display(),
                    project.manifest.as_deref().unwrap_or("none"),
                );
                let _ = store.write("onboarding", &proj_summary);
                if let Some(patterns) = build_language_patterns_memory(&project.languages) {
                    let _ = store.write("language-patterns", &patterns);
                }
            }
        }
    }

    // Gather protected memory state for the LLM merge flow
    let protected_memories = ctx
        .agent
        .with_project(|p| {
            let memories_dir = p.root.join(".codescout").join("memories");
            let protected = &p.config.memory.protected;
            Ok(gather_protected_memory_state(
                &p.memory,
                &memories_dir,
                &p.root,
                protected,
            ))
        })
        .await?;

    // Build the key-files manifest for the prompt (paths only, no content)
    let mut key_files: Vec<String> = Vec::new();
    if let Some(ref p) = gathered.readme_path {
        key_files.push(p.clone());
    }
    if gathered.claude_md_exists {
        key_files.push("CLAUDE.md".to_string());
    }
    if let Some(ref p) = gathered.build_file_name {
        key_files.push(p.clone());
    }

    // Build the onboarding instruction prompt
    let is_workspace = gathered.projects.len() > 1;
    let prompt = crate::prompts::build_onboarding_prompt(&crate::prompts::OnboardingContext {
        languages: &lang_list,
        top_level: &top_level,
        key_files: &key_files,
        ci_files: &gathered.ci_files,
        entry_points: &gathered.entry_points,
        test_dirs: &gathered.test_dirs,
        index_ready: index_status["ready"].as_bool().unwrap_or(false),
        index_files: index_status["files"].as_u64().unwrap_or(0) as usize,
        index_chunks: index_status["chunks"].as_u64().unwrap_or(0) as usize,
        projects: &gathered.projects,
        is_workspace,
    });

    // Build the system prompt draft scaffold
    let libraries: Vec<crate::library::registry::LibraryEntry> = ctx
        .agent
        .library_registry()
        .await
        .map(|r| r.all().to_vec())
        .unwrap_or_default();
    let system_prompt_draft = build_system_prompt_draft(
        &lang_list,
        &gathered.entry_points,
        Some(&root),
        Some(&gathered.projects),
        &libraries,
    );

    let discovered_projects: Vec<serde_json::Value> = gathered
        .projects
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "root": p.relative_root.to_string_lossy(),
                "languages": p.languages,
                "manifest": p.manifest,
            })
        })
        .collect();

    let features_suggestion = gathered.features_md.is_none().then_some(
        "No FEATURES.md found. Consider creating docs/FEATURES.md to document \
         implemented capabilities — helps agents understand what's already built \
         and avoid re-suggesting existing features.",
    );

    // Per-project protected memory state for workspace mode.
    let (workspace_mode, per_project_protected) = if gathered.projects.len() > 1 {
        let protected = ctx
            .agent
            .with_project(|p| Ok(p.config.memory.protected.clone()))
            .await
            .unwrap_or_default();
        let mut map = serde_json::Map::new();
        for project in &gathered.projects {
            let mem_dir = if project.relative_root == std::path::Path::new(".") {
                root.join(".codescout").join("memories")
            } else {
                root.join(".codescout")
                    .join("projects")
                    .join(&project.id)
                    .join("memories")
            };
            let project_root = root.join(&project.relative_root);
            if let Ok(store) = crate::memory::MemoryStore::from_dir(mem_dir.clone()) {
                let state =
                    gather_protected_memory_state(&store, &mem_dir, &project_root, &protected);
                map.insert(project.id.clone(), state);
            }
        }
        (true, Some(Value::Object(map)))
    } else {
        (false, None)
    };

    // Build the subagent prompt by concatenating preamble + onboarding prompt +
    // system prompt draft + gathered data + epilogue
    let subagent_prompt = {
        let mut sp = build_subagent_preamble();
        sp.push_str(&prompt);
        if !system_prompt_draft.is_empty() {
            sp.push_str("\n\n## System Prompt Draft\n\n");
            sp.push_str(&system_prompt_draft);
        }
        if let Some(suggestion) = features_suggestion {
            sp.push_str(&format!("\n\n> {suggestion}"));
        }
        // Append gathered data that the subagent needs
        sp.push_str("\n\n## Gathered Data\n\n");
        sp.push_str(&format!(
            "**Hardware:** {}\n\n",
            serde_json::to_string_pretty(&hw).unwrap_or_default()
        ));
        sp.push_str(&format!(
            "**Model options:** {}\n\n",
            serde_json::to_string_pretty(&model_options).unwrap_or_default()
        ));
        if !protected_memories.is_null() {
            sp.push_str(&format!(
                "**Protected memories:** {}\n\n",
                serde_json::to_string_pretty(&protected_memories).unwrap_or_default()
            ));
        }
        if workspace_mode {
            if let Some(ref ppm) = per_project_protected {
                if !ppm.is_null() {
                    sp.push_str(&format!(
                        "**Per-project protected memories:** {}\n\n",
                        serde_json::to_string_pretty(ppm).unwrap_or_default()
                    ));
                }
            }
        }
        sp.push_str(&build_subagent_epilogue());
        sp
    };

    // Optimistic version write for full onboarding (force=true on existing project)
    ctx.agent
        .with_project(|p| {
            let config_path = p.root.join(".codescout").join("project.toml");
            if config_path.exists() {
                let mut config = crate::config::project::ProjectConfig::load_or_default(&p.root)?;
                config.project.onboarding_version = Some(ONBOARDING_VERSION);
                let toml_str = toml::to_string_pretty(&config)?;
                std::fs::write(&config_path, &toml_str)?;
            }
            Ok(())
        })
        .await?;

    Ok(json!({
        "languages": lang_list,
        "top_level": top_level,
        "config_created": created_config,
        "has_readme": gathered.readme_path.is_some(),
        "has_claude_md": gathered.claude_md_exists,
        "build_file": gathered.build_file_name,
        "entry_points": gathered.entry_points,
        "test_dirs": gathered.test_dirs,
        "ci_files": gathered.ci_files,
        "features_md": gathered.features_md,
        "index_status": index_status,
        "workspace_mode": workspace_mode,
        "projects": discovered_projects,
        "subagent_prompt": subagent_prompt,
    }))
}

/// Extract a u64 from a JSON value that may be a Number or a numeric String.
fn get_timeout_u64(v: &Value) -> Option<u64> {
    match v {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.parse::<u64>().ok(),
        _ => None,
    }
}

/// Parse the timeout from run_command input with leniency for:
/// - wrong key name (`timeout` instead of `timeout_secs`)
/// - millisecond values passed as `timeout_secs` (value > 86_400)
///
/// Returns `(resolved_seconds, optional_hint_for_agent)`.
fn parse_timeout_input(input: &Value) -> (u64, Option<String>) {
    // Canonical key: timeout_secs
    if let Some(v) = get_timeout_u64(&input["timeout_secs"]) {
        if v == 0 {
            return (
                30,
                Some("timeout_secs: 0 is invalid — using default of 30s.".to_string()),
            );
        }
        if v > 86_400 {
            let converted = v / 1_000;
            return (
                converted,
                Some(format!(
                    "timeout_secs: {v} looks like milliseconds — converted to {converted}s. \
                     Use timeout_secs with a value in seconds."
                )),
            );
        }
        return (v, None);
    }

    // Fallback: wrong key name `timeout`
    if let Some(v) = get_timeout_u64(&input["timeout"]) {
        if v == 0 {
            return (
                30,
                Some(
                    "Unknown parameter 'timeout' — use timeout_secs. \
                     Value 0 is invalid, using default of 30s."
                        .to_string(),
                ),
            );
        }
        if v >= 1_000 {
            let converted = v / 1_000;
            return (
                converted,
                Some(format!(
                    "Unknown parameter 'timeout' — use timeout_secs. \
                     Converted {v}ms → {converted}s."
                )),
            );
        }
        // v < 1000 → already seconds
        return (
            v,
            Some(format!(
                "Unknown parameter 'timeout' — use timeout_secs. \
                 Interpreted {v} as seconds."
            )),
        );
    }

    // Neither key present
    (30, None)
}

#[async_trait::async_trait]
impl Tool for RunCommand {
    fn name(&self) -> &str {
        "run_command"
    }
    fn description(&self) -> &str {
        "Run a shell command in the project root. Large output is buffered as @cmd_* refs."
    }

    fn long_docs(&self) -> Option<&str> {
        Some(
            "## Output buffering\n\
             \n\
             Short output (< 50 lines) is returned inline.\n\
             Long output is stored as `@cmd_xxxx` and a smart summary is returned.\n\
             Query the buffer in a follow-up: `run_command(\"grep FAILED @cmd_xxxx\")`.\n\
             Never pipe output inline — use the buffer ref instead.\n\
             \n\
             ## Key parameters\n\
             \n\
             - `command`: shell command string. May reference `@cmd_*` buffer refs.\n\
             - `cwd`: subdirectory relative to project root.\n\
             - `timeout_secs`: default 30; raise for long builds.\n\
             - `run_in_background=true`: detach and return immediately.\n\
             - `interactive=true`: spawn with stdin/stdout for REPLs.\n\
             - `acknowledge_risk=true`: bypass the dangerous-command gate (use the `@ack_*` \
             handle from the rejection response instead).\n\
             \n\
             ## Dangerous commands\n\
             \n\
             Commands matching destructive patterns (rm -rf, dd, mkfs, …) are blocked.\n\
             The rejection response contains an `@ack_*` handle — pass it as `acknowledge_risk` \
             to proceed after the user confirms.\n\
             \n\
             ## Tips\n\
             \n\
             - `cargo test` → buffer ref → `grep FAILED @cmd_xxx` to find failures.\n\
             - `cargo build` → buffer ref → `grep error @cmd_xxx` to find errors.\n\
             - Add trusted commands to `shell_allow_always` in `project.toml [security]`.",
        )
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["command"],
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command. May reference @cmd_* buffers (e.g. grep FAILED @cmd_abc)."
                },
                "timeout_secs": { "type": "integer", "default": 30, "description": "Max seconds (default 30)." },
                "cwd": { "type": "string", "description": "Subdirectory relative to project root." },
                "acknowledge_risk": { "type": "boolean", "description": "Bypass dangerous-command check. Prefer @ack_* handle from the rejected response." },
                "run_in_background": { "type": "boolean", "description": "Detach and return immediately. Use for long-running or backgrounded (&) commands." },
                "interactive": { "type": "boolean", "description": "Spawn process with interactive stdin/stdout. Elicits input after each output chunk. Use for REPLs, prompts, and interactive CLIs." }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        use super::output_buffer::OutputBuffer;

        let command = super::require_str_param(&input, "command")?;
        let (timeout_secs, timeout_hint) = parse_timeout_input(&input);
        let acknowledge_risk = parse_bool_param(&input["acknowledge_risk"]);
        let run_in_background = parse_bool_param(&input["run_in_background"]);
        let interactive = parse_bool_param(&input["interactive"]);
        let cwd_param = input["cwd"].as_str();
        let root = ctx.agent.require_project_root().await?;
        let security = ctx.agent.security_config().await;

        // --- Interactive mode: elicitation-driven stdin loop ---
        if interactive {
            return run_command_interactive(
                command,
                cwd_param,
                timeout_secs,
                &root,
                &security,
                ctx,
            )
            .await;
        }

        // --- Early dispatch: @ack_* handle ---
        if looks_like_ack_handle(command) {
            let stored = ctx.output_buffer.get_dangerous(command).ok_or_else(|| {
                super::RecoverableError::with_hint(
                    "ack handle expired or unknown",
                    "Re-run the original command to get a fresh handle.",
                )
            })?;
            return run_command_inner(
                &stored.command,
                &stored.command,
                stored.timeout_secs,
                true, // acknowledge_risk
                stored.cwd.as_deref(),
                false, // buffer_only
                false, // run_in_background — ack re-dispatch is always foreground
                &root,
                &security,
                ctx,
            )
            .await;
        }

        // --- Step 1: Resolve @cmd_ buffer references ---
        let (resolved_command, temp_files, buffer_only, refreshed_handles) =
            ctx.output_buffer.resolve_refs(command)?;

        // Helper: run inner logic then always clean up temp files.
        let mut result = run_command_inner(
            command,
            &resolved_command,
            timeout_secs,
            acknowledge_risk,
            cwd_param,
            buffer_only,
            run_in_background,
            &root,
            &security,
            ctx,
        )
        .await;

        OutputBuffer::cleanup_temp_files(&temp_files);

        // Inject refresh indicator into stdout when any @file_* handle was auto-refreshed.
        if !refreshed_handles.is_empty() {
            if let Ok(ref mut val) = result {
                let prefix: String = refreshed_handles
                    .iter()
                    .map(|id| {
                        format!(
                            "↻ {} refreshed from disk (file changed since last read)\n",
                            id
                        )
                    })
                    .collect();
                // Note: silently skips injection if "stdout" is absent (e.g. pending_ack
                // shape or buffered-output summary). These cases are extremely unlikely
                // to co-occur with a @file_* refresh, but worth noting.
                if let Some(stdout) = val["stdout"].as_str() {
                    val["stdout"] = serde_json::json!(format!("{}{}", prefix, stdout));
                }
            }
        }

        // Attach timeout hint when the timeout parameter was auto-corrected.
        if let Some(ref hint) = timeout_hint {
            if let Ok(ref mut val) = result {
                val["timeout_hint"] = json!(hint);
            }
        }

        result
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_run_command(result))
    }
}

fn format_onboarding(result: &Value) -> String {
    let langs = result["languages"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "?".to_string());
    let created = result["config_created"].as_bool().unwrap_or(false);
    let config_note = if created { " · config created" } else { "" };
    let workspace_note = if result["workspace_mode"].as_bool().unwrap_or(false) {
        let count = result["projects"].as_array().map(|a| a.len()).unwrap_or(0);
        format!(" · workspace ({count} projects)")
    } else {
        String::new()
    };
    format!("[{langs}]{config_note}{workspace_note}")
}

fn format_run_command(result: &Value) -> String {
    let mut s = if result["output_id"].is_string() {
        let exit = result["exit_code"].as_i64().unwrap_or(0);
        let check = if exit == 0 { "✓" } else { "✗" };
        let output_id = result["output_id"].as_str().unwrap_or("");
        match result["type"].as_str() {
            Some("test") => {
                let passed = result["passed"].as_u64().unwrap_or(0);
                let failed = result["failed"].as_u64().unwrap_or(0);
                let ignored = result["ignored"].as_u64().unwrap_or(0);
                let mut s = format!("{check} exit {exit} · {passed} passed");
                if failed > 0 {
                    s.push_str(&format!(" · {failed} FAILED"));
                }
                if ignored > 0 {
                    s.push_str(&format!(" · {ignored} ignored"));
                }
                s.push_str(&format!("  (query {output_id})"));
                s
            }
            Some("build") => {
                let errors = result["errors"].as_u64().unwrap_or(0);
                if errors > 0 {
                    format!("{check} exit {exit} · {errors} errors  (query {output_id})")
                } else {
                    format!("{check} exit {exit}  (query {output_id})")
                }
            }
            _ => format!("{check} exit {exit}  (query {output_id})"),
        }
    } else if result["timed_out"].as_bool().unwrap_or(false) {
        "✗ timed out".to_string()
    } else {
        let exit = result["exit_code"].as_i64().unwrap_or(0);
        let stdout_lines = result["stdout"]
            .as_str()
            .map(|s| s.lines().count())
            .unwrap_or(0);
        let check = if exit == 0 { "✓" } else { "✗" };
        format!("{check} exit {exit} · {stdout_lines} lines")
    };

    // Append timeout hint after all branch logic so it covers every output shape.
    if let Some(hint) = result["timeout_hint"].as_str() {
        s.push_str(&format!("\n⚠ timeout: {hint}"));
    }

    s
}

/// Returns true when `command` is a bare `@ack_<8hex>` handle.
fn looks_like_ack_handle(command: &str) -> bool {
    let s = command.trim();
    if !s.starts_with("@ack_") {
        return false;
    }
    let suffix = &s[5..]; // after "@ack_"
    suffix.len() == 8 && suffix.chars().all(|c| c.is_ascii_hexdigit())
}

/// Reassemble a buffered command summary with a stable, reader-friendly field order.
///
/// Dynamic field appending (`obj["key"] = val`) always places fields last, which
/// caused `output_id` (the buffer reference) to land after `stdout`/`failures`/
/// `first_error` (the bulk content). Correct order:
///   type → exit_code → output_id → [counts] → [content]
fn rebuild_buffered_summary(raw: Value, output_id: &str) -> Value {
    // These are large text fields — always go last.
    const CONTENT_FIELDS: &[&str] = &["stdout", "failures", "first_error"];

    let mut map = serde_json::Map::new();

    // 1. Status identity
    if let Some(v) = raw.get("type") {
        map.insert("type".into(), v.clone());
    }
    if let Some(v) = raw.get("exit_code") {
        map.insert("exit_code".into(), v.clone());
    }

    // 2. Buffer reference — most action-relevant, agent needs this to query results
    map.insert("output_id".into(), json!(output_id));

    // 3. Type-specific compact fields (counts, not content)
    let raw_obj = raw.as_object().expect("summary is always an object");
    for (k, v) in raw_obj {
        if !["type", "exit_code"].contains(&k.as_str()) && !CONTENT_FIELDS.contains(&k.as_str()) {
            map.insert(k.clone(), v.clone());
        }
    }

    // 4. Content fields last — bulk payload
    for field in CONTENT_FIELDS {
        if let Some(v) = raw_obj.get(*field) {
            map.insert((*field).into(), v.clone());
        }
    }

    Value::Object(map)
}

/// Interactive mode: spawn a process with piped stdin/stdout/stderr, then drive it
/// via MCP elicitation in a loop until the process exits or the user cancels.
///
/// Design notes (spike — E-3):
/// - Uses a 150 ms settle window to batch initial output before the first elicit.
/// - On each elicit round-trip we collect whatever is available (non-blocking drain),
///   show it to the user, and send their input back to the process.
/// - Empty input = user wants to cancel; we kill the process and return accumulated output.
/// - If elicitation is unavailable (no peer), returns a RecoverableError guiding the
///   caller to use the non-interactive path.
///
/// Latency concern (noted for spike evaluation):
///   Each stdin→stdout round-trip requires one MCP elicitation request+response, which
///   adds roughly the Claude Code UI round-trip latency (~1-3 s) per interaction step.
///   This is acceptable for slow interactive CLIs (setup wizards, REPLs with human
///   think-time) but unusable for high-frequency interactive programs.
async fn run_command_interactive(
    command: &str,
    cwd_param: Option<&str>,
    _timeout_secs: u64,
    root: &Path,
    security: &crate::util::path_security::PathSecurityConfig,
    ctx: &ToolContext,
) -> anyhow::Result<Value> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::process::Command;

    // Gate: elicitation must be available.
    if ctx.peer.is_none() {
        return Err(super::RecoverableError::with_hint(
            "interactive mode requires elicitation support",
            "The MCP client does not support elicitation. Use run_command without interactive: true.",
        )
        .into());
    }

    // Dangerous command check — block in interactive mode to keep the spike focused.
    if let Some(reason) = crate::util::path_security::is_dangerous_command(command, security) {
        return Err(super::RecoverableError::with_hint(
            format!("interactive mode blocked dangerous command: {reason}"),
            "Remove the dangerous pattern or use the non-interactive path with acknowledge_risk: true.",
        )
        .into());
    }

    // Resolve working directory.
    let work_dir = if let Some(rel) = cwd_param {
        let candidate = root.join(rel);
        candidate.canonicalize().map_err(|e| {
            super::RecoverableError::with_hint(
                format!("cwd '{rel}' is not a valid directory: {e}"),
                "Provide a relative path to an existing subdirectory of the project.",
            )
        })?
    } else {
        root.to_path_buf()
    };

    // Spawn with piped stdin/stdout/stderr.
    let (shell, shell_args) = crate::platform::shell_command(command);
    let mut child = Command::new(shell)
        .args(&shell_args)
        .current_dir(&work_dir)
        .env("GIT_PAGER", "cat")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().expect("stdin piped");
    let mut stdout_reader = tokio::io::BufReader::new(child.stdout.take().expect("stdout piped"));
    let mut stderr_reader = tokio::io::BufReader::new(child.stderr.take().expect("stderr piped"));

    let mut accumulated_output = String::new();

    // Drain available output from stdout+stderr using a settle window.
    // We use two separate buffers to avoid the double-borrow-of-mut-buf compiler error
    // when both futures reference the same buffer slice simultaneously.
    //
    // Loop structure: alternate trying stdout vs stderr within the settle timeout;
    // break out of the loop when both are silent for `settle_ms` ms (timeout fires).
    //
    // Note: this is an inner async fn — Rust supports these as non-capturing closures.
    // We cannot use a closure here because async closures that borrow mutable state across
    // await points are not yet stable (rust-lang/rust#62290).
    async fn drain_with_settle(
        stdout_reader: &mut tokio::io::BufReader<tokio::process::ChildStdout>,
        stderr_reader: &mut tokio::io::BufReader<tokio::process::ChildStderr>,
        settle_ms: u64,
    ) -> String {
        let settle = std::time::Duration::from_millis(settle_ms);
        let mut output = String::new();
        // Two independent buffers — one per reader — avoids the E0499 double-borrow.
        let mut out_buf = [0u8; 4096];
        let mut err_buf = [0u8; 4096];

        loop {
            tokio::select! {
                result = tokio::time::timeout(settle, stdout_reader.read(&mut out_buf)) => {
                    match result {
                        Ok(Ok(n)) if n > 0 => {
                            output.push_str(&String::from_utf8_lossy(&out_buf[..n]));
                        }
                        _ => break, // timeout or EOF
                    }
                }
                result = tokio::time::timeout(settle, stderr_reader.read(&mut err_buf)) => {
                    match result {
                        Ok(Ok(n)) if n > 0 => {
                            output.push_str(&String::from_utf8_lossy(&err_buf[..n]));
                        }
                        _ => break, // timeout or EOF
                    }
                }
            }
        }
        output
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
    struct InteractiveInput {
        /// Text to send to the process stdin (leave empty to cancel and kill the process)
        input: String,
    }
    rmcp::elicit_safe!(InteractiveInput);

    // Interaction loop.
    let mut round = 0u32;
    const MAX_ROUNDS: u32 = 50; // guard against runaway loops
    loop {
        if round >= MAX_ROUNDS {
            let _ = child.kill().await;
            accumulated_output.push_str("\n[interactive: max rounds reached, process killed]");
            break;
        }
        round += 1;

        // Read post-spawn / post-input output with 150 ms settle.
        let chunk = drain_with_settle(&mut stdout_reader, &mut stderr_reader, 150).await;
        if !chunk.is_empty() {
            accumulated_output.push_str(&chunk);
        }

        // Check whether the process already exited.
        match child.try_wait() {
            Ok(Some(status)) => {
                let code = status.code().unwrap_or(-1);
                // Drain any remaining output after exit.
                let tail = drain_with_settle(&mut stdout_reader, &mut stderr_reader, 50).await;
                if !tail.is_empty() {
                    accumulated_output.push_str(&tail);
                }
                return Ok(json!({
                    "exit_code": code,
                    "stdout": accumulated_output,
                    "interactive_rounds": round,
                }));
            }
            Ok(None) => {} // still running
            Err(e) => {
                accumulated_output.push_str(&format!("\n[interactive: wait error: {e}]"));
                break;
            }
        }

        // Elicit next input from the user.
        let display_output = if accumulated_output.len() > 4000 {
            // Show only the tail to keep the elicitation dialog readable.
            &accumulated_output[crate::tools::floor_char_boundary(
                &accumulated_output,
                accumulated_output.len() - 4000,
            )..]
        } else {
            &accumulated_output
        };
        let prompt = format!(
            "Process output (round {round}):\n```\n{display_output}\n```\n\nEnter input to send to stdin, or leave empty to cancel:"
        );

        let elicited = ctx.elicit::<InteractiveInput>(prompt).await?;

        match elicited {
            None => {
                // Elicitation unavailable mid-session (shouldn't happen — we checked at entry).
                let _ = child.kill().await;
                accumulated_output
                    .push_str("\n[interactive: elicitation unavailable, process killed]");
                break;
            }
            Some(InteractiveInput { input }) if input.is_empty() => {
                // User cancelled.
                let _ = child.kill().await;
                accumulated_output.push_str("\n[interactive: cancelled by user]");
                break;
            }
            Some(InteractiveInput { mut input }) => {
                // Send input to the process (append newline if missing).
                if !input.ends_with('\n') {
                    input.push('\n');
                }
                if let Err(e) = stdin.write_all(input.as_bytes()).await {
                    accumulated_output
                        .push_str(&format!("\n[interactive: stdin write error: {e}]"));
                    let _ = child.kill().await;
                    break;
                }
            }
        }
    }

    // Final drain after loop exit.
    let tail = drain_with_settle(&mut stdout_reader, &mut stderr_reader, 50).await;
    if !tail.is_empty() {
        accumulated_output.push_str(&tail);
    }

    Ok(json!({
        "exit_code": -1,
        "stdout": accumulated_output,
        "interactive_rounds": round,
        "note": "process killed or loop exited before natural termination",
    }))
}

/// Inner logic for `RunCommand::call`, extracted so temp-file cleanup
/// always happens in the caller regardless of early returns.
#[allow(clippy::too_many_arguments)]
async fn run_command_inner(
    original_command: &str,
    resolved_command: &str,
    timeout_secs: u64,
    acknowledge_risk: bool,
    cwd_param: Option<&str>,
    buffer_only: bool,
    run_in_background: bool,
    root: &Path,
    security: &crate::util::path_security::PathSecurityConfig,
    ctx: &ToolContext,
) -> anyhow::Result<Value> {
    use super::command_summary::{
        count_lines, detect_command_type, detect_terminal_filter, needs_summary, strip_ansi_codes,
        summarize_build_output, summarize_generic, summarize_test_output, truncate_lines,
        truncate_lines_and_bytes, CommandType, BUFFER_QUERY_INLINE_CAP,
    };
    use crate::util::path_security::is_dangerous_command;

    // --- Step 2: Dangerous command gate ---
    // Order: (a) acknowledge_risk bypass → (b) pending_ack two-round-trip fallback.
    if !buffer_only && !acknowledge_risk {
        // Use resolved_command (with @refs substituted) so buffer-only grep/awk
        // commands don't get flagged for patterns in the buffer content.
        if let Some(reason) = is_dangerous_command(resolved_command, security) {
            let handle = ctx.output_buffer.store_dangerous(
                resolved_command.to_string(),
                cwd_param.map(str::to_string),
                timeout_secs,
            );
            return Ok(serde_json::json!({
                "pending_ack": handle,
                "reason": reason,
                "hint": format!("run_command(\"{handle}\") to execute")
            }));
        }
    }

    // --- Step 2.5: Source file access block ---
    if !buffer_only && !acknowledge_risk {
        if let Some(hint) = crate::util::path_security::check_source_file_access(resolved_command) {
            return Err(super::RecoverableError::with_hint(
                "shell access to source files is blocked",
                &hint,
            )
            .into());
        }
    }

    // --- Step 3: Shell command mode check (skip for buffer-only queries) ---
    if !buffer_only {
        match security.shell_command_mode.as_str() {
            "disabled" => {
                return Err(super::RecoverableError::with_hint(
                    "shell commands are disabled",
                    "Set security.shell_command_mode = \"warn\" or \"unrestricted\" in .codescout/project.toml",
                ).into());
            }
            "unrestricted" | "warn" | "" => {} // allowed
            other => {
                return Err(super::RecoverableError::with_hint(
                    format!("unknown shell_command_mode: '{}'", other),
                    "Use \"warn\", \"unrestricted\", or \"disabled\".",
                )
                .into());
            }
        }
    }

    // --- Step 4: Resolve working directory ---
    let work_dir = if let Some(rel) = cwd_param {
        let candidate = root.join(rel);
        let canonical = candidate.canonicalize().map_err(|e| {
            super::RecoverableError::with_hint(
                format!("cwd '{}' is not a valid directory: {}", rel, e),
                "Provide a relative path to an existing subdirectory of the project.",
            )
        })?;
        let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let under_project = canonical.starts_with(canonical_root.as_path());
        let under_tmp = canonical.starts_with("/tmp");
        if !under_project && !under_tmp {
            return Err(super::RecoverableError::with_hint(
                format!("cwd '{}' escapes project root", rel),
                "The cwd must be a subdirectory within the project, or a path under /tmp.",
            )
            .into());
        }
        canonical
    } else {
        root.to_path_buf()
    };

    // --- Step 4.7: Background spawn with warm return ---
    if run_in_background {
        if buffer_only {
            return Err(super::RecoverableError::with_hint(
                "run_in_background cannot be used with buffer queries",
                "Remove run_in_background, or run the query as a plain command without @ref interpolation.",
            )
            .into());
        }

        let log_tmp = tempfile::Builder::new()
            .prefix("codescout-bg-")
            .suffix(".log")
            .tempfile()?;
        let log_path = log_tmp.path().to_path_buf();
        let (log_file, _) = log_tmp.keep()?;
        let log_stderr = log_file.try_clone()?;

        // Child handle dropped intentionally — process runs detached, adopted by init.
        let (shell, shell_args) = crate::platform::shell_command(resolved_command);
        tokio::process::Command::new(shell)
            .args(&shell_args)
            .current_dir(&work_dir)
            .env("GIT_PAGER", "cat")
            .stdout(std::process::Stdio::from(log_file))
            .stderr(std::process::Stdio::from(log_stderr))
            .spawn()?;

        // Warm return: 5s window captures startup output and fast failures.
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        let log_content = std::fs::read_to_string(&log_path).unwrap_or_default();
        let tail_50: String = {
            let lines: Vec<&str> = log_content.lines().collect();
            let start = lines.len().saturating_sub(50);
            lines[start..].join("\n")
        };

        let ref_id = ctx.output_buffer.store_background(log_path);

        let mut bg_result = serde_json::json!({
            "output_id": ref_id,
            "hint": format!(
                "Process running. Output captured in {} — use run_command(\"tail -50 {}\") or grep/cat as needed.",
                ref_id, ref_id
            )
        });
        if !tail_50.is_empty() {
            bg_result["stdout"] = json!(tail_50);
        }
        return Ok(bg_result);
    }

    // --- Step 4.5: Tee injection for terminal filter commands ---
    // When the last pipe stage is a known filter (grep, head, tail, sed, awk, etc.),
    // inject `tee /tmp/codescout-unfiltered-XXXX` before the filter so the caller
    // can surface the unfiltered stream as a buffer ref without re-running the command.

    // RAII guard: deletes the named tmpfile when dropped, ensuring cleanup on all
    // exit paths (success, error, and timeout arms of the match below).
    struct TmpfileGuard(String);
    impl Drop for TmpfileGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    let (effective_command, unfiltered_tmpfile): (String, Option<TmpfileGuard>) = if !buffer_only {
        if let Some(pipe_pos) = detect_terminal_filter(resolved_command) {
            // Use tempfile::NamedTempFile for unpredictable path (SF-3).
            // persist() converts it to a regular file we manage via TmpfileGuard.
            let named = tempfile::Builder::new()
                .prefix("codescout-unfiltered-")
                .tempfile()?;
            let tmppath = named.into_temp_path();
            let tmpfile = tmppath.to_string_lossy().to_string();
            // Keep the file on disk — TmpfileGuard handles cleanup.
            tmppath.keep()?;
            // Safety (SF-4): the path is generated by tempfile under $TMPDIR
            // and contains only alphanumeric chars, hyphens, and dots — no
            // shell metacharacters. We document this invariant rather than
            // adding a shell-escape dependency.
            if !tmpfile
                .chars()
                .all(|c| c.is_alphanumeric() || c == '/' || c == '-' || c == '_' || c == '.')
            {
                return Err(super::RecoverableError::new(format!(
                    "temporary file path contains unexpected characters: {}",
                    tmpfile,
                ))
                .into());
            }
            let cmd = format!(
                "{} | tee {} | {}",
                resolved_command[..pipe_pos].trim_end(),
                tmpfile,
                resolved_command[pipe_pos + 1..].trim_start()
            );
            (cmd, Some(TmpfileGuard(tmpfile)))
        } else {
            (resolved_command.to_string(), None)
        }
    } else {
        (resolved_command.to_string(), None)
    };

    // --- Step 5: Execute command ---
    // On Unix we spawn into a new process group (process_group(0) → PGID = child PID)
    // so killpg() can reap the entire tree on timeout.  Without this, dropping the tokio
    // future orphans curl/grep/tee/head and they keep running until the download finishes.
    //
    // `kill_on_drop(true)` is the cancellation lifeline: when the rmcp request is
    // cancelled (user pressed Escape), call_tool_inner drops the tool future, which
    // drops `child_output_fut`, which drops the `Child` — and tokio then SIGKILLs the
    // immediate child.  We *also* keep the timeout-path killpg() below for the case
    // where the future isn't dropped: SIGKILL on the lone shell wouldn't propagate to
    // the pipeline (curl, grep, tee, etc.), but killpg() reaps the whole group.
    //
    // We also reset SIGPIPE to SIG_DFL in pre_exec.  Claude Code's Node.js parent sets
    // SIGPIPE=SIG_IGN; every spawned process inherits it.  With SIG_IGN, a `| head -N`
    // pipeline never terminates via SIGPIPE: tee ignores the broken pipe from head and
    // keeps draining curl's output into the tmpfile until the download completes.
    #[cfg(unix)]
    let (child_output_fut, child_pgid) = {
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c")
            .arg(&effective_command)
            .current_dir(&work_dir)
            .env("GIT_PAGER", "cat")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .process_group(0) // new process group; PGID = child PID
            .kill_on_drop(true); // SIGKILL on Drop — reaps shell on cancel
                                 // SAFETY: pre_exec runs in the child after fork(), before exec().
                                 // signal() is async-signal-safe (POSIX).  No locks are held at this point.
        unsafe {
            cmd.pre_exec(|| {
                libc::signal(libc::SIGPIPE, libc::SIG_DFL);
                Ok(())
            });
        }
        let child = cmd.spawn()?;
        let pgid: Option<i32> = child.id().map(|id| id as i32);
        // Drop guard: if the future is cancelled, we want the *entire pipeline*
        // killed — not just the shell. tokio's kill_on_drop only SIGKILLs the
        // immediate child; killpg() walks the whole process group. We attach
        // the guard to the future so its Drop runs on cancellation.
        let pgid_for_guard = pgid;
        let fut: std::pin::Pin<
            Box<dyn std::future::Future<Output = std::io::Result<std::process::Output>> + Send>,
        > = Box::pin(async move {
            struct PgidKillGuard(Option<i32>);
            impl Drop for PgidKillGuard {
                fn drop(&mut self) {
                    if let Some(pgid) = self.0 {
                        // SAFETY: pgid was created with process_group(0); SIGKILL is
                        // safe to send to our own group. No-op if already reaped.
                        unsafe { libc::killpg(pgid, libc::SIGKILL) };
                    }
                }
            }
            let guard = PgidKillGuard(pgid_for_guard);
            let result = child.wait_with_output().await;
            // Successful completion: disarm the guard so we don't try to kill
            // an already-reaped pgid (harmless but pointless).
            std::mem::forget(guard);
            result
        });
        (fut, pgid)
    };

    #[cfg(windows)]
    let (child_output_fut, child_pgid) = {
        let fut: std::pin::Pin<
            Box<dyn std::future::Future<Output = std::io::Result<std::process::Output>> + Send>,
        > = Box::pin(
            tokio::process::Command::new("cmd")
                .arg("/C")
                .arg(&effective_command)
                .current_dir(&work_dir)
                .env("GIT_PAGER", "cat")
                .kill_on_drop(true)
                .output(),
        );
        (fut, None::<i32>)
    };

    // Heartbeat: send elapsed-seconds progress every 3s while the command runs.
    // AbortOnDrop guarantees the task is cancelled even when early `return`s fire.
    struct AbortOnDrop(tokio::task::JoinHandle<()>);
    impl Drop for AbortOnDrop {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let progress_clone = ctx.progress.clone();
    let _heartbeat = AbortOnDrop(tokio::spawn(async move {
        let start = std::time::Instant::now();
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            if let Some(p) = &progress_clone {
                p.report(start.elapsed().as_secs() as u32, None).await;
            }
        }
    }));

    match tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        child_output_fut,
    )
    .await
    {
        Ok(Ok(output)) => {
            let raw_stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let raw_stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let exit_code = output.status.code().unwrap_or(-1);

            // Buffer-only queries (e.g. `grep @cmd_A`, `cat @cmd_B`) display output
            // inline as JSON text. ANSI escape codes are opaque to LLMs and bloat byte
            // counts, causing byte-budget exhaustion that silently drops all content
            // (stdout_shown=0). Strip them here so line/byte caps operate on visible
            // text only. Direct command output is not stripped — callers that want
            // clean output can pipe through `sed 's/\x1b\[[0-9;]*m//g'`.
            let raw_stdout = if buffer_only {
                strip_ansi_codes(&raw_stdout)
            } else {
                raw_stdout
            };
            let raw_stderr = if buffer_only {
                strip_ansi_codes(&raw_stderr)
            } else {
                raw_stderr
            };

            // --- Step 6.5: Read tee capture and store as unfiltered_output ref ---
            let unfiltered_ref: Option<(String, bool)> =
                if let Some(ref tmpfile) = unfiltered_tmpfile {
                    let capture = std::fs::read_to_string(&tmpfile.0).ok();
                    // tmpfile dropped at end of enclosing match arm — TmpfileGuard::drop() removes it
                    // Skip empty captures: when the terminal filter (e.g. grep) matched nothing,
                    // both raw_stdout and the tee file are empty — surfacing a handle to an empty
                    // buffer is misleading and offers no value to the caller.
                    capture.and_then(|content| {
                        if content.is_empty() {
                            return None;
                        }
                        let (stored, truncated) = if crate::tools::exceeds_inline_limit(&content) {
                            let mut byte_budget = crate::tools::MAX_INLINE_TOKENS * 4;
                            let capped: String = content
                                .lines()
                                .take_while(|line| {
                                    if byte_budget == 0 {
                                        return false;
                                    }
                                    byte_budget = byte_budget.saturating_sub(line.len() + 1);
                                    true
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            (capped, true)
                        } else {
                            (content, false)
                        };
                        let ref_id = ctx.output_buffer.store(
                            original_command.to_string(),
                            stored,
                            String::new(), // unfiltered capture is stdout-only; stderr belongs to the main buffer
                            exit_code,
                        );
                        Some((ref_id, truncated))
                    })
                } else {
                    None
                };

            // --- Step 6: Decide whether to buffer + summarize ---
            let mut result = if needs_summary(&raw_stdout, &raw_stderr) {
                // When the command was querying a buffer ref (e.g. `sed @cmd_A`),
                // creating a *new* buffer causes an infinite loop: the agent sees
                // a fresh ref, queries it again, gets another ref, and so on.
                // Break the cycle by returning an error that guides the agent
                // toward a more targeted query instead.
                if buffer_only {
                    // Truncate to BUFFER_QUERY_INLINE_CAP lines total (stderr priority: up to 20,
                    // remainder goes to stdout) and return inline. Do NOT create a new buffer
                    // ref — that would cause an infinite query loop.
                    const STDERR_BUDGET: usize = 20;
                    // For buffer-only commands (e.g. `cat @cmd_A`), the shell command
                    // produces empty stderr. Augment with the original buffer entry's
                    // stored stderr so the agent gets the full picture on replay.
                    let buffer_stderr: String = if raw_stderr.is_empty() {
                        original_command
                            .find("@cmd_")
                            .or_else(|| original_command.find("@file_"))
                            .and_then(|pos| {
                                original_command[pos..]
                                    .split_whitespace()
                                    .next()
                                    .and_then(|tok| ctx.output_buffer.get(tok))
                            })
                            .map(|e| e.stderr)
                            .unwrap_or_default()
                    } else {
                        raw_stderr.clone()
                    };
                    let stderr_budget = STDERR_BUDGET.min(count_lines(&buffer_stderr));
                    let stdout_budget = BUFFER_QUERY_INLINE_CAP - stderr_budget;

                    // Compute stderr first so we know its byte size for the stdout budget.
                    let (stderr_out, stderr_shown, stderr_total) =
                        truncate_lines(&buffer_stderr, STDERR_BUDGET);

                    // Byte budget: ensure the final JSON stays under TOOL_OUTPUT_BUFFER_THRESHOLD
                    // so call_content() does not immediately re-buffer the result as @tool_*.
                    // That re-buffering creates an infinite query loop:
                    //   grep @cmd_A → inline JSON → >10KB → @tool_B → jq @tool_B → same → @tool_C…
                    // Overhead ≈ 300 bytes for JSON keys, stderr content, and truncation fields.
                    const JSON_OVERHEAD: usize = 300;
                    let stdout_byte_budget = crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD
                        .saturating_sub(JSON_OVERHEAD)
                        .saturating_sub(stderr_out.len());

                    let (stdout_out, stdout_shown, stdout_total) =
                        truncate_lines_and_bytes(&raw_stdout, stdout_budget, stdout_byte_budget);

                    let was_truncated = stdout_shown < stdout_total || stderr_shown < stderr_total;

                    let mut result = json!({"exit_code": exit_code});
                    if !stdout_out.is_empty() {
                        result["stdout"] = json!(stdout_out);
                    }
                    if !stderr_out.is_empty() {
                        result["stderr"] = json!(stderr_out);
                    }
                    if was_truncated {
                        result["truncated"] = json!(true);
                        result["stdout_shown"] = json!(stdout_shown);
                        result["stdout_total"] = json!(stdout_total);
                        if stderr_total > 0 {
                            result["stderr_shown"] = json!(stderr_shown);
                            result["stderr_total"] = json!(stderr_total);
                        }
                        let stderr_note = if stderr_total > 0 {
                            format!(", stderr {stderr_shown}/{stderr_total}")
                        } else {
                            String::new()
                        };
                        let next_start = stdout_shown + 1;
                        let next_end = stdout_shown + BUFFER_QUERY_INLINE_CAP;
                        result["hint"] = json!(format!(
                            "Output capped at {BUFFER_QUERY_INLINE_CAP} lines \
                             (stdout {stdout_shown}/{stdout_total}{stderr_note}). \
                             Next page: sed -n '{next_start},{next_end}p' @ref. \
                             Or grep 'keyword' @ref for targeted search.",
                        ));
                    }
                    // buffer_only => tee injection was skipped entirely (unfiltered_tmpfile is None),
                    // so no unfiltered_output field injection is needed before this early return.
                    return Ok(result);
                }

                let output_id = ctx.output_buffer.store(
                    original_command.to_string(),
                    raw_stdout.clone(),
                    raw_stderr.clone(),
                    exit_code,
                );

                let cmd_type = detect_command_type(original_command);
                let cmd_summary = match cmd_type {
                    CommandType::Test => summarize_test_output(&raw_stdout, &raw_stderr, exit_code),
                    CommandType::Build => {
                        summarize_build_output(&raw_stdout, &raw_stderr, exit_code)
                    }
                    CommandType::Generic => summarize_generic(&raw_stdout, &raw_stderr, exit_code),
                };

                // Rebuild with correct field order so output_id (the buffer reference
                // the agent needs) appears before content fields (stdout/failures/first_error).
                rebuild_buffered_summary(cmd_summary, &output_id)
            } else {
                // Short output — but for buffer-only queries, a single grep match
                // inside a compact-JSON @tool_* ref can be thousands of bytes even
                // with just 1 line.  That would push the result JSON over
                // TOOL_OUTPUT_BUFFER_THRESHOLD and cause call_content to store it
                // as a *new* @tool_* ref, creating an infinite query loop:
                //   grep @tool_A → giant line → @tool_B → read_file @tool_B → @tool_C…
                // Apply the same byte budget used in the needs_summary+buffer_only
                // path so that never happens.
                if buffer_only
                    && raw_stdout.len() + raw_stderr.len()
                        > crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD
                            .saturating_sub(300 /* JSON overhead */)
                {
                    const JSON_OVERHEAD: usize = 300;
                    let byte_budget = crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD
                        .saturating_sub(JSON_OVERHEAD)
                        .saturating_sub(raw_stderr.len());
                    let (stdout_out, stdout_shown, stdout_total) =
                        truncate_lines_and_bytes(&raw_stdout, BUFFER_QUERY_INLINE_CAP, byte_budget);
                    let mut r = json!({"exit_code": exit_code});
                    if !stdout_out.is_empty() {
                        r["stdout"] = json!(stdout_out);
                    }
                    if !raw_stderr.is_empty() {
                        r["stderr"] = json!(raw_stderr);
                    }
                    if stdout_shown < stdout_total {
                        r["truncated"] = json!(true);
                        r["hint"] = json!(
                            "Match truncated: a single grep match inside a @tool_* ref \
                             contains compact JSON (one very long line). \
                             Use read_file(@tool_abc, json_path=\"$.field\") to extract \
                             a specific field, or read_file(@tool_abc, start_line=N, \
                             end_line=M) to browse sections of the pretty-printed result."
                        );
                    }
                    r
                } else {
                    let mut r = json!({"exit_code": exit_code});
                    if !raw_stdout.is_empty() {
                        r["stdout"] = json!(raw_stdout);
                    }
                    if !raw_stderr.is_empty() {
                        r["stderr"] = json!(raw_stderr);
                    }
                    r
                }
            };

            // Attach unfiltered_output ref if we captured via tee
            if let Some((ref ref_id, truncated)) = unfiltered_ref {
                result["unfiltered_output"] = json!(ref_id);
                if truncated {
                    result["unfiltered_truncated"] = json!(true);
                }
            }

            Ok(result)
        }
        Ok(Err(e)) => {
            Err(super::RecoverableError::new(format!("command execution error: {}", e)).into())
        }
        Err(_) => {
            // Kill the entire process group so orphaned children (curl, grep, tee, etc.)
            // are reaped immediately rather than running to completion in the background.
            #[cfg(unix)]
            if let Some(pgid) = child_pgid {
                // SAFETY: pgid is the process group we created with process_group(0) above.
                // killpg with SIGKILL is the only reliable way to stop the whole pipeline
                // tree (sh + curl + grep + tee + head) in one shot.
                unsafe { libc::killpg(pgid, libc::SIGKILL) };
            }
            Ok(json!({
                "timed_out": true,
                "stderr": format!("Command timed out after {} seconds", timeout_secs),
                "exit_code": null,
                "hint": "If the command launches background processes (e.g. with &), use run_in_background: true — shell & leaves background processes holding the stdout pipe open, so output() never gets EOF. run_in_background spawns via a log file instead and returns immediately."
            }))
        }
    }
}

#[allow(dead_code)] // Kept as safety net for byte-level shell_output_limit_bytes config.
fn truncate_output(output: &str, limit: usize) -> (String, bool) {
    if output.len() > limit {
        let safe_end = crate::tools::floor_char_boundary(output, limit);
        (
            format!(
                "{}\n... (truncated, showing first {} of {} bytes)",
                &output[..safe_end],
                safe_end,
                output.len()
            ),
            true,
        )
    } else {
        (output.to_string(), false)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use crate::tools::command_summary::BUFFER_QUERY_INLINE_CAP;
    #[test]
    fn system_prompt_draft_includes_per_project_memory_refs() {
        use std::path::PathBuf;
        let projects = vec![
            crate::workspace::DiscoveredProject {
                id: "api".to_string(),
                relative_root: PathBuf::from("api"),
                languages: vec!["rust".to_string()],
                manifest: Some("Cargo.toml".to_string()),
            },
            crate::workspace::DiscoveredProject {
                id: "web".to_string(),
                relative_root: PathBuf::from("web"),
                languages: vec!["typescript".to_string()],
                manifest: Some("package.json".to_string()),
            },
        ];
        let draft = build_system_prompt_draft(
            &["rust".to_string(), "typescript".to_string()],
            &[],
            None,
            Some(&projects),
            &Vec::new(),
        );
        assert!(
            draft.contains("memory(project:"),
            "should reference per-project memories"
        );
        assert!(draft.contains("api"), "should mention api project");
        assert!(draft.contains("web"), "should mention web project");
    }

    #[test]
    fn subagent_preamble_contains_activate_project() {
        let preamble = build_subagent_preamble();
        assert!(
            preamble.contains("onboarding subagent"),
            "preamble must identify the subagent role"
        );
        assert!(
            preamble.contains("activate_project"),
            "preamble must instruct subagent to activate project"
        );
        assert!(
            preamble.contains("read_only: false"),
            "preamble must request write access"
        );
    }

    #[test]
    fn subagent_epilogue_contains_return_contract() {
        let epilogue = build_subagent_epilogue();
        assert!(
            epilogue.contains("Exploration Summary"),
            "epilogue must define exploration summary format"
        );
        assert!(
            epilogue.contains("Memories Written"),
            "epilogue must request memory list"
        );
        assert!(
            epilogue.contains("activate_project"),
            "epilogue must instruct subagent to restore project state"
        );
    }

    #[test]
    fn version_needs_refresh_when_none() {
        assert!(onboarding_version_stale(None));
    }

    #[test]
    fn version_needs_refresh_when_old() {
        assert!(onboarding_version_stale(Some(0)));
    }

    #[test]
    fn version_current_when_equal() {
        assert!(!onboarding_version_stale(Some(ONBOARDING_VERSION)));
    }

    #[test]
    fn version_current_when_newer_than_compiled() {
        assert!(!onboarding_version_stale(Some(ONBOARDING_VERSION + 1)));
    }

    #[test]
    fn prompt_refresh_subagent_prompt_contains_memory_reads() {
        let topics = vec!["architecture".to_string(), "conventions".to_string()];
        let prompt = build_prompt_refresh_subagent_prompt(&topics);
        assert!(prompt.contains("activate_project"));
        assert!(prompt.contains("architecture"));
        assert!(prompt.contains("conventions"));
        assert!(prompt.contains("system-prompt.md"));
        assert!(prompt.contains("Do NOT re-explore"));
    }

    #[test]
    fn is_subagent_capable_detects_claude() {
        assert!(is_subagent_capable(Some("claude-code")));
        assert!(is_subagent_capable(Some("Claude Code")));
        assert!(is_subagent_capable(Some("claude-code-ide")));
        assert!(!is_subagent_capable(Some("cursor")));
        assert!(!is_subagent_capable(Some("copilot")));
        assert!(!is_subagent_capable(Some("windsurf")));
        assert!(!is_subagent_capable(None));
    }

    #[test]
    fn build_heading_map_extracts_level2_headings() {
        let prompt = "# Title\n\nIntro text.\n\n## Phase 1: Explore\nStep 1.\nStep 2.\nMore.\n\n## Phase 2: Write\nA.\nB.\n\n## After\nFinal.\n";
        let sections = build_heading_map(prompt);
        assert_eq!(sections.len(), 3);
        assert!(sections[0].starts_with("1. ## Phase 1: Explore"));
        assert!(sections[0].contains("lines)"));
        assert!(sections[1].starts_with("2. ## Phase 2: Write"));
        assert!(sections[2].starts_with("3. ## After"));
    }

    #[test]
    fn build_buffered_onboarding_instructions_claude() {
        let instructions =
            build_buffered_onboarding_instructions(".codescout/tmp/onboarding-prompt.md", true);
        assert!(
            instructions.contains(".codescout/tmp/onboarding-prompt.md"),
            "must contain the prompt path"
        );
        assert!(
            instructions.contains("subagent"),
            "Claude instructions must mention subagent"
        );
        assert!(
            instructions.contains("read_markdown"),
            "must tell how to read via read_markdown"
        );
        // Must have numbered checklist
        assert!(
            instructions.contains("1. read_markdown"),
            "must have numbered phase checklist"
        );
        assert!(
            instructions.contains("## THE IRON LAW"),
            "checklist must start with THE IRON LAW"
        );
        assert!(
            instructions.contains("## Return Contract"),
            "checklist must end with Return Contract"
        );
    }

    #[test]
    fn build_buffered_onboarding_instructions_generic() {
        let instructions =
            build_buffered_onboarding_instructions(".codescout/tmp/onboarding-prompt.md", false);
        assert!(
            instructions.contains(".codescout/tmp/onboarding-prompt.md"),
            "must contain the prompt path"
        );
        assert!(
            !instructions.contains("subagent"),
            "generic instructions must NOT mention subagent"
        );
        assert!(
            instructions.contains("read_markdown"),
            "must tell how to read via read_markdown"
        );
        // Must have numbered checklist
        assert!(
            instructions.contains("1. read_markdown"),
            "must have numbered phase checklist"
        );
    }

    #[test]
    fn build_buffered_refresh_instructions_claude() {
        let instructions = build_buffered_refresh_instructions(
            ".codescout/tmp/onboarding-prompt.md",
            Some(1),
            2,
            true,
        );
        assert!(instructions.contains(".codescout/tmp/onboarding-prompt.md"));
        assert!(instructions.contains("v1"));
        assert!(instructions.contains("v2"));
        assert!(instructions.contains("subagent"));
        assert!(instructions.contains("read_markdown"));
        assert!(!instructions.contains("read_file"));
    }

    #[test]
    fn build_buffered_refresh_instructions_generic() {
        let instructions = build_buffered_refresh_instructions(
            ".codescout/tmp/onboarding-prompt.md",
            None,
            2,
            false,
        );
        assert!(instructions.contains(".codescout/tmp/onboarding-prompt.md"));
        assert!(instructions.contains("pre-versioning"));
        assert!(!instructions.contains("subagent"));
        assert!(instructions.contains("read_markdown"));
        assert!(!instructions.contains("read_file"));
    }

    #[test]
    fn build_per_project_prompt_contains_project_context() {
        let project = crate::workspace::DiscoveredProject {
            id: "backend".to_string(),
            relative_root: std::path::PathBuf::from("."),
            languages: vec!["kotlin".to_string(), "java".to_string()],
            manifest: Some("build.gradle.kts".to_string()),
        };
        let siblings = vec![
            ("mcp-server".to_string(), vec!["rust".to_string()]),
            ("python-svc".to_string(), vec!["python".to_string()]),
        ];
        let prompt = build_per_project_prompt(&project, &siblings);

        // Must contain project identity
        assert!(prompt.contains("backend"), "must contain project id");
        assert!(prompt.contains("kotlin"), "must contain languages");
        assert!(prompt.contains("build.gradle.kts"), "must contain manifest");

        // Must contain sibling info (for context, not deep-diving)
        assert!(prompt.contains("mcp-server"), "must mention siblings");
        assert!(
            prompt.contains("Do NOT deep-dive"),
            "must warn against sibling deep-dives"
        );

        // Must contain exploration steps
        assert!(
            prompt.contains("## Phase 2: Explore"),
            "must contain exploration phase"
        );
        assert!(
            prompt.contains("list_symbols"),
            "must contain exploration instructions"
        );

        // Must contain memory writing instructions
        assert!(
            prompt.contains("## Phase 3: Write"),
            "must contain memory phase"
        );
        assert!(
            prompt.contains("project=\"backend\""),
            "must scope memories to project"
        );

        // Must contain iron law
        assert!(prompt.contains("IRON LAW"), "must contain iron law");

        // Must contain return contract
        assert!(
            prompt.contains("## Return Contract"),
            "must contain return contract"
        );

        // Must NOT contain workspace synthesis instructions
        assert!(
            !prompt.contains("Workspace Memory Synthesis"),
            "must NOT contain workspace synthesis"
        );
    }

    #[test]
    fn build_synthesis_prompt_contains_readback_and_claude_md() {
        let projects = vec![
            ("backend".to_string(), vec!["kotlin".to_string()]),
            ("mcp-server".to_string(), vec!["rust".to_string()]),
        ];
        let prompt = build_synthesis_prompt(&projects);

        // Must contain memory readback commands for each project
        assert!(prompt.contains("memory(action=\"read\", project=\"backend\""));
        assert!(prompt.contains("memory(action=\"read\", project=\"mcp-server\""));

        // Must contain workspace memory topics
        assert!(prompt.contains("architecture"));
        assert!(prompt.contains("conventions"));
        assert!(prompt.contains("development-commands"));
        assert!(prompt.contains("domain-glossary"));
        assert!(prompt.contains("gotchas"));

        // Must contain CLAUDE.md refresh instructions
        assert!(
            prompt.contains("CLAUDE.md"),
            "must include CLAUDE.md refresh"
        );
        assert!(
            prompt.contains("preserve"),
            "must mention preserving user content"
        );

        // Must contain system prompt generation
        assert!(prompt.contains("system-prompt"));
    }

    #[test]
    fn build_workspace_instructions_claude_contains_parallel_dispatch() {
        let project_prompts = vec![
            (
                "backend".to_string(),
                ".codescout/tmp/onboarding-project-backend.md".to_string(),
            ),
            (
                "mcp".to_string(),
                ".codescout/tmp/onboarding-project-mcp.md".to_string(),
            ),
        ];
        let synthesis_path = ".codescout/tmp/onboarding-workspace-synthesis.md";
        let main_prompt_path = ".codescout/tmp/onboarding-prompt.md";
        let instructions =
            build_workspace_instructions(main_prompt_path, &project_prompts, synthesis_path, true);

        // Must mention parallel dispatch
        assert!(instructions.contains("parallel") || instructions.contains("PARALLEL"));
        // Must reference each project prompt
        assert!(instructions.contains("onboarding-project-backend.md"));
        assert!(instructions.contains("onboarding-project-mcp.md"));
        // Must reference synthesis prompt
        assert!(instructions.contains("onboarding-workspace-synthesis.md"));
        // Must reference Phase 0-1 from main prompt
        assert!(instructions.contains("Phase 0") || instructions.contains("Phase 1"));
        // Must mention subagent
        assert!(instructions.contains("subagent"));
    }

    #[test]
    fn build_workspace_instructions_generic_is_sequential() {
        let project_prompts = vec![(
            "backend".to_string(),
            ".codescout/tmp/onboarding-project-backend.md".to_string(),
        )];
        let synthesis_path = ".codescout/tmp/onboarding-workspace-synthesis.md";
        let main_prompt_path = ".codescout/tmp/onboarding-prompt.md";
        let instructions =
            build_workspace_instructions(main_prompt_path, &project_prompts, synthesis_path, false);

        assert!(!instructions.contains("subagent"));
        assert!(instructions.contains("onboarding-project-backend.md"));
        assert!(instructions.contains("read_markdown"));
    }

    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn lsp() -> Arc<dyn crate::lsp::LspProvider> {
        crate::lsp::LspManager::new_arc()
    }

    async fn project_ctx() -> (tempfile::TempDir, ToolContext) {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        // Create some source files for language detection
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.path().join("lib.py"), "def hello(): pass").unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        (
            dir,
            ToolContext {
                agent,
                lsp: lsp(),
                output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(
                    20,
                )),
                progress: None,
                peer: None,
                section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                    crate::tools::section_coverage::SectionCoverage::new(),
                )),
            },
        )
    }

    /// Like project_ctx() but uses the given directory as the project root.
    /// Caller is responsible for keeping the tempdir alive.
    async fn project_ctx_at(root: &std::path::Path) -> ToolContext {
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        std::fs::write(root.join("main.rs"), "fn main() {}").unwrap();
        let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
        ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        }
    }

    /// Create a two-project workspace layout in the given directory.
    /// Returns (api_dir, web_dir).
    fn setup_workspace_dirs(root: &std::path::Path) -> (PathBuf, PathBuf) {
        let api_dir = root.join("api");
        std::fs::create_dir_all(api_dir.join("src")).unwrap();
        std::fs::write(api_dir.join("Cargo.toml"), "[package]\nname = \"api\"").unwrap();
        std::fs::write(api_dir.join("src/main.rs"), "fn main() {}").unwrap();
        let web_dir = root.join("web");
        std::fs::create_dir_all(web_dir.join("src")).unwrap();
        std::fs::write(
            web_dir.join("package.json"),
            r#"{"name":"web","scripts":{"build":"tsc"}}"#,
        )
        .unwrap();
        std::fs::write(web_dir.join("src/index.ts"), "console.log('hello')").unwrap();
        (api_dir, web_dir)
    }

    #[tokio::test]
    async fn onboarding_detects_languages() {
        let (_dir, ctx) = project_ctx().await;
        let result = Onboarding.call(json!({}), &ctx).await.unwrap();
        let langs: Vec<&str> = result["languages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(langs.contains(&"rust"));
        assert!(langs.contains(&"python"));
    }

    #[tokio::test]
    async fn onboarding_creates_config() {
        let (dir, ctx) = project_ctx().await;
        // Remove the config if it exists
        let _ = std::fs::remove_file(dir.path().join(".codescout/project.toml"));

        let result = Onboarding.call(json!({}), &ctx).await.unwrap();
        assert_eq!(result["config_created"], true);
        assert!(dir.path().join(".codescout/project.toml").exists());
    }

    #[tokio::test]
    async fn onboarding_returns_status_when_already_done() {
        let (dir, ctx) = project_ctx().await;
        let _ = std::fs::remove_file(dir.path().join(".codescout/project.toml"));

        // First call does full onboarding
        let result = Onboarding.call(json!({}), &ctx).await.unwrap();
        assert!(result.get("languages").is_some()); // full onboarding result

        // Second call (no force) returns status instead
        let result = Onboarding.call(json!({}), &ctx).await.unwrap();
        assert_eq!(result["onboarded"], true);
        assert_eq!(result["has_config"], true);
        assert_eq!(result["has_onboarding_memory"], true);

        // Force re-scan
        let result = Onboarding
            .call(json!({ "force": true }), &ctx)
            .await
            .unwrap();
        assert!(result.get("languages").is_some()); // full onboarding again
    }
    #[tokio::test]
    async fn onboarding_returns_instruction_prompt() {
        let (_dir, ctx) = project_ctx().await;
        let result = Onboarding.call(json!({}), &ctx).await.unwrap();
        let prompt = result["subagent_prompt"].as_str().unwrap();
        assert!(prompt.contains("## Rules"));
        assert!(prompt.contains("## Memories to Create"));
        assert!(prompt.contains("rust")); // detected language
    }

    #[tokio::test]
    async fn onboarding_returns_subagent_prompt_and_instructions() {
        let (_dir, ctx) = project_ctx().await;
        let result = Onboarding.call(json!({}), &ctx).await.unwrap();

        // New fields must exist
        assert!(
            result.get("subagent_prompt").is_some(),
            "response must include subagent_prompt"
        );
        assert!(
            result["subagent_prompt"].is_string(),
            "subagent_prompt must be a string"
        );
        // Old fields must be gone
        assert!(
            result.get("instructions").is_none(),
            "instructions field must be removed"
        );
        assert!(
            result.get("system_prompt_draft").is_none(),
            "system_prompt_draft must be removed"
        );

        // subagent_prompt must contain preamble, body, and epilogue
        let prompt = result["subagent_prompt"].as_str().unwrap();
        assert!(
            prompt.contains("activate_project"),
            "subagent_prompt must contain preamble"
        );
        assert!(
            prompt.contains("## Return Contract"),
            "subagent_prompt must contain epilogue"
        );
        assert!(
            prompt.contains("Explore the Code") || prompt.contains("Memories to Create"),
            "subagent_prompt must contain onboarding prompt body"
        );
        assert!(
            prompt.contains("## System Prompt Draft"),
            "subagent_prompt must contain system prompt draft section"
        );

        // Lightweight metadata still present
        assert!(result.get("languages").is_some());
        assert!(result.get("config_created").is_some());
    }

    #[tokio::test]
    async fn onboarding_errors_without_project() {
        let ctx = ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };
        assert!(Onboarding.call(json!({}), &ctx).await.is_err());
    }

    #[tokio::test]
    async fn onboarding_status_includes_memories_and_message() {
        let (_dir, ctx) = project_ctx().await;

        // Run onboarding first
        Onboarding.call(json!({}), &ctx).await.unwrap();

        // Status call returns guidance message and memories
        let result = Onboarding.call(json!({}), &ctx).await.unwrap();
        let msg = result["message"].as_str().unwrap();
        assert!(msg.contains("already performed"));
        assert!(result["memories"].as_array().unwrap().len() > 0);
    }

    #[tokio::test]
    async fn onboarding_status_includes_private_memories_when_present() {
        let (_dir, ctx) = project_ctx().await;

        // Run full onboarding first (creates config + onboarding memory)
        Onboarding.call(json!({}), &ctx).await.unwrap();

        // Seed a private memory
        ctx.agent
            .with_project(|p| p.private_memory.write("my-prefs", "verbose"))
            .await
            .unwrap();

        // Fast-path status call should include private memories
        let result = Onboarding.call(json!({}), &ctx).await.unwrap();
        assert!(result["onboarded"].as_bool().unwrap_or(false));
        let private = result["private_memories"].as_array().unwrap();
        assert!(private.iter().any(|v| v.as_str() == Some("my-prefs")));
        assert!(result["message"].as_str().unwrap().contains("my-prefs"));
    }

    #[tokio::test]
    async fn onboarding_status_omits_private_memories_field_when_empty() {
        let (_dir, ctx) = project_ctx().await;

        // Run full onboarding first (creates config + onboarding memory), no private memory
        Onboarding.call(json!({}), &ctx).await.unwrap();

        // Fast-path status call should NOT include private_memories field
        let result = Onboarding.call(json!({}), &ctx).await.unwrap();
        assert!(result["onboarded"].as_bool().unwrap_or(false));
        assert!(result["private_memories"].is_null());
        assert!(!result["message"].as_str().unwrap().contains("private"));
    }

    #[tokio::test]
    async fn onboarding_call_content_delivers_message_when_already_done() {
        let (_dir, ctx) = project_ctx().await;

        // First call does full onboarding (creates config + writes memory)
        Onboarding.call(json!({}), &ctx).await.unwrap();

        // Second call (no force) — call_content must deliver the message, not "[?]"
        let content = Onboarding.call_content(json!({}), &ctx).await.unwrap();
        assert_eq!(content.len(), 1);
        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        assert!(
            text.contains("already performed"),
            "expected already-onboarded message, got: {text:?}"
        );
        assert!(
            text.contains("onboarding"),
            "expected memory list in message, got: {text:?}"
        );
        assert!(
            !text.contains("[?]"),
            "call_content must not emit [?] placeholder, got: {text:?}"
        );
    }

    #[tokio::test]
    async fn onboarding_call_content_writes_prompt_file() {
        let (_dir, ctx) = project_ctx().await;
        let content = Onboarding
            .call_content(json!({ "force": true }), &ctx)
            .await
            .unwrap();

        // Must return exactly 1 block
        assert_eq!(
            content.len(),
            1,
            "call_content must return 1 structured block, got {}",
            content.len()
        );

        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        let parsed: serde_json::Value =
            serde_json::from_str(text).expect("block must be valid JSON");

        // Must have prompt_path pointing at the markdown file
        let prompt_path = parsed["prompt_path"].as_str().unwrap_or("");
        assert!(
            prompt_path.contains("onboarding-prompt.md"),
            "response must contain prompt_path with onboarding-prompt.md, got: {}",
            &text[..text.len().min(200)]
        );

        // Must contain read_markdown instructions
        let instructions = parsed["instructions"].as_str().unwrap_or("");
        assert!(
            instructions.contains("read_markdown"),
            "response must contain read_markdown instructions"
        );
        assert!(
            !instructions.contains("read_file"),
            "response must NOT contain read_file instructions"
        );

        // Must NOT contain output_id (@tool_ ref)
        assert!(
            parsed.get("output_id").is_none(),
            "response must NOT have output_id"
        );

        // Must NOT contain raw prompt body content (heading names in sections[] are ok)
        assert!(
            !text.contains("REQUIRED_KEYS") && !text.contains("subagent_prompt"),
            "response must NOT contain raw prompt body content (should be in file)"
        );
    }

    #[tokio::test]
    async fn onboarding_call_content_writes_markdown_file() {
        let (_dir, ctx) = project_ctx().await;
        let content = Onboarding
            .call_content(json!({ "force": true }), &ctx)
            .await
            .unwrap();

        assert_eq!(content.len(), 1);
        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        let parsed: serde_json::Value = serde_json::from_str(text).expect("must be JSON");

        let prompt_path = parsed["prompt_path"]
            .as_str()
            .expect("must have prompt_path");
        assert!(prompt_path.contains("onboarding-prompt.md"));
        assert!(parsed.get("output_id").is_none(), "must NOT have output_id");

        let root = ctx.agent.project_root().await.unwrap();
        let full_path = root.join(prompt_path);
        assert!(full_path.exists());

        let sections = parsed["sections"].as_array().expect("must have sections");
        assert!(!sections.is_empty());

        let instructions = parsed["instructions"].as_str().unwrap_or("");
        assert!(instructions.contains("read_markdown"));
    }

    #[tokio::test]
    async fn onboarding_status_includes_per_project_memories_for_workspace() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        setup_workspace_dirs(root);
        let ctx = project_ctx_at(root).await;

        // Full workspace onboarding — writes per-project onboarding memories
        Onboarding.call(json!({}), &ctx).await.unwrap();

        // Second call hits the already-onboarded fast path
        let result = Onboarding.call(json!({}), &ctx).await.unwrap();

        assert!(result["onboarded"].as_bool().unwrap_or(false));

        // project_memories field is present and non-empty
        let pm = &result["project_memories"];
        assert!(
            pm.is_object(),
            "expected project_memories object, got: {pm}"
        );
        assert!(
            !pm.as_object().unwrap().is_empty(),
            "project_memories should be non-empty after workspace onboarding"
        );

        // Message mentions per-project memories and the project: param hint
        let msg = result["message"].as_str().unwrap();
        assert!(
            msg.contains("Per-project memories"),
            "message should mention per-project memories"
        );
        assert!(
            msg.contains("project:"),
            "message should include project scoping hint"
        );
    }

    #[tokio::test]
    async fn onboarding_call_content_force_delivers_instructions() {
        let (_dir, ctx) = project_ctx().await;

        // force=true must always deliver the full instructions, never "[?]"
        let content = Onboarding
            .call_content(json!({ "force": true }), &ctx)
            .await
            .unwrap();
        assert_eq!(
            content.len(),
            1,
            "call_content must return 1 structured block, got {}",
            content.len()
        );

        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        assert!(
            !text.contains("[?]"),
            "call_content must not emit [?] placeholder, got: {text:?}"
        );

        // Must be valid JSON with prompt_path and instructions
        let parsed: serde_json::Value =
            serde_json::from_str(text).expect("call_content block must be valid JSON");
        assert!(
            parsed["prompt_path"]
                .as_str()
                .is_some_and(|s| s.contains("onboarding-prompt.md")),
            "must have prompt_path pointing to onboarding-prompt.md, got: {:?}",
            parsed["prompt_path"]
        );
        let instructions = parsed["instructions"].as_str().unwrap_or("");
        assert!(
            instructions.contains("read_markdown") || instructions.contains("subagent"),
            "instructions must guide the agent, got: {instructions:?}"
        );
        assert!(
            !instructions.contains("read_file"),
            "instructions must NOT reference read_file, got: {instructions:?}"
        );
    }

    #[tokio::test]
    async fn onboarding_call_content_returns_two_blocks() {
        // Test name kept for history; new contract is 1 structured JSON block.
        let (_dir, ctx) = project_ctx().await;
        let content = Onboarding
            .call_content(json!({ "force": true }), &ctx)
            .await
            .unwrap();

        // Must return exactly 1 content block (file path)
        assert_eq!(
            content.len(),
            1,
            "call_content must return 1 structured block, got {}",
            content.len()
        );

        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        let parsed: serde_json::Value =
            serde_json::from_str(text).expect("block must be valid JSON");

        // prompt_path must point to the markdown file
        let prompt_path = parsed["prompt_path"].as_str().unwrap_or("");
        assert!(
            prompt_path.contains("onboarding-prompt.md"),
            "prompt_path must contain onboarding-prompt.md, got: {prompt_path:?}"
        );

        // sections must be present and non-empty
        let empty = vec![];
        let sections = parsed["sections"].as_array().unwrap_or(&empty);
        assert!(!sections.is_empty(), "sections must be non-empty");

        // instructions must not contain raw subagent prompt body (long prose),
        // but may reference heading names in the checklist.
        let instructions = parsed["instructions"].as_str().unwrap_or("");
        assert!(
            !instructions.contains("NO MEMORIES WRITTEN WITHOUT COMPLETING"),
            "instructions must NOT contain raw prompt body (should be in file)"
        );

        // instructions must reference read_markdown
        assert!(
            instructions.contains("read_markdown"),
            "instructions must reference read_markdown"
        );
    }

    // ---- Task 5 tests: refresh_prompt parameter ----

    /// Helper: build a fully onboarded project context (config + onboarding memory written).
    /// `project_ctx()` creates an empty project — we need to run full onboarding first so
    /// the fast-path checks (has_config && has_onboarding_memory) pass.
    async fn onboarded_project_ctx() -> (tempfile::TempDir, ToolContext) {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };
        // Run full onboarding to write config + onboarding memory
        Onboarding.call(json!({}), &ctx).await.unwrap();
        (dir, ctx)
    }

    #[tokio::test]
    async fn refresh_prompt_on_onboarded_project_returns_refresh_response() {
        let (_dir, ctx) = onboarded_project_ctx().await;

        // refresh_prompt=true must trigger the refresh path even when version is current
        let result = Onboarding
            .call(json!({ "refresh_prompt": true }), &ctx)
            .await
            .unwrap();

        assert_eq!(
            result["onboarded"].as_bool().unwrap_or(false),
            true,
            "onboarded must be true"
        );
        assert_eq!(
            result["explicit_refresh"].as_bool().unwrap_or(false),
            true,
            "explicit_refresh flag must be set"
        );
        assert!(
            result.get("subagent_prompt").is_some(),
            "must include subagent_prompt"
        );
        assert!(
            result["subagent_prompt"]
                .as_str()
                .unwrap()
                .contains("activate_project"),
            "subagent_prompt must contain activate_project"
        );
    }

    #[tokio::test]
    async fn refresh_prompt_on_unonboarded_project_returns_error() {
        // No config, no memories — project_ctx() gives us a bare project dir
        let (_dir, ctx) = project_ctx().await;

        let err = Onboarding
            .call(json!({ "refresh_prompt": true }), &ctx)
            .await
            .unwrap_err();

        let recoverable = err
            .downcast::<crate::tools::RecoverableError>()
            .expect("expected RecoverableError for refresh_prompt on unonboarded project");
        assert!(
            recoverable.message.contains("fully onboarded"),
            "error message must mention fully onboarded, got: {:?}",
            recoverable.message
        );
    }

    #[tokio::test]
    async fn force_takes_priority_over_refresh_prompt() {
        // force=true + refresh_prompt=true must do a full re-scan, not a lightweight refresh.
        // project_ctx() is fine: force=true bypasses the onboarding check entirely.
        let (_dir, ctx) = project_ctx().await;

        let result = Onboarding
            .call(json!({ "force": true, "refresh_prompt": true }), &ctx)
            .await
            .unwrap();

        // Full onboarding result must NOT have explicit_refresh
        assert!(
            result.get("explicit_refresh").is_none(),
            "explicit_refresh must not be set on force path"
        );
        // Full onboarding result has languages, subagent_prompt with "Explore the Code"
        let prompt = result["subagent_prompt"].as_str().unwrap_or("");
        assert!(
            prompt.contains("Explore the Code") || prompt.contains("Memories to Create"),
            "full onboarding subagent_prompt must contain onboarding body, got: {prompt:?}"
        );
    }

    // ---- Task 6 test: call_content routing for version refresh ----

    #[tokio::test]
    async fn onboarding_call_content_returns_two_blocks_for_version_refresh() {
        // Test name kept for history; new contract is 1 structured JSON block.
        let (_dir, ctx) = onboarded_project_ctx().await;

        // Manually write a stale (version=None) config to disk, then reload so the
        // agent's in-memory config reflects the stale state.
        let config_path = ctx
            .agent
            .with_project(|p| {
                let config_path = p.root.join(".codescout").join("project.toml");
                let mut config = crate::config::project::ProjectConfig::load_or_default(&p.root)?;
                config.project.onboarding_version = None;
                let toml_str = toml::to_string_pretty(&config)?;
                std::fs::write(&config_path, &toml_str)?;
                Ok(config_path)
            })
            .await
            .unwrap();
        ctx.agent.reload_config_if_project_toml(&config_path).await;

        let content = Onboarding.call_content(json!({}), &ctx).await.unwrap();

        assert_eq!(
            content.len(),
            1,
            "version refresh must return 1 structured block, got {}",
            content.len()
        );

        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        let parsed: serde_json::Value =
            serde_json::from_str(text).expect("block must be valid JSON");

        // Must have a prompt_path
        assert!(
            parsed["prompt_path"]
                .as_str()
                .is_some_and(|s| s.contains("onboarding-prompt.md")),
            "must have prompt_path, got: {:?}",
            parsed["prompt_path"]
        );

        // Must NOT have output_id
        assert!(parsed.get("output_id").is_none(), "must NOT have output_id");

        // instructions must contain version info
        let instructions = parsed["instructions"].as_str().unwrap_or("");
        assert!(
            instructions.contains("v2")
                || instructions.contains("outdated")
                || instructions.contains("refresh"),
            "instructions must contain version info, got: {instructions:?}"
        );

        // instructions must reference read_markdown
        assert!(
            instructions.contains("read_markdown"),
            "instructions must reference read_markdown, got: {instructions:?}"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn execute_shell_command_timeout_is_enforced() {
        let (_dir, ctx) = project_ctx().await;
        let result = RunCommand
            .call(json!({ "command": "sleep 10", "timeout_secs": 1 }), &ctx)
            .await
            .unwrap();
        assert_eq!(result["timed_out"], true, "command should have timed out");
        assert!(result["stderr"]
            .as_str()
            .unwrap()
            .contains("timed out after 1 seconds"));
        let hint = result["hint"].as_str().unwrap_or("");
        assert!(
            hint.contains("run_in_background"),
            "timeout hint should mention run_in_background, got: {hint}"
        );
    }

    #[tokio::test]
    async fn execute_shell_command_fast_command_succeeds() {
        let (_dir, ctx) = project_ctx().await;
        let result = RunCommand
            .call(json!({ "command": "echo hello", "timeout_secs": 5 }), &ctx)
            .await
            .unwrap();
        assert_eq!(result["timed_out"], serde_json::Value::Null);
        assert!(result["stdout"].as_str().unwrap().contains("hello"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn execute_shell_command_output_truncated() {
        let (_dir, ctx) = project_ctx().await;
        let result = RunCommand
            .call(
                json!({ "command": "seq 1 100000", "timeout_secs": 10 }),
                &ctx,
            )
            .await
            .unwrap();
        // Large output is buffered, not byte-truncated.
        assert!(
            result["output_id"].as_str().is_some(),
            "large output should be buffered with output_id"
        );
        assert!(result["hint"].is_null(), "hint field should be absent");
        assert!(
            result["total_stdout_lines"].is_null(),
            "total_stdout_lines should be absent"
        );
    }

    #[tokio::test]
    async fn execute_shell_command_small_output_not_truncated() {
        let (_dir, ctx) = project_ctx().await;
        let result = RunCommand
            .call(json!({ "command": "echo hello", "timeout_secs": 5 }), &ctx)
            .await
            .unwrap();
        // Short output: no output_id, direct stdout
        assert_eq!(result["output_id"], serde_json::Value::Null);
        assert!(result["stdout"].as_str().unwrap().contains("hello"));
    }

    #[tokio::test]
    async fn run_command_does_not_include_warning() {
        let (_dir, ctx) = project_ctx().await;
        let result = RunCommand
            .call(json!({ "command": "echo test", "timeout_secs": 5 }), &ctx)
            .await
            .unwrap();
        assert!(
            result["warning"].is_null(),
            "run_command should not emit a warning field"
        );
    }

    #[tokio::test]
    async fn execute_shell_command_exit_code_preserved() {
        let (_dir, ctx) = project_ctx().await;
        let result = RunCommand
            .call(json!({ "command": "exit 42", "timeout_secs": 5 }), &ctx)
            .await
            .unwrap();
        assert_eq!(result["exit_code"], 42);
    }

    #[tokio::test]
    async fn execute_shell_command_echo_cross_platform() {
        let (_dir, ctx) = project_ctx().await;
        // "echo hello" works on both sh and cmd.exe
        let result = RunCommand
            .call(json!({ "command": "echo hello", "timeout_secs": 5 }), &ctx)
            .await
            .unwrap();
        let stdout = result["stdout"].as_str().unwrap();
        assert!(
            stdout.contains("hello"),
            "stdout should contain 'hello': {}",
            stdout
        );
    }

    #[test]
    fn gather_context_reads_readme_and_build_file() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("README.md"),
            "# My Project\nA test project.",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"",
        )
        .unwrap();
        let ctx = gather_project_context(dir.path(), vec![]);
        assert_eq!(ctx.readme_path.as_deref(), Some("README.md"));
        assert_eq!(ctx.build_file_name.as_deref(), Some("Cargo.toml"));
        assert!(!ctx.claude_md_exists);
    }

    #[test]
    fn gather_context_finds_ci_files() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".github/workflows")).unwrap();
        std::fs::write(dir.path().join(".github/workflows/ci.yml"), "name: CI").unwrap();
        let ctx = gather_project_context(dir.path(), vec![]);
        assert_eq!(ctx.ci_files, vec![".github/workflows/ci.yml"]);
    }

    #[test]
    fn gather_context_finds_entry_points_and_test_dirs() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();
        std::fs::create_dir_all(dir.path().join("tests")).unwrap();
        let ctx = gather_project_context(dir.path(), vec![]);
        assert!(ctx.entry_points.contains(&"src/main.rs".to_string()));
        assert!(ctx.test_dirs.contains(&"tests".to_string()));
    }

    #[test]
    fn gather_context_handles_empty_project() {
        let dir = tempdir().unwrap();
        let ctx = gather_project_context(dir.path(), vec![]);
        assert!(ctx.readme_path.is_none());
        assert!(ctx.build_file_name.is_none());
        assert!(!ctx.claude_md_exists);
        assert!(ctx.ci_files.is_empty());
        assert!(ctx.entry_points.is_empty());
        assert!(ctx.test_dirs.is_empty());
    }

    #[tokio::test]
    async fn onboarding_returns_gathered_context_fields() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.path().join("README.md"), "# Test Project").unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        std::fs::create_dir_all(dir.path().join("tests")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };
        let result = Onboarding.call(json!({}), &ctx).await.unwrap();

        assert_eq!(result["has_readme"], true);
        assert_eq!(result["build_file"], "Cargo.toml");
        assert!(result["test_dirs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "tests"));
        // Verify the subagent_prompt is present
        assert!(result.get("subagent_prompt").is_some());
        // Verify the subagent_prompt references key files (paths, not embedded content)
        let prompt = result["subagent_prompt"].as_str().unwrap();
        assert!(prompt.contains("README.md"));
    }

    #[tokio::test]
    async fn onboarding_includes_system_prompt_draft_in_subagent_prompt() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("README.md"), "# Test Project\nA test.").unwrap();
        std::fs::write(dir.path().join("main.py"), "print('hello')").unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };
        let result = Onboarding.call(json!({}), &ctx).await.unwrap();

        // system_prompt_draft should NOT be a top-level field
        assert!(
            result.get("system_prompt_draft").is_none(),
            "system_prompt_draft must not be a top-level field"
        );
        // It should be embedded in subagent_prompt
        let prompt = result["subagent_prompt"].as_str().unwrap();
        assert!(
            prompt.contains("## System Prompt Draft"),
            "subagent_prompt should contain system prompt draft section"
        );
    }

    #[tokio::test]
    async fn onboarding_writes_language_patterns_memory() {
        let (_dir, ctx) = project_ctx().await;
        // project_ctx creates main.rs (rust) and lib.py (python)
        let _result = Onboarding.call(json!({}), &ctx).await.unwrap();

        // Verify the language-patterns memory was written
        let memory_content = ctx
            .agent
            .with_project(|p| p.memory.read("language-patterns"))
            .await
            .unwrap()
            .expect("language-patterns memory should exist");
        assert!(
            memory_content.contains("### Rust"),
            "should contain Rust patterns"
        );
        assert!(
            memory_content.contains("### Python"),
            "should contain Python patterns"
        );
        assert!(
            memory_content.contains("Anti-patterns"),
            "should contain anti-patterns section"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_command_dangerous_blocked_without_acknowledge() {
        let (_dir, ctx) = project_ctx().await;
        let result = RunCommand
            .call(
                json!({ "command": "rm -rf /tmp/codescout_test_nonexistent" }),
                &ctx,
            )
            .await
            .expect("dangerous command should return Ok with pending_ack");
        // Now returns a pending_ack handle instead of an error
        assert!(
            result.get("pending_ack").is_some(),
            "should have pending_ack key: {:?}",
            result
        );
        assert!(
            result["pending_ack"].as_str().unwrap().starts_with("@ack_"),
            "pending_ack should start with @ack_: {:?}",
            result["pending_ack"]
        );
        assert!(result.get("reason").is_some(), "should have reason key");
    }

    #[tokio::test]
    async fn run_command_dangerous_allowed_with_acknowledge() {
        let (_dir, ctx) = project_ctx().await;
        // Use a safe command but with acknowledge_risk: true — should succeed
        let result = RunCommand
            .call(
                json!({ "command": "echo safe", "acknowledge_risk": true }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(result["stdout"].as_str().unwrap().contains("safe"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_command_buffer_only_skips_safety() {
        let (_dir, ctx) = project_ctx().await;
        // Store some output in the buffer (must exceed token budget to trigger buffering)
        let result = RunCommand
            .call(json!({ "command": "seq 1 3000", "timeout_secs": 5 }), &ctx)
            .await
            .unwrap();
        let output_id = result["output_id"].as_str().unwrap();

        // grep on buffer ref only — should skip both dangerous-command check
        // and shell_command_mode check (buffer_only = true).
        let query = format!("grep '^5$' {}", output_id);
        let result2 = RunCommand
            .call(json!({ "command": query, "timeout_secs": 5 }), &ctx)
            .await
            .unwrap();
        // No warning should be present when buffer_only
        // (the default mode is "warn" which adds warning for non-buffer commands)
        assert_eq!(
            result2["warning"],
            serde_json::Value::Null,
            "buffer-only queries should not get shell warning"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_command_cwd_works() {
        let (dir, ctx) = project_ctx().await;
        // Create a subdirectory with a file
        let sub = dir.path().join("subdir");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("hello.txt"), "world").unwrap();

        let result = RunCommand
            .call(
                json!({ "command": "cat hello.txt", "cwd": "subdir", "timeout_secs": 5 }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(result["stdout"].as_str().unwrap().trim(), "world");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_command_cwd_rejects_traversal() {
        let (_dir, ctx) = project_ctx().await;
        let result = RunCommand
            .call(
                json!({ "command": "ls", "cwd": "../../etc", "timeout_secs": 5 }),
                &ctx,
            )
            .await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("escapes project root") || err_msg.contains("not a valid directory"),
            "should reject traversal: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn run_command_dangerous_rejected_without_ack() {
        let (_dir, ctx) = project_ctx().await;
        let result = RunCommand
            .call(json!({"command": "rm -rf /tmp/ce_nonexistent_test"}), &ctx)
            .await
            .expect("dangerous command should return Ok with pending_ack, not Err");
        // Previously returned Err(RecoverableError); now returns Ok with a pending_ack handle.
        assert!(
            result.get("pending_ack").is_some(),
            "should have pending_ack key: {:?}",
            result
        );
        assert!(
            result["pending_ack"].as_str().unwrap().starts_with("@ack_"),
            "pending_ack should start with @ack_: {:?}",
            result["pending_ack"]
        );
        assert!(
            result.get("reason").is_some(),
            "should have reason key: {:?}",
            result
        );
        assert!(
            result.get("hint").is_some(),
            "should have hint key: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn dangerous_command_returns_ack_handle() {
        let (dir, ctx) = project_ctx().await;
        let root = dir.path().to_path_buf();
        let security = Default::default();
        let result = run_command_inner(
            "rm -rf /dist",
            "rm -rf /dist",
            30,
            false, // acknowledge_risk
            None,  // cwd_param
            false, // buffer_only
            false, // run_in_background
            &root,
            &security,
            &ctx,
        )
        .await
        .expect("should return Ok with pending_ack, not Err");

        assert!(
            result.get("pending_ack").is_some(),
            "should have pending_ack key"
        );
        assert!(
            result["pending_ack"].as_str().unwrap().starts_with("@ack_"),
            "pending_ack should start with @ack_: {:?}",
            result["pending_ack"]
        );
        assert!(result.get("reason").is_some(), "should have reason key");
        assert!(result.get("hint").is_some(), "should have hint key");
    }

    #[tokio::test]
    async fn run_in_background_returns_bg_handle() {
        let (dir, ctx) = project_ctx().await;
        let root = dir.path().to_path_buf();
        let security = Default::default();

        let result = run_command_inner(
            "echo hello-bg-test",
            "echo hello-bg-test",
            30,
            false, // acknowledge_risk
            None,  // cwd_param
            false, // buffer_only
            true,  // run_in_background
            &root,
            &security,
            &ctx,
        )
        .await
        .expect("should succeed");

        let output_id = result["output_id"].as_str().expect("output_id missing");
        assert!(
            output_id.starts_with("@bg_"),
            "expected @bg_ prefix, got {output_id}"
        );
        let stdout = result["stdout"].as_str().unwrap_or("");
        assert!(
            stdout.contains("hello-bg-test"),
            "expected stdout to contain echo output, got: {stdout}"
        );
        let hint = result["hint"].as_str().unwrap_or("");
        assert!(
            hint.contains(output_id),
            "hint should reference the handle, got: {hint}"
        );
    }

    #[tokio::test]
    async fn run_in_background_rejects_buffer_only() {
        let (dir, ctx) = project_ctx().await;
        let root = dir.path().to_path_buf();
        let security = crate::util::path_security::PathSecurityConfig::default();
        let result = run_command_inner(
            "echo x", "echo x", 30, false, // acknowledge_risk
            None,  // cwd_param
            true,  // buffer_only
            true,  // run_in_background
            &root, &security, &ctx,
        )
        .await;
        let err = result.unwrap_err();
        assert!(
            err.downcast_ref::<crate::tools::RecoverableError>()
                .is_some(),
            "expected RecoverableError, got: {err}"
        );
        assert!(
            err.to_string().contains("buffer queries"),
            "error should mention buffer queries, got: {err}"
        );
    }

    /// A command that backgrounds a subprocess with `&` causes the foreground `output()` call
    /// to hang: the background process inherits the stdout pipe FD and keeps it open until it
    /// exits, preventing EOF.  With a short timeout this manifests as `timed_out: true`.
    /// The hint in the response should point the caller to `run_in_background: true`.
    #[cfg(unix)]
    #[tokio::test]
    async fn pipe_inheritance_from_shell_background_causes_timeout() {
        let (_dir, ctx) = project_ctx().await;
        // `sleep 60 &` — sh forks sleep (background), sleep inherits the stdout pipe,
        // sh exits but sleep keeps the pipe open for 60 s → output() can't get EOF.
        let result = RunCommand
            .call(json!({ "command": "sleep 60 &", "timeout_secs": 1 }), &ctx)
            .await
            .unwrap();
        assert_eq!(
            result["timed_out"], true,
            "background subprocess holding pipe should cause timeout"
        );
        let hint = result["hint"].as_str().unwrap_or("");
        assert!(
            hint.contains("run_in_background"),
            "hint should mention run_in_background, got: {hint}"
        );
    }

    /// `run_in_background: true` routes stdout to a log file, not a pipe, so background
    /// subprocesses holding the log FD open does not block the caller.  Even a command
    /// that would hang indefinitely in foreground mode returns promptly.
    #[cfg(unix)]
    #[tokio::test]
    async fn run_in_background_avoids_pipe_inheritance_hang() {
        let (_dir, ctx) = project_ctx().await;
        // Same pattern as the timeout test, but using run_in_background: true.
        // Should return a @bg_ handle without timing out.
        let result = RunCommand
            .call(
                json!({ "command": "echo launched && sleep 60 &", "run_in_background": true }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(
            result["timed_out"].is_null(),
            "run_in_background should not produce timed_out, got: {:?}",
            result["timed_out"]
        );
        let output_id = result["output_id"].as_str().expect("output_id missing");
        assert!(
            output_id.starts_with("@bg_"),
            "expected @bg_ handle, got: {output_id}"
        );
        // Warm-window stdout should contain the echo output.
        let stdout = result["stdout"].as_str().unwrap_or("");
        assert!(
            stdout.contains("launched"),
            "stdout should capture echo output within warm window, got: {stdout}"
        );
    }

    #[tokio::test]
    async fn run_command_safe_command_not_blocked() {
        let (_dir, ctx) = project_ctx().await;
        let result = RunCommand
            .call(json!({"command": "echo hello"}), &ctx)
            .await;
        assert!(result.is_ok(), "echo should not be blocked: {:?}", result);
    }

    #[tokio::test]
    async fn run_command_blocks_cat_on_source_file() {
        let (_dir, ctx) = project_ctx().await;
        let result = RunCommand
            .call(json!({"command": "cat src/main.rs"}), &ctx)
            .await;
        let err = result.unwrap_err();
        let rec = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .expect("should be a RecoverableError");
        assert!(
            rec.message.contains("source files is blocked"),
            "expected source-file block message, got: {}",
            rec.message
        );
    }

    #[tokio::test]
    async fn run_command_source_block_bypassed_with_acknowledge_risk() {
        let (dir, ctx) = project_ctx().await;
        std::fs::write(dir.path().join("tiny.rs"), "fn main() {}\n").unwrap();
        let result = RunCommand
            .call(
                json!({"command": "cat tiny.rs", "acknowledge_risk": true}),
                &ctx,
            )
            .await;
        assert!(
            result.is_ok(),
            "acknowledge_risk should bypass source block"
        );
    }

    #[tokio::test]
    async fn run_command_source_block_not_triggered_for_markdown() {
        let (dir, ctx) = project_ctx().await;
        std::fs::write(dir.path().join("README.md"), "# hello\n").unwrap();
        let result = RunCommand
            .call(json!({"command": "cat README.md"}), &ctx)
            .await;
        assert!(result.is_ok(), "cat on markdown should not be blocked");
    }

    #[tokio::test]
    async fn run_command_source_block_not_triggered_for_non_source() {
        let (dir, ctx) = project_ctx().await;
        std::fs::write(dir.path().join("data.txt"), "hello\n").unwrap();
        let result = RunCommand
            .call(json!({"command": "cat data.txt"}), &ctx)
            .await;
        assert!(result.is_ok(), "cat on .txt should not be blocked");
    }

    #[tokio::test]
    async fn run_command_cwd_rejects_nonexistent_path() {
        let (_dir, ctx) = project_ctx().await;
        let result = RunCommand
            .call(
                json!({"command": "ls", "cwd": "definitely_nonexistent_subdir_xyz"}),
                &ctx,
            )
            .await;
        assert!(result.is_err(), "nonexistent cwd should be rejected");
        let err = result.unwrap_err();
        let rec = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .expect("should be RecoverableError");
        assert!(
            rec.message.contains("not accessible") || rec.message.contains("not a valid"),
            "got: {}",
            rec.message
        );
    }

    #[tokio::test]
    async fn run_command_cwd_rejects_path_escaping_root() {
        let (_dir, ctx) = project_ctx().await;
        // Use /var — it always exists, is outside any temp project root, and is
        // not under /tmp (which is now an allowed cwd root).
        let result = RunCommand
            .call(json!({"command": "ls", "cwd": "/var"}), &ctx)
            .await;
        assert!(
            result.is_err(),
            "absolute cwd outside root should be rejected"
        );
        let err = result.unwrap_err();
        let rec = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .expect("should be RecoverableError");
        assert!(
            rec.message.contains("escapes project root"),
            "got: {}",
            rec.message
        );
    }

    #[tokio::test]
    async fn run_command_buffer_only_skips_speed_bump() {
        let (_dir, ctx) = project_ctx().await;
        // Store directly in buffer — no need to run a command that may or may not buffer
        // depending on the current buffering threshold.
        let id = ctx
            .output_buffer
            .store("test_cmd".into(), "rm -rf data\n".into(), "".into(), 0);
        // "rm" appears in the buffer content, but the query command is buffer-only.
        // It should NOT be rejected as dangerous.
        let result = RunCommand
            .call(json!({"command": format!("grep rm {}", id)}), &ctx)
            .await;
        // Should succeed (or fail with grep exit 1 "not found") — but NOT as a RecoverableError
        // about dangerous commands.
        match result {
            Ok(v) => {
                assert!(
                    v.get("error")
                        .map(|e| !e
                            .as_str()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains("dangerous"))
                        .unwrap_or(true),
                    "buffer-only grep should not be flagged as dangerous"
                );
            }
            Err(e) => {
                let rec = e.downcast_ref::<crate::tools::RecoverableError>();
                assert!(
                    rec.map(|r| !r.message.to_lowercase().contains("dangerous"))
                        .unwrap_or(false),
                    "buffer-only should not fail with dangerous error"
                );
            }
        }
    }

    #[test]
    fn run_command_schema_has_cwd_and_acknowledge_risk() {
        let schema = RunCommand.input_schema();

        let cwd = &schema["properties"]["cwd"];
        assert!(cwd.is_object(), "cwd should be a schema object");
        assert_eq!(cwd["type"], "string", "cwd type should be string");

        let ack = &schema["properties"]["acknowledge_risk"];
        assert!(
            ack.is_object(),
            "acknowledge_risk should be a schema object"
        );
        assert_eq!(
            ack["type"], "boolean",
            "acknowledge_risk type should be boolean"
        );

        let required = schema["required"].as_array().unwrap();
        assert!(
            required.iter().any(|v| v == "command"),
            "command must remain required"
        );
    }

    // Task 4 TDD regression tests — buffer-backed smart summaries + buffer ref execution
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn run_command_short_output_returned_directly() {
        let (_dir, ctx) = project_ctx().await;
        let result = RunCommand
            .call(json!({"command": "echo hello"}), &ctx)
            .await
            .unwrap();
        assert!(
            result.get("output_id").is_none(),
            "short output should not buffer: got output_id {:?}",
            result.get("output_id")
        );
        assert!(
            result["stdout"].as_str().unwrap().contains("hello"),
            "stdout should contain 'hello': {:?}",
            result["stdout"]
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_command_large_output_stored_in_buffer() {
        let (_dir, ctx) = project_ctx().await;
        // seq 3000 produces ~14KB, exceeding MAX_INLINE_TOKENS * 4 (~10KB)
        let result = RunCommand
            .call(json!({"command": "seq 1 3000"}), &ctx)
            .await
            .unwrap();
        let output_id = result["output_id"]
            .as_str()
            .expect("large output should have output_id");
        assert!(
            output_id.starts_with("@cmd_"),
            "output_id should start with @cmd_: {}",
            output_id
        );
        assert!(result["hint"].is_null(), "hint field should be absent");
        assert!(
            result["total_stdout_lines"].is_null(),
            "total_stdout_lines should be absent"
        );
        let entry = ctx.output_buffer.get(output_id).unwrap();
        assert!(
            entry.stdout.contains("50\n"),
            "buffered stdout should contain '50\\n'"
        );
        assert!(
            entry.stdout.contains("3000\n"),
            "buffered stdout should contain '3000\\n'"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_command_buffer_ref_executes_correctly() {
        let (_dir, ctx) = project_ctx().await;
        let r1 = RunCommand
            .call(json!({"command": "seq 1 3000"}), &ctx)
            .await
            .unwrap();
        let output_id = r1["output_id"].as_str().unwrap();
        let r2 = RunCommand
            .call(
                json!({"command": format!("grep '^50$' {}", output_id)}),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(r2["exit_code"], 0, "grep should find '50': {:?}", r2);
        assert_eq!(
            r2["stdout"].as_str().unwrap().trim(),
            "50",
            "stdout should be exactly '50'"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_command_buffer_only_above_threshold_truncates_inline() {
        // BUFFER_QUERY_INLINE_CAP + 1 lines — strictly above the inline cap.
        // Must return Ok with truncated content, NOT an error or a new buffer ref.
        // Each line is padded to ~120 bytes so total exceeds the token budget.
        let (_dir, ctx) = project_ctx().await;
        let content: String = (1..=BUFFER_QUERY_INLINE_CAP + 1)
            .map(|i| format!("{i:>120}\n"))
            .collect();
        let id = ctx.output_buffer.store("cmd".into(), content, "".into(), 0);
        let result = RunCommand
            .call(json!({ "command": format!("cat {}", id) }), &ctx)
            .await
            .expect("expected Ok with truncated inline output");
        assert_eq!(
            result["truncated"], true,
            "should be truncated: {:?}",
            result
        );
        let shown = result["stdout_shown"].as_u64().unwrap() as usize;
        assert!(
            shown > 0 && shown <= BUFFER_QUERY_INLINE_CAP,
            "stdout_shown should be >0 and <=inline cap, got {shown}: {:?}",
            result
        );
        assert_eq!(
            result["stdout_total"],
            BUFFER_QUERY_INLINE_CAP + 1,
            "stdout_total should be full count: {:?}",
            result
        );
        assert!(
            result.get("output_id").is_none(),
            "must not create a new buffer ref: {:?}",
            result
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_command_buffer_only_at_threshold_returns_inline() {
        // Content exactly at MAX_INLINE_TOKENS token budget — the check is `>` not `>=`,
        // so this must return content inline, not error.
        let (_dir, ctx) = project_ctx().await;
        // Build content that is exactly MAX_INLINE_TOKENS * 4 bytes (at the limit, not over)
        let target_bytes = crate::tools::MAX_INLINE_TOKENS * 4;
        let mut content = String::new();
        for i in 1.. {
            let line = format!("{i}\n");
            if content.len() + line.len() > target_bytes {
                break;
            }
            content.push_str(&line);
        }
        let id = ctx.output_buffer.store("cmd".into(), content, "".into(), 0);
        let result = RunCommand
            .call(json!({ "command": format!("cat {}", id) }), &ctx)
            .await
            .expect("expected inline output at threshold");
        assert!(
            result.get("stdout").is_some(),
            "expected stdout field: {:?}",
            result
        );
        assert!(
            result.get("output_id").is_none(),
            "should not be buffered: {:?}",
            result
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_command_buffer_only_large_single_line_does_not_rebuffer() {
        // Regression: grep on a @tool_* ref returns the entire compact-JSON blob as
        // one line.  Even when estimated tokens are low, the byte
        // size can exceed the inline token budget.  The result must be truncated
        // inline — never stored as a new @tool_* ref (which would create an infinite
        // query loop: grep @tool_A → @tool_B → grep @tool_B → @tool_C…).
        let (_dir, ctx) = project_ctx().await;

        // Create a @cmd_* buffer whose content is one very long line (>5 KB).
        let long_line = "x".repeat(crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD + 1000);
        let id = ctx
            .output_buffer
            .store("cmd".into(), long_line, "".into(), 0);

        // cat @cmd_* triggers buffer_only; the single-line stdout exceeds the byte budget.
        let result = RunCommand
            .call(json!({ "command": format!("cat {}", id) }), &ctx)
            .await
            .expect("should return truncated inline result, not error");

        // Must be inline (no output_id) and must be truncated with a hint.
        assert!(
            result.get("output_id").is_none(),
            "must not create new buffer ref: {:?}",
            result
        );
        // stdout may be absent when the single line exceeded the byte budget entirely
        // (stdout_shown=0, stdout_total=1) — truncated+hint communicate the situation.
        assert_eq!(
            result.get("truncated").and_then(|v| v.as_bool()),
            Some(true),
            "must be marked truncated: {:?}",
            result
        );
        let hint = result["hint"].as_str().unwrap_or("");
        assert!(
            !hint.is_empty(),
            "hint should guide to next page or read_file: {}",
            hint
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_command_buffer_only_large_output_no_new_ref() {
        // Regression: `sed @cmd_A` that reproduces a large buffer must
        // return truncated inline content, NOT a new @cmd_B reference.
        // Use 150 lines (> BUFFER_QUERY_INLINE_CAP=100) to trigger truncation.
        let (_dir, ctx) = project_ctx().await;

        let large_content: String = (1..=250).map(|i| format!("{i:>60}\n")).collect();
        let id = ctx
            .output_buffer
            .store("original_cmd".into(), large_content, "".into(), 0);

        let result = RunCommand
            .call(
                json!({ "command": format!("sed -n '1,250p' {}", id) }),
                &ctx,
            )
            .await
            .expect("expected Ok with truncated inline output");

        assert!(
            result.get("output_id").is_none(),
            "must not create a new buffer ref: {:?}",
            result
        );
        assert_eq!(
            result["truncated"], true,
            "should be truncated: {:?}",
            result
        );
        assert_eq!(
            result["stdout_total"], 250usize,
            "stdout_total: {:?}",
            result
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_command_buffer_only_long_lines_fit_under_threshold() {
        // Regression: buffer-only queries with long lines (e.g. Java/Kotlin log output
        // with timestamps and class names, ~200 chars/line) must produce a response JSON
        // that stays under TOOL_OUTPUT_BUFFER_THRESHOLD.  Before the fix, a 100-line cap
        // on 200-char lines produced ~20 KB of stdout, which call_content() re-buffered
        // as @tool_* — creating an infinite query loop:
        //   grep @cmd_A → inline JSON (>10KB) → @tool_B → jq @tool_B → same → @tool_C…
        let (_dir, ctx) = project_ctx().await;

        // 200-char lines: typical Java log output with timestamp + class + message.
        let long_line = "x".repeat(200);
        let content: String = (0..=BUFFER_QUERY_INLINE_CAP)
            .map(|_| format!("{long_line}\n"))
            .collect();
        let id = ctx.output_buffer.store("cmd".into(), content, "".into(), 0);

        let result = RunCommand
            .call(json!({ "command": format!("cat {}", id) }), &ctx)
            .await
            .expect("expected Ok");

        // Core assertion: the serialized JSON must fit under the re-buffering threshold.
        let json_size = serde_json::to_string(&result).unwrap().len();
        assert!(
            json_size <= crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD,
            "buffer_only response ({json_size} bytes) must not exceed TOOL_OUTPUT_BUFFER_THRESHOLD \
             ({} bytes) — would cause infinite @tool_* re-buffering loop",
            crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD,
        );

        // Must also avoid creating a new buffer ref.
        assert!(
            result.get("output_id").is_none(),
            "must not create a new buffer ref: {:?}",
            result
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_command_buffer_only_stderr_gets_priority() {
        // stderr = 25 lines (> 20 cap) + stdout = 250 lines (> remaining budget).
        // Expected: stderr_shown = 20, stdout_shown = 80 (BUFFER_QUERY_INLINE_CAP - 20).
        // Lines padded to ~60 bytes so total exceeds the token budget.
        let (_dir, ctx) = project_ctx().await;
        let stdout: String = (1..=250).map(|i| format!("out{i:>60}\n")).collect();
        let stderr: String = (1..=25).map(|i| format!("err{i:>60}\n")).collect();
        let id = ctx.output_buffer.store("cmd".into(), stdout, stderr, 0);
        let result = RunCommand
            .call(json!({ "command": format!("cat {}", id) }), &ctx)
            .await
            .expect("expected Ok");
        assert_eq!(
            result["stderr_shown"], 20usize,
            "stderr_shown: {:?}",
            result
        );
        assert_eq!(
            result["stderr_total"], 25usize,
            "stderr_total: {:?}",
            result
        );
        assert_eq!(
            result["stdout_shown"],
            BUFFER_QUERY_INLINE_CAP - 20,
            "stdout_shown: {:?}",
            result
        );
        assert_eq!(
            result["stdout_total"], 250usize,
            "stdout_total: {:?}",
            result
        );
        assert_eq!(result["truncated"], true);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_command_buffer_only_short_stderr_gives_budget_to_stdout() {
        // stderr = 10 lines (< 20 cap) + stdout = 250 lines (> remaining budget).
        // Expected: stderr_shown = 10, stdout_shown = 90 (BUFFER_QUERY_INLINE_CAP - 10).
        // Lines padded to ~60 bytes so total exceeds the token budget.
        let (_dir, ctx) = project_ctx().await;
        let stdout: String = (1..=250).map(|i| format!("out{i:>60}\n")).collect();
        let stderr: String = (1..=10).map(|i| format!("err{i:>60}\n")).collect();
        let id = ctx.output_buffer.store("cmd".into(), stdout, stderr, 0);
        let result = RunCommand
            .call(json!({ "command": format!("cat {}", id) }), &ctx)
            .await
            .expect("expected Ok");
        assert_eq!(
            result["stdout_shown"],
            BUFFER_QUERY_INLINE_CAP - 10,
            "stdout_shown: {:?}",
            result
        );
        assert_eq!(
            result["stdout_total"], 250usize,
            "stdout_total: {:?}",
            result
        );
        assert_eq!(result["truncated"], true);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_command_buffer_only_within_limit_no_truncation_fields() {
        // combined = 45 lines (< 50 threshold) — must NOT add truncated/shown/total fields.
        // needs_summary returns false, so we fall through to the short-output branch.
        let (_dir, ctx) = project_ctx().await;
        let stdout: String = (1..=30).map(|i| format!("out{i}\n")).collect();
        let stderr: String = (1..=15).map(|i| format!("err{i}\n")).collect();
        let id = ctx.output_buffer.store("cmd".into(), stdout, stderr, 0);
        let result = RunCommand
            .call(json!({ "command": format!("cat {}", id) }), &ctx)
            .await
            .expect("expected Ok");
        assert!(
            result.get("truncated").is_none(),
            "no truncated field: {:?}",
            result
        );
        assert!(
            result.get("stdout_shown").is_none(),
            "no stdout_shown: {:?}",
            result
        );
        assert!(
            result.get("output_id").is_none(),
            "no buffer ref: {:?}",
            result
        );
    }

    #[test]
    fn language_hints_covers_main_languages() {
        for lang in &[
            "rust",
            "python",
            "typescript",
            "javascript",
            "go",
            "java",
            "kotlin",
            "c",
            "cpp",
            "tsx",
            "jsx",
        ] {
            assert!(
                language_navigation_hints(lang).is_some(),
                "expected hints for '{}'",
                lang
            );
        }
    }

    #[test]
    fn language_hints_returns_none_for_unsupported() {
        // "bash" and "markdown" are real detect_language() values, just without hints
        assert!(language_navigation_hints("markdown").is_none());
        assert!(language_navigation_hints("bash").is_none());
        assert!(language_navigation_hints("unknown_lang").is_none());
    }

    #[test]
    fn system_prompt_draft_includes_language_hints() {
        let langs = vec!["rust".to_string(), "python".to_string()];
        let draft = build_system_prompt_draft(&langs, &[], None, None, &[]);
        assert!(
            draft.contains("## Language Navigation"),
            "should have Language Navigation section"
        );
        assert!(draft.contains("**rust:**"), "should have rust hints");
        assert!(draft.contains("**python:**"), "should have python hints");
        assert!(draft.contains("symbol"), "hints should mention symbol");
    }

    #[test]
    fn system_prompt_draft_omits_hints_for_unsupported_languages() {
        let langs = vec!["markdown".to_string()];
        let draft = build_system_prompt_draft(&langs, &[], None, None, &[]);
        assert!(
            !draft.contains("## Language Navigation"),
            "should not have Language Navigation for markdown-only"
        );
    }

    #[test]
    fn system_prompt_draft_isolates_hints_per_language() {
        let langs = vec!["python".to_string()];
        let draft = build_system_prompt_draft(&langs, &[], None, None, &[]);
        assert!(draft.contains("**python:**"), "should have python hints");
        assert!(
            !draft.contains("impl Trait for Type"),
            "rust hints should not leak into python-only draft"
        );
    }

    #[test]
    fn system_prompt_draft_includes_language_patterns_hint() {
        let langs = vec!["rust".to_string(), "python".to_string()];
        let entries = vec!["src/main.rs".to_string()];
        let draft = build_system_prompt_draft(&langs, &entries, None, None, &[]);
        assert!(
            draft.contains("language-patterns"),
            "draft should reference language-patterns memory"
        );
    }

    #[test]
    fn system_prompt_draft_is_concise() {
        let draft = build_system_prompt_draft(&[], &[], None, None, &[]);
        // Private memory rules removed — duplicates server_instructions.md
        assert!(
            !draft.contains("Private Memory Rules"),
            "draft should NOT include Private Memory Rules (covered by server_instructions)"
        );
        assert!(
            !draft.contains("Semantic Memories"),
            "draft should NOT include Semantic Memories section (covered by server_instructions)"
        );
        // Core sections still present
        assert!(draft.contains("## Entry Points"));
        assert!(draft.contains("## Key Abstractions"));
        assert!(draft.contains("## Navigation Strategy"));
        assert!(draft.contains("## Project Rules"));
    }

    #[test]
    fn system_prompt_draft_single_project_nav_strategy_unchanged() {
        // Single project: classic numbered list under ## Navigation Strategy
        let langs = vec!["rust".to_string()];
        let entries = vec!["src/main.rs".to_string()];
        let draft = build_system_prompt_draft(&langs, &entries, None, None, &[]);
        assert!(draft.contains("## Navigation Strategy\n"));
        assert!(
            draft.contains("list_symbols(\"src/main.rs\")"),
            "single-project nav should use first entry point"
        );
        assert!(
            !draft.contains("### "),
            "single-project draft should not have per-project subsections"
        );
    }

    #[test]
    fn system_prompt_draft_multi_project_nav_strategy_has_subsections() {
        use crate::workspace::DiscoveredProject;
        let projects = vec![
            DiscoveredProject {
                id: "backend".to_string(),
                relative_root: std::path::PathBuf::from("backend"),
                languages: vec!["rust".to_string()],
                manifest: Some("Cargo.toml".to_string()),
            },
            DiscoveredProject {
                id: "frontend".to_string(),
                relative_root: std::path::PathBuf::from("frontend"),
                languages: vec!["typescript".to_string()],
                manifest: Some("package.json".to_string()),
            },
        ];
        let draft = build_system_prompt_draft(&[], &[], None, Some(&projects), &[]);
        assert!(
            draft.contains("### backend (rust)"),
            "should have backend subsection"
        );
        assert!(
            draft.contains("### frontend (typescript)"),
            "should have frontend subsection"
        );
        assert!(
            draft.contains("scope=\"project:backend\""),
            "should have scoped semantic_search for backend"
        );
        assert!(
            draft.contains("scope=\"project:frontend\""),
            "should have scoped semantic_search for frontend"
        );
        assert!(
            draft.contains("memory(project: \"backend\""),
            "should have per-project memory hint for backend"
        );
        assert!(
            draft.contains("list_symbols(\"backend\")"),
            "should use project root as placeholder entry point"
        );
    }

    #[test]
    fn system_prompt_draft_multi_project_workspace_level_orient_step() {
        use crate::workspace::DiscoveredProject;
        let projects = vec![
            DiscoveredProject {
                id: "a".to_string(),
                relative_root: std::path::PathBuf::from("a"),
                languages: vec![],
                manifest: None,
            },
            DiscoveredProject {
                id: "b".to_string(),
                relative_root: std::path::PathBuf::from("b"),
                languages: vec![],
                manifest: None,
            },
        ];
        let draft = build_system_prompt_draft(&[], &[], None, Some(&projects), &[]);
        assert!(
            draft.contains("orient yourself to the workspace"),
            "workspace-level orient step should be present"
        );
    }

    #[test]
    fn system_prompt_draft_multi_project_search_tips_has_scope_warning() {
        use crate::workspace::DiscoveredProject;
        let projects = vec![
            DiscoveredProject {
                id: "backend".to_string(),
                relative_root: std::path::PathBuf::from("backend"),
                languages: vec!["rust".to_string()],
                manifest: Some("Cargo.toml".to_string()),
            },
            DiscoveredProject {
                id: "frontend".to_string(),
                relative_root: std::path::PathBuf::from("frontend"),
                languages: vec!["typescript".to_string()],
                manifest: Some("package.json".to_string()),
            },
        ];
        let draft = build_system_prompt_draft(&[], &[], None, Some(&projects), &[]);
        assert!(
            draft.contains("Workspace mode"),
            "should warn about workspace scoping in Search Tips"
        );
        assert!(
            draft.contains("project: \"backend\""),
            "should include per-project example for backend"
        );
        assert!(
            draft.contains("project: \"frontend\""),
            "should include per-project example for frontend"
        );
    }

    #[test]
    fn system_prompt_draft_single_project_search_tips_no_scope_warning() {
        let draft = build_system_prompt_draft(&[], &[], None, None, &[]);
        assert!(
            !draft.contains("Workspace mode"),
            "single-project draft should not have workspace scoping warning"
        );
    }

    #[test]
    fn system_prompt_draft_multi_project_rust_search_tip_uses_type_hint() {
        use crate::workspace::DiscoveredProject;
        let projects = vec![
            DiscoveredProject {
                id: "core".to_string(),
                relative_root: std::path::PathBuf::from("core"),
                languages: vec!["rust".to_string()],
                manifest: None,
            },
            DiscoveredProject {
                id: "ui".to_string(),
                relative_root: std::path::PathBuf::from("ui"),
                languages: vec!["typescript".to_string()],
                manifest: None,
            },
        ];
        let draft = build_system_prompt_draft(&[], &[], None, Some(&projects), &[]);
        assert!(
            draft.contains("key type or trait name"),
            "rust project tip should mention type/trait"
        );
        assert!(
            draft.contains("handler or component name"),
            "typescript project tip should mention handler/component"
        );
    }

    #[tokio::test]
    async fn onboarding_discovers_sub_projects() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Root: Kotlin
        std::fs::write(root.join("build.gradle.kts"), "").unwrap();
        std::fs::create_dir_all(root.join("src/main/kotlin")).unwrap();
        std::fs::write(root.join("src/main/kotlin/App.kt"), "fun main() {}").unwrap();

        // Sub: TypeScript
        let mcp = root.join("mcp-server");
        std::fs::create_dir_all(mcp.join("src")).unwrap();
        std::fs::write(mcp.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();
        std::fs::write(mcp.join("src/index.ts"), "").unwrap();

        // Sub: Python
        let py = root.join("python-services");
        std::fs::create_dir_all(&py).unwrap();
        std::fs::write(py.join("requirements.txt"), "flask\n").unwrap();
        std::fs::write(py.join("app.py"), "").unwrap();

        let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };

        let result = Onboarding
            .call(serde_json::json!({"force": true}), &ctx)
            .await
            .unwrap();

        let projects = result
            .get("projects")
            .expect("onboarding should return projects");
        let projects_arr = projects.as_array().unwrap();
        assert_eq!(
            projects_arr.len(),
            3,
            "should discover 3 projects (root + mcp-server + python-services), got {}",
            projects_arr.len()
        );

        // System prompt draft is now inside subagent_prompt
        let prompt = result["subagent_prompt"].as_str().unwrap();
        assert!(
            prompt.contains("mcp-server"),
            "subagent_prompt should mention mcp-server"
        );
    }

    #[test]
    fn run_command_format_compact_test_result() {
        let tool = RunCommand;
        let result = json!({
            "type": "test", "exit_code": 0,
            "passed": 533, "failed": 0, "ignored": 0,
            "output_id": "@cmd_abc123"
        });
        let text = tool.format_compact(&result).unwrap();
        assert!(text.contains("533"), "got: {text}");
        assert!(text.contains("passed"), "got: {text}");
    }

    #[test]
    fn run_command_format_compact_short_output() {
        let tool = RunCommand;
        let result = json!({ "stdout": "hello\nworld", "stderr": "", "exit_code": 0 });
        let text = tool.format_compact(&result).unwrap();
        assert!(text.contains("exit 0"), "got: {text}");
    }

    // Fix A: buffer-only queries should use BUFFER_QUERY_INLINE_CAP, not
    // the summarization threshold. A 100-line result should be returned fully inline.
    #[tokio::test]
    async fn buffer_query_returns_up_to_200_lines_inline() {
        let (_dir, ctx) = project_ctx().await;
        // Directly store 100 lines in the buffer (bypasses needs_summary)
        let content: String = (1..=100).map(|i| format!("{i}\n")).collect();
        let output_id = ctx.output_buffer.store("cmd".into(), content, "".into(), 0);

        // Query the buffer — 100 lines is within the BUFFER_QUERY_INLINE_CAP
        let query = format!("cat {output_id}");
        let result2 = RunCommand
            .call(json!({ "command": query, "timeout_secs": 5 }), &ctx)
            .await
            .unwrap();
        let stdout = result2["stdout"].as_str().unwrap_or("");
        let line_count = stdout.lines().count();
        assert_eq!(
            line_count, 100,
            "buffer query of 100 lines should return all 100 inline (got {line_count})"
        );
        assert!(
            result2["truncated"].is_null(),
            "should not be truncated when within inline cap"
        );
    }

    // Fix B: the truncation hint for buffer queries should show the *next* page range,
    // not always start from line 1.
    #[tokio::test]
    async fn buffer_query_truncation_hint_shows_next_page() {
        let (_dir, ctx) = project_ctx().await;
        // Directly store 300 lines (> BUFFER_QUERY_INLINE_CAP=100) in the buffer.
        // Lines padded to ~40 bytes so total exceeds token budget.
        let content: String = (1..=300).map(|i| format!("{i:>40}\n")).collect();
        let output_id = ctx.output_buffer.store("cmd".into(), content, "".into(), 0);

        // Query it — output exceeds 100-line cap, so hint should show next-page command
        let query = format!("cat {output_id}");
        let result2 = RunCommand
            .call(json!({ "command": query, "timeout_secs": 5 }), &ctx)
            .await
            .unwrap();
        let hint = result2["hint"].as_str().unwrap_or("");
        // Hint must guide to the NEXT page (line 101 onwards), not back to line 1
        assert!(
            hint.contains("101"),
            "hint should show next-page start (101), got: {hint}"
        );
        assert!(
            !hint.contains("'1,"),
            "hint must not restart from line 1, got: {hint}"
        );
    }

    // Fix C: when the first run_command looks like a plain file read (cat file),
    // the buffer creation hint should suggest read_file as an alternative.
    #[tokio::test]
    async fn cat_file_no_hint_field() {
        let (dir, ctx) = project_ctx().await;
        let md_path = dir.path().join("big_plan.md");
        let content: String = (1..=60).map(|i| format!("line {i}\n")).collect();
        std::fs::write(&md_path, content).unwrap();

        let result = RunCommand
            .call(
                json!({ "command": "cat big_plan.md", "timeout_secs": 5 }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(result["hint"].is_null(), "hint field should be absent");
    }

    #[tokio::test]
    async fn ack_handle_executes_stored_command() {
        let (_dir, ctx) = project_ctx().await;
        let handle = ctx
            .output_buffer
            .store_dangerous("echo hello_ack".to_string(), None, 30);

        let tool = RunCommand;
        let input = serde_json::json!({ "command": handle });
        let result = tool
            .call(input, &ctx)
            .await
            .expect("ack call should succeed");

        let stdout = result["stdout"].as_str().unwrap_or("");
        assert!(
            stdout.contains("hello_ack"),
            "expected 'hello_ack' in stdout, got: {stdout}"
        );
    }

    #[tokio::test]
    async fn ack_handle_unknown_returns_recoverable_error() {
        let (_dir, ctx) = project_ctx().await;
        let tool = RunCommand;
        let input = serde_json::json!({ "command": "@ack_deadbeef" });
        let err = tool
            .call(input, &ctx)
            .await
            .expect_err("unknown ack handle should return Err");
        assert!(
            err.to_string().contains("expired"),
            "error should mention 'expired', got: {err}"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_command_prepends_refresh_indicator_for_stale_file_handle() {
        use std::fs;
        let (dir, ctx) = project_ctx().await;

        let path = dir.path().join("data.txt");
        fs::write(&path, "original").unwrap();
        let id = ctx
            .output_buffer
            .store_file(path.to_string_lossy().to_string(), "original".to_string());

        // Make the file look newer than the cached entry
        let future = std::time::SystemTime::now() + std::time::Duration::from_secs(2);
        filetime::set_file_mtime(&path, filetime::FileTime::from_system_time(future)).unwrap();

        let result = RunCommand
            .call(json!({ "command": format!("cat {}", id) }), &ctx)
            .await
            .unwrap();

        let stdout = result["stdout"].as_str().unwrap();
        assert!(
            stdout.starts_with(&format!("↻ {} refreshed from disk", id)),
            "expected refresh indicator, got: {:?}",
            stdout
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_command_buffered_output_has_output_id_before_stdout() {
        // Regression: output_id (the buffer reference the agent needs to query results)
        // was appended dynamically after the summary object was built, placing it AFTER
        // stdout/content fields. It must appear before content.
        let (_dir, ctx) = project_ctx().await;
        // seq 100 produces 100 lines, exceeding the token budget to trigger buffering.
        let result = RunCommand
            .call(json!({ "command": "seq 3000" }), &ctx)
            .await
            .unwrap();

        assert!(
            result["output_id"].is_string(),
            "expected buffered output (output_id present) for large command, got: {result:?}"
        );

        let keys: Vec<&str> = result
            .as_object()
            .unwrap()
            .keys()
            .map(|s| s.as_str())
            .collect();

        let output_id_pos = keys.iter().position(|k| *k == "output_id").unwrap();
        // stdout is the content field in generic summaries; failures/first_error in others.
        // We assert output_id appears before any content-heavy field.
        let stdout_pos = keys
            .iter()
            .position(|k| *k == "stdout")
            .unwrap_or(keys.len());

        assert!(
            output_id_pos < stdout_pos,
            "output_id must appear before stdout (content payload), got key order: {keys:?}"
        );
    }

    #[tokio::test]
    async fn piped_grep_returns_unfiltered_ref() {
        let (dir, ctx) = project_ctx().await;
        // Create a file with several lines; grep for just one
        std::fs::write(
            dir.path().join("items.txt"),
            "apple\nbanana\ncherry\ndates\nelderberry\n",
        )
        .unwrap();
        let result = RunCommand
            .call(json!({ "command": "cat items.txt | grep apple" }), &ctx)
            .await
            .unwrap();

        // unfiltered_output ref should be present
        assert!(
            result["unfiltered_output"].is_string(),
            "expected unfiltered_output field, got: {result}"
        );
        let ref_id = result["unfiltered_output"].as_str().unwrap();

        // Query the buffer: full content should include banana (filtered out by grep)
        let full = RunCommand
            .call(json!({ "command": format!("cat {ref_id}") }), &ctx)
            .await
            .unwrap();
        let stdout = full["stdout"].as_str().unwrap_or("");
        assert!(
            stdout.contains("banana"),
            "unfiltered output missing 'banana': {stdout}"
        );
        assert!(
            stdout.contains("apple"),
            "unfiltered output missing 'apple': {stdout}"
        );
    }

    #[tokio::test]
    async fn non_filter_pipe_no_unfiltered_ref() {
        let (_dir, ctx) = project_ctx().await;
        // Second stage is not a known filter — no unfiltered_output
        let result = RunCommand
            .call(json!({ "command": "echo hello | cat" }), &ctx)
            .await
            .unwrap();
        assert!(
            result.get("unfiltered_output").is_none(),
            "unexpected unfiltered_output for non-filter pipe: {result}"
        );
    }

    #[tokio::test]
    async fn grep_no_match_suppresses_unfiltered_ref() {
        let (dir, ctx) = project_ctx().await;
        std::fs::write(dir.path().join("items.txt"), "apple\nbanana\ncherry\n").unwrap();

        // `cat | grep | head`: tee is injected before `head`, capturing grep's output.
        // When grep matches nothing, the tee file is empty → unfiltered_output should be
        // suppressed (no value in surfacing a handle to an empty buffer).
        let result = RunCommand
            .call(
                json!({ "command": "cat items.txt | grep zzz_no_match | head -5" }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(
            result.get("unfiltered_output").is_none(),
            "unfiltered_output should be absent when middle filter matches nothing, got: {result}"
        );
        assert!(
            result.get("stdout").is_none(),
            "stdout should be absent when grep matches nothing, got: {result}"
        );

        // Contrast: single-pipe `cat | grep` puts the tee before grep, capturing the full
        // cat output — that IS useful even when grep finds nothing.
        let result2 = RunCommand
            .call(
                json!({ "command": "cat items.txt | grep zzz_no_match" }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(
            result2["unfiltered_output"].is_string(),
            "unfiltered_output should be present for single-pipe grep (tee captures cat output): {result2}"
        );
    }

    #[tokio::test]
    async fn unfiltered_truncated_when_over_threshold() {
        let (dir, ctx) = project_ctx().await;
        // Write content exceeding MAX_INLINE_TOKENS token budget; grep for just one line
        let over_bytes = crate::tools::MAX_INLINE_TOKENS * 4 + 1000;
        let mut content = String::new();
        for i in 0.. {
            content.push_str(&format!("line{i}\n"));
            if content.len() > over_bytes {
                break;
            }
        }
        std::fs::write(dir.path().join("big.txt"), &content).unwrap();
        let result = RunCommand
            .call(json!({ "command": "cat big.txt | grep line0" }), &ctx)
            .await
            .unwrap();
        // truncated flag should be set (content exceeds token budget)
        assert_eq!(
            result["unfiltered_truncated"],
            json!(true),
            "expected truncated flag: {result}"
        );
    }

    #[test]
    fn language_patterns_covers_all_supported_languages() {
        let supported = [
            "rust",
            "python",
            "typescript",
            "javascript",
            "go",
            "java",
            "kotlin",
        ];
        for lang in &supported {
            assert!(
                language_patterns(lang).is_some(),
                "language_patterns() should return Some for {lang}"
            );
        }
    }

    #[test]
    fn language_patterns_returns_none_for_unsupported() {
        assert!(language_patterns("haskell").is_none());
        assert!(language_patterns("ruby").is_none());
        assert!(language_patterns("c").is_none());
    }

    #[test]
    fn build_language_patterns_memory_assembles_detected_languages() {
        let langs = vec!["rust".to_string(), "python".to_string()];
        let result = build_language_patterns_memory(&langs);
        assert!(result.is_some());
        let content = result.unwrap();
        assert!(content.contains("### Rust"));
        assert!(content.contains("### Python"));
        assert!(!content.contains("### Go"));
        assert!(content.starts_with("# Language Patterns"));
    }

    #[test]
    fn build_language_patterns_memory_returns_none_for_unsupported_only() {
        let langs = vec!["haskell".to_string(), "ruby".to_string()];
        let result = build_language_patterns_memory(&langs);
        assert!(result.is_none());
    }

    #[test]
    fn build_language_patterns_memory_returns_none_for_empty() {
        let result = build_language_patterns_memory(&[]);
        assert!(result.is_none());
    }

    // ---------- hardware detection ----------

    #[test]
    fn model_options_ollama_available_recommends_allminilm() {
        let ctx = super::HardwareContext {
            ollama_available: true,
            ollama_host: "http://localhost:11434".into(),
            gpu: None,
            ram_gb: 16,
            cpu_cores: 8,
        };
        let opts = super::model_options_for_hardware(&ctx);
        // With Ollama: local:AllMiniLML6V2Q (recommended) + url hint + Jina = 3 entries
        assert_eq!(opts.len(), 3);
        assert_eq!(opts[0].id, "local:AllMiniLML6V2Q");
        assert!(opts[0].recommended);
        assert!(!opts[1].recommended);
        assert!(!opts[2].recommended);
    }

    #[test]
    fn model_options_cpu_only_recommends_jina() {
        let ctx = super::HardwareContext {
            ollama_available: false,
            ollama_host: "http://localhost:11434".into(),
            gpu: None,
            ram_gb: 8,
            cpu_cores: 4,
        };
        let opts = super::model_options_for_hardware(&ctx);
        // Without Ollama: local:AllMiniLML6V2Q (recommended) + Jina + url hint = 3 entries
        assert_eq!(opts[0].id, "local:AllMiniLML6V2Q");
        assert!(opts[0].recommended);
        // url hint is last
        assert_eq!(opts[opts.len() - 1].id, "url");
    }

    #[test]
    fn model_options_exactly_one_recommended() {
        let ctx = super::HardwareContext {
            ollama_available: true,
            ollama_host: "http://localhost:11434".into(),
            gpu: Some(super::GpuInfo::Nvidia {
                name: "RTX 3080".into(),
                vram_mb: 10240,
            }),
            ram_gb: 32,
            cpu_cores: 16,
        };
        let opts = super::model_options_for_hardware(&ctx);
        let recommended_count = opts.iter().filter(|o| o.recommended).count();
        assert_eq!(recommended_count, 1);
    }

    #[test]
    fn model_options_default_is_local_allminilm() {
        let hw = super::HardwareContext {
            ollama_available: false,
            ollama_host: "http://localhost:11434".into(),
            gpu: None,
            ram_gb: 16,
            cpu_cores: 8,
        };
        let options = super::model_options_for_hardware(&hw);
        assert_eq!(options[0].id, "local:AllMiniLML6V2Q");
        assert!(options[0].recommended);
        // Must have a url hint option
        assert!(
            options.iter().any(|o| o.reason.contains("url")),
            "must mention url as an option"
        );
    }

    #[test]
    fn model_options_with_ollama_still_recommends_local() {
        let hw = super::HardwareContext {
            ollama_available: true,
            ollama_host: "http://localhost:11434".into(),
            gpu: None,
            ram_gb: 16,
            cpu_cores: 8,
        };
        let options = super::model_options_for_hardware(&hw);
        assert_eq!(options[0].id, "local:AllMiniLML6V2Q");
        assert!(options[0].recommended);
        // Ollama option should mention url
        assert!(
            options
                .iter()
                .any(|o| o.reason.contains("url") || o.reason.contains("Ollama")),
            "must mention Ollama or url option"
        );
    }

    #[test]
    fn ollama_tcp_addr_strips_http_prefix() {
        assert_eq!(
            super::ollama_tcp_addr("http://localhost:11434"),
            "localhost:11434"
        );
        assert_eq!(
            super::ollama_tcp_addr("https://remote:11434"),
            "remote:11434"
        );
        assert_eq!(super::ollama_tcp_addr("localhost:11434"), "localhost:11434");
        assert_eq!(super::ollama_tcp_addr("myhost"), "myhost:11434");
    }

    #[tokio::test]
    async fn onboarding_includes_hardware_and_model_options() {
        let (_dir, ctx) = project_ctx().await;
        let result = Onboarding.call(json!({}), &ctx).await.unwrap();

        // hardware and model_options are now inside subagent_prompt
        let prompt = result["subagent_prompt"].as_str().unwrap();
        assert!(
            prompt.contains("**Hardware:**"),
            "subagent_prompt must contain hardware data"
        );
        assert!(
            prompt.contains("cpu_cores"),
            "subagent_prompt must contain cpu_cores"
        );
        assert!(
            prompt.contains("**Model options:**"),
            "subagent_prompt must contain model options"
        );
        assert!(
            prompt.contains("recommended"),
            "subagent_prompt must contain recommended model info"
        );
    }

    #[tokio::test]
    async fn onboarding_writes_recommended_model_to_config() {
        let (dir, ctx) = project_ctx().await;
        // Remove any pre-existing config so onboarding creates a fresh one
        let _ = std::fs::remove_file(dir.path().join(".codescout/project.toml"));

        let result = Onboarding.call(json!({}), &ctx).await.unwrap();

        let toml = std::fs::read_to_string(dir.path().join(".codescout/project.toml")).unwrap();
        // model_options are now inside subagent_prompt; verify the config was written
        // with the recommended model by checking subagent_prompt contains the model
        // and the config contains a model setting
        let prompt = result["subagent_prompt"].as_str().unwrap();
        assert!(
            prompt.contains("**Model options:**"),
            "subagent_prompt must contain model options"
        );
        assert!(
            toml.contains("model = "),
            "project.toml should contain a model setting\ntoml:\n{toml}"
        );
        // Should NOT contain the old hardcoded default
        assert!(
            !toml.contains("mxbai-embed-large"),
            "project.toml should not contain mxbai-embed-large\ntoml:\n{toml}"
        );
    }

    #[tokio::test]
    async fn onboarding_includes_protected_memories_for_existing_topic() {
        let (dir, ctx) = project_ctx().await;

        // Pre-populate a protected memory with content
        let memories_dir = dir.path().join(".codescout").join("memories");
        std::fs::create_dir_all(&memories_dir).unwrap();
        std::fs::write(
            memories_dir.join("gotchas.md"),
            "# Gotchas\n\n- **Problem:** foo\n  **Fix:** bar\n",
        )
        .unwrap();

        // Create config with protected = ["gotchas"]
        let config_path = dir.path().join(".codescout").join("project.toml");
        std::fs::write(
            &config_path,
            "[project]\nname = \"test\"\nlanguages = [\"rust\"]\n\n[memory]\nprotected = [\"gotchas\"]\n",
        )
        .unwrap();

        // Force onboarding
        let result = Onboarding
            .call(json!({ "force": true }), &ctx)
            .await
            .unwrap();

        // protected_memories is no longer top-level — it's inside subagent_prompt
        assert!(result.get("protected_memories").is_none());
        let prompt = result["subagent_prompt"].as_str().unwrap();
        assert!(
            prompt.contains("**Protected memories:**"),
            "subagent_prompt must contain protected memories"
        );
        assert!(
            prompt.contains("gotchas"),
            "subagent_prompt must mention gotchas topic"
        );
        assert!(
            prompt.contains("# Gotchas"),
            "subagent_prompt must contain gotchas content"
        );
    }

    #[tokio::test]
    async fn onboarding_protected_memory_missing_topic() {
        let (dir, ctx) = project_ctx().await;

        // Config protects "gotchas" but no gotchas.md exists
        let config_path = dir.path().join(".codescout").join("project.toml");
        std::fs::write(
            &config_path,
            "[project]\nname = \"test\"\nlanguages = [\"rust\"]\n\n[memory]\nprotected = [\"gotchas\"]\n",
        )
        .unwrap();

        let result = Onboarding
            .call(json!({ "force": true }), &ctx)
            .await
            .unwrap();

        // protected_memories now inside subagent_prompt
        let prompt = result["subagent_prompt"].as_str().unwrap();
        assert!(prompt.contains("**Protected memories:**"));
        // The missing topic should show exists: false in the serialized JSON
        assert!(prompt.contains("\"exists\": false"));
    }

    #[tokio::test]
    async fn onboarding_excludes_programmatic_from_protected() {
        let (dir, ctx) = project_ctx().await;

        let config_path = dir.path().join(".codescout").join("project.toml");
        std::fs::write(
            &config_path,
            "[project]\nname = \"test\"\nlanguages = [\"rust\"]\n\n[memory]\nprotected = [\"onboarding\", \"language-patterns\", \"gotchas\"]\n",
        )
        .unwrap();

        let result = Onboarding
            .call(json!({ "force": true }), &ctx)
            .await
            .unwrap();

        // protected_memories now inside subagent_prompt as serialized JSON
        let prompt = result["subagent_prompt"].as_str().unwrap();
        assert!(prompt.contains("**Protected memories:**"));
        // Programmatic topics excluded — should not appear as keys in the serialized JSON
        assert!(
            !prompt.contains("\"onboarding\":"),
            "onboarding should be excluded from protected memories"
        );
        assert!(
            !prompt.contains("\"language-patterns\":"),
            "language-patterns should be excluded from protected memories"
        );
        // Non-programmatic topic still present
        assert!(
            prompt.contains("\"gotchas\":"),
            "gotchas should be present in protected memories"
        );
    }

    #[tokio::test]
    async fn onboarding_protected_memory_untracked_no_anchors() {
        let (dir, ctx) = project_ctx().await;

        let memories_dir = dir.path().join(".codescout").join("memories");
        std::fs::create_dir_all(&memories_dir).unwrap();
        std::fs::write(
            memories_dir.join("gotchas.md"),
            "# Gotchas\n\n- Some gotcha referencing src/main.rs\n",
        )
        .unwrap();
        // No .anchors.toml file created

        let config_path = dir.path().join(".codescout").join("project.toml");
        std::fs::write(
            &config_path,
            "[project]\nname = \"test\"\nlanguages = [\"rust\"]\n\n[memory]\nprotected = [\"gotchas\"]\n",
        )
        .unwrap();

        let result = Onboarding
            .call(json!({ "force": true }), &ctx)
            .await
            .unwrap();

        // Staleness info is now serialized inside subagent_prompt
        let prompt = result["subagent_prompt"].as_str().unwrap();
        assert!(prompt.contains("\"untracked\": true"));
    }

    #[tokio::test]
    async fn onboarding_protected_memory_stale_anchors() {
        let (dir, ctx) = project_ctx().await;

        // Write a source file and compute its hash
        let src_file = dir.path().join("main.rs");
        std::fs::write(&src_file, "fn main() {}").unwrap();
        let original_hash = crate::embed::index::hash_file(&src_file).unwrap();

        // Create a protected memory referencing that file
        let memories_dir = dir.path().join(".codescout").join("memories");
        std::fs::create_dir_all(&memories_dir).unwrap();
        std::fs::write(
            memories_dir.join("gotchas.md"),
            "# Gotchas\n\n- **Problem:** main.rs has issue\n  **Fix:** fix it\n",
        )
        .unwrap();

        // Create anchor sidecar with the original hash
        use crate::memory::anchors::{
            anchor_path_for_topic, write_anchor_file, AnchorFile, PathAnchor,
        };
        let anchor_file = AnchorFile {
            anchors: vec![PathAnchor {
                path: "main.rs".to_string(),
                hash: original_hash,
            }],
        };
        let anchor_path = anchor_path_for_topic(&memories_dir, "gotchas");
        write_anchor_file(&anchor_path, &anchor_file).unwrap();

        // Now modify the source file so the hash changes
        std::fs::write(&src_file, "fn main() { println!(\"changed\"); }").unwrap();

        // Config
        let config_path = dir.path().join(".codescout").join("project.toml");
        std::fs::write(
            &config_path,
            "[project]\nname = \"test\"\nlanguages = [\"rust\"]\n\n[memory]\nprotected = [\"gotchas\"]\n",
        )
        .unwrap();

        let result = Onboarding
            .call(json!({ "force": true }), &ctx)
            .await
            .unwrap();

        // Staleness info is now serialized inside subagent_prompt
        let prompt = result["subagent_prompt"].as_str().unwrap();
        assert!(prompt.contains("\"untracked\": false"));
        assert!(prompt.contains("\"status\": \"changed\""));
        assert!(prompt.contains("\"path\": \"main.rs\""));
    }

    #[tokio::test]
    async fn onboarding_protected_memory_fresh_anchors() {
        let (dir, ctx) = project_ctx().await;

        // Write a source file and compute its hash
        let src_file = dir.path().join("main.rs");
        std::fs::write(&src_file, "fn main() {}").unwrap();
        let current_hash = crate::embed::index::hash_file(&src_file).unwrap();

        // Create a protected memory referencing that file
        let memories_dir = dir.path().join(".codescout").join("memories");
        std::fs::create_dir_all(&memories_dir).unwrap();
        std::fs::write(
            memories_dir.join("gotchas.md"),
            "# Gotchas\n\n- **Problem:** main.rs has issue\n  **Fix:** fix it\n",
        )
        .unwrap();

        // Create anchor sidecar with the CURRENT hash (file hasn't changed)
        use crate::memory::anchors::{
            anchor_path_for_topic, write_anchor_file, AnchorFile, PathAnchor,
        };
        let anchor_file = AnchorFile {
            anchors: vec![PathAnchor {
                path: "main.rs".to_string(),
                hash: current_hash,
            }],
        };
        let anchor_path = anchor_path_for_topic(&memories_dir, "gotchas");
        write_anchor_file(&anchor_path, &anchor_file).unwrap();

        // Do NOT modify the source file — it stays the same

        // Config
        let config_path = dir.path().join(".codescout").join("project.toml");
        std::fs::write(
            &config_path,
            "[project]\nname = \"test\"\nlanguages = [\"rust\"]\n\n[memory]\nprotected = [\"gotchas\"]\n",
        )
        .unwrap();

        let result = Onboarding
            .call(json!({ "force": true }), &ctx)
            .await
            .unwrap();

        // Staleness info is now serialized inside subagent_prompt
        let prompt = result["subagent_prompt"].as_str().unwrap();
        assert!(prompt.contains("\"untracked\": false"));
        // Fresh = no stale files, so stale_files should be empty array
        assert!(prompt.contains("\"stale_files\": []"));
    }

    #[tokio::test]
    async fn onboarding_force_with_protected_memory_full_flow() {
        let (dir, ctx) = project_ctx().await;

        // First onboarding — creates everything fresh
        let _ = Onboarding.call(json!({}), &ctx).await.unwrap();

        // Manually write a gotchas memory to simulate user curation
        let memories_dir = dir.path().join(".codescout").join("memories");
        std::fs::write(
            memories_dir.join("gotchas.md"),
            "# Gotchas\n\n- **Problem:** custom user gotcha\n  **Fix:** do the thing\n",
        )
        .unwrap();

        // Force re-onboarding
        let result = Onboarding
            .call(json!({ "force": true }), &ctx)
            .await
            .unwrap();

        // Should have standard fields plus subagent_prompt
        assert!(result.get("languages").is_some());
        assert!(result.get("subagent_prompt").is_some());
        // Old fields removed
        assert!(result.get("instructions").is_none());
        assert!(result.get("protected_memories").is_none());

        // Protected memories are now inside subagent_prompt
        let prompt = result["subagent_prompt"].as_str().unwrap();
        assert!(prompt.contains("custom user gotcha"));
        // No anchor sidecar was created, so staleness should be untracked
        assert!(prompt.contains("\"untracked\": true"));
    }

    #[tokio::test]
    async fn onboarding_creates_workspace_toml_for_multi_project() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Root: Kotlin
        std::fs::write(root.join("build.gradle.kts"), "").unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/App.kt"), "").unwrap();

        // Sub: TypeScript
        let mcp = root.join("mcp-server");
        std::fs::create_dir_all(&mcp).unwrap();
        std::fs::write(mcp.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();

        let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };

        Onboarding
            .call(serde_json::json!({"force": true}), &ctx)
            .await
            .unwrap();

        let ws_path = crate::config::workspace::workspace_config_path(&root);
        assert!(
            ws_path.exists(),
            "workspace.toml should be created for multi-project repos"
        );

        let content = std::fs::read_to_string(&ws_path).unwrap();
        let config: crate::config::workspace::WorkspaceConfig = toml::from_str(&content).unwrap();
        assert_eq!(
            config.projects.len(),
            2,
            "should have 2 projects (root + mcp-server), got: {:?}",
            config.projects.iter().map(|p| &p.id).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn onboarding_skips_workspace_toml_for_single_project() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

        let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };

        Onboarding
            .call(serde_json::json!({"force": true}), &ctx)
            .await
            .unwrap();

        let ws_path = crate::config::workspace::workspace_config_path(&root);
        assert!(
            !ws_path.exists(),
            "workspace.toml should NOT be created for single-project repos"
        );
    }

    #[tokio::test]
    async fn single_project_onboarding_unchanged() {
        let (_dir, ctx) = project_ctx().await;
        let result = Onboarding.call(json!({}), &ctx).await.unwrap();

        // Single project: no workspace_mode field or it's false
        assert!(result.get("workspace_mode").is_none() || result["workspace_mode"] == false);
        // subagent_prompt should contain the standard Phase 1/Phase 2, not workspace phases
        let prompt = result["subagent_prompt"].as_str().unwrap_or("");
        assert!(prompt.contains("Phase 2: Explore the Code"));
        assert!(prompt.contains("Phase 3: Write the Memories"));
        assert!(!prompt.contains("Workspace Survey"));
        assert!(!prompt.contains("Workspace Survey"));
    }

    #[tokio::test]
    async fn single_project_call_content_has_no_project_prompts() {
        let (_dir, ctx) = project_ctx().await;
        let content = Onboarding.call_content(json!({}), &ctx).await.unwrap();
        assert_eq!(content.len(), 1);
        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        let parsed: serde_json::Value = serde_json::from_str(text).expect("must be JSON");
        assert!(
            parsed.get("project_prompts").is_none(),
            "single-project must NOT have project_prompts"
        );
        assert!(
            parsed.get("synthesis_prompt_path").is_none(),
            "single-project must NOT have synthesis_prompt_path"
        );
    }

    #[tokio::test]
    async fn onboarding_call_content_includes_workspace_info() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        setup_workspace_dirs(root);

        let ctx = project_ctx_at(root).await;
        let content = Onboarding.call_content(json!({}), &ctx).await.unwrap();
        assert_eq!(
            content.len(),
            1,
            "call_content must return 1 structured block, got {}",
            content.len()
        );

        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        let parsed: serde_json::Value =
            serde_json::from_str(text).expect("block must be valid JSON");

        // summary should mention workspace
        let summary = parsed["summary"].as_str().unwrap_or("");
        assert!(
            summary.contains("workspace") || summary.contains("project"),
            "summary should mention workspace mode, got: {summary}"
        );

        // prompt_path must point at the markdown file
        let prompt_path = parsed["prompt_path"].as_str().unwrap_or("");
        assert!(
            prompt_path.contains("onboarding-prompt.md"),
            "must have prompt_path pointing to onboarding-prompt.md, got: {prompt_path:?}"
        );

        // Must NOT have output_id
        assert!(
            parsed.get("output_id").is_none(),
            "must NOT have output_id (old buffer pattern removed)"
        );

        // The file content itself should contain workspace instructions.
        let full_path = root.join(prompt_path);
        assert!(
            full_path.exists(),
            "onboarding-prompt.md must exist on disk"
        );
        let file_content = std::fs::read_to_string(&full_path).unwrap();
        assert!(
            file_content.contains("Workspace Survey"),
            "file content should include workspace instructions"
        );

        // Must have project_prompts array (workspace parallel dispatch)
        let project_prompts = parsed["project_prompts"]
            .as_array()
            .expect("workspace call_content must have project_prompts");
        assert!(
            project_prompts.len() >= 2,
            "workspace must have at least 2 project prompts, got {}",
            project_prompts.len()
        );

        // Must have synthesis_prompt_path
        assert!(
            parsed["synthesis_prompt_path"].as_str().is_some(),
            "workspace call_content must have synthesis_prompt_path"
        );
    }

    #[tokio::test]
    async fn onboarding_call_content_workspace_writes_per_project_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        setup_workspace_dirs(root);

        let ctx = project_ctx_at(root).await;
        let content = Onboarding
            .call_content(json!({ "force": true }), &ctx)
            .await
            .unwrap();

        assert_eq!(content.len(), 1);
        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        let parsed: serde_json::Value = serde_json::from_str(text).expect("must be JSON");

        // Must have project_prompts array
        let project_prompts = parsed["project_prompts"]
            .as_array()
            .expect("workspace must have project_prompts");
        assert!(
            project_prompts.len() >= 2,
            "must have at least 2 project prompts"
        );

        // Each entry must have id and path
        for pp in project_prompts {
            let id = pp["id"].as_str().expect("must have id");
            let path = pp["path"].as_str().expect("must have path");
            assert!(
                path.contains("onboarding-project-"),
                "path must contain project prefix"
            );
            // File must exist
            assert!(
                root.join(path).exists(),
                "prompt file must exist for {}",
                id
            );
        }

        // Must have synthesis_prompt_path
        let synthesis_path = parsed["synthesis_prompt_path"]
            .as_str()
            .expect("must have synthesis_prompt_path");
        assert!(
            root.join(synthesis_path).exists(),
            "synthesis file must exist"
        );

        // Instructions must mention read_markdown
        let instructions = parsed["instructions"].as_str().unwrap_or("");
        assert!(
            instructions.contains("read_markdown"),
            "instructions must reference read_markdown"
        );
    }

    #[tokio::test]
    async fn onboarding_includes_workspace_mode_and_per_project_protected() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        setup_workspace_dirs(root);

        let ctx = project_ctx_at(root).await;
        let result = Onboarding.call(json!({}), &ctx).await.unwrap();

        assert_eq!(result["workspace_mode"], true);
        // per_project_protected_memories is now inside subagent_prompt
        assert!(result.get("per_project_protected_memories").is_none());
        let prompt = result["subagent_prompt"].as_str().unwrap();
        // Each discovered project should have an entry in the serialized protected memories
        assert!(
            prompt.contains("**Per-project protected memories:**"),
            "subagent_prompt must contain per-project protected memories"
        );
        assert!(prompt.contains("api"), "api project must be mentioned");
        assert!(prompt.contains("web"), "web project must be mentioned");
    }

    #[tokio::test]
    async fn onboarding_writes_per_project_programmatic_memories() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        setup_workspace_dirs(root);

        let ctx = project_ctx_at(root).await;
        Onboarding.call(json!({}), &ctx).await.unwrap();

        // Per-project memory directories should exist with onboarding + language-patterns
        let api_mem = root.join(".codescout/projects/api/memories");
        assert!(
            api_mem.join("onboarding.md").exists(),
            "api onboarding memory missing"
        );
        assert!(
            api_mem.join("language-patterns.md").exists(),
            "api language-patterns missing"
        );
        let web_mem = root.join(".codescout/projects/web/memories");
        assert!(
            web_mem.join("onboarding.md").exists(),
            "web onboarding memory missing"
        );
        assert!(
            web_mem.join("language-patterns.md").exists(),
            "web language-patterns missing"
        );
    }

    #[tokio::test]
    async fn workspace_onboarding_full_flow() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        setup_workspace_dirs(root);

        let ctx = project_ctx_at(root).await;

        // First onboarding
        let result = Onboarding.call(json!({}), &ctx).await.unwrap();

        // Workspace mode active
        assert_eq!(result["workspace_mode"], true);
        assert!(result["projects"].as_array().unwrap().len() >= 2);

        // Per-project programmatic memories written
        assert!(root
            .join(".codescout/projects/api/memories/onboarding.md")
            .exists());
        assert!(root
            .join(".codescout/projects/web/memories/onboarding.md")
            .exists());

        // workspace.toml created
        assert!(crate::config::workspace::workspace_config_path(&root).exists());

        // subagent_prompt contains workspace sections and system prompt draft
        let prompt = result["subagent_prompt"].as_str().unwrap();
        assert!(
            prompt.contains("Workspace"),
            "subagent_prompt should contain workspace content"
        );
        assert!(
            prompt.contains("Workspace Survey"),
            "subagent_prompt should contain Phase 1A"
        );

        // System prompt draft is inside subagent_prompt
        assert!(prompt.contains("## System Prompt Draft"));
        assert!(prompt.contains("api"));
        assert!(prompt.contains("web"));
        assert!(prompt.contains("memory(project:"));

        // call_content delivers 1 structured JSON block with prompt_path
        let content = Onboarding
            .call_content(json!({ "force": true }), &ctx)
            .await
            .unwrap();
        assert_eq!(
            content.len(),
            1,
            "call_content must return 1 structured block"
        );
        let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
        let parsed: serde_json::Value =
            serde_json::from_str(text).expect("block must be valid JSON");

        // prompt_path must point to the markdown file
        let prompt_path = parsed["prompt_path"].as_str().unwrap_or("");
        assert!(
            prompt_path.contains("onboarding-prompt.md"),
            "must have prompt_path pointing to onboarding-prompt.md, got: {prompt_path:?}"
        );

        // Must NOT have output_id
        assert!(
            parsed.get("output_id").is_none(),
            "must NOT have output_id (old buffer pattern removed)"
        );

        // summary should contain workspace info
        let summary = parsed["summary"].as_str().unwrap_or("");
        assert!(
            summary.contains("workspace") || summary.contains("project"),
            "summary should mention workspace, got: {summary}"
        );

        // The file on disk has workspace content
        let full_path = root.join(prompt_path);
        assert!(
            full_path.exists(),
            "onboarding-prompt.md must exist on disk"
        );
        let file_content = std::fs::read_to_string(&full_path).unwrap();
        assert!(
            file_content.contains("Workspace Survey"),
            "file content must contain workspace content"
        );

        // Must have project_prompts (new parallel dispatch fields)
        let project_prompts = parsed["project_prompts"]
            .as_array()
            .expect("workspace full flow must have project_prompts");
        assert!(
            project_prompts.len() >= 2,
            "must have at least 2 project prompts"
        );
        for pp in project_prompts {
            assert!(
                pp["id"].as_str().is_some(),
                "each project prompt must have id"
            );
            assert!(
                pp["path"].as_str().is_some(),
                "each project prompt must have path"
            );
            let pp_path = pp["path"].as_str().unwrap();
            assert!(
                root.join(pp_path).exists(),
                "project prompt file must exist for {}",
                pp["id"]
            );
        }

        // Must have synthesis_prompt_path
        let synthesis_path = parsed["synthesis_prompt_path"]
            .as_str()
            .expect("must have synthesis_prompt_path");
        assert!(
            root.join(synthesis_path).exists(),
            "synthesis file must exist on disk"
        );

        // format_compact shows workspace info
        let compact = Onboarding.format_compact(&result).unwrap_or_default();
        assert!(compact.contains("workspace"));
    }

    #[test]
    fn parse_timeout_input_correct_key_small() {
        let input = serde_json::json!({ "timeout_secs": 120 });
        let (secs, hint) = parse_timeout_input(&input);
        assert_eq!(secs, 120);
        assert!(hint.is_none());
    }

    #[test]
    fn parse_timeout_input_correct_key_boundary() {
        let input = serde_json::json!({ "timeout_secs": 86400 });
        let (secs, hint) = parse_timeout_input(&input);
        assert_eq!(secs, 86400);
        assert!(hint.is_none());
    }

    #[test]
    fn parse_timeout_input_correct_key_over_boundary() {
        let input = serde_json::json!({ "timeout_secs": 86401 });
        let (secs, hint) = parse_timeout_input(&input);
        assert_eq!(secs, 86);
        let h = hint.unwrap();
        assert!(h.contains("86401"), "hint should contain raw value: {h}");
        assert!(
            h.contains("86s"),
            "hint should contain converted value: {h}"
        );
    }

    #[test]
    fn parse_timeout_input_correct_key_large() {
        let input = serde_json::json!({ "timeout_secs": 120_000u64 });
        let (secs, hint) = parse_timeout_input(&input);
        assert_eq!(secs, 120);
        assert!(hint.is_some());
    }

    #[test]
    fn parse_timeout_input_correct_key_zero() {
        let input = serde_json::json!({ "timeout_secs": 0 });
        let (secs, hint) = parse_timeout_input(&input);
        assert_eq!(secs, 30);
        assert!(hint.is_some());
    }

    #[test]
    fn parse_timeout_input_wrong_key_small() {
        let input = serde_json::json!({ "timeout": 300 });
        let (secs, hint) = parse_timeout_input(&input);
        assert_eq!(secs, 300);
        assert!(hint.is_some());
    }

    #[test]
    fn parse_timeout_input_wrong_key_large() {
        let input = serde_json::json!({ "timeout": 120_000u64 });
        let (secs, hint) = parse_timeout_input(&input);
        assert_eq!(secs, 120);
        assert!(hint.is_some());
    }

    #[test]
    fn parse_timeout_input_wrong_key_zero() {
        let input = serde_json::json!({ "timeout": 0 });
        let (secs, hint) = parse_timeout_input(&input);
        assert_eq!(secs, 30);
        assert!(hint.is_some());
    }

    #[test]
    fn parse_timeout_input_neither_key() {
        let input = serde_json::json!({});
        let (secs, hint) = parse_timeout_input(&input);
        assert_eq!(secs, 30);
        assert!(hint.is_none());
    }

    #[test]
    fn parse_timeout_input_both_keys_valid() {
        // timeout_secs wins; timeout is silently ignored; no hint (timeout_secs value is valid)
        let input = serde_json::json!({ "timeout_secs": 60, "timeout": 5000 });
        let (secs, hint) = parse_timeout_input(&input);
        assert_eq!(secs, 60);
        assert!(hint.is_none());
    }

    /// A dangerous command must return the pending_ack shape (two-round-trip pattern).
    #[tokio::test]
    async fn dangerous_command_returns_pending_ack() {
        let (_dir, ctx) = project_ctx().await;
        assert!(
            ctx.peer.is_none(),
            "test requires peer: None — dangerous commands bypass peer"
        );

        let result = RunCommand
            .call(
                json!({ "command": "rm -rf /tmp/test_elicitation_placeholder" }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(
            result["pending_ack"].is_string(),
            "dangerous command without peer must return pending_ack handle, got: {result}"
        );
        assert!(
            result["reason"].is_string(),
            "response must include a reason, got: {result}"
        );
    }

    #[test]
    fn parse_timeout_input_both_keys_secs_large() {
        // timeout_secs wins and triggers conversion hint; timeout is ignored
        let input = serde_json::json!({ "timeout_secs": 120_000u64, "timeout": 5000 });
        let (secs, hint) = parse_timeout_input(&input);
        assert_eq!(secs, 120);
        assert!(hint.is_some());
    }

    #[tokio::test]
    async fn onboarding_triggers_refresh_when_version_stale() {
        let dir = tempdir().unwrap();
        let config_dir = dir.path().join(".codescout");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

        let config = crate::config::project::ProjectConfig {
            project: crate::config::project::ProjectSection {
                name: "test".into(),
                languages: vec!["rust".into()],
                encoding: "utf-8".into(),
                system_prompt: None,
                tool_timeout_secs: 60,
                onboarding_version: None, // pre-versioning → stale
            },
            embeddings: Default::default(),
            ignored_paths: Default::default(),
            security: Default::default(),
            memory: Default::default(),
            libraries: Default::default(),
            lsp: Default::default(),
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        std::fs::write(config_dir.join("project.toml"), &toml_str).unwrap();

        let mem_dir = config_dir.join("memories");
        std::fs::create_dir_all(&mem_dir).unwrap();
        std::fs::write(mem_dir.join("onboarding.md"), "Languages: rust").unwrap();

        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };

        let result = Onboarding.call(json!({}), &ctx).await.unwrap();

        assert!(
            result.get("subagent_prompt").is_some(),
            "stale version must trigger refresh"
        );
        assert_eq!(result["version_stale"].as_bool(), Some(true));
        let prompt = result["subagent_prompt"].as_str().unwrap();
        assert!(
            prompt.contains("Do NOT re-explore"),
            "must be lightweight refresh"
        );
    }

    #[tokio::test]
    async fn onboarding_fast_path_when_version_current() {
        let dir = tempdir().unwrap();
        let config_dir = dir.path().join(".codescout");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

        let config = crate::config::project::ProjectConfig {
            project: crate::config::project::ProjectSection {
                name: "test".into(),
                languages: vec!["rust".into()],
                encoding: "utf-8".into(),
                system_prompt: None,
                tool_timeout_secs: 60,
                onboarding_version: Some(ONBOARDING_VERSION),
            },
            embeddings: Default::default(),
            ignored_paths: Default::default(),
            security: Default::default(),
            memory: Default::default(),
            libraries: Default::default(),
            lsp: Default::default(),
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        std::fs::write(config_dir.join("project.toml"), &toml_str).unwrap();

        let mem_dir = config_dir.join("memories");
        std::fs::create_dir_all(&mem_dir).unwrap();
        std::fs::write(mem_dir.join("onboarding.md"), "Languages: rust").unwrap();

        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };

        let result = Onboarding.call(json!({}), &ctx).await.unwrap();

        assert_eq!(result["onboarded"].as_bool(), Some(true));
        assert!(
            result.get("subagent_prompt").is_none(),
            "current version must not trigger refresh"
        );
    }
}
