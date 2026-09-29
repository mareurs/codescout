"""Count saved responses, not new model calls. Usage: python recount_saved.py T9B_DIR."""
import collections
import hashlib
import json
import pathlib
import sys


def main():
    root = pathlib.Path(sys.argv[1])
    data = json.loads((root / "gate.json").read_text())
    rows = data["results"]
    counts = collections.Counter()
    sequences = collections.Counter()
    usage = collections.Counter()
    answers = collections.Counter()
    logs = sorted((root / "logs").glob("*.log"))
    hashes = {}
    for log in logs:
        hashes[log.name] = hashlib.sha256(log.read_bytes()).hexdigest()
        events = [json.loads(line) for line in log.read_text().split("\n--- stderr ---")[0].splitlines() if line.strip()]
        sequences[tuple(e["type"] + (":" + e["item"]["type"] if "item" in e else "") for e in events)] += 1
        for event in events:
            if event["type"] == "turn.completed":
                usage.update(event["usage"])
            if event["type"] == "item.completed" and event["item"]["type"] == "agent_message":
                answers[event["item"]["text"]] += 1
    saved_answers = collections.Counter(v["raw"] for r in rows for v in r["votes"])
    assert saved_answers == answers, "saved votes differ from logged answers"
    for row in rows:
        assert len(row["votes"]) == 3
        for field in ("is_mistake", "is_correction", "detectability"):
            value, n = collections.Counter(v[field] for v in row["votes"]).most_common(1)[0]
            assert row["majority"][field] == (value if n >= 2 else None)
        counts[row["kind"]] += 1
    corrections = [r for r in rows if r["mode"] == "correction"]
    audits = [r for r in rows if r["kind"] == "rtd" and r["mode"] == "audit"]
    assert len(corrections) == 21 and len(audits) == 8
    controls = [r for r in rows if r["kind"] == "control"]
    # Recompute agreements directly from the saved per-vote detectability values.
    agreement = sum(r["majority"]["detectability"] == r["expected"]["detectability"] for r in corrections)
    measured = {"detectability": agreement,
                "peer_yes_flags": sum(r["expected"]["peer_yes"] and r["majority"]["is_mistake"] is True for r in audits),
                "yes_flags": sum(r["majority"]["is_mistake"] is True for r in audits),
                "control_fires": sum(r["majority"]["is_mistake"] is True for r in controls)}
    assert all(n == data["score"]["checks"][key]["count"] for key, n in measured.items())
    result = {
        "model_calls": 0, "saved_json_sha256": hashlib.sha256((root / "gate.json").read_bytes()).hexdigest(),
        "rows_by_kind": dict(counts), "vote_logs": len(logs),
        "saved_answers_match_logs": True, "majorities_checked": ["is_mistake", "is_correction", "detectability"],
        "attempt_counts": dict(collections.Counter(len(v["attempts"]) for r in rows for v in r["votes"])),
        "event_sequences": {" > ".join(k):v for k,v in sequences.items()}, "usage": dict(usage),
        "controls_is_mistake": {str(k):v for k,v in collections.Counter(r["majority"]["is_mistake"] for r in controls).items()},
        "corrections": [{"id":r["id"], "expected":r["expected"], "detectability":r["majority"]["detectability"],
                         "is_correction_votes":[v["is_correction"] for v in r["votes"]]} for r in corrections],
        "log_sha256": hashes,
    }
    # Expected is a metadata object in this instrument; preserve its shape above.
    result["registered_score"] = data["score"]["checks"]
    result["independently_recounted_scores"] = measured
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
