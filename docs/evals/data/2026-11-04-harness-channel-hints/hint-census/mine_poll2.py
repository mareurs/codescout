import sqlite3, json, collections, re
con = sqlite3.connect("file:.codescout/usage.db?mode=ro", uri=True)
rows = con.execute("select id, substr(called_at,1,10), coalesce(cc_session_id,session_id,''), coalesce(agent_id,''), input_json, output_json, latency_ms from tool_calls where tool_name='run_command' order by id").fetchall()
per=collections.defaultdict(lambda:[0,0,0,0])  # day-bucket -> running polls, sleep calls, handles>=3, agent polls
poll=collections.defaultdict(list)
for id_,day,s,a,ij,oj,lat in rows:
    try: d=json.loads(ij)
    except Exception: continue
    cmd=d.get('command','') or ''
    per_k='>=10-30' if day>='2026-10-30' else '<10-30'
    for h in set(re.findall(r'@bg_[0-9a-f]+',cmd)):
        run = h in (oj or '') and 'running' in (oj or '')
        if run: poll[(s,a,h,per_k)].append(id_)
    if re.match(r'\s*sleep\s+\d+',cmd) and not d.get('run_in_background'): per[per_k][1]+=1
for k,v in poll.items():
    per[k[3]][0]+=len(v)
    if len(v)>=3: per[k[3]][2]+=1
    if k[1]: per[k[3]][3]+=len(v)
for k,v in per.items(): print(k,'polls_while_running',v[0],'foreground_sleep',v[1],'handles>=3polls',v[2],'polls_by_subagents',v[3])
