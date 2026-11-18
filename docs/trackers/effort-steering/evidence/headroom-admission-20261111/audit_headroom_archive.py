"""Independent GNU digest and exact known-input archive check; no extraction."""
from pathlib import Path
import hashlib
import json
import subprocess
import tarfile

MAIN = Path("/home/marius/work/claude/codescout")
ROOT = Path("/var/tmp/ep11-01a15")
DEST = MAIN / "docs/trackers/effort-steering/evidence/headroom-admission-20261111"

def main():
    closure_path = DEST / "closure.json"
    closure = json.loads(closure_path.read_bytes())
    archive = Path(closure["archive"])
    verification_path = ROOT / "headroom-verification-v4/verification.json"
    verification = json.loads(verification_path.read_bytes())
    expected = {"bound-inputs/" + Path(name).as_posix().lstrip("/"): digest
                for name, digest in verification["source_after"].items()}
    assert len(expected) == len(verification["source_after"]) == 192
    gnu = subprocess.run(["sha256sum", str(archive)], check=True, capture_output=True, text=True)
    digest = gnu.stdout.split()[0]
    assert digest == closure["archive_sha256"]
    seen = set()
    bound = {}
    files = 0
    manifests = {}
    forbidden_members = []
    with tarfile.open(archive, mode="r|gz") as source:
        for member in source:
            name = member.name
            assert name not in seen
            seen.add(name)
            assert not Path(name).is_absolute() and ".." not in Path(name).parts
            if name.startswith("paid-campaign-headroom-v3/") or name.endswith("/.config/s5/token-claude-stef"):
                forbidden_members.append(name)
            if name in expected:
                assert member.isfile()
                check = hashlib.sha256()
                with source.extractfile(member) as stream:
                    for block in iter(lambda: stream.read(1024*1024), b""):
                        check.update(block)
                bound[name] = check.hexdigest()
                assert bound[name] == expected[name]
            if name in ("manifest.json", "provenance.json"):
                manifests[name] = json.loads(source.extractfile(member).read())
            elif member.isfile():
                files += 1
    assert bound == expected and not forbidden_members
    assert len(seen) == closure["payload_entries"] + 2
    assert files == closure["regular_payloads"]
    assert set(manifests["manifest.json"]) == seen - {"manifest.json", "provenance.json"}
    assert manifests["provenance.json"]["is_paid_outcome"] is False
    result = {
        "schema": "effort-headroom-archive-independent-check-v1",
        "producer": {"path": str(Path(__file__).resolve()),
                     "sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()},
        "archive": str(archive), "archive_sha256_gnu": digest,
        "closure_sha256": hashlib.sha256(closure_path.read_bytes()).hexdigest(),
        "verification_sha256": hashlib.sha256(verification_path.read_bytes()).hexdigest(),
        "payload_members": len(seen)-2, "regular_payloads": files,
        "expected_bound_inputs": len(expected), "independently_hashed_bound_inputs": len(bound),
        "all_known_inputs_match": bound == expected, "forbidden_file_members": forbidden_members,
        "scope": "GNU checks whole compressed digest; independent Python streams all member headers and hashes only the192known code/input payloads against the consuming verification, not a producer-derived input count.",
        "limits": "Does not rehash other payloads a third time; the archive producer separately reopened all selected payloads. No extraction, live paid output read, provider call, invoice proof or efficacy claim."
    }
    with (DEST / "independent-consumer-check.json").open("x") as stream:
        json.dump(result, stream, indent=2)
    print(json.dumps(result), flush=True)

if __name__ == "__main__":
    main()
