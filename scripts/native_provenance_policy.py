"""Conservative lifecycle policy. Call only after prefix integrity AND live fence verification."""
from dataclasses import dataclass
from native_provenance import _require, _transitions

@dataclass(frozen=True)
class CertifiedExecution:
    operations: tuple
    wrapper_call_ids: frozenset[str]

_WRAPPERS = {"exec", "wait", "functions.exec", "functions.wait"}

def certify_execution(prefix, history_rows):
    _transitions(prefix.events, prefix.call_id)
    identity = prefix.identity
    _require(identity["root"] == identity["thread"], "child lineage is not certified")
    metas = [row.get("payload") for _, row in history_rows if row.get("type") == "session_meta"]
    _require(len(metas) == 1 and isinstance(metas[0], dict), "native history metadata is ambiguous")
    meta = metas[0]
    _require(meta.get("session_id") == identity["root"] and meta.get("id") == identity["thread"],
             "native history identity differs")
    _require(not any(meta.get(key) is not None for key in
                     ("forked_from_id", "history_base", "parent_thread_id", "subagent_history_start_ordinal")),
             "external lineage is not certified")
    _require(not isinstance(meta.get("source"), dict) or not meta["source"].get("subagent"),
             "child source is not certified")
    _require(not any(row.get("type") == "compacted" or
                     (row.get("type") == "event_msg" and isinstance(row.get("payload"),dict)
                      and row["payload"].get("type") == "item_completed"
                      and isinstance(row["payload"].get("item"),dict)
                      and row["payload"]["item"].get("type") == "ContextCompaction")
                     for _, row in history_rows), "compacted lineage is not certified")
    receipts, dispatches, outcomes, mcp, raw_results = {}, set(), {}, {}, {}
    cells, callbacks, closed, runtime_calls = {}, {}, {}, {}
    for event in prefix.events[1:]:
        kind, data = event["kind"], event["data"]
        call = data.get("call_id")
        _require(kind not in {"incomplete", "lineage"}, "native epoch is incomplete or has unsupported lineage")
        if kind == "received":
            _require(data.get("arguments_complete") is True and isinstance(data.get("arguments"), (dict,str)),
                     "native arguments were omitted")
            _require(isinstance(data.get("source"), dict) and isinstance(data.get("cwd"), str),
                     "native source/cwd is missing")
            receipts[call] = data
            source = data["source"]
            if source.get("kind") == "code_mode":
                key = (source.get("cell_id"), source.get("runtime_id"))
                _require(all(isinstance(v,str) and v for v in key) and key not in runtime_calls,
                         "runtime call identity was reused")
                runtime_calls[key] = call
            else:
                _require(source.get("kind") == "direct", "native source is unsupported")
        elif kind == "dispatch":
            dispatches.add(call)
        elif kind in {"outcome", "cancelled_before_dispatch"}:
            outcomes[call] = kind
        elif kind == "mcp_dispatch":
            _require(call not in mcp and data.get("fence_capability") is True,
                     "MCP dispatch lacks negotiated fence capability or was replayed")
            _require(isinstance(data.get("arguments"), dict) and isinstance(data.get("tool"), str),
                     "actual MCP arguments are missing")
            mcp[call] = data
        elif kind == "mcp_outcome":
            _require(call in mcp and call not in raw_results and isinstance(data.get("result"), dict),
                     "MCP result is missing, reordered or replayed")
            raw_results[call] = data["result"]
        elif kind == "cell_start":
            cell = data.get("cell_id")
            _require(isinstance(cell,str) and cell and cell not in cells and call not in cells.values(),
                     "cell or wrapper binding was reused")
            cells[cell] = call
        elif kind == "cell_callback":
            cell, runtime, seq = data.get("cell_id"), data.get("runtime_id"), data.get("sequence")
            _require(cell in cells and cell not in closed and isinstance(runtime,str) and runtime
                     and type(seq) is int and seq > 0, "unsupported or late cell callback")
            slots = callbacks.setdefault(cell,{})
            _require(seq not in slots and runtime not in slots.values(), "cell callback identity was reused")
            slots[seq] = runtime
        elif kind == "cell_closed":
            cell, seq = data.get("cell_id"), data.get("final_sequence")
            _require(cell in cells and cell not in closed and type(seq) is int and seq >= 0,
                     "cell close is unsupported or replayed")
            closed[cell] = seq
        elif kind == "cell_retired":
            _require(data.get("cell_id") in cells, "unknown cell retired")
        elif kind not in {"checkpoint", "cancelled_after_dispatch"}:
            _require(False, "unsupported execution event")
    current = receipts.get(prefix.call_id)
    _require(current is not None and prefix.call_id in mcp and prefix.call_id not in outcomes,
             "formatter is not the current native invocation")
    actual = mcp[prefix.call_id]
    args = actual["arguments"]
    _require(actual["tool"] == "run_command" and args.get("command") in
             {"bash scripts/fmt-mine.sh", "./scripts/fmt-mine.sh"} and
             all(key not in args or args[key] is False for key in ("run_in_background","interactive")),
             "only the exact foreground formatter may remain pending")
    source = current["source"]
    active_cell = source.get("cell_id") if source.get("kind") == "code_mode" else None
    _require(active_cell is None or active_cell in cells, "formatter cell is not bound")
    for cell, wrapper in cells.items():
        _require(wrapper in receipts and receipts[wrapper].get("scope") == "wrapper"
                 and receipts[wrapper].get("tool") in _WRAPPERS, "cell wrapper receipt is missing")
        slots = callbacks.get(cell,{})
        last = closed.get(cell)
        if cell == active_cell:
            _require(last is None, "formatter arrived after cell close")
            last = max(slots,default=0)
        else:
            _require(last is not None and wrapper in outcomes, "another cell remains active")
        _require(set(slots) == set(range(1,last+1)), "issued cell slots are incomplete")
        for runtime in slots.values():
            _require((cell,runtime) in runtime_calls, "callback has no native broker receipt")
    _require(set(runtime_calls) == {(cell,runtime) for cell,slots in callbacks.items() for runtime in slots.values()},
             "broker receipt has no wire callback")
    operations = []
    wrappers = set()
    for call, receipt in receipts.items():
        if receipt.get("scope") == "wrapper":
            _require(receipt.get("tool") in _WRAPPERS, "unsupported orchestration wrapper")
            wrappers.add(call)
            _require(call in outcomes or (active_cell is not None and cells[active_cell] == call),
                     "unrelated wrapper remains pending")
            continue
        _require(receipt.get("scope") == "operation", "unsupported native call scope")
        if call == prefix.call_id:
            continue
        _require(call in outcomes, "another native call remains pending")
        if outcomes[call] == "cancelled_before_dispatch":
            _require(call not in dispatches and call not in mcp, "cancelled call was dispatched")
            continue
        _require(call in mcp and call in raw_results, "native effect lacks supported actual MCP result")
        operations.append((receipt,mcp[call],raw_results[call]))
    history_wrappers = set()
    for _,row in history_rows:
        payload = row.get("payload")
        if row.get("type") != "response_item" or not isinstance(payload,dict):
            continue
        if payload.get("type") in {"function_call","custom_tool_call"} and payload.get("name") in _WRAPPERS:
            call = payload.get("call_id")
            _require(isinstance(call,str) and call not in history_wrappers, "history wrapper replay is ambiguous")
            history_wrappers.add(call)
    _require(history_wrappers == wrappers, "native journal does not cover every history wrapper")
    return CertifiedExecution(tuple(operations),frozenset(wrappers))
