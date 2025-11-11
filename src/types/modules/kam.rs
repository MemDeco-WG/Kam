use crate::types::modules::KamModule;

/// Kam-specific module behaviors. For now this is a thin wrapper around the
/// shared `KamModule` implementation. Later we can add build-specific helpers
/// here.

pub struct KamSpecific {
    pub inner: KamModule,
}

impl KamSpecific {
    pub fn from_module(m: KamModule) -> Self { Self { inner: m } }
}
