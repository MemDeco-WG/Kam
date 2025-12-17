#!/bin/sh
# shellcheck shell=ash

# =============================================================================
# 工具模块注册表
# =============================================================================

# 注册所有可用模块
KAM_MODULES=""

# 注册模块
# 用法: register_module "模块名" "描述"
register_module() {
    module="$1"
    desc="$2"
    eval "KAM_MODULE_DESC_${module}=\"${desc}\""
    KAM_MODULES="${KAM_MODULES} ${module}"
}

# =============================================================================
# 模块将从 kam_utils 目录自动发现（不再手工列举）
# =============================================================================

# =============================================================================
# 自动发现并注册自定义拓展模块
# =============================================================================

# 自动扫描并注册 kam_utils 目录中的所有 .sh 文件（排除内部模块）
_discover_custom_modules() {
    _discover_custom_modules_kam_utils_dir="${KAM_UTILS_DIR:-}"
    if [ -z "$_discover_custom_modules_kam_utils_dir" ]; then
        # 使用 $0 的路径作为回退（如果为相对路径则转换为绝对路径）
        _discover_custom_modules_kam_utils_dir="$(dirname "$0")"
        [ "$_discover_custom_modules_kam_utils_dir" = "." ] && _discover_custom_modules_kam_utils_dir="$(pwd)"
    fi

    # 扫描所有 .sh 文件（排除 _ 开头的内部文件）
    for module_file in "${_discover_custom_modules_kam_utils_dir}"/*.sh; do
        [ -f "$module_file" ] || continue

        # 获取文件名（不含路径和扩展名）
        _discover_custom_modules_module_name=$(basename "$module_file" .sh)

        # 跳过内部模块（_开头）
        case "${_discover_custom_modules_module_name}" in
            _*) continue ;;
        esac

        # 尝试从文件中提取模块描述
        desc=""
        if [ -r "$module_file" ]; then
            # 查找文件开头的描述注释
            desc=$(head -n 10 "$module_file" | grep -E "^#.*模块.*：" | head -n 1 | sed 's/^#[[:space:]]*//')
            [ -z "$desc" ] && desc="自定义模块：${_discover_custom_modules_module_name}"
        fi

        # 注册模块
        register_module "$_discover_custom_modules_module_name" "$desc"
    done
}

# 执行自动发现
_discover_custom_modules
