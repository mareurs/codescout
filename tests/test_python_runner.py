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


@pytest.mark.parametrize("failed_lane", ["none", "python", "fmt", "clippy", "lean", "default"])
def test_gate_runs_every_lane_and_keeps_default_cargo_last(tmp_path, failed_lane):
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    shutil.copyfile(ROOT / "scripts/gate.sh", scripts / "gate.sh")
    (scripts / "slot-pool.sh").write_text("# Fixture supplies CARGO_TARGET_DIR.\n")
    fmt = scripts / "fmt-mine.sh"
    fmt.write_text(
        "#!/usr/bin/env bash\n"
        "echo fmt >> \"$CALLS\"\n"
        "[ \"$FAILED_LANE\" != fmt ]\n"
    )
    fmt.chmod(0o755)
    commands = tmp_path / "bin"
    commands.mkdir()
    cargo = commands / "cargo"
    cargo.write_text(
        "#!/usr/bin/env bash\n"
        "echo \"cargo $*\" >> \"$CALLS\"\n"
        "lane=default\n"
        "[ \"$1\" = clippy ] && lane=clippy\n"
        "case \"$*\" in *--no-default-features*) lane=lean ;; esac\n"
        "[ \"$FAILED_LANE\" != \"$lane\" ]\n"
    )
    cargo.chmod(0o755)
    python = commands / "python3"
    python.write_text(
        "#!/usr/bin/env bash\n"
        "echo \"python3 $*\" >> \"$CALLS\"\n"
        "[ \"$FAILED_LANE\" != python ]\n"
    )
    python.chmod(0o755)
    env = os.environ.copy()
    env.update(
        CLAUDE_CODE_SESSION_ID="runner-gate-fixture",
        CARGO_TARGET_DIR=str(tmp_path / "target"),
        SLOT_FD="",
        CALLS=str(tmp_path / "calls.txt"),
        FAILED_LANE=failed_lane,
        PATH=str(commands) + os.pathsep + env["PATH"],
    )
    result = subprocess.run(
        ["bash", str(scripts / "gate.sh")], cwd=tmp_path, env=env,
        text=True, capture_output=True, timeout=30,
    )
    assert (tmp_path / "calls.txt").read_text().splitlines() == [
        "python3 ./scripts/python-tests.py --lane light",
        "fmt",
        "cargo clippy --workspace --all-targets --features local-embed -- -D warnings",
        "cargo test --workspace --no-default-features",
        "cargo test --workspace",
    ]
    assert result.returncode == (0 if failed_lane == "none" else 1)
    assert f"PYTHON_LIGHT={1 if failed_lane == 'python' else 0}" in result.stdout
