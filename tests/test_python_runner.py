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
        # The presence check is a subcommand lookup, not a lane: answered first, never recorded.
        "if [ \"$1\" = nextest ] && [ \"${2:-}\" = --version ]; then [ -z \"${NO_NEXTEST:-}\" ]; exit $?; fi\n"
        "echo \"cargo $*\" >> \"$CALLS\"\n"
        "echo \"$CARGO_TARGET_DIR\" >> \"$TARGETS\"\n"
        "[ -z \"${CARGO_STARTED:-}\" ] || touch \"$CARGO_STARTED\"\n"
        "echo \"${TMPDIR:-<unset>}\" >> \"$TMPDIRS\"\n"
        # TMPDIR_LITTER: leave a file in whatever TMPDIR this lane sees, as a test would.
        "[ -z \"${TMPDIR_LITTER:-}\" ] || touch \"$TMPDIR/litter-$$\"\n"
        "lane=default\n"
        "[ \"$1\" = clippy ] && lane=clippy\n"
        "case \"$*\" in *--no-default-features*) [ \"$1\" = clippy ] || lane=lean ;; esac\n"
        "case \"$*\" in *--doc*) lane=$lane-doc ;; esac\n"
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
        "echo FIXTURE-PYTHON-OUTPUT\n"
        # PY_WAIT_FOR_CARGO: record whether a cargo lane had started by the time this lane ran on.
        "if [ -n \"${PY_WAIT_FOR_CARGO:-}\" ]; then\n"
        "  seen=serial\n"
        "  for _ in $(seq 1 100); do [ -e \"$CARGO_STARTED\" ] && { seen=overlapped; break; }; sleep 0.05; done\n"
        "  echo \"$seen\" > \"$PY_RESULT\"\n"
        "fi\n"
        # PY_HOLD: stay alive, and say which process, until the named file is removed.
        "if [ -n \"${PY_HOLD:-}\" ]; then\n"
        "  echo $$ > \"$PY_PID_FILE\"\n"
        "  while [ -e \"$PY_HOLD\" ]; do sleep 0.05; done\n"
        "fi\n"
        "[ \"$FAILED_LANE\" != python ]\n"
    )
    python.chmod(0o755)
    findmnt = commands / "findmnt"
    findmnt.write_text("#!/usr/bin/env bash\necho \"${FIXTURE_FSTYPE:-tmpfs}\"\n")
    findmnt.chmod(0o755)
    (tmp_path / "orig-tmp").mkdir()
    (tmp_path / "tmpbase").mkdir()
    env = os.environ.copy()
    for key in ("CLAUDE_CODE_SESSION_ID", "CODEX_SESSION_ID", "CODEX_THREAD_ID",
                "CODESCOUT_CONTEXT_FILE", "SLOT_FD", "HOLD_DEFAULT", "PY_HOLD", "PY_WAIT_FOR_CARGO"):
        env.pop(key, None)
    env.update(
        CARGO_TARGET_DIR=str(tmp_path / "target"), SLOT_FD="",
        CALLS=str(tmp_path / "calls.txt"), TARGETS=str(tmp_path / "targets.txt"),
        CARGO_STARTED=str(tmp_path / "cargo-started"), PY_RESULT=str(tmp_path / "py-result"),
        TMPDIRS=str(tmp_path / "tmpdirs.txt"), TMPDIR=str(tmp_path / "orig-tmp"),
        CODESCOUT_GATE_TMPFS_BASE=str(tmp_path / "tmpbase"),
        PY_PID_FILE=str(tmp_path / "py-pid"),
        FORMAT_MARKER=str(tmp_path / "format-called"), SNAPSHOT_RECORD=str(tmp_path / "snapshot.json"),
        FMT_EXIT="0", FAILED_LANE="none", REAL_PYTHON=sys.executable,
        PATH=str(commands) + os.pathsep + env["PATH"],
    )
    return tmp_path, env


def run_gate_fixture(fixture, *args, **changes):
    root, env = fixture
    return subprocess.run(["bash", str(root / "scripts/gate.sh"), *args], cwd=root,
                          env=env | changes, text=True, capture_output=True, timeout=30)


def assert_gate_calls(root, fmt_args=None, interrupted=False):
    python_call = "python3 ./scripts/python-tests.py --lane light"
    expected = []
    if fmt_args is not None:
        expected.append("fmt " + fmt_args)
    expected += [
        "cargo clippy --workspace --all-targets --features local-embed -- -D warnings",
        "cargo clippy --workspace --all-targets --no-default-features -- -D warnings",
        "cargo nextest run --workspace --no-default-features --no-fail-fast",
        "cargo test --workspace --doc --no-default-features",
        "cargo nextest run --workspace --no-fail-fast",
        "cargo test --workspace --doc",
    ]
    # An interrupted gate stops when the command it was inside returns: bash runs the TERM trap's
    # `exit 143` only after that foreground command, so the doctest step after it never starts.
    if interrupted:
        expected = expected[:-1]
    # The Python lane runs beside the Rust lanes, so where its call lands among theirs is not pinned;
    # that it ran exactly once is, and so is the order of everything else (default Cargo last).
    calls = (root / "calls.txt").read_text().splitlines()
    assert calls.count(python_call) == 1
    assert [call for call in calls if call != python_call] == expected
    cargo_calls = 5 if interrupted else 6
    assert (root / "targets.txt").read_text().splitlines() == [str(root / "target")] * cargo_calls


@pytest.mark.parametrize("failed_lane", ["none", "python", "fmt", "clippy", "lean", "lean-doc", "default", "default-doc"])
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
    # A lane's doctest step belongs to that lane: its failure turns the lane's own field red.
    failed_field_lane = failed_lane.removesuffix("-doc")
    for field, lane in fields.items():
        assert f"{field}={1 if failed_field_lane == lane else 0}" in result.stdout
    state = ("differences" if mode == "check" else "ownership-refused") if failed_lane == "fmt" else "passed"
    assert f"FMT_STATE={state}" in result.stdout
    if mode != "check":
        import json
        record = json.loads((root / "snapshot.json").read_text())
        assert record["mode"] == 0o600
        assert record["context"]["harness"] == mode
        assert not Path(record["path"]).is_relative_to(root)
        assert not Path(record["path"]).exists()
def test_gate_lane_is_red_when_only_its_doctests_fail(tmp_path):
    # nextest does not run doctests, so each test lane has a second command. A green nextest run
    # followed by a red doctest run must still turn the lane red, and the other lane must still run.
    fixture = gate_fixture(tmp_path)
    root, env = fixture
    env["CODEX_SESSION_ID"] = "runner-gate-fixture"
    result = run_gate_fixture(fixture, FAILED_LANE="lean-doc")
    assert_gate_calls(root, "")
    assert result.returncode == 1, result.stderr
    assert "LEAN=1" in result.stdout and "DEFAULT=0" in result.stdout


def test_gate_without_nextest_exits_2_before_any_lane(tmp_path):
    # No fallback to libtest: a gate that ran `cargo test` on one machine and nextest on another
    # would be two gates under one name. The check comes before every lane and before the lease.
    fixture = gate_fixture(tmp_path)
    result = run_gate_fixture(fixture, NO_NEXTEST="1")
    assert result.returncode == 2
    assert "cargo install cargo-nextest --locked" in result.stderr
    assert not (tmp_path / "calls.txt").exists()
    assert not (tmp_path / "format-called").exists()


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
        assert_gate_calls(root, "", interrupted=True)
    finally:
        hold.unlink(missing_ok=True)
        if process.poll() is None:
            process.kill()
            process.communicate(timeout=10)


def test_python_lane_runs_beside_the_cargo_lanes(tmp_path):
    # The light Python lane needs neither the leased target dir nor a built binary, so it starts first
    # and runs while the Rust lanes do. The fixture lane waits up to 5 s for a cargo lane to start and
    # records what it saw: "serial" means the gate ran it to its end before any cargo lane began.
    fixture = gate_fixture(tmp_path)
    root, _ = fixture
    result = run_gate_fixture(fixture, "--check", PY_WAIT_FOR_CARGO="1")
    assert result.returncode == 0, result.stderr
    assert (root / "py-result").read_text().strip() == "overlapped"
    assert_gate_calls(root, "--check")
    # Its output is held and printed once, whole, before the summary, so it cannot interleave with a
    # cargo lane's output.
    assert result.stdout.count("FIXTURE-PYTHON-OUTPUT") == 1
    assert result.stdout.index("FIXTURE-PYTHON-OUTPUT") < result.stdout.index("GATE SECONDS")


def test_gate_waits_for_a_python_lane_that_outlasts_the_cargo_lanes(tmp_path):
    import time
    fixture = gate_fixture(tmp_path)
    root, env = fixture
    hold = root / "hold-python"
    hold.touch()
    env.update(PY_HOLD=str(hold), FAILED_LANE="python")
    process = subprocess.Popen(["bash", str(root / "scripts/gate.sh"), "--check"], cwd=root, env=env,
                               text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)

    def cargo_lanes_done():
        calls = root / "calls.txt"
        return calls.exists() and calls.read_text().splitlines()[-1:] == ["cargo test --workspace --doc"]

    try:
        deadline = time.monotonic() + 10
        while not cargo_lanes_done() and process.poll() is None and time.monotonic() < deadline:
            time.sleep(0.05)
        assert cargo_lanes_done(), "the cargo lanes never ran while the Python lane was still running"
        # The verdict needs the Python exit code, so the gate must still be waiting for it.
        assert process.poll() is None
        hold.unlink()
        out, err = process.communicate(timeout=10)
        assert process.returncode == 1, err
        assert "PYTHON_LIGHT=1" in out
    finally:
        hold.unlink(missing_ok=True)
        if process.poll() is None:
            process.kill()
            process.communicate(timeout=10)


def test_interrupted_gate_stops_its_python_lane(tmp_path):
    import signal
    import time
    fixture = gate_fixture(tmp_path)
    root, env = fixture
    py_hold = root / "hold-python"
    cargo_hold = root / "hold-default"
    started = root / "default-started"
    py_hold.touch()
    cargo_hold.touch()
    env.update(PY_HOLD=str(py_hold), HOLD_DEFAULT=str(cargo_hold), DEFAULT_STARTED=str(started))
    process = subprocess.Popen(["bash", str(root / "scripts/gate.sh"), "--check"], cwd=root, env=env,
                               text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    pid = None
    try:
        deadline = time.monotonic() + 10
        while not (started.exists() and (root / "py-pid").exists()) and process.poll() is None \
                and time.monotonic() < deadline:
            time.sleep(0.05)
        assert started.exists() and (root / "py-pid").exists()
        pid = int((root / "py-pid").read_text())
        os.kill(pid, 0)  # alive while the gate runs
        process.send_signal(signal.SIGTERM)
        cargo_hold.unlink()
        process.communicate(timeout=10)
        assert process.returncode == 143
        for _ in range(60):
            try:
                os.kill(pid, 0)
            except ProcessLookupError:
                break
            time.sleep(0.05)
        else:
            raise AssertionError(f"the Python lane process {pid} outlived the interrupted gate")
    finally:
        py_hold.unlink(missing_ok=True)
        cargo_hold.unlink(missing_ok=True)
        if process.poll() is None:
            process.kill()
            process.communicate(timeout=10)
        if pid is not None:
            try:
                os.kill(pid, 9)
            except ProcessLookupError:
                pass


def tmpdir_lines(root):
    return (root / "tmpdirs.txt").read_text().splitlines()


def test_nextest_lanes_get_a_private_tmpfs_tmpdir(tmp_path):
    # Measured 2026-11-05 (memory research/gate-tmpdir-tmpfs-20261105): the nextest phase ran faster with
    # TMPDIR on tmpfs in 3 of 3 pairs. Only the nextest commands get it; clippy and the doctest steps
    # keep the caller's TMPDIR, because those were not measured.
    fixture = gate_fixture(tmp_path)
    root, _ = fixture
    result = run_gate_fixture(fixture, "--check")
    assert result.returncode == 0, result.stderr
    lines = tmpdir_lines(root)
    original = str(root / "orig-tmp")
    # clippy, lean clippy, lean nextest, lean doctests, default nextest, default doctests
    assert len(lines) == 6
    assert lines[0] == lines[1] == lines[3] == lines[5] == original
    assert lines[2] == lines[4] != original
    private = Path(lines[2])
    assert private.parent == root / "tmpbase" and private.name.startswith("codescout-gate-tmp.")
    assert f"test temp files in {private}" in result.stdout


@pytest.mark.parametrize("failed_lane", ["none", "python", "lean", "default"])
def test_the_private_tmpdir_is_removed_whatever_the_outcome(tmp_path, failed_lane):
    fixture = gate_fixture(tmp_path)
    root, _ = fixture
    run_gate_fixture(fixture, "--check", FAILED_LANE=failed_lane, TMPDIR_LITTER="1")
    private = Path(tmpdir_lines(root)[2])
    # Not the caller's directory: the removal below would otherwise be of nothing.
    assert private != root / "orig-tmp" and private.parent == root / "tmpbase"
    assert not private.exists()
    assert list((root / "tmpbase").iterdir()) == []


def test_an_interrupted_gate_removes_its_private_tmpdir(tmp_path):
    import signal
    import time
    fixture = gate_fixture(tmp_path)
    root, env = fixture
    hold = root / "hold-default"
    started = root / "default-started"
    hold.touch()
    env.update(HOLD_DEFAULT=str(hold), DEFAULT_STARTED=str(started), TMPDIR_LITTER="1")
    process = subprocess.Popen(["bash", str(root / "scripts/gate.sh"), "--check"], cwd=root, env=env,
                               text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        deadline = time.monotonic() + 10
        while not started.exists() and process.poll() is None and time.monotonic() < deadline:
            time.sleep(0.05)
        assert started.exists()
        private = Path(tmpdir_lines(root)[2])
        assert private.parent == root / "tmpbase" and private.exists()
        process.send_signal(signal.SIGTERM)
        hold.unlink()
        process.communicate(timeout=10)
        assert process.returncode == 143
        assert not private.exists()
        assert list((root / "tmpbase").iterdir()) == []
    finally:
        hold.unlink(missing_ok=True)
        if process.poll() is None:
            process.kill()
            process.communicate(timeout=10)


@pytest.mark.parametrize("change", [
    {"FIXTURE_FSTYPE": "ext4"},                       # the base is not tmpfs
    {"CODESCOUT_GATE_TMPFS_MIN_MB": "999999999"},     # tmpfs without room
    {"CODESCOUT_GATE_TMPFS_BASE": "off"},             # switched off
])
def test_no_private_tmpdir_unless_tmpfs_has_room_and_is_not_switched_off(tmp_path, change):
    fixture = gate_fixture(tmp_path)
    root, _ = fixture
    result = run_gate_fixture(fixture, "--check", **change)
    assert result.returncode == 0, result.stderr
    assert set(tmpdir_lines(root)) == {str(root / "orig-tmp")}
    assert list((root / "tmpbase").iterdir()) == []
    assert "test temp files" not in result.stdout


def test_a_stale_private_tmpdir_left_by_a_killed_gate_is_swept(tmp_path):
    # SIGKILL cannot run the EXIT trap, so the next gate removes what a killed one left. Only a
    # directory with the gate's own name and older than half a day goes: a fresh one may belong to a gate
    # another session is running, and a directory with another name is not this script's to remove.
    import time
    fixture = gate_fixture(tmp_path)
    root, _ = fixture
    base = root / "tmpbase"
    stale, fresh, other = (base / "codescout-gate-tmp.stale1", base / "codescout-gate-tmp.fresh1",
                           base / "unrelated-dir")
    for directory in (stale, fresh, other):
        directory.mkdir()
        (directory / "file").write_text("x")
    old = time.time() - 2 * 86400
    for directory in (stale, other):
        os.utime(directory, (old, old))
    result = run_gate_fixture(fixture, "--check")
    assert result.returncode == 0, result.stderr
    assert not stale.exists()
    assert fresh.exists()
    assert other.exists()
