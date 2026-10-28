//! Native file operations and local application state.
pub mod drag_session;
pub mod files;
pub mod ignores;
pub mod operations;
pub mod recent;
pub mod search;
#[cfg(any(feature = "desktop", test))]
pub(crate) mod shells;
pub mod tags;
pub mod terminals;
pub mod transfers;
pub mod volumes;
