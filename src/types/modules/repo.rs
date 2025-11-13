use crate::types::modules::KamModule;

pub struct RepoModule {
    pub inner: KamModule,
}

impl_from_module!(RepoModule);
