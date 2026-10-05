"""Check relative Markdown file links without fetching external websites."""
import re
import subprocess
from pathlib import Path
from urllib.parse import unquote, urlsplit

root=Path(__file__).resolve().parents[1]
files=subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard','*.md'],cwd=root,text=True).splitlines()
errors=[]; checked=0
for name in sorted(set(files)):
    path=root/name
    if not path.is_file():continue
    text=re.sub(r'```.*?```','',path.read_text(),flags=re.S)
    for match in re.finditer(r'!?\[[^\]]*\]\(([^)]+)\)',text):
        target=match.group(1).split(' "',1)[0].strip('<>')
        url=urlsplit(target)
        if url.scheme or url.netloc or not url.path:continue
        checked+=1
        destination=(path.parent/unquote(url.path)).resolve()
        if not destination.is_relative_to(root) or not destination.exists():
            errors.append(f'{name}: missing/outside repository link: {target}')
if errors:raise SystemExit('\n'.join(errors))
print(f'Documentation links: {checked} relative file targets exist (external URLs/anchors not checked).')
