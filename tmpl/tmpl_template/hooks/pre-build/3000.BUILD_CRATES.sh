#!/bin/bash
# shellcheck source=../lib/utils.sh
# shellcheck source=Kam/KamHooks/lib/utils.sh
. "$KAM_HOOKS_ROOT/lib/utils.sh"
# shellcheck source=../lib/build_utils.sh
# shellcheck source=Kam/KamHooks/lib/build_utils.sh
. "$KAM_HOOKS_ROOT/lib/build_utils.sh"

log_warn " comment out to enable build crates!" && exit 0

require_command cargo "cargo not found ."
