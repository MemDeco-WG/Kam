#!/bin/bash

# Generate symbols.txt with all public symbols (fn, struct, enum, trait, const, static, type)
# Captures all lines starting with 'pub ' from Rust source files

find src -name "*.rs" -exec awk '/^pub / { print FILENAME ":" NR ":" $0 }' {} + > symbols.txt

echo "Public symbols generated in symbols.txt"
