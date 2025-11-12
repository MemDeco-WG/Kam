use crate::types::modules::KamModule;

pub struct RepoModule {
    pub inner: KamModule,
}

impl RepoModule {
    pub fn from_module(m: KamModule) -> Self {
        Self { inner: m }
    }
}
