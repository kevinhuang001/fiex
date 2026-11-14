//! External VFS provider fixture used by the Core process-restart smoke test.
use fiex_plugin_sdk as sdk;
use sdk::RpcError;
use sdk::filesystem::{FsEntry, FsListing, FsOperation, FsRequest, VfsProvider};

struct MockVfs;

impl VfsProvider for MockVfs {
    fn operations(&self) -> &'static [FsOperation] {
        &[FsOperation::List, FsOperation::Stat]
    }

    fn list(&self, request: &FsRequest<'_>) -> Result<FsListing, RpcError> {
        if !request.node_id_or_root().is_empty() {
            return Err(RpcError::invalid_params("unknown fixture directory"));
        }
        Ok(FsListing::new([fixture_entry()]))
    }

    fn stat(&self, request: &FsRequest<'_>) -> Result<FsEntry, RpcError> {
        if request.node_id() != Some("fixture-file") {
            return Err(RpcError::invalid_params("unknown fixture node"));
        }
        Ok(fixture_entry())
    }
}

fn fixture_entry() -> FsEntry {
    FsEntry::file("fixture-file", "fixture.txt")
        .with_size(7)
        .allowing([FsOperation::Stat])
}

fn main() {
    sdk::Providers::new()
        .filesystem(MockVfs)
        .serve_with("org.fiex.mock-vfs", |_ctx| {
            Err(RpcError::method_not_found("mock-vfs method"))
        })
        .expect("mock VFS provider IPC failed");
}
