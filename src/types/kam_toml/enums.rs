

use serde::{Serialize, Deserialize, Serializer, Deserializer};
use serde::de::{self, Visitor};
use std::fmt;

/// 支持的 CPU 架构枚举（序列化为字符串，例如 "arm", "arm64", "x86_64"）
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum SupportedArch {
    Arm,
    Arm64,
    X86,
    X86_64,
    Other(String),
}

impl Serialize for SupportedArch {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            SupportedArch::Arm => serializer.serialize_str("arm"),
            SupportedArch::Arm64 => serializer.serialize_str("arm64"),
            SupportedArch::X86 => serializer.serialize_str("x86"),
            SupportedArch::X86_64 => serializer.serialize_str("x86_64"),
            SupportedArch::Other(s) => serializer.serialize_str(s),
        }
    }
}

impl<'de> Deserialize<'de> for SupportedArch {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ArchVisitor;

        impl<'de> Visitor<'de> for ArchVisitor {
            type Value = SupportedArch;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a CPU architecture string")
            }

            fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
            where
                E: de::Error,
            {
                let key = v.trim();
                let key_lc = key.to_ascii_lowercase();
                Ok(match key_lc.as_str() {
                    // ARM family aliases
                    "arm" | "armv7" | "armv7l" | "armv6" | "armhf" => SupportedArch::Arm,
                    // ARM64 / AArch64
                    "arm64" | "aarch64" => SupportedArch::Arm64,
                    // 32-bit x86 aliases
                    "x86" | "i386" | "i486" | "i586" | "i686" => SupportedArch::X86,
                    // 64-bit x86 aliases
                    "x86_64" | "x64" | "amd64" => SupportedArch::X86_64,
                    other => SupportedArch::Other(other.to_string()),
                })
            }
        }

        deserializer.deserialize_str(ArchVisitor)
    }
}

impl fmt::Display for SupportedArch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            SupportedArch::Arm => "arm",
            SupportedArch::Arm64 => "arm64",
            SupportedArch::X86 => "x86",
            SupportedArch::X86_64 => "x86_64",
            SupportedArch::Other(s) => return write!(f, "{}", s),
        };
        write!(f, "{}", s)
    }
}

// Allow comparing SupportedArch and String bidirectionally so existing code
// that works with `String` (e.g. `Vec<String>::contains`) keeps working.
impl PartialEq<String> for SupportedArch {
    fn eq(&self, other: &String) -> bool {
        self.to_string() == *other
    }
}

impl PartialEq<SupportedArch> for String {
    fn eq(&self, other: &SupportedArch) -> bool {
        *self == other.to_string()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
#[serde(rename_all = "lowercase")]
/// 模块类型（序列化为字符串，用于 `kam.toml` 中的 `module_type` 字段）
///
/// - `kam`：代表一个可发布的 Kam 模块
/// - `template`：代表一个模板（用于生成其他模块）
/// - `library`：代表一个仅作为库使用的模块
/// - `repo`：代表一个模块仓库
pub enum ModuleType {
    Kam,
    Template,
    Library,
    Repo,
}
