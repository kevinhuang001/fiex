//! Shared registration and candidate selection for native provider modules.
//!
//! Provider implementations stay behind typed handles. The registry stores
//! their trait-object Arcs without depending on the desktop application,
//! Plugin Host, or any built-in module.

mod registry;

pub use registry::{
    global_runtime_registry, ModuleDescriptor, ModuleRegistrar, ProviderCandidate, RuntimeRegistry,
    TypedProviderRegistry,
};
