//! Plugin commands: host invocation, status, panels, permissions, storage,
//! marketplace installation, decorations, themes, errors, search sources and
//! the host-owned native file dialogs.

use crate::core::files;

#[macro_use]
mod invoke;
pub(crate) use invoke::*;

#[macro_use]
mod installation;
pub(crate) use installation::*;

#[macro_use]
mod queries;
pub(crate) use queries::*;

#[macro_use]
mod storage;
pub(crate) use storage::*;

#[macro_use]
mod resources;
pub(crate) use resources::*;

#[macro_use]
mod contributions;
pub(crate) use contributions::*;
