#!/bin/sh
# shellcheck shell=ash
# =============================================================================
# 系统等待模块 - 内部函数（非公开API）
# =============================================================================

# 等待启动完成（内部实现）
_wait_boot_impl() {
    run_quiet resetprop -w sys.boot_completed 0
    _wait_boot_impl_sleep="${1:-}"
    [ -n "$_wait_boot_impl_sleep" ] && sleep "$_wait_boot_impl_sleep"
}

# 等待解锁（设备解锁）（内部实现）
_wait_unlock_impl() {
    _wait_unlock_impl_sleep="${1:-}"
    _wait_boot_impl "$_wait_unlock_impl_sleep"
    until [ -d /sdcard/Android ]; do sleep 1; done
    [ -n "$_wait_unlock_impl_sleep" ] && sleep "$_wait_unlock_impl_sleep"
}

# 等待网络连接（内部实现）
_wait_net_impl() {
	_wait_net_impl_timeout="${1:-30}"
	_wait_net_impl_count=0
	while [ "$_wait_net_impl_count" -lt "$_wait_net_impl_timeout" ]; do
		ping -c 1 8.8.8.8 >/dev/null 2>&1 && return 0
		sleep 1
		_wait_net_impl_count=$((_wait_net_impl_count+1))
	done
	return 1
}
