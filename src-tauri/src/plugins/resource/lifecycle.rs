//! Release all host handles owned by an operation or task.
pub(crate) fn release(owner: &str) {
    super::fs_access::cleanup(owner);
    crate::plugins::ipc::cleanup(owner);
    crate::plugins::net::cleanup(owner);
}
