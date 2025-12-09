#!/bin/bash
exit 0
. "$KAM_HOOKS_ROOT/lib/utils.sh"
. "$KAM_HOOKS_ROOT/lib/detect_installer.sh"

require_command gh "please install gh (github-cli)."

require_command cargo "please install cargo."

has_command cross || cargo install cross

# Attempt to build with cross. If `libunwind` is missing, provide fallbacks:
# 1) Retry with `RUSTFLAGS="-C panic=abort"` to avoid requiring libunwind
# 2) If NDK is available, try building with cargo-ndk
build_target="aarch64-linux-android"
log_info "Building for ${build_target} (using cross by default)..."

build_output="$(cross build --target ${build_target} --release 2>&1)"
build_status=$?

if [ "$build_status" -eq 0 ]; then
    log_success "cross build completed successfully"
else
    # Detect libunwind linker error
    printf '%s\n' "$build_output" | grep -q "cannot find -lunwind"
    if [ $? -eq 0 ]; then
        log_warn "Linker error: libunwind not found. Trying fallback: RUSTFLAGS='-C panic=abort' (avoids libunwind)."
        if env RUSTFLAGS="-C panic=abort" cross build --target ${build_target} --release; then
            log_success "Build succeeded with panic=abort (no libunwind required)"
        else
            log_warn "Fallback with panic=abort failed. Attempting cargo-ndk fallback if available..."
            if ! has_command cargo-ndk; then
                log_info "Installing cargo-ndk via cargo (this may take a while)..."
                cargo install cargo-ndk
            fi

            # Try to auto-detect NDK path if not explicitly set
            if [ -z "${ANDROID_NDK_HOME:-}" ]; then
                if [ -n "${ANDROID_SDK_ROOT:-}" ] && [ -d "${ANDROID_SDK_ROOT}/ndk" ]; then
                    ANDROID_NDK_HOME=$(ls -d "${ANDROID_SDK_ROOT}/ndk/"* 2>/dev/null | tail -n 1)
                elif [ -n "${ANDROID_HOME:-}" ] && [ -d "${ANDROID_HOME}/ndk" ]; then
                    ANDROID_NDK_HOME=$(ls -d "${ANDROID_HOME}/ndk/"* 2>/dev/null | tail -n 1)
                fi
            fi

            if [ -n "${ANDROID_NDK_HOME:-}" ]; then
                log_info "Detected Android NDK at ${ANDROID_NDK_HOME}. Building via cargo-ndk..."
                ANDROID_NDK_HOME="${ANDROID_NDK_HOME}" cargo ndk -t arm64 build --release || exit 1
            else
                log_error "Android NDK not found. Set ANDROID_NDK_HOME or ANDROID_SDK_ROOT/ANDROID_HOME with an NDK installed,"
                log_error "or use panic=abort to avoid libunwind, or provide libunwind in your NDK/toolchain."
                printf '%s\n' "$build_output"
                exit 1
            fi
        fi
    else
        log_error "cross build failed for an unknown reason. Full output:"
        printf '%s\n' "$build_output"
        exit 1
    fi
fi

# move build artifacts .

mkdir -p $KAM_MODULE_ROOT/system/bin

cp target/aarch64-linux-android/release/kam $KAM_MODULE_ROOT/system/bin/kam
