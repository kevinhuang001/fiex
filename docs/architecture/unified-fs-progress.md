# Unified FS/VFS migration (protocol 6)

This is an implementation checkpoint against `Fiex_Unified_FS_VFS_Architecture_Agent_Plan(2).md`, not a completed acceptance matrix.

Core's `ResourceBroker` resolves the `ResourceRef` in the captured operation scope, then dispatches to `LocalFsAdapter`, `PluginFsAdapter` or `UsbFsAdapter`. Plugin storage continues to receive `vfs.*`; Features call typed SDK `Files` over the `fs.*` Core service. The old `host.fs.*` name is removed from the SDK, native dispatch and official plugin source. The SDK and Core use one `FsOperation` enumeration. The initialization handshake rejects mismatched protocol versions. Stable per-process opaque node identities remain distinct from authorization.

File streams are bounded to 256 KiB per transfer and owned by the invoking operation. The plugin storage adapter commits writes through the provider, never a downloaded cache. USB NTFS, ext4, Btrfs, APFS and HFS+ offer bounded offset reads. NTFS/ext4 writes stage bounded blocks to a sibling device file and switch names after the staged stream completes. Core no longer changes raw block device node permissions; if an OS grant is unavailable, raw device access fails. On Linux a currently mounted block device is rejected. This conservative behavior can require the user to unmount or grant access through the OS before using a raw device.

The desktop transfer command now plans every source and destination through Core ResourceBroker, detects conflicts before executing writes, and copies in 256 KiB blocks. Source removal happens only after successful writes with no skips or unresolved conflicts. Preview reads a bounded prefix through the same broker. Native open uses a leased export, so USB files are not loaded into a single Vec. Recent retains source identities for local, plugin and USB items. The generic feature invocation path no longer substitutes a cached local path for a VFS selection.

External drag uses a Core export lease for virtual resources and returns ordinary paths to the existing platform drag bridge. It recursively stages directories with bounded reads. The lease lasts 30 minutes and has a 100 GiB aggregate quota and 100,000 item limit. Local paths can be handed to the platform directly. Feature matching now rejects source predicates and evaluates content/operation predicates over every selected node; empty `allowedOps` is an empty set.

## Remaining implementation work

- Migrate Recent, Applications and Trash fully into storage adapters and separate their special actions; remove the legacy `VirtualResolver` storage implementation.
- Retire the old local transfer and USB transfer implementations after their remaining internal and undo call sites migrate. The new transfer planner still needs rollback of already committed destination items when a later item fails and explicit byte-based progress. Device replacement and network disconnection need manual verification.
- Complete independent capability snapshots, revision/generation invalidation, explicit mount registration, and precise view policies for all builtin roots and child refs.
- Unify native Resolver aggregation, clipboard, internal drag session refs and platform-specific promised-file support. Windows and macOS still use the existing Tauri drag plugin; no native OLE/AppKit promise bridge is implemented here.
- Remove remaining source-specific desktop listing/file-info dispatch, legacy export caches, and old Core path-based commands after all call sites are migrated.

No A01–A25 item is marked complete by this checkpoint. Manual Windows/macOS/Linux drag interoperability and device hot-unplug behavior require platform verification.


## Review checkpoint: 2026-10-29

This checkpoint reflects the current protocol-6 PR stack, not a release declaration:

- Core PR #21: `kevinhuang001/fiex` / `refactor/unified-fs-broker`.
- SDK PR #9: `kevinhuang001/fiex-sdk` / protocol 6.
- Official plugins PR #19: `kevinhuang001/fiex-official-plugins`.

### Verified improvements

- Resolved scoped selection of not-yet-existing local children. Strict Core tests that previously failed `invalid selected path` now pass on Windows, Linux and macOS in the tested baseline.
- Restricted file exports to actual read/list capabilities; enforced concurrent lease quotas before staging, lease expiry cleanup, incomplete stream rejection and symlink export rejection.
- Blocked ordinary read/mutate/export on physical Trash entries. Restore, purge and empty must remain explicit Core actions. Windows recycle actions now reject an indexed object if its name, original location, deletion timestamp or kind changed.
- Prevented destructive cross-kind transfer replacements. Full rollback of previously committed items in a larger transfer is still outstanding.
- Protected the Applications private-store importer from following symlink source trees.
- Removed the superseded Core remote upload engine; corrected protocol-6 documentation and pinned Core/plugins to the same updated SDK gitlink.
- Updated demonstration fixtures and E2E expectations. A Core CI baseline from before the latest hardening patches passed frontend/E2E and all three desktop platform checks. **The baseline does not validate newer commits.**
- Added a fast all-crate Rust formatting gate to official plugin CI before long three-platform builds; resolved observed formatting failures in Git, WebDAV, GDrive, SMB and other plugins.

### Release-blocking architectural gaps

- **Built-in virtual storage**: `core/fs/mod.rs` still rejects opaque built-in paths. Migrate Recent, Applications and Trash into dedicated adapters; split storage from the old `VirtualResolver`.
- **Desktop dispatch**: `desktop/files/listing.rs` still switches between remote, plugin, USB, virtual and local paths. Resolve through resource capabilities instead.
- **Transfer transactions**: `core/fs/transfer.rs` does not roll back targets already committed before a later failure. Add rollback/recovery, revision checks, cancellation and byte progress.
- **Native interoperability**: `desktop/volumes.rs` lacks Windows OLE and macOS file-promise bridges. External drops into plugin/USB VFS are not fully unified.
- **Identity and invalidation**: `core/drag_session.rs` still stores path pairs. Preserve refs across window drags, revoke stale sessions and implement mount/node generation checks.
- **Large-file proof**: bounded chunks exist, but the full 10GB simulated test matrix, OS consumer lifetime, cancellation and recovery checks have not run.

The **A01–A25 acceptance matrix is still unverified end-to-end**. None of the rows is marked fully accepted by this checkpoint. At a minimum A16/A17 (Windows/macOS external drag), A14/A15 (cross-source transaction safety), A19/A20 (external drop and ref-bearing internal drag), and A21 (invalidation) block stable release.

### Release gate

Do **not** merge or publish solely because the SDK CI or an earlier Core run is green. Require the **latest SHA** of each PR to pass every relevant job, the remaining architecture migrations to land, the A01–A25 scenarios to be validated and the three-OS external drag to be manually exercised before tagging.
