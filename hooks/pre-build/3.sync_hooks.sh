#!/bin/bash
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

# Helper to copy a file if it exists and only overwrite if destination exists
copy_if_exists() {
    src="$1"
    dst="$2"

    # Only proceed if the source file exists
    if [ ! -f "$src" ]; then
        return 0
    fi

    # Only overwrite when the destination file exists — do not create new files in templates
    if [ -f "$dst" ]; then
        if cp -f "$src" "$dst"; then
            log_info "Copied: $(basename \"$src\") -> $dst"

            # Ensure shell scripts stay executable
            case "$(basename \"$dst\")" in
                *.sh)
                    chmod +x "$dst" 2>/dev/null || true
                    ;;
            esac
        else
            log_warn "Failed to copy: $src -> $dst"
        fi
    else
        log_info "Skipping copy; destination does not exist: $dst"
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

    # Copy lib utilities (these files are shared across templates; always copy/sync them)
    if [ -d "$KAM_HOOKS_SRC/lib" ]; then
        for libitem in "$KAM_HOOKS_SRC"/lib/*; do
            [ -e "$libitem" ] || continue
            dest="$tmpl_dir/hooks/lib/$(basename "$libitem")"
            # Recursively copy files or directories; overwrite existing ones in templates
            if cp -a "$libitem" "$dest"; then
                log_info "Synced shared lib: $(basename "$libitem") -> $dest"
            else
                log_warn "Failed to copy: $libitem -> $dest"
            fi
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
                lowerbase=$(printf "%s" "$base" | tr '[:upper:]' '[:lower:]')

                # Only overwrite if a same-name (exact or lowercase) file already exists in the template.
                if [ -f "$target_stage_dir/$base" ]; then
                    dest="$target_stage_dir/$base"
                    copy_if_exists "$srcfile" "$dest"
                elif [ -f "$target_stage_dir/$lowerbase" ]; then
                    dest="$target_stage_dir/$lowerbase"
                    copy_if_exists "$srcfile" "$dest"
                else
                    log_info "Skipping $base for $tmpl_name/$stage - target file does not exist"
                fi
            done
        fi
    done

    # Set executable bit for shell scripts under hooks
    if [ -d "$tmpl_dir/hooks" ]; then
        find "$tmpl_dir/hooks" -type f -name "*.sh" -exec chmod +x {} \; >/dev/null 2>&1 || true
    fi

done

# Also sync into the project's hooks directory (KAM_PROJECT_ROOT/hooks)
KAM_PROJECT_HOOKS="${KAM_PROJECT_HOOKS:-$KAM_PROJECT_ROOT/hooks}"

# Skip if the project hooks root is the same as the KamHooks source root
if [ -n "$KAM_PROJECT_HOOKS" ] && [ "$KAM_PROJECT_HOOKS" != "$KAM_HOOKS_SRC" ]; then
    log_info "Syncing KamHooks into project hooks at: $KAM_PROJECT_HOOKS"

    # Ensure project hooks directories exist
    mkdir -p "$KAM_PROJECT_HOOKS" "$KAM_PROJECT_HOOKS/pre-build" "$KAM_PROJECT_HOOKS/post-build" "$KAM_PROJECT_HOOKS/lib" >/dev/null 2>&1 || true

    # Copy root-level files (always copy to project hooks)
    if [ -f "$KAM_HOOKS_SRC/LICENSE" ]; then
        if cp -f "$KAM_HOOKS_SRC/LICENSE" "$KAM_PROJECT_HOOKS/LICENSE"; then
            log_info "Copied: LICENSE -> $KAM_PROJECT_HOOKS/LICENSE"
        else
            log_warn "Failed to copy: LICENSE -> $KAM_PROJECT_HOOKS/LICENSE"
        fi
    fi

    if [ -f "$KAM_HOOKS_SRC/README.md" ]; then
        if cp -f "$KAM_HOOKS_SRC/README.md" "$KAM_PROJECT_HOOKS/README.md"; then
            log_info "Copied: README.md -> $KAM_PROJECT_HOOKS/README.md"
        else
            log_warn "Failed to copy: README.md -> $KAM_PROJECT_HOOKS/README.md"
        fi
    fi

    # Copy lib utilities (create or overwrite in project)
    if [ -d "$KAM_HOOKS_SRC/lib" ]; then
        for libitem in "$KAM_HOOKS_SRC"/lib/*; do
            [ -e "$libitem" ] || continue
            dest="$KAM_PROJECT_HOOKS/lib/$(basename "$libitem")"
            if cp -a "$libitem" "$dest"; then
                log_info "Synced project lib: $(basename \"$libitem\") -> $dest"
                chmod +x "$dest" 2>/dev/null || true
            else
                log_warn "Failed to copy: $libitem -> $dest"
            fi
        done
    fi

    # Copy/pre-create pre-build and post-build scripts (overwrite/create)
    for stage in pre-build post-build; do
        src_stage_dir="$KAM_HOOKS_SRC/$stage"
        proj_stage_dir="$KAM_PROJECT_HOOKS/$stage"

        if [ -d "$src_stage_dir" ]; then
            mkdir -p "$proj_stage_dir" >/dev/null 2>&1 || true
            for srcfile in "$src_stage_dir"/*; do
                [ -f "$srcfile" ] || continue
                dest="$proj_stage_dir/$(basename "$srcfile")"
                if cp -a "$srcfile" "$dest"; then
                    log_info "Synced project hook: $(basename \"$srcfile\") -> $dest"
                    case "$(basename "$dest")" in
                        *.sh)
                            chmod +x "$dest" 2>/dev/null || true
                            ;;
                    esac
                else
                    log_warn "Failed to copy: $srcfile -> $dest"
                fi
            done
        fi
    done
fi

log_success "Hooks sync complete"
