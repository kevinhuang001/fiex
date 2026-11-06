//! Typed provider registry facade over shared runtime storage.
mod module_registrar;
mod runtime;
mod tests;
mod typed_registry;
mod types;

pub use module_registrar::ModuleRegistrar;
pub use runtime::{global_runtime_registry, RuntimeRegistry};
pub use typed_registry::TypedProviderRegistry;
pub use types::{ModuleDescriptor, ProviderCandidate};
