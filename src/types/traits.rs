use crate::types::kam_toml::KamToml;

/// Trait for types that can be converted from and to KamToml
pub trait KamConvertible<'a> {
    /// Create an instance from KamToml
    fn from_kam(kam: &'a KamToml) -> Self;

    /// Convert to KamToml
    fn to_kam(&self) -> KamToml;
}
