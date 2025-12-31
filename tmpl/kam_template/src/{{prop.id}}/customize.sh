# shellcheck shell=ash
# {{prop.name}} customize.sh - 最小 wrapper
#
# 约束：入口脚本只做最小工作（加载 kamfw + 初始化 HOME/KAM_HOME + 调度 phase）

SKIPUNZIP=1

# 1) 解压 kamfw（安装期需要；SKIPUNZIP=1 时需手动）
unzip -o "$ZIPFILE" "lib/kamfw/*" -d "$MODPATH" >&2 || abort "! Failed to extract lib/kamfw"

# 2) 加载 kamfw 运行时（提供 import/kamfw/run 等能力）
[ -f "$MODPATH/lib/kamfw/.kamfwrc" ] && . "$MODPATH/lib/kamfw/.kamfwrc" || abort "! .kamfwrc missing"

# 3) 安装期的模块目录
#    注意：安装期常见变量是 MODPATH；运行期常见变量是 MODDIR。
export MODDIR="${MODDIR:-$MODPATH}"

# 4) 生命周期调度（业务逻辑应在 kamfw_phase_install 内实现/覆盖）
import __runtime__
kamfw run install -- "$@"
