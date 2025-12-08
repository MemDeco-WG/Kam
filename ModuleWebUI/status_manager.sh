#!/bin/bash

# ModuleWebUI 状态管理脚本
# 专注于状态文件写入和处理管理

# 配置变量
MODULE_PATH="${MODULE_PATH:-/data/adb/modules/ModuleWebUI}"
STATUS_DIR="$MODULE_PATH/"
LOG_DIR="$MODULE_PATH/logs"
STATUS_FILE="$STATUS_DIR/info.txt"
LOG_FILE="$LOG_DIR/status.log"

# 创建必要目录
init_dirs() {
    mkdir -p "$STATUS_DIR" "$LOG_DIR"
}

# 日志记录函数
log_message() {
    local level="$1"
    local message="$2"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    echo "[$timestamp] [$level] $message" >> "$LOG_FILE"
}

# 写入状态信息
write_status() {
    local status="$1"
    local pid="$2"
    local start_time="$3"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    
    init_dirs
    
    cat > "$STATUS_FILE" << EOF
status=$status
pid=${pid:-}
startTime=${start_time:-}
lastUpdate=$timestamp
EOF
    
    log_message "INFO" "状态更新: $status"
}

# 检查当前状态
check_status() {
    if [[ -f "$STATUS_FILE" ]]; then
        cat "$STATUS_FILE"
    else
        echo "status=unknown"
        echo "pid="
        echo "startTime="
        echo "lastUpdate=$(date '+%Y-%m-%d %H:%M:%S')"
    fi
}

# 初始化状态文件
init_status() {
    write_status "stopped" "" ""
    log_message "INFO" "状态文件初始化完成"
}

# 更新为运行状态
set_running() {
    local pid="$1"
    local start_time="${2:-$(date '+%Y-%m-%d %H:%M:%S')}"
    write_status "running" "$pid" "$start_time"
}

# 更新为停止状态
set_stopped() {
    write_status "stopped" "" ""
}

# 更新为错误状态
set_error() {
    local error_msg="${1:-未知错误}"
    write_status "error" "" ""
    log_message "ERROR" "模块错误: $error_msg"
}

# 更新为正常退出状态
set_normal_exit() {
    write_status "normal-exit" "" ""
    log_message "INFO" "模块正常退出"
}

# 监控模式 - 持续检查状态
monitor() {
    local interval="${1:-5}"
    log_message "INFO" "开始状态监控，间隔: ${interval}秒"
    
    while true; do
        # 这里可以添加实际的状态检测逻辑
        # 例如检查进程是否存在、服务是否响应等
        
        if [[ -f "$STATUS_FILE" ]]; then
            local current_status=$(grep "^status=" "$STATUS_FILE" | cut -d'=' -f2)
            local current_pid=$(grep "^pid=" "$STATUS_FILE" | cut -d'=' -f2)
            
            # 如果状态为running但进程不存在，更新为stopped
            if [[ "$current_status" == "running" && -n "$current_pid" ]]; then
                if ! kill -0 "$current_pid" 2>/dev/null; then
                    log_message "WARN" "进程 $current_pid 不存在，更新状态为stopped"
                    set_stopped
                fi
            fi
        fi
        
        sleep "$interval"
    done
}

# 显示帮助信息
show_help() {
    cat << EOF
ModuleWebUI 状态管理脚本

用法: $0 <命令> [参数]

命令:
  init                    初始化状态文件
  check                   检查当前状态
  running <pid> [time]    设置为运行状态
  stopped                 设置为停止状态
  error [message]         设置为错误状态
  normal-exit             设置为正常退出状态
  monitor [interval]      监控模式（默认5秒间隔）
  help                    显示此帮助信息

示例:
  $0 init
  $0 check
  $0 running 1234
  $0 stopped
  $0 error "配置文件错误"
  $0 monitor 10

EOF
}

# 主程序
case "$1" in
    init)
        init_status
        ;;
    check)
        check_status
        ;;
    running)
        if [[ -z "$2" ]]; then
            echo "错误: 需要提供进程ID"
            exit 1
        fi
        set_running "$2" "$3"
        ;;
    stopped)
        set_stopped
        ;;
    error)
        set_error "$2"
        ;;
    normal-exit)
        set_normal_exit
        ;;
    monitor)
        monitor "$2"
        ;;
    help|--help|-h)
        show_help
        ;;
    "")
        echo "错误: 需要提供命令"
        show_help
        exit 1
        ;;
    *)
        echo "错误: 未知命令 '$1'"
        show_help
        exit 1
        ;;
esac