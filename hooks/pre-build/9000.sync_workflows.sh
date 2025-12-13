#!/bin/bash

# $CI
if [ -n "$CI" ]; then
	exit 0
fi

# Load helpers
. "$KAM_HOOKS_ROOT/lib/utils.sh"


# cp workflows
cp -rf "$KAM_PROJECT_ROOT/KamModuleX/.github/workflows" "$KAM_PROJECT_ROOT/.github/"
log_info "Synchronized GitHub workflows from KamModuleX to main project."

# cp to tmpl/*/.github/workflows
for tmpl_dir in "$KAM_PROJECT_ROOT/tmpl"/*/; do
	cp -rf "$KAM_PROJECT_ROOT/.github/workflows" "$tmpl_dir/.github/"
	log_info "Synchronized GitHub workflows to template: $tmpl_dir"
done

exit 0
