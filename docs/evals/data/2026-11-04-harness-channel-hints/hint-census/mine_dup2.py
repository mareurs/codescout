import sqlite3, json, collections, re, hashlib
con = sqlite3.connect("file:.codescout/usage.db?mode=ro", uri=True)
rows = con.execute("select id, tool_name, outcome, coalesce(cc_session_id,session_id,''), coalesce(agent_id,''), input_json, output_json, called_at, latency_ms from tool_calls where tool_name in ('run_command','edit_file','edit_code','create_file','memory') order by id").fetchall()
allrows = con.execute("select id, tool_name, coalesce(cc_session_id,session_id,''), coalesce(agent_id,'') from tool_calls order by id").fetchall()
g=collections.defaultdict(list)
for r in allrows: g[(r[2],r[3])].append(r)
pos={}
for k,l in g.items():
    for i,r in enumerate(l): pos[r[0]]=(k,i)
byid={}
rc=[r for r in rows if r[1]=='run_command' and r[2]=='success']
last={}  # (sess,agent,cmd)->row
cat=collections.Counter(); tot=0; same_out=0; ex={}; waste_ms=collections.Counter()
for r in rc:
    try: d=json.loads(r[5]); cmd=d.get('command','')
    except Exception: continue
    if d.get('run_in_background') or d.get('interactive'): continue
    key=(r[3],r[4],cmd, d.get('cwd'), d.get('workspace'))
    p=last.get(key)
    last[key]=r
    if not p: continue
    # within <=5 calls in same agent log
    k1,i1=pos[p[0]]; k2,i2=pos[r[0]]
    if k1!=k2 or i2-i1>5: continue
    # any write between?
    between=g[k1][i1+1:i2]
    wr=any(b[1] in ('edit_file','edit_code','create_file') for b in between)
    w=cmd.split()[0] if cmd.split() else ''
    if w=='cd': 
        m=re.search(r'&&\s*(\S+)',cmd); w=m.group(1) if m else w
    sub=' '.join(cmd.split()[:2]) if w in('git','cargo') else w
    tot+=1
    same = hashlib.md5((p[6] or '').encode()).hexdigest()==hashlib.md5((r[6] or '').encode()).hexdigest()
    cat[(sub, 'write_between' if wr else 'no_write', 'same_out' if same else 'diff_out')]+=1
    ex.setdefault((sub,'write_between' if wr else 'no_write','same_out' if same else 'diff_out'),(p[0],r[0]))
    if not wr and same: waste_ms[sub]+=r[8]
print(tot)
for k,v in cat.most_common(22): print(v,k,ex[k])
print('same-cmd same-output no-write wasted ms by cmd', waste_ms.most_common(6))
