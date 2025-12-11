#!/bin/bash

. "$KAM_HOOKS_ROOT/lib/utils.sh"

# Sign artifacts in $KAM_DIST_DIR if KAM_SIGN_ENABLED=1
if [ "$KAM_SIGN_ENABLED" != "1" ]; then
	log_info "KAM_SIGN_ENABLED != 1, skipping signing"
	exit 0
fi

# By default skip signing in CI to avoid interactive prompts. Set KAM_SIGN_ALLOW_CI=1 to force.
if [ "${CI:-}" = "true" ] && [ "${KAM_SIGN_ALLOW_CI:-0}" != "1" ]; then
    log_info "Running in CI environment, skipping signing (set KAM_SIGN_ALLOW_CI=1 to override)"
    exit 0
fi

# Ensure kam is present - if it's not, skip signing gracefully.
if ! has_command kam; then
    log_warn "Command 'kam' not found; skipping signing"
    exit 0
fi

# If there is no interactive TTY, avoid prompting for passwords; skip by default unless forced
# Use KAM_SIGN_ALLOW_NONINTERACTIVE=1 to force signing in non-interactive environments,
# or KAM_SIGN_ALLOW_CI=1 to explicitly allow signing in CI contexts.
if [ "${KAM_SIGN_ALLOW_NONINTERACTIVE:-}" != "1" ] && [ "${KAM_SIGN_ALLOW_CI:-0}" != "1" ] && [ ! -t 0 ]; then
    log_info "No interactive TTY detected; skipping signing (set KAM_SIGN_ALLOW_NONINTERACTIVE=1 or KAM_SIGN_ALLOW_CI=1 to override)"
    exit 0
fi

DIST=${KAM_DIST_DIR:-${KAM_PROJECT_ROOT:-$PWD}/dist}

if [ ! -d "$DIST" ]; then
    log_warn "Dist directory $DIST not found; nothing to sign"
    exit 0
fi

log_info "Signing artifacts in $DIST using 'kam sign' (and cosign OIDC on GitHub Actions if available)"

# Use bash's nullglob to avoid literal unmatched globbing; create artifacts array for later usage
shopt -s nullglob
artifacts=( "$DIST"/"$KAM_MODULE_ID"*.zip )
if [ "${#artifacts[@]}" -eq 0 ]; then
    log_info "No artifacts found in $DIST matching $KAM_MODULE_ID*.zip; skipping signing"
    # Restore shell glob behavior and exit successfully.
    shopt -u nullglob
    exit 0
fi

# If running in GitHub Actions (or GH token present) and cosign is available, attempt keyless OIDC signing.
# This allows cosign to obtain an ephemeral cert (via Fulcio) proving the build environment.
if { [ -n "${GITHUB_ACTIONS:-}" ] || [ -n "${GH_TOKEN:-}" ] || [ -n "${GITHUB_TOKEN:-}" ]; } && has_command cosign; then
    log_info "Detected GitHub Actions + cosign; performing keyless OIDC signing (cosign) for artifacts in $DIST"
    for artifact in "${artifacts[@]}"; do
        if [ -f "$artifact" ]; then
            log_info "Signing with cosign (keyless/OIDC): ${artifact}"
            # Try to sign with cosign using keyless OIDC flow. If we are in GH Actions with proper permissions,
            # cosign will fetch a GitHub Actions OIDC token and request a certificate from Fulcio.
            if ! cosign sign --keyless "$artifact"; then
                log_warn "cosign keyless sign failed for ${artifact}"
            else
                log_info "cosign keyless sign succeeded for ${artifact}"
            fi
        fi
    done
else
    if [ -n "${GITHUB_ACTIONS:-}" ] || [ -n "${GH_TOKEN:-}" ] || [ -n "${GITHUB_TOKEN:-}" ]; then
        log_warn "GitHub Actions detected but 'cosign' not installed; skipping keyless OIDC sign"
    fi
fi

# Always perform 'kam' signing as the project's signing step for DSSE/sigstore bundles and timestamping.
# This ensures repository-specific signature bundles and timestamps are generated as before.

# Guard: only run kam sign if there are matching artifacts in DIST to avoid errors with an unmatched glob.
# Use bash's nullglob so the glob expands to zero entries (instead of a literal pattern) when none exist.
shopt -s nullglob
artifacts=( "$DIST"/"$KAM_MODULE_ID"*.zip )
if [ "${#artifacts[@]}" -eq 0 ]; then
    log_info "No artifacts found in $DIST matching $KAM_MODULE_ID*.zip; skipping signing"
    # Restore shell glob behavior and exit successfully.
    shopt -u nullglob
    exit 0
fi

# Build sign options from environment; defaults keep original behavior.
KAM_SIGN_SECRET=${KAM_SIGN_SECRET:-}
KAM_SIGN_KEY_PATH=${KAM_SIGN_KEY_PATH:-}
KAM_SIGN_SIGSTORE=${KAM_SIGN_SIGSTORE:-1}
KAM_SIGN_TIMESTAMP=${KAM_SIGN_TIMESTAMP:-1}
KAM_SIGN_FULCIO=${KAM_SIGN_FULCIO:-}
KAM_SIGN_OIDC_TOKEN_ENV=${KAM_SIGN_OIDC_TOKEN_ENV:-SIGSTORE_ID_TOKEN}

for artifact in "${artifacts[@]}"; do
    if [ -f "$artifact" ]; then
        cmd=( "kam" "sign" "$artifact" "--out" "$DIST" )
        if [ -n "$KAM_SIGN_SECRET" ]; then
            cmd+=( "--secret" "$KAM_SIGN_SECRET" )
        fi
        if [ -n "$KAM_SIGN_KEY_PATH" ]; then
            cmd+=( "--key-path" "$KAM_SIGN_KEY_PATH" )
        fi
        if [ "$KAM_SIGN_SIGSTORE" != "0" ]; then
            cmd+=( "--sigstore" )
        fi
        if [ "$KAM_SIGN_TIMESTAMP" != "0" ]; then
            cmd+=( "-t" )
        fi
        # Enable Fulcio (keyless) if requested explicitly or if SIGSTORE_ID_TOKEN is present.
        if [ "$KAM_SIGN_FULCIO" = "1" ] || [ -n "${SIGSTORE_ID_TOKEN:-}" ]; then
            cmd+=( "--fulcio" )
            cmd+=( "--oidc-token-env" "${KAM_SIGN_OIDC_TOKEN_ENV}" )
        fi
        # Run signing; do not fail the entire build if signing fails.
        if ! "${cmd[@]}"; then
            log_warn "kam sign failed for ${artifact}; continuing build"
        else
            log_success "Signed ${artifact}"
        fi
    fi
done
