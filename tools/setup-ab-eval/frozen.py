"""Frozen arm inputs (final review I-7): the full arm's config and every plugin tree the cs and
full arms mount, copied once into a study-owned directory, hashed, and re-checked at every batch.

    freeze_inputs(manifest, dest) -> record        copy + hash; refuses an existing dest
    tree_sha256(path) -> hex                       deterministic hash of a file or a tree
    tree_hashes(root, names) -> {rel: hex}         every frozen entry
    check_trees(root, expected) -> None            raises Mismatch when any hash differs
    frozen_manifest(manifest, root) -> manifest    the manifest with every path in `root`
    scan_tree(root) / scan_strings(obj)            the secret scan (pattern names, never values)

Before this module the runner bind-mounted each plugin dir straight from the live profile cache
(`~/.claude/plugins/cache/...`) and copied `~/.claude/CLAUDE.md` and `settings.json` at every
run, so an edit or a cache cleanup between the pilot and the extension would change the arms
silently. Now the runner reads only the frozen copy (run.run_batch passes frozen_manifest() to
arms.arm_invocation, which refuses any path outside `frozen_root`), and run_batch refuses to
start when any tree hash differs from the registration's `frozen.trees` (the _check_cli
pattern). The Stage 2 freeze records `frozen.root` and `frozen.trees` in the registration.

LAYOUT of `dest` (default FROZEN = <data>/study/frozen):
    config/CLAUDE.md        a byte copy of the manifest's CLAUDE.md
    config/settings.json    the full arm's settings: arms.sanitize_user_settings(), then ONLY the
                            keys in SETTINGS_KEYS (the 17 keys Stage 1's full runs kept, read from
                            their arm-record.json), and of `permissions` only PERMISSION_KEYS
                            (`deny`, `defaultMode`; fix round 2, N-1). Everything else is not
                            copied, named (never valued) in frozen.json.
    plugins/<name>/         a copy of each plugin tree (the companion from plugins_cs, the rest
                            from plugins_full), symlinks kept as symlinks, WITHOUT the files
                            SECRET_FILE_GLOBS names (`.env*`, key files, credential files; fix
                            round 2, N-1; listed by path in frozen.json); an absolute symlink or
                            one that leaves its tree refuses the freeze.
    frozen.json             {"trees": {rel: sha256}, "sources": {rel: host path}, "settings":
                            {"copied_keys", "not_copied_keys", "permission_keys",
                            "not_copied_permission_keys"}, "excluded_files": [rel], "secret_scan":
                            {"files_scanned", "hits": 0}}

SECRET SCAN (fix round 2, N-1). Every copied settings string (decoded: the scan walks the parsed
JSON, so a quote inside a string is matched unescaped), CLAUDE.md, and every file of every
copied plugin tree are matched against SECRET_PATTERNS; any hit refuses the freeze, naming the
file and the pattern, never the value. Patterns: anthropic `sk-ant-`, GitHub `gh[pousr]_`,
Slack `xox?-`, AWS `AKIA`, Google `AIza`, Bearer headers, PEM private keys, the `NAME=value`
form (PGPASSWORD=, MYSQL_PWD=, *API_KEY=, *_TOKEN=, *_SECRET=, *PASSWORD=), and a JSON
`"password": "..."` body. Minimum lengths and a placeholder lookahead keep documentation
examples (`sk-ant-...`, `$TOKEN`, `<key>`) out.

TREE HASH. sha256 over one line per entry, entries in sorted relative-path order (os.walk,
symlinks never followed): `d <rel>` for a directory, `f <rel> <x|-> <sha256 of the bytes>` for a
regular file (x = owner-executable), `l <rel> <link target>` for a symlink; anything else (a
FIFO, a socket, a device) raises. A single file hashes as one `f . ...` line.

Stdlib only; calls no docker, no model and reads no token file.
"""
import fnmatch
import hashlib
import json
import os
import re
import shutil
import stat
import uuid
from pathlib import Path

import arms
import tasks

FROZEN = tasks.DATA / "study" / "frozen"
CONFIG_DIR = "config"
PLUGINS_DIR = "plugins"
RECORD = "frozen.json"
CONFIG_FILES = ("CLAUDE.md", "settings.json")
# The user-settings keys the Stage 1 full runs kept (setup-ab-data/study/runs/*.full.0/
# arm-record.json `settings_kept`, identical in all four): the full arm gets these and no other.
SETTINGS_KEYS = ("agentPushNotifEnabled", "autoCompactEnabled", "autoMemoryEnabled",
                 "cleanupPeriodDays", "editorMode", "enabledPlugins", "fileCheckpointingEnabled",
                 "model", "outputStyle", "permissions", "remoteControlAtStartup",
                 "skillOverrides", "skipAutoPermissionPrompt", "skipWorkflowUsageWarning",
                 "theme", "tui", "voiceEnabled")
# Fix round 2 (N-1): of `permissions`, only these. `allow` and `ask` are rule lists, inert under
# --dangerously-skip-permissions (the operator's `allow` carries credentials); additionalDirectories
# names a host path. `deny` removes Bash, Edit, NotebookEdit and Write from full's tool list.
PERMISSION_KEYS = ("deny", "defaultMode")
# Fix round 2 (N-1): files never copied into a frozen plugin tree (fnmatch on the base name).
SECRET_FILE_GLOBS = (".env*", "*.pem", "*.key", "id_*", "*.p12", "credentials*", ".netrc",
                     ".npmrc", ".pgpass")
SECRET_PATTERNS = {
    "anthropic-key": re.compile(r"sk-ant-[A-Za-z0-9_-]{20,}"),
    "github-token": re.compile(r"\bgh[pousr]_[A-Za-z0-9]{30,}"),
    "slack-token": re.compile(r"\bxox[abprs]-[A-Za-z0-9-]{20,}"),
    "aws-access-key": re.compile(r"\bAKIA[0-9A-Z]{16}\b"),
    "google-api-key": re.compile(r"\bAIza[0-9A-Za-z_-]{35}"),
    "bearer-header": re.compile(r"(?i)\bbearer\s+[A-Za-z0-9._~+/-]{20,}=*"),
    "pem-private-key": re.compile(r"-----BEGIN (?:[A-Z0-9]+ )*PRIVATE KEY-----"),
    # The value must follow on the same line ([ \t], not \s: `KEY=` then a newline is empty).
    "name-equals-secret": re.compile(
        r"(?<![A-Za-z0-9_])(?:PGPASSWORD|MYSQL_PWD|[A-Za-z0-9_]*(?:API_KEY|_TOKEN|_SECRET|"
        r"SECRET_KEY|PASSWORD|PASSWD))[ \t]*=[ \t]*(?![$<{%*\\]|\.\.\.)"
        r"(?P<value>[^\s'\"`;)]{6,})"),
    "json-password": re.compile(r"(?i)\"(?:password|passwd)\"\s*:\s*\"(?![$<{*]|\.\.\.)"
                                r"(?P<value>[^\"\s]{4,})\""),
}
# Documentation placeholders a `value` group may hold without being a credential (lower-cased).
PLACEHOLDER_VALUES = frozenset({
    "ollama", "dummy", "none", "local", "test", "placeholder", "unused", "changeme", "secret",
    "abc123", "your_key", "not-needed", "fake", "token", "sk-local", "ghp_xxx", "xxx", "noop",
    "apikey", "api_key", "null", "empty", "nokey", "no-key", "sk-dummy", "example", "your-key",
    "your_api_key", "your-api-key", "your_token", "your-token", "password", "xxxxxx",
    "xxxxxxxx"})
# ... and a value starting with one of these is a placeholder too (`your_api_key_here`).
PLACEHOLDER_PREFIXES = ("your", "xxx", "example", "replace", "insert", "placeholder")


class Mismatch(RuntimeError):
    """A frozen input differs from what was frozen, or the freeze cannot be made safely."""


# --- the secret scan -----------------------------------------------------------------------------

def excluded_name(name):
    """True for a file or directory name SECRET_FILE_GLOBS keeps out of a frozen plugin tree."""
    return any(fnmatch.fnmatchcase(name, g) for g in SECRET_FILE_GLOBS)


def scan_text(text):
    """The names of every SECRET_PATTERNS pattern found in `text` (never the match). A match
    whose `value` group is a PLACEHOLDER_VALUES word does not count."""
    out = []
    for n, p in SECRET_PATTERNS.items():
        for m in p.finditer(text):
            value = m.groupdict().get("value")
            if value is None or not (value.lower() in PLACEHOLDER_VALUES
                                     or value.lower().startswith(PLACEHOLDER_PREFIXES)):
                out.append(n)
                break
    return sorted(out)


def _strings(obj):
    if isinstance(obj, str):
        yield obj
    elif isinstance(obj, dict):
        for k, v in obj.items():
            yield str(k)
            yield from _strings(v)
    elif isinstance(obj, list):
        for v in obj:
            yield from _strings(v)


def scan_strings(obj):
    """scan_text over every decoded string (keys too) of a parsed JSON value."""
    return sorted({n for s in _strings(obj) for n in scan_text(s)})


def scan_tree(root):
    """(files scanned, [(relative path, [pattern names])]) over every regular file under `root`;
    a .json file is scanned raw and as decoded strings. Symlinks are not followed."""
    root = Path(root)
    hits, files_scanned = [], 0
    for d, dirs, files in os.walk(root, followlinks=False):
        for name in files:
            p = Path(d) / name
            if p.is_symlink() or not p.is_file():
                continue
            files_scanned += 1
            text = p.read_bytes().decode("utf-8", errors="replace")
            found = set(scan_text(text))
            if name.endswith(".json"):
                try:
                    found |= set(scan_strings(json.loads(text)))
                except ValueError:
                    pass
            if found:
                hits.append((p.relative_to(root).as_posix(), sorted(found)))
    return files_scanned, sorted(hits)


def _excluded_paths(root):
    root = Path(root)
    out = []
    for d, dirs, files in os.walk(root, followlinks=False):
        for name in sorted(dirs + files):
            if excluded_name(name):
                out.append((Path(d) / name).relative_to(root).as_posix())
        dirs[:] = [x for x in dirs if not excluded_name(x)]
    return sorted(out)


# --- tree hash -----------------------------------------------------------------------------------

def _file_line(rel, path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    x = "x" if os.lstat(path).st_mode & stat.S_IXUSR else "-"
    return f"f {rel} {x} {h.hexdigest()}"


def _lines(root, skip_secret_files=False):
    root = Path(root)
    st = os.lstat(root)
    if stat.S_ISREG(st.st_mode):
        return [_file_line(".", root)]
    if not stat.S_ISDIR(st.st_mode):
        raise Mismatch(f"{root} is not a regular file or a directory")
    out = []
    for d, dirs, files in os.walk(root, followlinks=False):
        if skip_secret_files:
            dirs[:] = [x for x in dirs if not excluded_name(x)]
            files = [x for x in files if not excluded_name(x)]
        dirs.sort()
        for name in sorted(dirs + files):
            p = Path(d) / name
            rel = p.relative_to(root).as_posix()
            m = os.lstat(p).st_mode
            if stat.S_ISLNK(m):
                out.append(f"l {rel} {os.readlink(p)}")
            elif stat.S_ISDIR(m):
                out.append(f"d {rel}")
            elif stat.S_ISREG(m):
                out.append(_file_line(rel, p))
            else:
                raise Mismatch(f"{p}: not a file, directory or symlink")
    return sorted(out, key=lambda line: line.split(" ", 2)[1])


def tree_sha256(path, skip_secret_files=False):
    """The TREE HASH of a file or a directory (see the module docstring); with
    `skip_secret_files`, as if the SECRET_FILE_GLOBS files were absent (the source of a copy)."""
    return hashlib.sha256("\n".join(_lines(path, skip_secret_files)).encode()).hexdigest()


# --- what is frozen ------------------------------------------------------------------------------

def plugin_sources(manifest):
    """{plugin name: host install path} for every plugin the cs and full arms mount: the
    companion (plugins_cs) and every plugins_full entry. A plugins_full companion must be the
    same tree as plugins_cs."""
    out = {arms.COMPANION: manifest["plugins_cs"]["install_path"]}
    for p in manifest["plugins_full"]:
        name, path = p["name"], p["install_path"]
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", name):
            raise Mismatch(f"plugin name {name!r} is not a plain directory name")
        if name in out and out[name] != path:
            raise Mismatch(f"plugin {name} has two install paths: {out[name]}, {path}")
        out[name] = path
    return out


def config_sources(manifest):
    """{file name: host path} of the full arm's config files (CONFIG_FILES only)."""
    out = {}
    for src in manifest["full_config_files"]:
        name = Path(src).name
        if name not in CONFIG_FILES or name in out:
            raise Mismatch(f"full_config_files entry {src}: not one of {CONFIG_FILES} (or twice)")
        out[name] = src
    return out


def entry_names(manifest):
    """The relative names of every frozen entry, in a fixed order."""
    return ([f"{CONFIG_DIR}/{n}" for n in CONFIG_FILES if n in config_sources(manifest)]
            + [f"{PLUGINS_DIR}/{n}" for n in plugin_sources(manifest)])


def frozen_settings(settings):
    """(the settings the full arm gets, a record of the key NAMES copied and not copied).
    Refuses (Mismatch) when any decoded string to be copied matches SECRET_PATTERNS."""
    kept, _ = arms.sanitize_user_settings(settings)
    out = {k: kept[k] for k in SETTINGS_KEYS if k in kept}
    perms = out.get("permissions")
    record = {"permission_keys": [], "not_copied_permission_keys": []}
    if isinstance(perms, dict):
        out["permissions"] = {k: perms[k] for k in PERMISSION_KEYS if k in perms}
        record = {"permission_keys": sorted(out["permissions"]),
                  "not_copied_permission_keys": sorted(set(perms) - set(PERMISSION_KEYS))}
    elif "permissions" in out:
        raise Mismatch("settings permissions is not an object")
    hits = scan_strings(out)
    if hits:
        raise Mismatch(f"a settings value to be frozen matches {hits}; nothing written")
    record.update(copied_keys=sorted(out), not_copied_keys=sorted(set(settings) - set(out)))
    return out, record


def _check_links(tree):
    tree = Path(tree).resolve()
    for d, dirs, files in os.walk(tree, followlinks=False):
        for name in dirs + files:
            p = Path(d) / name
            if p.is_symlink():
                target = os.readlink(p)
                if os.path.isabs(target) or not (p.parent / target).resolve().is_relative_to(tree):
                    raise Mismatch(f"{p} -> {target}: a symlink that leaves its plugin tree")


def _ignore_secret_files(directory, names):
    return [n for n in names if excluded_name(n)]


def freeze_inputs(manifest, dest=FROZEN):
    """Copy, scan and hash the arm inputs into `dest` (which must not exist); returns
    frozen.json. Any secret-scan hit refuses and leaves nothing behind."""
    dest = Path(dest)
    if dest.exists() or dest.is_symlink():
        raise Mismatch(f"{dest} exists: a freeze is made once; move it aside to make another")
    configs, plugins = config_sources(manifest), plugin_sources(manifest)
    tmp = dest.with_name(f"{dest.name}.partial-{uuid.uuid4().hex[:8]}")
    tmp.mkdir(parents=True)
    try:
        (tmp / CONFIG_DIR).mkdir()
        record = {"sources": {}, "settings": None, "excluded_files": []}
        for name, src in configs.items():
            rel = f"{CONFIG_DIR}/{name}"
            if name == "settings.json":
                kept, rec = frozen_settings(json.loads(Path(src).read_text()))
                (tmp / rel).write_text(json.dumps(kept, indent=2) + "\n")
                record["settings"] = rec
            else:
                shutil.copyfile(src, tmp / rel)
            record["sources"][rel] = str(src)
        (tmp / PLUGINS_DIR).mkdir()
        for name, src in plugins.items():
            if not Path(src).is_dir() or Path(src).is_symlink():
                raise Mismatch(f"plugin {name}: {src} is not a directory")
            _check_links(src)
            shutil.copytree(src, tmp / PLUGINS_DIR / name, symlinks=True,
                            ignore=_ignore_secret_files)
            record["sources"][f"{PLUGINS_DIR}/{name}"] = str(src)
            record["excluded_files"] += [f"{PLUGINS_DIR}/{name}/{p}"
                                         for p in _excluded_paths(src)]
            if (tree_sha256(tmp / PLUGINS_DIR / name)
                    != tree_sha256(src, skip_secret_files=True)):
                raise Mismatch(f"plugin {name}: the copy does not hash like its source")
        files, hits = scan_tree(tmp)
        if hits:
            raise Mismatch(f"secret scan: {len(hits)} file(s) match: "
                           f"{[f'{p} ({', '.join(n)})' for p, n in hits]}; nothing written")
        record["secret_scan"] = {"files_scanned": files, "hits": 0,
                                 "patterns": sorted(SECRET_PATTERNS)}
        record["trees"] = tree_hashes(tmp, entry_names(manifest))
        (tmp / RECORD).write_text(json.dumps(record, indent=2, sort_keys=True) + "\n")
        os.rename(tmp, dest)
    except BaseException:
        shutil.rmtree(tmp, ignore_errors=True)
        raise
    return record


def tree_hashes(root, names):
    """{rel: tree_sha256(root / rel)} for every name."""
    root = Path(root)
    out = {}
    for rel in names:
        p = root / rel
        if not (p.exists() or p.is_symlink()):
            raise Mismatch(f"{p} is missing")
        out[rel] = tree_sha256(p)
    return out


def check_trees(root, expected):
    """Raise Mismatch unless every entry of `expected` ({rel: sha256}, non-empty) hashes the same
    under `root` now."""
    if not isinstance(expected, dict) or not expected:
        raise Mismatch("no frozen tree hashes recorded; the registration must carry frozen.trees")
    try:
        actual = tree_hashes(root, sorted(expected))
    except (OSError, Mismatch) as e:
        raise Mismatch(f"frozen inputs under {root} are incomplete: {e}")
    bad = sorted(rel for rel in expected if actual[rel] != expected[rel])
    if bad:
        raise Mismatch(f"frozen inputs changed after the freeze: {bad} under {root}")


def frozen_manifest(manifest, root):
    """A copy of `manifest` whose plugin and config paths name the frozen copies under `root`,
    with `frozen_root` set (arms.arm_invocation then refuses any path outside it)."""
    root = Path(root).resolve()
    m = json.loads(json.dumps(manifest))
    m["frozen_root"] = str(root)
    m["plugins_cs"]["install_path"] = str(root / PLUGINS_DIR / arms.COMPANION)
    for p in m["plugins_full"]:
        p["install_path"] = str(root / PLUGINS_DIR / p["name"])
    m["full_config_files"] = [str(root / CONFIG_DIR / Path(src).name)
                              for src in manifest["full_config_files"]]
    return m


def main(argv=None):
    import argparse
    import sys
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="action", required=True)
    f = sub.add_parser("freeze", help="copy and hash the arm inputs (once)")
    f.add_argument("--manifest", type=Path, required=True)
    f.add_argument("--dest", type=Path, default=FROZEN)
    c = sub.add_parser("check", help="re-hash and compare with frozen.json (read only)")
    c.add_argument("--dest", type=Path, default=FROZEN)
    s = sub.add_parser("scan", help="the secret scan over a dir: counts and paths, never values")
    s.add_argument("--dest", type=Path, default=FROZEN)
    a = ap.parse_args(argv)
    if a.action == "scan":
        files, hits = scan_tree(a.dest)
        print(json.dumps({"dest": str(a.dest), "files_scanned": files, "files_with_hits":
                          len(hits), "hits": [{"path": p, "patterns": n} for p, n in hits]},
                         indent=2))
        return 1 if hits else 0
    try:
        if a.action == "freeze":
            rec = freeze_inputs(json.loads(a.manifest.read_text()), a.dest)
        else:
            rec = json.loads((a.dest / RECORD).read_text())
            check_trees(a.dest, rec["trees"])
    except Mismatch as e:
        print(f"REFUSED: {e}", file=sys.stderr)
        return 1
    print(json.dumps({"dest": str(a.dest), "trees": rec["trees"], "settings": rec["settings"],
                      "excluded_files": rec.get("excluded_files"),
                      "secret_scan": rec.get("secret_scan")}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
