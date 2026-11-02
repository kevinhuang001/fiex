# Code structure

Fiex has one React frontend and one Rust library. Tauri commands form the boundary between them; command names and serialized payloads do not depend on internal Rust module paths.

## Frontend

- `src/components/` contains the explorer, settings, dialogs, plugin surfaces, and presentation components.
- `src/hooks/` owns application state, explorer navigation, file operations, and desktop integration.
- `src/lib/` contains typed frontend APIs, shared file models, dialogs, and demo fixtures.
- `src-tauri/` owns native commands, platform integrations, Core services, provider routing, and the plugin host.

## Native library

| Directory | Responsibility |
| --- | --- |
| `core/fs/` | Scoped file broker, provider routing, streaming, export, caching, and transfer orchestration. |
| `core/volumes/native_vfs/` | Native `VfsProvider` implementations and Core resource projections. |
| `core/volumes/applications/` | Application catalog, per-platform adapters, user store, and folder mappings. |
| `core/search/` | Search orchestration and local/provider search sources. |
| `desktop/` | Tauri command adapters, window lifecycle, startup, and scheduling. |
| `plugins/provider_registry.rs` and `plugins/provider_registry/` | Typed native/plugin registration, matcher state, and provider instance lifecycle. |
| `plugins/events/` | Shared event routing, scope preparation, validation, and collection policy. |
| `plugins/providers/` | Feature provider implementations, native providers, and plugin IPC proxies. |
| `plugins/resource/` | Resource references, scope authorization, capability projection, and resource lifecycle. |
| `plugins/host/` | Manifest validation, process lifecycle, transport, and host RPC dispatch. |
| `plugins/security/` | Permission decisions, prompts, grants, and platform sandboxing. |
| `plugins/vfs/` | Navigation and projection for plugin-owned virtual views. |
| `remote/` | Connection configuration, remote cache, reconciliation, and synchronization. |
| `usbfs/` | Raw-device filesystem drivers and their scoped access gates. |
| `sdk/` | Pinned SDK source and shared contract definitions used by Core CI. |

The typed contracts in `sdk/rust/contracts/` are the single provider interface. `plugins/provider_registry/` owns registrations; `plugins/events/` applies event policy; `plugins/providers/` owns provider behavior. Native providers execute directly in Core. Plugin providers use the reusable IPC proxies. Filesystem access always goes through a scoped `ResourceBroker` and the same `VfsProvider` request/result types.

`plugins/events::Resolver` names an event candidate used by the router. It is not a separate legacy provider interface. Feature-specific provider implementations and proxies live under `plugins/providers/`.

See [Unified Provider Architecture](architecture/unified-provider-architecture.md) for the current contract and [archived migration reports](architecture/history/) for dated implementation snapshots.

## Validation

Run `npm run format:check`, `npm test`, `npm run build`, and the Rust format, unit-test, and Clippy checks. CI additionally validates the native desktop build and packages it on Windows, Linux, and macOS. Real remote-provider fixtures and official plugin packaging run in the `fiex-official-plugins` repository.
