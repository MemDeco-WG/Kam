#!/bin/bash
# shellcheck source=../lib/utils.sh
# shellcheck source=hooks/lib/utils.sh
. "$KAM_HOOKS_ROOT/lib/utils.sh"

log_warn " comment out to enable build crates!" && exit 0

# shellcheck source=../lib/build_utils.sh
# shellcheck source=hooks/lib/build_utils.sh
. "$KAM_HOOKS_ROOT/lib/build_utils.sh"

# Build crates

build_multi_arch "$(detect_build_tool)"
