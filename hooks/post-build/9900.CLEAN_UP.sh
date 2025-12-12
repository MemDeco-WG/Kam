#!/bin/bash

. "$KAM_HOOKS_ROOT/lib/utils.sh"

log_warn " comment out to enable !" && exit 0

if [ "$KAM_RELEASE_ENABLED" != "1" ]; then
    log_info "KAM_RELEASE_ENABLED != 1, skipping clean up"
    exit 0
fi

rm -r "$KAM_DIST_DIR" || log_error "failed to clean up."

log_success "Cleaned up $KAM_DIST_DIR"
