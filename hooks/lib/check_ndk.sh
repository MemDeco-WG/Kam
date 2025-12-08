#!/bin/bash

check_ndk_installed() {
    if [ -z "$ANDROID_NDK_HOME" ]; then
        echo "ANDROID_NDK_HOME is not set"
        return 1
    fi

    if [ ! -d "$ANDROID_NDK_HOME" ]; then
        echo "ANDROID_NDK_HOME is not a directory"
        return 1
    fi

    if [ ! -f "$ANDROID_NDK_HOME/ndk-build" ]; then
        echo "ndk-build not found in ANDROID_NDK_HOME"
        return 1
    fi

    return 0
}

# check if the NDK is installed
{check_ndk_installed && exit 0;}|| echo "NDK is not installed!"

exit 1
