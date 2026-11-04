import sqlite3, json, re, collections
con = sqlite3.connect("file:.codescout/usage.db?mode=ro", uri=True)
rows = con.execute("select id, input_json, output_json, coalesce(cc_session_id,session_id,''), coalesce(agent_id,''), called_at from tool_calls where tool_name='run_command' and outcome='success' order by id").fetchall()
n=zero=phrase=0; ex=[]; nextfound=0; byday=collections.Counter()
by={}
for r in rows: by.setdefault((r[3],r[4]),[]).append(r)
pos={r[0]:(k,i) for k,l in by.items() for i,r in enumerate(l)}
for id_,ij,oj,s,a,ca in rows:
    try: d=json.loads(ij); cmd=d.get('command','') or ''
    except Exception: continue
    m=re.match(r"\s*(?:cd \S+ && )?(grep|rg)\s+(?:-\S+\s+)*(?:-e\s+)?(['\"])(.+?)\2",cmd)
    if not m or '|' in cmd.split(m.group(0))[-1]: continue
    n+=1
    try:
        t=json.loads(oj)[0]['text']; e=json.loads(t)
    except Exception: continue
    if not isinstance(e,dict): continue
    out=(e.get('stdout') or '').strip()
    if out or e.get('exit_code') not in (1,): continue
    zero+=1
    pat=m.group(3)
    if len(pat.split())>=2 and not re.search(r'\\\||\|',pat):
        phrase+=1; byday[ca[:10]>='2026-10-28']+=1
        k,i=pos[id_]; 
        nxt=by[k][i+1:i+3]
        if any(x[1] and x[1]!=ij and ('grep' in x[1] or 'rg ' in x[1] or 'symbols' in x[1]) for x in nxt): nextfound+=1
        if len(ex)<3: ex.append((id_,pat[:60]))
print('simple grep/rg run_command',n,'zero-hit exit1',zero,'multiword phrase zero',phrase,'last7d',byday[True],'followed by another search in next2',nextfound,ex)
