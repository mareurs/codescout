"""Grade a SWE-bench-Live MultiLang Rust task: run the record's own log parser, decide verdicts.

    run_parser(parser_src, log, timeout)       the record's `log_parser` run on a test log, in a subprocess
    verdict(statuses, f2p, p2p)                did every expected test pass
    control_verdict(base, gold, f2p, p2p)      is the task itself sound (fails at base, passes with gold)

`log_parser` is untrusted dataset code. It runs only in a child `python -I` with a timeout and an
environment of PATH alone (no HOME, no tokens), and the child is killed with its process group on
timeout. A parser that hangs, raises, prints garbage or returns non-strings is a `ParserError`,
which a caller must grade as a failure, never as a pass. Only the exact status `"pass"` passes;
`"fail"`, `"skip"` and anything unknown do not. Stdlib only.
"""
import json
import os
import signal
import subprocess
import sys
import tempfile

PASS = "pass"
MAX_EXAMPLES = 10

# Reads {"src", "log"} from stdin, runs src, calls parser(log). The parser's own prints go to
# /dev/null; only the result JSON reaches the real stdout.
_DRIVER = """\
import json, os, sys
req = json.loads(sys.stdin.read())
out = sys.stdout
sys.stdout = open(os.devnull, "w")
ns = {"__name__": "log_parser"}
exec(compile(req["src"], "<log_parser>", "exec"), ns)
result = ns["parser"](req["log"])
if not isinstance(result, dict) or not all(isinstance(k, str) and isinstance(v, str) for k, v in result.items()):
    sys.exit("parser result is not a dict of str to str")  # json.dumps would coerce int keys to str
out.write(json.dumps(result))
out.flush()
"""


class ParserError(ValueError):
    """The record's log parser did not produce a usable {test name: status} mapping."""


def _kill_group(proc):
    """SIGKILL the child's process group, then reap the child; never blocks for long."""
    try:
        os.killpg(proc.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        pass  # a SIGKILLed child is unreapable only if the kernel is stuck; do not hang on it



def run_parser(parser_src, log, timeout=30.0):
    """`parser(log)` of the untrusted `parser_src`, as a dict of str to str. ParserError on a
    timeout, a non-zero exit, output that is not JSON, a non-dict, or a non-string key or value."""
    request = json.dumps({"src": parser_src, "log": log})
    # stdin, stdout and stderr are temp files, not pipes: a descendant that escapes the process
    # group (setsid) and keeps a pipe open would block a read-to-EOF forever, but it cannot make
    # a file read or `wait(timeout)` block.
    with tempfile.TemporaryFile("w+") as fin, tempfile.TemporaryFile("w+") as fout, \
            tempfile.TemporaryFile("w+") as ferr:
        fin.write(request)
        fin.seek(0)
        proc = subprocess.Popen(
            [sys.executable, "-I", "-c", _DRIVER], stdin=fin, stdout=fout, stderr=ferr,
            env={"PATH": os.environ.get("PATH", "/usr/bin:/bin")}, start_new_session=True,
        )
        try:
            proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            _kill_group(proc)
            raise ParserError(f"log parser timed out after {timeout}s") from None
        _kill_group(proc)  # the child exited; reap anything it left in its group
        fout.seek(0)
        out = fout.read()
        ferr.seek(0)
        err = ferr.read()
    if proc.returncode != 0:
        tail = (err or "").strip().splitlines()[-1:] or [""]
        raise ParserError(f"log parser exited {proc.returncode}: {tail[0][:200]}")
    try:
        statuses = json.loads(out)
    except ValueError:
        raise ParserError("log parser output is not JSON") from None
    if not isinstance(statuses, dict):
        raise ParserError(f"log parser returned {type(statuses).__name__}, not a dict")
    if not all(isinstance(k, str) and isinstance(v, str) for k, v in statuses.items()):
        raise ParserError("log parser returned a non-string key or value")
    return statuses


def verdict(statuses, f2p, p2p):
    """Pass only when every FAIL_TO_PASS and PASS_TO_PASS name has status exactly `"pass"` and
    there is at least one. `missing` counts expected names absent from `statuses` (also counted
    in `not_passing`); `examples` are the first few names that did not pass."""
    expected = list(f2p) + list(p2p)
    failing = [n for n in expected if statuses.get(n) != PASS]
    missing = [n for n in expected if n not in statuses]
    return {"pass": bool(expected) and not failing, "expected": len(expected),
            "not_passing": len(failing), "missing": len(missing),
            "examples": failing[:MAX_EXAMPLES]}


def control_verdict(base, gold, f2p, p2p):
    """Is the task sound: no FAIL_TO_PASS test passes at the base commit, and every expected
    test passes with the gold patch. A task failing this cannot separate arms and is not ok."""
    f2p_at_base = sum(1 for n in f2p if base.get(n) == PASS)
    not_passing_gold = sum(1 for n in list(f2p) + list(p2p) if gold.get(n) != PASS)
    reasons = []
    if not f2p:
        reasons.append("no fail-to-pass tests")
    if f2p_at_base:
        reasons.append("fail-to-pass test passes at base")
    if not_passing_gold:
        reasons.append("expected tests do not pass with the gold patch")
    return {"ok": not reasons, "reasons": reasons, "f2p_passing_at_base": f2p_at_base,
            "expected_not_passing_at_gold": not_passing_gold, "expected": len(f2p) + len(p2p)}
