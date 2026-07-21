import sys, re

wat = open(sys.argv[1]).read()

# Replace env function imports with local no-op functions (in-place, preserves indices)
wat = re.sub(
    r'\(import "env" "([^"]+)" \(func (\([^)]+\)) (\([^)]+\))\)\)',
    r'(func $\1 \2 \3)',
    wat)

# Replace env memory import with local memory export (in-place, preserves indices)
wat = re.sub(
    r'\(import "env" "memory" \(memory (\([^)]+\)) (\d+)\)\)',
    r'(memory (export "memory") \1 \2)',
    wat)

open(sys.argv[1], 'w').write(wat)
