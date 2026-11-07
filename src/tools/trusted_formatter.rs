//! Only the shipped foreground formatter closure may avoid an unresolved-shell marker.
//! Installed OS tools remain trusted; repository scripts and startup overrides do not.
use anyhow::{bail, ensure, Context};
use serde_json::Value;
use std::path::{Path, PathBuf};

const SCRIPTS: &[(&str, &str)] = &[
    (
        "fmt-mine.sh",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/fmt-mine.sh")),
    ),
    (
        "fmt_paths.py",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/fmt_paths.py")),
    ),
    (
        "session_context.py",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/session_context.py"
        )),
    ),
    (
        "file-provenance.py",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/file-provenance.py"
        )),
    ),
    (
        "claude_history.py",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/claude_history.py"
        )),
    ),
    (
        "codex_history.py",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/codex_history.py"
        )),
    ),
    (
        "provenance_records.py",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/provenance_records.py"
        )),
    ),
    (
        "history_cache.py",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/history_cache.py"
        )),
    ),
    (
        "write_targets.py",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/write_targets.py"
        )),
    ),
    (
        "native_provenance.py",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/native_provenance.py"
        )),
    ),
    (
        "native_provenance_policy.py",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/native_provenance_policy.py"
        )),
    ),
    (
        "native_provenance_fence.py",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/native_provenance_fence.py"
        )),
    ),
    (
        "native_provenance_history.py",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/native_provenance_history.py"
        )),
    ),
];

pub(crate) fn verify(root: &Path, input: &Value) -> anyhow::Result<()> {
    ensure!(
        input.get("cwd").is_none(),
        "formatter cwd overrides are unsupported"
    );
    let allowed = [
        "command",
        "workspace",
        "effect",
        "timeout_secs",
        "acknowledge_risk",
        "run_in_background",
        "interactive",
    ];
    ensure!(
        input
            .as_object()
            .is_some_and(|map| map.keys().all(|key| allowed.contains(&key.as_str()))),
        "formatter request has unsupported parameters"
    );
    for (name, body) in SCRIPTS {
        let path = root.join("scripts").join(name);
        ensure!(
            path.canonicalize()?.starts_with(root),
            "formatter helper leaves the checkout: {name}"
        );
        ensure!(
            std::fs::symlink_metadata(&path)?.is_file(),
            "formatter helper is not a regular file: {name}"
        );
        ensure!(
            std::fs::read(&path)? == body.as_bytes(),
            "formatter helper differs from the shipped implementation: {name}"
        );
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    for parent in root.ancestors() {
        if home.as_deref() == Some(parent) {
            break;
        }
        for name in ["rust-toolchain", "rust-toolchain.toml"] {
            let path = parent.join(name);
            if !path.try_exists()? {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            let parsed = toml::from_str::<toml::Value>(&text).ok();
            let table = parsed.as_ref().and_then(|value| value.get("toolchain"));
            ensure!(
                table.and_then(|value| value.get("path")).is_none(),
                "repository custom toolchain paths are unsupported"
            );
            let channel = table
                .and_then(|value| value.get("channel"))
                .and_then(toml::Value::as_str)
                .or_else(|| (name == "rust-toolchain" && !text.contains('[')).then(|| text.trim()))
                .context("unsupported repository toolchain configuration")?;
            let parts = channel.split('.').collect::<Vec<_>>();
            ensure!(
                matches!(channel, "stable" | "beta" | "nightly")
                    || ((parts.len() == 2 || parts.len() == 3)
                        && parts[0] == "1"
                        && parts.iter().all(|part| !part.is_empty()
                            && part.bytes().all(|byte| byte.is_ascii_digit()))),
                "repository custom toolchain names are unsupported"
            );
        }
        for name in ["config", "config.toml"] {
            ensure!(
                !parent.join(".cargo").join(name).try_exists()?,
                "repository Cargo configuration is unsupported by native formatting"
            );
        }
    }
    if let Some(cargo) = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| home.map(|p| p.join(".cargo")))
    {
        for name in ["config", "config.toml"] {
            let path = cargo.join(name);
            if !path.try_exists()? {
                continue;
            }
            let value: toml::Value = toml::from_str(&std::fs::read_to_string(&path)?)?;
            ensure!(
                value.get("alias").and_then(|v| v.get("fmt")).is_none(),
                "Cargo fmt aliases are unsupported"
            );
            for key in ["rustc", "rustc-wrapper", "rustc-workspace-wrapper"] {
                ensure!(
                    value.get("build").and_then(|v| v.get(key)).is_none(),
                    "Cargo executable overrides are unsupported"
                );
            }
            if let Some(environment) = value.get("env").and_then(toml::Value::as_table) {
                for key in environment.keys() {
                    ensure!(
                        !matches!(key.as_str(), "PATH" | "HOME" | "ENV" | "CARGO_HOME")
                            && ![
                                "RUST",
                                "PYTHON",
                                "BASH",
                                "LD_",
                                "DYLD_",
                                "CARGO_ALIAS_",
                                "CARGO_BUILD_RUSTC"
                            ]
                            .iter()
                            .any(|prefix| key.starts_with(prefix)),
                        "Cargo environment controls formatter execution"
                    );
                }
            }
        }
    }
    let path = std::env::var_os("PATH").context("formatter PATH is missing")?;
    for component in std::env::split_paths(&path) {
        ensure!(
            component.is_absolute(),
            "formatter PATH contains a relative entry"
        );
        ensure!(
            !component.starts_with(root)
                && !component
                    .canonicalize()
                    .is_ok_and(|resolved| resolved.starts_with(root)),
            "formatter PATH resolves tools inside the checkout"
        );
    }
    let mut python = std::process::Command::new("python3");
    apply_environment(&mut python);
    let output=python.args(["-I","-c","import json,sys;print(json.dumps(sorted(sys.stdlib_module_names|{'sitecustomize','usercustomize'})))"]).output()?;
    ensure!(
        output.status.success(),
        "cannot inspect trusted Python import names"
    );
    let names: Vec<String> = serde_json::from_slice(&output.stdout)?;
    for directory in [root.to_path_buf(), root.join("scripts")] {
        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name();
            let text = name.to_string_lossy();
            let module = text.split('.').next().unwrap_or(&text);
            ensure!(
                text != "__pycache__",
                "repository Python caches are unsupported by native formatting"
            );
            if SCRIPTS
                .iter()
                .any(|(name, _)| name.strip_suffix(".py") == Some(module))
            {
                ensure!(
                    directory == root.join("scripts")
                        && SCRIPTS.iter().any(|(name, _)| *name == text),
                    "repository entry can replace a verified Python helper: {}",
                    path.display()
                );
            }
            if names.iter().any(|name| name == module) {
                bail!(
                    "repository entry can shadow a standard Python module: {}",
                    path.display()
                );
            }
        }
    }
    Ok(())
}

pub(crate) fn apply_environment(command: &mut std::process::Command) {
    let identity: Vec<_> = command
        .get_envs()
        .filter_map(|(key, value)| {
            ["CODEX_SESSION_ID", "CODEX_THREAD_ID", "CODEX_VERSION"]
                .iter()
                .any(|name| key == std::ffi::OsStr::new(name))
                .then(|| value.map(|value| (key.to_owned(), value.to_owned())))
                .flatten()
        })
        .collect();
    command.env_clear();
    for key in [
        "HOME",
        "PATH",
        "USER",
        "LOGNAME",
        "LANG",
        "LC_ALL",
        "TMPDIR",
        "TEMP",
        "TMP",
        "RUSTUP_HOME",
        "CARGO_HOME",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    for (key, value) in identity {
        command.env(key, value);
    }
    command
        .env("PYTHONNOUSERSITE", "1")
        .env("PYTHONSAFEPATH", "1")
        .env("PYTHONDONTWRITEBYTECODE", "1");
}

#[cfg(test)]
pub(crate) fn materialize_fixture(root: &Path) {
    std::fs::create_dir_all(root.join("scripts")).unwrap();
    for (name, body) in SCRIPTS {
        std::fs::write(root.join("scripts").join(name), body).unwrap();
    }
}

#[cfg(test)]
#[path = "trusted_formatter_tests.rs"]
mod tests;
