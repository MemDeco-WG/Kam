include!("template_init_impl/render_support.rs");
use crate::cmds::base_manifest::{
    managed_base_excludes_from_template, materialize_workflow_bases,
    restore_project_bases as restore_kam_bases,
};
include!("template_init_impl/entrypoints.rs");
