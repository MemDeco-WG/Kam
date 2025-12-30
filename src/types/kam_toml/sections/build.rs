use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[allow(non_snake_case)]
// 额外包含的文件配置
pub struct ExtraInclude {
    // 源文件相对路径（相对于项目根目录）
    pub source: String,
    // 目标路径（打包进压缩包的相对路径）
    pub dest: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[allow(non_snake_case)]
// 打包/构建配置节
// - source_dir：自定义源代码目录（默认为 "src/{{id}}"，初始化时会展开为实际模块 ID）
// - target_dir：打包输出目录，默认 "dist"
// - output_file：可选的输出文件名（为空时使用 <id>-<versionCode>-<version>.zip）
// - hooks_dir：钩子脚本目录，默认 "hooks"
// - extra_includes：额外包含的文件列表
// - exclude：额外的排除路径列表（支持 glob 模式）
// - include：强制包含的路径列表（覆盖 exclude，支持 glob 模式）
// - respect_gitignore：是否尊重顶层 `.gitignore` 的规则（默认 false），
//   由 `kam.toml` 中 `kam.build.respect_gitignore` 显式控制。
pub struct BuildSection {
    pub source_dir: Option<String>,
    pub target_dir: Option<String>,
    pub output_file: Option<String>,
    pub hooks_dir: Option<String>,
    pub extra_includes: Option<Vec<ExtraInclude>>,
    pub exclude: Option<Vec<String>>,
    pub include: Option<Vec<String>>,
    pub respect_gitignore: Option<bool>,
}

impl Default for BuildSection {
    fn default() -> Self {
        Self {
            source_dir: Some("src/{{id}}".to_string()),
            target_dir: Some("dist".to_string()),
            output_file: Some("{{id}}-{{versionCode}}-{{version}}".to_string()),
            hooks_dir: Some("hooks".to_string()),
            extra_includes: None,
            exclude: Some(vec![
                ".git/".to_string(),
                "target/".to_string(),
                "node_modules/".to_string(),
                ".DS_Store".to_string(),
                "Thumbs.db".to_string(),
                "*.tmp".to_string(),
                "*.log".to_string(),
                "*.bak".to_string(),
                ".kam/".to_string(),
            ]),
            include: Some(vec![
                "META-INF/".to_string(),
                "system/".to_string(),
                "customize.sh".to_string(),
                "module.prop".to_string(),
                "service.sh".to_string(),
                "post-fs-data.sh".to_string(),
                "uninstall.sh".to_string(),
            ]),
            // By default do NOT respect .gitignore; explicit control via kam.toml.
            respect_gitignore: Some(false),
        }
    }
}
