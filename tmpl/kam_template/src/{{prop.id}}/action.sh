#!/system/bin/sh
#
# action.sh
#
# 🚨 模块卡片按钮点击时执行，需新版
# This script is executed when the user clicks the "Action" button in the KernelSU Manager
# or triggers an action via the Module WebUI.
#
# ---------------------------------------------------------------------------------------
# EXECUTION CONTEXT
# ---------------------------------------------------------------------------------------
# - TRIGGER:      User interaction in KernelSU Manager (Action button) or WebUI.
# - ENV:          Runs in KernelSU's BusyBox ash shell (Standalone Mode).
#                 $MODDIR is set to the module's directory.
#                 $KSU_MODULE is set to the module ID.
# - OUTPUT:       Standard output (echo) is usually displayed to the user (e.g., as a Toast).
#
# ---------------------------------------------------------------------------------------
# REQUIREMENTS
# ---------------------------------------------------------------------------------------
# 🚨 版本要求
# Minimum supported versions (required):
# - Magisk (stable): 28.0+
# - Magisk (alpha builds): alpha28001+ (e.g., 28001 or newer)
# - KernelSU kernel module: build 11986 / KernelSU v1.0.2+
# - (M/R)KernelSU (NEXT): build 12300+
#
# Notes:
# - These version constraints are required for KernelSU Manager (UI), KernelSU kernel
#   driver functionality, and the `ksud` utility that Module WebUI/Action scripts rely on.
# - 'alpha28001+' refers to Magisk alpha/canary builds and may be required for
#   certain alpha-only features. Test accordingly if you're using alpha builds.
# - '(M/R)KernelSU (NEXT)' refers to Main/Release NEXT builds of KernelSU where
#   newer manager and kernel build IDs are used.
# - If your module requires a higher version, add required runtime checks before
#   invoking version-specific features or APIs.
#
# ---------------------------------------------------------------------------------------
# MODULE CONFIGURATION (ksud)
# ---------------------------------------------------------------------------------------
# 🚨 仅限新版本
# KernelSU provides a persistent key-value store for modules.
#
# Get value:      val=$(ksud module config get <key>)
# Set persist:    ksud module config set <key> <value>
# Set temp:       ksud module config set --temp <key> <value>
# Delete:         ksud module config delete <key>
# List all:       ksud module config list
#
# ---------------------------------------------------------------------------------------

MODDIR=${0%/*}

# ---------------------------------------------------------------------------------------
# EXAMPLE: Simple Feature Toggle
# ---------------------------------------------------------------------------------------
# This example toggles a feature flag and updates the module description.

# Read current state (default to false if empty)
# STATE=$(ksud module config get feature_enabled)
# [ -z "$STATE" ] && STATE="false"

# if [ "$STATE" = "true" ]; then
#     # Disable feature
#     ksud module config set feature_enabled "false"
#     ksud module config set override.description "Feature is currently DISABLED"
#     echo "Feature disabled"
# else
#     # Enable feature
#     ksud module config set feature_enabled "true"
#     ksud module config set override.description "Feature is currently ENABLED"
#     echo "Feature enabled"
# fi

# ---------------------------------------------------------------------------------------
# EXAMPLE: Managed Features
# ---------------------------------------------------------------------------------------
# Modules can control KernelSU internal features.
# Supported keys: manage.su_compat, manage.kernel_umount, manage.enhanced_security

# ksud module config set manage.su_compat true
# ui_print "Enforced SU Compatibility"

# ---------------------------------------------------------------------------------------
# DEFAULT ACTION
# ---------------------------------------------------------------------------------------

ui_print "Action script executed!"
ui_print "Edit action.sh to add custom logic."


# ---------------------------------------------------------------------------------
# Use Nga utils
# -----------------------------------------------------------------------------------
# run2null echo "这句话将消失"
# run22null echo "这句话不会消失" # 仅移除标准错误
# echo $(until_key) # 输出按下的按键

# 音量+	KEY_VOLUMEUP	up
# 音量-	KEY_VOLUMEDOWN	down
# 电源键	KEY_POWER	power
# 静音键	KEY_MUTE	mute
# 肩键等额外按键	KEY_FX	fX

# echo $(until_key_up_down) # 输出按下的按键，只能为 up 或 down
# echo $(until_key_up_down_power) # 输出按下的按键，只能为 up 或 down 或 power

# echo $(until_key_up) # 输出按下的按键，只能为 up
# echo $(until_key_down) # 输出按下的按键，只能为 down
# echo $(until_key_power) # 输出按下的按键，只能为 power

# goto_url "https://bilibili.com" # 跳转 bilibili
# goto_app "ren.shiror.su/dev.oom_wg.ssu.SSUUI" # 打开app

# echo "我现在在 '$(get_work_dir .)' 正好好待着呢" # 输出后将会是 “我现在在 '<当前目录的父目录路径>' 正好好待着呢”

# newline # 不传入内容，默认打印一行空行

# newline 3 # 传入内容，打印指定行数的空行

# ---------------------------------------------------------------------------------
# Use Nga utils
# -----------------------------------------------------------------------------------
