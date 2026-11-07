"""`_pid_alive` in scripts/file-provenance.py: a liveness check that signals nothing.

On Windows `os.kill(pid, 0)` is not a probe. `signal.CTRL_C_EVENT` is 0, so the call asks the console
to deliver Ctrl+C, every process attached to that console receives it, and the call never raises for
a pid that is gone
(docs/issues/2026-11-07-pid-alive-probe-os-kill-0-sends-ctrl-c-to-the-whole-console-on-windows.md).

Load-bearing: every behavioural case runs in a CHILD that owns its console (`CREATE_NEW_CONSOLE` on
Windows). On the broken code the call interrupts whatever shares the caller's console, which here
would be the test runner itself. The sentinel the child probes shares the CHILD's console, so the
signal has somewhere to land if the code under test sends one. The child installs a SIGINT handler
and always writes its result, so a broken `_pid_alive` fails an assertion instead of killing the
child before it can say what happened (the first version of this test did exactly that).
"""
import json
import subprocess
import sys
import textwrap
from pathlib import Path

import pytest

SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"

PROBE = textwrap.dedent(
    """
    import importlib.util, json, os, signal, subprocess, sys, time

    scripts, out = sys.argv[1], sys.argv[2]
    interrupts = []
    signal.signal(signal.SIGINT, lambda *a: interrupts.append(1))
    sys.path.insert(0, scripts)
    spec = importlib.util.spec_from_file_location(
        "fp_pid_alive", os.path.join(scripts, "file-provenance.py"))
    fp = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(fp)

    marker, ready = out + ".sigint", out + ".ready"
    sentinel = subprocess.Popen([sys.executable, "-c", (
        "import signal, sys, time\\n"
        "marker, ready = sys.argv[1], sys.argv[2]\\n"
        "signal.signal(signal.SIGINT, lambda *a: open(marker, 'w').write('x'))\\n"
        "open(ready, 'w').write('x')\\n"
        "t0 = time.time()\\n"
        "while time.time() - t0 < 12:\\n"
        "    time.sleep(0.05)\\n"
    ), marker, ready])
    result = {}
    try:
        deadline = time.time() + 15
        while not os.path.exists(ready) and time.time() < deadline:
            time.sleep(0.05)
        result["live_int"] = fp._pid_alive(sentinel.pid)
        result["live_str"] = fp._pid_alive(str(sentinel.pid))
        time.sleep(1.0)  # a console event is delivered asynchronously
        result["sentinel_interrupted"] = os.path.exists(marker)
        result["self"] = fp._pid_alive(os.getpid())
        gone = subprocess.Popen([sys.executable, "-c", "pass"])
        gone.wait()
        time.sleep(0.2)
        result["exited"] = fp._pid_alive(gone.pid)
        result["none"] = fp._pid_alive(None)
        result["junk"] = fp._pid_alive("not-a-pid")
        # Exists but is not ours to open: Windows pid 4 (System), POSIX pid 1 (init). Alive.
        result["protected"] = fp._pid_alive(4 if sys.platform == "win32" else 1)
        if sys.platform == "win32":
            # ctypes truncates an oversized integer to a DWORD, which here lands on a live pid.
            result["out_of_range"] = fp._pid_alive(2**32 + os.getpid())
    except BaseException as exc:
        result["error"] = repr(exc)
    finally:
        sentinel.kill()
        sentinel.wait()
        time.sleep(0.5)
        result["prober_interrupted"] = bool(interrupts)
        with open(out, "w") as f:
            json.dump(result, f)
    """
)


@pytest.fixture(scope="module")
def probe(tmp_path_factory):
    out = tmp_path_factory.mktemp("pid_alive") / "probe.json"
    kwargs = {}
    if sys.platform == "win32":
        hidden = subprocess.STARTUPINFO()
        hidden.dwFlags |= subprocess.STARTF_USESHOWWINDOW
        hidden.wShowWindow = subprocess.SW_HIDE
        kwargs = {"creationflags": subprocess.CREATE_NEW_CONSOLE, "startupinfo": hidden}
    subprocess.run([sys.executable, "-c", PROBE, str(SCRIPTS), str(out)], timeout=90, **kwargs)
    assert out.exists(), "the probe child died before it could write its result"
    return json.loads(out.read_text())


def test_the_probe_itself_ran_to_the_end(probe):
    assert "error" not in probe, probe.get("error")


def test_a_live_process_is_reported_alive_by_int_and_by_string_pid(probe):
    # `live_sessions` passes the pid both ways: a file stem (str) and a record field (int).
    assert probe["live_int"] is True
    assert probe["live_str"] is True


def test_probing_a_live_process_sends_a_signal_to_nobody(probe):
    # Two audiences: the probed process, and the prober, which shares its console.
    assert probe["sentinel_interrupted"] is False
    assert probe["prober_interrupted"] is False


def test_the_calling_process_is_alive(probe):
    assert probe["self"] is True


def test_a_process_that_has_exited_is_reported_dead(probe):
    assert probe["exited"] is False


def test_a_value_that_is_not_a_pid_is_dead(probe):
    assert probe["none"] is False
    assert probe["junk"] is False
def test_a_process_that_exists_but_cannot_be_opened_is_alive(probe):
    # The third value: "exists, not ours" is alive, as `PermissionError` is on POSIX.
    assert probe["protected"] is True


@pytest.mark.skipif(sys.platform != "win32", reason="the range guard belongs to the Windows branch")
def test_a_pid_outside_the_dword_range_is_dead(probe):
    assert probe["out_of_range"] is False
