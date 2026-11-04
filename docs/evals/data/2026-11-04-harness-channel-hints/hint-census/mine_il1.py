import sqlite3, json, collections
con = sqlite3.connect("file:.codescout/usage.db?mode=ro", uri=True)
rows = con.execute("select id, tool_name, outcome, coalesce(err_family,''), coalesce(cc_session_id,session_id,''), coalesce(agent_id,''), input_json, called_at from tool_calls order by id").fetchall()
g=collections.defaultdict(list)
for r in rows: g[(r[4],r[5])].append(r)
def inp(r):
    try: return json.loads(r[6]) if r[6] else {}
    except Exception: return {}
n=0; nxt=collections.Counter(); samepath_noforce=0; chain=collections.Counter()
for k,l in g.items():
    for i,r in enumerate(l):
        if r[3]!='il1_read_overlaps_symbol': continue
        n+=1; p=inp(r).get('path')
        x=l[i+1] if i+1<len(l) else None
        if not x: nxt['end']+=1; continue
        ix=inp(x)
        if x[1]=='read_file' and ix.get('path')==p:
            nxt['same_path_force' if ix.get('force') else 'same_path_noforce']+=1
        elif x[1]=='symbols': nxt['symbols']+=1
        elif x[1]=='read_file' and x[3]=='il1_read_overlaps_symbol': nxt['other_il1']+=1
        else: nxt['other:'+x[1]]+=1
print(n, nxt.most_common(10))
# how many il1 errors were force=false first-time reads of file with range, how many later got forced
