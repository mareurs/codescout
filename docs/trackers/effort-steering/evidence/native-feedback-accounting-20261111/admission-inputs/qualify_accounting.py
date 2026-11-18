"""Qualify only the new accounting component against actual frozen historical receipts."""
import copy
from decimal import Decimal, localcontext
import hashlib
import json
from pathlib import Path
import shutil
import sys

MAIN = Path("/home/marius/work/claude/codescout")
OUT = MAIN / "docs/trackers/effort-steering/evidence/native-feedback-accounting-20261111/admission-inputs"
HISTORY = MAIN / "docs/trackers/effort-steering/evidence/native-feedback-accounting-20261111/anchored"
REVISION = Path("/home/marius/.codex/worktrees/2aa6/codescout")
sys.path.insert(0, str(REVISION / "tools/effort_protocol"))
import feedback_admission


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    history_path = HISTORY / "reconciliation.json"
    assert sha(history_path) == "f11ad81dabf4c18495281d90601a253cbce40f8ca08117e40a7ca45493b740f5"
    history = json.loads(history_path.read_bytes())
    anchor_path = HISTORY / "anchor-check.json"
    anchor = json.loads(anchor_path.read_bytes())
    audit_path = Path(anchor["closed_audit"])
    assert sha(audit_path) == anchor["closed_audit_sha256"]
    audit = json.loads(audit_path.read_bytes())
    bindings = dict(audit["bindings"])
    for path in (history_path, anchor_path, audit_path):
        bindings[str(path)] = sha(path)
    ledger = {"ceiling_usd": "200", "charges": [history["historical_opening"], *history["canonical_charges"]],
              "holds": history["holds_unchanged"], "reservations": []}
    for row in ledger["charges"] + ledger["holds"]:
        assert sha(Path(row["source"])) == row["receipt_sha256"]
        bindings[row["source"]] = row["receipt_sha256"]
    with localcontext() as ctx:
        ctx.prec = 128
        proposed = str(Decimal(history["known_client_cost_usd"]) + Decimal(history["unresolved_reserve_usd"]) + Decimal("112"))
    evidence = {"schema": "client-ledger-reconciliation-v1", "ledger": ledger,
                "opening_balance_source": history["historical_opening"]["source"],
                "opening_balance_sha256": history["historical_opening"]["receipt_sha256"],
                "opening_balance_usd": history["historical_opening"]["cost_usd"],
                "known_client_cost_usd": history["known_client_cost_usd"], "native_receipts": 48,
                "native_client_cost_usd": history["native_client_cost_usd"],
                "unresolved_reserve_usd": history["unresolved_reserve_usd"],
                "proposed_new_allocation_usd": "112", "commitment_if_new_allocation_usd": proposed,
                "receipt_bindings": {r["source"]: r["receipt_sha256"]
                                     for r in ledger["charges"][1:] + ledger["holds"]},
                "canonical_history": {"path": str(history_path), "sha256": sha(history_path)},
                "history_anchor": {"path": str(anchor_path), "sha256": sha(anchor_path)},
                "earlier_charge_rescan": "Opening remains recorded;48native sessions reopened.",
                "provider_invoice_reconciliation": "unmeasured"}
    OUT.mkdir(parents=True, exist_ok=True)
    path = OUT / "reconciliation.json"
    with path.open("x") as stream:
        stream.write(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
    bindings[str(path)] = sha(path)
    reg = {"schema": "feedback-accounting-component-v1", "ledger": ledger, "bindings": bindings,
           "carried_holds": [{k: row[k] for k in ("id", "source", "receipt_sha256", "reserved_usd")}
                             for row in ledger["holds"]]}
    proof = {"path": str(path), "sha256": sha(path)}
    result = feedback_admission.verify_accounting(reg, proof)
    assert result["native_receipts"] == 48
    assert Decimal(result["commitment_if_new_allocation_usd"]) == Decimal("185.375358899999998524")
    # A consistently re-summed47-session ledger passes the general raw-reader but
    # must still fail the new exact historical coverage control.
    variants = []
    altered = copy.deepcopy(evidence)
    altered["proposed_new_allocation_usd"] = "78"
    variants.append(("old-allocation", altered))
    altered = copy.deepcopy(evidence)
    altered["ledger"]["holds"][0]["reserved_usd"] = "0"
    variants.append(("lost-old-hold", altered))
    altered = copy.deepcopy(evidence)
    dropped = altered["ledger"]["charges"].pop()
    with localcontext() as ctx:
        ctx.prec = 128
        altered["native_receipts"] = 47
        altered["native_client_cost_usd"] = str(Decimal(altered["native_client_cost_usd"]) - Decimal(dropped["cost_usd"]))
        altered["known_client_cost_usd"] = str(Decimal(altered["known_client_cost_usd"]) - Decimal(dropped["cost_usd"]))
        altered["commitment_if_new_allocation_usd"] = str(Decimal(altered["commitment_if_new_allocation_usd"]) - Decimal(dropped["cost_usd"]))
    del altered["receipt_bindings"][dropped["source"]]
    variants.append(("consistently-dropped-native-session", altered))
    observed = []
    for name, variant in variants:
        probe = OUT / (name + ".json")
        with probe.open("x") as stream:
            stream.write(json.dumps(variant, sort_keys=True, indent=2) + "\n")
        check = copy.deepcopy(reg)
        check["ledger"] = variant["ledger"]
        check["bindings"][str(probe)] = sha(probe)
        try:
            feedback_admission.verify_accounting(check, {"path": str(probe), "sha256": sha(probe)})
        except ValueError as exc:
            observed.append({"case": name, "rejected": True, "error": str(exc)})
        else:
            raise AssertionError("invalid accounting accepted: " + name)
    package = {"schema": "feedback-accounting-component-qualification-v1", "scope": "accounting only;not paid admission",
               "registration": reg, "proof": proof, "checked": result, "negative_controls": observed,
               "producer_sha256": sha(Path(__file__)),
               "verifier_sha256": sha(REVISION / "tools/effort_protocol/feedback_admission.py")}
    destination = OUT / "qualification.json"
    with destination.open("x") as stream:
        stream.write(json.dumps(package, indent=2, sort_keys=True) + "\n")
    shutil.copyfile(Path(__file__), OUT / "qualify_accounting.py")
    assert json.loads(destination.read_bytes()) == package
    print(json.dumps({"accounting": result, "negative_controls": observed, "qualification_sha256": sha(destination)}))


if __name__ == "__main__":
    main()
