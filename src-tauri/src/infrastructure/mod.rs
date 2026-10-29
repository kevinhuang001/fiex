//! Cross-cutting infrastructure. Business adapters do not own their caches,
//! persisted UI state, diagnostic logs, or notification presentation.
pub mod cache;
pub mod errors;
pub mod logging;
pub mod state;
