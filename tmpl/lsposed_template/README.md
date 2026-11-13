# {{prop.name}}

LSPosed legacy Xposed API 82 hook APK plus an optional Kam companion ZIP.
This is a hook module, not the LSPosed framework or a native Zygisk library.

Requirements: JDK 17+, Gradle 8.9, Android SDK platform 35 and build tools 35.0.0.
Set `ANDROID_HOME` or `android/local.properties`. Initialize with
`kam init demo -t lsposed`, then run `kam check` and `kam build`.
The pre-build hook runs Gradle and packages the debug APK at `apk/module.apk`
inside the ZIP. For APK-only development run
`gradle -p android :app:assembleDebug` directly.

Install `android/app/build/outputs/apk/debug/app-debug.apk` with `adb install -r`,
then enable the module in LSPosed, select Settings as scope and restart Settings.
The example only logs when Settings loads; verify the LSPosed log on a device.
Installing the companion ZIP alone does not activate the hook APK.

Before publishing, change the application ID, namespace, Java package,
`assets/xposed_init`, strings and scope together. Use your own release signing
configuration and `:app:assembleRelease`; never publish the debug APK as a
signed production release. API 82 is compileOnly and must not be bundled.

This template deliberately uses the legacy API for compatibility. Modern
libxposed modules require a different entry class and META-INF/xposed metadata;
do not mix the two formats.

References:
- https://github.com/rovo89/XposedBridge/wiki/Development-tutorial
- https://github.com/LSPosed/LSPosed/wiki/Develop-Xposed-Modules-Using-Modern-Xposed-API
- https://developer.android.com/build/releases/past-releases/agp-8-7-0-release-notes
