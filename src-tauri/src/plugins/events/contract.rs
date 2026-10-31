//! Core and plugins use the same SDK-owned event method / dispatch registry.
//! Keep no local copy of the method table: new Provider types are declared once.
pub(super) use crate::plugin_sdk::provider_contracts::{event, Dispatch};
