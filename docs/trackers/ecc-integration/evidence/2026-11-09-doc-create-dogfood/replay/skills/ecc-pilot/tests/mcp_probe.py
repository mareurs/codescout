"""Small stdio MCP client for native pilot checks; no authentication/model calls."""
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import threading
import time

PACKAGE = Path(__file__).resolve().parents[1]
LSP_TOOLS = {"references", "call_graph", "symbol_at"}


def copy_fixture(destination, configured=True):
    destination = Path(destination)
    ignore = shutil.ignore_patterns("__pycache__") if configured else shutil.ignore_patterns(
        "__pycache__", ".codescout")
    shutil.copytree(PACKAGE / "tests/fixtures", destination, ignore=ignore)
    return destination
def isolated_server_env(state_root):
    """Keep fixture catalogs and vector/state stores outside user data."""
    state_root = Path(state_root).resolve()
    state_root.mkdir(parents=True, exist_ok=True, mode=0o700)
    workspace = state_root / "workspace.toml"
    workspace.write_text("")
    env = dict(os.environ)
    env.pop("CODESCOUT_ALLOW_TEMP_WORKSPACE", None)
    env.update(
        LIBRARIAN_DB=str(state_root / "catalog.db"),
        LIBRARIAN_WORKSPACE=str(workspace),
        CODESCOUT_ARTIFACT_BACKEND="sqlite-vec",
        XDG_DATA_HOME=str(state_root / "data"),
        XDG_STATE_HOME=str(state_root / "state"),
        XDG_CONFIG_HOME=str(state_root / "config"))
    return env


def payload(result):
    if result.get("isError"):
        raise AssertionError(result)
    for block in result.get("content", []):
        if block.get("type") == "text":
            try:
                value = json.loads(block["text"])
            except json.JSONDecodeError:
                continue
            if isinstance(value, dict):
                return value
    raise AssertionError("No JSON object in MCP result: " + repr(result))


class McpProbe:
    def __init__(self, binary, project, env=None):
        self.binary = str(binary)
        self.project = Path(project).resolve()
        self.env = env if env is not None else isolated_server_env(
            self.project.parent / "codescout-state")
        self.responses = []
        self._messages = queue.Queue()
        self._next_id = 0

    def __enter__(self):
            self._process = subprocess.Popen(
                [self.binary, "start", "--project", str(self.project)],
                cwd=self.project, env=self.env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL, text=True)
            def collect():
                try:
                    for line in self._process.stdout:
                        self._messages.put(line)
                finally:
                    self._messages.put(None)
            self._reader = threading.Thread(target=collect, daemon=True)
            self._reader.start()
            try:
                self.request("initialize", {
                    "protocolVersion": "2024-11-05", "capabilities": {},
                    "clientInfo": {"name": "ecc-navigation-probe", "version": "0.1.0"}
                })
                self._send({"jsonrpc": "2.0", "method": "notifications/initialized"})
                return self
            except BaseException:
                self.__exit__(None, None, None)
                raise

    def _send(self, message):
        self._process.stdin.write(json.dumps(message) + "\n")
        self._process.stdin.flush()

    def request(self, method, params, timeout=60):
        self._next_id += 1
        idx = self._next_id
        self._send({"jsonrpc": "2.0", "id": idx, "method": method, "params": params})
        deadline = time.monotonic() + timeout
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError("MCP response timed out: " + method)
            try:
                line = self._messages.get(timeout=remaining)
            except queue.Empty as exc:
                raise TimeoutError("MCP response timed out: " + method) from exc
            if line is None:
                raise RuntimeError("MCP server exited before responding to " + method)
            response = json.loads(line)
            self.responses.append(response)
            if response.get("id") == idx:
                if "error" in response:
                    raise RuntimeError(response["error"])
                return response["result"]

    def call(self, name, **arguments):
        return payload(self.request("tools/call", {"name": name, "arguments": arguments}))

    def tools(self):
        return {row["name"] for row in self.request("tools/list", {})["tools"]}

    def __exit__(self, *_):
        self._process.terminate()
        try:
            self._process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self._process.kill()
            self._process.wait(timeout=5)
        self._reader.join(timeout=2)
        self._process.stdin.close()
        self._process.stdout.close()
