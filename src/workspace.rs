use std::path::{Path, PathBuf};

/// Sentinel project ID used when no specific sub-project claims a file or
/// when the root of the workspace has no build manifest.
pub const ROOT_PROJECT_ID: &str = "root";

/// A project discovered by manifest walk during onboarding.
#[derive(Debug, Clone)]
pub struct DiscoveredProject {
    pub id: String,
    pub relative_root: PathBuf,
    pub languages: Vec<String>,
    pub manifest: Option<String>,
}

/// Read up to `max_bytes` of a file as UTF-8. Used for manifest reads where
/// an unbounded `read_to_string` would DoS if the file is pathologically large.
/// Returns `Err` if the file cannot be opened or if the truncated bytes are
/// not valid UTF-8 — both are treated as "skip this manifest" by callers.
fn read_file_capped(path: &Path, max_bytes: u64) -> std::io::Result<String> {
    use std::io::Read;
    let f = std::fs::File::open(path)?;
    let mut buf = String::new();
    f.take(max_bytes).read_to_string(&mut buf)?;
    Ok(buf)
}

/// Walk the workspace root for build manifests and return discovered sub-projects.
pub fn discover_projects(
    workspace_root: &Path,
    max_depth: usize,
    exclude: &[String],
) -> Vec<DiscoveredProject> {
    let manifests: &[(&str, &[&str])] = &[
        ("Cargo.toml", &["rust"]),
        ("build.gradle.kts", &["kotlin", "java"]),
        ("build.gradle", &["kotlin", "java"]),
        ("go.mod", &["go"]),
        ("pom.xml", &["java"]),
        ("CMakeLists.txt", &["c", "cpp"]),
        ("mix.exs", &["elixir"]),
        ("Gemfile", &["ruby"]),
    ];
    let conditional_manifests: &[(&str, &[&str])] = &[
        ("package.json", &["typescript", "javascript"]),
        ("pyproject.toml", &["python"]),
        ("setup.py", &["python"]),
        ("requirements.txt", &["python"]),
    ];

    let mut manifest_dirs: std::collections::BTreeMap<PathBuf, (String, Vec<String>)> =
        std::collections::BTreeMap::new();

    let walker = ignore::WalkBuilder::new(workspace_root)
        .hidden(true)
        .git_ignore(true)
        .max_depth(Some(max_depth + 1))
        .build();

    for entry in walker.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_string();
        let dir = entry.path().parent().unwrap_or(entry.path()).to_path_buf();

        let rel_dir = dir.strip_prefix(workspace_root).unwrap_or(&dir);

        // Skip excluded directories
        if exclude.iter().any(|ex| {
            rel_dir
                .components()
                .any(|c| c.as_os_str().to_string_lossy() == *ex)
        }) {
            continue;
        }

        // Skip node_modules explicitly (ignore crate won't skip without .gitignore)
        if rel_dir
            .components()
            .any(|c| c.as_os_str() == "node_modules")
        {
            continue;
        }

        // Check unconditional manifests
        for (manifest_name, langs) in manifests {
            if file_name == *manifest_name && !manifest_dirs.contains_key(&dir) {
                manifest_dirs.insert(
                    dir.clone(),
                    (
                        manifest_name.to_string(),
                        langs.iter().map(|s| s.to_string()).collect(),
                    ),
                );
                break;
            }
        }

        // Check conditional manifests
        for (manifest_name, langs) in conditional_manifests {
            if file_name != *manifest_name || manifest_dirs.contains_key(&dir) {
                continue;
            }
            if *manifest_name == "package.json" {
                // Cap read at 1 MiB — matches config-file cap. A package.json
                // above this is pathological and would otherwise DoS agent
                // startup (discover_projects is called on every Agent::new /
                // activate, so a large manifest pays on every activation).
                if let Ok(content) = read_file_capped(entry.path(), 1024 * 1024) {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                        let has_scripts = json
                            .get("scripts")
                            .and_then(|v| v.as_object())
                            .map(|o| !o.is_empty())
                            .unwrap_or(false);
                        let has_main = json.get("main").is_some() || json.get("module").is_some();
                        if !has_scripts && !has_main {
                            continue;
                        }
                    } else {
                        continue;
                    }
                } else {
                    continue;
                }
            }
            if *manifest_name == "requirements.txt" && dir.join("pyproject.toml").exists() {
                continue;
            }
            manifest_dirs.insert(
                dir.clone(),
                (
                    manifest_name.to_string(),
                    langs.iter().map(|s| s.to_string()).collect(),
                ),
            );
            break;
        }
    }

    let workspace_name = workspace_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unnamed")
        .to_string();

    // Sort by depth so shallower (parent) projects come first
    let mut dirs: Vec<_> = manifest_dirs.into_iter().collect();
    dirs.sort_by_key(|(p, _)| p.components().count());

    let mut found: Vec<DiscoveredProject> = Vec::new();
    let mut found_roots: Vec<PathBuf> = Vec::new();

    for (dir, (manifest, languages)) in dirs {
        let rel = dir.strip_prefix(workspace_root).unwrap_or(&dir);
        let rel_path = if rel.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            rel.to_path_buf()
        };

        // Skip if dominated by a shallower project with matching language
        let dominated = found_roots.iter().any(|existing| {
            if rel_path == std::path::Path::new(".") || existing == std::path::Path::new(".") {
                return false;
            }
            rel_path.starts_with(existing)
                && found.iter().any(|p| {
                    p.relative_root == *existing
                        && p.languages.iter().any(|l| languages.contains(l))
                })
        });
        if dominated {
            continue;
        }

        let id = if rel_path == std::path::Path::new(".") {
            workspace_name.clone()
        } else {
            rel_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unnamed")
                .to_string()
        };

        // Deduplicate: if id already taken, use path-based name
        let final_id = if found.iter().any(|p| p.id == id) {
            rel_path.to_string_lossy().replace('/', "-")
        } else {
            id
        };

        found_roots.push(rel_path.clone());
        found.push(DiscoveredProject {
            id: final_id,
            relative_root: rel_path,
            languages,
            manifest: Some(manifest),
        });
    }

    // File-content dominance pass (after the manifest-based domination logic, so
    // it doesn't perturb project detection). Each project's `languages` should
    // reflect its actual file mix (dominance-ordered), not just its build
    // manifest — a polyglot root whose manifest ≠ dominant language (e.g. a
    // Python repo keeping package.json for tooling) was mislabeled. Manifest-only
    // dirs (empty scan) keep their manifest langs unchanged.
    // (2026-06-03-project-languages-from-manifest-not-files)
    for p in &mut found {
        let dominance = scan_languages_by_dominance(&workspace_root.join(&p.relative_root));
        if !dominance.is_empty() {
            p.languages = merge_languages(dominance, &p.languages);
        }
    }

    // Ensure root project is first
    if let Some(root_idx) = found
        .iter()
        .position(|p| p.relative_root == std::path::Path::new("."))
    {
        if root_idx != 0 {
            let root = found.remove(root_idx);
            found.insert(0, root);
        }
    }

    found
}
/// Add the `[[project]]` entries a user declared in `workspace.toml` to the manifest-walk
/// result, so a directory with no build manifest (or one the walk cannot see) can be a member.
///
/// The walk stays the authority: a declared entry never renames or replaces a discovered
/// project. Onboarding writes every discovered project back into `workspace.toml` as a
/// `[[project]]` snapshot, so an entry that names a discovered root is the expected case and
/// changes nothing.
///
/// A declared entry is skipped, never repaired, unless its root is a relative path to an
/// existing directory that stays inside the workspace and is not named by `exclude`, and its
/// id is unused and names no path. A mis-rooted file once declared eight roots that did not
/// exist, and each became a memory directory
/// (`docs/issues/archive/2026-08-08-workspace-toml-mis-rooted-declared-sibling-repos-as-projects.md`).
/// Each skip is logged, because a member that silently fails to appear is the defect this
/// function exists to end.
pub fn merge_declared_projects(
    workspace_root: &Path,
    mut discovered: Vec<DiscoveredProject>,
    declared: &[crate::config::workspace::ProjectEntry],
    exclude: &[String],
) -> Vec<DiscoveredProject> {
    let canonical_root =
        std::fs::canonicalize(workspace_root).unwrap_or_else(|_| workspace_root.to_path_buf());

    for entry in declared {
        let skip = |why: &str| {
            tracing::warn!(
                id = %entry.id,
                root = %entry.root,
                "workspace.toml [[project]] entry is not a member: {why}"
            );
        };

        let Some(relative_root) = declared_relative_root(&entry.root) else {
            skip("its root is not a relative path below the workspace root");
            continue;
        };
        if relative_root.components().any(|c| {
            exclude
                .iter()
                .any(|ex| c.as_os_str().to_string_lossy() == *ex)
        }) {
            skip("its root is named by exclude_projects");
            continue;
        }
        // The canonical path decides, so a link that leaves the workspace is refused.
        let inside = std::fs::canonicalize(workspace_root.join(&relative_root))
            .map(|abs| abs.is_dir() && abs != canonical_root && abs.starts_with(&canonical_root))
            .unwrap_or(false);
        if !inside {
            skip("its root is not an existing directory inside the workspace");
            continue;
        }
        if discovered.iter().any(|p| p.relative_root == relative_root) {
            // The expected case for an onboarding snapshot: the walk already found it.
            continue;
        }
        let id_is_usable = !entry.id.is_empty()
            && !entry.id.contains(['/', '\\'])
            && entry.id != "."
            && entry.id != ".."
            && entry.id != ROOT_PROJECT_ID
            && !discovered.iter().any(|p| p.id == entry.id);
        if !id_is_usable {
            skip("its id is empty, names a path, is reserved or is already used");
            continue;
        }

        let languages = if entry.languages.is_empty() {
            scan_languages_by_dominance(&workspace_root.join(&relative_root))
        } else {
            entry.languages.clone()
        };
        discovered.push(DiscoveredProject {
            id: entry.id.clone(),
            relative_root,
            languages,
            manifest: None,
        });
    }
    discovered
}

/// A declared root as a clean relative path, or `None` when it cannot name a directory below
/// the workspace root: empty, `.`, absolute, a drive path, or containing `..`.
///
/// Both separators are accepted because the file is written with forward slashes but hand
/// edited on every platform.
fn declared_relative_root(raw: &str) -> Option<PathBuf> {
    let unified = raw.replace('\\', "/");
    if unified.starts_with('/') {
        return None;
    }
    let mut clean = PathBuf::new();
    for part in unified.split('/').filter(|s| !s.is_empty() && *s != ".") {
        if part == ".." || part.contains(':') {
            return None;
        }
        clean.push(part);
    }
    (!clean.as_os_str().is_empty()).then_some(clean)
}

/// Source languages in `dir`, ordered by file-count dominance (most files
/// first). Bounded — `discover_projects` runs on every `Agent::new` / activate,
/// so the walk is depth- and count-capped and gitignore-aware. Ties broken
/// alphabetically for deterministic output.
/// (2026-06-03-project-languages-from-manifest-not-files)
pub(crate) fn scan_languages_by_dominance(dir: &Path) -> Vec<String> {
    // cap-class: RESULT_CAP workspace.scan_depth — probed
    const MAX_DEPTH: usize = 6;
    // cap-class: RESULT_CAP workspace.scan_files — probed
    const FILE_CAP: usize = 1000;
    let mut counts: std::collections::HashMap<&'static str, usize> =
        std::collections::HashMap::new();
    let walker = ignore::WalkBuilder::new(dir)
        .hidden(true)
        .git_ignore(true)
        .max_depth(Some(MAX_DEPTH))
        .build();
    let mut scanned = 0usize;
    for entry in walker.flatten() {
        if scanned >= FILE_CAP {
            break;
        }
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        if entry
            .path()
            .components()
            .any(|c| c.as_os_str() == "node_modules")
        {
            continue;
        }
        scanned += 1;
        if let Some(lang) = crate::ast::detect_language(entry.path()) {
            *counts.entry(lang).or_default() += 1;
        }
    }
    let mut pairs: Vec<(&'static str, usize)> = counts.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    pairs.into_iter().map(|(l, _)| l.to_string()).collect()
}

/// The dominant programming language in `dir` for `primary_language`, skipping
/// documentation/markup kinds that should never be a project's primary language.
pub(crate) fn dominant_language(dir: &Path) -> Option<String> {
    const SKIP: &[&str] = &["markdown"];
    scan_languages_by_dominance(dir)
        .into_iter()
        .find(|l| !SKIP.contains(&l.as_str()))
}

/// Merge file-dominance languages (ordered, most files first) with
/// manifest-declared languages: dominance first, manifest langs appended as a
/// fallback so manifest-only dirs (no source files) still resolve. Order-
/// preserving and de-duplicated.
fn merge_languages(dominance: Vec<String>, manifest: &[String]) -> Vec<String> {
    let mut out = dominance;
    for m in manifest {
        if !out.iter().any(|l| l == m) {
            out.push(m.clone());
        }
    }
    out
}

/// Lexically collapse `.` / `..` components in a path without touching the
/// filesystem. Used to prevent prefix-match bypass in `resolve_project_for_path`
/// — without this, `/workspace/project/../../etc/passwd` would lexically
/// start-with `/workspace/project` and be incorrectly attributed to the project.
///
/// Does not resolve symlinks; if the path exists, prefer `std::fs::canonicalize`.
fn lexically_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                // Pop a Normal component; otherwise preserve the `..` (e.g. a
                // path starting with `../` has nothing to pop).
                if !matches!(
                    out.components().next_back(),
                    Some(std::path::Component::Normal(_))
                ) {
                    out.push("..");
                } else {
                    out.pop();
                }
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Where a workspace member's memory lives.
///
/// - The root project (`relative_root` is `.`): `<workspace>/.codescout/memories`.
/// - A member that is its own git repository: `<member>/.codescout/memories`. It owns its
///   history, so its memory must not be written into the parent checkout. A `.git` FILE counts
///   too: a submodule or a linked worktree has one. This is also where a by-path activation of
///   that member already reads.
/// - Any other member, or an id no project owns (`relative_root` is `None`; the caller must
///   have validated it, see `Workspace::memory_dir_for_project`):
///   `<workspace>/.codescout/projects/<id>/memories`.
///
/// docs/issues/2026-10-28-a-nested-git-repo-cannot-join-a-workspace-as-a-project-and-pinned-onboarding-mixes-in-the-home-workspace.md
pub fn member_memory_dir(
    workspace_root: &Path,
    project_id: &str,
    relative_root: Option<&Path>,
) -> PathBuf {
    if relative_root == Some(Path::new(".")) {
        return workspace_root.join(".codescout").join("memories");
    }
    if let Some(rel) = relative_root {
        let member_root = workspace_root.join(rel);
        if member_root.join(".git").exists() {
            return member_root.join(".codescout").join("memories");
        }
    }
    workspace_root
        .join(".codescout")
        .join("projects")
        .join(project_id)
        .join("memories")
}

/// Resolve which project a file path belongs to using longest-prefix match.
///
/// The input path is lexically normalized (`..` collapsed) before the prefix
/// check so that traversal attempts like `project/../../etc/passwd` cannot
/// lexically match a project root. Callers downstream that read/write the
/// returned project's files must still re-validate the original `file_path`
/// with `path_security` — this function only classifies, it does not gate I/O.
pub fn resolve_project_for_path<'a>(
    projects: &'a [DiscoveredProject],
    workspace_root: &Path,
    file_path: &Path,
) -> Option<&'a DiscoveredProject> {
    let abs_file = if file_path.is_relative() {
        workspace_root.join(file_path)
    } else {
        file_path.to_path_buf()
    };
    let abs_file = lexically_normalize(&abs_file);

    projects
        .iter()
        .filter(|p| {
            let project_abs = if p.relative_root == std::path::Path::new(".") {
                workspace_root.to_path_buf()
            } else {
                workspace_root.join(&p.relative_root)
            };
            let project_abs = lexically_normalize(&project_abs);
            abs_file.starts_with(&project_abs)
        })
        .max_by_key(|p| p.relative_root.components().count())
}

/// Given a file path and a list of discovered projects, return the project ID.
/// Falls back to `ROOT_PROJECT_ID` if no project claims the file.
pub fn resolve_project_id(
    projects: &[DiscoveredProject],
    workspace_root: &Path,
    file_path: &Path,
) -> String {
    resolve_project_for_path(projects, workspace_root, file_path)
        .map(|p| p.id.clone())
        .unwrap_or_else(|| ROOT_PROJECT_ID.to_string())
}

/// State of a project within the workspace.
pub enum ProjectState {
    /// Not yet activated — LSP not started, config not loaded.
    Dormant,
    /// Fully activated with config, memory, LSP running.
    Activated(Box<crate::agent::ActiveProject>),
}

/// A project within the workspace — discovered metadata + runtime state.
pub struct Project {
    pub discovered: DiscoveredProject,
    pub state: ProjectState,
}

impl Project {
    pub fn new_dormant(discovered: DiscoveredProject) -> Self {
        Self {
            discovered,
            state: ProjectState::Dormant,
        }
    }
}

/// The top-level workspace containing all discovered projects.
pub struct Workspace {
    pub root: PathBuf,
    pub projects: Vec<Project>,
    /// Currently focused project ID (used by require_project_root).
    pub focused: Option<String>,
}

impl Workspace {
    pub fn new(root: PathBuf, projects: Vec<Project>) -> Self {
        // Focus defaults to the root project if present, else first project
        let focused = projects
            .iter()
            .find(|p| p.discovered.relative_root == std::path::Path::new("."))
            .or_else(|| projects.first())
            .map(|p| p.discovered.id.clone());
        Self {
            root,
            projects,
            focused,
        }
    }

    /// Return the absolute root path of the focused project.
    pub fn focused_project_root(&self) -> anyhow::Result<PathBuf> {
        let id = self
            .focused
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("No focused project"))?;
        self.project_root_by_id(id)
    }

    /// Return the absolute root path for a project by ID.
    pub fn project_root_by_id(&self, id: &str) -> anyhow::Result<PathBuf> {
        let project = self
            .projects
            .iter()
            .find(|p| p.discovered.id == id)
            .ok_or_else(|| anyhow::anyhow!("Project '{}' not found in workspace", id))?;
        let abs = if project.discovered.relative_root == std::path::Path::new(".") {
            self.root.clone()
        } else {
            self.root.join(&project.discovered.relative_root)
        };
        Ok(abs)
    }

    /// Resolve root: explicit project ID > file hint > focused project.
    pub fn resolve_root(
        &self,
        project: Option<&str>,
        file_hint: Option<&Path>,
    ) -> anyhow::Result<PathBuf> {
        match (project, file_hint) {
            (Some(id), _) => self.project_root_by_id(id),
            (None, Some(path)) => {
                // Longest-prefix match
                let discovered: Vec<_> =
                    self.projects.iter().map(|p| p.discovered.clone()).collect();
                let result = resolve_project_for_path(&discovered, &self.root, path);
                match result {
                    Some(p) => self.project_root_by_id(&p.id),
                    None => self.focused_project_root(),
                }
            }
            (None, None) => self.focused_project_root(),
        }
    }

    /// Return the focused project, if any.
    pub fn focused_active(&self) -> Option<&Project> {
        let id = self.focused.as_deref()?;
        self.projects.iter().find(|p| p.discovered.id == id)
    }

    pub fn focused_active_mut(&mut self) -> Option<&mut Project> {
        // borrow-checker: clone here to release the immutable borrow on self.focused
        // before mutably iterating self.projects.
        let id = self.focused.clone()?;
        self.projects.iter_mut().find(|p| p.discovered.id == id)
    }

    /// Whether `project_id` names a project of this workspace.
    ///
    /// `memory_dir_for_project` cannot answer this — a lookup miss and a known
    /// non-root project both take its `else` branch — so any caller that accepts
    /// an id from tool input must check here FIRST. Skipping the check is how a
    /// typo silently earned its own `projects/<id>/memories` tree, and how `read`
    /// came to answer "no memory topics exist yet" for a project that never
    /// existed. Measured 2026-08-08; see
    /// `docs/issues/archive/2026-08-08-memory-dir-for-project-materializes-any-id.md`.
    pub fn has_project(&self, project_id: &str) -> bool {
        self.projects.iter().any(|p| p.discovered.id == project_id)
    }

    /// Switch focus to a project by ID.
    ///
    /// Rejects an unknown id. That, plus `Workspace::new` seeding `focused` from
    /// `projects`, is what makes `self.focused` always a real project id — and it
    /// is why `resolve_memory_dir` validates only the caller-supplied id and
    /// leaves the `focused` fallback alone.
    pub fn set_focused(&mut self, project_id: &str) -> anyhow::Result<()> {
        if self.has_project(project_id) {
            self.focused = Some(project_id.to_string());
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Project '{}' not found in workspace",
                project_id
            ))
        }
    }

    /// Return all project IDs in the workspace.
    pub fn project_ids(&self) -> Vec<String> {
        self.projects
            .iter()
            .map(|p| p.discovered.id.clone())
            .collect()
    }

    /// Memory directory for a project. Root project -> workspace-level; a member that is its
    /// own git repository -> that member's own `.codescout/memories`; any other member ->
    /// per-project under the workspace. The layout rule lives in [`member_memory_dir`], which
    /// onboarding also calls, so the two cannot disagree.
    ///
    /// PRECONDITION: `project_id` must name a real project — check `has_project`
    /// first. This function cannot enforce it. A lookup miss and a known non-root
    /// project both take the workspace-layout branch, so an unknown id resolves to a
    /// plausible-looking `projects/<id>/memories` path that no project owns, and
    /// `MemoryStore::from_dir` then creates it on read as well as on write.
    ///
    /// An earlier fix stopped unknown ids from co-mingling into the root memory
    /// dir — right, but it routed them here instead of rejecting them, trading a
    /// co-mingling bug for a directory-littering one plus a `read` that reports a
    /// non-existent project as merely empty. Validation now lives in
    /// `resolve_memory_dir`; see
    /// `docs/issues/archive/2026-08-08-memory-dir-for-project-materializes-any-id.md`.
    pub fn memory_dir_for_project(&self, project_id: &str) -> PathBuf {
        let relative_root = self
            .projects
            .iter()
            .find(|p| p.discovered.id == project_id)
            .map(|p| p.discovered.relative_root.as_path());
        member_memory_dir(&self.root, project_id, relative_root)
    }
}

/// Helper to extract `&ActiveProject` from a focused project's activated state.
impl Project {
    pub fn as_active(&self) -> Option<&crate::agent::ActiveProject> {
        match &self.state {
            ProjectState::Activated(ap) => Some(ap.as_ref()),
            ProjectState::Dormant => None,
        }
    }

    pub fn as_active_mut(&mut self) -> Option<&mut crate::agent::ActiveProject> {
        match &mut self.state {
            ProjectState::Activated(ap) => Some(ap.as_mut()),
            ProjectState::Dormant => None,
        }
    }
}

/// Normalize a relative path reference against a base directory without touching
/// the filesystem (handles `../` components manually).
///
/// # Security
///
/// **NOT a filesystem-safe path normalizer.** This function intentionally
/// permits escaping `base` (e.g. `..` components that rise above it), because
/// it exists to resolve cross-project dependency paths in manifest files —
/// a Cargo workspace member may legitimately reference `../sibling-crate`,
/// which resolves to workspace-root/sibling-crate (above `base = project_root`).
///
/// The result is used only as a lookup key against a closed set of known
/// projects (`project_by_abs` in `infer_depends_on`). Unknown paths silently
/// miss the lookup and are discarded — they never reach an I/O sink.
///
/// **Callers wiring this to a filesystem read/write, shell command, or any
/// other capability that acts on the path MUST re-validate with `path_security`
/// (or equivalent) before using the result.** Keep this function private;
/// making it `pub` creates a cross-crate footgun.
fn normalize_path(base: &Path, relative: &str) -> PathBuf {
    let mut result = base.to_path_buf();
    for component in Path::new(relative).components() {
        match component {
            std::path::Component::ParentDir => {
                result.pop();
            }
            std::path::Component::Normal(c) => result.push(c),
            std::path::Component::CurDir => {}
            _ => result.push(component),
        }
    }
    result
}

/// Read one of a discovered sub-project's manifests as text, or `None` when it cannot be used.
/// A project directory holds one manifest that made it a project; the others (a `requirements.txt`
/// beside a `Cargo.toml`, a `settings.gradle`) are whatever the repository put there, so a name can
/// be a link to a FIFO or a device and a plain read would hold the onboarding. The read is a
/// bounded regular-file read ([`crate::util::fs::read_regular_config_text`]); absent, refused,
/// oversized and non-UTF-8 are all "skip this manifest", as a failed read always was. The reason
/// (never the content) goes to the debug log.
fn read_manifest(path: &Path) -> Option<String> {
    match crate::util::fs::read_regular_config_text(path) {
        Ok(text) => Some(text),
        Err(e) => {
            if !e.is_not_found() {
                tracing::debug!("manifest {} skipped: {e}", path.display());
            }
            None
        }
    }
}

/// Parse Cargo.toml `[dependencies]` / `[dev-dependencies]` for `path = "..."` entries.
fn deps_from_cargo(project_root: &Path) -> Vec<String> {
    let Some(content) = read_manifest(&project_root.join("Cargo.toml")) else {
        return vec![];
    };
    let Ok(table) = toml::from_str::<toml::Value>(&content) else {
        return vec![];
    };
    let mut paths = vec![];
    for section in &["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(deps) = table.get(section).and_then(|v| v.as_table()) {
            for dep in deps.values() {
                if let Some(p) = dep.get("path").and_then(|v| v.as_str()) {
                    paths.push(p.to_string());
                }
            }
        }
    }
    paths
}

/// Parse package.json for `"file:../..."` or `"workspace:../..."` dependency values.
fn deps_from_npm(project_root: &Path) -> Vec<String> {
    let Some(content) = read_manifest(&project_root.join("package.json")) else {
        return vec![];
    };
    let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) else {
        return vec![];
    };
    let mut paths = vec![];
    for section in &["dependencies", "devDependencies", "peerDependencies"] {
        if let Some(deps) = pkg.get(section).and_then(|v| v.as_object()) {
            for val in deps.values() {
                if let Some(s) = val.as_str() {
                    if let Some(path) = s
                        .strip_prefix("file:")
                        .or_else(|| s.strip_prefix("workspace:"))
                    {
                        if path.starts_with("../") || path.starts_with("./") {
                            paths.push(path.to_string());
                        }
                    }
                }
            }
        }
    }
    paths
}

/// Parse pyproject.toml for `{path = "..."}` dependency entries.
fn deps_from_pyproject(project_root: &Path) -> Vec<String> {
    let Some(content) = read_manifest(&project_root.join("pyproject.toml")) else {
        return vec![];
    };
    let Ok(table) = toml::from_str::<toml::Value>(&content) else {
        return vec![];
    };
    let mut paths = vec![];
    // poetry: [tool.poetry.dependencies]
    for dep in table
        .get("tool")
        .and_then(|t| t.get("poetry"))
        .and_then(|p| p.get("dependencies"))
        .and_then(|d| d.as_table())
        .into_iter()
        .flat_map(|t| t.values())
    {
        if let Some(p) = dep.get("path").and_then(|v| v.as_str()) {
            paths.push(p.to_string());
        }
    }
    // PEP 517 / uv: [dependencies] with path entries (less common but handle it)
    for dep in table
        .get("project")
        .and_then(|p| p.get("dependencies"))
        .and_then(|d| d.as_array())
        .into_iter()
        .flatten()
    {
        if let Some(s) = dep.as_str() {
            // Editable path dep: "my-pkg @ file:../sibling" or just "../sibling"
            if let Some(rest) = s.find("@ file:").map(|i| &s[i + 7..]) {
                if rest.starts_with("../") || rest.starts_with("./") {
                    paths.push(rest.to_string());
                }
            }
        }
    }
    paths
}

/// Parse requirements.txt for `-e ../sibling` editable installs.
fn deps_from_requirements(project_root: &Path) -> Vec<String> {
    let Some(content) = read_manifest(&project_root.join("requirements.txt")) else {
        return vec![];
    };
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let path = line.strip_prefix("-e ")?;
            if path.starts_with("../") || path.starts_with("./") {
                Some(path.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Parse settings.gradle / settings.gradle.kts for `includeBuild("../sibling")`.
fn deps_from_gradle(project_root: &Path) -> Vec<String> {
    let content = ["settings.gradle.kts", "settings.gradle"]
        .iter()
        .find_map(|f| read_manifest(&project_root.join(f)))
        .unwrap_or_default();

    let mut paths = vec![];
    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("includeBuild(") {
            // Extract quoted string: includeBuild("../sibling")
            let inner = rest.trim_end_matches(')').trim();
            let path = inner
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| inner.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')));
            if let Some(p) = path {
                if p.starts_with("../") || p.starts_with("./") {
                    paths.push(p.to_string());
                }
            }
        }
    }
    paths
}

/// Infer cross-project dependencies for a single project from its manifest files.
///
/// Returns a sorted, deduplicated list of project IDs that `project_root` depends on,
/// based on path references found in Cargo.toml, package.json, pyproject.toml,
/// requirements.txt, and settings.gradle(.kts).
///
/// Only same-workspace references are returned — external paths are silently ignored.
pub fn infer_depends_on(
    project_root: &Path,
    workspace_root: &Path,
    all_projects: &[DiscoveredProject],
) -> Vec<String> {
    // Build a map: canonical absolute root → project id
    let project_by_abs: std::collections::HashMap<PathBuf, &str> = all_projects
        .iter()
        .map(|p| (workspace_root.join(&p.relative_root), p.id.as_str()))
        .collect();

    let raw_paths: Vec<String> = [
        deps_from_cargo(project_root),
        deps_from_npm(project_root),
        deps_from_pyproject(project_root),
        deps_from_requirements(project_root),
        deps_from_gradle(project_root),
    ]
    .into_iter()
    .flatten()
    .collect();

    let mut deps = std::collections::BTreeSet::new();
    for raw in raw_paths {
        let abs = normalize_path(project_root, &raw);
        if let Some(&id) = project_by_abs.get(&abs) {
            // Don't add self-references
            if abs != *project_root {
                deps.insert(id.to_string());
            }
        }
    }
    deps.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn resolve_project_for_path_rejects_dotdot_escape() {
        // Regression for S3: lexical starts_with without normalization would
        // attribute `/workspace/project-a/../../etc/passwd` to `project-a`
        // because the string prefix matches. After normalization the path
        // collapses to `/etc/passwd` and no project prefix matches.
        use std::path::PathBuf;
        let workspace = PathBuf::from("/workspace");
        let projects = vec![DiscoveredProject {
            id: "project-a".into(),
            relative_root: PathBuf::from("project-a"),
            languages: vec!["rust".into()],
            manifest: Some("Cargo.toml".into()),
        }];
        let escaped = PathBuf::from("/workspace/project-a/../../etc/passwd");
        let resolved = resolve_project_for_path(&projects, &workspace, &escaped);
        assert!(
            resolved.is_none(),
            "escape path must not match any project, got {:?}",
            resolved.map(|p| &p.id)
        );

        // Sanity: a real file inside the project still resolves.
        let legit = PathBuf::from("/workspace/project-a/src/main.rs");
        assert_eq!(
            resolve_project_for_path(&projects, &workspace, &legit).map(|p| p.id.as_str()),
            Some("project-a")
        );
    }

    #[test]
    fn discover_single_project_repo() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();

        let projects = discover_projects(dir.path(), 3, &[]);
        assert_eq!(projects.len(), 1);
        assert_eq!(
            projects[0].id,
            dir.path().file_name().unwrap().to_str().unwrap()
        );
        assert_eq!(projects[0].relative_root, std::path::Path::new("."));
        assert_eq!(projects[0].manifest, Some("Cargo.toml".to_string()));
    }

    #[test]
    fn discover_multi_project_repo() {
        let dir = tempdir().unwrap();
        // Root: Kotlin
        fs::write(dir.path().join("build.gradle.kts"), "").unwrap();
        // Sub: TypeScript
        let mcp = dir.path().join("mcp-server");
        fs::create_dir_all(&mcp).unwrap();
        fs::write(mcp.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();
        // Sub: Python
        let py = dir.path().join("python-services");
        fs::create_dir_all(&py).unwrap();
        fs::write(py.join("requirements.txt"), "flask\n").unwrap();

        let projects = discover_projects(dir.path(), 3, &[]);
        assert_eq!(projects.len(), 3);

        // Root project first
        assert_eq!(projects[0].relative_root, std::path::Path::new("."));
        assert_eq!(projects[0].manifest, Some("build.gradle.kts".to_string()));

        // Sub-projects sorted by id
        let ids: Vec<&str> = projects.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"mcp-server"));
        assert!(ids.contains(&"python-services"));
    }

    #[test]
    fn skips_node_modules_manifests() {
        let dir = tempdir().unwrap();
        // Root has a real project (non-empty scripts)
        fs::write(
            dir.path().join("package.json"),
            r#"{"scripts":{"test":"jest"}}"#,
        )
        .unwrap();
        let nm = dir.path().join("node_modules").join("dep");
        fs::create_dir_all(&nm).unwrap();
        fs::write(nm.join("package.json"), r#"{"name":"dep"}"#).unwrap();

        let projects = discover_projects(dir.path(), 3, &[]);
        assert_eq!(projects.len(), 1); // only root, not node_modules/dep
    }

    #[test]
    fn respects_exclude_list() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("build.gradle.kts"), "").unwrap();
        let tools = dir.path().join("tools");
        fs::create_dir_all(&tools).unwrap();
        fs::write(tools.join("requirements.txt"), "click\n").unwrap();

        let projects = discover_projects(dir.path(), 3, &["tools".to_string()]);
        assert_eq!(projects.len(), 1); // tools excluded
    }

    #[test]
    fn max_depth_limits_discovery() {
        let dir = tempdir().unwrap();
        // Manifest at depth 4 — should be skipped with max_depth=3
        let deep = dir
            .path()
            .join("a")
            .join("b")
            .join("c")
            .join("deep-service");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("Cargo.toml"), "[package]\nname = \"deep\"").unwrap();

        let projects = discover_projects(dir.path(), 3, &[]);
        assert!(
            projects.is_empty(),
            "manifest at depth 4 should be skipped with max_depth=3"
        );

        // But discoverable with max_depth=5
        let projects = discover_projects(dir.path(), 5, &[]);
        assert_eq!(projects.len(), 1);
    }

    #[test]
    fn id_collision_is_deduplicated() {
        let dir = tempdir().unwrap();
        // Two subdirectories named "api" at different paths
        let svc_api = dir.path().join("services").join("api");
        fs::create_dir_all(&svc_api).unwrap();
        fs::write(svc_api.join("Cargo.toml"), "[package]\nname = \"svc-api\"").unwrap();

        let tools_api = dir.path().join("tools").join("api");
        fs::create_dir_all(&tools_api).unwrap();
        fs::write(
            tools_api.join("Cargo.toml"),
            "[package]\nname = \"tools-api\"",
        )
        .unwrap();

        let projects = discover_projects(dir.path(), 3, &[]);
        let ids: Vec<&str> = projects.iter().map(|p| p.id.as_str()).collect();
        // IDs must be unique — one gets the plain name, other gets path-based name
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1], "IDs must be unique: got {:?}", ids);
    }
    fn declared(
        id: &str,
        root: &str,
        languages: &[&str],
    ) -> crate::config::workspace::ProjectEntry {
        crate::config::workspace::ProjectEntry {
            id: id.to_string(),
            root: root.to_string(),
            languages: languages.iter().map(|s| s.to_string()).collect(),
            depends_on: vec![],
        }
    }

    #[test]
    fn a_declared_manifestless_root_becomes_a_member() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("addon")).unwrap();
        fs::write(dir.path().join("addon/addon.py"), "print('x')").unwrap();

        let merged = merge_declared_projects(
            dir.path(),
            vec![],
            &[declared("thecrew", "addon", &["python"])],
            &[],
        );

        assert_eq!(merged.len(), 1, "got {merged:?}");
        assert_eq!(merged[0].id, "thecrew");
        assert_eq!(merged[0].relative_root, PathBuf::from("addon"));
        assert_eq!(merged[0].languages, vec!["python".to_string()]);
        assert_eq!(merged[0].manifest, None);
    }

    #[test]
    fn a_declared_member_without_languages_gets_them_from_its_files() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("addon")).unwrap();
        fs::write(dir.path().join("addon/addon.py"), "print('x')").unwrap();

        let merged = merge_declared_projects(
            dir.path(),
            vec![],
            &[declared("thecrew", "addon", &[])],
            &[],
        );

        assert_eq!(merged[0].languages, vec!["python".to_string()]);
    }

    #[test]
    fn a_discovered_project_is_never_renamed_by_a_declared_entry_for_its_root() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("svc")).unwrap();
        let found = vec![make_project("svc", "svc")];

        let merged = merge_declared_projects(
            dir.path(),
            found,
            &[declared("other", "svc", &["rust"])],
            &[],
        );

        let ids: Vec<&str> = merged.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["svc"]);
    }

    #[test]
    fn a_declared_root_that_cannot_be_a_member_is_skipped() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a_file"), "x").unwrap();
        let outside = tempdir().unwrap();
        let absolute = outside.path().to_string_lossy().into_owned();

        for (label, root) in [
            ("absolute", absolute.as_str()),
            ("parent escape", "../elsewhere"),
            ("embedded parent", "a/../../elsewhere"),
            ("missing", "no-such-dir"),
            ("a file", "a_file"),
            ("empty", ""),
            ("the workspace root", "."),
        ] {
            let merged = merge_declared_projects(
                dir.path(),
                vec![],
                &[declared("x", root, &["python"])],
                &[],
            );
            assert!(
                merged.is_empty(),
                "{label} root {root:?} became a member: {merged:?}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_declared_root_that_is_a_link_out_of_the_workspace_is_skipped() {
        let dir = tempdir().unwrap();
        let outside = tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("linked")).unwrap();

        let merged = merge_declared_projects(
            dir.path(),
            vec![],
            &[declared("linked", "linked", &["python"])],
            &[],
        );

        assert!(
            merged.is_empty(),
            "a link out of the workspace became a member: {merged:?}"
        );
    }

    #[test]
    fn a_declared_id_that_is_taken_or_names_a_path_is_skipped() {
        let dir = tempdir().unwrap();
        for d in ["one", "two", "three", "four"] {
            fs::create_dir_all(dir.path().join(d)).unwrap();
        }
        let found = vec![make_project("taken", "one")];

        let merged = merge_declared_projects(
            dir.path(),
            found,
            &[
                declared("taken", "two", &[]),
                declared("a/b", "three", &[]),
                declared("..", "four", &[]),
                declared("", "four", &[]),
            ],
            &[],
        );

        let ids: Vec<&str> = merged.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["taken"], "got {merged:?}");
    }

    #[test]
    fn a_root_declared_twice_is_one_member_and_the_first_id_wins() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("addon")).unwrap();

        let merged = merge_declared_projects(
            dir.path(),
            vec![],
            &[
                declared("first", "addon", &[]),
                declared("second", "./addon/", &[]),
            ],
            &[],
        );

        let ids: Vec<&str> = merged.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["first"]);
    }

    #[test]
    fn exclude_projects_applies_to_declared_roots_too() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("tests/fixtures/a")).unwrap();

        let merged = merge_declared_projects(
            dir.path(),
            vec![],
            &[declared("a", "tests/fixtures/a", &["rust"])],
            &["fixtures".to_string()],
        );

        assert!(
            merged.is_empty(),
            "an excluded root became a member: {merged:?}"
        );
    }
    #[test]
    fn a_declared_id_may_not_be_the_id_of_the_synthesized_root_project() {
        // When no manifest sits at the workspace root, the loader inserts a root project whose
        // id is `ROOT_PROJECT_ID`. A declared member with that id would make two projects
        // answer to one id.
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("addon")).unwrap();

        let merged = merge_declared_projects(
            dir.path(),
            vec![],
            &[declared(ROOT_PROJECT_ID, "addon", &[])],
            &[],
        );

        assert!(
            merged.is_empty(),
            "a member took the root project's id: {merged:?}"
        );
    }

    #[test]
    fn resolve_project_from_path_uses_longest_prefix() {
        let dir = tempdir().unwrap();
        let projects = vec![
            DiscoveredProject {
                id: ROOT_PROJECT_ID.into(),
                relative_root: ".".into(),
                languages: vec!["kotlin".into()],
                manifest: Some("build.gradle.kts".into()),
            },
            DiscoveredProject {
                id: "mcp-server".into(),
                relative_root: "mcp-server".into(),
                languages: vec!["typescript".into()],
                manifest: Some("package.json".into()),
            },
        ];

        // File inside mcp-server → resolves to mcp-server
        let result = resolve_project_for_path(
            &projects,
            dir.path(),
            &dir.path().join("mcp-server/src/index.ts"),
        );
        assert_eq!(result.unwrap().id, "mcp-server");

        // File at root → resolves to root
        let result = resolve_project_for_path(
            &projects,
            dir.path(),
            &dir.path().join("src/main/kotlin/App.kt"),
        );
        assert_eq!(result.unwrap().id, ROOT_PROJECT_ID);
    }

    // ── infer_depends_on tests ──────────────────────────────────────────────

    fn make_project(id: &str, relative_root: &str) -> DiscoveredProject {
        DiscoveredProject {
            id: id.to_string(),
            relative_root: PathBuf::from(relative_root),
            languages: vec![],
            manifest: None,
        }
    }

    /// The discriminating case `memory_dir_for_project` collapses: an id that is
    /// absent and an id that is present-but-not-root both take its `else` branch,
    /// so only `has_project` can tell them apart. A mutation flipping either arm
    /// of `has_project` must fail here.
    #[test]
    fn has_project_distinguishes_unknown_id_from_known_non_root_project() {
        let dir = tempdir().unwrap();
        let ws = Workspace::new(
            dir.path().to_path_buf(),
            vec![
                Project::new_dormant(make_project(ROOT_PROJECT_ID, ".")),
                Project::new_dormant(make_project("mcp-server", "mcp-server")),
            ],
        );

        assert!(ws.has_project(ROOT_PROJECT_ID), "root project is a project");
        assert!(
            ws.has_project("mcp-server"),
            "declared sub-project is a project"
        );
        assert!(
            !ws.has_project("mcp-serverr"),
            "a typo is NOT a project — this is the whole point"
        );
        assert!(!ws.has_project(""), "empty id is not a project");

        // The collapse being guarded against: both of these produce a
        // `projects/<id>/memories` path, and the paths are indistinguishable in
        // shape, so a caller cannot use the return value to detect the typo.
        let known = ws.memory_dir_for_project("mcp-server");
        let typo = ws.memory_dir_for_project("mcp-serverr");
        assert!(known.starts_with(dir.path().join(".codescout").join("projects")));
        assert!(typo.starts_with(dir.path().join(".codescout").join("projects")));
        assert_ne!(known, typo);

        // And the root project does NOT land under projects/ — the branch that
        // makes the `else` arm reachable for everything else.
        assert_eq!(
            ws.memory_dir_for_project(ROOT_PROJECT_ID),
            dir.path().join(".codescout").join("memories")
        );
    }
    /// A member that is its own git repository keeps its memory in its own `.codescout/`, not in
    /// the parent checkout. Its memory would otherwise be written into a repository that does not
    /// own it. A `.git` FILE counts too: a submodule or a linked worktree has one.
    #[test]
    fn a_member_with_its_own_git_repo_keeps_its_memory_in_its_own_directory() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("remote-fixes").join("addon");
        fs::create_dir_all(nested.join(".git")).unwrap();
        let plain = dir.path().join("plain");
        fs::create_dir_all(&plain).unwrap();
        let linked = dir.path().join("linked");
        fs::create_dir_all(&linked).unwrap();
        fs::write(linked.join(".git"), "gitdir: ../elsewhere\n").unwrap();
        let ws = Workspace::new(
            dir.path().to_path_buf(),
            vec![
                Project::new_dormant(make_project(ROOT_PROJECT_ID, ".")),
                Project::new_dormant(make_project("addon", "remote-fixes/addon")),
                Project::new_dormant(make_project("plain", "plain")),
                Project::new_dormant(make_project("linked", "linked")),
            ],
        );

        assert_eq!(
            ws.memory_dir_for_project("addon"),
            nested.join(".codescout").join("memories")
        );
        assert_eq!(
            ws.memory_dir_for_project("linked"),
            linked.join(".codescout").join("memories"),
            "a `.git` file marks a repository too"
        );
        assert_eq!(
            ws.memory_dir_for_project("plain"),
            dir.path()
                .join(".codescout")
                .join("projects")
                .join("plain")
                .join("memories"),
            "a member without its own repository keeps the workspace layout"
        );
        assert_eq!(
            ws.memory_dir_for_project(ROOT_PROJECT_ID),
            dir.path().join(".codescout").join("memories"),
            "the root project is unchanged even when it is a repository"
        );
    }

    /// `focused` is the reason `resolve_memory_dir` validates only the
    /// caller-supplied id. If either of these ever fails, that narrowing is
    /// wrong and the `focused` fallback needs validating too.
    #[test]
    fn focused_is_always_a_real_project_id() {
        let dir = tempdir().unwrap();
        let mut ws = Workspace::new(
            dir.path().to_path_buf(),
            vec![
                Project::new_dormant(make_project(ROOT_PROJECT_ID, ".")),
                Project::new_dormant(make_project("mcp-server", "mcp-server")),
            ],
        );

        let seeded = ws.focused.clone().expect("new() seeds focus");
        assert!(
            ws.has_project(&seeded),
            "Workspace::new must seed focus from projects, got {seeded}"
        );

        assert!(
            ws.set_focused("nope").is_err(),
            "unknown id must be refused"
        );
        assert_eq!(
            ws.focused.as_deref(),
            Some(seeded.as_str()),
            "a refused set_focused must not mutate focus"
        );

        ws.set_focused("mcp-server").unwrap();
        assert_eq!(ws.focused.as_deref(), Some("mcp-server"));
    }

    #[test]
    fn infer_depends_on_cargo_path_dep() {
        let dir = tempdir().unwrap();
        let ws = dir.path();
        let api = ws.join("api");
        let shared = ws.join("shared");
        fs::create_dir_all(&api).unwrap();
        fs::create_dir_all(&shared).unwrap();
        fs::write(
            api.join("Cargo.toml"),
            "[package]\nname = \"api\"\n\n[dependencies]\nshared = { path = \"../shared\" }\n",
        )
        .unwrap();
        let projects = vec![make_project("api", "api"), make_project("shared", "shared")];
        let deps = infer_depends_on(&api, ws, &projects);
        assert_eq!(deps, vec!["shared"]);
    }

    #[test]
    fn infer_depends_on_npm_workspace_protocol() {
        let dir = tempdir().unwrap();
        let ws = dir.path();
        let web = ws.join("web");
        let ui = ws.join("ui");
        fs::create_dir_all(&web).unwrap();
        fs::create_dir_all(&ui).unwrap();
        fs::write(
            web.join("package.json"),
            r#"{"name":"web","scripts":{"build":"tsc"},"dependencies":{"@app/ui":"workspace:../ui"}}"#,
        )
        .unwrap();
        let projects = vec![make_project("web", "web"), make_project("ui", "ui")];
        let deps = infer_depends_on(&web, ws, &projects);
        assert_eq!(deps, vec!["ui"]);
    }

    #[test]
    fn infer_depends_on_requirements_txt_editable() {
        let dir = tempdir().unwrap();
        let ws = dir.path();
        let svc = ws.join("svc");
        let lib = ws.join("lib");
        fs::create_dir_all(&svc).unwrap();
        fs::create_dir_all(&lib).unwrap();
        fs::write(svc.join("requirements.txt"), "-e ../lib\nrequests==2.31\n").unwrap();
        let projects = vec![make_project("svc", "svc"), make_project("lib", "lib")];
        let deps = infer_depends_on(&svc, ws, &projects);
        assert_eq!(deps, vec!["lib"]);
    }

    #[test]
    fn infer_depends_on_gradle_include_build() {
        let dir = tempdir().unwrap();
        let ws = dir.path();
        let app = ws.join("app");
        let core = ws.join("core");
        fs::create_dir_all(&app).unwrap();
        fs::create_dir_all(&core).unwrap();
        fs::write(
            app.join("settings.gradle.kts"),
            "includeBuild(\"../core\")\n",
        )
        .unwrap();
        let projects = vec![make_project("app", "app"), make_project("core", "core")];
        let deps = infer_depends_on(&app, ws, &projects);
        assert_eq!(deps, vec!["core"]);
    }

    #[test]
    fn infer_depends_on_no_manifests_returns_empty() {
        let dir = tempdir().unwrap();
        let ws = dir.path();
        let a = ws.join("a");
        fs::create_dir_all(&a).unwrap();
        let projects = vec![make_project("a", "a"), make_project("b", "b")];
        let deps = infer_depends_on(&a, ws, &projects);
        assert!(deps.is_empty());
    }

    #[test]
    fn infer_depends_on_ignores_external_paths() {
        let dir = tempdir().unwrap();
        let ws = dir.path();
        let api = ws.join("api");
        fs::create_dir_all(&api).unwrap();
        // References a path outside the workspace
        fs::write(
            api.join("Cargo.toml"),
            "[package]\nname = \"api\"\n\n[dependencies]\nexternal = { path = \"../../outside\" }\n",
        )
        .unwrap();
        let projects = vec![make_project("api", "api")];
        let deps = infer_depends_on(&api, ws, &projects);
        assert!(deps.is_empty());
    }

    #[test]
    fn package_json_without_scripts_or_main_is_skipped() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("data");
        fs::create_dir_all(&sub).unwrap();
        // package.json with no scripts/main/module — not a real project
        fs::write(
            sub.join("package.json"),
            r#"{"name":"data","version":"1.0"}"#,
        )
        .unwrap();

        let projects = discover_projects(dir.path(), 3, &[]);
        assert!(projects.is_empty());
    }
    #[test]
    fn polyglot_root_languages_reflect_file_dominance() {
        // Regression (2026-06-03-project-languages-from-manifest-not-files): a
        // Python-dominant repo keeping a package.json for tooling was labeled
        // typescript/javascript with Python DROPPED entirely (python is not in the
        // package.json manifest langs). languages must reflect the actual file mix.
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{"name":"poly","scripts":{"build":"tsc"}}"#,
        )
        .unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        for n in 0..4 {
            fs::write(
                dir.path().join(format!("src/mod{n}.py")),
                "def f():\n    pass\n",
            )
            .unwrap();
        }
        fs::write(dir.path().join("src/util.ts"), "export const x = 1;\n").unwrap();

        let projects = discover_projects(dir.path(), 3, &[]);
        assert_eq!(
            projects.len(),
            1,
            "expected single root project: {:?}",
            projects
        );
        // Manifest detection unchanged — package.json is the only manifest present.
        assert_eq!(projects[0].manifest, Some("package.json".to_string()));
        let langs = &projects[0].languages;
        // Pre-fix: ["typescript","javascript"] — python dropped. Post-fix: python
        // leads (4 .py vs 1 .ts), with manifest langs unioned as fallback.
        assert_eq!(
            langs.first().map(String::as_str),
            Some("python"),
            "python should lead: {:?}",
            langs
        );
        assert!(
            langs.iter().any(|l| l == "python"),
            "python must be present (was dropped pre-fix): {:?}",
            langs
        );
        assert!(
            langs.iter().any(|l| l == "typescript"),
            "manifest lang retained as fallback: {:?}",
            langs
        );
        assert_eq!(dominant_language(dir.path()).as_deref(), Some("python"));
    }
}
/// A discovered sub-project's sibling manifests are repository content: any of them can be a link
/// to a FIFO or a device. Each `deps_from_*` reader is driven with one in place of its file.
/// docs/issues/2026-10-20-reads-of-repository-files-that-remain-unbounded-after-the-config-loader-fix.md
#[cfg(all(test, unix))]
mod bounded_manifest_tests {
    use super::*;
    use crate::util::fs::special_file_fixtures::{make_fifo, within_deadline};

    type Reader = fn(&Path) -> Vec<String>;

    /// (file name, reader, a body that names `../foo`).
    const READERS: &[(&str, Reader, &str)] = &[
        (
            "Cargo.toml",
            deps_from_cargo,
            "[dependencies]\nfoo = { path = \"../foo\" }\n",
        ),
        (
            "package.json",
            deps_from_npm,
            "{\"dependencies\":{\"foo\":\"file:../foo\"}}",
        ),
        (
            "pyproject.toml",
            deps_from_pyproject,
            "[tool.poetry.dependencies]\nfoo = { path = \"../foo\" }\n",
        ),
        ("requirements.txt", deps_from_requirements, "-e ../foo\n"),
        (
            "settings.gradle.kts",
            deps_from_gradle,
            "includeBuild(\"../foo\")\n",
        ),
        (
            "settings.gradle",
            deps_from_gradle,
            "includeBuild('../foo')\n",
        ),
    ];

    fn foo() -> Vec<String> {
        vec!["../foo".to_string()]
    }

    fn read_in_time(what: &str, reader: Reader, root: &Path) -> Vec<String> {
        let root = root.to_path_buf();
        within_deadline(what, move || reader(&root))
    }

    /// A FIFO, and a link to one, in place of the manifest held the onboarding. It is skipped.
    #[test]
    fn a_fifo_or_a_link_to_one_in_place_of_a_manifest_is_skipped_and_never_a_hang() {
        for (name, reader, _) in READERS {
            for linked in [false, true] {
                let dir = tempfile::tempdir().unwrap();
                let file = dir.path().join(name);
                if linked {
                    let fifo = dir.path().join("elsewhere-pipe");
                    make_fifo(&fifo);
                    std::os::unix::fs::symlink(&fifo, &file).unwrap();
                } else {
                    make_fifo(&file);
                }
                let got = read_in_time(&format!("{name} a FIFO"), *reader, dir.path());
                assert!(got.is_empty(), "{name} linked={linked}: {got:?}");
            }
        }
    }

    /// Control: regular files, and links to them, are read as before.
    #[test]
    fn a_regular_manifest_and_a_link_to_one_are_still_read() {
        for (name, reader, body) in READERS {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join(name), body).unwrap();
            assert_eq!(reader(dir.path()), foo(), "{name}");

            let linked = tempfile::tempdir().unwrap();
            let real = linked.path().join("real-manifest");
            std::fs::write(&real, body).unwrap();
            std::os::unix::fs::symlink(&real, linked.path().join(name)).unwrap();
            assert_eq!(reader(linked.path()), foo(), "{name} linked");
        }
    }

    /// Exactly at the limit is read; one byte over is skipped. The padding is whitespace, so the
    /// plain read would parse the over-limit file and find `../foo`.
    #[test]
    fn a_manifest_at_the_limit_is_read_and_one_byte_over_is_skipped() {
        let limit = crate::util::fs::MAX_CONFIG_FILE_BYTES as usize;
        for (name, reader, body) in READERS {
            let dir = tempfile::tempdir().unwrap();
            let file = dir.path().join(name);
            let mut padded = (*body).to_string();
            padded.push_str(&" ".repeat(limit - padded.len()));
            std::fs::write(&file, &padded).unwrap();
            assert_eq!(reader(dir.path()), foo(), "{name} at the limit");
            padded.push(' ');
            std::fs::write(&file, &padded).unwrap();
            assert!(reader(dir.path()).is_empty(), "{name} over the limit");
        }
    }

    /// Missing and invalid UTF-8 give nothing, as a failed read always did.
    #[test]
    fn missing_and_invalid_manifests_give_nothing() {
        for (name, reader, _) in READERS {
            let dir = tempfile::tempdir().unwrap();
            assert!(reader(dir.path()).is_empty(), "{name} missing");
            std::fs::write(dir.path().join(name), [0xff, 0xfe]).unwrap();
            assert!(reader(dir.path()).is_empty(), "{name} invalid");
        }
    }

    /// The Gradle fall-through is unchanged: an unusable `settings.gradle.kts` still leaves
    /// `settings.gradle` to be tried, and two unusable ones leave nothing.
    #[test]
    fn an_unusable_kts_settings_file_still_falls_through_to_settings_gradle() {
        let dir = tempfile::tempdir().unwrap();
        make_fifo(&dir.path().join("settings.gradle.kts"));
        std::fs::write(
            dir.path().join("settings.gradle"),
            "includeBuild('../foo')\n",
        )
        .unwrap();
        assert_eq!(
            read_in_time("kts FIFO, groovy regular", deps_from_gradle, dir.path()),
            foo()
        );

        let both = tempfile::tempdir().unwrap();
        make_fifo(&both.path().join("settings.gradle.kts"));
        make_fifo(&both.path().join("settings.gradle"));
        assert!(read_in_time("both FIFOs", deps_from_gradle, both.path()).is_empty());
    }
}
