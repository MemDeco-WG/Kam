#!/bin/bash

. $KAM_HOOKS_ROOT/lib/utils.sh

if [ "$KAM_RELEASE_ENABLED" != "1" ]; then
    echo "Release is disabled, skipping upload"
    exit 0
fi

if ! require_command gh; then
    echo "gh command not found"
    exit 1
fi

# Generate detailed release notes
RELEASE_NOTES=$(cat <<EOF
# ${KAM_MODULE_NAME} v${KAM_MODULE_VERSION}

## Module Information
- **Version**: ${KAM_MODULE_VERSION}
- **Version Code**: ${KAM_MODULE_VERSION_CODE}
- **Module ID**: ${KAM_MODULE_ID}
- **Author**: ${KAM_MODULE_AUTHOR}

## Description
${KAM_MODULE_DESCRIPTION}

## Download
- [${KAM_MODULE_ID}.zip](https://github.com/\${GITHUB_REPOSITORY}/releases/download/${KAM_MODULE_VERSION}/${KAM_MODULE_ID}.zip)

## Installation
1. Download the module ZIP file
2. Install via Magisk/KernelSU/APatch Manager
3. Reboot your device

## Changelog
See [CHANGELOG.md](https://github.com/\${GITHUB_REPOSITORY}/blob/main/CHANGELOG.md) for detailed changes.

---
Built with [Kam](https://github.com/MemDeco-WG/Kam)
EOF
)

gh release create "$KAM_MODULE_VERSION" \
    --title "${KAM_MODULE_NAME} v${KAM_MODULE_VERSION}" \
    --notes "$RELEASE_NOTES" \
    "$KAM_DIST_DIR/*"

echo "Upload complete"
