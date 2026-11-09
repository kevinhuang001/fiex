use super::{Mount, MountInvocation, provider};
use fiex_provider_contracts::{FsOperation, FsRequest};
use serde_json::Value;

pub fn open_read(mount: &Mount, params: Value) -> Result<fiex_provider_contracts::FsRead, String> {
    let provider = provider(&mount.plugin_id)?;
    let invocation = MountInvocation::new(mount.clone(), "vfs.openRead", params);
    let request =
        FsRequest::new(&invocation, FsOperation::Read).map_err(|error| error.to_string())?;
    provider.read(&request).map_err(|error| error.to_string())
}

pub fn begin_write(
    mount: &Mount,
    params: Value,
) -> Result<fiex_provider_contracts::FsWrite, String> {
    let provider = provider(&mount.plugin_id)?;
    let invocation = MountInvocation::new(mount.clone(), "vfs.beginWrite", params);
    let request =
        FsRequest::new(&invocation, FsOperation::Write).map_err(|error| error.to_string())?;
    provider.write(&request).map_err(|error| error.to_string())
}
