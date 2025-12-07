#!/bin/bash

. $KAM_HOOKS_ROOT/lib/utils.sh

require_command "zip"

zip -r $KAM_PROJECT_ROOT/dist/templates.zip $KAM_PROJECT_ROOT/templates || exit 1

log_info "Templates built successfully"
