use git2::Repository;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::types::kam_toml::enums::ModuleType;

// 初始化前的数据结构
// 把各种参数和默认值都收集到这里
pub struct PreInitData {
    pub path: PathBuf,
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub template_vars: HashMap<String, String>,
    pub impl_template: String,
    pub module_type: ModuleType,
    pub update_json: Option<String>,
    pub kam_toml: KamToml,
}

// 准备初始化数据，在创建项目之前
// 这里主要是处理各种参数、默认值、git信息等
pub fn prepare_init(args: &super::InitArgs) -> Result<PreInitData, KamError> {
    let current_dir = std::env::current_dir()?;
    // 交互模式下name可能为空，默认用"."（当前目录）
    let project_name_raw = args.name.as_deref().unwrap_or(".");
    let project_path: PathBuf = if project_name_raw.starts_with('/')
        || project_name_raw.starts_with('\\')
        || project_name_raw.contains(':')
    {
        PathBuf::from(project_name_raw)
    } else {
        current_dir.join(project_name_raw)
    };

    // 检查冲突的标志
    // --tmpl 和 -t/--template 是互斥的，不能同时用
    let type_flags = [args.tmpl, args.template.is_some()]
        .iter()
        .filter(|&&x| x)
        .count();
    if type_flags > 1 {
        return Err(KamError::InvalidModuleType(
            "Cannot specify multiple module types: --tmpl, -t/--template".to_string(),
        ));
    }

    // 确定模块类型和模板
    // -t/--template 支持：
    //  1) 完整的模板ID（如 "kam_template" 或 "kam_template.tar.gz"）
    //  2) 简短的builtin ID（如 "kam", "meta", "ak3"），会自动映射到 "<id>_template"
    //  3) 本地路径或压缩包（如 /path/to/template.tar.gz 或 https://.../template.tar.gz）
    //
    // 注意：这里不再强制添加 "_template" 后缀，直接传原始输入
    // 让下一阶段的 impl_mod/template manager 来做智能发现
    let (module_type, impl_template) = if args.tmpl {
        (ModuleType::Template, "tmpl_template".to_string())
    } else if let Some(t) = &args.template {
        // Pass raw string. Discovery logic in `impl_mod.rs` will handle suffixing if needed.
        (ModuleType::Kam, t.clone())
    } else {
        // 默认是kam模块
        (ModuleType::Kam, "kam_template".to_string())
    };

    // 解析模板变量（从 --var 参数）
    let mut template_vars = crate::template::TemplateManager::parse_template_vars(&args.var)?;

    let version = args.version.as_deref().unwrap_or("1.0.0");

    // 早点发现Git信息，可以用来做默认值（id、author、repo URL等）
    let mut git_author: Option<String> = None;
    let mut git_repo_url: Option<String> = None;
    let mut git_branch: Option<String> = None;
    let mut git_repo_name: Option<String> = None;
    let mut git_owner: Option<String> = None;

    // 尝试发现git仓库
    if let Ok(repo) = Repository::discover(&current_dir) {
        // 获取 git user.name
        if let Ok(cfg) = repo.config()
            && let Ok(name) = cfg.get_string("user.name")
        {
            git_author = Some(name);
        }

        // 获取 remote URL
        if let Ok(remote) = repo.find_remote("origin")
            && let Some(url) = remote.url()
        {
            git_repo_url = Some(url.to_string());
        }
        // 如果没找到origin，就用第一个可用的remote
        if git_repo_url.is_none()
            && let Ok(remotes) = repo.remotes()
            && let Some(name) = remotes.get(0)
            && let Ok(remote) = repo.find_remote(name)
            && let Some(url) = remote.url()
        {
            git_repo_url = Some(url.to_string());
        }

        // 从 remote URL 解析 owner 和 repo name
        if let Some(ref url) = git_repo_url
            && let Some((owner, repo_name)) = parse_git_remote_url(url)
        {
            git_owner = Some(owner);
            git_repo_name = Some(repo_name);
        }

        // 获取当前分支名
        if let Ok(head) = repo.head()
            && let Some(name) = head.name()
        {
            // 尝试从 refs/heads/xxx 提取分支名
            if let Some(stripped) = name.strip_prefix("refs/heads/") {
                git_branch = Some(stripped.to_string());
            } else if let Some(branch_name) = head.shorthand() {
                // 如果已经是简短名称，直接使用
                git_branch = Some(branch_name.to_string());
            }
        }
    }

    // 默认分支名，如果无法获取则使用 "main"
    let default_branch = git_branch.as_deref().unwrap_or("main");

    // 智能推断项目名称：优先使用 git 仓库名，其次目录名
    let project_name_str = args.project_name.as_ref().map_or_else(
        || {
            if let Some(ref repo_name) = git_repo_name {
                // 将仓库名转换为更友好的显示名称（去掉下划线/横线，首字母大写等）
                let mut name = repo_name.clone();
                // 简单的美化：将下划线和横线替换为空格，并首字母大写
                name = name.replace(['_', '-'], " ");
                // 简单的首字母大写（只处理第一个单词）
                if let Some(first_char) = name.chars().next() {
                    let mut chars: Vec<char> = name.chars().collect();
                    chars[0] = first_char.to_uppercase().next().unwrap_or(first_char);
                    name = chars.into_iter().collect();
                }
                name
            } else if project_name_raw != "." {
                // 从路径提取目录名并美化
                let dir_name = std::path::Path::new(project_name_raw)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(project_name_raw)
                    .to_string();
                let mut name = dir_name.replace(['_', '-'], " ");
                if let Some(first_char) = name.chars().next() {
                    let mut chars: Vec<char> = name.chars().collect();
                    chars[0] = first_char.to_uppercase().next().unwrap_or(first_char);
                    name = chars.into_iter().collect();
                }
                name
            } else {
                // 从当前目录名提取
                let dir_name = current_dir
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Example Module Name")
                    .to_string();
                let mut name = dir_name.replace(['_', '-'], " ");
                if let Some(first_char) = name.chars().next() {
                    let mut chars: Vec<char> = name.chars().collect();
                    chars[0] = first_char.to_uppercase().next().unwrap_or(first_char);
                    name = chars.into_iter().collect();
                }
                name
            }
        },
        |name| name.clone(),
    );

    // 智能推断描述：优先从 README.md 提取，其次使用默认值
    let description_str = args.description.as_ref().map_or_else(
        || {
            // 尝试从 README.md 提取描述
            let readme_paths = [
                current_dir.join("README.md"),
                current_dir.join("README"),
                current_dir.join("readme.md"),
            ];

            let mut extracted_desc: Option<String> = None;
            for readme_path in &readme_paths {
                if let Ok(content) = std::fs::read_to_string(readme_path) {
                    // 提取第一段非空文本作为描述
                    for line in content.lines() {
                        let trimmed = line.trim();
                        // 跳过标题（以 # 开头）、空行、代码块标记等
                        if !trimmed.is_empty()
                            && !trimmed.starts_with('#')
                            && !trimmed.starts_with("```")
                            && !trimmed.starts_with("<!--")
                            && trimmed.len() > 10
                        {
                            // 取前 200 个字符作为描述
                            let desc = if trimmed.len() > 200 {
                                format!("{}...", &trimmed[..200])
                            } else {
                                trimmed.to_string()
                            };
                            extracted_desc = Some(desc);
                            break;
                        }
                    }
                    if extracted_desc.is_some() {
                        break;
                    }
                }
            }

            extracted_desc.unwrap_or_else(|| match module_type {
                ModuleType::Kam => "Describe your module here".to_string(),
                ModuleType::Template => "Describe your template here".to_string(),
            })
        },
        |desc| desc.clone(),
    );
    template_vars.insert("project_name".to_string(), project_name_str.clone());
    template_vars.insert("description".to_string(), description_str.to_string());

    // 确定ID：优先用 --id，没有就用git repo名（如果检测到仓库且name不是'.'），再没有就用文件夹名
    let id = args.id.as_ref().map_or_else(
        || {
            if project_name_raw == "." {
                // if git repo identified, prefer repo name
                if let Some(repo_url) = git_repo_url.clone() {
                    if let Some((_owner, repo_name)) = parse_git_remote_url(&repo_url) {
                        repo_name
                    } else {
                        std::env::current_dir()
                            .unwrap()
                            .file_name()
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .to_string()
                    }
                } else {
                    std::env::current_dir()
                        .unwrap()
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string()
                }
            } else {
                // Extract basename from path (e.g., "/tmp/test_kam_init" -> "test_kam_init")
                std::path::Path::new(project_name_raw)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(project_name_raw)
                    .to_string()
            }
        },
        |custom_id| custom_id.clone(),
    );

    // 验证ID格式：只能包含字母数字、点、横线、下划线
    if !id
        .chars()
        .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_')
    {
        return Err(KamError::InvalidConfig(format!(
            "Invalid module ID '{}': ID must contain only alphanumeric characters, dots, dashes, and underscores",
            id
        )));
    }

    if id.is_empty() {
        return Err(KamError::InvalidConfig(
            "Module ID cannot be empty".to_string(),
        ));
    }

    // 确定author，优先级：命令行参数 > git user.name > 全局配置 > 默认值
    // 全局配置位于 Kam 家目录下的 config.toml (可通过 `KAM_HOME` 环境变量覆盖，默认为 ~/.kam)
    let mut global_author: Option<String> = None;
    if let Ok(cfg_home) = crate::utils::kam_home_dir() {
        let cfg_path = cfg_home.join("config.toml");
        if cfg_path.exists()
            && let Ok(content) = std::fs::read_to_string(&cfg_path)
            && let Ok(v) = toml::from_str::<toml::Value>(&content)
            && let Some(prop) = v.get("prop")
            && let Some(author_val) = prop.get("author")
            && let Some(s) = author_val.as_str()
        {
            global_author = Some(s.to_string());
        }
    }

    let author = args.author.as_deref().map_or_else(
        || {
            if let Some(a) = git_author {
                a
            } else if let Some(a) = global_author {
                a
            } else {
                "Your Name".to_string()
            }
        },
        |a| a.to_string(),
    );

    let update_json_val = args
        .update_json
        .clone()
        .or_else(|| crate::types::kam_toml::sections::PropSection::default().updateJson);

    // 创建初始的KamToml，用默认值
    let mut kam_toml = KamToml::new_with_current_timestamp(
        id.clone(),
        project_name_str.to_string(),
        version.to_string(),
        Some(author),
        description_str.to_string(),
        update_json_val,
        None,
    );

    // 设置name和description（允许后续覆盖）
    kam_toml.prop.name = project_name_str.to_string();
    kam_toml.prop.description = description_str.to_string();

    let update_json = kam_toml.prop.updateJson.clone();

    if let Some(uj) = &update_json {
        template_vars.insert("update_json".to_string(), uj.clone());
    }

    // 设置zipUrl和changelog的默认值（用id）
    template_vars
        .entry("zipUrl".to_string())
        .or_insert_with(|| {
            format!(
                "https://github.com/user/repo/releases/latest/download/{}.zip",
                id
            )
        });
    template_vars
        .entry("changelog".to_string())
        .or_insert_with(|| {
            "https://raw.githubusercontent.com/user/repo/branch/CHANGELOG.md".to_string()
        });

    // 如果发现了git仓库，就用更智能的默认值
    if let Some(repo_url) = git_repo_url {
        // 从remote URL解析出owner/repo
        if let (Some(owner), Some(repo_name)) = (git_owner, git_repo_name) {
            // 如果update_json没设置，就用raw.githubusercontent的默认值（使用实际分支名）
            if kam_toml.prop.updateJson.is_none() {
                let default_update = format!(
                    "https://raw.githubusercontent.com/{}/{}/{}/update.json",
                    owner, repo_name, default_branch
                );
                kam_toml.prop.updateJson = Some(default_update.clone());
                template_vars.insert("update_json".to_string(), default_update);
            }

            // 如果zipUrl和changelog还是默认值，就用仓库相关的值替换
            template_vars
                .entry("zipUrl".to_string())
                .or_insert_with(|| {
                    format!(
                        "https://github.com/{}/{}/releases/latest/download/{}.zip",
                        owner, repo_name, id
                    )
                });
            template_vars
                .entry("changelog".to_string())
                .or_insert_with(|| {
                    format!(
                        "https://raw.githubusercontent.com/{}/{}/{}/CHANGELOG.md",
                        owner, repo_name, default_branch
                    )
                });

            // 设置mmrl的repo section（如果有的话），填充更多字段
            kam_toml
                .mmrl
                .get_or_insert_with(crate::types::kam_toml::sections::MmrlSection::default);
            if let Some(mmrl) = &mut kam_toml.mmrl {
                if mmrl.repo.is_none() {
                    mmrl.repo = Some(crate::types::kam_toml::sections::RepoSection::default());
                }
                if let Some(repo) = &mut mmrl.repo {
                    // 设置仓库 URL
                    repo.repository = Some(repo_url);

                    // 设置 homepage（GitHub 仓库主页）
                    if repo.homepage.is_none() || repo.homepage.as_ref().unwrap().is_empty() {
                        repo.homepage = Some(format!("https://github.com/{}/{}", owner, repo_name));
                    }

                    // 设置 readme URL
                    if repo.readme.is_none() || repo.readme.as_ref().unwrap().is_empty() {
                        repo.readme = Some(format!(
                            "https://raw.githubusercontent.com/{}/{}/{}/README.md",
                            owner, repo_name, default_branch
                        ));
                    }

                    // 设置 issues URL
                    if repo.issues.is_none() || repo.issues.as_ref().unwrap().is_empty() {
                        repo.issues =
                            Some(format!("https://github.com/{}/{}/issues", owner, repo_name));
                    }

                    // 设置 documentation URL（如果有 docs 目录或 README）
                    if repo.documentation.is_none()
                        || repo.documentation.as_ref().unwrap().is_empty()
                    {
                        repo.documentation = Some(format!(
                            "https://github.com/{}/{}/blob/{}/README.md",
                            owner, repo_name, default_branch
                        ));
                    }

                    // 检测并设置许可证信息
                    if repo.license.is_none() || repo.license.as_ref().unwrap().is_empty() {
                        // 尝试从 LICENSE 文件检测许可证类型
                        let license_paths = [
                            current_dir.join("LICENSE"),
                            current_dir.join("LICENSE.txt"),
                            current_dir.join("LICENSE.md"),
                            current_dir.join("license"),
                        ];

                        for license_path in &license_paths {
                            if let Ok(content) = std::fs::read_to_string(license_path) {
                                // 简单的许可证类型检测
                                let content_upper = content.to_uppercase();
                                if content_upper.contains("MIT") {
                                    repo.license = Some("MIT".to_string());
                                    repo.license_file = Some(
                                        license_path
                                            .file_name()
                                            .and_then(|s| s.to_str())
                                            .map(|s| s.to_string())
                                            .unwrap_or_else(|| "LICENSE".to_string()),
                                    );
                                    break;
                                } else if content_upper.contains("APACHE")
                                    && content_upper.contains("2.0")
                                {
                                    repo.license = Some("Apache-2.0".to_string());
                                    repo.license_file = Some(
                                        license_path
                                            .file_name()
                                            .and_then(|s| s.to_str())
                                            .map(|s| s.to_string())
                                            .unwrap_or_else(|| "LICENSE".to_string()),
                                    );
                                    break;
                                } else if content_upper.contains("GPL")
                                    && content_upper.contains("3.0")
                                {
                                    repo.license = Some("GPL-3.0".to_string());
                                    repo.license_file = Some(
                                        license_path
                                            .file_name()
                                            .and_then(|s| s.to_str())
                                            .map(|s| s.to_string())
                                            .unwrap_or_else(|| "LICENSE".to_string()),
                                    );
                                    break;
                                } else if content_upper.contains("GPL")
                                    && content_upper.contains("2.0")
                                {
                                    repo.license = Some("GPL-2.0".to_string());
                                    repo.license_file = Some(
                                        license_path
                                            .file_name()
                                            .and_then(|s| s.to_str())
                                            .map(|s| s.to_string())
                                            .unwrap_or_else(|| "LICENSE".to_string()),
                                    );
                                    break;
                                } else if content_upper.contains("BSD")
                                    && content_upper.contains("3")
                                {
                                    repo.license = Some("BSD-3-Clause".to_string());
                                    repo.license_file = Some(
                                        license_path
                                            .file_name()
                                            .and_then(|s| s.to_str())
                                            .map(|s| s.to_string())
                                            .unwrap_or_else(|| "LICENSE".to_string()),
                                    );
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 返回初始化数据，所有信息都准备好了
    Ok(PreInitData {
        path: project_path,
        id,
        name: project_name_str,
        version: version.to_string(),
        // author可能是None，用空字符串作为默认值
        // 虽然可能不太优雅，但至少不会panic
        author: kam_toml
            .prop
            .author
            .as_ref()
            .unwrap_or(&String::new())
            .clone(),
        description: description_str,
        update_json: kam_toml.prop.updateJson.clone(),
        template_vars,
        impl_template,
        module_type,
        kam_toml,
    })
    // 这个函数终于写完了，虽然有点长但逻辑还算清晰
    // TODO: 也许可以拆分成更小的函数？不过暂时先这样
}

// 解析git remote URL，提取owner和repo
// 支持：
// - git@github.com:owner/repo.git
// - https://github.com/owner/repo.git
fn parse_git_remote_url(remote: &str) -> Option<(String, String)> {
    let s = remote.trim();
    let s = if s.starts_with("git@") {
        // 把 git@ 格式转成 https:// 格式
        s.find(':').map_or_else(
            || s.to_string(),
            |idx| {
                let host = &s[4..idx];
                let path = &s[idx + 1..];
                format!("https://{}/{}", host, path)
            },
        )
    } else {
        s.to_string()
    };

    // 去掉scheme（https:// 或 http://）
    let path_start = s.find("//").map_or(0, |idx| idx + 2);
    let path = &s[path_start..];
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() >= 3 {
        let owner = parts[1].to_string();
        let mut repo = parts[2].to_string();
        // 去掉 .git 后缀
        if repo.ends_with(".git") {
            repo.truncate(repo.len() - 4);
        }
        return Some((owner, repo));
    }
    None
}
