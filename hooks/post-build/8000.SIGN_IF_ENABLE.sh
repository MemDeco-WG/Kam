#!/bin/bash

. "$KAM_HOOKS_ROOT/lib/utils.sh"

# Sign artifacts in $KAM_DIST_DIR if KAM_SIGN_ENABLED=1
if [ "$KAM_SIGN_ENABLED" != "1" ]; then
	log_info "KAM_SIGN_ENABLED != 1, skipping signing"
	exit 0
fi


log_info "Signing artifacts in $KAM_DIST_DIR (kam sign -s)..."

# Attempt to sign, but allow failure as requested ("失败也没关系")
if kam sign --dist "$KAM_DIST_DIR"; then
    log_success "Signing completed successfully."

    # Add verification instructions to release notes
    if [ -f "$KAM_RELEASE_NOTE" ]; then
        log_info "Adding verification instructions to release notes..."
        {
            echo ""
            echo "## 🔐 Signature Verification"
            echo ""
            echo "All artifacts have been signed. You can verify them using either:"
            echo ""
            echo "### Option 1: Using Developer Certificate (Recommended)"
            echo ""
            echo '```bash'
            echo "# One-time setup: Trust the Root CA"
            echo "kam secret trust --add-root https://raw.githubusercontent.com/kernelsu/developers/main/keyring/root-ca.pem --ca-name kernelsu-root"
            echo ""
            echo "# Import developer certificate from GitHub issue"
            echo "kam secret import-cert --repo kernelsu/developers --issue <ISSUE_NUMBER> --name <DEVELOPER_NAME>"
            echo ""
            echo "# Verify artifact"
            echo "kam verify ${KAM_MODULE_ID}.zip --cert-name <DEVELOPER_NAME>"
            echo '```'
            echo ""
            echo "### Option 2: Using Public Key"
            echo ""
            echo '```bash'
            echo "kam verify ${KAM_MODULE_ID}.zip --key /path/to/public-key.pem"
            echo '```'
            echo ""
            echo "> **Note**: The .sig file must be in the same directory as the artifact."
            echo "> If not, specify with: \`--sig path/to/file.sig\`"
        } >> "$KAM_RELEASE_NOTE"
        log_success "Verification instructions added to release notes."
    fi
else
    log_warn "Signing failed. Continuing build process as failure is allowed."
    # We exit 0 to ensure the build pipeline doesn't stop
    exit 0
fi
