/*
Kam/src/cmds/about/handler.rs
Use comfy_table for rendering About output, dynamic values, and remove any hardcoded ASCII art.
*/

use comfy_table::{Attribute, Cell, ContentArrangement, Row, Table, presets::UTF8_FULL};
use regex::Regex;

use crate::errors::KamError;
use crate::types::kam_toml::KamToml;
use crate::utils::Utils;

use super::args::AboutArgs;

// 从字符串里提取邮箱地址
// 优先找 <email@domain> 这种格式，没有就用正则匹配
fn extract_email(text: &str) -> Option<String> {
    // 先找 <...> 格式的
    if let Some(start) = text.find('<')
        && let Some(end_rel) = text[start..].find('>')
    {
        let candidate = text[start + 1..start + end_rel].trim();
        if !candidate.is_empty() {
            return Some(candidate.to_string());
        }
    }

    // 回退方案：用简单的邮箱正则匹配
    // 这个正则可能不够完善，但大部分情况够用了
    if let Ok(re) = Regex::new(r"([a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+)")
        && let Some(cap) = re.captures(text)
    {
        return Some(cap[1].to_string());
    }

    None
}

// 从候选列表里选第一个非空的值，都没有就用fallback
// 这个函数主要是为了处理各种可能的来源（kam.toml、Cargo metadata等）
fn pick_first<'a>(candidates: Vec<Option<&'a str>>, fallback: &'a str) -> String {
    for v in candidates.into_iter().flatten() {
        let vtrim = v.trim();
        if !vtrim.is_empty() {
            return vtrim.to_string();
        }
    }
    fallback.to_string()
}

// 执行 kam about 命令
// 就是显示一些项目信息，用表格展示，看起来比较好看
pub fn run(_args: AboutArgs) -> Result<(), KamError> {
    // 优先用当前目录的kam.toml，没有就用Cargo的metadata
    let cwd = std::env::current_dir().map_err(KamError::Io)?;
    let kam_toml = KamToml::load_from_dir(&cwd).ok();

    // 优先级：kam.toml > Cargo metadata > 默认值
    let name = pick_first(
        vec![
            kam_toml.as_ref().map(|k| k.prop.name.as_str()),
            option_env!("CARGO_PKG_NAME"),
        ],
        "kam",
    );

    let version = pick_first(
        vec![
            kam_toml.as_ref().map(|k| k.prop.version.as_str()),
            option_env!("CARGO_PKG_VERSION"),
        ],
        "unknown",
    );

    // Normalize display version to avoid double leading 'v' (e.g., avoid 'vv1.2.3')
    let display_version = if version.to_lowercase().starts_with('v') {
        version.clone()
    } else {
        format!("v{}", version)
    };

    let description = pick_first(
        vec![
            kam_toml.as_ref().map(|k| k.prop.description.as_str()),
            option_env!("CARGO_PKG_DESCRIPTION"),
        ],
        "",
    );

    // Author: prefer kam.toml prop.author, otherwise CARGO_PKG_AUTHORS
    let author_raw = pick_first(
        vec![
            kam_toml.as_ref().and_then(|k| k.prop.author.as_deref()),
            option_env!("CARGO_PKG_AUTHORS"),
        ],
        "LIghJUNction",
    );

    // 邮箱的优先级：从author字符串提取 > 环境变量 KAM_AUTHOR_EMAIL > Cargo metadata > 没有
    let mut email = extract_email(&author_raw);
    if email.is_none()
        && let Ok(val) = std::env::var("KAM_AUTHOR_EMAIL")
        && !val.trim().is_empty()
    {
        email = Some(val);
    }
    if email.is_none()
        && let Some(pkg_authors) = option_env!("CARGO_PKG_AUTHORS")
    {
        email = extract_email(pkg_authors);
    }

    // 开发者网站链接
    let dev_site = "https://developers.kernelsu.org/";

    // 用comfy_table做个好看的表格
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_content_arrangement(ContentArrangement::Dynamic);

    // 表头：名字和版本，加粗和颜色
    table.set_header(vec![
        Cell::new(format!("{} v{}", name, version))
            .add_attribute(Attribute::Bold)
            .fg(comfy_table::Color::Cyan),
        Cell::new(""),
    ]);

    // Rows:
    table.add_row(Row::from(vec![
        Cell::new(crate::i18n::tr_key("about.author")).add_attribute(Attribute::Bold),
        Cell::new(author_raw.clone()),
    ]));

    if let Some(email) = email {
        table.add_row(Row::from(vec![
            Cell::new(crate::i18n::tr_key("about.email")).add_attribute(Attribute::Bold),
            Cell::new(email),
        ]));
    }

    table.add_row(Row::from(vec![
        Cell::new(crate::i18n::tr_key("about.developer")).add_attribute(Attribute::Bold),
        Cell::new(dev_site),
    ]));

    if !description.is_empty() {
        table.add_row(Row::from(vec![
            Cell::new(crate::i18n::tr_key("about.description")).add_attribute(Attribute::Bold),
            Cell::new(description),
        ]));
    }

    // 如果有仓库信息（从Cargo metadata）也显示出来
    if let Some(repo) = option_env!("CARGO_PKG_REPOSITORY")
        && !repo.trim().is_empty()
    {
        table.add_row(Row::from(vec![
            Cell::new(crate::i18n::tr_key("about.repository")).add_attribute(Attribute::Bold),
            Cell::new(repo),
        ]));
    }

    // 打印banner和表格，看起来比较专业（虽然其实没啥用）
    Utils::banner(&format!("{} {}", name, display_version));
    println!("{}", table);
    println!(); // 空行，视觉上舒服点

    Utils::info(crate::i18n::tr_key("about.info.command_informational"));
    Utils::info(crate::i18n::tr_key("about.info.use_other_commands"));
    println!();

    Utils::section(crate::i18n::tr_key("about.thanks"));
    Utils::info(crate::i18n::tr_key("about.enjoy"));
    Utils::success(crate::i18n::tr_key("about.powered")); // 这句有点中二，但留着吧
    // 其实这个命令没啥用，就是显示个信息，但至少看起来比较专业（？）

    Ok(())
}
