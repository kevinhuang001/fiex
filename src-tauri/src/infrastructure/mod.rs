//! Cross-cutting infrastructure. Business adapters do not own their caches,
//! persisted UI state, diagnostic logs, or notification presentation.
pub mod logging;
pub mod errors;
pub mod state;
pub mod cache;
