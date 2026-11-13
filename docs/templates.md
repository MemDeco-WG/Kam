# 模板规范入口

Kam 模板开发以两份规范为准：

- [Kam TOML 规范](kam-toml.md)：字段、类型、默认值、渲染变量和 hook 环境变量。
- [Kam 模板开发规范](template-development.md)：模板目录结构、raw-copy 规则、渲染边界、打包导入和验证流程。

内置模板源码位于 `tmpl/`，Cargo 安装包使用 `src/assets/tmpl/*.tar.gz` 中的归档。修改内置模板时，应重新生成归档并检查生成项目行为：

```bash
for template in ak3_template downloader_template kam_template lsposed_template meta_template tmpl_template zygisk_template; do
  cargo run -- build "tmpl/${template}" --quiet
  cp "templates/${template}.tar.gz" "src/assets/tmpl/${template}.tar.gz"
done

cargo run -- init /tmp/kam-template-smoke -t tmpl/kam_template --force
cargo run -- validate /tmp/kam-template-smoke
cargo run -- build /tmp/kam-template-smoke
```

For a ready-to-customize incremental module installer, use the bundled wrapper:

```bash
kam installer ./my-installer --repository owner/module --asset Module.zip \
  --module-id Module --project-name "Module Installer" --author "Your Name"
```

The generated downloader verifies GitHub's release digest, ZIP integrity, and
module ID. Verified archives are cached under `/data/adb/cache/{module_id}`;
override with `--cache-dir` when a manager-specific location is required.

## Zygisk and LSPosed templates

- `kam init demo -t zygisk`: native Zygisk API module. Requires Android NDK
  (`ANDROID_NDK_HOME`). The hook builds four ABIs into `zygisk/<abi>.so`.
  Keep the pinned upstream API header and its license; choose target processes
  before adding hooks. Magisk needs Zygisk enabled; KernelSU/APatch need a
  compatible provider. A successful ZIP build is not device injection proof.
- `kam init demo -t lsposed`: legacy Xposed API 82 hook APK, plus a companion
  Kam ZIP containing `apk/module.apk`. Requires JDK 17+, Gradle 8.9 and Android
  SDK 35. Install the APK separately and enable/select scope in LSPosed; the ZIP
  alone never activates the APK. The example only logs in Settings.
- Change application ID, namespace, Java package, `assets/xposed_init`, strings
  and scope together before publishing an APK. Keep API 82 `compileOnly`.
  Modern libxposed entrypoints and META-INF/xposed files are a different format.
- Check `kam check`, build hooks, ZIP paths and APK contents locally. Report
  missing SDK/NDK/Gradle dependencies and distinguish packaging from device
  acceptance. Never silently package stale native libraries or a stale APK.
