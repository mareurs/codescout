import sqlite3, json, collections, re
con = sqlite3.connect("file:.codescout/usage.db?mode=ro", uri=True)
for fam in ('il3_pipe_to_trimmer','il3_shell_on_source'):
    rows=con.execute("select id, coalesce(cc_session_id,session_id,''), coalesce(agent_id,''), input_json, called_at from tool_calls where err_family=? and called_at>='2026-10-28' order by id",(fam,)).fetchall()
    per=collections.Counter((r[1],r[2]) for r in rows)
    print(fam,'recent7d',len(rows),'callers',len(per),'callers with >=3',sum(1 for v in per.values() if v>=3),'max',max(per.values()), 'subagent share', sum(1 for r in rows if r[2])/len(rows))
    shapes=collections.Counter()
    for r in rows:
        try: c=json.loads(r[3]).get('command','')
        except Exception: c=''
        if fam=='il3_pipe_to_trimmer':
            if re.search(r'\|\s*(tail|head|grep|sed|awk|sort|wc)[^|]*(;|&&|\|\|)',c): s='filter then ;/&&/||'
            elif '|&' in c: s='|&'
            elif '<<' in c: s='heredoc'
            elif '@' in c and re.search(r'@(cmd|bg|tool|file)_',c): s='buffer-ref'
            elif re.search(r'\|\s*(cut|tr|jq|python3?|xargs|cat|tee|sort)\b',c) and not re.search(r'\|\s*(tail|head|grep)\b[^|]*$',c): s='last stage not terminal filter'
            else: s='other'
        else:
            s='path:'+(re.search(r'(\.\w+)\b',c).group(1) if re.search(r'(\.\w+)\b',c) else '?')
        shapes[s]+=1
    print(shapes.most_common(7))
