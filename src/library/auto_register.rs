use std::path::{Path, PathBuf};

use crate::library::registry::DiscoveryMethod;
use crate::tools::ToolContext;

pub struct DiscoveredDep {
    pub name: String,
    pub version_spec: Option<String>,
}

pub struct RegisteredDep {
    pub name: String,
    pub language: String,
    pub source_available: bool,
}

/// Auto-register dependencies from all detected ecosystems.
/// Best-effort: never fails, never blocks activation.
pub async fn auto_register_deps(project_root: &Path, ctx: &ToolContext) -> Vec<RegisteredDep> {
    let mut all_deps: Vec<(DiscoveredDep, String, Option<PathBuf>)> = vec![];

    // Rust
    collect_cargo_deps(project_root, &mut all_deps);
    // Node
    collect_node_deps(project_root, &mut all_deps);
    // Python
    collect_python_deps(project_root, &mut all_deps);
    // Go
    collect_go_deps(project_root, &mut all_deps);
    // Java/Kotlin
    collect_jvm_deps(project_root, &mut all_deps);

    // Batch registration: single write lock
    if all_deps.is_empty() {
        return vec![];
    }

    let result: anyhow::Result<Vec<RegisteredDep>> = async {
        let mut inner = ctx.agent.inner.write().await;
        let project = inner
            .active_project_mut()
            .ok_or_else(|| anyhow::anyhow!("no active project"))?;

        let mut newly_registered = vec![];
        for (dep, language, source_path) in &all_deps {
            let already = project.library_registry.lookup(&dep.name).is_some();
            let source_available = source_path.is_some();
            let path = source_path.clone().unwrap_or_default();

            // Let register() handle all precedence (ManifestScan vs Manual).
            project.library_registry.register(
                dep.name.clone(),
                path,
                language.clone(),
                DiscoveryMethod::ManifestScan,
                source_available,
            );
            if !already {
                newly_registered.push(RegisteredDep {
                    name: dep.name.clone(),
                    language: language.clone(),
                    source_available,
                });
            }
        }

        let registry_path = project.root.join(".codescout").join("libraries.json");
        project.library_registry.save(&registry_path)?;
        Ok(newly_registered)
    }
    .await;

    result.unwrap_or_default()
}

// ── Rust (Cargo) ────────────────────────────────────────────────────────────

fn collect_cargo_deps(
    project_root: &Path,
    out: &mut Vec<(DiscoveredDep, String, Option<PathBuf>)>,
) {
    let cargo_toml = project_root.join("Cargo.toml");
    let content = match std::fs::read_to_string(&cargo_toml) {
        Ok(s) => s,
        Err(_) => return,
    };
    let deps = parse_cargo_deps(&content);
    if deps.is_empty() {
        return;
    }

    let home = match crate::platform::home_dir() {
        Some(h) => h,
        None => return,
    };
    let registry_src = home.join(".cargo").join("registry").join("src");
    let index_dirs: Vec<PathBuf> = match std::fs::read_dir(&registry_src) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect(),
        Err(_) => vec![],
    };

    for dep in deps {
        let source = find_cargo_source(&index_dirs, &dep.name);
        out.push((dep, "rust".to_string(), source));
    }
}

/// Parse direct dependency names from Cargo.toml content.
/// Handles both `name = "version"` and `name = { version = "..." }` forms.
/// Skips `[dev-dependencies]` and `[build-dependencies]`.
pub fn parse_cargo_deps(toml: &str) -> Vec<DiscoveredDep> {
    let mut deps = vec![];
    let mut in_deps = false;

    for line in toml.lines() {
        let trimmed = line.trim();

        // Section header
        if trimmed.starts_with('[') {
            // [dependencies] or [dependencies.foo] but NOT [dev-dependencies] etc.
            in_deps = trimmed == "[dependencies]"
                || trimmed.starts_with("[dependencies.")
                || trimmed.starts_with("[workspace.dependencies");
            continue;
        }

        if !in_deps {
            continue;
        }

        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Extract the key (dep name) from lines like:
        //   serde = "1.0"
        //   serde = { version = "1.0", features = [...] }
        //   serde.workspace = true
        if let Some(eq_pos) = trimmed.find('=') {
            let key = trimmed[..eq_pos].trim();
            // Strip any dotted suffix (e.g. "serde.workspace" → "serde")
            let base_name = key.split('.').next().unwrap_or(key);
            // Normalize hyphens/underscores — Cargo treats them as equivalent
            let normalized = base_name.replace('-', "_");
            if !normalized.is_empty() && !deps.iter().any(|d: &DiscoveredDep| d.name == normalized)
            {
                deps.push(DiscoveredDep {
                    name: normalized,
                    version_spec: None,
                });
            }
        }
    }

    deps
}

/// Find the source directory for a crate in the cargo registry index dirs.
/// Prefers the highest version when multiple versions exist.
pub fn find_cargo_source(index_dirs: &[PathBuf], crate_name: &str) -> Option<PathBuf> {
    // Cargo normalizes `-` to `_` in dir names too — search for both
    let prefix_hyphen = format!("{}-", crate_name.replace('_', "-"));
    let prefix_under = format!("{}-", crate_name.replace('-', "_"));

    let mut candidates: Vec<PathBuf> = vec![];

    for idx_dir in index_dirs {
        if let Ok(rd) = std::fs::read_dir(idx_dir) {
            for entry in rd.filter_map(|e| e.ok()) {
                let fname = entry.file_name();
                let name = fname.to_string_lossy();
                if (name.starts_with(&prefix_hyphen) || name.starts_with(&prefix_under))
                    && entry.path().is_dir()
                {
                    candidates.push(entry.path());
                }
            }
        }
    }

    if candidates.is_empty() {
        return None;
    }

    // Pick the lexicographically largest (roughly highest version)
    candidates.sort();
    candidates.into_iter().next_back()
}

// ── Node/TypeScript ─────────────────────────────────────────────────────────

pub fn parse_node_deps(content: &str) -> Vec<DiscoveredDep> {
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(content) else {
        return vec![];
    };
    let Some(deps) = parsed.get("dependencies").and_then(|d| d.as_object()) else {
        return vec![];
    };
    deps.keys()
        .map(|k| DiscoveredDep {
            name: k.clone(),
            version_spec: deps[k].as_str().map(String::from),
        })
        .collect()
}

pub fn find_node_source(project_root: &Path, dep_name: &str) -> Option<PathBuf> {
    let candidate = project_root.join("node_modules").join(dep_name);
    if !candidate.is_dir() {
        return None;
    }
    // Resolve symlinks and ensure the candidate still lives under the project
    // root. A poisoned `node_modules/express` symlinked to `/etc` would
    // otherwise be auto-registered on every `activate_project` (phase-5 I2).
    let canon_candidate = std::fs::canonicalize(&candidate).ok()?;
    let canon_root =
        std::fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    if !canon_candidate.starts_with(&canon_root) {
        tracing::warn!(
            "skipping node dep '{}': {} resolves outside project root {}",
            dep_name,
            canon_candidate.display(),
            canon_root.display(),
        );
        return None;
    }
    Some(candidate)
}

fn detect_node_language(pkg_dir: &Path) -> &'static str {
    if pkg_dir.join("tsconfig.json").exists() {
        "typescript"
    } else {
        "javascript"
    }
}

fn collect_node_deps(project_root: &Path, out: &mut Vec<(DiscoveredDep, String, Option<PathBuf>)>) {
    let pkg_json = project_root.join("package.json");
    let content = match std::fs::read_to_string(&pkg_json) {
        Ok(s) => s,
        Err(_) => return,
    };
    let deps = parse_node_deps(&content);
    for dep in deps {
        let source = find_node_source(project_root, &dep.name);
        let lang = source
            .as_ref()
            .map(|p| detect_node_language(p))
            .unwrap_or("javascript");
        out.push((dep, lang.to_string(), source));
    }
}

// ── Python ──────────────────────────────────────────────────────────────────

/// PEP 503 normalization adapted for filesystem: lowercase, replace runs of [-_.] with _
fn normalize_python_name(name: &str) -> String {
    let lower = name.to_lowercase();
    let mut result = String::with_capacity(lower.len());
    let mut prev_sep = false;
    for ch in lower.chars() {
        if ch == '-' || ch == '_' || ch == '.' {
            if !prev_sep {
                result.push('_');
                prev_sep = true;
            }
        } else {
            result.push(ch);
            prev_sep = false;
        }
    }
    result
}

pub fn parse_python_deps_pyproject(content: &str) -> Vec<DiscoveredDep> {
    let mut deps = vec![];
    let mut in_project = false;
    let mut in_deps = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_deps = false;
            in_project = trimmed == "[project]";
            continue;
        }
        // Only match `dependencies = [` under [project] table
        if in_project && trimmed.starts_with("dependencies") && trimmed.contains('[') {
            in_deps = true;
            continue;
        }
        if in_deps {
            if trimmed == "]" {
                in_deps = false;
                continue;
            }
            // Strip quotes: "requests>=2.28,<3" → requests>=2.28,<3
            let stripped = trimmed.trim_matches(|c| c == '"' || c == '\'' || c == ',');
            if stripped.is_empty() {
                continue;
            }
            // Extract package name: everything before [, >=, ==, !=, ~=, <, >, ;, @, whitespace
            let name_end = stripped
                .find(|c: char| {
                    c == '['
                        || c == '>'
                        || c == '<'
                        || c == '='
                        || c == '!'
                        || c == '~'
                        || c == ';'
                        || c == '@'
                        || c.is_whitespace()
                })
                .unwrap_or(stripped.len());
            let raw_name = &stripped[..name_end];
            if !raw_name.is_empty() {
                deps.push(DiscoveredDep {
                    name: normalize_python_name(raw_name),
                    version_spec: None,
                });
            }
        }
    }
    deps
}

pub fn parse_python_deps_requirements(content: &str) -> Vec<DiscoveredDep> {
    let mut deps = vec![];
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
            continue;
        }
        if trimmed.contains("://") {
            continue;
        }
        let name_end = trimmed
            .find(|c: char| {
                c == '>'
                    || c == '<'
                    || c == '='
                    || c == '!'
                    || c == '~'
                    || c == '['
                    || c == ';'
                    || c == '@'
                    || c.is_whitespace()
            })
            .unwrap_or(trimmed.len());
        let raw_name = &trimmed[..name_end];
        if !raw_name.is_empty() {
            deps.push(DiscoveredDep {
                name: normalize_python_name(raw_name),
                version_spec: None,
            });
        }
    }
    deps
}

pub fn find_python_source(project_root: &Path, dep_name: &str) -> Option<PathBuf> {
    let canon_root =
        std::fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    let venv_dirs = [".venv", "venv", ".env", "env"];
    for venv in &venv_dirs {
        let lib = project_root.join(venv).join("lib");
        let Ok(entries) = std::fs::read_dir(&lib) else {
            continue;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            // python3.X directory
            let sp = entry.path().join("site-packages").join(dep_name);
            if !sp.is_dir() {
                continue;
            }
            // Resolve symlinks and ensure the candidate still lives under the
            // project root (phase-5 I2).
            let Ok(canon_sp) = std::fs::canonicalize(&sp) else {
                continue;
            };
            if !canon_sp.starts_with(&canon_root) {
                tracing::warn!(
                    "skipping python dep '{}': {} resolves outside project root {}",
                    dep_name,
                    canon_sp.display(),
                    canon_root.display(),
                );
                continue;
            }
            return Some(sp);
        }
    }
    None
}

fn collect_python_deps(
    project_root: &Path,
    out: &mut Vec<(DiscoveredDep, String, Option<PathBuf>)>,
) {
    let pyproject = project_root.join("pyproject.toml");
    let requirements = project_root.join("requirements.txt");

    let deps = if pyproject.exists() {
        match std::fs::read_to_string(&pyproject) {
            Ok(s) => parse_python_deps_pyproject(&s),
            Err(_) => vec![],
        }
    } else if requirements.exists() {
        match std::fs::read_to_string(&requirements) {
            Ok(s) => parse_python_deps_requirements(&s),
            Err(_) => vec![],
        }
    } else {
        return;
    };

    for dep in deps {
        let source = find_python_source(project_root, &dep.name);
        out.push((dep, "python".to_string(), source));
    }
}

// ── Go ──────────────────────────────────────────────────────────────────────

pub fn parse_go_deps(content: &str) -> Vec<DiscoveredDep> {
    let mut deps = vec![];
    let mut in_require = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "require (" {
            in_require = true;
            continue;
        }
        if trimmed == ")" {
            in_require = false;
            continue;
        }
        // Single-line require
        if trimmed.starts_with("require ") && !trimmed.contains('(') {
            let rest = trimmed.strip_prefix("require ").unwrap().trim();
            if let Some((mod_path, _)) = rest.split_once(' ') {
                deps.push(DiscoveredDep {
                    name: mod_path.to_string(),
                    version_spec: None,
                });
            }
            continue;
        }
        if in_require {
            // "github.com/foo/bar v1.2.3" or "github.com/foo/bar v1.2.3 // indirect"
            let parts: Vec<&str> = trimmed.splitn(3, ' ').collect();
            if parts.len() >= 2 {
                deps.push(DiscoveredDep {
                    name: parts[0].to_string(),
                    version_spec: Some(parts[1].to_string()),
                });
            }
        }
    }
    deps
}

/// Go module cache encodes uppercase letters as !lowercase
pub fn go_encode_module_path(path: &str) -> String {
    let mut result = String::with_capacity(path.len() + 4);
    for ch in path.chars() {
        if ch.is_ascii_uppercase() {
            result.push('!');
            result.push(ch.to_ascii_lowercase());
        } else {
            result.push(ch);
        }
    }
    result
}

pub fn find_go_source(mod_cache: &Path, module_path: &str) -> Option<PathBuf> {
    let encoded = go_encode_module_path(module_path);
    let mod_dir = mod_cache.join(&encoded);

    // Module cache stores versioned dirs: github.com/foo/bar@v1.2.3
    // We need to find the right version directory
    if let Ok(rd) = std::fs::read_dir(mod_dir.parent()?) {
        let prefix = format!("{}@", mod_dir.file_name()?.to_string_lossy());
        let mut candidates: Vec<PathBuf> = rd
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix) && e.path().is_dir())
            .map(|e| e.path())
            .collect();
        if !candidates.is_empty() {
            candidates.sort();
            return candidates.into_iter().next_back();
        }
    }

    // Fallback: try exact path (for replace directives pointing to local dirs)
    if mod_dir.is_dir() {
        Some(mod_dir)
    } else {
        None
    }
}

/// Re-derive Go's module cache dir the way `go env GOMODCACHE` would, WITHOUT
/// spawning `go`. Every `CreateProcessW` on the locked-down Windows VDI is
/// EDR-taxed, and a raw `go env` with no timeout can hang dep discovery (same
/// hazard class as WIN-14/WIN-24). Unlike git there is no library binding, so
/// the value is computed from the environment instead. (WIN-25)
///
/// Order mirrors Go's own: `$GOMODCACHE` if set, else `$GOPATH/pkg/mod` if
/// `$GOPATH` is set, else the `<home>/go/pkg/mod` default. A value persisted
/// only via `go env -w` (never exported) is not honored — Go source-linking
/// then degrades to source-not-found for those deps; it never hangs or crashes.
/// Pure over its `lookup` + `home` inputs so it tests on the Linux gate.
fn go_mod_cache_from(
    lookup: impl Fn(&str) -> Option<String>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(v) = lookup("GOMODCACHE").filter(|s| !s.is_empty()) {
        return Some(PathBuf::from(v));
    }
    let gopath = match lookup("GOPATH").filter(|s| !s.is_empty()) {
        Some(p) => PathBuf::from(p),
        None => home?.join("go"),
    };
    Some(gopath.join("pkg").join("mod"))
}

fn collect_go_deps(project_root: &Path, out: &mut Vec<(DiscoveredDep, String, Option<PathBuf>)>) {
    let go_mod = project_root.join("go.mod");
    let content = match std::fs::read_to_string(&go_mod) {
        Ok(s) => s,
        Err(_) => return,
    };
    let deps = parse_go_deps(&content);
    if deps.is_empty() {
        return;
    }

    // Locate GOMODCACHE without spawning `go env` (sync CreateProcessW hangs
    // under EDR on the Windows VDI — WIN-25); re-derive what `go` would print.
    let mod_cache = match go_mod_cache_from(|k| std::env::var(k).ok(), crate::platform::home_dir())
    {
        Some(p) => p,
        None => return,
    };

    for dep in deps {
        let source = find_go_source(&mod_cache, &dep.name);
        out.push((dep, "go".to_string(), source));
    }
}

// ── Java/Kotlin (Gradle + Maven) ────────────────────────────────────────────

pub fn parse_gradle_deps(content: &str) -> Vec<DiscoveredDep> {
    let mut deps = vec![];
    // Match Kotlin DSL: implementation("group:artifact:version")
    // Match Groovy DSL: implementation 'group:artifact:version'
    // Configs: implementation, api, compileOnly, runtimeOnly (NOT test*)
    let re = regex::Regex::new(
        r#"(?:implementation|api|compileOnly|runtimeOnly)\s*(?:\(\s*["']|['"])([^"']+?)["']"#,
    )
    .unwrap();
    for cap in re.captures_iter(content) {
        let coord = &cap[1];
        // group:artifact:version — extract artifact (middle part)
        let parts: Vec<&str> = coord.split(':').collect();
        if parts.len() >= 2 {
            deps.push(DiscoveredDep {
                name: parts[1].to_string(),
                version_spec: parts.get(2).map(|v| v.to_string()),
            });
        }
    }
    deps
}

pub fn parse_maven_deps(content: &str) -> Vec<DiscoveredDep> {
    let mut deps = vec![];
    let mut current_artifact: Option<String> = None;
    let mut current_scope: Option<String> = None;
    let mut in_dependency = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.contains("<dependency>") {
            in_dependency = true;
            current_artifact = None;
            current_scope = None;
        }
        if trimmed.contains("</dependency>") {
            if in_dependency {
                if let Some(artifact) = current_artifact.take() {
                    let is_test = current_scope.as_deref() == Some("test");
                    if !is_test {
                        deps.push(DiscoveredDep {
                            name: artifact,
                            version_spec: None,
                        });
                    }
                }
            }
            in_dependency = false;
        }
        if in_dependency {
            if let Some(val) = extract_xml_value(trimmed, "artifactId") {
                current_artifact = Some(val);
            }
            if let Some(val) = extract_xml_value(trimmed, "scope") {
                current_scope = Some(val);
            }
        }
    }
    deps
}

fn extract_xml_value(line: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = line.find(&open)? + open.len();
    let end = line.find(&close)?;
    Some(line[start..end].to_string())
}

fn detect_jvm_language(project_root: &Path) -> &'static str {
    if project_root.join("build.gradle.kts").exists() {
        "kotlin"
    } else {
        "java"
    }
}

fn collect_jvm_deps(project_root: &Path, out: &mut Vec<(DiscoveredDep, String, Option<PathBuf>)>) {
    let language = detect_jvm_language(project_root);

    // Try Gradle first, then Maven
    let deps = if let Ok(content) = std::fs::read_to_string(project_root.join("build.gradle.kts"))
        .or_else(|_| std::fs::read_to_string(project_root.join("build.gradle")))
    {
        parse_gradle_deps(&content)
    } else if let Ok(content) = std::fs::read_to_string(project_root.join("pom.xml")) {
        parse_maven_deps(&content)
    } else {
        return;
    };

    // JVM deps are always registered without source
    for dep in deps {
        out.push((dep, language.to_string(), None));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Cargo tests ─────────────────────────────────────────────────────

    #[test]
    fn parse_cargo_deps_basic() {
        let toml = r#"
[package]
name = "foo"

[dependencies]
serde = "1.0"
tokio = { version = "1", features = ["full"] }
anyhow = "1"

[dev-dependencies]
tempfile = "3"

[build-dependencies]
build_script = "0.1"
"#;
        let deps = parse_cargo_deps(toml);
        let names: Vec<&str> = deps.iter().map(|d| d.name.as_str()).collect();
        // Direct deps present
        assert!(names.contains(&"serde"));
        assert!(names.contains(&"tokio"));
        assert!(names.contains(&"anyhow"));
        // dev-deps and build-deps must NOT be included
        assert!(!names.contains(&"tempfile"));
        assert!(!names.contains(&"build_script"));
    }

    #[test]
    fn parse_cargo_deps_normalises_hyphens() {
        let toml = r#"
[dependencies]
my-crate = "1.0"
"#;
        let deps = parse_cargo_deps(toml);
        assert!(
            deps.iter().any(|d| d.name == "my_crate"),
            "hyphens should be normalised to underscores"
        );
    }

    // ── Node tests ──────────────────────────────────────────────────────

    #[test]
    fn parse_node_deps_basic() {
        let json = r#"{
            "dependencies": { "express": "^4.18.0", "lodash": "4.17.21" },
            "devDependencies": { "jest": "^29.0.0" }
        }"#;
        let deps = parse_node_deps(json);
        assert_eq!(deps.len(), 2);
        assert!(deps.iter().any(|d| d.name == "express"));
        assert!(deps.iter().any(|d| d.name == "lodash"));
        // devDependencies excluded
        assert!(!deps.iter().any(|d| d.name == "jest"));
    }

    #[test]
    fn parse_node_deps_scoped_packages() {
        let json = r#"{"dependencies": {"@babel/core": "^7.0.0", "@types/node": "^20.0.0"}}"#;
        let deps = parse_node_deps(json);
        assert_eq!(deps.len(), 2);
        assert!(deps.iter().any(|d| d.name == "@babel/core"));
    }

    #[test]
    fn find_node_source_in_node_modules() {
        let dir = tempfile::tempdir().unwrap();
        let nm = dir.path().join("node_modules").join("express");
        std::fs::create_dir_all(&nm).unwrap();
        std::fs::write(nm.join("package.json"), "{}").unwrap();
        assert_eq!(find_node_source(dir.path(), "express"), Some(nm));
    }

    #[test]
    fn find_node_source_scoped() {
        let dir = tempfile::tempdir().unwrap();
        let nm = dir.path().join("node_modules").join("@babel").join("core");
        std::fs::create_dir_all(&nm).unwrap();
        std::fs::write(nm.join("package.json"), "{}").unwrap();
        assert_eq!(find_node_source(dir.path(), "@babel/core"), Some(nm));
    }

    // ── Python tests ────────────────────────────────────────────────────

    #[test]
    fn normalize_python_name_basic() {
        assert_eq!(normalize_python_name("My-Package"), "my_package");
        assert_eq!(normalize_python_name("zope.interface"), "zope_interface");
        assert_eq!(normalize_python_name("foo_bar"), "foo_bar");
        assert_eq!(normalize_python_name("Flask-RESTful"), "flask_restful");
        assert_eq!(normalize_python_name("a--b..c__d"), "a_b_c_d");
    }

    #[test]
    fn parse_python_deps_pyproject_basic() {
        let toml = r#"
[project]
dependencies = [
    "requests>=2.28,<3",
    "numpy[extra1]>=1.24",
    "importlib-metadata; python_version < '3.8'",
    "flask",
]
"#;
        let deps = parse_python_deps_pyproject(toml);
        assert_eq!(deps.len(), 4);
        assert!(deps.iter().any(|d| d.name == "requests"));
        assert!(deps.iter().any(|d| d.name == "numpy"));
        assert!(deps.iter().any(|d| d.name == "importlib_metadata"));
        assert!(deps.iter().any(|d| d.name == "flask"));
    }

    #[test]
    fn parse_python_deps_requirements_basic() {
        let txt =
            "requests>=2.28\n# comment\n-r other.txt\n-e ./local\ngit+https://foo\nflask==2.0\n";
        let deps = parse_python_deps_requirements(txt);
        assert_eq!(deps.len(), 2);
        assert!(deps.iter().any(|d| d.name == "requests"));
        assert!(deps.iter().any(|d| d.name == "flask"));
    }

    #[test]
    fn find_python_source_in_venv() {
        let dir = tempfile::tempdir().unwrap();
        let sp = dir
            .path()
            .join(".venv/lib/python3.11/site-packages/requests");
        std::fs::create_dir_all(&sp).unwrap();
        assert_eq!(find_python_source(dir.path(), "requests"), Some(sp));
    }

    // ── Go tests ────────────────────────────────────────────────────────

    #[test]
    fn parse_go_deps_basic() {
        let gomod = r#"
module github.com/myorg/myapp

go 1.21

require (
	github.com/gin-gonic/gin v1.9.1
	golang.org/x/sync v0.5.0
)

require (
	github.com/indirect/dep v0.1.0 // indirect
)
"#;
        let deps = parse_go_deps(gomod);
        // Include both direct and indirect
        assert!(deps.len() >= 2);
        assert!(deps.iter().any(|d| d.name == "github.com/gin-gonic/gin"));
        assert!(deps.iter().any(|d| d.name == "golang.org/x/sync"));
    }

    #[test]
    fn go_encode_module_path_basic() {
        assert_eq!(
            go_encode_module_path("github.com/Azure/azure-sdk"),
            "github.com/!azure/azure-sdk"
        );
        assert_eq!(
            go_encode_module_path("github.com/foo/bar"),
            "github.com/foo/bar"
        );
    }

    #[test]
    fn find_go_source_in_modcache() {
        let dir = tempfile::tempdir().unwrap();
        let mod_dir = dir.path().join("github.com/gin-gonic/gin@v1.9.1");
        std::fs::create_dir_all(&mod_dir).unwrap();
        std::fs::write(mod_dir.join("go.mod"), "module gin").unwrap();
        assert!(find_go_source(dir.path(), "github.com/gin-gonic/gin").is_some());
    }
    #[test]
    fn go_mod_cache_from_resolves_in_order() {
        use super::go_mod_cache_from;
        let home = Some(PathBuf::from("/home/u"));

        // 1. GOMODCACHE wins outright, even when GOPATH is also set.
        let v = go_mod_cache_from(
            |k| match k {
                "GOMODCACHE" => Some("/explicit/modcache".to_string()),
                "GOPATH" => Some("/some/gopath".to_string()),
                _ => None,
            },
            home.clone(),
        );
        assert_eq!(v, Some(PathBuf::from("/explicit/modcache")));

        // 2. No GOMODCACHE → GOPATH/pkg/mod.
        let v = go_mod_cache_from(
            |k| (k == "GOPATH").then(|| "/my/gopath".to_string()),
            home.clone(),
        );
        assert_eq!(v, Some(PathBuf::from("/my/gopath/pkg/mod")));

        // 3. Neither set → <home>/go/pkg/mod default.
        let v = go_mod_cache_from(|_| None, home);
        assert_eq!(v, Some(PathBuf::from("/home/u/go/pkg/mod")));

        // 4. Empty env values are treated as unset.
        let v = go_mod_cache_from(
            |k| matches!(k, "GOMODCACHE" | "GOPATH").then(String::new),
            Some(PathBuf::from("/h")),
        );
        assert_eq!(v, Some(PathBuf::from("/h/go/pkg/mod")));

        // 5. Nothing set and no home → None (cannot derive, caller skips Go).
        assert_eq!(go_mod_cache_from(|_| None, None), None);
    }

    // ── Java/Kotlin tests ───────────────────────────────────────────────

    #[test]
    fn parse_gradle_deps_kotlin_dsl() {
        let gradle = r#"
dependencies {
    implementation("com.fasterxml.jackson.core:jackson-databind:2.15.0")
    api("org.jetbrains.kotlin:kotlin-stdlib:1.9.0")
    testImplementation("junit:junit:4.13.2")
    compileOnly("org.projectlombok:lombok:1.18.30")
}
"#;
        let deps = parse_gradle_deps(gradle);
        assert!(deps.iter().any(|d| d.name == "jackson-databind"));
        assert!(deps.iter().any(|d| d.name == "kotlin-stdlib"));
        assert!(deps.iter().any(|d| d.name == "lombok"));
        // testImplementation is excluded
        assert!(!deps.iter().any(|d| d.name == "junit"));
    }

    #[test]
    fn parse_gradle_deps_groovy_dsl() {
        let gradle = "dependencies {\n    implementation 'com.google.guava:guava:32.1.2-jre'\n}\n";
        let deps = parse_gradle_deps(gradle);
        assert!(deps.iter().any(|d| d.name == "guava"));
    }

    #[test]
    fn parse_maven_deps_basic() {
        let pom = r#"
<dependencies>
    <dependency>
        <groupId>com.fasterxml.jackson.core</groupId>
        <artifactId>jackson-databind</artifactId>
        <version>2.15.0</version>
    </dependency>
    <dependency>
        <groupId>junit</groupId>
        <artifactId>junit</artifactId>
        <scope>test</scope>
    </dependency>
</dependencies>
"#;
        let deps = parse_maven_deps(pom);
        assert!(deps.iter().any(|d| d.name == "jackson-databind"));
        // test scope excluded
        assert!(!deps.iter().any(|d| d.name == "junit"));
    }

    // ── Integration tests ───────────────────────────────────────────────

    #[tokio::test]
    async fn auto_register_deps_multi_ecosystem() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();

        // Cargo.toml
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname=\"test\"\n\n[dependencies]\nserde = \"1\"\n",
        )
        .unwrap();

        // package.json
        std::fs::write(
            root.join("package.json"),
            r#"{"dependencies":{"express":"^4.0"}}"#,
        )
        .unwrap();

        // node_modules for express
        let nm = root.join("node_modules/express");
        std::fs::create_dir_all(&nm).unwrap();
        std::fs::write(nm.join("package.json"), "{}").unwrap();

        // build.gradle.kts (no source available)
        std::fs::write(
            root.join("build.gradle.kts"),
            "dependencies {\n    implementation(\"com.google:guava:32.0\")\n}\n",
        )
        .unwrap();

        let agent = crate::agent::Agent::new(Some(root.to_path_buf()))
            .await
            .unwrap();
        let ctx = crate::tools::ToolContext {
            agent,
            lsp: crate::lsp::mock::MockLspProvider::with_client(
                crate::lsp::mock::MockLspClient::default(),
            ),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
            workspace_override: None,
        };

        let registered = auto_register_deps(root, &ctx).await;

        // Should have deps from multiple ecosystems
        assert!(
            registered
                .iter()
                .any(|r| r.name == "express" && r.source_available),
            "express should be registered with source"
        );
        assert!(
            registered
                .iter()
                .any(|r| r.name == "guava" && !r.source_available),
            "guava should be registered without source"
        );
    }

    #[tokio::test]
    async fn auto_register_deps_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        std::fs::write(
            root.join("package.json"),
            r#"{"dependencies":{"express":"^4.0"}}"#,
        )
        .unwrap();
        let nm = root.join("node_modules/express");
        std::fs::create_dir_all(&nm).unwrap();
        std::fs::write(nm.join("package.json"), "{}").unwrap();

        let agent = crate::agent::Agent::new(Some(root.to_path_buf()))
            .await
            .unwrap();
        let ctx = crate::tools::ToolContext {
            agent,
            lsp: crate::lsp::mock::MockLspProvider::with_client(
                crate::lsp::mock::MockLspClient::default(),
            ),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
            workspace_override: None,
        };

        let first = auto_register_deps(root, &ctx).await;
        let second = auto_register_deps(root, &ctx).await;

        // Second call registers nothing new
        assert!(!first.is_empty());
        assert!(
            second.is_empty(),
            "second activation should not re-register"
        );

        // Registry has exactly one entry per dep
        let count = ctx
            .agent
            .with_project(|p| {
                Ok(p.library_registry
                    .all()
                    .iter()
                    .filter(|e| e.name == "express")
                    .count())
            })
            .await
            .unwrap();
        assert_eq!(count, 1);
    }
}
