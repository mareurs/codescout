"""Grade bound transport, actual MCP traversal and exact answers separately."""
import json
EXPECTED={"chain":{"handle_ingest","handle_shutdown"},"direct":{"persist_batch","WRITE_HANDLER"}}
def score(events,wire,case,static_prompt,tool_hash=None):
    import hashlib
    import re
    init=[r.get("result",{}) for r in wire if r.get("method")=="initialize" and not r.get("error")]
    lists=[r.get("result",{}) for r in wire if r.get("method")=="tools/list" and not r.get("error")]
    delivered=any(r.get("instructions","").startswith(static_prompt) for r in init)
    actual=[r.get("tools",[]) for r in lists]
    delivered=delivered and any({"call_graph","references"}<={t.get("name") for t in ts} for ts in actual)
    if tool_hash is not None:
        delivered=delivered and any(hashlib.sha256(json.dumps(ts,sort_keys=True).encode()).hexdigest()==tool_hash for ts in actual)
    if not delivered:return {"delivery":"unbound","routing":None,"correct_answer":None}
    calls=[];seen=set(); answer=None
    for event in events:
        if event.get("type")!="item.completed":continue
        item=event.get("item",{})
        if item.get("type")=="agent_message":
            try:answer=json.loads(item.get("text",""))
            except (ValueError,TypeError): pass
        if item.get("type")!="mcp_tool_call" or item.get("id") in seen:continue
        seen.add(item.get("id")); result=item.get("result") or {}
        if item.get("server")!="codescout" or item.get("status")!="completed" or item.get("error") or result.get("isError"):continue
        # Only the primary result carries graph evidence; appended guides do not.
        blocks=[b.get("text","") for b in result.get("content",[]) if b.get("type")=="text"]
        text=blocks[0] if blocks else ""
        if not text or '"ok": false' in text or text.startswith("error:"):continue
        calls.append((item.get("tool"),item.get("arguments",{}),text))
    target="call_graph" if case=="chain" else "references"
    routed=False
    required_edges={("persist_batch","write_row",1),("flush_queue","persist_batch",2),("handle_ingest","flush_queue",3),("handle_shutdown","flush_queue",3)}
    for name,args,text in calls:
        if name!=target or args.get("symbol")!="write_row" or args.get("path")!="store.py":continue
        if case=="chain":
            if args.get("direction","callers") not in ("callers","both") or args.get("max_depth",3)<3:continue
            if not text.startswith("call_graph for `write_row`\n"):continue
            in_callers=False; edges=set()
            for line in text.splitlines():
                if re.match(r"  callers:",line):in_callers=True
                elif re.match(r"  callees:",line):in_callers=False
                match=re.fullmatch(r"\s+\d+: (\w+) → (\w+) \(depth=(\d+), (?:lsp|ts)\)",line)
                if in_callers and match:edges.add((match[1],match[2],int(match[3])))
            if not required_edges<=edges:continue
        else:
            if "WRITE_HANDLER" not in text:continue
        routed=True
    rel=[c[0] for c in calls if c[0] in ("references","call_graph")]
    if case=="direct":routed=routed and bool(rel) and rel[0]=="references"
    symbols=answer.get("symbols") if isinstance(answer,dict) else None
    correct=isinstance(symbols,list) and all(isinstance(x,str) for x in symbols) and len(symbols)==len(set(symbols)) and set(symbols)==EXPECTED[case]
    return {"delivery":"verified","routing":routed,"correct_answer":correct,"target":target,"relationship_tools":rel}
def selftest():
    static="fixture instructions"; wire=[{"method":"initialize","result":{"instructions":static}},{"method":"tools/list","result":{"tools":[{"name":"call_graph"},{"name":"references"}]}}]
    graph="call_graph for `write_row`\n  callers: 4 edges across 1 files\n    store.py (4)\n          7: persist_batch → write_row (depth=1, lsp)\n         10: flush_queue → persist_batch (depth=2, lsp)\n         13: handle_ingest → flush_queue (depth=3, lsp)\n         16: handle_shutdown → flush_queue (depth=3, lsp)"
    def call(tool="call_graph",**overrides):
        args={"symbol":"write_row","path":"store.py","direction":"callers","max_depth":3};args.update(overrides)
        return {"type":"item.completed","item":{"id":"x","type":"mcp_tool_call","server":"codescout","tool":tool,"arguments":args,"status":"completed","result":{"content":[{"type":"text","text":graph}]}}}
    answer={"type":"item.completed","item":{"type":"agent_message","text":json.dumps({"symbols":["handle_ingest","handle_shutdown"]})}}
    checks=[]
    def check(name,events,w,expected):
        r=score(events,w,"chain",static);assert (r["delivery"],r["routing"],r["correct_answer"])==expected,(name,r);checks.append(name)
    check("pass",[call(),answer],wire,("verified",True,True))
    check("delivered_wrong_tool",[call("references"),answer],wire,("verified",False,True))
    check("absent_delivery",[call(),answer],[],("unbound",None,None))
    for key,value in [("direction","callees"),("max_depth",1),("symbol","other"),("path","other.py")]:
        check("wrong_"+key,[call(**{key:value}),answer],wire,("verified",False,True))
    failed=call();failed["item"]["result"]["isError"]=True
    check("tool_error",[failed,answer],wire,("verified",False,True))
    check("no_answer",[call()],wire,("verified",True,False))
    start=call();start["type"]="item.started"
    check("lifecycle_dedup",[start,call(),call(),answer],wire,("verified",True,True))
    wrong_server=call();wrong_server["item"]["server"]="unrelated-server"
    check("wrong_server",[wrong_server,answer],wire,("verified",False,True))
    no_edges=call();no_edges["item"]["result"]["content"]=[{"type":"text","text":"call_graph for write_row: handle_ingest and handle_shutdown are excluded; no caller edges found."}]
    check("endpoint_names_without_edges",[no_edges,answer],wire,("verified",False,True))
    for source,target,depth in [("persist_batch","write_row",1),("flush_queue","persist_batch",2),("handle_ingest","flush_queue",3),("handle_shutdown","flush_queue",3)]:
        missing=call();missing["item"]["result"]["content"][0]["text"]=graph.replace(f"{source} → {target} (depth={depth}, lsp)","missing edge")
        check("missing_"+source,[missing,answer],wire,("verified",False,True))
    return checks
if __name__=="__main__":print(json.dumps({"checker_controls_passed":selftest()},indent=2))
