//! Legacy module name retained only as a facade for existing call sites.
//! All records and storage are owned by the independent infrastructure service.
pub use crate::infrastructure::logging::{
    LogRecord as PluginError, report, report_limited, report_log_limited, list, clear,
};
