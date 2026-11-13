#!/bin/sh
set -eu
# Gradle 8.9 and JDK 17 are required by AGP 8.7.3.
command -v gradle >/dev/null || { echo 'Install Gradle 8.9 and JDK 17 (see README)' >&2; exit 1; }
gradle --no-daemon -p "$KAM_PROJECT_ROOT/android" :app:assembleDebug
mkdir -p "$KAM_MODULE_ROOT/apk"
cp "$KAM_PROJECT_ROOT/android/app/build/outputs/apk/debug/app-debug.apk" "$KAM_MODULE_ROOT/apk/module.apk"
