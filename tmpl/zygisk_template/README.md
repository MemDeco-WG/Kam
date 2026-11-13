# {{prop.name}}

Zygisk native module, initialized with `kam init demo -t zygisk`.
Requires Android NDK and Android API 23+. Set `ANDROID_NDK_HOME`, then run
`kam check` and `kam build`. The pre-build hook compiles all four Android ABIs
and places libraries at `zygisk/<abi>.so` inside the module ZIP.

The bundled public API header is from the pinned upstream revision recorded in
`native/UPSTREAM.md`; retain its license. Edit `native/jni/module.cpp` to add
process selection and hooks. The example unloads without changing app behavior.
Do not unload after installing hooks that still reference module code.

Install the ZIP with your root manager. Enable Magisk Zygisk or install a
compatible Zygisk provider for KernelSU/APatch, then reboot. Packaging alone
does not prove injection works; verify on a device using the target process.

Reference: https://github.com/topjohnwu/zygisk-module-sample
