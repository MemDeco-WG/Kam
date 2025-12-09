#!/bin/bash
exit 0
. "$KAM_HOOKS_ROOT/lib/utils.sh"


log_info "Building WebUI for module: $KAM_MODULE_ID"

WEBUI_DIR="$KAM_PROJECT_ROOT/ModuleWebUI"
BUILD_SCRIPT="$WEBUI_DIR/build.sh"

if [ ! -d "$WEBUI_DIR" ]; then
    log_error "ModuleWebUI directory not found at $WEBUI_DIR"
    exit 1
fi

if [ ! -x "$BUILD_SCRIPT" ]; then
    log_error "Build script not found or not executable at $BUILD_SCRIPT"
    # Try to make it executable
    chmod +x "$BUILD_SCRIPT" 2>/dev/null || true
    if [ ! -x "$BUILD_SCRIPT" ]; then
        exit 1
    fi
fi

# Execute build script
# We change directory to ModuleWebUI because the build script might rely on relative paths (e.g. node_modules)
(
    cd "$WEBUI_DIR" || exit 1
    ./build.sh "$KAM_MODULE_ID"
)

if [ $? -ne 0 ]; then
    log_error "WebUI build failed"
    exit 1
fi

# Move dist to webroot
# For template builds, install to src/Kam/webroot (the template directory)
# instead of src/kam_template/webroot (which would be created based on kam.toml id)
DIST_DIR="$WEBUI_DIR/dist"
TARGET_WEBROOT="$KAM_PROJECT_ROOT/src/Kam/webroot"

if [ ! -d "$DIST_DIR" ]; then
    log_error "Dist directory not found at $DIST_DIR after build"
    exit 1
fi

log_info "Installing WebUI to $TARGET_WEBROOT"

# Ensure parent directory exists (though KAM_WEB_ROOT usually implies it's inside module root)
mkdir -p "$(dirname "$TARGET_WEBROOT")"

# Remove existing webroot if it exists
if [ -d "$TARGET_WEBROOT" ]; then
    rm -rf "$TARGET_WEBROOT"
fi

# Move dist to webroot
mv "$DIST_DIR" "$TARGET_WEBROOT"

if [ $? -eq 0 ]; then
    log_success "WebUI built and installed successfully"
else
    log_error "Failed to move WebUI artifacts"
    exit 1
fi
