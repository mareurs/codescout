import sqlite3, json, collections, re, os
con = sqlite3.connect("file:.codescout/usage.db?mode=ro", uri=True)
rows = con.execute("select id, input_json, error_msg, agent_id, cc_session_id, called_at from tool_calls where err_family='path_not_found' and tool_name='read_file'").fetchall()
n=inmain=0; sess=set(); agents=set(); ids=[]
for id_, ij, em, ag, cs, ca in rows:
    p=json.loads(ij).get('path','') if ij else ''
    m=re.search(r"searched (\S+)\)", em or ''); s=m.group(1) if m else ''
    if '/.claude/worktrees/' in (s or p):
        rel = re.sub(r'.*/\.claude/worktrees/[^/]+/','',p) if p.startswith('/') else p
        n+=1; sess.add(cs); agents.add(ag)
        if os.path.exists('/home/marius/work/claude/codescout/'+rel): inmain+=1
        ids.append((id_,ca[:10]))
print('worktree path_not_found', n, 'exists in main checkout now', inmain, 'sessions', len(sess), 'agents', len(agents))
print(ids[:5], ids[-3:])
# what each such agent did next: count of subsequent successes reading same rel from main path
