use crate::types::kam_toml::KamToml;

// 构建module.prop文件内容
// 就是简单的key=value格式，一行一行拼起来
pub fn build_prop(kt: &KamToml) -> String {
    let mut content = String::new();
    content.push_str(&format!("id={}\n", kt.prop.id));
    content.push_str(&format!("name={}\n", kt.prop.name));
    content.push_str(&format!("version={}\n", kt.prop.version));
    content.push_str(&format!("versionCode={}\n", kt.prop.versionCode));
    // author是可选的，有的话才写
    if let Some(author) = &kt.prop.author {
        content.push_str(&format!("author={}\n", author));
    }
    content.push_str(&format!("description={}\n", kt.prop.description));
    if let Some(uj) = &kt.prop.updateJson {
        content.push_str(&format!("updateJson={}\n", uj));
    }
    content
}

// 构建module.json内容（MMRL格式）
// 这个函数有点复杂，主要是处理各种可选字段和嵌套结构
pub fn build_module_json(kt: &KamToml) -> serde_json::Value {
    let metamodule = kt.prop.metamodule;
    let summary = kt.prop.description.clone();
    // 优先用repository，没有就用homepage，都没有就空字符串
    let source_url = kt
        .mmrl
        .as_ref()
        .and_then(|m| m.repo.as_ref())
        .and_then(|r| r.repository.clone())
        .or_else(|| {
            kt.mmrl
                .as_ref()
                .and_then(|m| m.repo.as_ref())
                .and_then(|r| r.homepage.clone())
        })
        .unwrap_or_default();

    // 收集作者列表，从prop.author和mmrl.repo.maintainers里找
    let mut authors: Vec<serde_json::Value> = vec![];
    if let Some(author) = &kt.prop.author
        && !author.trim().is_empty() {
            let mut author_obj = serde_json::Map::new();
            author_obj.insert(
                "type".to_string(),
                serde_json::Value::String("add".to_string()),
            );
            author_obj.insert(
                "name".to_string(),
                serde_json::Value::String(author.clone()),
            );
            authors.push(serde_json::Value::Object(author_obj));
        }
    // 从mmrl.repo.maintainers里也收集作者
    // maintainers可以是字符串或对象，需要分别处理
    if let Some(mmrl) = &kt.mmrl
        && let Some(repo) = &mmrl.repo
            && let Some(maintainers) = &repo.maintainers {
                for maint in maintainers {
                    match maint {
                        crate::types::kam_toml::sections::repo::MaintainerEntry::Name(s) => {
                            // 简单字符串格式，直接转成对象
                            let mut obj = serde_json::Map::new();
                            obj.insert(
                                "type".to_string(),
                                serde_json::Value::String("add".to_string()),
                            );
                            obj.insert("name".to_string(), serde_json::Value::String(s.clone()));
                            authors.push(serde_json::Value::Object(obj));
                        }
                        crate::types::kam_toml::sections::repo::MaintainerEntry::Object(m) => {
                            // 对象格式，可能有type、name、link等字段
                            let mut obj = serde_json::Map::new();
                            let t = m.r#type.clone().unwrap_or_else(|| "add".to_string());
                            obj.insert("type".to_string(), serde_json::Value::String(t));
                            obj.insert(
                                "name".to_string(),
                                serde_json::Value::String(m.name.clone()),
                            );
                            if let Some(link) = &m.link {
                                obj.insert(
                                    "link".to_string(),
                                    serde_json::Value::String(link.clone()),
                                );
                            }
                            authors.push(serde_json::Value::Object(obj));
                        }
                    }
                }
            }

    let mut root = serde_json::Map::new();
    root.insert(
        "metamodule".to_string(),
        serde_json::Value::Bool(metamodule),
    );
    root.insert("summary".to_string(), serde_json::Value::String(summary));
    root.insert(
        "sourceUrl".to_string(),
        serde_json::Value::String(source_url),
    );
    root.insert(
        "additionalAuthors".to_string(),
        serde_json::Value::Array(authors),
    );
    serde_json::Value::Object(root)
}

// 构建update.json内容
// 包含版本、下载链接、changelog等
pub fn build_update_json(kt: &KamToml) -> serde_json::Value {
    let version = kt.prop.version.clone();
    let version_code = kt.prop.versionCode;

    // 获取仓库URL（优先repository，没有就用homepage）
    let repo_url = kt
        .mmrl
        .as_ref()
        .and_then(|m| m.repo.as_ref())
        .and_then(|r| r.repository.clone())
        .or_else(|| {
            kt.mmrl
                .as_ref()
                .and_then(|m| m.repo.as_ref())
                .and_then(|r| r.homepage.clone())
        })
        .unwrap_or_default();

    // 构建zip下载URL
    // 如果是GitHub就按GitHub的格式，否则用通用格式
    // 都没有就用默认的占位符URL
    let zip_url = if repo_url.contains("github.com") {
        format!(
            "{}/releases/latest/download/{}.zip",
            repo_url.trim_end_matches('/'),
            kt.prop.id
        )
    } else if !repo_url.is_empty() {
        // 非GitHub仓库，也用类似的格式（虽然可能不太对）
        format!(
            "{}/releases/latest/download/{}.zip",
            repo_url.trim_end_matches('/'),
            kt.prop.id
        )
    } else {
        // 没有仓库URL，用默认占位符
        format!(
            "https://github.com/user/repo/releases/latest/download/{}.zip",
            kt.prop.id
        )
    };

    // 构建changelog URL
    // 优先用配置的changelog，没有就根据仓库URL推断
    let changelog = kt
        .mmrl
        .as_ref()
        .and_then(|m| m.repo.as_ref())
        .and_then(|r| r.changelog.clone())
        .or_else(|| {
            if !repo_url.is_empty() && repo_url.contains("github.com") {
                // GitHub仓库，用raw.githubusercontent.com
                Some(format!(
                    "{}/raw/main/CHANGELOG.md",
                    repo_url.trim_end_matches('/')
                ))
            } else if !repo_url.is_empty() {
                // 其他仓库，直接拼接CHANGELOG.md
                Some(format!("{}/CHANGELOG.md", repo_url.trim_end_matches('/')))
            } else {
                // 都没有，用默认占位符
                Some("https://raw.githubusercontent.com/user/repo/main/CHANGELOG.md".to_string())
            }
        })
        .unwrap_or_default();

    let mut root = serde_json::Map::new();
    root.insert("version".to_string(), serde_json::Value::String(version));
    root.insert("versionCode".to_string(), serde_json::json!(version_code));
    root.insert("zipUrl".to_string(), serde_json::Value::String(zip_url));
    root.insert(
        "changelog".to_string(),
        serde_json::Value::String(changelog),
    );
    serde_json::Value::Object(root)
}

// 构建repo.json内容（MMRL仓库格式）
// 这个函数也很长，主要是把kam.toml的mmrl.repo字段转成JSON
pub fn build_repo_json(kt: &KamToml) -> serde_json::Value {
    let mut root = serde_json::Map::new();

    // 如果有mmrl配置，就提取相关字段
    if let Some(mmrl) = &kt.mmrl
        && let Some(repo) = &mmrl.repo {
            // license字段
            if let Some(license) = &repo.license
                && !license.is_empty() {
                    root.insert(
                        "license".to_string(),
                        serde_json::Value::String(license.clone()),
                    );
                }

            if let Some(home) = &repo.homepage
                && !home.is_empty() {
                    root.insert(
                        "homepage".to_string(),
                        serde_json::Value::String(home.clone()),
                    );
                }

            if let Some(readme) = &repo.readme
                && !readme.is_empty() {
                    root.insert(
                        "readme".to_string(),
                        serde_json::Value::String(readme.clone()),
                    );
                }

            if let Some(screens) = &repo.screenshots {
                root.insert(
                    "screenshots".to_string(),
                    serde_json::Value::Array(
                        screens
                            .iter()
                            .map(|s| serde_json::Value::String(s.clone()))
                            .collect(),
                    ),
                );
            }

            if let Some(categories) = &repo.categories {
                root.insert(
                    "categories".to_string(),
                    serde_json::Value::Array(
                        categories
                            .iter()
                            .map(|s| serde_json::Value::String(s.clone()))
                            .collect(),
                    ),
                );
            }

            if let Some(devices) = &repo.devices {
                root.insert(
                    "devices".to_string(),
                    serde_json::Value::Array(
                        devices
                            .iter()
                            .map(|s| serde_json::Value::String(s.clone()))
                            .collect(),
                    ),
                );
            }

            if let Some(arch) = &repo.arch {
                root.insert(
                    "arch".to_string(),
                    serde_json::Value::Array(
                        arch.iter()
                            .map(|s| serde_json::Value::String(s.clone()))
                            .collect(),
                    ),
                );
            }

            if let Some(require) = &repo.require {
                root.insert(
                    "require".to_string(),
                    serde_json::Value::Array(
                        require
                            .iter()
                            .map(|s| serde_json::Value::String(s.clone()))
                            .collect(),
                    ),
                );
            }

            if let Some(donate) = &repo.donate
                && !donate.is_empty() {
                    root.insert(
                        "donate".to_string(),
                        serde_json::Value::String(donate.clone()),
                    );
                }

            if let Some(support) = &repo.support
                && !support.is_empty() {
                    root.insert(
                        "support".to_string(),
                        serde_json::Value::String(support.clone()),
                    );
                }

            if let Some(cover) = &repo.cover
                && !cover.is_empty() {
                    root.insert(
                        "cover".to_string(),
                        serde_json::Value::String(cover.clone()),
                    );
                }

            if let Some(icon) = &repo.icon
                && !icon.is_empty() {
                    root.insert("icon".to_string(), serde_json::Value::String(icon.clone()));
                }

            // maintainers字段：支持字符串或对象格式
            if let Some(maintainers) = &repo.maintainers {
                let mut arr: Vec<serde_json::Value> = vec![];
                for maint in maintainers {
                    match maint {
                        crate::types::kam_toml::sections::repo::MaintainerEntry::Name(s) => {
                            // 简单字符串格式，转成对象
                            let mut obj = serde_json::Map::new();
                            obj.insert(
                                "type".to_string(),
                                serde_json::Value::String("add".to_string()),
                            );
                            obj.insert("name".to_string(), serde_json::Value::String(s.clone()));
                            arr.push(serde_json::Value::Object(obj));
                        }
                        crate::types::kam_toml::sections::repo::MaintainerEntry::Object(m) => {
                            // 对象格式，可能有type、name、link等字段
                            let mut obj = serde_json::Map::new();
                            let t = m.r#type.clone().unwrap_or_else(|| "add".to_string());
                            obj.insert("type".to_string(), serde_json::Value::String(t));
                            obj.insert(
                                "name".to_string(),
                                serde_json::Value::String(m.name.clone()),
                            );
                            if let Some(link) = &m.link {
                                obj.insert(
                                    "link".to_string(),
                                    serde_json::Value::String(link.clone()),
                                );
                            }
                            arr.push(serde_json::Value::Object(obj));
                        }
                    }
                }
                root.insert("maintainers".to_string(), serde_json::Value::Array(arr));
            }

            if let Some(note) = &repo.note
                && let Ok(v) = serde_json::to_value(note) {
                    root.insert("note".to_string(), v);
                }

            if let Some(manager) = &repo.manager
                && let Ok(v) = serde_json::to_value(manager) {
                    root.insert("manager".to_string(), v);
                }
        }
    serde_json::Value::Object(root)
}

// 构建track.json内容
// 这个格式可能不太常用，但留着总没错
pub fn build_track_json(kt: &KamToml) -> serde_json::Value {
    let mut root = serde_json::Map::new();

    root.insert(
        "id".to_string(),
        serde_json::Value::String(kt.prop.id.clone()),
    );
    root.insert("enable".to_string(), serde_json::Value::Bool(true)); // 默认启用

    // source字段：优先repository，没有就用homepage，再没有就用updateJson
    let source = kt
        .mmrl
        .as_ref()
        .and_then(|m| m.repo.as_ref())
        .and_then(|r| r.repository.clone())
        .or_else(|| {
            kt.mmrl
                .as_ref()
                .and_then(|m| m.repo.as_ref())
                .and_then(|r| r.homepage.clone())
        })
        .or_else(|| kt.prop.updateJson.clone())
        .unwrap_or_default();
    root.insert("source".to_string(), serde_json::Value::String(source));

    // update_to字段：优先updateJson，没有就根据repository推断，再没有就用版本号
    // 这个逻辑有点复杂，但至少能覆盖大部分情况
    let update_to = if let Some(uj) = &kt.prop.updateJson {
        uj.clone()
    } else if let Some(mmrl) = &kt.mmrl {
        if let Some(repo) = &mmrl.repo {
            if let Some(repo_url) = &repo.repository {
                // 有仓库URL，按GitHub releases格式推断
                format!(
                    "{}/releases/latest/download/{}.zip",
                    repo_url.trim_end_matches('/'),
                    kt.prop.id
                )
            } else {
                kt.prop.version.clone() // 没有URL，用版本号
            }
        } else {
            kt.prop.version.clone()
        }
    } else {
        kt.prop.version.clone()
    };
    root.insert(
        "update_to".to_string(),
        serde_json::Value::String(update_to),
    );

    let repo_json = build_repo_json(kt);
    if let Some(map) = repo_json.as_object() {
        for (k, v) in map {
            root.insert(k.clone(), v.clone());
        }
    }

    root.insert("enable".to_string(), serde_json::Value::Bool(true));

    // features字段：检查各种功能是否启用
    // 这个列表可能不够完整，但至少覆盖了常见的功能
    let mut features_map = serde_json::Map::new();
    let keys = vec![
        "service",
        "post_fs_data",
        "resetprop",
        "sepolicy",
        "zygisk",
        "apks",
        "webroot",
        "post_mount",
        "boot_completed",
        "modconf",
    ];
    let repo_features = kt
        .mmrl
        .as_ref()
        .and_then(|m| m.repo.as_ref())
        .and_then(|r| r.features.clone())
        .unwrap_or_default();
    // 检查每个功能是否在features列表里
    for k in keys {
        let val = repo_features.iter().any(|s| s == k);
        features_map.insert(k.to_string(), serde_json::Value::Bool(val));
    }
    root.insert(
        "features".to_string(),
        serde_json::Value::Object(features_map),
    );

    serde_json::Value::Object(root)
}

// 构建config.json内容
// 这个格式可能也不太常用，但为了完整性还是实现了
pub fn build_config_json(kt: &KamToml) -> serde_json::Value {
    let mut root = serde_json::Map::new();

    // id：优先从repository URL提取，没有就用prop.id
    let id = kt
        .mmrl
        .as_ref()
        .and_then(|m| m.repo.as_ref())
        .and_then(|r| r.repository.as_ref())
        .map(|s| s.split('/').next_back().unwrap_or(s).to_string()) // 取URL最后一部分
        .unwrap_or_else(|| kt.prop.id.clone());
    root.insert("id".to_string(), serde_json::Value::String(id));

    root.insert(
        "name".to_string(),
        serde_json::Value::String(kt.prop.name.clone()),
    );

    // base_url：优先homepage，没有就用repository
    // 确保末尾有斜杠（虽然可能不太重要）
    let base_url = kt
        .mmrl
        .as_ref()
        .and_then(|m| m.repo.as_ref())
        .and_then(|r| r.homepage.clone())
        .or_else(|| {
            kt.mmrl
                .as_ref()
                .and_then(|m| m.repo.as_ref().and_then(|r| r.repository.clone()))
        })
        .unwrap_or_default();
    let mut base_url = base_url.clone();
    if !base_url.is_empty() && !base_url.ends_with('/') {
        base_url.push('/'); // 确保末尾有斜杠
    }
    root.insert("base_url".to_string(), serde_json::Value::String(base_url));

    if let Some(mmrl) = &kt.mmrl
        && let Some(repo) = &mmrl.repo {
            if let Some(website) = &repo.homepage
                && !website.is_empty() {
                    root.insert(
                        "website".to_string(),
                        serde_json::Value::String(website.clone()),
                    );
                }
            if let Some(support) = &repo.support
                && !support.is_empty() {
                    root.insert(
                        "support".to_string(),
                        serde_json::Value::String(support.clone()),
                    );
                }
            if let Some(donate) = &repo.donate
                && !donate.is_empty() {
                    root.insert(
                        "donate".to_string(),
                        serde_json::Value::String(donate.clone()),
                    );
                }
            if let Some(sub) = &repo.documentation
                && !sub.is_empty() {
                    root.insert(
                        "submission".to_string(),
                        serde_json::Value::String(sub.clone()),
                    );
                }
        }

    if !kt.prop.description.trim().is_empty() {
        root.insert(
            "description".to_string(),
            serde_json::Value::String(kt.prop.description.clone()),
        );
    }

    if let Some(mmrl) = &kt.mmrl
        && let Some(repo) = &mmrl.repo
            && let Some(max) = repo.max_num {
                root.insert("max_num".to_string(), serde_json::Value::Number(max.into()));
            }

    root.insert("enable_log".to_string(), serde_json::Value::Bool(true));
    root.insert(
        "log_dir".to_string(),
        serde_json::Value::String("log".to_string()),
    );

    serde_json::Value::Object(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prop_basic() {
        let kt = KamToml::new_with_current_timestamp(
            "example_module_id".to_string(),
            "Example Module Name".to_string(),
            "1.0.0".to_string(),
            Some("Your Name".to_string()),
            "A description".to_string(),
            None,
            None,
        );
        let s = build_prop(&kt);
        assert!(s.contains("id=example_module_id"));
    }
}
