import sqlite3, json, collections, re, os
con = sqlite3.connect("file:.codescout/usage.db?mode=ro", uri=True)
rows = con.execute("select id, tool_name, input_json, error_msg from tool_calls where err_family='path_not_found'").fetchall()
c = collections.Counter(); ex={}
for id_, t, ij, em in rows:
    try: p = json.loads(ij).get('path') or json.loads(ij).get('relative_path') or ''
    except Exception: p=''
    m = re.search(r"searched (\S+)\)", em or '')
    s = m.group(1) if m else ''
    d = os.path.dirname(p) or '.'
    d = re.sub(r'/home/marius/\.cache/codescout-resident/.*','<resident-cache>',d)
    d = re.sub(r'.*/worktrees/[^/]+','<wt>',d)
    d = re.sub(r'^/home/marius/work/claude/codescout/','',d)
    key=(t, d[:50])
    c[key]+=1; ex.setdefault(key,id_)
for k,v in c.most_common(18): print(v,k,ex[k])
# does the file exist now (relative to repo) -- for relative paths in docs/issues
tot=ex_=0
for id_, t, ij, em in rows:
    try: p=json.loads(ij).get('path','')
    except Exception: continue
    if p.startswith('docs/issues/') or p.startswith('docs/'):
        tot+=1
        if os.path.exists(p): ex_+=1
print('docs/* relative notfound',tot,'exists now',ex_)
