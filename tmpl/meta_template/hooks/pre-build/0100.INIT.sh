#!/bin/bash
# shellcheck source=../lib/utils.sh
. "$KAM_HOOKS_ROOT/lib/utils.sh"

log_warn " comment out to enable INIT!" && exit 0

git submodule update --init --recursive # if needed

git submodule update --remote --merge --recursive # if needed

# Additional initialization steps can be added here
