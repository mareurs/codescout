import sqlite3, json, collections
con = sqlite3.connect("file:.codescout/usage.db?mode=ro", uri=True)
rows = con.execute("select id, tool_name, outcome, coalesce(cc_session_id,session_id,''), coalesce(agent_id,''), input_json, overflowed, substr(output_json,1,0), latency_ms from tool_calls order by id").fetchall()
g=collections.defaultdict(list)
for r in rows: g[(r[3],r[4])].append(r)
dup=collections.Counter(); dupex={}; wasted=collections.Counter()
for k,calls in g.items():
    for i,c in enumerate(calls):
        if c[2]!='success' or not c[5]: continue
        for x in calls[i+1:i+6]:
            if x[1]==c[1] and x[5]==c[5] and x[2]=='success':
                dup[c[1]]+=1; dupex.setdefault(c[1],(c[0],x[0])); break
print('identical success repeat within next 5 same agent:', dup.most_common(8), {k:dupex[k] for k in dup})
ov=collections.Counter(); 
for r in rows:
    if r[6]: ov[r[1]]+=1
print('overflowed', ov.most_common(8))
