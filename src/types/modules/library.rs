use crate::types::modules::KamModule;

pub struct LibraryModule {
    pub inner: KamModule,
}

impl_from_module!(LibraryModule);
