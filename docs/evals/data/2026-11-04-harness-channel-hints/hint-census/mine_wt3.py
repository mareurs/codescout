import sqlite3, json, re, os, collections
MAIN='/home/marius/work/claude/codescout/'
con = sqlite3.connect("file:.codescout/usage.db?mode=ro", uri=True)
rows = con.execute("select id, tool_name, input_json, error_msg, called_at, coalesce(agent_id,'') from tool_calls where err_family='path_not_found'").fetchall()
c=collections.Counter(); ids=collections.defaultdict(list)
for id_,t,ij,em,ca,ag in rows:
    m=re.search(r"searched (\S+)\)", em or ''); s=m.group(1) if m else ''
    if not s or s.startswith(MAIN) and '/worktrees/' not in s and '/.worktrees/' not in s: continue
    root=None
    for pat in (r'(.*/\.claude/worktrees/[^/]+/)', r'(.*/\.codex/worktrees/[^/]+/[^/]+/)', r'(.*/\.worktrees/[^/]+/)', r'(.*codescout\.worktrees/[^/]+/)'):
        mm=re.match(pat,s)
        if mm: root=mm.group(1); break
    if not root: continue
    rel=s[len(root):]
    ex=os.path.exists(MAIN+rel)
    wk='w>=10-28' if ca[:10]>='2026-10-28' else 'before'
    k=(wk,'exists_in_main' if ex else 'absent_in_main')
    c[k]+=1; ids[k].append(id_)
for k,v in sorted(c.items()): print(v,k,ids[k][-2:])
