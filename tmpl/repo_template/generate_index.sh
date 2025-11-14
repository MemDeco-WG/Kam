#!/bin/bash

# Generate index from modules.json
# This script creates a Cargo-like index structure for Kam modules

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INDEX_DIR="$SCRIPT_DIR/index"
MODULES_JSON="$SCRIPT_DIR/json/modules.json"

if [ ! -f "$MODULES_JSON" ]; then
    echo "Error: $MODULES_JSON not found"
    exit 1
fi

# Ensure index directory exists
mkdir -p "$INDEX_DIR"

# Function to get prefix for module ID
get_prefix() {
    local id="$1"
    if [ ${#id} -eq 1 ]; then
        echo "${id}${id}"
    else
        echo "${id:0:2}"
    fi
}

# Function to compute SHA256 checksum
compute_checksum() {
    local url="$1"
    echo -n "$url" | sha256sum | cut -d' ' -f1
}

# Read modules.json and generate index entries
jq -r '.modules[] | @base64' "$MODULES_JSON" | while read -r module_encoded; do
    module=$(echo "$module_encoded" | base64 --decode)

    id=$(echo "$module" | jq -r '.id')
    name=$(echo "$module" | jq -r '.name')
    author=$(echo "$module" | jq -r '.author')
    description=$(echo "$module" | jq -r '.description')

    prefix=$(get_prefix "$id")
    index_file="$INDEX_DIR/$prefix/$id"

    mkdir -p "$(dirname "$index_file")"

    # Clear existing index file
    > "$index_file"

    # Process each version
    echo "$module" | jq -r '.versions[] | @base64' | while read -r version_encoded; do
        version_data=$(echo "$version_encoded" | base64 --decode)

        vers=$(echo "$version_data" | jq -r '.version')
        versionCode=$(echo "$version_data" | jq -r '.versionCode')
        zipUrl=$(echo "$version_data" | jq -r '.zipUrl')
        changelog=$(echo "$version_data" | jq -r '.changelog // ""')
        size=$(echo "$version_data" | jq -r '.size // 0')
        timestamp=$(echo "$version_data" | jq -r '.timestamp // 0')

        cksum=$(compute_checksum "$zipUrl")

        # Create index entry
        entry=$(jq -n \
            --arg name "$id" \
            --arg vers "$vers" \
            --argjson versionCode "$versionCode" \
            --arg zipUrl "$zipUrl" \
            --arg changelog "$changelog" \
            --argjson size "$size" \
            --argjson timestamp "$timestamp" \
            --arg author "$author" \
            --arg description "$description" \
            --arg cksum "$cksum" \
            '{
                name: $name,
                vers: $vers,
                versionCode: $versionCode,
                zipUrl: $zipUrl,
                changelog: ($changelog | if . == "" then null else . end),
                size: $size,
                timestamp: $timestamp,
                author: $author,
                description: $description,
                cksum: $cksum,
                yanked: false
            }')

        echo "$entry" >> "$index_file"
    done
done

echo "Index generated successfully in $INDEX_DIR"
