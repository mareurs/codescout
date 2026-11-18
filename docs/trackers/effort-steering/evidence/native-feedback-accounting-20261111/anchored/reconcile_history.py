"""Read-only canonical client-session reconciliation; writes a new audit artifact."""
from decimal import Decimal, localcontext
import hashlib
import json
from pathlib import Path
import sys

MAIN = Path("/home/marius/work/claude/codescout")
ROOT = Path("/var/tmp/ep11-01a15/paid-campaign-headroom-v3")
OUT = MAIN / "docs/trackers/effort-steering/evidence/native-feedback-accounting-20261111"
sys.path.insert(0, "/home/marius/.codex/worktrees/2aa6/codescout/tools/effort_protocol")
from campaign import accounting_totals


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def bound(path, expected):
    path = Path(path)
    assert path.is_file() and not path.is_symlink(), path
    assert sha(path) == expected, path
    return json.loads(path.read_bytes())


def main():
    registration = json.loads((ROOT / "registration.json").read_bytes())
    ledger = json.loads((ROOT / "ledger-final.json").read_bytes())
    raw_paths = sorted(ROOT.glob("attempts/*/*/native/receipt.json"))
    invocation_paths = sorted(ROOT.glob("attempts/*/*/invocation.json"))
    invocations = {}
    for path in invocation_paths:
        value = bound(path, registration["bindings"].get(str(path), sha(path)))
        assert value["invocation_id"] not in invocations
        invocations[value["invocation_id"]] = (path, value)
    canonical, aliases, auxiliary = [], [], []
    opening = None
    old_raw = new_raw = 0
    for row in ledger["charges"]:
        identity = row["id"]
        if identity == "recorded-pre-native-opening-balance":
            record = bound(row["source"], row["receipt_sha256"])
            assert Decimal(str(record["prior_campaign_spend_usd"])) == Decimal(row["cost_usd"])
            opening = dict(row)
            aliases.append({"ledger_id": identity, "kind": "carried-recorded-opening", "source": row["source"]})
            continue
        if identity.endswith("::shared-charge"):
            descriptor = registration["admission"]["shared_preparation"]
            record = bound(descriptor["path"], row["receipt_sha256"])
            assert descriptor["sha256"] == row["receipt_sha256"]
            assert Decimal(row["cost_usd"]) == Decimal(str(record["new_client_model_cost_usd"])) == 0
            assert record["client_author_calls"] == 0
            auxiliary.append({"ledger_row": row, "source": descriptor["path"], "receipt": record})
            aliases.append({"ledger_id": identity, "kind": "documented-zero-shared-preparation", "source": descriptor["path"]})
            continue
        if identity.startswith("native-client:"):
            path = Path(row["source"])
            raw = bound(path, row["receipt_sha256"])
            old_raw += 1
            normalized_path = None
        else:
            normalized_path, normalized = invocations[identity]
            assert sha(normalized_path) == row["receipt_sha256"]
            assert normalized["cost_is_final"] is True
            assert Decimal(str(normalized["cost_usd"])) == Decimal(row["cost_usd"])
            path = Path(normalized["native_path"]) / "receipt.json"
            assert path in raw_paths
            raw = json.loads(path.read_bytes())
            new_raw += 1
        assert raw["scripted"] is False and raw["unknown_provider_cost"] is False
        assert isinstance(raw["session_id"], str) and raw["session_id"]
        assert Decimal(str(raw["cost_usd"])) == Decimal(row["cost_usd"])
        canonical_id = "native-client:" + raw["session_id"]
        if normalized_path is None:
            assert identity == canonical_id
        canonical.append({"id": canonical_id, "cost_usd": row["cost_usd"],
                          "source": str(path), "receipt_sha256": sha(path)})
        aliases.append({"ledger_id": identity, "canonical_id": canonical_id,
                        "normalized_source": str(normalized_path) if normalized_path else None,
                        "native_source": str(path)})
    assert opening is not None and old_raw == 32 and new_raw == 16
    assert len(canonical) == len({r["id"] for r in canonical}) == 48
    assert len(aliases) == len(ledger["charges"]) == 50 and len(auxiliary) == 1
    assert len(ledger["holds"]) == 1 and not ledger["reservations"]
    hold = ledger["holds"][0]
    bound(hold["source"], hold["receipt_sha256"])
    assert hold["settled"] is False and hold["cost_usd"] is None and Decimal(hold["reserved_usd"]) == 4
    with localcontext() as context:
        context.prec = 128
        native = sum((Decimal(r["cost_usd"]) for r in canonical), Decimal(0))
        known = native + Decimal(opening["cost_usd"])
        held = Decimal(hold["reserved_usd"])
        commitment = known + held
        proposed = commitment + Decimal(112)
        measured = accounting_totals(ledger)
        assert Decimal(measured["committed_usd"]) == commitment
        previous = json.loads((MAIN / "docs/trackers/effort-steering/evidence/closed-campaign-audit-20261111/audit.json").read_bytes())
        assert Decimal(previous["known_series_client_cost_usd"]) == known
        assert known == Decimal("69.375358899999998524")
        assert proposed == Decimal("185.375358899999998524") <= Decimal(ledger["ceiling_usd"])
    result = {"schema": "client-session-reconciliation-audit-v2", "original_ledger": str(ROOT / "ledger-final.json"),
              "original_ledger_sha256": sha(ROOT / "ledger-final.json"), "native_receipts": 48,
              "canonical_charges": canonical, "original_row_mappings": aliases,
              "historical_opening": opening, "shared_zero_evidence": auxiliary,
              "holds_unchanged": ledger["holds"], "native_client_cost_usd": str(native),
              "known_client_cost_usd": str(known), "unresolved_reserve_usd": str(held),
              "current_commitment_usd": str(commitment), "prospective_allocation_usd": "112",
              "prospective_commitment_usd": str(proposed), "paid_admitted": False,
              "earlier_opening_raw_rescan": False, "provider_invoice_reconciliation": "unmeasured",
              "root_review_embedding_cpu_costs": "unmeasured", "producer_sha256": sha(Path(__file__))}
    OUT.mkdir(parents=True, exist_ok=True)
    destination = OUT / "reconciliation.json"
    with destination.open("x") as stream:
        stream.write(json.dumps(result, sort_keys=True, indent=2) + "\n")
    assert json.loads(destination.read_bytes()) == result
    print(json.dumps({k: result[k] for k in ("native_receipts", "native_client_cost_usd",
          "known_client_cost_usd", "unresolved_reserve_usd", "prospective_commitment_usd", "paid_admitted")}))
    print("artifact_sha256=" + sha(destination))


if __name__ == "__main__":
    main()
