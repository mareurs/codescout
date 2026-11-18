"""Anchor the full 48-session rescan to the independently preserved closed audit."""
import hashlib
import importlib.util
import json
from pathlib import Path

MAIN = Path("/home/marius/work/claude/codescout")
AUDIT = MAIN / "docs/trackers/effort-steering/evidence/closed-campaign-audit-20261111/audit.json"
EXPECTED = "838b384d8b0c871f32d98324da4686063cdedc3cab5509f5a3a6e620492309af"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    assert digest(AUDIT) == EXPECTED
    audit = json.loads(AUDIT.read_bytes())
    bindings = audit["bindings"]
    def reopen():
        for name, expected in bindings.items():
            path = Path(name)
            assert path.is_file() and not path.is_symlink(), path
            assert digest(path) == expected, path
    reopen()
    source = Path(__file__).with_name("reconcile_history.py")
    spec = importlib.util.spec_from_file_location("history_producer", source)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    initial = module.OUT / "reconciliation.json"
    old_digest = digest(initial)
    module.OUT = module.OUT / "anchored"
    module.main()
    output = module.OUT / "reconciliation.json"
    assert digest(output) == old_digest
    reopen()
    assert digest(AUDIT) == EXPECTED
    result = {"schema": "client-session-reconciliation-anchor-v1",
              "closed_audit": str(AUDIT), "closed_audit_sha256": EXPECTED,
              "all_closed_audit_bindings_reopened_before_after": len(bindings),
              "reconciliation": str(output), "reconciliation_sha256": digest(output),
              "same_bytes_as_initial_rescan": True,
              "producer": str(source), "producer_sha256": digest(source),
              "anchor_producer_sha256": digest(Path(__file__))}
    with (module.OUT / "anchor-check.json").open("x") as stream:
        stream.write(json.dumps(result, indent=2, sort_keys=True) + "\n")
    print(json.dumps(result))


if __name__ == "__main__":
    main()
