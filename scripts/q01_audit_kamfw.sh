#!/usr/bin/env bash
set -euo pipefail

# Q01 诊断脚本：扫描 kam_template/kamfw 相关 redlines 并输出可复现证据
# 输出：.collab/outbox/Q01__audit__YYYYMMDD-HHMM.md

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

out_dir=".collab/outbox"
mkdir -p "$out_dir"

stamp="$(date +%Y%m%d-%H%M)"
out_file="$out_dir/Q01__audit__${stamp}.md"

audit_scope=(
  "tmpl/kam_template/src/{{prop.id}}/.local/bin"
  "tmpl/kam_template/src/{{prop.id}}/lib/kamfw"
  "src"
)

# helper: run a command, capture stdout+stderr as fenced block
run_cmd() {
  local cmd="$1"
  printf "\n\n#### 命令\n\n\`\`\`bash\n%s\n\`\`\`\n\n#### 输出\n\n\`\`\`\n" "$cmd" >>"$out_file"
  # shellcheck disable=SC2086
  bash -lc "$cmd" >>"$out_file" 2>&1 || true
  printf "\n\`\`\`\n" >>"$out_file"
}

# helper: include file snippet
snippet() {
  local file="$1"; local from="${2:-1}"; local to="${3:-160}"
  if [[ -f "$file" ]]; then
    printf "\n\n**文件**: \`%s\`\n\n\`\`\`\n" "$file" >>"$out_file"
    nl -ba "$file" | sed -n "${from},${to}p" >>"$out_file" || true
    printf "\n\`\`\`\n" >>"$out_file"
  else
    printf "\n\n**文件**: \`%s\` (不存在)\n" "$file" >>"$out_file"
  fi
}

# Determine status/risk based on findings
p0_hit=false
p1_hit=false

# quick scanners
hit_or_true() {
  local path="$1"
  if rg -n "\|\|\s*true\b" "$path" >/dev/null 2>&1; then
    p0_hit=true
  fi
}

hit_swallow_err() {
  local path="$1"
  # typical swallow patterns: "2>/dev/null || true", bare "|| true", or "|| :"
  if rg -n "\|\|\s*(true|:)\b" "$path" >/dev/null 2>&1; then
    p0_hit=true
  fi
  if rg -n "2>/dev/null\s*\|\|\s*(true|:)\b" "$path" >/dev/null 2>&1; then
    p0_hit=true
  fi
}

hit_duplicate_fallback_print() {
  local path="$1"
  # heuristic: multiple definitions of print/ui_print/printf fallback across files
  # not strict, but indicates copy/paste
  local count
  count=$(rg -n "^(print|ui_print)\s*\(\)" "$path" 2>/dev/null | wc -l | tr -d ' ')
  if [[ "${count:-0}" -ge 2 ]]; then
    p0_hit=true
  fi
}

hit_inconsistent_output() {
  local path="$1"
  # mix of echo/printf/print/ui_print
  if rg -n "\becho\b" "$path" >/dev/null 2>&1 && rg -n "\bprint\b\s*\(" "$path" >/dev/null 2>&1; then
    p1_hit=true
  fi
}

# initialize report
cat >"$out_file" <<EOF
---
audit_date: "$(date -Iseconds)"
auditor: Q01
scope:
  - tmpl/kam_template/src/{{prop.id}}/.local/bin/*
  - tmpl/kam_template/src/{{prop.id}}/lib/kamfw/*.sh + .kamfwrc
  - src/** (kam init / template render / workflows tera)
redlines:
  - 重复逻辑 (print/ui_print/printf fallback)
  - 隐式回退 (|| true / 吞错 / 默认值掩盖)
  - 不一致 (命名/目录/输出)
  - 左右脑互搏 (严格失败 vs silent continue)
  - 敷衍修复 (为过构建引入债)
---

## (A) 结论摘要

> 该结论由脚本基于启发式扫描自动生成；最终以 Lead 人工复核为准。
EOF

# run scans with reproducible commands
for p in "${audit_scope[@]}"; do
  if [[ -e "$p" ]]; then
    hit_or_true "$p"
    hit_swallow_err "$p"
    hit_duplicate_fallback_print "$p"
    hit_inconsistent_output "$p"
  fi
done

status="可接受"
risk="P2"
if [[ "$p0_hit" == true ]]; then
  status="拒收"
  risk="P0"
elif [[ "$p1_hit" == true ]]; then
  status="需整改"
  risk="P1"
fi

printf "- 状态: **%s**\n- 风险等级: **%s**\n" "$status" "$risk" >>"$out_file"

cat >>"$out_file" <<'EOF'

## (B) 证据
EOF

# Evidence 1: implicit fallback / swallow errors
run_cmd "rg -n \"\\|\\|\\s*(true|:)\\b\" tmpl/kam_template/src/{{prop.id}}/lib/kamfw tmpl/kam_template/src/{{prop.id}}/.local/bin src || true"

# Evidence 2: print/ui_print duplication
run_cmd "rg -n \"^(print|ui_print)\\s*\\(\\)\" tmpl/kam_template/src/{{prop.id}}/lib/kamfw tmpl/kam_template/src/{{prop.id}}/.local/bin || true"

# Evidence 3: printf/echo mixed output style
run_cmd "rg -n \"\\becho\\b|\\bprintf\\b\" tmpl/kam_template/src/{{prop.id}}/lib/kamfw tmpl/kam_template/src/{{prop.id}}/.local/bin || true"

# Pinpoint known hotspots
snippet "tmpl/kam_template/src/{{prop.id}}/.local/bin/kamfw" 1 200
snippet "tmpl/kam_template/src/{{prop.id}}/lib/kamfw/.kamfwrc" 1 220

# Evidence 4: possible Tera confusion in workflows templates (heuristic)
run_cmd "rg -n \"\\{\\{\\s*[^}]+\\s*\\}\\}|\\{\\%\\s*[^%]+\\s*\\%\\}\" src .github/workflows tmpl/kam_template 2>/dev/null || true"

cat >>"$out_file" <<'EOF'

## (C) 整改建议

### 必须遵循的结构性方向（非补丁式）
1) **输出统一**：仅允许一个权威输出 API（例如 `print/ui_print`），禁止各脚本自带 printf/echo fallback。
   - 将 shim 与 .kamfwrc 的输出策略收敛到 `lib/kamfw/logging.sh` 或 `lib/kamfw/rich.sh` 的单点实现。

2) **显式失败**：禁止 `|| true` 吞错（除非对“幂等/可选步骤”有明确注释且记录原因）。
   - 对 `chcon ... || true` 这种场景，要么：
     - 记录为 `warn` 并输出一次明确告警；要么
     - 判断能力（如 `has_command chcon` + 检查返回码）并在关键路径失败。

3) **去重**：重复逻辑（尤其 print/ui_print/printf fallback、ABI 选择）必须抽取到一个库函数；入口脚本只负责调用。

4) **一致性检查**：同一概念（如 MODDIR/MODPATH/KAMFW_DIR）定义/优先级必须在一个地方定义，并在其他脚本中只读取，不要各自推导。

### 建议由谁执行
- **SR01**：重构 kamfw shim 与 .kamfwrc 的输出/错误处理单点化；去除重复 fallback。
- **H01**：补齐 shellcheck 规则与最小自测脚本（至少覆盖 shim 找不到二进制 / ABI 分支）。
- **D01**：将“输出 API / 错误处理哲学”写入决策文档并引用到模板。
EOF

printf "\n\n---\n生成: %s\n" "$out_file" >>"$out_file"

echo "Wrote: $out_file"
