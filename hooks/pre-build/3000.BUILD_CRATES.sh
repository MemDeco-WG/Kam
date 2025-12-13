#!/bin/bash
. "$KAM_HOOKS_ROOT/lib/utils.sh"

log_warn " comment out to enable build crates!" && exit 0

. "$KAM_HOOKS_ROOT/lib/build_utils.sh"

# Build crates

build_multi_arch "$(detect_build_tool)"
