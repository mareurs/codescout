"""Run trusted skill enumeration blocks, without inferring occupancy or reporting a peer count."""
import pathlib,re,subprocess
base=pathlib.Path('/home/marius/.codex/plugins/cache/sdd-misc-plugins/codescout-companion/1.21.0/skills/reaching-peer-sessions')
rows=''
for name in ['claude-code','codex']:
    section=(base/'references'/(name+'.md')).read_text().split('## Enumerate\n',1)[1].split('## Identify me\n',1)[0]
    block=re.search(r'```bash\n(.*?)\n```',section,re.S).group(1)
    result=subprocess.run(['bash','-c',block],text=True,capture_output=True,check=True)
    rows+=result.stdout
merge=re.search(r'```bash\n(.*?)\n```',(base/'SKILL.md').read_text(),re.S).group(1)
result=subprocess.run(['bash','-c',merge],input=rows,text=True,capture_output=True,check=True)
private=pathlib.Path('/home/marius/work/claude/codescout/.codescout/ecc-bfs-identity-private-20261110/precommit-peer-inventory.log')
private.write_text(result.stdout);private.chmod(0o600)
print(result.stdout)
