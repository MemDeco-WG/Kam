#!/bin/bash

. "$KAM_HOOKS_ROOT/lib/utils.sh"

# optionally update changelog using commitizen.
require_command cz "commitizen not found; cannot update changelog." || exit 0

log_info "Updating CHANGELOG.md using commitizen (cz)..."
cz ch

