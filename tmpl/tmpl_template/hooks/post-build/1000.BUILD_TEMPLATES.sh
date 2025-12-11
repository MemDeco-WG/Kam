#!/bin/bash
# 2.BUILD_TEMPLATES.sh - compress all files in templates directory into dist/templates.zip

. "$KAM_HOOKS_ROOT/lib/utils.sh"

# Optional: disable packaging using environment variable KAM_BUILD_TEMPLATES_DISABLED=1
if [ "${KAM_BUILD_TEMPLATES_DISABLED:-0}" = "1" ]; then
  log_info "Skipping templates packaging due to KAM_BUILD_TEMPLATES_DISABLED=1"
  exit 0
fi

# Where to output the ZIP (KAM_DIST_DIR preferred, default to KAM_PROJECT_ROOT/dist)
DIST="${KAM_DIST_DIR:-$KAM_PROJECT_ROOT/dist}"
TEMPLATES_DIR="$KAM_PROJECT_ROOT/templates"
TMPL_DIR="$KAM_PROJECT_ROOT/tmpl"

# Nothing to do if neither tmpl nor templates dir exist
if [ ! -d "$TMPL_DIR" ] && [ ! -d "$TEMPLATES_DIR" ]; then
  log_info "No 'tmpl' or 'templates' directory found; skipping templates packaging."
  exit 0
fi

# Ensure `zip` is available
if ! has_command "zip"; then
  log_warn "zip command not available; skipping templates packaging."
  exit 0
fi

# Check whether tar is available; if not, we will still include prebuilt .tar.gz files but skip packing tmpl/ directories
TAR_OK=0
if has_command "tar"; then
  TAR_OK=1
else
  log_warn "tar command not available; skipping tarring of tmpl/ directories (prebuilt archives will still be included)."
fi

# Ensure output directory exists
mkdir -p "$DIST"

# Remove existing zip to avoid appending to it
rm -f "$DIST/templates.zip"

# Create a temporary directory to collect artifacts
TMPDIR="$(mktemp -d)" || { log_error "Failed to create temporary directory"; exit 1; }

# Prepare a flag to track whether we found any artifacts to zip
FOUND_ARTIFACTS=0

# 1) Handle `tmpl/` development templates: create <name>.tar.gz per directory
if [ -d "$TMPL_DIR" ]; then
  for item in "$TMPL_DIR"/*; do
    [ -e "$item" ] || continue
    name=$(basename "$item")
    # If this is a directory and tar is available, create a tar.gz
    if [ -d "$item" ]; then
      if [ "$TAR_OK" -eq 1 ]; then
        tar -C "$item" -czf "$TMPDIR/${name}.tar.gz" . > /dev/null 2>&1
        if [ $? -ne 0 ]; then
          log_warn "Failed to pack tmpl/$name (skipping)"
          continue
        fi
        FOUND_ARTIFACTS=1
        log_info "Packaged tmpl/$name -> $TMPDIR/${name}.tar.gz"
      else
        log_warn "tar not available; skipping tmpl/$name"
      fi
    else
      # If it's a file and appears to be an archive, copy it
      case "$item" in
        *.tar.gz|*.tgz|*.tar)
          cp -f "$item" "$TMPDIR/" || log_warn "Failed to copy $item to temporary dir"
          FOUND_ARTIFACTS=1
          ;;
        *)
          log_warn "Skipping $item (unsupported file in tmpl/ for packaging)"
          ;;
      esac
    fi
  done
fi

# 2) Copy prebuilt archives from templates/ directory
if [ -d "$TEMPLATES_DIR" ]; then
  for file in "$TEMPLATES_DIR"/*; do
    [ -e "$file" ] || continue
    case "$file" in
      *.tar.gz|*.tgz|*.tar)
        cp -f "$file" "$TMPDIR/" || log_warn "Failed to copy $file to temporary dir"
        FOUND_ARTIFACTS=1
        ;;
      *)
        log_info "Skipping non-archive file in templates/: $file"
        ;;
    esac
  done
fi

# If we have no artifacts, cleanup and exit
if [ "$FOUND_ARTIFACTS" -eq 0 ]; then
  log_info "No template artifacts found to package under tmpl/ or templates/; skipping packaging."
  rm -rf "$TMPDIR"
  exit 0
fi

# Build the final ZIP with the collected tar.gz artifacts
# Use -j to avoid including directory structure
zip -j "$DIST/templates.zip" "$TMPDIR"/* > /dev/null 2>&1
if [ $? -ne 0 ]; then
  log_error "Failed to create $DIST/templates.zip"
  rm -rf "$TMPDIR"
  exit 1
fi

log_success "Templates packaged at $DIST/templates.zip"

# Cleanup
rm -rf "$TMPDIR"

kam tmpl import "$DIST/templates.zip" -f

exit 0
