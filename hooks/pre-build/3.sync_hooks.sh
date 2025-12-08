#!/bin/sh
#
# 3.sync_hooks.sh
#
# Synchronize KamHooks files into tmpl templates (pre-build / post-build / lib)
# This script:
#  - Copies root files (LICENSE, README.md) from KamHooks to each template root
#  - Copies pre-build/post-build scripts and lib utilities into each template hooks dir
#  - Preserves existing template-specific scripts if they exist with different names
#  - Makes copied .sh scripts executable
#
# NOTE:
#  - This script intentionally only copies files that exist in the source.
#  - It will not remove or modify other files in template directories.
#  - It will prefer target lowercase filenames when found (e.g. 1.sync-module-files.sh)
#

# Load helpers
. "$KAM_HOOKS_ROOT/lib/utils.sh"

log_info "Syncing hooks..."

# Define source / target roots
KAM_HOOKS_SRC="${KAM_HOOKS_SRC:-$KAM_PROJECT_ROOT/KamHooks}"
KAM_TMPL_ROOT="${KAM_TMPL_ROOT:-$KAM_PROJECT_ROOT/tmpl}"

# Basic sanity checks
if [ -z "$KAM_PROJECT_ROOT" ]; then
    log_warn "KAM_PROJECT_ROOT is not set; using current directory"
    KAM_PROJECT_ROOT="$(pwd)"
fi

if [ ! -d "$KAM_HOOKS_SRC" ]; then
    log_warn "KamHooks source not found at $KAM_HOOKS_SRC; nothing to sync"
    exit 0
fi

if [ ! -d "$KAM_TMPL_ROOT" ]; then
    log_warn "tmpl directory not found at $KAM_TMPL_ROOT; nothing to sync"
    exit 0
fi

# Helper to copy a file if it exists
copy_if_exists() {
    src="$1"
    dst="$2"
    if [ -f "$src" ]; then
        mkdir -p "$(dirname "$dst")"
        if cp -f "$src" "$dst"; then
            log_info "Copied: $(basename "$src") -> $dst"
        else
            log_warn "Failed to copy: $src -> $dst"
        fi
    fi
}

# Iterate over each template in tmpl/
for tmpl_dir in "$KAM_TMPL_ROOT"/*; do
    [ -d "$tmpl_dir" ] || continue
    tmpl_name=$(basename "$tmpl_dir")
    log_info "Syncing template: $tmpl_name"

    # Ensure hooks structure exists
    mkdir -p "$tmpl_dir/hooks/pre-build" "$tmpl_dir/hooks/post-build" "$tmpl_dir/hooks/lib"

    # Root-level files
    copy_if_exists "$KAM_HOOKS_SRC/LICENSE" "$tmpl_dir/LICENSE"
    copy_if_exists "$KAM_HOOKS_SRC/README.md" "$tmpl_dir/README.md"

    # Copy lib utilities
    if [ -d "$KAM_HOOKS_SRC/lib" ]; then
        for libfile in "$KAM_HOOKS_SRC"/lib/*; do
            [ -f "$libfile" ] || continue
            copy_if_exists "$libfile" "$tmpl_dir/hooks/lib/$(basename "$libfile")"
        done
    fi

    # Copy hooks for pre-build and post-build stages
    for stage in pre-build post-build; do
        src_stage_dir="$KAM_HOOKS_SRC/$stage"
        target_stage_dir="$tmpl_dir/hooks/$stage"

        if [ -d "$src_stage_dir" ]; then
            mkdir -p "$target_stage_dir"
            for srcfile in "$src_stage_dir"/*; do
                [ -f "$srcfile" ] || continue
                base=$(basename "$srcfile")
                # Prefer an existing lowercase counterpart on the template side:
                lowerbase=$(printf "%s" "$base" | tr '[:upper:]' '[:lower:]')
                if [ -f "$target_stage_dir/$lowerbase" ]; then
                    dest="$target_stage_dir/$lowerbase"
                else
                    dest="$target_stage_dir/$base"
                fi
                copy_if_exists "$srcfile" "$dest"
            done
        fi
    done

    # Set executable bit for shell scripts under hooks
    if [ -d "$tmpl_dir/hooks" ]; then
        find "$tmpl_dir/hooks" -type f -name "*.sh" -exec chmod +x {} \; >/dev/null 2>&1 || true
    fi

done

log_success "Hooks sync complete"
