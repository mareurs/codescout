import sqlite3, json, collections, re, os
con = sqlite3.connect("file:.codescout/usage.db?mode=ro", uri=True)
rows = con.execute("select id, input_json, error_msg, agent_id, cc_session_id from tool_calls where err_family='path_not_found' and tool_name='read_file'").fetchall()
c=collections.Counter(); ex={}
for id_, ij, em, ag, cs in rows:
    p=json.loads(ij).get('path','') if ij else ''
    m=re.search(r"searched (\S+)\)", em or ''); s=m.group(1) if m else ''
    root = s[:-len(p)] if p and s.endswith(p) else s
    root = re.sub(r'(/worktrees/)[^/]+', r'\1<id>', root)
    isdoc = p.startswith('docs/') or '/docs/' in p
    key=(root[:70], isdoc, p.endswith('.md'), 'agent' if ag else 'main')
    c[key]+=1; ex.setdefault(key,id_)
for k,v in c.most_common(14): print(v,k,ex[k])
