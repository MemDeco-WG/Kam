#!/bin/bash
set -e

# Setup test environment
RM_DIR="test_env_v4"
rm -rf "$RM_DIR"
mkdir "$RM_DIR"
cd "$RM_DIR"

echo "=== 1. Creating a template project ==="
# Create a template called 'mytmpl'
../target/debug/kam init mytmpl --tmpl --force

cat >> mytmpl/kam.toml <<EOF
[kam.tmpl.variables.my_custom_var]
var_type = "string"
required = false
default = "default"
EOF

echo "=== 2. Testing Implicit Local Path (no ./ prefix) ==="
# 'mytmpl' directory exists in CWD. 'kam init -t mytmpl' should find it now.
../target/debug/kam init myproj_implicit -t mytmpl --var my_custom_var="implicit" --force

if [ ! -f "myproj_implicit/kam.toml" ]; then
    echo "Error: implicit local path discovery failed"
    exit 1
fi
echo "Implicit path success."

echo "=== 3. Testing Fallback to _template suffix ==="
# 'kam' matches no local file, but matches 'kam_template' built-in (after suffixing).
../target/debug/kam init myproj_fallback -t kam --force

if [ ! -f "myproj_fallback/kam.toml" ]; then
    echo "Error: Fallback to _template failed for built-in 'kam'"
    exit 1
fi
# Verify it's actually the kam template (should have correct id or structure)
grep 'id = "myproj_fallback"' myproj_fallback/kam.toml > /dev/null
echo "Fallback success."

echo "=== All checks passed ==="
