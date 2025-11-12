
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
/// 打包/构建配置节
///
/// - `target_dir`：打包输出目录，默认 "dist"
/// - `output_file`：可选的输出文件名（为空时使用 `<id>-<version>.zip`）
/// - `pre_build` / `post_build`：可选的 shell/PowerShell 钩子命令字符串
pub struct BuildSection {
    pub target_dir: Option<String>,
    pub output_file: Option<String>,
    pub pre_build: Option<String>,
    pub post_build: Option<String>,
}

impl Default for BuildSection {
    fn default() -> Self {
        // Provide sensible cross-platform defaults for pre/post build hooks.
        // On Windows we use a simple echo, on other platforms use echo as well
        // but prefer single quotes to avoid PowerShell vs shell quoting issues.
        let pre = if cfg!(target_os = "windows") {
            Some("echo \"pre build...\"".to_string())
        } else {
            Some("echo 'pre build...'".to_string())
        };

        let post = if cfg!(target_os = "windows") {
            Some("echo \"post build...\"".to_string())
        } else {
            Some("echo 'post build...'".to_string())
        };

        BuildSection {
            target_dir: Some("dist".to_string()),
            output_file: Some(String::new()),
            pre_build: pre,
            post_build: post,
        }
    }
}
