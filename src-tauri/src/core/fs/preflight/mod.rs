//! Shared checks that add dynamic restrictions to resource capabilities.
mod availability;
mod operations;

pub(crate) use availability::clear_availability_cache;
