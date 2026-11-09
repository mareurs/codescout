"""Keep the exact pre-action host transcript private; publish only bounded provenance."""
import hashlib
import json
from pathlib import Path
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[4]
source=Path("/home/marius/.codex/sessions/2026/11/09/rollout-2026-11-09T10-24-45-01a1bf68-e650-7f20-911e-b53f86597c27.jsonl")
lines=source.read_bytes().splitlines(keepends=True)
rows=[json.loads(line) for line in lines]
matches=[i for i,row in enumerate(rows) if row.get("type")=="response_item" and row.get("payload",{}).get("role")=="user" and "\n".join(block.get("text","") for block in row["payload"].get("content",[]) if block.get("type")=="input_text").strip()=="ok, continua"]
assert matches,"Exact active user-request event not found"
start=matches[-1]
end=next(i for i in range(start+1,len(rows)) if rows[i].get("type")=="response_item" and rows[i].get("payload",{}).get("role")=="assistant")
data=b"".join(lines[:end])
private=ROOT/".codescout/ecc-create-finalization-private-20261109"
assert (private/".gitignore").read_text()=="*\n"
output=private/"pre-action-controller-input.jsonl"
assert not output.exists(),"Do not overwrite a private immutable input snapshot"
output.write_bytes(data);output.chmod(0o600)
contexts=[row["payload"] for row in rows[:end] if row.get("type")=="turn_context"]
context=contexts[-1] if contexts else {}
record=dict(source=str(source),private_snapshot=str(output),sha256=hashlib.sha256(data).hexdigest(),bytes=len(data),cutoff="before the first assistant response to the latest exact user event 'ok, continua'",user_event_timestamp=rows[start].get("timestamp"),configured_model=context.get("model"),configured_effort=context.get("effort"),cwd=context.get("cwd"),scope="Exact local host transcript prefix retained privately; includes personal policy and prior conversation, never candidate-public input. No backend attestation. Neutral brief and frozen Git tree are separate; any future replay must preregister conditioning/skill-removal and contamination controls. No replay ran.")
(HERE/"session-input-provenance.json").write_text(json.dumps(record,indent=2)+"\n")
print(json.dumps(dict(private_snapshot=str(output),sha256=record["sha256"],bytes=len(data),configured_model=record["configured_model"],configured_effort=record["configured_effort"])))
