use super::*;

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    materialize_fixture(dir.path());
    dir
}

#[test]
fn repository_toolchain_selection_requires_a_managed_toolchain_name() {
    for filename in ["rust-toolchain", "rust-toolchain.toml"] {
        for channel in ["1.99.1", "stable", "custom-toolchain"] {
            let dir = fixture();
            std::fs::write(
                dir.path().join(filename),
                format!("[toolchain]\nchannel=\"{channel}\"\n"),
            )
            .unwrap();
            assert_eq!(
                verify(
                    dir.path(),
                    &serde_json::json!({"command":"bash scripts/fmt-mine.sh"})
                )
                .is_ok(),
                channel != "custom-toolchain",
                "{filename}: {channel}"
            );
        }
    }
}

#[test]
fn helper_packages_and_cached_bytecode_cannot_replace_verified_sources() {
    for directory in ["", "scripts"] {
        for variant in ["package", "cache", "bytecode", "extension"] {
            let dir = fixture();
            let directory = dir.path().join(directory);
            match variant {
                "package" => {
                    let package = directory.join("native_provenance_history");
                    std::fs::create_dir_all(&package).unwrap();
                    std::fs::write(package.join("__init__.py"), "# harmless import fixture")
                        .unwrap();
                }
                "cache" => {
                    std::fs::create_dir_all(directory.join("__pycache__")).unwrap();
                }
                "bytecode" => {
                    std::fs::write(
                        directory.join("native_provenance_history.pyc"),
                        "harmless fixture",
                    )
                    .unwrap();
                }
                "extension" => {
                    std::fs::write(
                        directory.join("native_provenance_history.so"),
                        "harmless fixture",
                    )
                    .unwrap();
                }
                _ => unreachable!(),
            }
            assert!(
                verify(
                    dir.path(),
                    &serde_json::json!({"command":"bash scripts/fmt-mine.sh"})
                )
                .is_err(),
                "accepted {variant} in {}",
                directory.display()
            );
        }
    }
}

#[test]
fn exact_shipped_formatter_and_helpers_are_required() {
    let dir = fixture();
    verify(
        dir.path(),
        &serde_json::json!({"command":"bash scripts/fmt-mine.sh"}),
    )
    .unwrap();
    std::fs::write(dir.path().join("scripts/fmt_paths.py"), "alternate helper").unwrap();
    assert!(verify(
        dir.path(),
        &serde_json::json!({"command":"bash scripts/fmt-mine.sh"})
    )
    .is_err());
}

#[test]
fn alternate_entrypoint_cwd_and_shadowed_imports_are_not_exempt() {
    for variant in [
        "entry",
        "cwd",
        "root-import",
        "sibling-import",
        "cached-import",
        "cargo-config",
        "rustup-config",
    ] {
        let dir = fixture();
        let mut input = serde_json::json!({"command":"bash scripts/fmt-mine.sh"});
        match variant {
            "rustup-config" => {
                std::fs::write(
                    dir.path().join("rust-toolchain.toml"),
                    "[toolchain]\npath=\"./untrusted\"\n",
                )
                .unwrap();
            }
            "entry" => {
                std::fs::write(dir.path().join("scripts/fmt-mine.sh"), "alternate entry").unwrap()
            }
            "cwd" => {
                input["cwd"] = serde_json::json!(dir.path());
            }
            "root-import" => {
                std::fs::write(dir.path().join("json.py"), "alternate import").unwrap()
            }
            "sibling-import" => {
                std::fs::write(dir.path().join("scripts/pathlib.py"), "alternate import").unwrap()
            }
            "cached-import" => std::fs::write(
                dir.path().join("scripts/hashlib.pyc"),
                "alternate cached import",
            )
            .unwrap(),
            "cargo-config" => {
                std::fs::create_dir(dir.path().join(".cargo")).unwrap();
                std::fs::write(
                    dir.path().join(".cargo/config.toml"),
                    "[alias]\nfmt='another-command'",
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
        assert!(verify(dir.path(), &input).is_err(), "{variant}");
    }
}

#[test]
fn formatter_environment_excludes_startup_and_helper_overrides() {
    let mut command = std::process::Command::new("bash");
    command
        .env("CODEX_SESSION_ID", "native-root")
        .env("CODEX_THREAD_ID", "native-thread")
        .env("BASH_ENV", "alternate-startup")
        .env("FMT_MINE_PROVENANCE", "alternate-helper")
        .env("PYTHONPATH", "alternate-python")
        .env("RUSTFMT", "alternate-rustfmt");
    apply_environment(&mut command);
    let env: std::collections::BTreeMap<_, _> = command
        .get_envs()
        .filter_map(|(k, v)| v.map(|v| (k.to_owned(), v.to_owned())))
        .collect();
    assert_eq!(
        env.get(std::ffi::OsStr::new("CODEX_SESSION_ID")),
        Some(&std::ffi::OsString::from("native-root"))
    );
    for name in ["BASH_ENV", "FMT_MINE_PROVENANCE", "PYTHONPATH", "RUSTFMT"] {
        assert!(!env.contains_key(std::ffi::OsStr::new(name)), "{name}");
    }
    assert_eq!(
        env.get(std::ffi::OsStr::new("PYTHONNOUSERSITE")),
        Some(&std::ffi::OsString::from("1"))
    );
    assert_eq!(
        env.get(std::ffi::OsStr::new("PYTHONDONTWRITEBYTECODE")),
        Some(&std::ffi::OsString::from("1"))
    );
    assert_eq!(
        env.get(std::ffi::OsStr::new("PYTHONSAFEPATH")),
        Some(&std::ffi::OsString::from("1"))
    );
}
