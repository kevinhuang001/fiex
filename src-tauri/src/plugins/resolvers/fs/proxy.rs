//! Core's persistent IPC implementation of the shared VFS provider contract.
//! Mount identity, credentials and scopes stay in the per-call invocation.
use super::Mount;
use crate::plugins::native_invocation::NativeProviderInvocation;
use fiex_provider_contracts::{
    FsOperation, FsRequest, ProviderDescriptor, ProviderError, ProviderInstances,
    ProviderInvocation, ProviderKind, ProviderMatchRule, ProviderRequestContext,
    SearchCapabilities, TransferPreflight, VfsPage, VfsProvider,
};
use serde_json::{json, Value};
use std::{
    any::Any,
    sync::{Arc, Mutex, OnceLock},
};

pub(super) struct MountInvocation {
    base: NativeProviderInvocation,
    mount: Mount,
}

impl MountInvocation {
    pub fn new(mount: Mount, method: &str, params: Value) -> Self {
        Self {
            base: NativeProviderInvocation::new(method, params, &[], None),
            mount,
        }
    }

    fn mount(ctx: &dyn ProviderInvocation) -> Result<&Mount, ProviderError> {
        ctx.local_service_handle()
            .and_then(|value| value.downcast_ref::<Self>())
            .map(|invocation| &invocation.mount)
            .ok_or_else(|| ProviderError::internal("missing Core mount context"))
    }
}

impl ProviderInvocation for MountInvocation {
    fn request(&self) -> &ProviderRequestContext {
        self.base.request()
    }

    fn local_service_handle(&self) -> Option<&dyn Any> {
        Some(self)
    }
}

struct IpcVfsProvider {
    plugin_id: String,
}

impl IpcVfsProvider {
    fn mount<'a>(&self, ctx: &'a dyn ProviderInvocation) -> Result<&'a Mount, ProviderError> {
        let mount = MountInvocation::mount(ctx)?;
        if mount.plugin_id != self.plugin_id {
            return Err(ProviderError::invalid_params("VfsProvider owner mismatch"));
        }
        Ok(mount)
    }

    fn invoke(
        &self,
        ctx: &dyn ProviderInvocation,
        method: &str,
        params: Value,
    ) -> Result<Value, ProviderError> {
        if ctx.method() != method {
            return Err(ProviderError::invalid_params("VfsProvider method mismatch"));
        }
        self.mount(ctx)?
            .invoke_remote(method, params)
            .map_err(ProviderError::internal)
    }
}

impl VfsProvider for IpcVfsProvider {
    fn supports_preflight(&self) -> bool {
        true
    }

    fn preflight(
        &self,
        ctx: &dyn ProviderInvocation,
        request: &TransferPreflight,
    ) -> Result<fiex_provider_contracts::PreflightDecision, ProviderError> {
        let result = self.invoke(
            ctx,
            "vfs.preflight",
            serde_json::to_value(request)
                .map_err(|error| ProviderError::invalid_params(error.to_string()))?,
        )?;
        serde_json::from_value(result)
            .map_err(|error| ProviderError::invalid_params(error.to_string()))
    }

    fn operations(&self) -> &'static [FsOperation] {
        &FsOperation::ALL
    }

    fn search_capabilities(&self, ctx: &dyn ProviderInvocation) -> SearchCapabilities {
        let supports_search = self
            .mount(ctx)
            .is_ok_and(|mount| mount.supports(FsOperation::Search));
        SearchCapabilities {
            name: supports_search,
            content: false,
        }
    }

    fn discover(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<FsOperation>, ProviderError> {
        Ok(self.mount(ctx)?.operations().unwrap_or_default())
    }

    fn list(
        &self,
        request: &FsRequest<'_>,
    ) -> Result<fiex_provider_contracts::FsListing, ProviderError> {
        let result = self.invoke(request.ctx(), "vfs.list", request.to_params()?)?;
        let limit = request.limit();
        let page: fiex_provider_contracts::VfsPage = serde_json::from_value(result.clone())
            .map_err(|error| ProviderError::invalid_params(format!("invalid VFS page: {error}")))?;
        page.validate_page(limit)
            .map_err(ProviderError::invalid_params)?;
        serde_json::from_value(result)
            .map_err(|error| ProviderError::invalid_params(error.to_string()))
    }

    fn stat(
        &self,
        request: &FsRequest<'_>,
    ) -> Result<fiex_provider_contracts::FsEntry, ProviderError> {
        let result = self.invoke(request.ctx(), "vfs.stat", request.to_params()?)?;
        serde_json::from_value(result)
            .map_err(|error| ProviderError::invalid_params(error.to_string()))
    }

    fn read(
        &self,
        request: &FsRequest<'_>,
    ) -> Result<fiex_provider_contracts::FsRead, ProviderError> {
        let result = self.invoke(request.ctx(), "vfs.openRead", request.to_params()?)?;
        let stream_id = result["streamId"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| ProviderError::invalid_params("provider did not return a stream id"))?
            .to_owned();
        let mount = self.mount(request.ctx())?.clone();
        let size = result.get("size").and_then(Value::as_u64);
        Ok(fiex_provider_contracts::FsRead::new(
            RemoteReader {
                mount,
                stream_id,
                offset: 0,
                eof: false,
                closed: false,
            },
            size,
        ))
    }

    fn write(
        &self,
        request: &FsRequest<'_>,
    ) -> Result<fiex_provider_contracts::FsWrite, ProviderError> {
        let result = self.invoke(request.ctx(), "vfs.beginWrite", request.to_params()?)?;
        let stream_id = result["streamId"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                ProviderError::invalid_params("provider did not return a write stream id")
            })?
            .to_owned();
        let mount = self.mount(request.ctx())?.clone();
        let finished = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let writer = RemoteWriter {
            mount: mount.clone(),
            stream_id: stream_id.clone(),
            offset: 0,
            finished: finished.clone(),
        };
        Ok(fiex_provider_contracts::FsWrite::with_commit(
            writer,
            move || {
                let result = mount
                    .invoke_remote("vfs.commit", json!({"streamId":stream_id}))
                    .map_err(ProviderError::internal)?;
                finished.store(true, std::sync::atomic::Ordering::Release);
                if result.is_null() || result.as_object().is_some_and(|object| object.is_empty()) {
                    return Ok(fiex_provider_contracts::FsEntry::default());
                }
                serde_json::from_value(result)
                    .map_err(|error| ProviderError::invalid_params(error.to_string()))
            },
        ))
    }

    fn mkdir(
        &self,
        request: &FsRequest<'_>,
    ) -> Result<fiex_provider_contracts::FsEntry, ProviderError> {
        let result = self.invoke(request.ctx(), "vfs.mkdir", request.to_params()?)?;
        serde_json::from_value(result)
            .map_err(|error| ProviderError::invalid_params(error.to_string()))
    }

    fn rename(
        &self,
        request: &FsRequest<'_>,
    ) -> Result<fiex_provider_contracts::FsEntry, ProviderError> {
        let result = self.invoke(request.ctx(), "vfs.rename", request.to_params()?)?;
        serde_json::from_value(result)
            .map_err(|error| ProviderError::invalid_params(error.to_string()))
    }

    fn remove(&self, request: &FsRequest<'_>) -> Result<(), ProviderError> {
        self.invoke(request.ctx(), "vfs.remove", request.to_params()?)?;
        Ok(())
    }

    fn search(
        &self,
        request: &FsRequest<'_>,
    ) -> Result<fiex_provider_contracts::FsListing, ProviderError> {
        let result = self.invoke(request.ctx(), "vfs.search", request.to_params()?)?;
        let page: VfsPage = serde_json::from_value(result)
            .map_err(|error| ProviderError::invalid_params(error.to_string()))?;
        page.validate_page(request.limit())
            .map_err(ProviderError::invalid_params)?;
        let entries = page
            .entries
            .into_iter()
            .map(serde_json::from_value)
            .collect::<Result<Vec<fiex_provider_contracts::FsEntry>, _>>()
            .map_err(|error| ProviderError::invalid_params(error.to_string()))?;
        Ok(fiex_provider_contracts::FsListing {
            entries,
            next_cursor: page.next_cursor,
            truncated: page.truncated,
            ..Default::default()
        })
    }
}

fn registry() -> &'static Mutex<ProviderInstances<dyn VfsProvider>> {
    static REGISTERED: OnceLock<Mutex<ProviderInstances<dyn VfsProvider>>> = OnceLock::new();
    REGISTERED.get_or_init(|| Mutex::new(ProviderInstances::new(ProviderKind::Vfs)))
}

pub(super) fn forget_plugin(plugin_id: &str) {
    if let Ok(mut providers) = registry().lock() {
        providers.unregister(&format!("plugin:{plugin_id}"));
    }
}

pub(super) fn provider(plugin_id: &str) -> Result<Arc<dyn VfsProvider>, String> {
    if plugin_id.is_empty() {
        return Err("missing VfsProvider plugin identity".into());
    }
    let mut registry = registry()
        .lock()
        .map_err(|_| "VfsProvider registry unavailable".to_string())?;
    let id = format!("plugin:{plugin_id}");
    if let Some(provider) = registry.get_arc(&id) {
        return Ok(provider);
    }
    registry.register(
        ProviderDescriptor {
            id: id.clone(),
            kind: ProviderKind::Vfs,
            priority: 50,
            r#match: ProviderMatchRule::default(),
        },
        Arc::new(IpcVfsProvider {
            plugin_id: plugin_id.into(),
        }),
    )?;
    registry
        .get_arc(&id)
        .ok_or_else(|| "VfsProvider registration failed".into())
}

pub(super) fn open_read(
    mount: &Mount,
    params: Value,
) -> Result<fiex_provider_contracts::FsRead, String> {
    let provider = provider(&mount.plugin_id)?;
    let invocation = MountInvocation::new(mount.clone(), "vfs.openRead", params);
    let request =
        FsRequest::new(&invocation, FsOperation::Read).map_err(|error| error.to_string())?;
    provider.read(&request).map_err(|error| error.to_string())
}

pub(super) fn begin_write(
    mount: &Mount,
    params: Value,
) -> Result<fiex_provider_contracts::FsWrite, String> {
    let provider = provider(&mount.plugin_id)?;
    let invocation = MountInvocation::new(mount.clone(), "vfs.beginWrite", params);
    let request =
        FsRequest::new(&invocation, FsOperation::Write).map_err(|error| error.to_string())?;
    provider.write(&request).map_err(|error| error.to_string())
}

pub(super) fn invoke(mount: &Mount, method: &str, params: Value) -> Result<Value, String> {
    let provider = provider(&mount.plugin_id)?;
    let invocation = MountInvocation::new(mount.clone(), method, params.clone());
    let result = match method {
        "vfs.list" => {
            let request = FsRequest::new(&invocation, FsOperation::List)
                .map_err(|error| error.to_string())?;
            serde_json::to_value(provider.list(&request).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())
        }
        "vfs.stat" => {
            let request = FsRequest::new(&invocation, FsOperation::Stat)
                .map_err(|error| error.to_string())?;
            serde_json::to_value(provider.stat(&request).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())
        }
        "vfs.mkdir" => {
            let request = FsRequest::new(&invocation, FsOperation::Mkdir)
                .map_err(|error| error.to_string())?;
            serde_json::to_value(
                provider
                    .mkdir(&request)
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())
        }
        "vfs.rename" => {
            let request = FsRequest::new(&invocation, FsOperation::Rename)
                .map_err(|error| error.to_string())?;
            serde_json::to_value(
                provider
                    .rename(&request)
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())
        }
        "vfs.remove" => {
            let request = FsRequest::new(&invocation, FsOperation::Remove)
                .map_err(|error| error.to_string())?;
            provider
                .remove(&request)
                .map_err(|error| error.to_string())?;
            Ok(Value::Null)
        }
        "vfs.search" => {
            let request = FsRequest::new(&invocation, FsOperation::Search)
                .map_err(|error| error.to_string())?;
            serde_json::to_value(
                provider
                    .search(&request)
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())
        }
        "vfs.preflight" => {
            let request: TransferPreflight = serde_json::from_value(params)
                .map_err(|error| format!("invalid preflight request: {error}"))?;
            serde_json::to_value(
                provider
                    .preflight(&invocation, &request)
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())
        }
        "vfs.discover" => {
            let operations = provider
                .discover(&invocation)
                .map_err(|error| error.to_string())?;
            let search = provider.search_capabilities(&invocation);
            Ok(json!({"operations":operations,"search":search}))
        }
        // These wire methods carry transport stream IDs. The shared VFS trait
        // turns open/begin operations into Rust Read/Write streams below; the
        // chunk protocol is retained only as the proxy's IPC transport.
        "vfs.openRead" | "vfs.beginWrite" | "vfs.readStream" | "vfs.closeRead"
        | "vfs.writeStream" | "vfs.commit" | "vfs.abort" => mount.invoke_remote(method, params),
        _ => Err("unsupported VfsProvider method".into()),
    };
    result
}

struct RemoteReader {
    mount: Mount,
    stream_id: String,
    offset: u64,
    eof: bool,
    closed: bool,
}

impl std::io::Read for RemoteReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        if buffer.is_empty() || self.eof {
            return Ok(0);
        }
        let limit = buffer.len().min(4 * 1024 * 1024);
        let reply = self
            .mount
            .invoke_remote(
                "vfs.readStream",
                json!({"streamId":self.stream_id,"offset":self.offset,"maxBytes":limit}),
            )
            .map_err(std::io::Error::other)?;
        let encoded = reply["base64"]
            .as_str()
            .ok_or_else(|| std::io::Error::other("provider stream reply is missing data"))?;
        if encoded.len() > limit.div_ceil(3) * 4 {
            return Err(std::io::Error::other(
                "provider exceeded the requested chunk size",
            ));
        }
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|_| std::io::Error::other("provider returned invalid base64 data"))?;
        if bytes.len() > limit {
            return Err(std::io::Error::other(
                "provider exceeded the requested chunk size",
            ));
        }
        let count = bytes.len();
        buffer[..count].copy_from_slice(&bytes);
        self.offset = self.offset.saturating_add(count as u64);
        self.eof = reply["eof"].as_bool().unwrap_or(false) || count == 0;
        Ok(count)
    }
}

impl Drop for RemoteReader {
    fn drop(&mut self) {
        if !self.closed {
            let _ = self
                .mount
                .invoke_remote("vfs.closeRead", json!({"streamId":self.stream_id}));
            self.closed = true;
        }
    }
}

struct RemoteWriter {
    mount: Mount,
    stream_id: String,
    offset: u64,
    finished: Arc<std::sync::atomic::AtomicBool>,
}

impl std::io::Write for RemoteWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        if bytes.is_empty() {
            return Ok(0);
        }
        let chunk = &bytes[..bytes.len().min(4 * 1024 * 1024)];
        let reply = self
            .mount
            .invoke_remote(
                "vfs.writeStream",
                json!({"streamId":self.stream_id,"offset":self.offset,"base64":STANDARD.encode(chunk)}),
            )
            .map_err(std::io::Error::other)?;
        let written = reply["written"].as_u64().ok_or_else(|| {
            std::io::Error::other("provider write reply is missing written count")
        })?;
        if written == 0 || written > chunk.len() as u64 {
            return Err(std::io::Error::other(
                "provider returned an invalid written count",
            ));
        }
        self.offset = self
            .offset
            .checked_add(written)
            .ok_or_else(|| std::io::Error::other("write offset overflow"))?;
        Ok(written as usize)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for RemoteWriter {
    fn drop(&mut self) {
        if !self.finished.load(std::sync::atomic::Ordering::Acquire) {
            let _ = self
                .mount
                .invoke_remote("vfs.abort", json!({"streamId":self.stream_id}));
            self.finished
                .store(true, std::sync::atomic::Ordering::Release);
        }
    }
}
