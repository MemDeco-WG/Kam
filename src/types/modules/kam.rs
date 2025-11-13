use crate::types::modules::KamModule;

/// Kam-specific module behaviors. For now this is a thin wrapper around the
/// shared `KamModule` implementation. Later we can add build-specific helpers
/// here.

pub struct KamSpecific {
    pub inner: KamModule,
}

impl_from_module!(KamSpecific);
