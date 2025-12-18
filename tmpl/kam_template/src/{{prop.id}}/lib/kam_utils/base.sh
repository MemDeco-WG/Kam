# shellcheck shell=ash
# =============================================================================
# 基础工具模块 - 公开API
# =============================================================================

# 加载内部模块
_kam_utils_dir="$(dirname "${0}")"
[ -f "${_kam_utils_dir}/_base.sh" ] && . "${_kam_utils_dir}/_base.sh"

# 基础打印函数
pprint() {
    if command -v ui_print >/dev/null 2>&1; then
        ui_print "$1"
    else
        # 兼容非 Magisk 环境
        [ -z "$OUTFD" ] && echo "$1" || echo "ui_print $1\nui_print" >>"/proc/self/fd/$OUTFD"
    fi
}

# 带前缀的打印
msg() { pprint "> $1"; }
err() { pprint "⚠️ $1"; }
warn() { pprint "⚠️ $1"; }
info() { pprint "ℹ️ $1"; }

# 换行函数
newline() {
    count="${1:-1}"
    while [ "$count" -gt 0 ]; do
        pprint ""
        count=$((count-1))
    done
}

# 打印多行
plns() { for line in "$@"; do pprint "$line"; done; }

# 安全删除
rmrf() { [ -e "$1" ] && rm -rf "$@" 2>/dev/null; }

# 复制并设置权限
cp_perm() {
    src="$1" dest="$2" perm="${3:-0644}"
    [ -f "$src" ] && cp "$src" "$dest" && chmod "$perm" "$dest"
}

# 设置目录权限
set_perm_dir() { find "$@" -type d -exec chmod 0755 {} + 2>/dev/null; }

# 设置文件权限
set_perm_file() { find "$@" -type f -exec chmod "${1:-0644}" {} + 2>/dev/null; }

# 设置可执行权限
set_exec() { chmod a+x "$@" 2>/dev/null; }

# 设置 SELinux 上下文
set_selinux() { chcon -R u:object_r:system_file:s0 "$@" 2>/dev/null; }

# 运行并忽略输出（重命名，由原 `null` 改为更明确的 `run_quiet`）
run_quiet() { "$@" >/dev/null 2>&1; }

# 运行并忽略错误
ignore_err() { "$@" 2>/dev/null; }

# 检查命令是否存在
command_exists() { command -v "$1" >/dev/null 2>&1; }

# 获取给定路径的目录（绝对路径）
dir_of() { dirname "$(readlink -f "$1")"; }

# 字符串是否等于列表中任一值
one_of() {
    str="$1"
    shift
    for target in "$@"; do [ "$target" = "$str" ] && return; done
    return 1
}

# 格式化日期
# 用法: fdate
fdate() {
    date +"%Y-%m-%d %H:%M:%S.%3N"
}

# 日志记录
# 用法: log "INFO|ERROR|WARNING|DEBUG" "消息"
log() {
    level="$1"
    message="$2"
    logfile="${LOG_FILE:-}"

    # 定义颜色（错误色可通过环境变量 KAM_COLOR_ERROR 配置，格式: #RRGGBB 或 RRGGBB；默认日系暖橙 #FF9150）
    _log_normal="\033[0m"
    # 读取并解析 KAM_COLOR_ERROR（支持 #RRGGBB 或 RRGGBB），解析失败回退到默认值 FF9150
    _KAM_COLOR_ERR="${KAM_COLOR_ERROR:-#FF9150}"
    _kam_color_hex="${_KAM_COLOR_ERR#\#}"
    if [ "${#_kam_color_hex}" -ne 6 ]; then
        _kam_color_hex="FF9150"
    fi
    _log_r_hex="$(printf "%s" "${_kam_color_hex}" | cut -c1-2)"
    _log_g_hex="$(printf "%s" "${_kam_color_hex}" | cut -c3-4)"
    _log_b_hex="$(printf "%s" "${_kam_color_hex}" | cut -c5-6)"
    _log_r_dec=$(printf "%d" "0x${_log_r_hex}" 2>/dev/null || printf "%d" "0xFF")
    _log_g_dec=$(printf "%d" "0x${_log_g_hex}" 2>/dev/null || printf "%d" "0x91")
    _log_b_dec=$(printf "%d" "0x${_log_b_hex}" 2>/dev/null || printf "%d" "0x50")
    _log_red=$(printf '\033[38;2;%d;%d;%dm' "${_log_r_dec}" "${_log_g_dec}" "${_log_b_dec}")
    _log_green="\033[1;32m"
    _log_yellow="\033[1;33m"
    _log_blue="\033[1;34m"

    # 根据级别选择颜色
    _log_color=""
    case $level in
        INFO) _log_color="${_log_blue}" ;;
        ERROR) _log_color="${_log_red}" ;;
        WARNING) _log_color="${_log_yellow}" ;;
        DEBUG) _log_color="${_log_green}" ;;
        *) _log_color="${_log_green}" ;;
    esac

    # 格式化消息
    _log_current_time=$(fdate)
    _log_formatted_message="${_log_current_time} [$level]: $message"

    # 输出到控制台或日志文件
    if [ -t 1 ]; then
        printf '%b\n' "${_log_color}${_log_formatted_message}${_log_normal}"
    elif [ -n "$logfile" ]; then
        # 确保日志文件存在
        [ ! -f "$logfile" ] && touch "$logfile" && chmod 600 "$logfile"
        echo "${_log_formatted_message}" >> "$logfile" 2>&1
    else
        echo "${_log_formatted_message}"
    fi
}
