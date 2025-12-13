//! Lightweight i18n / language detection module
//!
//! This module:
//! - Detects the user's preferred language (using a third-party crate when available)
//! - Allows overriding the language via `kam config` (key: `language` or `ui.language`)
//! - Provides a small lookup-based translation system (EN <-> ZH) for common UI phrases
//! - Exposes convenience helpers/macros for formatting localized strings
//!
//! Notes:
//! - The translation system is intentionally minimal: it maps a set of well-known
//!   phrases used across the CLI. If a phrase is not known, the original string
//!   is returned unchanged. This allows gradual internationalization without
//!   touching every call site immediately.
//! - Detection tries in order:
//!   1. `kam config` language configuration (local -> global)
//!   2. system locale via the `sys-locale` crate
//!   3. fallback to `LANG` environment variable
//!   4. default to English
//!
//! Usage examples:
//!
//! // initialize early in main()
//! kam::i18n::init();
//!
//! // simple translation of a literal
//! let msg = kam::i18n::tr_key("Thanks for using Kam");
//!
//! // format a translated template
//! use kam::i18n::trf;
//! let formatted = trf!("Building module: {} v{}", &module_id, &version);
//! ```
use std::fmt::Display;
use std::sync::RwLock;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Language {
    En,
    Zh,
}

impl Default for Language {
    fn default() -> Self {
        Language::En
    }
}

// RwLock should be OK for a static global language setting.
// We default to English and allow changing it at runtime via `set_language`.
static CURRENT_LANGUAGE: RwLock<Language> = RwLock::new(Language::En);

/// Initialize i18n subsystem.
///
/// This should be called once at program start (e.g. in `main()`).
/// It will attempt to discover an overridden language via `kam config`,
/// otherwise it will fall back to system locale detection (via `sys-locale`)
/// and finally environment variable `LANG`.
pub fn init() {
    // 0. Environment variables take highest precedence. This allows quick one-off
    // overrides without changing config files.
    // Supported keys:
    // - KAM_UI_LANGUAGE : explicit UI language (e.g. "en", "zh", "zh-CN")
    // - KAM_LANG        : a secondary option for legacy or convenience
    if let Ok(val) = std::env::var("KAM_UI_LANGUAGE") {
        if !val.trim().is_empty() {
            let _ = set_language_str(&val);
            return;
        }
    }
    if let Ok(val) = std::env::var("KAM_LANG") {
        if !val.trim().is_empty() {
            let _ = set_language_str(&val);
            return;
        }
    }

    // 1. Try to read config (local or global). Prefer a 'language' or 'ui.language' key.
    // We call into config::read_language_from_config which is expected to be
    // a tiny public helper in `src/cmds/config.rs`.
    if let Some(lang_cfg) = get_language_from_config() {
        let _ = set_language_str(&lang_cfg);
        return;
    }

    // 2. Try to use sys-locale (third-party crate). If not available or None, fallback.
    #[allow(unused_mut)]
    {
        if let Some(detected) = detect_language_system() {
            let _ = set_language_str(&detected);
            return;
        }
    }

    // 3. Fallback to checking LANG env var.
    if let Ok(lang_env) = std::env::var("LANG") {
        let _ = set_language_str(&lang_env);
        return;
    }

    // 4. Otherwise default (already set to English)
}

/// Returns the current language.
pub fn current_language() -> Language {
    let lock = CURRENT_LANGUAGE.read().unwrap();
    *lock
}

/// Set language by enum
pub fn set_language(lang: Language) {
    let mut w = CURRENT_LANGUAGE.write().unwrap();
    *w = lang;
}

/// Set language by a string (e.g. "zh", "zh-CN", "en_US", "en")
/// Returns Err if the string is not recognized.
pub fn set_language_str(s: &str) -> Result<(), String> {
    let s_lc = s.trim().to_lowercase();
    let lang = if s_lc.starts_with("zh") {
        Language::Zh
    } else if s_lc.starts_with("en") {
        Language::En
    } else {
        return Err(format!("Unsupported language string: {}", s));
    };
    set_language(lang);
    Ok(())
}

/// Attempt to detect the language using the system locale. This uses the
/// third-party crate `sys-locale` if available (cfg gate), otherwise returns None.
fn detect_language_system() -> Option<String> {
    // Try the sys-locale crate first (if available).
    // If `sys-locale` wasn't added to Cargo.toml this will not compile; the user
    // may need to add it. We gracefully handle the absence of the crate by
    // using the LANG environment fallback in `init()`.
    #[cfg(feature = "sys-locale")]
    {
        if let Some(locale) = sys_locale::get_locale() {
            return Some(locale);
        }
    }

    // If `sys-locale` isn't available, try environment variable.
    if let Ok(lang) = std::env::var("LANG") {
        return Some(lang);
    }

    None
}

// --- Small translation system -------------------------------------------------

/// Translate a string key/template according to current language and return the
/// localized template string (with placeholders left intact, e.g. "{}").
///
/// If no translation mapping exists for the given string, it returns the
/// original string.
pub fn tr_key<'a>(key: &'a str) -> &'a str {
    match current_language() {
        Language::En => {
            // if input is in Chinese and we have mapping -> return English
            if let Some(en) = zh_to_en(key) {
                en
            } else {
                key
            }
        }
        Language::Zh => {
            // if input is in English and we have mapping -> return Chinese
            if let Some(zh) = en_to_zh(key) {
                zh
            } else {
                key
            }
        }
    }
}

/// Convenience formatting helper (callable in code). It takes a translation
/// `template_key` and format args (like `format!`) and returns the formatted
/// string in the active language.
///
/// Example:
///   let s = tr_fmt("Building module: {} v{}", &module_id, &version);
pub fn tr_fmt(template_key: &str, args: &[&dyn Display]) -> String {
    let tmpl = tr_key(template_key);
    // Very simple formatting: we just pass into `format!` by building a string.
    // Since we can't expand varargs at runtime, we use an intermediate approach:
    // - We replace `{}` in the template sequentially with `Display::to_string`
    //   of args. This is intentionally simple (matches how `{}` placeholders are
    //   used in the repository).
    let mut out = String::new();
    let mut remaining = tmpl;
    for arg in args {
        if let Some(pos) = remaining.find("{}") {
            out.push_str(&remaining[..pos]);
            out.push_str(&format!("{}", arg));
            remaining = &remaining[pos + 2..];
        } else {
            // no more placeholders, append the rest and break
            out.push_str(remaining);
            remaining = "";
            break;
        }
    }
    out.push_str(remaining);
    out
}

/// Helper for one-off translation. Fancy macros are provided (trf) but some
/// code paths may prefer calling this directly.
pub fn tr_fmt_single<T: Display>(template_key: &str, arg: T) -> String {
    tr_fmt(template_key, &[&arg])
}

// A tiny translation map implemented as match arms.
// We implement EN -> ZH and ZH -> EN maps as functions returning Option<&'static str>.
// Expand the mapping for the most common phrases used by the CLI.
fn en_to_zh(s: &str) -> Option<&'static str> {
    match s {
        "This command is informational only; it doesn't modify files or the registry." =>
            Some("此命令仅供参考，不会修改文件或注册表。"),
        "Use other commands (e.g., `kam init`, `kam build`) to perform actions." =>
            Some("请使用其他命令（例如 `kam init`, `kam build`）执行实际操作。"),
        "Thanks for using Kam" => Some("感谢使用 Kam"),
        "Enjoy your module tooling experience!" => Some("祝您使用愉快！"),
        "Powered by the Kam CLI — Happy building!" => Some("由 Kam CLI 提供支持 — 祝构建顺利！"),
        "workspace member {} not found" => Some("工作区成员 {} 未找到"),
        "Skipping {}: no kam.toml found" => Some("跳过 {}：未找到 kam.toml"),
        "Building workspace member: {}" => Some("构建工作区成员：{}"),
        "Failed to get current dir: {}" => Some("无法获取当前目录：{}"),
        "Failed to change to {}: {}" => Some("切换到目录 {} 失败: {}"),
        "Failed to build {}: {}" => Some("构建 {} 失败：{}"),
        "Skipping {}: failed to load kam.toml: {}" => Some("跳过 {}：加载 kam.toml 失败: {}"),
        "Failed to restore cwd: {}" => Some("恢复当前工作目录失败：{}"),
        "Invalid glob pattern '{}': {}" => Some("无效的 glob 模式 '{}': {}"),
        "✿ Workspace Build Summary ✿" => Some("✿ 工作区构建摘要 ✿"),
        "✗ Failed: {}" => Some("✗ 失败: {}"),
        "✓ Successfully built module: {}" => Some("✓ 成功构建模块: {}"),
        "Building module: {} v{}" => Some("构建模块：{} v{}"),
        "Skipping build hooks for template packaging" => Some("跳过用于模板打包的构建钩子"),
        "Packaging artifacts..." => Some("正在打包制品..."),
        "Source directory not found: {}" => Some("源目录未找到: {}"),
        "Generating module.prop" => Some("正在生成 module.prop"),
        "Using existing module.prop (from pre-build hook)" => Some("使用现有的 module.prop（来自预构建钩子）"),
        // Additional mapping for common messages (Set/Unset, secrets, interactive prompts, preview/next steps)
        "Set {} = {} in {}" => Some("设置 {} = {} 到 {}"),
        "Unset {} in {}" => Some("{} 已在 {} 中移除"),
        "No secrets stored." => Some("未存储任何机密。"),
        "No trusted Root CAs." => Some("没有受信任的根证书颁发机构。"),
        "(Non-interactive mode detected: falling back to text input.)" => Some("(检测到非交互模式：退回到文本输入。)"),
        "You may press Enter to accept a default value shown in brackets" => Some("您可以按 Enter 接受方括号中显示的默认值"),
        "Variable: {}" => Some("变量：{}"),
        "{} - Note: {}" => Some("{} - 备注：{}"),
        "Preview cancelled." => Some("预览已取消。"),
        "Interactive initialization completed successfully." => Some("交互式初始化已成功完成。"),
        "Next steps:" => Some("下一步："),
        "✓ Successfully built template: {}" => Some("✓ 成功构建模板: {}"),
        // Cache-related strings
        "No templates found in local cache." => Some("本地缓存中未找到模板。"),
        "Local Cached Templates" => Some("本地缓存模板"),
        "Cache cleaned successfully" => Some("缓存清理成功"),
        "Cache directory is already empty or does not exist." => Some("缓存目录已经为空或不存在。"),
        "Template '{}' added to cache from {}" => Some("模板 '{}' 已从 {} 添加到缓存"),
        "Template '{}' removed from cache" => Some("模板 '{}' 已从缓存中移除"),
        // Secrets / Trust strings
        "Secret '{}' saved" => Some("密钥 '{}' 已保存"),
        "Secret '{}' written to {}" => Some("密钥 '{}' 已写入到 {}"),
        "Secret '{}' removed" => Some("密钥 '{}' 已移除"),
        "Secret '{}' exported to {}" => Some("密钥 '{}' 已导出到 {}"),
        "Secret '{}' imported" => Some("密钥 '{}' 已导入"),
        "Public key for secret '{}' exported to {}" => Some("密钥 '{}' 的公钥已导出到 {}"),
        "Certificate chain '{}' imported successfully" => Some("证书链 '{}' 已成功导入"),
        "Root CA '{}' added to trust store" => Some("根证书 '{}' 已添加到信任存储"),
        "Root CA '{}' removed from trust store" => Some("根证书 '{}' 已从信任存储移除"),
        "Stored Secrets" => Some("存储的密钥"),
        "Trusted Root CAs" => Some("受信任的根证书颁发机构"),
        // Table labels
        "Item" => Some("配置项"),
        "Value" => Some("值"),
        "Module" => Some("模块"),
        "Total" => Some("总计"),
        "Succeeded" => Some("成功"),
        "Failed" => Some("失败"),
        "Build Time" => Some("构建时间"),
        "Total Duration" => Some("总耗时"),
        // CA fetching error messages
        "Failed to fetch CA: {}" => Some("获取 CA 失败: {}"),
        "Failed to read CA: {}" => Some("读取 CA 失败: {}"),
        // Public key / signer errors
        "Failed to derive public key: {}" => Some("推导公钥失败: {}"),
        "Failed to parse private key: {}" => Some("解析私钥失败: {}"),
        "Failed to extract public key: {}" => Some("提取公钥失败: {}"),
        "Failed to create signer: {}" => Some("创建签名器失败: {}"),
        "Failed to update signer: {}" => Some("更新签名器失败: {}"),
        "Failed to sign: {}" => Some("签名失败: {}"),
        "Failed to base64 decode signature: {}" => Some("Base64 解码签名失败: {}"),
        "Failed to parse public key PEM from {}: {}" => Some("从 {} 解析公钥 PEM 失败: {}"),
        "Failed to parse public key from certificate: {}" => Some("从证书解析公钥失败: {}"),
        "Failed to create verifier: {}" => Some("创建验证器失败: {}"),
        "Failed to update verifier: {}" => Some("更新验证器失败: {}"),
        "Verification error: {}" => Some("验证错误: {}"),
        "Verification FAILED for '{}'" => Some("验证失败：{}"),
        "Verification successful" => Some("验证成功"),
        "Verified" => Some("已验证"),
        "Source file not found: {}" => Some("源文件未找到: {}"),
        "Signature file not found: {}" => Some("签名文件未找到: {}"),
        "No trusted Root CAs found. Add one with: kam secret trust --add-root <ca.pem> --ca-name <name>" => Some("未找到受信任的根 CA。请使用：kam secret trust --add-root <ca.pem> --ca-name <name> 来添加"),
        "Verifying certificate chain..." => Some("正在验证证书链..."),
        "Certificate chain verified successfully" => Some("证书链验证成功"),
        "Loading cached certificate '{}'..." => Some("正在加载缓存证书 '{}'..."),
        "Fetching certificate from GitHub issue {}..." => Some("从 GitHub issue {} 获取证书..."),
        // Validate related
        "kam.toml not found at {}" => Some("在 {} 未找到 kam.toml"),
        "Validating {}..." => Some("正在验证 {}..."),
        "Failed to parse kam.toml: {}" => Some("解析 kam.toml 失败: {}"),
        "No issues found. kam.toml is valid." => Some("未发现问题。kam.toml 有效。"),
        "Errors:" => Some("错误："),
        "Warnings:" => Some("警告："),
        "Validation failed. Please fix the errors above." => Some("验证失败。请修复上述错误。"),
        "Validation passed with warnings." => Some("验证通过，但有警告。"),
        // Export related
        "Exported module.prop to {}" => Some("已导出 module.prop 到 {}"),
        "Exported module.json to {}" => Some("已导出 module.json 到 {}"),
        "Exported repo.json to {}" => Some("已导出 repo.json 到 {}"),
        "Exported track.json to {}" => Some("已导出 track.json 到 {}"),
        "Exported config.json to {}" => Some("已导出 config.json 到 {}"),
        "Exported update.json to {}" => Some("已导出 update.json 到 {}"),
        // Check related
        "Some issues found." => Some("发现一些问题。"),
        "No issues found." => Some("未发现问题。"),
        // Version related
        "Bumped version: {} -> {}" => Some("版本已更新：{} -> {}"),
        "Version unchanged: {}" => Some("版本未更改：{}"),
        "Updated versionCode: {} -> {}" => Some("已更新 versionCode：{} -> {}"),
        "Current version: {}" => Some("当前版本：{}"),
        "Current versionCode: {}" => Some("当前 versionCode：{}"),
        // Init/Interactive related
        "Please enter 'y' or 'n'." => Some("请输入 'y' 或 'n'。"),
        "Choose a template to use" => Some("选择要使用的模板"),
        "Enter local template path or archive file (leave empty to download default templates)" => Some("输入本地模板路径或归档文件（留空以下载默认模板）"),
        "Enter path to local template (file or dir)" => Some("输入本地模板路径（文件或目录）"),
        "Enter custom value for {}" => Some("输入 {} 的自定义值"),
        "Enter value for {} (index or value)" => Some("输入 {} 的值（索引或值）"),
        "Enter true/false for {} (default: {})" => Some("为 {} 输入 true/false（默认：{}）"),
        "Please enter 'true' or 'false'" => Some("请输入 'true' 或 'false'"),
        "Enter value for {}" => Some("输入 {} 的值"),
        "Failed to parse existing config: {}" => Some("解析现有配置失败：{}"),
        "Failed to serialize config: {}" => Some("序列化配置失败：{}"),
        "Failed to prepare initialization defaults: {}" => Some("准备初始化默认值失败：{}"),
        "Saved base configuration to ~/.kam/config.toml" => Some("已保存基础配置到 ~/.kam/config.toml"),
        "Summary" => Some("摘要"),
        "Path" => Some("路径"),
        "Template variables:" => Some("模板变量："),
        "Recommend: {}" => Some("推荐：{}"),
        "Or you may run the interactive helper script: {}" => Some("或者您可以运行交互式辅助脚本：{}"),
        "Recommended: {}" => Some("推荐：{}"),
        "(Template directory empty)" => Some("（模板目录为空）"),
        "Interactive Kam Init" => Some("交互式 Kam 初始化"),
        // Template pull related
        "Downloading templates from: {}" => Some("正在从 {} 下载模板"),
        "Importing downloaded templates..." => Some("正在导入已下载的模板..."),
        "Templates downloaded and imported successfully" => Some("模板已成功下载并导入"),
        "Could not determine file size. Progress bar will be disabled." => Some("无法确定文件大小。进度条将被禁用。"),
        // Sign related
        "Signed '{}' -> {}" => Some("已签名 '{}' -> {}"),
        "Signing failed for {}: {}" => Some("签名失败 {}: {}"),
        "Either specify 'src' or --dist/--all to sign artifacts" => Some("请指定 'src' 或 --dist/--all 来签名制品"),
        // Verify related
        "Loading certificate chain from {}..." => Some("正在从 {} 加载证书链..."),
        "Calculating hash for '{}'..." => Some("正在计算 '{}' 的哈希值..."),
        // Build hooks related
        "✿ Running {} hooks from {} ({} script(s)) ✿" => Some("✿ 正在运行来自 {} 的 {} 钩子（{} 个脚本）✿"),
        "[{} {}/{}] {}" => Some("[{} {}/{}] {}"),
        "Running pre-build hooks" => Some("正在运行预构建钩子"),
        "Running post-build hooks" => Some("正在运行后构建钩子"),
        "Build complete" => Some("构建完成"),
        // About related
        "Author" => Some("作者"),
        "Email" => Some("邮箱"),
        "Developer" => Some("开发者"),
        "Description" => Some("描述"),
        "Repository" => Some("仓库"),
        // Status labels
        "状态" => Some("状态"),
        "统计项" => Some("统计项"),
        "✓ 成功" => Some("✓ 成功"),
        _ => None,
    }
}

fn zh_to_en(s: &str) -> Option<&'static str> {
    match s {
        "此命令仅供参考，不会修改文件或注册表。" =>
            Some("This command is informational only; it doesn't modify files or the registry."),
        "请使用其他命令（例如 `kam init`, `kam build`）执行实际操作。" =>
            Some("Use other commands (e.g., `kam init`, `kam build`) to perform actions."),
        "感谢使用 Kam" => Some("Thanks for using Kam"),
        "祝您使用愉快！" => Some("Enjoy your module tooling experience!"),
        "由 Kam CLI 提供支持 — 祝构建顺利！" => Some("Powered by the Kam CLI — Happy building!"),
        "工作区成员 {} 未找到" => Some("workspace member {} not found"),
        "跳过 {}：未找到 kam.toml" => Some("Skipping {}: no kam.toml found"),
        "构建工作区成员：{}" => Some("Building workspace member: {}"),
        "无法获取当前目录：{}" => Some("Failed to get current dir: {}"),
        "切换到目录 {} 失败: {}" => Some("Failed to change to {}: {}"),
        "构建 {} 失败：{}" => Some("Failed to build {}: {}"),
        "跳过 {}：加载 kam.toml 失败: {}" => Some("Skipping {}: failed to load kam.toml: {}"),
        "恢复当前工作目录失败：{}" => Some("Failed to restore cwd: {}"),
        "无效的 glob 模式 '{}': {}" => Some("Invalid glob pattern '{}': {}"),
        "✿ 工作区构建摘要 ✿" => Some("✿ Workspace Build Summary ✿"),
        "✗ 失败: {}" => Some("✗ Failed: {}"),
        "✓ 成功构建模块: {}" => Some("✓ Successfully built module: {}"),
        "构建模块：{} v{}" => Some("Building module: {} v{}"),
        "跳过用于模板打包的构建钩子" => Some("Skipping build hooks for template packaging"),
        "正在打包制品..." => Some("Packaging artifacts..."),
        "源目录未找到: {}" => Some("Source directory not found: {}"),
        "正在生成 module.prop" => Some("Generating module.prop"),
        "使用现有的 module.prop（来自预构建钩子）" => Some("Using existing module.prop (from pre-build hook)"),
        // Additional mapping for common messages (reverse direction: zh -> en)
        "设置 {} = {} 到 {}" => Some("Set {} = {} in {}"),
        "{} 已在 {} 中移除" => Some("Unset {} in {}"),
        "未存储任何机密。" => Some("No secrets stored."),
        "没有受信任的根证书颁发机构。" => Some("No trusted Root CAs."),
        "(检测到非交互模式：退回到文本输入。)" => Some("(Non-interactive mode detected: falling back to text input.)"),
        "您可以按 Enter 接受方括号中显示的默认值" => Some("You may press Enter to accept a default value shown in brackets"),
        "变量：{}" => Some("Variable: {}"),
        "{} - 备注：{}" => Some("{} - Note: {}"),
        "预览已取消。" => Some("Preview cancelled."),
        "交互式初始化已成功完成。" => Some("Interactive initialization completed successfully."),
        "下一步：" => Some("Next steps:"),
        "✓ 成功构建模板: {}" => Some("✓ Successfully built template: {}"),
        // Cache-related strings
        "本地缓存中未找到模板。" => Some("No templates found in local cache."),
        "本地缓存模板" => Some("Local Cached Templates"),
        "缓存清理成功" => Some("Cache cleaned successfully"),
        "缓存目录已经为空或不存在。" => Some("Cache directory is already empty or does not exist."),
        "模板 '{}' 已从 {} 添加到缓存" => Some("Template '{}' added to cache from {}"),
        "模板 '{}' 已从缓存中移除" => Some("Template '{}' removed from cache"),
        // Secrets / Trust strings (zh -> en)
        "密钥 '{}' 已保存" => Some("Secret '{}' saved"),
        "密钥 '{}' 已写入到 {}" => Some("Secret '{}' written to {}"),
        "密钥 '{}' 已移除" => Some("Secret '{}' removed"),
        "密钥 '{}' 已导出到 {}" => Some("Secret '{}' exported to {}"),
        "密钥 '{}' 已导入" => Some("Secret '{}' imported"),
        "密钥 '{}' 的公钥已导出到 {}" => Some("Public key for secret '{}' exported to {}"),
        "证书链 '{}' 已成功导入" => Some("Certificate chain '{}' imported successfully"),
        "根证书 '{}' 已添加到信任存储" => Some("Root CA '{}' added to trust store"),
        "根证书 '{}' 已从信任存储移除" => Some("Root CA '{}' removed from trust store"),
        "存储的密钥" => Some("Stored Secrets"),
        "受信任的根证书颁发机构" => Some("Trusted Root CAs"),
        // Table labels
        "配置项" => Some("Item"),
        "值" => Some("Value"),
        "模块" => Some("Module"),
        "总计" => Some("Total"),
        "成功" => Some("Succeeded"),
        "失败" => Some("Failed"),
        "构建时间" => Some("Build Time"),
        "总耗时" => Some("Total Duration"),
        // CA fetching error messages (zh -> en)
        "获取 CA 失败: {}" => Some("Failed to fetch CA: {}"),
        "读取 CA 失败: {}" => Some("Failed to read CA: {}"),
        // Public key / signer errors (zh -> en)
        "推导公钥失败: {}" => Some("Failed to derive public key: {}"),
        "解析私钥失败: {}" => Some("Failed to parse private key: {}"),
        "提取公钥失败: {}" => Some("Failed to extract public key: {}"),
        "创建签名器失败: {}" => Some("Failed to create signer: {}"),
        "更新签名器失败: {}" => Some("Failed to update signer: {}"),
        "签名失败: {}" => Some("Failed to sign: {}"),
        "Base64 解码签名失败: {}" => Some("Failed to base64 decode signature: {}"),
        "从 {} 解析公钥 PEM 失败: {}" => Some("Failed to parse public key PEM from {}: {}"),
        "从证书解析公钥失败: {}" => Some("Failed to parse public key from certificate: {}"),
        "创建验证器失败: {}" => Some("Failed to create verifier: {}"),
        "更新验证器失败: {}" => Some("Failed to update verifier: {}"),
        "验证错误: {}" => Some("Verification error: {}"),
        "验证失败：{}" => Some("Verification FAILED for '{}'"),
        "验证成功" => Some("Verification successful"),
        "已验证" => Some("Verified"),
        "源文件未找到: {}" => Some("Source file not found: {}"),
        "签名文件未找到: {}" => Some("Signature file not found: {}"),
        "未找到受信任的根 CA。请使用：kam secret trust --add-root <ca.pem> --ca-name <name> 来添加" => Some("No trusted Root CAs found. Add one with: kam secret trust --add-root <ca.pem> --ca-name <name>"),
        "正在验证证书链..." => Some("Verifying certificate chain..."),
        "证书链验证成功" => Some("Certificate chain verified successfully"),
        "正在加载缓存证书 '{}'..." => Some("Loading cached certificate '{}'..."),
        "从 GitHub issue {} 获取证书..." => Some("Fetching certificate from GitHub issue {}..."),
        // Validate related (zh -> en)
        "在 {} 未找到 kam.toml" => Some("kam.toml not found at {}"),
        "正在验证 {}..." => Some("Validating {}..."),
        "解析 kam.toml 失败: {}" => Some("Failed to parse kam.toml: {}"),
        "未发现问题。kam.toml 有效。" => Some("No issues found. kam.toml is valid."),
        "错误：" => Some("Errors:"),
        "警告：" => Some("Warnings:"),
        "验证失败。请修复上述错误。" => Some("Validation failed. Please fix the errors above."),
        "验证通过，但有警告。" => Some("Validation passed with warnings."),
        // Export related (zh -> en)
        "已导出 module.prop 到 {}" => Some("Exported module.prop to {}"),
        "已导出 module.json 到 {}" => Some("Exported module.json to {}"),
        "已导出 repo.json 到 {}" => Some("Exported repo.json to {}"),
        "已导出 track.json 到 {}" => Some("Exported track.json to {}"),
        "已导出 config.json 到 {}" => Some("Exported config.json to {}"),
        "已导出 update.json 到 {}" => Some("Exported update.json to {}"),
        // Check related (zh -> en)
        "发现一些问题。" => Some("Some issues found."),
        "未发现问题。" => Some("No issues found."),
        // Version related (zh -> en)
        "版本已更新：{} -> {}" => Some("Bumped version: {} -> {}"),
        "版本未更改：{}" => Some("Version unchanged: {}"),
        "已更新 versionCode：{} -> {}" => Some("Updated versionCode: {} -> {}"),
        "当前版本：{}" => Some("Current version: {}"),
        "当前 versionCode：{}" => Some("Current versionCode: {}"),
        // Init/Interactive related (zh -> en)
        "请输入 'y' 或 'n'。" => Some("Please enter 'y' or 'n'."),
        "选择要使用的模板" => Some("Choose a template to use"),
        "输入本地模板路径或归档文件（留空以下载默认模板）" => Some("Enter local template path or archive file (leave empty to download default templates)"),
        "输入本地模板路径（文件或目录）" => Some("Enter path to local template (file or dir)"),
        "输入 {} 的自定义值" => Some("Enter custom value for {}"),
        "输入 {} 的值（索引或值）" => Some("Enter value for {} (index or value)"),
        "为 {} 输入 true/false（默认：{}）" => Some("Enter true/false for {} (default: {})"),
        "请输入 'true' 或 'false'" => Some("Please enter 'true' or 'false'"),
        "输入 {} 的值" => Some("Enter value for {}"),
        "解析现有配置失败：{}" => Some("Failed to parse existing config: {}"),
        "序列化配置失败：{}" => Some("Failed to serialize config: {}"),
        "准备初始化默认值失败：{}" => Some("Failed to prepare initialization defaults: {}"),
        "已保存基础配置到 ~/.kam/config.toml" => Some("Saved base configuration to ~/.kam/config.toml"),
        "摘要" => Some("Summary"),
        "路径" => Some("Path"),
        "模板变量：" => Some("Template variables:"),
        "推荐：{}" => Some("Recommend: {}"),
        "或者您可以运行交互式辅助脚本：{}" => Some("Or you may run the interactive helper script: {}"),
        "（模板目录为空）" => Some("(Template directory empty)"),
        "交互式 Kam 初始化" => Some("Interactive Kam Init"),
        // Template pull related (zh -> en)
        "正在从 {} 下载模板" => Some("Downloading templates from: {}"),
        "正在导入已下载的模板..." => Some("Importing downloaded templates..."),
        "模板已成功下载并导入" => Some("Templates downloaded and imported successfully"),
        "无法确定文件大小。进度条将被禁用。" => Some("Could not determine file size. Progress bar will be disabled."),
        // Sign related (zh -> en)
        "已签名 '{}' -> {}" => Some("Signed '{}' -> {}"),
        "签名失败 {}: {}" => Some("Signing failed for {}: {}"),
        "请指定 'src' 或 --dist/--all 来签名制品" => Some("Either specify 'src' or --dist/--all to sign artifacts"),
        // Verify related (zh -> en)
        "正在从 {} 加载证书链..." => Some("Loading certificate chain from {}..."),
        "正在计算 '{}' 的哈希值..." => Some("Calculating hash for '{}'..."),
        // Build hooks related (zh -> en)
        "✿ 正在运行来自 {} 的 {} 钩子（{} 个脚本）✿" => Some("✿ Running {} hooks from {} ({} script(s)) ✿"),
        "[{} {}/{}] {}" => Some("[{} {}/{}] {}"),
        "正在运行预构建钩子" => Some("Running pre-build hooks"),
        "正在运行后构建钩子" => Some("Running post-build hooks"),
        "构建完成" => Some("Build complete"),
        // Config command related (zh -> en)
        "内置配置项：" => Some("Built-in configuration keys:"),
        "示例：" => Some("Example:"),
        "注意：您也可以设置自定义键。使用 'kam config list' 查看所有已配置的值。" => Some("Note: You can also set custom keys. Use 'kam config list' to see all configured values."),
        "UI 语言偏好（优先于 'language'）" => Some("UI language preference (preferred over 'language')"),
        "UI 语言偏好（备用，建议使用 'ui.language'）" => Some("UI language preference (fallback, use 'ui.language' instead)"),
        "新项目的默认作者名称（在初始化时保存）" => Some("Default author name for new projects (saved during init)"),
        "新项目的默认模块名称（在初始化时保存）" => Some("Default module name for new projects (saved during init)"),
        "新项目的默认版本（在初始化时保存）" => Some("Default version for new projects (saved during init)"),
        // Config command related (en -> zh)
        "Built-in configuration keys:" => Some("内置配置项："),
        "Example:" => Some("示例："),
        "Note: You can also set custom keys. Use 'kam config list' to see all configured values." => Some("注意：您也可以设置自定义键。使用 'kam config list' 查看所有已配置的值。"),
        "UI language preference (preferred over 'language')" => Some("UI 语言偏好（优先于 'language'）"),
        "UI language preference (fallback, use 'ui.language' instead)" => Some("UI 语言偏好（备用，建议使用 'ui.language'）"),
        "Default author name for new projects (saved during init)" => Some("新项目的默认作者名称（在初始化时保存）"),
        "Default module name for new projects (saved during init)" => Some("新项目的默认模块名称（在初始化时保存）"),
        "Default version for new projects (saved during init)" => Some("新项目的默认版本（在初始化时保存）"),
        // About related (zh -> en)
        "作者" => Some("Author"),
        "邮箱" => Some("Email"),
        "开发者" => Some("Developer"),
        "描述" => Some("Description"),
        "仓库" => Some("Repository"),
        // Status labels (zh -> en)
        "状态" => Some("状态"),
        "统计项" => Some("统计项"),
        "✓ 成功" => Some("✓ 成功"),
        _ => None,
    }
}

// --- Public helpers for config-based language retrieval ----------------------

/// Tries to read `language` or `ui.language` from the effective `kam` config
/// file (local if in a project, otherwise global). To avoid code duplication,
/// the config module provides a small public helper. If that helper is not
/// available, this function falls back to a best-effort attempt to parse the
/// default config path.
fn get_language_from_config() -> Option<String> {
    // Prefer calling into `cmds::config::read_language_from_config()` if it exists.
    // To keep backward compatibility in case it isn't exported, we do this in a
    // guarded fashion.
    // Note: This function expects `cmds::config::read_language_from_config()` to be
    // public and return `Option<String>`. If you changed `config.rs` keep an eye on
    // compilation errors and export a tiny helper there.
    match std::panic::catch_unwind(|| {
        // Use fully-qualified absolute path to call function in case `cfg` differs:
        crate::cmds::config::read_language_from_config()
    }) {
        Ok(v) => v,
        Err(_) => None,
    }
}

// --- Macros -----------------------------------------------------------------
// Macro-style formatting that performs lookup of the template first and then
// formats it with the provided arguments.
//
// Usage:
//   trf!("Building module: {} v{}", &module_id, &version)
// trf macro has been moved to crate root (lib.rs) to ensure visibility across modules.
// This duplicate definition was removed from i18n.rs to avoid conflicts.

// Also provide a simple `tr` function to translate a single key.
pub fn tr<'a>(s: &'a str) -> String {
    tr_key(s).to_string()
}

// --- Unit tests (optional) --------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_en_to_zh() {
        set_language(Language::Zh);
        assert_eq!(tr_key("Thanks for using Kam"), "感谢使用 Kam");
    }

    #[test]
    fn test_translate_zh_to_en() {
        set_language(Language::En);
        assert_eq!(tr_key("配置项"), "Item");
    }

    #[test]
    fn test_trf_macro_behaviour() {
        set_language(Language::Zh);
        // Using the macro, with English key
        let s = trf!("Building module: {} v{}", "mod", "1.0");
        assert!(s.contains("构建模块") || s.contains("构建模块："));
    }
}
