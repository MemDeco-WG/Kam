#!/bin/bash
# This file is a compatibility loader; implementation lives in split parts.
__kam_hooks_utils_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ ! -d "${__kam_hooks_utils_dir}/utils_runtime" ]; then
    printf '%s\n' "Missing split library: ${__kam_hooks_utils_dir}/utils_runtime" >&2
    return 1 2>/dev/null || exit 1
fi
. "${__kam_hooks_utils_dir}/utils_runtime/environment.sh" || { __kam_part_status=$?; unset __kam_hooks_utils_dir; return "$__kam_part_status" 2>/dev/null || exit "$__kam_part_status"; }
. "${__kam_hooks_utils_dir}/utils_runtime/module_permissions.sh" || { __kam_part_status=$?; unset __kam_hooks_utils_dir; return "$__kam_part_status" 2>/dev/null || exit "$__kam_part_status"; }
unset __kam_hooks_utils_dir __kam_part_status
