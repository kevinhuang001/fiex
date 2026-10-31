//! Core and plugins use the same SDK-owned event method / dispatch registry.
//! Keep no local copy of the method table: new Provider types are declared once.
pub(super) use fiex_provider_contracts::{event, Dispatch};
