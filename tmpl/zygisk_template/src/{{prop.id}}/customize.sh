# Magisk provides Zygisk; KernelSU/APatch need a compatible Zygisk provider.
[ "${BOOTMODE:-false}" = true ] || abort '! Install from the running Android system'
[ "${API:-0}" -ge 23 ] || abort '! Android API 23+ is required'
case "${ARCH:-}" in
  arm64) abi=arm64-v8a ;;
  arm) abi=armeabi-v7a ;;
  x64) abi=x86_64 ;;
  x86) abi=x86 ;;
  *) abort '! Unsupported device architecture' ;;
esac
[ -f "$MODPATH/zygisk/$abi.so" ] || abort '! Missing Zygisk library for this device'
set_perm_recursive "$MODPATH/zygisk" 0 0 0755 0644
ui_print '- Enable Zygisk (or a compatible provider), then reboot.'
