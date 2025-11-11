use crate::types::modules::KamModule;

pub struct LibraryModule {
    pub inner: KamModule,
}

impl LibraryModule {
    pub fn from_module(m: KamModule) -> Self { Self { inner: m } }
}
