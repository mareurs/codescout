"""Overlay a transport-bound live prefix on one native history, never a cached batch."""
from dataclasses import replace
from pathlib import Path
from codex_history import _native_mcp_operation
from native_provenance import ProofError, MAX_STREAM_BYTES, _json, _regular, _require, verify_checkpoint
from native_provenance_fence import verify_fence
from native_provenance_policy import certify_execution
from provenance_records import CoverageGap, HistoryBatch

_OPAQUE_REASONS = {
    "opaque Codex custom-tool wrapper; inner execution is not recoverable",
    "unsupported Codex tool or orchestration wrapper",
}

def apply_checkpoint(path, batch, environ, root):
    if not environ.get("CODESCOUT_NATIVE_PROVENANCE"):
        return batch
    try:
        # Only the exact history captured by this invocation may borrow its proof.
        candidate = _json(environ["CODESCOUT_NATIVE_PROVENANCE"])
        _require(isinstance(candidate,dict), "native certificate is not an object")
        checkpoint = candidate.get("checkpoint")
        _require(isinstance(checkpoint,dict) and isinstance(checkpoint.get("identity"),dict),
                 "native checkpoint identity is missing")
        if checkpoint["identity"].get("history") != str(path):
            return batch
        certificate = verify_fence(environ,root)
        native = {"root":environ.get("CODEX_SESSION_ID"),"thread":environ.get("CODEX_THREAD_ID")}
        prefix = verify_checkpoint(checkpoint,native,path,certificate["invocation_id"])
        with _regular(path) as stream:
            raw = stream.read(MAX_STREAM_BYTES+1)
        _require(len(raw) <= MAX_STREAM_BYTES and raw.endswith(b"\n"), "native history is oversized or truncated")
        rows = []
        for number,line in enumerate(raw.splitlines(),1):
            row = _json(line)
            _require(isinstance(row,dict), "native history contains a nonobject")
            rows.append((number,row))
        execution = certify_execution(prefix,rows)
        removable = set()
        for number,row in rows:
            payload = row.get("payload")
            if row.get("type") == "response_item" and isinstance(payload,dict) and payload.get("call_id") in execution.wrapper_call_ids:
                removable.add(f"{path}:{number}")
        additions = []
        by_id = {op.source_id:op for op in batch.operations}
        for receipt,actual,result in execution.operations:
            # An observed shell command may run arbitrary scripts. Its target set
            # is not a completeness proof, so never discharge an opaque wrapper
            # that contains a prior shell effect.
            _require(actual["tool"] != "run_command", "prior shell effects are not fully recoverable")
            cwd = Path(receipt["cwd"])
            _require(cwd.is_absolute(), "native operation cwd is not absolute")
            item = {"id":receipt["call_id"],"server":"codescout","tool":actual["tool"],
                    "arguments":actual["arguments"],"status":"completed","result":result}
            op,reason = _native_mcp_operation({"thread_id":native["thread"],"item":item},None,
                native["root"],native["thread"],None,False,cwd)
            _require(op is not None and not reason, "actual MCP operation is unsupported")
            previous = by_id.get(op.source_id)
            if previous is not None:
                _require((previous.name,previous.input,previous.cwd,previous.outcome) ==
                         (op.name,op.input,op.cwd,op.outcome), "native operation replay conflicts with history")
            else:
                additions.append(op)
        # Verify the same live lease/generation after reading every artifact.
        verify_fence(environ,root)
        gaps = [gap for gap in batch.gaps if not
                (gap.source_id in removable and gap.reason in _OPAQUE_REASONS)]
        return HistoryBatch(batch.operations+additions,gaps)
    except (ProofError,OSError,ValueError,TypeError,KeyError,UnicodeError,RecursionError) as error:
        return HistoryBatch(list(batch.operations),list(batch.gaps)+[
            CoverageGap(str(path),f"native execution checkpoint refused: {error}",root)])
