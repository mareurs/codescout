//! Per-project configuration loaded from `.codescout/project.toml`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Owed by every refusal whose remedy is an edit to a `[security]` key of
/// `.codescout/project.toml`.
///
/// Two facts the remedy has to carry. The file is read again when it changes
/// (`ActiveProject::config_is_stale`), so an edit made in an editor reaches the next tool
/// call; this used to say the config was cached for the life of the process, which stopped
/// being true on 2026-09-14
/// (`docs/issues/archive/2026-09-14-an-out-of-band-project-toml-edit-never-invalidates-the-cached-config.md`).
/// And codescout's own write tools REFUSE an edit to `[security]`
/// (`config::protected_files`): the remedy this text used to prescribe, `edit_file`, is now a
/// refused action for exactly the keys these refusals are about, so naming it would send the
/// reader to a call that fails. The step that is theirs to take is an editor.
///
/// **The `file_write_enabled` refusal does not use this** — it has its own text in
/// `util::path_security`.
pub const CONFIG_IS_CACHED_REMEDY: &str = "That file is read again when it changes, but \
     codescout's own write tools cannot edit its [security] section (a session that could would \
     be able to raise its own privileges): edit it yourself in an editor, or ask the user to. \
     The next tool call picks the edit up; restarting the MCP server (`/mcp`) always does.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project: ProjectSection,
    #[serde(default)]
    pub embeddings: EmbeddingsSection,
    #[serde(default)]
    pub ignored_paths: IgnoredPathsSection,
    #[serde(default)]
    pub security: SecuritySection,
    #[serde(default)]
    pub memory: MemorySection,
    #[serde(default)]
    pub libraries: LibrariesSection,
    #[serde(default)]
    pub lsp: LspSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSection {
    pub name: String,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default = "default_encoding")]
    pub encoding: String,
    #[serde(default = "default_timeout")]
    pub tool_timeout_secs: u64,
    /// Deprecated: use `.codescout/system-prompt.md` instead.
    /// This field is still read as a fallback if the file doesn't exist.
    #[serde(default)]
    pub system_prompt: Option<String>,
    /// The ONBOARDING_VERSION a system-prompt regeneration was last *stamped* for.
    /// `None` means pre-versioning — treated as stale.
    ///
    /// This records a **completed** regeneration, witnessed via
    /// `system_prompt_sha256` below. It used to be written when a refresh was
    /// merely requested, which certified work that had not happened yet — see
    /// `docs/issues/archive/2026-09-04-refresh-prompt-stamps-the-version-before-the-work.md`.
    #[serde(default)]
    pub onboarding_version: Option<u32>,
    /// SHA-256 of `.codescout/system-prompt.md` as it stood when a refresh was last
    /// requested — the content the pending regeneration is expected to supersede.
    /// A hash that has since moved is positive evidence the regeneration happened,
    /// which is the only signal available: nothing in this process writes that file,
    /// so a subagent does it later, or never. An absent file hashes as empty content
    /// so a project whose prompt is not yet written still records a baseline.
    ///
    /// `None` means no request has ever been recorded on this machine, and is
    /// deliberately NOT read as a difference. `project.toml` is gitignored while
    /// `system-prompt.md` is tracked, so `None` beside a present file is exactly the
    /// state a fresh clone arrives in — treating that as a witnessed regeneration
    /// would report another machine's stale prompt as current.
    #[serde(default)]
    pub system_prompt_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsSection {
    /// Model identifier — the prefix selects the backend:
    ///   "local:<variant>"      → fastembed ONNX in-process, no daemon
    ///   "local-dir:<path>"     → ONNX weights from disk, never the network
    ///   "ollama:<model>"       → Ollama daemon at $OLLAMA_HOST
    ///   "openai:<model>"       → OpenAI API (`api_key` or OPENAI_API_KEY)
    ///   bare name              → sent verbatim as the model name when `url` is set
    ///
    /// The `local:` variants, from `crates/codescout-embed/src/local.rs`:
    ///   "local:AllMiniLML6V2Q"           → 384d, INT8, ~22MB — the built-in default
    ///   "local:AllMiniLML6V2"            → 384d, full precision
    ///   "local:BGESmallENV15"            → 384d, full precision
    ///   "local:NomicEmbedTextV15Q"       → 768d, INT8, ~158MB
    ///   "local:NomicEmbedTextV15"        → 768d, full precision, ~547MB
    ///   "local:JinaEmbeddingsV2BaseCode" → 768d, code-specific, ~300MB
    ///
    /// (local:BGESmallENV15Q still parses but is GPU-only and crashes on CPU;
    /// `parse_model`'s own error text says so while still accepting it.)
    ///
    /// **`None` means "not configured at this level"**, and that is the whole
    /// reason it is an `Option`. This struct is deserialised at BOTH config
    /// levels, so a global `~/.config/codescout/config.toml` setting only `url`
    /// must not also assert a model it never mentioned — with a `serde(default)`
    /// of the built-in model it would, and the global layer would silently pin
    /// every project to `local:AllMiniLML6V2Q`. The default is applied once at
    /// RESOLUTION ([`Self::model_or_default`], `merge_embed_config`), never at
    /// deserialisation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Base URL for an OpenAI-compatible embedding endpoint.
    ///
    /// When set, the `model` field is sent as the model name in the request body.
    /// The URL should point to the API base (e.g., `http://127.0.0.1:43300/v1`).
    /// Works with llama.cpp, vLLM, TEI, Ollama, OpenAI, and any server implementing
    /// `POST /v1/embeddings`.
    ///
    /// When absent, the `model` field's prefix determines the backend
    /// (`local:`, `ollama:`, `openai:`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// API key for the embedding endpoint. Only used when `url` is set.
    /// Can also be provided via the `EMBED_API_KEY` environment variable.
    ///
    /// Stored as `SensitiveString` so a stray `tracing::debug!(?config)` or
    /// any other diagnostic dump of the config hides the key rather than
    /// writing it to logs/stdout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<crate::config::sensitive::SensitiveString>,
    /// **Ignored** — kept for backwards-compatible deserialisation of existing
    /// `project.toml` files that set a `chunk_size` key. Same treatment as
    /// `chunk_overlap` below, for a different reason.
    ///
    /// It was read by one function, `EmbeddingsSection::effective_chunk_size`, which
    /// no production code ever called: real indexing takes `STACK_CHUNK_TARGET`
    /// (`src/retrieval/sync.rs`, overridable via `CODESCOUT_CHUNK_TARGET`) straight to
    /// `split_file`. That function was removed on 2026-08-14 rather than wired up,
    /// because the model-derived budget it computed exceeds what the local embedder
    /// will actually tokenize — see
    /// `docs/issues/archive/2026-08-11-chunk-size-for-model-dead-on-production-path.md`.
    ///
    /// **To change chunk size, set `CODESCOUT_CHUNK_TARGET`.** It is the only live
    /// knob. This field is retained solely so an existing `project.toml` carrying it
    /// still deserialises; deleting it would turn a silently-ignored key into a hard
    /// parse error on upgrade.
    #[serde(default, skip_serializing, rename = "chunk_size")]
    pub _chunk_size_ignored: Option<usize>,
    /// Ignored — kept for backwards-compatible deserialisation of existing
    /// `project.toml` files that include a `chunk_overlap` key.
    ///
    /// Overlap is meaningless for AST-aware chunking (clean semantic boundaries)
    /// and was removed from the public API. The plain-text fallback paths also
    /// use 0 overlap so each sub-chunk is distinct.
    #[serde(default, skip_serializing, rename = "chunk_overlap")]
    pub _chunk_overlap_ignored: Option<usize>,
    /// Max concurrent in-flight embedding requests during `index_project`.
    ///
    /// Defaults to 8 (see `DEFAULT_MAX_INFLIGHT`). Bump this when using a
    /// remote GPU embedding backend (llama-server, TEI, vLLM) — the GPU can
    /// handle far more than 8 parallel batches, and higher inflight keeps it
    /// saturated while codescout writes the previous group to SQLite.
    ///
    /// For local CPU backends (`local:`, default), keep at 8 or lower — each
    /// embed call already uses all cores, so more inflight just queues work.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_inflight: Option<usize>,
    /// Number of files processed per indexing group (embed → scatter → write).
    ///
    /// Defaults to 50 (see `DEFAULT_FILE_GROUP_SIZE`). Larger groups reduce
    /// the number of DB-write stalls per full reindex but raise peak RAM to
    /// O(group × chunks). On a remote-GPU setup, 200–500 is often better.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_group_size: Option<usize>,
}

/// `[ignored_paths]` in `project.toml`.
///
/// **`Default` is hand-written, not derived, and that is load-bearing.** The serde
/// field default on `patterns` (`default_ignored_patterns`) only fires when the
/// `[ignored_paths]` table is *present* without a `patterns` key. An *absent* table
/// takes `Default::default()` via `ProjectConfig`'s `#[serde(default)]`, and the derived
/// impl yields `Vec::new()` — so a project with no `[ignored_paths]` section silently
/// got no default ignores, while one with an empty section got them. The design's
/// "key absent → defaults apply" has to hold on both paths, so both route through the
/// same function. An explicit `patterns = []` still means "ignore nothing".
/// `docs/issues/archive/2026-10-16-default-ignore-patterns-skip-a-project-with-no-ignored-paths-section.md`.
///
/// **Whether `patterns` was written down is remembered** (`patterns_explicit`), because
/// the write-back must not change what the file means. Onboarding and the version
/// stamps re-serialise a whole `ProjectConfig` over `project.toml`. Writing the default
/// list out would freeze today's defaults into every such file, so a later addition could
/// never reach it — but omitting the key *whenever the list equals the defaults* is wrong
/// too: with a global `patterns = ["generated"]`, a project that explicitly lists the
/// defaults would lose the key and the global list would fill the gap on the next load.
/// The key is therefore omitted only when it was absent at load **and** the list is still
/// the defaults; an explicit list (including `[]`) and any in-memory edit are written.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "IgnoredPathsRaw", into = "IgnoredPathsRaw")]
pub struct IgnoredPathsSection {
    pub patterns: Vec<String>,
    /// Glob patterns that the librarian walker visits even when
    /// `.gitignore`/`.git/info/exclude`/a global excludesfile would otherwise
    /// skip them — e.g. a directory tracked only on a local/private branch
    /// and deliberately excluded from whatever branch the repo publishes.
    /// Read directly from `.codescout/project.toml` by
    /// `crate::librarian::indexer::read_force_include`, not threaded through
    /// this struct at runtime (librarian's `ToolContext` doesn't carry the
    /// main server's parsed config) — this field exists for schema
    /// documentation/validation, not as the live read path.
    pub force_include: Vec<String>,
    /// True when `patterns` was present in the merged TOML this was loaded from. Not a
    /// config key; see the type's doc for why the write-back needs it.
    patterns_explicit: bool,
}

/// The on-disk shape of [`IgnoredPathsSection`]: `patterns` is an `Option` so absent and
/// empty stay distinguishable through a load.
#[derive(Serialize, Deserialize)]
struct IgnoredPathsRaw {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    patterns: Option<Vec<String>>,
    #[serde(default)]
    force_include: Vec<String>,
}

impl From<IgnoredPathsRaw> for IgnoredPathsSection {
    fn from(raw: IgnoredPathsRaw) -> Self {
        Self {
            patterns_explicit: raw.patterns.is_some(),
            patterns: raw.patterns.unwrap_or_else(default_ignored_patterns),
            force_include: raw.force_include,
        }
    }
}

impl From<IgnoredPathsSection> for IgnoredPathsRaw {
    fn from(s: IgnoredPathsSection) -> Self {
        let implicit = !s.patterns_explicit && s.patterns == default_ignored_patterns();
        Self {
            patterns: (!implicit).then_some(s.patterns),
            force_include: s.force_include,
        }
    }
}

impl Default for IgnoredPathsSection {
    fn default() -> Self {
        Self {
            patterns: default_ignored_patterns(),
            force_include: Vec::new(),
            patterns_explicit: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecuritySection {
    /// Security profile: "default" (sandboxed) or "root" (unrestricted).
    #[serde(default)]
    pub profile: crate::util::path_security::SecurityProfile,
    /// Additional directories where writes are allowed (beyond project root).
    #[serde(default)]
    pub extra_write_roots: Vec<String>,
    /// Shell command mode: "unrestricted", "warn" (default), "disabled"
    #[serde(default = "default_shell_mode")]
    pub shell_command_mode: String,
    /// Enable file write tools: create_file, edit_file, symbol write tools (default: true)
    #[serde(default = "default_true")]
    pub file_write_enabled: bool,
    /// Enable semantic search and indexing tools (default: true)
    #[serde(default = "default_true")]
    pub indexing_enabled: bool,
    /// Additional regex patterns to flag as dangerous commands.
    #[serde(default)]
    pub shell_dangerous_patterns: Vec<String>,
    /// Seconds to wait for the cross-process write lock before returning a
    /// RecoverableError. Default: 5.
    #[serde(default = "default_write_lock_timeout")]
    pub write_lock_timeout_secs: u64,
    /// Approximate raw source-byte threshold above which `index_project` requires
    /// user confirmation via MCP elicitation. Default: 500 MB.
    #[serde(default = "default_max_index_bytes")]
    pub max_index_bytes: u64,
}

impl Default for SecuritySection {
    fn default() -> Self {
        Self {
            profile: crate::util::path_security::SecurityProfile::Default,
            extra_write_roots: Vec::new(),
            shell_command_mode: default_shell_mode(),
            file_write_enabled: true,
            indexing_enabled: true,
            shell_dangerous_patterns: Vec::new(),
            write_lock_timeout_secs: 5,
            max_index_bytes: default_max_index_bytes(),
        }
    }
}

fn default_shell_mode() -> String {
    "warn".into()
}

fn default_true() -> bool {
    true
}

fn default_write_lock_timeout() -> u64 {
    5
}

fn default_max_index_bytes() -> u64 {
    500 * 1024 * 1024
}

impl SecuritySection {
    pub fn to_path_security_config(&self) -> crate::util::path_security::PathSecurityConfig {
        crate::util::path_security::PathSecurityConfig {
            profile: self.profile,
            extra_write_roots: self
                .extra_write_roots
                .iter()
                .map(std::path::PathBuf::from)
                .collect(),
            shell_command_mode: self.shell_command_mode.clone(),
            file_write_enabled: self.file_write_enabled,
            indexing_enabled: self.indexing_enabled,
            library_paths: Vec::new(),
            shell_dangerous_patterns: self.shell_dangerous_patterns.clone(),
            max_index_bytes: self.max_index_bytes,
            // No project root is in scope here, so there is nothing to name.
            // `project_security_config` fills this in where `p.root` exists;
            // leaving it None makes the refusal fall back to its unhedged
            // original wording rather than assert a cause it cannot attribute.
            write_block: None,
        }
    }
}

/// Strictness of the `shell_command_mode` values the dispatcher knows
/// (`run_command_inner`): higher is stricter. `None` for any other string, which that
/// dispatcher refuses outright.
fn shell_mode_rank(mode: &str) -> Option<u8> {
    match mode {
        "unrestricted" => Some(0),
        "warn" => Some(1),
        "disabled" => Some(2),
        _ => None,
    }
}

/// What the GLOBAL config says about the three keys a project's absence inherits from it, as
/// read for a failed project reload. `Unreadable` is its own state, not `Absent`: a global
/// config that cannot be read cannot be shown to be permissive.
pub(crate) enum GlobalSecurity {
    Absent,
    Set(crate::config::global::GlobalSecuritySection),
    Unreadable,
}

impl GlobalSecurity {
    /// Read the global config on its own, apart from the project file that failed.
    pub(crate) fn load() -> Self {
        match crate::config::global::GlobalConfig::load() {
            Ok(Some(g)) => Self::Set(g.security),
            Ok(None) => Self::Absent,
            Err(_) => Self::Unreadable,
        }
    }
}

/// `*field` after a failed reload, for a boolean that is `true` when the gate is OPEN. The
/// value the file means: its own when set (a non-boolean cannot be read as open, so counts as
/// closed); else the global config's, else the built-in default (open); a global config that
/// cannot be read counts as closed. The gate becomes the stricter of that and what it was.
fn tighten_gate(
    field: &mut bool,
    raw: Option<&toml::Value>,
    global: &GlobalSecurity,
    from_global: fn(&crate::config::global::GlobalSecuritySection) -> Option<bool>,
) {
    let wanted = match raw {
        Some(toml::Value::Boolean(open)) => *open,
        Some(_) => false,
        None => match global {
            GlobalSecurity::Absent => true,
            GlobalSecurity::Set(g) => from_global(g).unwrap_or(true),
            GlobalSecurity::Unreadable => false,
        },
    };
    *field = *field && wanted;
}

impl SecuritySection {
    /// The security policy while `project.toml` FAILS TO LOAD: the stricter of the previous
    /// policy and what the file says, never the previous policy alone.
    ///
    /// Keeping the last good config whole (the earlier behaviour) is looser than the file in
    /// exactly the case that matters: a tightening saved in the same edit as an entry the
    /// loader refuses never took effect, and nothing said so. A value is read from `file`
    /// only to TIGHTEN, so a failed reload can never loosen anything:
    ///
    /// - `file_write_enabled`, `indexing_enabled`: `false` closes the gate; a value that is
    ///   not a boolean closes it too. **Absent means what the file would mean without the
    ///   invalid entry**: the global config's value if it sets one, else the built-in
    ///   default, and a global config that cannot be read counts as the strictest value.
    ///   The gate becomes the stricter of that and what it was (so deleting a looser key in
    ///   the same save does not keep it).
    /// - `shell_command_mode`: the same rule; a string that is not a known mode, or a
    ///   non-string, counts as `disabled`.
    /// - `profile` and `extra_write_roots` are confirmed or dropped: `Root` stays only if
    ///   the file still says `profile = "root"`, and a write root stays only if the file
    ///   still lists that exact entry. Absent or unreadable confirms nothing, because for
    ///   these two keys absent means the default (sandboxed, no extra roots) and the global
    ///   config cannot set either. A root the file ADDED is not adopted.
    ///
    /// `file` is `None` when the file could not be read as TOML at all (syntax error,
    /// unreadable, over the size cap). Nothing in it can then be confirmed, so it is treated
    /// as an empty file AND file writes are disabled: the one gate whose silent survival
    /// the user cannot see.
    ///
    /// Not covered, by design: `shell_dangerous_patterns` added by the file and a lowered
    /// `max_index_bytes`. (A tightening that only the global config carries is covered, via
    /// `global`.)
    pub(crate) fn tighten_for_unloadable_file(
        &mut self,
        file: Option<&toml::Table>,
        global: &GlobalSecurity,
    ) {
        let empty = toml::Table::new();
        if file.is_none() {
            self.file_write_enabled = false;
        }
        let security = file
            .and_then(|f| f.get("security"))
            .and_then(toml::Value::as_table)
            .unwrap_or(&empty);

        tighten_gate(
            &mut self.file_write_enabled,
            security.get("file_write_enabled"),
            global,
            |g| g.file_write_enabled,
        );
        tighten_gate(
            &mut self.indexing_enabled,
            security.get("indexing_enabled"),
            global,
            |g| g.indexing_enabled,
        );

        // The mode the file means, as a rank: its own when set (anything unknown or not a
        // string is the strictest), else the global config's, else the default `warn`; an
        // unreadable global config is the strictest. A stricter rank replaces the current one.
        let wanted_rank = match security.get("shell_command_mode") {
            Some(toml::Value::String(wanted)) => shell_mode_rank(wanted).unwrap_or(2),
            Some(_) => 2,
            None => match global {
                GlobalSecurity::Absent => 1,
                GlobalSecurity::Set(g) => g
                    .shell_command_mode
                    .as_deref()
                    .map_or(1, |m| shell_mode_rank(m).unwrap_or(2)),
                GlobalSecurity::Unreadable => 2,
            },
        };
        let current_rank = shell_mode_rank(&self.shell_command_mode).unwrap_or(2);
        if wanted_rank > current_rank {
            self.shell_command_mode = match wanted_rank {
                1 => "warn",
                _ => "disabled",
            }
            .into();
        }

        let still_root = matches!(
            security.get("profile"),
            Some(toml::Value::String(p)) if p == "root"
        );
        if !still_root {
            self.profile = crate::util::path_security::SecurityProfile::Default;
        }

        match security.get("extra_write_roots") {
            Some(toml::Value::Array(listed)) => self
                .extra_write_roots
                .retain(|root| listed.iter().any(|v| v.as_str() == Some(root.as_str()))),
            _ => self.extra_write_roots.clear(),
        }
    }
}

impl ProjectConfig {
    /// `project.toml` as a bare TOML table, for the failed-reload salvage in
    /// [`SecuritySection::tighten_for_unloadable_file`]. A missing file is an empty table;
    /// `None` is a file that exists but cannot be read, is over the same 1 MiB cap the
    /// loader enforces, or is not TOML.
    pub(crate) fn read_project_toml_table(root: &Path) -> Option<toml::Table> {
        let path = root.join(".codescout").join("project.toml");
        let metadata = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Some(toml::Table::new()),
            Err(_) => return None,
        };
        if metadata.len() > 1024 * 1024 {
            return None;
        }
        toml::from_str(&std::fs::read_to_string(&path).ok()?).ok()
    }
}

/// A toml error as message, line and column, **never the source excerpt**.
///
/// `toml::de::Error`'s `Display` prints the offending source line. `project.toml` can be a link
/// to any readable file, so that line can be a secret from a file the read deny-list protects
/// (`~/.pgpass`), and the error reaches the caller (the failed-reload notice, the activation
/// refusal, the log). `text` is the file's content, for the line number; pass `None` when the
/// error comes from deserialising an already-parsed value (it has no span). Quoted values in the
/// message (`invalid type: string "..."`, `unknown variant `...``) are redacted by
/// [`redact_values`].
pub(crate) fn describe_toml_error(text: Option<&str>, e: &toml::de::Error) -> String {
    let message = redact_values(e.message());
    match (text, e.span()) {
        (Some(text), Some(span)) if span.start <= text.len() => {
            let before = &text.as_bytes()[..span.start];
            let line = before.iter().filter(|b| **b == b'\n').count() + 1;
            let col = before.iter().rev().take_while(|b| **b != b'\n').count() + 1;
            format!("{message} (line {line}, column {col})")
        }
        _ => message,
    }
}

/// Replace what a serde/toml message quotes from the INPUT with `…`: every double-quoted
/// segment, and the backticked name after `unknown variant` / `unknown field` (the other
/// backticked names are our own field names and stay: `missing field `project``).
fn redact_values(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut rest = message;
    while let Some(i) = rest.find(['"', '`']) {
        let quote = rest.as_bytes()[i] as char;
        let (head, tail) = rest.split_at(i + 1);
        out.push_str(head);
        let Some(end) = tail.find(quote) else {
            out.push_str(tail);
            return out;
        };
        let redact = quote == '"'
            || head.trim_end_matches('`').ends_with("unknown variant ")
            || head.trim_end_matches('`').ends_with("unknown field ");
        out.push_str(if redact { "…" } else { &tail[..end] });
        out.push(quote);
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

/// What every response says while `project.toml` fails to load. See
/// `ActiveProject::config_load_error`; the error text is the loader's own, which names the
/// entry.
pub fn failed_reload_notice(error: &str) -> String {
    format!(
        "`.codescout/project.toml` FAILED TO LOAD, so none of your last edit is applied: \
         {error}. The previous settings stay in force; from the file only a security \
         TIGHTENING is applied (file_write_enabled/indexing_enabled = false, a stricter \
         shell_command_mode, profile other than \"root\", fewer extra_write_roots), and a \
         file that is not valid TOML disables file writes. Fix the file in an editor; the \
         next save is re-read."
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySection {
    /// Min drift score to trigger reverse-drift staleness flag (0.0-1.0)
    #[serde(default = "default_staleness_drift_threshold")]
    pub staleness_drift_threshold: f32,
    /// Min similarity for semantic anchor creation (0.0-1.0)
    #[serde(default = "default_semantic_anchor_min_similarity")]
    pub semantic_anchor_min_similarity: f32,
    /// Number of top chunks to consider for semantic anchoring
    #[serde(default = "default_semantic_anchor_top_n")]
    pub semantic_anchor_top_n: usize,
    /// Memory topics protected from blind overwrite during force re-onboarding.
    /// Protected topics go through a staleness-check + merge + user-approval flow.
    #[serde(default = "default_protected_topics")]
    pub protected: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibrariesSection {
    #[serde(default)]
    pub auto_index: bool,
    #[serde(default)]
    pub auto_fetch_sources: bool,
    #[serde(default = "default_fetch_timeout")]
    pub fetch_timeout_secs: u64,
    #[serde(default)]
    pub version_overrides: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LspSection {
    /// Per-language overrides, keyed by language name ("rust", "java", ...).
    #[serde(flatten)]
    pub langs: std::collections::HashMap<String, LspLangOverride>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LspLangOverride {
    /// Force `mux: false` (direct-process) or `mux: true` (multiplexer).
    /// `None` means "use the built-in default from servers::default_config".
    #[serde(default)]
    pub mux: Option<bool>,
}

fn default_fetch_timeout() -> u64 {
    300
}

impl Default for LibrariesSection {
    fn default() -> Self {
        Self {
            auto_index: false,
            auto_fetch_sources: false,
            fetch_timeout_secs: default_fetch_timeout(),
            version_overrides: std::collections::HashMap::new(),
        }
    }
}

fn default_staleness_drift_threshold() -> f32 {
    0.3
}
fn default_semantic_anchor_min_similarity() -> f32 {
    // Qdrant RRF uses 1/(1+rank): rank-1=0.500, rank-2=0.333, ..., rank-9≈0.100.
    // 0.1 admits roughly the top 9 ranks, which after per-file dedupe yields a
    // sensible anchor budget for typical memory writes.
    0.1
}
fn default_semantic_anchor_top_n() -> usize {
    10
}
fn default_protected_topics() -> Vec<String> {
    vec!["gotchas".to_string()]
}

impl Default for MemorySection {
    fn default() -> Self {
        Self {
            staleness_drift_threshold: default_staleness_drift_threshold(),
            semantic_anchor_min_similarity: default_semantic_anchor_min_similarity(),
            semantic_anchor_top_n: default_semantic_anchor_top_n(),
            protected: default_protected_topics(),
        }
    }
}

impl Default for EmbeddingsSection {
    /// Every field is `None` — "nothing configured at this level".
    ///
    /// `model` used to default to `default_embed_model()` here, which was
    /// correct while this struct existed only at the project level and wrong the
    /// moment it also became the global one: `serde(default)` fires for an
    /// ABSENT `[embeddings]` table, so a global config that never mentioned a
    /// model would have serialised one into the merge base and pinned every
    /// project to it. The default now lives at resolution
    /// ([`Self::model_or_default`], `merge_embed_config`), which is the only
    /// layer that can see every level at once.
    fn default() -> Self {
        Self {
            model: None,
            url: None,
            api_key: None,
            _chunk_size_ignored: None,
            _chunk_overlap_ignored: None,
            max_inflight: None,
            file_group_size: None,
        }
    }
}

impl EmbeddingsSection {
    /// The configured model, or the built-in default when no level set one.
    ///
    /// The single place the `[embeddings]`-side default is applied. Callers that
    /// want "what model is this project using?" ask here rather than reading the
    /// field, so an unset value cannot be mistaken for a configured one.
    ///
    /// **This is not the same question as "what goes on the wire".** When a url
    /// is configured, the name sent to the server is
    /// `RetrievalConfig::dense_model_name()`, which also applies the operator's
    /// override and strips the routing prefix. Reading this field to size a
    /// request or label a backend is how the two copies drift — `src/main.rs`
    /// carries a comment about exactly that hazard.
    pub fn model_or_default(&self) -> String {
        self.model
            .clone()
            .filter(|m| !m.trim().is_empty())
            .unwrap_or_else(default_embed_model)
    }

    /// Resolve the concurrent in-flight embedding request limit for indexing.
    /// Defaults to 8. See `max_inflight` doc for tuning guidance.
    pub fn effective_max_inflight(&self) -> usize {
        // cap-class: NOT_A_CAP — concurrency ceiling for in-flight embedding requests; it paces the work, every chunk is still indexed
        const DEFAULT_MAX_INFLIGHT: usize = 8;
        self.max_inflight
            .filter(|&n| n > 0)
            .unwrap_or(DEFAULT_MAX_INFLIGHT)
    }

    /// Resolve the per-group file count for indexing. Defaults to 50.
    /// See `file_group_size` doc for tuning guidance.
    pub fn effective_file_group_size(&self) -> usize {
        const DEFAULT_FILE_GROUP_SIZE: usize = 50;
        self.file_group_size
            .filter(|&n| n > 0)
            .unwrap_or(DEFAULT_FILE_GROUP_SIZE)
    }
}

fn default_encoding() -> String {
    "utf-8".into()
}
fn default_timeout() -> u64 {
    60
}
pub fn default_embed_model() -> String {
    "local:AllMiniLML6V2Q".into()
}

/// Directories excluded from indexing by default.
///
/// These are compiled with **gitignore semantics** for the code index
/// (`build_ignore_matcher`), so a bare name matches at ANY depth — `target`
/// prunes `crates/foo/target` too. That reach is what makes the selection
/// criterion strict:
///
/// > **A default may only name a directory that cannot plausibly contain
/// > hand-authored source.**
///
/// Deliberately NOT included, despite being proposed in
/// `docs/trackers/index-scope-default-ignores.md`:
/// - `models` — a *very* common source directory (Django/Rails/domain models).
///   At any depth it would silently prune `src/models/`, which is exactly the
///   "default-on risks silently skipping code a user wants indexed" failure the
///   tracker warns about.
/// - `vendor`, `bin`, `obj`, `out` — conventionally build output or vendored
///   deps, but each is a legitimate source directory in some ecosystems. Left to
///   per-project `[ignored_paths]`.
///
/// Binary weights (`*.pt`, `*.safetensors`, `*.onnx`, `*.gguf`) are NOT listed:
/// they are already excluded from *embedding* by the `lang_for_ext` extension
/// allowlist. Adding them here would only save walk time, not memory, and this
/// list is directory-oriented.
fn default_ignored_patterns() -> Vec<String> {
    [
        // VCS / codescout's own state
        ".git",
        ".codescout",
        ".worktrees",
        ".claude",
        // Build output / dependency trees (unambiguous)
        "node_modules",
        "target",
        "dist",
        "build",
        // Python: venvs, vendored deps, tool caches
        ".venv",
        "venv",
        "site-packages",
        "__pycache__",
        ".mypy_cache",
        ".pytest_cache",
        ".ruff_cache",
        ".tox",
        ".nox",
        // JS/TS build + package-manager caches
        ".next",
        ".nuxt",
        ".svelte-kit",
        ".turbo",
        ".parcel-cache",
        // JVM / Gradle
        ".gradle",
        // Other ecosystems' caches
        ".terraform",
        ".dart_tool",
        ".stack-work",
        // Environment / editor state
        ".direnv",
        ".idea",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect()
}

#[allow(dead_code)]
fn merge_toml(base: toml::Value, overlay: toml::Value) -> toml::Value {
    match (base, overlay) {
        (toml::Value::Table(mut base_map), toml::Value::Table(overlay_map)) => {
            for (k, v) in overlay_map {
                let merged = if let Some(base_val) = base_map.remove(&k) {
                    merge_toml(base_val, v)
                } else {
                    v
                };
                base_map.insert(k, merged);
            }
            toml::Value::Table(base_map)
        }
        (_, overlay) => overlay,
    }
}

impl ProjectConfig {
    /// Apply the highest-precedence embedding overrides to an already-resolved config.
    ///
    /// Pure — takes the values rather than reading `CODESCOUT_EMBED_*` itself, so the
    /// precedence rule is testable without `set_var`. See
    /// `docs/issues/archive/2026-07-13-test-env-access-ub-nonserial-writers-race-build-tool-context.md`.
    ///
    /// A `Some` override wins; a `None` leaves the configured value alone. Note
    /// that `None` here means "the variable was unset", never "clear the field" —
    /// the two are distinct and conflating them would let an absent env var erase
    /// a project's config.
    pub(crate) fn apply_embed_overrides(&mut self, model: Option<String>, url: Option<String>) {
        if let Some(model) = model {
            self.embeddings.model = Some(model);
        }
        if let Some(url) = url {
            self.embeddings.url = Some(url);
        }
    }

    /// Load from `.codescout/project.toml`, or return a sensible default
    /// derived from the directory name.  Global config (~/.config/codescout/config.toml)
    /// is loaded first as the base layer; project.toml is merged on top.
    ///
    /// Env var overrides (highest precedence, override everything):
    ///   `CODESCOUT_EMBED_MODEL` — embedding model identifier
    ///   `CODESCOUT_EMBED_URL`   — embedding endpoint URL
    pub fn load_or_default(root: &Path) -> Result<Self> {
        use crate::config::global::GlobalConfig;

        let global_base: toml::Value = GlobalConfig::load()?
            .map(|g| g.to_toml_value())
            .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));

        let mut config = Self::load_with_global_base(root, global_base)?;
        config.apply_embed_overrides(
            crate::config::embedding_env::read(&crate::config::embedding_env::MODEL),
            crate::config::embedding_env::read(&crate::config::embedding_env::URL),
        );
        Ok(config)
    }

    /// Inner implementation — accepts an already-resolved global base so tests can
    /// inject it directly without touching environment variables.
    pub(crate) fn load_with_global_base(root: &Path, global_base: toml::Value) -> Result<Self> {
        // Every project's config goes through here, so a project cannot become resident
        // without its root reaching the write guard's identity check. Registering is
        // widening-only: see `protected_files::PROJECT_ROOTS`.
        crate::config::protected_files::register_project_root(root);
        let config_path = root.join(".codescout").join("project.toml");

        let project_overlay: toml::Value = if config_path.exists() {
            let metadata = std::fs::metadata(&config_path)?;
            if metadata.len() > 1024 * 1024 {
                anyhow::bail!(
                    "project.toml exceeds 1 MiB limit ({} bytes)",
                    metadata.len()
                );
            }
            let text = std::fs::read_to_string(&config_path)?;
            toml::from_str(&text).map_err(|e| {
                anyhow::anyhow!(
                    "parsing {}: {}",
                    config_path.display(),
                    describe_toml_error(Some(&text), &e)
                )
            })?
        } else {
            let name = root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unnamed")
                .to_string();
            let mut project_map = toml::map::Map::new();
            project_map.insert("name".to_string(), toml::Value::String(name));
            let mut root_map = toml::map::Map::new();
            root_map.insert("project".to_string(), toml::Value::Table(project_map));
            toml::Value::Table(root_map)
        };

        let merged = merge_toml(global_base, project_overlay);
        let config: Self = toml::Value::try_into(merged).map_err(|e| {
            anyhow::anyhow!(
                "invalid configuration for {}: {}",
                config_path.display(),
                describe_toml_error(None, &e)
            )
        })?;
        // Name the file: the merged config may also carry the global config, and a reader with
        // several projects cannot tell which one stopped.
        config.validate_list_limits().with_context(|| {
            format!(
                "invalid configuration for {} (merged with the global config)",
                config_path.display()
            )
        })?;
        Ok(config)
    }

    /// Enforce upper bounds on user-controlled list fields. The 1 MiB file cap
    /// already limits total size, but 1 MiB of TOML easily encodes tens of
    /// thousands of regex patterns — each evaluated on every shell invocation,
    /// every path check, every ignore walk. Cap the counts to keep a crafted
    /// or copy-pasted manifest from DoS'ing per-request hot paths. Also refuses an
    /// `extra_write_roots` entry that is not an absolute path.
    fn validate_list_limits(&self) -> Result<()> {
        // cap-class: NOT_A_CAP — config validation ceiling; a breach bails with an error naming the count, no entry is silently dropped
        const MAX_EXTRA_WRITE_ROOTS: usize = 128;
        // cap-class: NOT_A_CAP — config validation ceiling; a breach bails with an error naming the count, no entry is silently dropped
        const MAX_SHELL_DANGEROUS_PATTERNS: usize = 256;
        // cap-class: NOT_A_CAP — config validation ceiling; a breach bails with an error naming the count, no entry is silently dropped
        const MAX_IGNORED_PATTERNS: usize = 1024;

        if self.security.extra_write_roots.len() > MAX_EXTRA_WRITE_ROOTS {
            anyhow::bail!(
                "security.extra_write_roots has {} entries, exceeds limit of {}",
                self.security.extra_write_roots.len(),
                MAX_EXTRA_WRITE_ROOTS,
            );
        }
        // An entry that is not an absolute path is not a directory anyone can have meant: the
        // empty string became the empty path (a prefix of EVERY path), and a relative one was
        // resolved against the server's cwd rather than the project. Refuse, naming the entry.
        // (`classify_write_path_in` skips such entries too: a config can change after this runs.)
        for (i, root) in self.security.extra_write_roots.iter().enumerate() {
            let path = std::path::Path::new(root);
            if !path.is_absolute() {
                anyhow::bail!(
                    "security.extra_write_roots entry {i} ({root:?}) is not an absolute path; \
                     use a full path (a leading '~' is not expanded)",
                );
            }
            // `/home/u/work/..` is `/home/u`: the spelling hides how wide the root is.
            if path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                anyhow::bail!(
                    "security.extra_write_roots entry {i} ({root:?}) contains a '..' component; \
                     write the directory out without it",
                );
            }
        }
        if self.security.shell_dangerous_patterns.len() > MAX_SHELL_DANGEROUS_PATTERNS {
            anyhow::bail!(
                "security.shell_dangerous_patterns has {} entries, exceeds limit of {}",
                self.security.shell_dangerous_patterns.len(),
                MAX_SHELL_DANGEROUS_PATTERNS,
            );
        }
        if self.ignored_paths.patterns.len() > MAX_IGNORED_PATTERNS {
            anyhow::bail!(
                "ignored_paths.patterns has {} entries, exceeds limit of {}",
                self.ignored_paths.patterns.len(),
                MAX_IGNORED_PATTERNS,
            );
        }
        Ok(())
    }

    pub fn default_for(name: String) -> Self {
        Self {
            project: ProjectSection {
                name,
                languages: vec![],
                encoding: default_encoding(),
                tool_timeout_secs: default_timeout(),
                system_prompt: None,
                onboarding_version: None,
                system_prompt_sha256: None,
            },
            embeddings: EmbeddingsSection::default(),
            ignored_paths: IgnoredPathsSection::default(),
            security: SecuritySection::default(),
            memory: MemorySection::default(),
            libraries: LibrariesSection::default(),
            lsp: LspSection::default(),
        }
    }

    /// Path to the per-project data directory.
    pub fn data_dir(root: &Path) -> PathBuf {
        root.join(".codescout")
    }
}

#[test]
fn security_section_defaults_write_lock_timeout_to_5s() {
    let toml = "";
    let config: SecuritySection = toml::from_str(toml).unwrap();
    assert_eq!(config.write_lock_timeout_secs, 5);
}

#[test]
fn security_section_accepts_custom_write_lock_timeout() {
    let toml = "write_lock_timeout_secs = 10";
    let config: SecuritySection = toml::from_str(toml).unwrap();
    assert_eq!(config.write_lock_timeout_secs, 10);
}

#[test]
fn lsp_section_parses_per_language_opt_out() {
    let toml = r#"
[project]
name = "demo"

[lsp.rust]
mux = false

[lsp.python]
mux = true
"#;
    let cfg: ProjectConfig = toml::from_str(toml).unwrap();
    assert_eq!(cfg.lsp.langs.get("rust").and_then(|o| o.mux), Some(false));
    assert_eq!(cfg.lsp.langs.get("python").and_then(|o| o.mux), Some(true));
    assert!(!cfg.lsp.langs.contains_key("go"));
}

#[test]
fn lsp_section_absent_parses_to_empty_map() {
    let toml = r#"
[project]
name = "demo"
"#;
    let cfg: ProjectConfig = toml::from_str(toml).unwrap();
    assert!(cfg.lsp.langs.is_empty());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_embed_model_is_allminilm() {
        assert_eq!(default_embed_model(), "local:AllMiniLML6V2Q");
    }

    #[test]
    fn default_config_has_expected_embeddings() {
        let cfg = ProjectConfig::default_for("my-project".into());
        assert_eq!(cfg.embeddings.model_or_default(), "local:AllMiniLML6V2Q");
    }

    #[test]
    fn security_section_default_enables_write_git_indexing() {
        // Previously derived Default gave false for all bool fields,
        // silently disabling write tools for projects without a [security] TOML block.
        let sec = SecuritySection::default();
        assert!(
            sec.file_write_enabled,
            "file_write_enabled should default to true"
        );
        assert!(
            sec.indexing_enabled,
            "indexing_enabled should default to true"
        );
    }

    #[test]
    fn project_config_default_for_enables_write_tools() {
        // default_for() is used when no .codescout/project.toml exists.
        let cfg = ProjectConfig::default_for("test-project".into());
        assert!(cfg.security.file_write_enabled);
        assert!(cfg.security.indexing_enabled);
    }

    #[test]
    fn toml_without_security_section_enables_write_tools() {
        // When [security] is entirely absent from TOML, serde calls Default::default()
        // for SecuritySection. This must agree with the serde field-level defaults.
        let toml = "[project]\nname = \"test\"";
        let cfg: ProjectConfig = toml::from_str(toml).unwrap();
        assert!(cfg.security.file_write_enabled);
        assert!(cfg.security.indexing_enabled);
    }

    /// `redact_values` removes what the message quotes from the input and keeps our own names.
    #[test]
    fn redact_values_keeps_our_field_names_and_drops_the_files_values() {
        assert_eq!(
            super::redact_values("missing field `project`"),
            "missing field `project`"
        );
        assert_eq!(
            super::redact_values("invalid type: string \"S3CRET\", expected a boolean"),
            "invalid type: string \"…\", expected a boolean"
        );
        assert_eq!(
            super::redact_values("unknown variant `S3CRET`, expected `default` or `root`"),
            "unknown variant `…`, expected `default` or `root`"
        );
        assert_eq!(
            super::redact_values("unknown field `S3CRET`"),
            "unknown field `…`"
        );
        assert_eq!(
            super::redact_values("unterminated \"quote"),
            "unterminated \"quote"
        );
    }

    /// A parse error is reported as message, line and column, and never the source line.
    #[test]
    fn describe_toml_error_gives_line_and_column_not_the_source_line() {
        let text = "a = 1\nS3CRET line\n";
        let e = toml::from_str::<toml::Table>(text).unwrap_err();
        let d = super::describe_toml_error(Some(text), &e);
        assert!(d.contains("line 2, column"), "{d}");
        assert!(!d.contains("S3CRET"), "{d}");
    }

    /// `read_project_toml_table` is the salvage's only eye on the file, and each of its four
    /// outcomes decides a policy: missing is an empty file (confirms nothing, closes
    /// nothing), anything unreadable is `None` (which also closes writes).
    #[test]
    fn read_project_toml_table_distinguishes_missing_valid_oversized_and_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert_eq!(
            ProjectConfig::read_project_toml_table(root),
            Some(toml::Table::new()),
            "a missing file is an empty table, not unreadable"
        );

        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        let file = root.join(".codescout").join("project.toml");
        std::fs::write(&file, "[security]\nfile_write_enabled = false\n").unwrap();
        let table = ProjectConfig::read_project_toml_table(root).expect("valid TOML");
        assert_eq!(
            table["security"]["file_write_enabled"].as_bool(),
            Some(false)
        );

        std::fs::write(&file, "[security\n").unwrap();
        assert_eq!(
            ProjectConfig::read_project_toml_table(root),
            None,
            "invalid TOML is unreadable"
        );

        std::fs::write(&file, format!("# {}\n", "x".repeat(1024 * 1024 + 16))).unwrap();
        assert_eq!(
            ProjectConfig::read_project_toml_table(root),
            None,
            "over the loader's 1 MiB cap is unreadable, though it is valid TOML"
        );

        // A path that cannot be stat'ed for a reason other than "not there" is not "missing".
        let blocked = tempfile::tempdir().unwrap();
        std::fs::write(blocked.path().join(".codescout"), "a file, not a directory").unwrap();
        assert_eq!(
            ProjectConfig::read_project_toml_table(blocked.path()),
            None,
            "an unreadable path is not an empty file"
        );
    }

    /// `PathSecurityConfig::default()` is the STANDARD policy, whatever a repository says.
    ///
    /// **What this test is, honestly.** `default()` is a constant literal: no file is read to
    /// build it, so no `project.toml` can change it, and nothing here observes such a change
    /// happening. What the test protects is (a) the literal itself (a mutant that makes
    /// `default()` carry `Root` is killed) and (b) that the repository's own `profile` is
    /// carried into THAT PROJECT'S config by `SecuritySection::to_path_security_config`, so the
    /// two values stay distinct, with the standard deny-list on the `default()` side: a refactor
    /// that dropped the project's profile (so the control fails) or merged the two (so the last
    /// assertions fail) is caught. A caller that builds a config from a repository file and one
    /// from `default()` gets two different policies, and this pins which is which. It does not
    /// pin what a future caller chooses to pass.
    #[test]
    fn a_repository_project_toml_cannot_change_what_path_security_config_default_means() {
        use crate::util::path_security::{validate_read_path, PathSecurityConfig, SecurityProfile};

        let home = crate::platform::resolved_home_dir()
            .expect("a home directory, from $HOME or the account");
        let protected = home.join(".ssh").join("id_rsa");
        let protected = protected.to_str().unwrap();

        assert_eq!(
            PathSecurityConfig::default().profile,
            SecurityProfile::Default
        );
        assert!(
            validate_read_path(protected, None, &PathSecurityConfig::default()).is_err(),
            "control: the standard deny-list refuses ~/.ssh"
        );

        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        std::fs::write(
            dir.path().join(".codescout").join("project.toml"),
            "[project]\nname = \"t\"\n\n[security]\nprofile = \"root\"\n",
        )
        .unwrap();
        let loaded = ProjectConfig::load_with_global_base(
            dir.path(),
            toml::Value::Table(toml::map::Map::new()),
        )
        .unwrap();
        assert_eq!(
            loaded.security.profile,
            SecurityProfile::Root,
            "control: the repository's own project.toml is honoured for that project"
        );
        assert!(
            validate_read_path(protected, None, &loaded.security.to_path_security_config()).is_ok(),
            "control: the file's Root profile really does lift the deny-list for that config"
        );

        let standard = PathSecurityConfig::default();
        assert_eq!(
            standard.profile,
            SecurityProfile::Default,
            "loading a project.toml with profile = \"root\" changed default()"
        );
        assert!(
            validate_read_path(protected, None, &standard).is_err(),
            "default() no longer carries the standard deny-list after a root-profile project.toml"
        );
    }

    #[test]
    fn system_prompt_defaults_to_none() {
        let toml = "[project]\nname = \"test\"";
        let cfg: ProjectConfig = toml::from_str(toml).unwrap();
        assert!(cfg.project.system_prompt.is_none());
    }

    #[test]
    fn system_prompt_parses_from_toml() {
        let toml = "[project]\nname = \"test\"\nsystem_prompt = \"Use pytest for testing.\"";
        let cfg: ProjectConfig = toml::from_str(toml).unwrap();
        assert_eq!(
            cfg.project.system_prompt.as_deref(),
            Some("Use pytest for testing.")
        );
    }

    #[test]
    fn memory_section_defaults() {
        let toml = "[project]\nname = \"test\"";
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        assert!((config.memory.staleness_drift_threshold - 0.3).abs() < 0.01);
        assert!((config.memory.semantic_anchor_min_similarity - 0.1).abs() < 0.01);
        assert_eq!(config.memory.semantic_anchor_top_n, 10);
    }

    #[test]
    fn memory_section_override() {
        let toml = "[project]\nname = \"test\"\n[memory]\nstaleness_drift_threshold = 0.5\n";
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        assert!((config.memory.staleness_drift_threshold - 0.5).abs() < 0.01);
    }

    #[test]
    fn memory_section_default_includes_gotchas() {
        let section = MemorySection::default();
        assert_eq!(section.protected, vec!["gotchas".to_string()]);
    }

    #[test]
    fn memory_section_serde_roundtrip_with_protected() {
        let toml_str = r#"
staleness_drift_threshold = 0.3
protected = ["gotchas", "conventions"]
"#;
        let section: MemorySection = toml::from_str(toml_str).unwrap();
        assert_eq!(
            section.protected,
            vec!["gotchas".to_string(), "conventions".to_string()]
        );

        // Round-trip
        let serialized = toml::to_string_pretty(&section).unwrap();
        let deserialized: MemorySection = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.protected, section.protected);
    }

    #[test]
    fn memory_section_missing_protected_uses_default() {
        let toml_str = r#"
staleness_drift_threshold = 0.3
"#;
        let section: MemorySection = toml::from_str(toml_str).unwrap();
        assert_eq!(section.protected, vec!["gotchas".to_string()]);
    }

    #[test]
    fn project_config_deserializes_libraries_section() {
        let toml = r#"
[project]
name = "test"

[libraries]
auto_index = true
auto_fetch_sources = true
fetch_timeout_secs = 120
"#;
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        assert!(config.libraries.auto_index);
        assert!(config.libraries.auto_fetch_sources);
        assert_eq!(config.libraries.fetch_timeout_secs, 120);
    }

    #[test]
    fn project_config_libraries_defaults() {
        let toml = "[project]\nname = \"test\"\n";
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        assert!(!config.libraries.auto_index);
        assert!(!config.libraries.auto_fetch_sources);
        assert_eq!(config.libraries.fetch_timeout_secs, 300);
    }

    #[test]
    fn security_profile_parses_from_toml() {
        let toml_str = "[project]\nname = \"test\"\n\n[security]\nprofile = \"root\"\n";
        let config: ProjectConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.security.profile,
            crate::util::path_security::SecurityProfile::Root
        );
    }

    #[test]
    fn security_profile_defaults_to_default() {
        let toml_str = "[project]\nname = \"test\"\n\n[security]\nshell_command_mode = \"warn\"\n";
        let config: ProjectConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.security.profile,
            crate::util::path_security::SecurityProfile::Default
        );
    }

    #[test]
    fn stale_debug_enforce_symbol_tools_key_is_ignored_not_rejected() {
        // The `debug_enforce_symbol_tools` security flag was retired. SecuritySection
        // carries no `deny_unknown_fields`, so an existing project.toml that still sets
        // the key must parse (key silently ignored), not error on startup.
        let toml_str =
            "[project]\nname = \"test\"\n\n[security]\ndebug_enforce_symbol_tools = true\n";
        let config: ProjectConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.project.name, "test");
    }
    #[test]
    fn stale_drift_detection_enabled_key_is_ignored_not_rejected() {
        // `drift_detection_enabled` was retired: nothing in src/ ever read it, and
        // the feature it named does not exist on the Qdrant retrieval stack.
        // EmbeddingsSection carries no `deny_unknown_fields`, so the many existing
        // project.toml files that still set it must parse with the key silently
        // ignored, not error on startup. Removing a config field is only safe
        // because of that, which is why this asserts it rather than assuming it.
        let toml_str = "[project]\nname = \"test\"\n\n[embeddings]\ndrift_detection_enabled = false\nmodel = \"local:BGESmallENV15\"\n";
        let config: ProjectConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.project.name, "test");
        // The neighbouring key in the same section still lands — proof the stale
        // key was skipped rather than the whole section being dropped.
        assert_eq!(
            config.embeddings.model.as_deref(),
            Some("local:BGESmallENV15")
        );
    }

    #[test]
    fn project_section_deserializes_onboarding_version() {
        let toml_with = r#"
            name = "test"
            languages = ["rust"]
            onboarding_version = 2
        "#;
        let section: ProjectSection = toml::from_str(toml_with).unwrap();
        assert_eq!(section.onboarding_version, Some(2));
    }

    #[test]
    fn project_section_deserializes_without_onboarding_version() {
        let toml_without = r#"
            name = "test"
            languages = ["rust"]
        "#;
        let section: ProjectSection = toml::from_str(toml_without).unwrap();
        assert_eq!(section.onboarding_version, None);
    }

    #[test]
    fn embeddings_section_parses_url_and_api_key() {
        let toml_str = r#"
[project]
name = "test"
languages = ["rust"]

[embeddings]
model = "nomic-embed-text-v1.5"
url = "http://127.0.0.1:43300/v1"
api_key = "test-key-123"
"#;
        let config: ProjectConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.embeddings.url.as_deref(),
            Some("http://127.0.0.1:43300/v1")
        );
        assert_eq!(
            config.embeddings.api_key.as_ref().map(|k| k.as_str()),
            Some("test-key-123")
        );
    }

    #[test]
    fn embeddings_section_url_defaults_to_none() {
        let toml_str = r#"
[project]
name = "test"
languages = ["rust"]

[embeddings]
model = "ollama:nomic-embed-text"
"#;
        let config: ProjectConfig = toml::from_str(toml_str).unwrap();
        assert!(config.embeddings.url.is_none());
        assert!(config.embeddings.api_key.is_none());
    }

    #[test]
    fn security_section_default_max_index_bytes_is_500mb() {
        let sec = SecuritySection::default();
        assert_eq!(sec.max_index_bytes, 500 * 1024 * 1024);
    }

    #[test]
    fn project_config_default_propagates_max_index_bytes() {
        let cfg = ProjectConfig::default_for("test-project".into());
        assert_eq!(cfg.security.max_index_bytes, 500 * 1024 * 1024);
    }

    /// `chunk_size` is retained purely so an existing `project.toml` carrying it still
    /// deserialises. The four `effective_chunk_size_*` tests that used to sit here were
    /// removed on 2026-08-14 with the function they covered: it had zero production
    /// callers, and the model-derived budget it computed exceeded what the local
    /// embedder actually tokenizes (fastembed caps at 512 tokens for every model).
    ///
    /// This replaces them with the only contract still owed: **an upgrade must not turn
    /// a silently-ignored key into a parse error.**
    #[test]
    fn a_retained_chunk_size_key_still_deserialises_and_is_ignored() {
        let parsed: EmbeddingsSection =
            toml::from_str("model = \"local:AllMiniLML6V2Q\"\nchunk_size = 1500\n")
                .expect("an existing project.toml carrying chunk_size must still parse");
        assert_eq!(
            parsed._chunk_size_ignored,
            Some(1500),
            "the key should land in the ignored field, not be dropped by serde"
        );

        // And it must not be written back out — `skip_serializing` keeps a dead knob
        // from reappearing in files codescout generates.
        let round_tripped = toml::to_string(&parsed).expect("serialises");
        assert!(
            !round_tripped.contains("chunk_size"),
            "an ignored key must not be re-emitted: {round_tripped}"
        );
    }

    #[test]
    fn effective_max_inflight_defaults_to_8() {
        let sec = EmbeddingsSection::default();
        assert_eq!(sec.effective_max_inflight(), 8);
    }

    #[test]
    fn effective_max_inflight_user_value_honored() {
        let sec = EmbeddingsSection {
            max_inflight: Some(32),
            ..Default::default()
        };
        assert_eq!(sec.effective_max_inflight(), 32);
    }

    #[test]
    fn effective_max_inflight_zero_falls_back_to_default() {
        let sec = EmbeddingsSection {
            max_inflight: Some(0),
            ..Default::default()
        };
        assert_eq!(sec.effective_max_inflight(), 8);
    }

    #[test]
    fn effective_file_group_size_defaults_to_50() {
        let sec = EmbeddingsSection::default();
        assert_eq!(sec.effective_file_group_size(), 50);
    }

    #[test]
    fn effective_file_group_size_user_value_honored() {
        let sec = EmbeddingsSection {
            file_group_size: Some(200),
            ..Default::default()
        };
        assert_eq!(sec.effective_file_group_size(), 200);
    }

    #[test]
    fn embeddings_section_parses_inflight_and_group_size() {
        let toml = r#"
            [embeddings]
            model = "local:AllMiniLML6V2Q"
            max_inflight = 16
            file_group_size = 100
        "#;
        #[derive(serde::Deserialize)]
        struct Wrap {
            embeddings: EmbeddingsSection,
        }
        let w: Wrap = toml::from_str(toml).unwrap();
        assert_eq!(w.embeddings.max_inflight, Some(16));
        assert_eq!(w.embeddings.file_group_size, Some(100));
    }

    /// Project config TOML round-trip: an explicit (now-ignored) chunk_size deserializes,
    /// missing key produces None.
    #[test]
    fn project_config_chunk_size_round_trip() {
        let toml_with = r#"
[project]
name = "test"
[embeddings]
model = "local:AllMiniLML6V2Q"
chunk_size = 1500
"#;
        let cfg: ProjectConfig = toml::from_str(toml_with).unwrap();
        assert_eq!(cfg.embeddings._chunk_size_ignored, Some(1500));

        let toml_without = r#"
[project]
name = "test"
[embeddings]
model = "local:AllMiniLML6V2Q"
"#;
        let cfg: ProjectConfig = toml::from_str(toml_without).unwrap();
        assert_eq!(cfg.embeddings._chunk_size_ignored, None);
    }

    // The four `load_with_global_base` tests below were previously
    // `#[allow(dead_code)]` "stale test — missing #[test]" stubs: they were DISABLED
    // because exercising the layering required faking HOME, and faking HOME meant
    // `set_var` in a parallel test binary. Injecting the global base instead removes
    // the env entirely — so they are real, running tests again.
    #[test]
    fn load_or_default_applies_global_when_project_absent() {
        let dir = tempfile::tempdir().unwrap();
        let global_base =
            toml::from_str::<toml::Value>("[embeddings]\nmodel = \"local:BGESmallENV15\"\n")
                .unwrap();

        let cfg = ProjectConfig::load_with_global_base(dir.path(), global_base).unwrap();
        assert_eq!(cfg.embeddings.model.as_deref(), Some("local:BGESmallENV15"));
    }

    #[test]
    fn load_or_default_project_wins_over_global() {
        let dir = tempfile::tempdir().unwrap();
        let codescout_dir = dir.path().join(".codescout");
        std::fs::create_dir_all(&codescout_dir).unwrap();
        std::fs::write(
            codescout_dir.join("project.toml"),
            "[project]\nname = \"proj\"\n\n[embeddings]\nmodel = \"project-model\"\n",
        )
        .unwrap();
        let global_base =
            toml::from_str::<toml::Value>("[embeddings]\nmodel = \"global-model\"\n").unwrap();

        let cfg = ProjectConfig::load_with_global_base(dir.path(), global_base).unwrap();
        assert_eq!(
            cfg.embeddings.model.as_deref(),
            Some("project-model"),
            "project.toml must override the global layer"
        );
    }

    #[test]
    fn load_or_default_global_fills_gap_in_project() {
        // project.toml sets `model` but not `max_index_bytes`; the global layer
        // supplies the latter without being clobbered by the overlay.
        let dir = tempfile::tempdir().unwrap();
        let codescout_dir = dir.path().join(".codescout");
        std::fs::create_dir_all(&codescout_dir).unwrap();
        std::fs::write(
            codescout_dir.join("project.toml"),
            "[project]\nname = \"proj\"\n\n[embeddings]\nmodel = \"project-model\"\n",
        )
        .unwrap();
        let global_base =
            toml::from_str::<toml::Value>("[security]\nmax_index_bytes = 12345\n").unwrap();

        let cfg = ProjectConfig::load_with_global_base(dir.path(), global_base).unwrap();
        assert_eq!(cfg.embeddings.model.as_deref(), Some("project-model"));
        assert_eq!(
            cfg.security.max_index_bytes, 12345,
            "global layer must fill fields the project layer does not set"
        );
    }

    #[test]
    fn load_or_default_no_global_behaves_as_before() {
        // Empty global base == no global config on disk.
        let dir = tempfile::tempdir().unwrap();
        let codescout_dir = dir.path().join(".codescout");
        std::fs::create_dir_all(&codescout_dir).unwrap();
        std::fs::write(
            codescout_dir.join("project.toml"),
            "[project]\nname = \"proj\"\n",
        )
        .unwrap();
        let empty = toml::Value::Table(toml::map::Map::new());

        let cfg = ProjectConfig::load_with_global_base(dir.path(), empty).unwrap();
        assert_eq!(cfg.project.name, "proj");
        assert_eq!(
            cfg.embeddings.model_or_default(),
            default_embed_model(),
            "with no global layer the built-in defaults stand"
        );
    }

    #[test]
    fn env_vars_override_model_and_url() {
        // CODESCOUT_EMBED_* are the highest-precedence layer: they beat both the
        // project and global config. Tested through the pure `apply_embed_overrides`
        // seam rather than by mutating process env — see the module note in
        // config/global.rs on why no test here may call set_var.
        let mut cfg = ProjectConfig::default_for("proj".into());
        cfg.embeddings.model = Some("config-model".into());
        cfg.embeddings.url = Some("http://config-host/v1".into());

        cfg.apply_embed_overrides(
            Some("EnvModel".to_string()),
            Some("http://env-host/v1".to_string()),
        );

        assert_eq!(cfg.embeddings.model.as_deref(), Some("EnvModel"));
        assert_eq!(cfg.embeddings.url.as_deref(), Some("http://env-host/v1"));
    }

    #[test]
    fn absent_env_vars_leave_config_untouched() {
        let mut cfg = ProjectConfig::default_for("proj".into());
        cfg.embeddings.model = Some("config-model".into());
        cfg.embeddings.url = Some("http://config-host/v1".into());

        cfg.apply_embed_overrides(None, None);

        assert_eq!(cfg.embeddings.model.as_deref(), Some("config-model"));
        assert_eq!(cfg.embeddings.url.as_deref(), Some("http://config-host/v1"));
    }

    #[test]
    fn validate_list_limits_rejects_oversized_patterns() {
        // Regression for S5 — a 1 MiB TOML can encode tens of thousands of
        // regex patterns; each would be compiled/evaluated on every ignore
        // walk. The cap keeps that hot path bounded.
        let toml_str = r#"
[project]
name = "x"
languages = []

[embeddings]
model = "local:test"
"#;
        let mut cfg: ProjectConfig = toml::from_str(toml_str).unwrap();

        // Under the default limit passes.
        cfg.validate_list_limits().unwrap();

        // Exceeding the cap on ignored_paths.patterns rejects.
        cfg.ignored_paths.patterns = (0..2000).map(|i| format!("p{i}")).collect();
        let err = cfg.validate_list_limits().unwrap_err();
        assert!(
            err.to_string().contains("ignored_paths.patterns"),
            "expected ignored_paths limit error, got {err}"
        );
    }

    /// An empty `extra_write_roots` entry made the empty path a write root, and
    /// `Path::starts_with("")` is true for every path: every file outside the deny list became
    /// writable (`F9a` in `docs/issues/2026-10-17-extra-write-roots-entry-is-not-validated-an-empty-one-admits-every-path.md`).
    /// A relative one is resolved against the server's cwd, not the project. Neither is a
    /// directory the author can have meant, so the config is refused, naming the entry.
    #[test]
    fn validate_list_limits_rejects_an_extra_write_root_that_is_not_absolute() {
        let mut cfg = ProjectConfig::default_for("x".into());
        cfg.validate_list_limits().unwrap();

        for bad in ["", ".", "sub/dir", "../sibling", "~/elsewhere"] {
            cfg.security.extra_write_roots = vec![bad.to_string()];
            let err = cfg
                .validate_list_limits()
                .expect_err(&format!("{bad:?} is not an absolute path"));
            let msg = err.to_string();
            assert!(
                msg.contains("security.extra_write_roots") && msg.contains("absolute"),
                "{bad:?}: the error must name the field and the rule, got {msg}"
            );
        }

        // The offending entry is named even when it is not the first.
        let good = std::env::temp_dir().display().to_string();
        cfg.security.extra_write_roots = vec![good.clone(), String::new()];
        let err = cfg.validate_list_limits().unwrap_err();
        assert!(err.to_string().contains("entry 1"), "got {err}");

        // Absolute entries are untouched.
        cfg.security.extra_write_roots = vec![good];
        cfg.validate_list_limits().unwrap();
    }

    /// An absolute entry with a `..` component hides how wide the root is: `/home/u/work/..` is
    /// `/home/u`. Refused at load, naming the entry, like a relative one.
    #[test]
    fn validate_list_limits_rejects_an_absolute_extra_write_root_with_dotdot() {
        let mut cfg = ProjectConfig::default_for("x".into());
        let spelled = std::env::temp_dir().join("work/..").display().to_string();
        cfg.security.extra_write_roots = vec![spelled.clone()];
        let err = cfg
            .validate_list_limits()
            .expect_err("a `..` entry must be refused");
        let msg = err.to_string();
        assert!(
            msg.contains("security.extra_write_roots") && msg.contains("'..'"),
            "the error must name the field and the `..` rule, got {msg}"
        );
        assert!(msg.contains(&spelled), "the entry is named, got {msg}");
    }

    /// The stderr line for a project that will not start must name the file: before, it read
    /// `Error: security.extra_write_roots entry 0 ("~/scratch") is not an absolute path ...` and a
    /// reader with several projects (or a global config in the merge) had to guess which.
    #[test]
    fn a_config_that_fails_to_load_names_the_project_toml() {
        let dir = tempfile::tempdir().unwrap();
        let cfg_dir = dir.path().join(".codescout");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        let path = cfg_dir.join("project.toml");

        std::fs::write(
            &path,
            "[project]\nname = \"x\"\n\n[security]\nextra_write_roots = [\"~/scratch\"]\n",
        )
        .unwrap();
        let err = ProjectConfig::load_with_global_base(
            dir.path(),
            toml::Value::Table(Default::default()),
        )
        .expect_err("a `~` extra root is refused");
        let msg = format!("{err:#}");
        assert!(
            msg.contains(&path.display().to_string()),
            "the file is named: {msg}"
        );
        assert!(
            msg.contains("extra_write_roots"),
            "the cause is kept: {msg}"
        );

        // A TOML syntax error is named the same way.
        std::fs::write(&path, "[project\nname = ").unwrap();
        let err = ProjectConfig::load_with_global_base(
            dir.path(),
            toml::Value::Table(Default::default()),
        )
        .expect_err("invalid TOML");
        assert!(
            format!("{err:#}").contains(&path.display().to_string()),
            "the file is named for a parse error too: {err:#}"
        );
    }

    // docs/issues/archive/2026-10-16-default-ignore-patterns-skip-a-project-with-no-ignored-paths-section.md
    //
    // The serde field default on `IgnoredPathsSection::patterns` applies only when the
    // `[ignored_paths]` table is PRESENT; an absent table takes the struct's `Default`,
    // which used to be the derived one (`patterns = []`). Every case below must resolve
    // to the same defaults, and an explicit empty list must stay an explicit opt-out.
    #[test]
    fn absent_ignored_paths_section_gets_default_patterns() {
        let dir = tempfile::tempdir().unwrap();
        let codescout_dir = dir.path().join(".codescout");
        std::fs::create_dir_all(&codescout_dir).unwrap();
        std::fs::write(
            codescout_dir.join("project.toml"),
            "[project]\nname = \"proj\"\n",
        )
        .unwrap();

        let empty_global = toml::Value::Table(toml::map::Map::new());
        let cfg = ProjectConfig::load_with_global_base(dir.path(), empty_global).unwrap();
        assert_eq!(
            cfg.ignored_paths.patterns,
            default_ignored_patterns(),
            "no [ignored_paths] section must mean \"defaults apply\" (design § 3)"
        );
    }

    #[test]
    fn missing_project_toml_gets_default_patterns() {
        let dir = tempfile::tempdir().unwrap();
        let empty_global = toml::Value::Table(toml::map::Map::new());
        let cfg = ProjectConfig::load_with_global_base(dir.path(), empty_global).unwrap();
        assert_eq!(cfg.ignored_paths.patterns, default_ignored_patterns());
    }

    #[test]
    fn ignored_paths_section_without_patterns_key_gets_default_patterns() {
        let dir = tempfile::tempdir().unwrap();
        let codescout_dir = dir.path().join(".codescout");
        std::fs::create_dir_all(&codescout_dir).unwrap();
        std::fs::write(
            codescout_dir.join("project.toml"),
            "[project]\nname = \"proj\"\n\n[ignored_paths]\nforce_include = [\"docs\"]\n",
        )
        .unwrap();

        let empty_global = toml::Value::Table(toml::map::Map::new());
        let cfg = ProjectConfig::load_with_global_base(dir.path(), empty_global).unwrap();
        assert_eq!(cfg.ignored_paths.patterns, default_ignored_patterns());
        assert_eq!(cfg.ignored_paths.force_include, vec!["docs".to_string()]);
    }

    #[test]
    fn explicit_empty_patterns_still_means_ignore_nothing() {
        // The deliberate opt-out (e.g. a Kotlin backend whose source lives under `build/`)
        // must survive the fix: only an ABSENT key falls back to the defaults.
        let dir = tempfile::tempdir().unwrap();
        let codescout_dir = dir.path().join(".codescout");
        std::fs::create_dir_all(&codescout_dir).unwrap();
        std::fs::write(
            codescout_dir.join("project.toml"),
            "[project]\nname = \"proj\"\n\n[ignored_paths]\npatterns = []\n",
        )
        .unwrap();

        let empty_global = toml::Value::Table(toml::map::Map::new());
        let cfg = ProjectConfig::load_with_global_base(dir.path(), empty_global).unwrap();
        assert!(
            cfg.ignored_paths.patterns.is_empty(),
            "explicit `patterns = []` is an opt-out and must not be replaced by defaults"
        );
    }

    #[test]
    fn default_for_and_default_agree_on_default_patterns() {
        // `default_for` and onboarding build the section via `Default::default()`.
        assert_eq!(
            ProjectConfig::default_for("x".into())
                .ignored_paths
                .patterns,
            default_ignored_patterns()
        );
        assert_eq!(
            IgnoredPathsSection::default().patterns,
            default_ignored_patterns()
        );
    }

    #[test]
    fn default_patterns_are_not_frozen_into_a_written_project_toml() {
        // Onboarding and the version stamps re-serialise a whole `ProjectConfig`. Writing
        // the default list out would pin today's defaults into that file forever.
        let written = toml::to_string_pretty(&ProjectConfig::default_for("x".into())).unwrap();
        // Line-anchored: `shell_dangerous_patterns` also contains the substring.
        assert!(
            !written.lines().any(|l| l.starts_with("patterns")),
            "default patterns must stay implicit in a written project.toml, got:\n{written}"
        );
        let back: ProjectConfig = toml::from_str(&written).unwrap();
        assert_eq!(back.ignored_paths.patterns, default_ignored_patterns());
    }

    #[test]
    fn explicit_empty_patterns_survive_a_serialise_round_trip() {
        // The re-serialisation above must not flip a deliberate opt-out back to defaults:
        // an empty list differs from the defaults, so it has to be written.
        let mut cfg = ProjectConfig::default_for("x".into());
        cfg.ignored_paths.patterns = Vec::new();
        let written = toml::to_string_pretty(&cfg).unwrap();
        let back: ProjectConfig = toml::from_str(&written).unwrap();
        assert!(
            back.ignored_paths.patterns.is_empty(),
            "written:\n{written}"
        );

        let mut cfg = ProjectConfig::default_for("x".into());
        cfg.ignored_paths.patterns = vec!["custom".into()];
        let back: ProjectConfig = toml::from_str(&toml::to_string_pretty(&cfg).unwrap()).unwrap();
        assert_eq!(back.ignored_paths.patterns, vec!["custom".to_string()]);
    }

    // Load -> write back -> reload, with a GLOBAL config layer present. Onboarding and the
    // version stamps re-serialise a whole `ProjectConfig` over `project.toml`, so the
    // written file must resolve to the SAME patterns the first load produced. Omitting the
    // key on "equals the defaults" alone breaks that: a project that explicitly lists the
    // defaults loses the key, and the global layer's list then fills the gap on reload.
    fn load_write_back_reload(
        project_toml: Option<&str>,
        global_toml: &str,
    ) -> (Vec<String>, String, Vec<String>) {
        let dir = tempfile::tempdir().unwrap();
        if let Some(text) = project_toml {
            let cs = dir.path().join(".codescout");
            std::fs::create_dir_all(&cs).unwrap();
            std::fs::write(cs.join("project.toml"), text).unwrap();
        }
        let global: toml::Value = toml::from_str(global_toml).unwrap();
        let first = ProjectConfig::load_with_global_base(dir.path(), global.clone()).unwrap();
        let written = toml::to_string_pretty(&first).unwrap();
        let cs = dir.path().join(".codescout");
        std::fs::create_dir_all(&cs).unwrap();
        std::fs::write(cs.join("project.toml"), &written).unwrap();
        let second = ProjectConfig::load_with_global_base(dir.path(), global).unwrap();
        (
            first.ignored_paths.patterns,
            written,
            second.ignored_paths.patterns,
        )
    }

    #[test]
    fn explicit_default_patterns_survive_write_back_when_a_global_list_exists() {
        let explicit = format!(
            "[project]\nname = \"p\"\n\n[ignored_paths]\npatterns = {}\n",
            toml::Value::try_from(default_ignored_patterns()).unwrap()
        );
        let (first, written, second) = load_write_back_reload(
            Some(&explicit),
            "[ignored_paths]\npatterns = [\"generated\"]\n",
        );
        assert_eq!(first, default_ignored_patterns(), "project layer wins");
        assert_eq!(
            second, first,
            "write-back changed the meaning of the file:\n{written}"
        );
    }

    #[test]
    fn explicit_empty_patterns_survive_write_back_when_a_global_list_exists() {
        let (first, written, second) = load_write_back_reload(
            Some("[project]\nname = \"p\"\n\n[ignored_paths]\npatterns = []\n"),
            "[ignored_paths]\npatterns = [\"generated\"]\n",
        );
        assert!(first.is_empty());
        assert_eq!(second, first, "write-back changed the file:\n{written}");
    }

    #[test]
    fn absent_key_stays_implicit_across_write_back_and_follows_the_global_layer() {
        // No project key, no global list: defaults, and the key is not frozen into the file.
        let (first, written, second) =
            load_write_back_reload(Some("[project]\nname = \"p\"\n"), "[ignored_paths]\n");
        assert_eq!(first, default_ignored_patterns());
        assert!(
            !written.lines().any(|l| l.starts_with("patterns")),
            "an absent key must stay absent:\n{written}"
        );
        assert_eq!(second, first);

        // No project key, a global list: the global list wins, and survives the round trip.
        let (first, _, second) = load_write_back_reload(
            Some("[project]\nname = \"p\"\n"),
            "[ignored_paths]\npatterns = [\"generated\"]\n",
        );
        assert_eq!(first, vec!["generated".to_string()]);
        assert_eq!(second, first);
    }

    #[test]
    fn a_missing_project_toml_with_a_global_list_round_trips() {
        let (first, written, second) =
            load_write_back_reload(None, "[ignored_paths]\npatterns = [\"generated\"]\n");
        assert_eq!(first, vec!["generated".to_string()]);
        assert_eq!(second, first, "written:\n{written}");
    }

    #[test]
    fn default_ignores_cover_dependency_trees_but_never_plausible_source_dirs() {
        // docs/issues/archive/2026-07-10-oom-blast-radius-cgroup-cap.md, item 3: broaden the
        // default-ignore set so more dependency/cache trees are pruned out of the box.
        let d = default_ignored_patterns();

        // The set is compiled with GITIGNORE semantics for the code index, so a bare
        // name prunes at any depth. Everything here must be a directory that cannot
        // plausibly hold hand-authored source.
        for expected in [
            "node_modules",
            "target",
            ".venv",
            "venv",
            "site-packages",
            "__pycache__",
            ".mypy_cache",
            ".pytest_cache",
            ".tox",
            ".next",
            ".gradle",
            ".terraform",
            ".direnv",
        ] {
            assert!(
                d.iter().any(|p| p == expected),
                "`{expected}` must be ignored by default; got {d:?}"
            );
        }

        // GUARD: these read like build output but are real source dirs in some
        // ecosystems. With gitignore semantics a bare entry would prune `src/models/`
        // (Django/Rails) or a legitimate `vendor/`/`bin/` — silently dropping code the
        // user wanted indexed. Keep them per-project, never default-on.
        // See docs/trackers/index-scope-default-ignores.md ("default-on risks silently
        // skipping code a user wants indexed").
        for forbidden in ["models", "vendor", "bin", "obj", "out", "src", "lib"] {
            assert!(
                !d.iter().any(|p| p == forbidden),
                "`{forbidden}` must NOT be a default ignore — it is a plausible source \
                 directory and gitignore semantics would prune it at any depth"
            );
        }

        // No duplicates — the list is unioned with the user's patterns.
        let mut sorted = d.clone();
        sorted.sort();
        let before = sorted.len();
        sorted.dedup();
        assert_eq!(before, sorted.len(), "default ignore list has duplicates");
    }

    #[test]
    fn merge_toml_overlay_wins_scalar() {
        let base = toml::Value::Table(toml::toml! { [embeddings] model = "base-model" });
        let overlay = toml::Value::Table(toml::toml! { [embeddings] model = "project-model" });
        let merged = super::merge_toml(base, overlay);
        assert_eq!(
            merged["embeddings"]["model"].as_str(),
            Some("project-model")
        );
    }

    #[test]
    fn merge_toml_base_fills_missing_key() {
        let base = toml::Value::Table(toml::toml! { [embeddings] model = "global-model" });
        let overlay = toml::Value::Table(toml::toml! { [embeddings] max_inflight = 4 });
        let merged = super::merge_toml(base, overlay);
        assert_eq!(merged["embeddings"]["model"].as_str(), Some("global-model"));
        assert_eq!(merged["embeddings"]["max_inflight"].as_integer(), Some(4));
    }

    #[test]
    fn merge_toml_nested_tables_merge_recursively() {
        let base = toml::Value::Table(toml::toml! {
            [security]
            file_write_enabled = false
            shell_command_mode = "warn"
        });
        let overlay = toml::Value::Table(toml::toml! {
            [security]
            shell_command_mode = "unrestricted"
        });
        let merged = super::merge_toml(base, overlay);
        assert_eq!(
            merged["security"]["file_write_enabled"].as_bool(),
            Some(false)
        );
        assert_eq!(
            merged["security"]["shell_command_mode"].as_str(),
            Some("unrestricted")
        );
    }

    #[test]
    fn merge_toml_non_table_overlay_replaces_base() {
        let base = toml::Value::String("base".into());
        let overlay = toml::Value::String("overlay".into());
        let merged = super::merge_toml(base, overlay);
        assert_eq!(merged.as_str(), Some("overlay"));
    }

    #[test]
    fn merge_toml_empty_overlay_returns_base() {
        let base = toml::Value::Table(toml::toml! { [embeddings] model = "base-model" });
        let overlay = toml::Value::Table(toml::map::Map::new());
        let merged = super::merge_toml(base, overlay);
        assert_eq!(merged["embeddings"]["model"].as_str(), Some("base-model"));
    }
}
