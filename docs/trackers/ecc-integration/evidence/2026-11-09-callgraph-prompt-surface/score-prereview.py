"""Grade bound transport, actual MCP traversal and exact answers separately."""
import json
EXPECTED={"chain":{"handle_ingest","handle_shutdown"},"direct":{"persist_batch","WRITE_HANDLER"}}
def score(events,wire,case,static_prompt,tool_hash=None):
    import hashlib
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
        if item.get("status")!="completed" or item.get("error") or result.get("isError"):continue
        text="\n".join(b.get("text","") for b in result.get("content",[]) if b.get("type")=="text")
        if not text or '"ok": false' in text or text.startswith("error:"):continue
        calls.append((item.get("tool"),item.get("arguments",{}),text))
    target="call_graph" if case=="chain" else "references"
    routed=False
    for name,args,text in calls:
        if name!=target or args.get("symbol")!="write_row" or args.get("path")!="store.py":continue
        if case=="chain":
            if args.get("direction","callers") not in ("callers","both") or args.get("max_depth",3)<3:continue
            if "call_graph for" not in text or not all(x in text for x in EXPECTED["chain"]):continue
        else:
            if "WRITE_HANDLER" not in text:continue
        routed=True
    rel=[c[0] for c in calls if c[0] in ("references","call_graph")]
    if case=="direct":routed=routed and bool(rel) and rel[0]=="references"
    symbols=answer.get("symbols") if isinstance(answer,dict) else None
    correct=isinstance(symbols,list) and len(symbols)==len(set(symbols)) and set(symbols)==EXPECTED[case]
    return {"delivery":"verified","routing":routed,"correct_answer":correct,"target":target,"relationship_tools":rel}
def selftest():
    static="fixture instructions"; wire=[{"method":"initialize","result":{"instructions":static}},{"method":"tools/list","result":{"tools":[{"name":"call_graph"},{"name":"references"}]}}]
    def call(tool="call_graph",**overrides):
        args={"symbol":"write_row","path":"store.py","direction":"callers","max_depth":3};args.update(overrides)
        return {"type":"item.completed","item":{"id":"x","type":"mcp_tool_call","tool":tool,"arguments":args,"status":"completed","result":{"content":[{"type":"text","text":"call_graph for write_row: handle_ingest handle_shutdown WRITE_HANDLER"}]}}}
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
    return checks
if __name__=="__main__":print(json.dumps({"checker_controls_passed":selftest()},indent=2))
