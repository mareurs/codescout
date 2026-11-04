import sqlite3, json, re, os, collections
con = sqlite3.connect("file:.codescout/usage.db?mode=ro", uri=True)
rows = con.execute("select id, input_json, error_msg, agent_id, cc_session_id, called_at from tool_calls where err_family='path_not_found' and tool_name='read_file'").fetchall()
c=collections.Counter(); ids=collections.defaultdict(list)
for id_,ij,em,ag,cs,ca in rows:
    p=json.loads(ij).get('path','') if ij else ''
    m=re.search(r"searched (\S+)\)", em or ''); s=m.group(1) if m else ''
    mm=re.search(r'/\.claude/worktrees/agent-([0-9a-f]+)/',s)
    if not mm: continue
    own = ag and ag.startswith(mm.group(1)[:8]) or (ag and mm.group(1).startswith(ag[:8]))
    rel=re.sub(r'.*/\.claude/worktrees/agent-[0-9a-f]+/','',s)
    exist=os.path.exists('/home/marius/work/claude/codescout/'+rel)
    k=('own_worktree' if own else 'other_agents_worktree', 'exists_in_main_now' if exist else 'absent_in_main')
    c[k]+=1; ids[k].append(id_)
for k,v in c.most_common(): print(v,k,ids[k][:3])
