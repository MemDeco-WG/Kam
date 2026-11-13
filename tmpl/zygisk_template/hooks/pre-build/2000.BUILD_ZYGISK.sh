#!/bin/sh
set -eu
: "${ANDROID_NDK_HOME:?Set ANDROID_NDK_HOME to an Android NDK installation}"
"$ANDROID_NDK_HOME/ndk-build" -C "$KAM_PROJECT_ROOT/native" clean
"$ANDROID_NDK_HOME/ndk-build" -C "$KAM_PROJECT_ROOT/native"
mkdir -p "$KAM_MODULE_ROOT/zygisk"
for abi in arm64-v8a armeabi-v7a x86 x86_64; do
    cp "$KAM_PROJECT_ROOT/native/libs/$abi/libkam_zygisk.so" "$KAM_MODULE_ROOT/zygisk/$abi.so"
done
