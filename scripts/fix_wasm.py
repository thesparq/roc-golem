import sys, re

wat = open(sys.argv[1]).read()

m = re.search(r'\(import "env" "memory" \(memory \([^)]+\) (\d+)\)\)', wat)
pages = m.group(1) if m else '1'

wat = re.sub(
    r'\s+\(import "env" "memory" \(memory \([^)]+\) \d+\)\)',
    '', wat)
wat = re.sub(
    r'\s+\(import "env" "roc_dealloc" .+\)',
    '', wat)

lines = wat.split('\n')
insert_at = 1
for i, line in enumerate(lines):
    if '(import' in line.strip():
        insert_at = i + 1

lines.insert(insert_at, f'  (memory (export "memory") {pages})')

open(sys.argv[1], 'w').write('\n'.join(lines))
