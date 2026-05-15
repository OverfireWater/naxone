import json
import os
import shutil

p = os.path.expanduser(r'~\.naxone\vhosts.json')
shutil.copy(p, p + '.bak')
print(f'已备份: {p}.bak')

with open(p, encoding='utf-8') as f:
    data = json.load(f)

before = len(data)
removed = []
kept = []
for v in data:
    root = v['document_root'].replace('/', os.sep).lower()
    is_dev = (os.sep + 'target' + os.sep + 'debug' + os.sep) in root \
        or (os.sep + 'target' + os.sep + 'release' + os.sep) in root
    if is_dev:
        removed.append((v['id'], v['document_root']))
    else:
        kept.append(v)

print(f'\n原 vhost 数: {before}')
print(f'清掉 dev 残留: {len(removed)}')
for vid, root in removed:
    print(f'  - {vid:<30} {root}')
print(f'\n保留正式 vhost: {len(kept)}')
for v in kept:
    print(f'  + {v["id"]:<30} {v["document_root"]}')

with open(p, 'w', encoding='utf-8') as f:
    json.dump(kept, f, indent=2, ensure_ascii=False)
print(f'\n已写回 {p}')
