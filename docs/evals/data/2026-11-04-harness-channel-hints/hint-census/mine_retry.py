import sqlite3, json, collections, sys
con = sqlite3.connect("file:.codescout/usage.db?mode=ro", uri=True)
rows = con.execute("select id, tool_name, outcome, coalesce(err_family,''), coalesce(cc_session_id,session_id,''), coalesce(agent_id,''), input_json, substr(called_at,1,19) from tool_calls order by id").fetchall()
g = collections.defaultdict(list)
for r in rows: g[(r[4], r[5])].append(r)
res = collections.defaultdict(lambda: collections.Counter())
ex = collections.defaultdict(list)
for k, calls in g.items():
    for i, c in enumerate(calls):
        if c[2] == 'success': continue
        fam = (c[1], c[3])
        res[fam]['n'] += 1
        nxt = calls[i+1:i+4]
        # same family again within next 3 calls
        if any(x[1]==c[1] and x[3]==c[3] and x[2]!='success' for x in nxt):
            res[fam]['repeat_err3'] += 1
            if len(ex[fam])<2: ex[fam].append(c[0])
        # next call is same tool and succeeded
        if nxt and nxt[0][1]==c[1] and nxt[0][2]=='success': res[fam]['next_same_tool_ok'] += 1
        if nxt and nxt[0][1]=='run_command' and c[1]!='run_command': res[fam]['next_is_runcmd']+=1
for fam, c in sorted(res.items(), key=lambda x:-x[1]['n'])[:40]:
    print(fam, dict(c), ex[fam])
