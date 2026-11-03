//! Provider-agnostic cache engines. Resource authorization belongs to service decorators.
pub mod disk;
pub(crate) mod lru;

pub mod memory;
