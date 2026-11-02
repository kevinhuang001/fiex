# Unified Provider Architecture

Core, the Rust SDK, and official plugins use one typed contract for provider registration, matching, invocation, and result validation. Native Core providers execute in process; plugin providers use the same contract through Core-owned IPC proxies.

## Module ownership

| Module | Owns |
| --- | --- |
| `sdk/rust/contracts/` | Shared provider traits, descriptors, DTOs, and validation rules. |
| `src-tauri/src/plugins/provider_registry.rs` and `src-tauri/src/plugins/provider_registry/` | Registered descriptors and instance lookup for plugin and native providers. |
| `src-tauri/src/plugins/events/` | Event routing, candidate selection, scoped invocation setup, and event result policy. |
| `src-tauri/src/plugins/providers/` | Feature providers and their native implementations or reusable plugin IPC proxies. |
| `src-tauri/src/core/fs/provider_registry/` | Resource ownership, provider routing, operation dispatch, and the plugin mount adapter. |
| `src-tauri/src/core/volumes/native_vfs/` | Core `VfsProvider` implementations for local storage, USB, Recent, Applications, and Trash. |
| `src-tauri/src/plugins/vfs/` | Core-owned plugin view identity, navigation, and projection into the shared VFS contract. |
| `src-tauri/src/core/fs/` | `ResourceBroker`, scoped file operations, caching, exports, and transfer orchestration. |
| `src-tauri/src/plugins/resource/` | Resource snapshots, ref binding, capability projection, authorization scopes, and lifecycle cleanup. |
| `src-tauri/src/core/volumes/applications/` | Application catalog, platform enumeration, private user store, folder mappings, and launch/icon adapters. |

## Source layout

```text
src-tauri/src/
├── core/
│   ├── fs/
│   │   ├── provider_registry/  # ownership, access, reads, search, writes, plugin mount adapter
│   │   └── transfer/           # plan, execute, rollback, and cleanup
│   ├── search/                 # query orchestration and local search
│   └── volumes/
│       ├── applications/
│       │   ├── folders/
│       │   ├── platform/       # linux, macOS, Windows
│       │   └── store/          # paths, listing, mutations
│       └── native_vfs/         # local, USB, Recent, Applications, Trash providers
└── plugins/
    ├── provider_registry/      # registration and provider instance lifecycle
    ├── events/                 # shared routing and policy
    ├── providers/              # feature-owned native providers and IPC proxies
    ├── resource/               # resource identity, authorization, capabilities, lifecycle
    └── vfs/                    # plugin-owned VFS views and projection
```

## Dispatch boundaries

Provider descriptors use the SDK's typed traits and matcher. The provider registry owns registration and instance lookup. `EventRouter` applies each event's collection policy and validates each provider result.

Filesystem calls first bind a resource reference to a Core-owned scope. The filesystem provider registry resolves its owner, then invokes either the registered native `VfsProvider` directly or the plugin mount proxy. Plugin supplied paths never become host filesystem authority. Provider declarations and node-level `allowedOps` both narrow the operation set; actual calls recheck authorization and resource state.

Feature modules under `plugins/providers/` own presentation and feature-specific validation. `plugins/events::Resolver` is the router's candidate record; it does not define a second provider execution interface. Context menus use the shared `ContextMenuProvider` resolve/execute contract, while VFS operations use `VfsProvider` and the shared filesystem DTOs.

URI parsing stays at resource ownership and concrete provider boundaries. A URI does not select an unrelated execution path or bypass provider matching. Native and plugin VFS implementations share the same descriptor, capability, request, and result model.

## Change and verification rules

- Keep each module focused on one domain or operation family; split large implementations by ownership boundary rather than adding forwarding or compatibility modules.
- Put provider-specific URI interpretation in its owning provider. Keep routing and policy generic.
- Add regression coverage beside the behavior it protects; cross-provider tests live in `core/fs/provider_tests.rs`.
- Validate Rust formatting, unit tests, Clippy, frontend checks, and the platform CI matrix before publishing a Core/provider change.

The dated migration reports are archived under `docs/architecture/history/`; they record earlier snapshots and are not the current module map.
