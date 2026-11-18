"""Independently reconstruct public source identity from sealed archive manifest."""
from pathlib import Path
import hashlib
import json
import tarfile

root = Path("/home/marius/work/claude/codescout/docs/trackers/effort-steering/evidence/shared-public-runtime-20261110")
closure = json.loads((root / "final-closure-v2.json").read_text())
archive = Path(closure["archive"])
assert hashlib.sha256(archive.read_bytes()).hexdigest() == closure["archive_sha256"]
roles = {".git", ".codescout", ".buddy", ".superpowers", ".claude", "target", "node_modules"}
observed = {}
with tarfile.open(archive, "r:gz") as tar:
    manifest = json.load(tar.extractfile("manifest.json"))
    for stem, expected in (("obscura-421", "e3832fe236e4f0f833dc2971c8be5336c715554cb75a21c8e811e6b1587b6cc4"),
                           ("lean-ctx-1168", "b0842438b7ff5beee3bbb6ef25e78ba0291a062b6362c6c31267a1437becbe30")):
        prefix = "public/" + stem + "/project/"
        entries = {}
        for name, value in manifest["entries"].items():
            if not name.startswith(prefix):
                continue
            relative = name[len(prefix):]
            if relative.split("/")[0] in roles or relative == "submission.json":
                continue
            if value["type"] == "file":
                raw = tar.extractfile(name).read()
                assert hashlib.sha256(raw).hexdigest() == value["sha256"]
                entries[relative] = {"kind": "file", "mode": value["mode"], "sha256": value["sha256"]}
            elif value["type"] == "directory":
                entries[relative] = {"kind": "directory", "mode": value["mode"]}
            else:
                entries[relative] = {"kind": "link", "target": value["target"]}
        actual = hashlib.sha256(json.dumps(entries, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
        assert actual == expected, (stem, actual, expected)
        observed[stem] = {"public_source_sha256": actual, "manifest_source_entries": len(entries),
                          "exact_frozen_digest_reconstructed": True}
    for name in manifest["owned_files"]:
        entry = manifest["entries"]["source/" + name]
        actual = Path("/home/marius/.codex/worktrees/cf66/codescout") / name
        assert hashlib.sha256(actual.read_bytes()).hexdigest() == entry["sha256"]
result = {"archive_sha256": closure["archive_sha256"], "public_projects": observed,
          "all_18_owned_source_hashes_still_match": True, "paid_ready": False}
with (root / "independent-readback-v2.json").open("x") as f:
    json.dump(result, f, indent=2)
print(json.dumps(result, indent=2))
