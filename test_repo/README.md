
# Kam Module Repository Template

## Description

This is a template for creating Kam module repositories. A repository hosts a collection of Kam modules that can be installed via package managers like MMRL.

This template provides a complete repository structure with:
- Module index for efficient package discovery
- JSON configuration for repository metadata
- Asset management for images and resources
- CI/CD workflows for automated publishing
- Standard documentation and licensing

## Repository Structure

```
kam_repo/
├── .github/
│   └── workflows/          # CI/CD workflows
├── assets/                 # Images and resources
│   └── cover.webp          # Repository cover image
├── index/                  # Module index directory
│   ├── 1/                  # Subfolders for modules by first char
│   ├── a/
│   └── ...                 # More subfolders as needed
├── json/                   # JSON configurations
│   ├── config.json         # Repository configuration
│   ├── README.md           # JSON config documentation
│   └── modules.json        # Module metadata(mmrl backported)
├── kam.toml                # Kam configuration
├── CHANGELOG.md            # Repository changelog
├── LICENSE                 # Repository license
├── generate_index.sh       # Script to generate index from modules.json
└── README.md               # This file
```

## Index Generation

The repository uses a Cargo-like index structure for efficient module discovery. Modules are stored in NDJSON files under `index/<prefix>/<id>`, where `<prefix>` is the first two characters of the module ID (or duplicated for single-character IDs).

To populate the index from `json/modules.json`:

1. Ensure `jq` and `sha256sum` are installed.
2. Run `./generate_index.sh`

This script:
- Parses `json/modules.json`
- For each module and each version, creates an NDJSON entry with fields: `name`, `vers`, `require`, `cksum` (SHA256 of zipUrl), `yanked`
- Writes to `index/<prefix>/<id>`

The `json/modules.json` is kept for backward compatibility with existing clients.
