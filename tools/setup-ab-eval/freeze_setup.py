"""Freeze the operator's daily Claude Code setup into setup-manifest.json.

Reads JSON and config files only; the one side effect is the manifest write. The
profile is a required argument: the operator names it, there is no default.
Stdlib only.
"""
import argparse
import hashlib
import json
import subprocess
import sys
import urllib.request
from pathlib import Path
from typing import Callable, Optional

# Plugins whose MCP server needs a browser or network: not reproducible in a container.
# An explicit, named set on purpose -- not a heuristic.
BROWSER_OR_NETWORK_MCP_PLUGINS = {
    "chrome-devtools-mcp": "MCP server drives a local browser (needs Chrome and a display/network)",
}

# Only these two files of a profile are ever listed. Transcripts, memory, todos,
# sessions and history are deliberately never enumerated.
CONFIG_FILE_NAMES = ("CLAUDE.md", "settings.json")

# Qdrant's HTTP API is on 6333; the env's CODESCOUT_QDRANT_URL points at gRPC 6334.
QDRANT_HTTP_URL = "http://127.0.0.1:6333"
QDRANT_HEALTH_PATH = "/healthz"


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def default_runner(argv) -> str:
    return subprocess.run(argv, capture_output=True, text=True, timeout=30, check=True).stdout


def http_probe(url: str, timeout: float = 2.0) -> bool:
    try:
        with urllib.request.urlopen(url, timeout=timeout) as r:
            return 200 <= r.status < 300
    except Exception:
        return False


def _declares_mcp(install: Path) -> bool:
    for name in (".mcp.json", "mcp.json"):
        f = install / name
        if f.is_file():
            try:
                if json.loads(f.read_text()).get("mcpServers"):
                    return True
            except (OSError, ValueError):
                pass
    for rel in (".claude-plugin/plugin.json", "plugin.json"):
        f = install / rel
        if f.is_file():
            try:
                if json.loads(f.read_text()).get("mcpServers"):
                    return True
            except (OSError, ValueError):
                pass
    return False


def _enabled_plugins(profile: Path) -> list:
    settings = json.loads((profile / "settings.json").read_text())
    records = json.loads((profile / "plugins" / "installed_plugins.json").read_text())["plugins"]
    root = profile.resolve()
    out = []
    for key, on in settings.get("enabledPlugins", {}).items():
        if not on:
            continue
        recs = records.get(key)
        if not recs:
            raise ValueError(f"plugin {key} is enabled but has no install record")
        rec = recs[-1]
        install = Path(rec["installPath"]).resolve()
        if not install.is_relative_to(root):
            raise ValueError(f"plugin {key} installPath {install} is outside profile {root}")
        out.append({"name": key.split("@")[0], "key": key, "install_path": str(install),
                    "version": rec.get("version"), "has_mcp": _declares_mcp(install)})
    return out


def freeze(profile_dir: Path, claude_json: Path, probe: Callable[[str], bool], *,
           cli_path: Path, runner: Callable = default_runner,
           out_path: Optional[Path] = None) -> dict:
    profile = Path(profile_dir)
    cs = json.loads(Path(claude_json).read_text())["mcpServers"]["codescout"]
    env = cs.get("env", {})
    cs_bin = Path(cs["command"])

    for key in ("CODESCOUT_EMBEDDER_URL", "CODESCOUT_SPARSE_EMBEDDER_URL", "CODESCOUT_RERANKER_URL"):
        # An unconfigured service is a config error, not a measured "absent".
        if not env.get(key):
            raise ValueError(f"{key} missing from codescout env in {claude_json}")

    retrieval = {
        "qdrant": probe(QDRANT_HTTP_URL + QDRANT_HEALTH_PATH),
        "dense": probe(env["CODESCOUT_EMBEDDER_URL"] + "/health"),
        "sparse": probe(env["CODESCOUT_SPARSE_EMBEDDER_URL"] + "/health"),
        "reranker": probe(env["CODESCOUT_RERANKER_URL"] + "/health"),
    }

    plugins = _enabled_plugins(profile)
    full, excluded = [], []
    for p in plugins:
        entry = {k: p[k] for k in ("name", "install_path", "version", "has_mcp")}
        if p["name"] in BROWSER_OR_NETWORK_MCP_PLUGINS:
            entry["reason"] = BROWSER_OR_NETWORK_MCP_PLUGINS[p["name"]]
            excluded.append(entry)
        else:
            full.append(entry)
    cs_plugin = next(({"install_path": p["install_path"], "version": p["version"]}
                      for p in plugins if p["name"] == "codescout-companion"), None)

    manifest = {
        "profile_dir": str(profile),
        "cli": {"path": str(cli_path),
                "version": runner([str(cli_path), "--version"]).strip(),
                "sha256": sha256_file(Path(cli_path))},
        # version_json is recorded verbatim; its dirty field is known-buggy, never gated on.
        "codescout": {"binary_sha256": sha256_file(cs_bin),
                      "version_json": json.loads(runner([str(cs_bin), "version"]))},
        "mcp_env": env,
        "retrieval": retrieval,
        "plugins_cs": cs_plugin,
        "plugins_full": full,
        "plugins_full_excluded": excluded,
        "full_config_files": [str(profile / n) for n in CONFIG_FILE_NAMES
                              if (profile / n).is_file()],
    }
    out = Path(out_path) if out_path else Path("setup-manifest.json")
    out.write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--profile", required=True, type=Path, help="profile dir; no default")
    ap.add_argument("--claude-json", required=True, type=Path)
    ap.add_argument("--cli", required=True, type=Path, help="Claude Code CLI binary")
    ap.add_argument("--out", type=Path, default=Path("setup-manifest.json"))
    a = ap.parse_args(argv)
    m = freeze(a.profile, a.claude_json, http_probe, cli_path=a.cli, out_path=a.out)
    print(json.dumps({"retrieval": m["retrieval"], "plugins_full": len(m["plugins_full"]),
                      "excluded": [p["name"] for p in m["plugins_full_excluded"]]}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
