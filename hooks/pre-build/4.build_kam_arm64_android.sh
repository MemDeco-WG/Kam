#!/bin/sh

. "$KAM_HOOKS_ROOT/lib/utils.sh"



require_command cross "Command 'cross' is required but not found. cargo install cross"

cross build --target aarch64-linux-android --release || exit 1

# move build artifacts .

mkdir -p $KAM_MODULE_ROOT/system/bin

cp target/aarch64-linux-android/release/kam $KAM_MODULE_ROOT/system/bin/kam
