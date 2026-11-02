//! Private user-app storage facade. Path validation, listing, and mutations
//! stay separate so each boundary has one owner.
mod listing;
mod mutations;
mod path;

#[cfg(test)]
pub(in crate::core::volumes) use listing::folder_listing;
pub(in crate::core::volumes::applications) use listing::list_user;
pub use listing::{list_at, stored_entry};
pub use mutations::{after_rename, create_stored, remove_user, rename_stored};
#[cfg(test)]
pub(in crate::core::volumes) use path::user_encoded;
pub use path::{candidate_path, is_user, stored_path, stored_uri, validate_child};
