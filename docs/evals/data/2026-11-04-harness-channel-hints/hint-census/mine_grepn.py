import sqlite3, json, re, collections
con = sqlite3.connect("file:.codescout/usage.db?mode=ro", uri=True)
rows = con.execute("select id, tool_name, input_json, output_json, coalesce(cc_session_id,session_id,''), coalesce(agent_id,''), called_at from tool_calls order by id").fetchall()
prod={}  # (sess,handle)->(tool,input)
res=collections.Counter(); ex={}
for id_,t,ij,oj,s,a,ca in rows:
    if t=='run_command' and ij:
        try: cmd=json.loads(ij).get('command','')
        except Exception: cmd=''
        for m in re.finditer(r'\bgrep\b((?:\s+-\S+)*)\s[^|;&]*?(@tool_[0-9a-f]+)',cmd):
            flags=m.group(1)
            if re.search(r'-[a-zA-Z]*n',flags):
                p=prod.get((s,m.group(2)))
                k=(p[0] if p else 'unknown', 'src_path' if p and re.search(r'\.(rs|py|ts|js|go|kt|java|mjs)\b',p[1] or '') else 'other')
                res[k]+=1; ex.setdefault(k,id_)
    if oj:
        for h in set(re.findall(r'(@tool_[0-9a-f]{6,})',oj[:3000])):
            if t!='run_command': prod.setdefault((s,h),(t,ij))
print(res.most_common(10)); print(ex)
