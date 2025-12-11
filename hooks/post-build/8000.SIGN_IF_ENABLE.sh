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

# If not running under a TTY (non-interactive shell), skip signing unless explicitly overridden.
# Use KAM_SIGN_ALLOW_NONINTERACTIVE=1 to force signing even in non-interactive environments.
if [ ! -t 0 ] && [ "${KAM_SIGN_ALLOW_NONINTERACTIVE:-0}" != "1" ] && [ "${KAM_SIGN_ALLOW_CI:-0}" != "1" ]; then
	log_info "No TTY detected; non-interactive shell. Skipping signing (set KAM_SIGN_ALLOW_NONINTERACTIVE=1 or KAM_SIGN_ALLOW_CI=1 to override)"
	exit 0
fi

# Ensure kam is present - if it's not, skip signing gracefully.
if ! has_command kam; then
	log_warn "Command 'kam' not found; skipping signing"
	exit 0
fi

DIST=${KAM_DIST_DIR:-${KAM_PROJECT_ROOT:-$PWD}/dist}

if [ ! -d "$DIST" ]; then
	log_warn "Dist directory $DIST not found; nothing to sign"
	exit 0
fi

# Find artifacts using bash globbing; skip if none found.
shopt -s nullglob
# Match both hyphenated and non-hyphenated artifact names (e.g., Kam-1.0.zip or Kam1.0.zip)
files=( "$DIST"/"$KAM_MODULE_ID"*.zip )
if [ ${#files[@]} -eq 0 ]; then
	log_warn "No artifacts matching $DIST/$KAM_MODULE_ID*.zip; nothing to sign"
	exit 0
fi

log_info "Signing artifacts in $DIST using 'kam sign'"

# Build the command with optional overrides
cmd=( "kam" "sign" "${files[@]}" "--out" "$DIST" )

# Optional: override secret name used by kam (default is 'main')
if [ -n "${KAM_SIGN_SECRET:-}" ]; then
	cmd+=( "--secret" "${KAM_SIGN_SECRET}" )
fi

# Optional: use a key path rather than secret storage
if [ -n "${KAM_SIGN_KEY_PATH:-}" ]; then
	cmd+=( "--key-path" "${KAM_SIGN_KEY_PATH}" )
fi

# Sigstore usage: default ON; disable with KAM_SIGN_SIGSTORE=0
if [ "${KAM_SIGN_SIGSTORE:-1}" != "0" ]; then
	cmd+=( "--sigstore" )
fi

# Timestamping: default ON; disable with KAM_SIGN_TIMESTAMP=0
if [ "${KAM_SIGN_TIMESTAMP:-1}" != "0" ]; then
	cmd+=( "-t" )
fi

# Optional: request Fulcio certificate (keyless signing). If your CI provides SIGSTORE_ID_TOKEN,
# set KAM_SIGN_FULCIO=1 to enable; optionally set KAM_SIGN_OIDC_TOKEN_ENV to specify the token env var.
if [ "${KAM_SIGN_FULCIO:-0}" = "1" ]; then
	cmd+=( "--fulcio" )
	if [ -n "${KAM_SIGN_OIDC_TOKEN_ENV:-}" ]; then
		cmd+=( "--oidc-token-env" "${KAM_SIGN_OIDC_TOKEN_ENV}" )
	fi
fi

# Run the signing command - do not fail the build if signing fails; just warn and continue.
if ! "${cmd[@]}"; then
	log_warn "Signing command failed; continuing build (set KAM_SIGN_DEBUG=1 for more info)"
else
	log_success "Signing finished"
fi

exit 0
