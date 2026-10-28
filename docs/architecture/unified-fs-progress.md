# Unified FS/VFS migration (protocol 6)

This is an implementation checkpoint against `Fiex_Unified_FS_VFS_Architecture_Agent_Plan(2).md`, not a completed acceptance matrix.

Core's `ResourceBroker` resolves the `ResourceRef` in the captured operation scope, then dispatches to `LocalFsAdapter`, `PluginFsAdapter`, `UsbFsAdapter` or the in-process `BuiltinFsAdapter`. Plugin storage continues to receive `vfs.*`; Features call typed SDK `Files` over the `fs.*` Core service. The old `host.fs.*` name is removed from the SDK, native dispatch and official plugin source. The SDK and Core use one `FsOperation` enumeration. The initialization handshake rejects mismatched protocol versions. Stable per-process opaque node identities remain distinct from authorization.

File streams are bounded to 256 KiB per transfer and owned by the invoking operation. The plugin storage adapter commits writes through the provider, never a downloaded cache. USB NTFS, ext4, Btrfs, APFS and HFS+ offer bounded offset reads. NTFS/ext4 writes stage bounded blocks to a sibling device file and switch names after the staged stream completes. Core no longer changes raw block device node permissions; if an OS grant is unavailable, raw device access fails. On Linux a currently mounted block device is rejected. This conservative behavior can require the user to unmount or grant access through the OS before using a raw device.

The desktop transfer command now plans every source and destination through Core ResourceBroker, detects conflicts before executing writes, and copies in 256 KiB blocks. Source removal happens only after successful writes with no skips or unresolved conflicts. Preview reads a bounded prefix through the same broker. Native open uses a leased export, so USB files are not loaded into a single Vec. Recent retains source identities for local, plugin and USB items. The generic feature invocation path no longer substitutes a cached local path for a VFS selection.

External drag uses a Core export lease for virtual resources and returns ordinary paths to the existing platform drag bridge. It recursively stages directories with bounded reads. The lease lasts 30 minutes and has a 100 GiB aggregate quota and 100,000 item limit. Local paths can be handed to the platform directly. Feature matching now rejects source predicates and evaluates content/operation predicates over every selected node; empty `allowedOps` is an empty set.

## Remaining implementation work

- Recent aliases, Applications user streams and Trash view capabilities now have an in-process `BuiltinFsAdapter`. Complete the legacy `VirtualResolver` storage/action split and remove duplicate desktop paths.
- Retire the old local transfer and USB transfer implementations after their remaining internal and undo call sites migrate. The transfer planner now stages overwrite backups and attempts rollback on later failure. Remote rollback recovery, explicit byte progress and durable transaction journaling remain. Device replacement and network disconnection need manual verification.
- Complete independent capability snapshots, revision/generation invalidation, explicit mount registration, and precise view policies for all builtin roots and child refs.
- Unify native Resolver aggregation, clipboard, complete cross-window drag identity/mount invalidation and platform-specific promised-file support. Windows and macOS still use the existing Tauri drag plugin; no native OLE/AppKit promise bridge is implemented here.
- Remove remaining source-specific desktop listing/file-info dispatch, legacy export caches, and old Core path-based commands after all call sites are migrated.

No A01–A25 item is marked complete by this checkpoint. Manual Windows/macOS/Linux drag interoperability and device hot-unplug behavior require platform verification.

## Latest implementation and verification checkpoint

The Core PR now includes scoped opaque Recent aliases (including source-backed reads),
Applications private-store streaming, a unified broker path for virtual resources,
partial cross-source rollback, staged external exports and ResourceRef-bearing internal
drag sessions. The Feature matching contract no longer accepts source-directory
predicates. Official plugin manifests have been checked for forbidden source matching.

The SDK protocol-6 CI has passed on macOS, Windows, Linux and UI SDK. One earlier
Core CI baseline passed all desktop and frontend jobs; it predates the latest
migration commits. The newer CI jobs and official-plugin real-host/network
integration must be evaluated at their **actual PR HEAD** before release.

**A01–A25 are not all accepted.** In particular, source-independent desktop
listing, durable recovery when remote rollback fails, mount-generation
invalidation, full external drop into plugin/USB targets, 10GB end-to-end
stream simulations, and physical Windows/macOS/Linux drag compatibility
still require further implementation or verification. Do not merge or tag
a stable release based only on compilation or an older green CI run.
