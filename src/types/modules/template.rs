use crate::types::modules::KamModule;

/// Template module helpers (stub).
pub struct TemplateModule {
    pub inner: KamModule,
}

impl TemplateModule {
    pub fn from_module(m: KamModule) -> Self { Self { inner: m } }
}
