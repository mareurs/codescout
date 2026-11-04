import sqlite3, json, collections, re
con = sqlite3.connect("file:.codescout/usage.db?mode=ro", uri=True)
rows = con.execute("select id, coalesce(cc_session_id,session_id,''), coalesce(agent_id,''), input_json, output_json, latency_ms, outcome from tool_calls where tool_name='run_command' order by id").fetchall()
sl=[0,0,0]; slsess=set(); slex=[]
poll=collections.defaultdict(list)
for id_,s,a,ij,oj,lat,oc in rows:
    try: d=json.loads(ij)
    except Exception: continue
    cmd=d.get('command','') or ''
    m=re.match(r'\s*(?:\S+=\S+\s+)*sleep\s+(\d+)',cmd)
    if m and not d.get('run_in_background'):
        sl[0]+=1; sl[1]+=lat; slsess.add(s)
        if int(m.group(1))>=30: sl[2]+=1
        if len(slex)<3: slex.append(id_)
    for h in set(re.findall(r'@bg_[0-9a-f]+',cmd)):
        running = ('"state\\\\": \\\\"running' in (oj or '')) or ('running' in (oj or '') and h in (oj or ''))
        poll[(s,a,h)].append((id_, running, lat))
n3=[(k,v) for k,v in poll.items() if sum(1 for x in v if x[1])>=3]
tot_polls_running=sum(1 for v in poll.values() for x in v if x[1])
print('foreground sleep cmds',sl,'sessions',len(slsess),slex)
print('bg handles referenced',len(poll),'refs while running',tot_polls_running,'handles with >=3 running polls',len(n3), 'in', len(set(k[0] for k,v in n3)),'sessions')
print('example', n3[0][0], [x[0] for x in n3[0][1]][:6] if n3 else None)
top=sorted(poll.items(), key=lambda kv:-sum(1 for x in kv[1] if x[1]))[:3]
print([(k[2],sum(1 for x in v if x[1]), sum(x[2] for x in v)//1000) for k,v in top])
