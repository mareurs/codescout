"""Run the Python runner and local gate against isolated fixture trees."""
import os
from pathlib import Path
import shutil
import subprocess
import sys

import pytest

ROOT = Path(__file__).resolve().parents[1]
RUNNER = ROOT / "scripts/python-tests.py"
HEAVY = (
    "test_phase1b_step4.py",
    "test_phase1b_step5.py",
    "test_phase1b_training.py",
)


@pytest.fixture
def project(tmp_path):
    tests = tmp_path / "tests"
    tests.mkdir()
    for name in ("test_alpha.py", "test_future_suite.py", *HEAVY):
        (tests / name).write_text(
            "from pathlib import Path\n"
            "def test_runs():\n"
            "    with Path('ran.txt').open('a') as stream:\n"
            f"        stream.write({name!r} + '\\n')\n"
        )
    # The fixture suites use no ML code. Stub only the optional dependency boundary.
    modules = tmp_path / "modules"
    modules.mkdir()
    (modules / "torch.py").write_text("")
    (modules / "sklearn.py").write_text("")
    return tmp_path


def run_runner(project, *args, no_site=False):
    env = os.environ.copy()
    inherited_path = "" if no_site else env.get("PYTHONPATH", "")
    env["PYTHONPATH"] = os.pathsep.join(
        path for path in (str(project / "modules"), inherited_path) if path
    )
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    env["PYTEST_DISABLE_PLUGIN_AUTOLOAD"] = "1"
    command = [sys.executable]
    if no_site:
        command.append("-S")
    return subprocess.run(
        [*command, str(RUNNER), "--root", str(project), *args],
        env=env, text=True, capture_output=True, timeout=30,
    )


@pytest.mark.parametrize(("lane", "selected", "excluded"), [
    ("light", ["test_alpha.py", "test_future_suite.py"], list(HEAVY)),
    ("heavy", list(HEAVY), ["test_alpha.py", "test_future_suite.py"]),
    ("all", ["test_alpha.py", "test_future_suite.py", *HEAVY], []),
])
def test_lanes_execute_their_files_and_report_the_partition(project, lane, selected, excluded):
    result = run_runner(project, "--lane", lane)
    assert result.returncode == 0, result.stdout + result.stderr
    assert (project / "ran.txt").read_text().splitlines() == selected
    assert f"Selected ({len(selected)} files):" in result.stdout
    assert f"Excluded ({len(excluded)} files):" in result.stdout
    selected_report, excluded_report = result.stdout.split("Excluded (", 1)
    for name in selected:
        assert "tests/" + name in selected_report
    for name in excluded:
        assert "tests/" + name in excluded_report


def test_default_lane_discovers_future_test_files(project):
    result = run_runner(project)
    assert result.returncode == 0, result.stdout + result.stderr
    assert (project / "ran.txt").read_text().splitlines() == [
        "test_alpha.py", "test_future_suite.py",
    ]


@pytest.mark.parametrize("lane", ["light", "heavy", "all"])
def test_deleted_heavy_inventory_fails_before_running_any_lane(project, lane):
    (project / "tests" / HEAVY[1]).unlink()
    result = run_runner(project, "--lane", lane)
    assert result.returncode == 2
    assert "tests/test_phase1b_step5.py" in result.stderr
    assert "missing" in result.stderr.lower()
    assert not (project / "ran.txt").exists()


def test_empty_light_lane_is_an_error(project):
    for name in ("test_alpha.py", "test_future_suite.py"):
        (project / "tests" / name).unlink()
    result = run_runner(project)
    assert result.returncode == 2
    assert "No test files selected" in result.stderr
    assert not (project / "ran.txt").exists()


@pytest.mark.parametrize(("source", "exit_code", "detail"), [
    ("def test_fails():\n    assert False, 'fixture failure'\n", 1, "fixture failure"),
    ("this is invalid python !!!\n", 2, "SyntaxError"),
])
def test_pytest_failure_and_collection_error_propagate(project, source, exit_code, detail):
    (project / "tests" / "test_broken.py").write_text(source)
    result = run_runner(project)
    assert result.returncode == exit_code, result.stdout + result.stderr
    assert detail in result.stdout + result.stderr


@pytest.mark.parametrize(("lane", "stub_pytest", "missing", "requirements"), [
    ("light", False, ["pytest"], "requirements-python-light.txt"),
    ("heavy", True, ["torch", "sklearn"], "requirements-python-heavy.txt"),
])
def test_missing_dependencies_fail_with_an_install_hint(project, lane, stub_pytest, missing, requirements):
    shutil.rmtree(project / "modules")
    (project / "modules").mkdir()
    if stub_pytest:
        (project / "modules" / "pytest.py").write_text("")
    result = run_runner(project, "--lane", lane, no_site=True)
    assert result.returncode == 2
    for module in missing:
        assert module in result.stderr
    assert "pip install" in result.stderr
    assert requirements in result.stderr
    assert not (project / "ran.txt").exists()


def gate_fixture(tmp_path):
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    for name in ("gate.sh", "session_context.py"):
        shutil.copyfile(ROOT / "scripts" / name, scripts / name)
    shutil.copyfile(ROOT / "scripts/session-context-v1.schema.json", scripts / "session-context-v1.schema.json")
    subprocess.run(["git", "init", "-q", str(tmp_path)], check=True)
    (scripts / "slot-pool.sh").write_text("# Fixture supplies CARGO_TARGET_DIR.\n")
    fmt = scripts / "fmt-mine.sh"
    fmt.write_text(
        "#!/usr/bin/env bash\n"
        "echo \"fmt $*\" >> \"$CALLS\"\n"
        "touch \"$FORMAT_MARKER\"\n"
        "if [ -n \"${CODESCOUT_CONTEXT_FILE:-}\" ]; then\n"
        "  \"$REAL_PYTHON\" -c 'import json,os,pathlib; p=pathlib.Path(os.environ[\"CODESCOUT_CONTEXT_FILE\"]); value={\"path\":str(p),\"mode\":p.stat().st_mode & 511,\"context\":json.loads(p.read_text())}; pathlib.Path(os.environ[\"SNAPSHOT_RECORD\"]).write_text(json.dumps(value))'\n"
        "  \"$REAL_PYTHON\" ./scripts/session_context.py --workspace \"$PWD\" >/dev/null || exit 2\n"
        "fi\n"
        "exit \"$FMT_EXIT\"\n"
    )
    fmt.chmod(0o755)
    commands = tmp_path / "bin"
    commands.mkdir()
    cargo = commands / "cargo"
    cargo.write_text(
        "#!/usr/bin/env bash\n"
        "echo \"cargo $*\" >> \"$CALLS\"\n"
        "echo \"$CARGO_TARGET_DIR\" >> \"$TARGETS\"\n"
        "lane=default\n"
        "[ \"$1\" = clippy ] && lane=clippy\n"
        "case \"$*\" in *--no-default-features*) lane=lean ;; esac\n"
        "if [ \"$lane\" = default ] && [ -n \"${HOLD_DEFAULT:-}\" ]; then\n"
        "  touch \"$DEFAULT_STARTED\"\n"
        "  while [ -e \"$HOLD_DEFAULT\" ]; do sleep 0.05; done\n"
        "fi\n"
        "[ \"$FAILED_LANE\" != \"$lane\" ]\n"
    )
    cargo.chmod(0o755)
    python = commands / "python3"
    python.write_text(
        "#!/usr/bin/env bash\n"
        "if [ \"${1:-}\" != ./scripts/python-tests.py ]; then exec \"$REAL_PYTHON\" \"$@\"; fi\n"
        "echo \"python3 $*\" >> \"$CALLS\"\n"
        "[ \"$FAILED_LANE\" != python ]\n"
    )
    python.chmod(0o755)
    env = os.environ.copy()
    for key in ("CLAUDE_CODE_SESSION_ID", "CODEX_SESSION_ID", "CODEX_THREAD_ID",
                "CODESCOUT_CONTEXT_FILE", "SLOT_FD", "HOLD_DEFAULT"):
        env.pop(key, None)
    env.update(
        CARGO_TARGET_DIR=str(tmp_path / "target"), SLOT_FD="",
        CALLS=str(tmp_path / "calls.txt"), TARGETS=str(tmp_path / "targets.txt"),
        FORMAT_MARKER=str(tmp_path / "format-called"), SNAPSHOT_RECORD=str(tmp_path / "snapshot.json"),
        FMT_EXIT="0", FAILED_LANE="none", REAL_PYTHON=sys.executable,
        PATH=str(commands) + os.pathsep + env["PATH"],
    )
    return tmp_path, env


def run_gate_fixture(fixture, *args, **changes):
    root, env = fixture
    return subprocess.run(["bash", str(root / "scripts/gate.sh"), *args], cwd=root,
                          env=env | changes, text=True, capture_output=True, timeout=30)


def assert_gate_calls(root, fmt_args=None):
    expected = ["python3 ./scripts/python-tests.py --lane light"]
    if fmt_args is not None:
        expected.append("fmt " + fmt_args)
    expected += [
        "cargo clippy --workspace --all-targets --features local-embed -- -D warnings",
        "cargo test --workspace --no-default-features",
        "cargo test --workspace",
    ]
    assert (root / "calls.txt").read_text().splitlines() == expected
    assert (root / "targets.txt").read_text().splitlines() == [str(root / "target")] * 3


@pytest.mark.parametrize("failed_lane", ["none", "python", "fmt", "clippy", "lean", "default"])
@pytest.mark.parametrize("mode", ["claude-code", "codex", "check"])
def test_gate_runs_every_lane_and_keeps_default_cargo_last(tmp_path, failed_lane, mode):
    fixture = gate_fixture(tmp_path)
    root, env = fixture
    env["FAILED_LANE"] = failed_lane
    env["FMT_EXIT"] = "1" if failed_lane == "fmt" else "0"
    args = ("--check",) if mode == "check" else ()
    if mode != "check":
        env["CLAUDE_CODE_SESSION_ID" if mode == "claude-code" else "CODEX_SESSION_ID"] = "runner-gate-fixture"
    result = run_gate_fixture(fixture, *args)
    assert_gate_calls(root, "--check" if mode == "check" else "")
    assert result.returncode == (0 if failed_lane == "none" else 1), result.stderr
    fields = {"PYTHON_LIGHT": "python", "FMT": "fmt", "CLIPPY": "clippy", "LEAN": "lean", "DEFAULT": "default"}
    for field, lane in fields.items():
        assert f"{field}={1 if failed_lane == lane else 0}" in result.stdout
    state = ("differences" if mode == "check" else "ownership-refused") if failed_lane == "fmt" else "passed"
    assert f"FMT_STATE={state}" in result.stdout
    if mode != "check":
        import json
        record = json.loads((root / "snapshot.json").read_text())
        assert record["mode"] == 0o600
        assert record["context"]["harness"] == mode
        assert not Path(record["path"]).is_relative_to(root)
        assert not Path(record["path"]).exists()


@pytest.mark.parametrize("context", ["missing", "conflicting", "unsupported"])
def test_gate_context_block_keeps_all_other_lanes(tmp_path, context):
    fixture = gate_fixture(tmp_path)
    root, env = fixture
    if context == "conflicting":
        env.update(CLAUDE_CODE_SESSION_ID="a", CODEX_SESSION_ID="b")
    elif context == "unsupported":
        env["CODESCOUT_CONTEXT_FILE"] = str(root / "missing-context.json")
    result = run_gate_fixture(fixture)
    assert_gate_calls(root)
    assert result.returncode == 1
    assert "FMT=2" in result.stdout and "FMT_STATE=context-blocked" in result.stdout
    assert not (root / "format-called").exists()


def test_explicit_context_gate_forwards_snapshot_only(tmp_path):
    import json
    fixture = gate_fixture(tmp_path)
    root, _ = fixture
    context = root / "explicit context.json"
    context.write_text(json.dumps({"schema_version": 1, "harness": "codex", "source": "explicit",
                                  "session_id": "native-id", "agent_id": None, "workspace": str(root),
                                  "harness_version": None}))
    result = run_gate_fixture(fixture, "--context", str(context))
    assert result.returncode == 0, result.stderr
    assert_gate_calls(root, "")
    record = json.loads((root / "snapshot.json").read_text())
    assert record["path"] != str(context)
    assert not Path(record["path"]).exists()
    assert context.exists()


@pytest.mark.parametrize("conflict", [False, True])
def test_gate_exclusive_authorization_with_missing_or_conflicting_identity(tmp_path, conflict):
    fixture = gate_fixture(tmp_path)
    root, env = fixture
    if conflict:
        env.update(CLAUDE_CODE_SESSION_ID="a", CODEX_SESSION_ID="b")
    result = run_gate_fixture(fixture, "--exclusive-checkout", str(root))
    assert_gate_calls(root, None if conflict else "--exclusive-checkout " + str(root))
    assert result.returncode == (1 if conflict else 0)
    assert f"FMT_STATE={'context-blocked' if conflict else 'passed'}" in result.stdout


@pytest.mark.parametrize("mode", ["check", "authored"])
def test_gate_formatter_tool_error_is_reported_without_skipping_default(tmp_path, mode):
    fixture = gate_fixture(tmp_path)
    result = run_gate_fixture(fixture, *(("--check",) if mode == "check" else ()),
                              FMT_EXIT="2", **({"CODEX_SESSION_ID": "me"} if mode == "authored" else {}))
    assert result.returncode == 1
    assert "FMT=2" in result.stdout and "FMT_STATE=tool-error" in result.stdout
    assert_gate_calls(tmp_path, "--check" if mode == "check" else "")


@pytest.mark.parametrize("args", [("--check", "--check"), ("--bogus",), ("--context",),
                                   ("--check", "--exclusive-checkout", "/tmp"),
                                   ("--exclusive-checkout", "relative"), ("--exclusive-checkout", "/tmp")])
def test_gate_invalid_entry_runs_no_lanes(tmp_path, args):
    fixture = gate_fixture(tmp_path)
    result = run_gate_fixture(fixture, *args)
    assert result.returncode == 2
    assert not (tmp_path / "calls.txt").exists()
def test_interrupted_gate_cleans_private_context_snapshot(tmp_path):
    import json
    import signal
    import time
    fixture = gate_fixture(tmp_path)
    root, env = fixture
    hold = root / "hold-default"
    started = root / "default-started"
    hold.touch()
    env.update(CODEX_SESSION_ID="me", HOLD_DEFAULT=str(hold), DEFAULT_STARTED=str(started))
    process = subprocess.Popen(["bash", str(root / "scripts/gate.sh")], cwd=root, env=env,
                               text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        deadline = time.monotonic() + 10
        while not started.exists() and process.poll() is None and time.monotonic() < deadline:
            time.sleep(0.05)
        assert started.exists()
        record = json.loads((root / "snapshot.json").read_text())
        snapshot = Path(record["path"])
        assert snapshot.exists()
        process.send_signal(signal.SIGTERM)
        hold.unlink()
        process.communicate(timeout=10)
        assert process.returncode == 143
        assert not snapshot.exists()
        assert_gate_calls(root, "")
    finally:
        hold.unlink(missing_ok=True)
        if process.poll() is None:
            process.kill()
            process.communicate(timeout=10)
