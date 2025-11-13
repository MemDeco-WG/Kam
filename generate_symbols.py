#!/usr/bin/env python3

import os
import sys
from pathlib import Path

def generate_symbols(src_dir, output_file):
    symbols = []
    src_path = Path(src_dir)

    # Find all .rs files recursively
    for rs_file in src_path.rglob("*.rs"):
        try:
            with open(rs_file, 'r', encoding='utf-8') as f:
                for line_num, line in enumerate(f, start=1):
                    if line.strip().startswith('pub '):
                        symbols.append(f"{rs_file}:{line_num}:{line.rstrip()}")
        except Exception as e:
            print(f"Error reading {rs_file}: {e}", file=sys.stderr)

    # Write to output file
    with open(output_file, 'w', encoding='utf-8') as f:
        for symbol in symbols:
            f.write(symbol + '\n')

    print(f"Public symbols generated in {output_file}")

if __name__ == "__main__":
    src_dir = "src"
    output_file = "symbols.txt"
    generate_symbols(src_dir, output_file)
