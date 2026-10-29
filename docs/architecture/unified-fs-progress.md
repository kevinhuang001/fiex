# Protocol 6 FS/VFS audit and implementation status

Reviewed against `Fiex_Unified_FS_VFS_Architecture_Agent_Plan(2).md` using Core
PR #21, SDK PR #9 and official plugins PR #19. This is a current implementation
report, not a declaration that all A01–A25 scenarios have passed.

## Implemented and hardened

- Core ResourceBroker captures scoped opaque refs and routes local, plugin,
  USB and builtin storage through FsAdapter. Desktop listing, file information,
  preview prefixes, transfers and external export use the broker. Feature calls
  use `fs.*`; storage-provider calls use `vfs.*`; protocol 6 rejects old peers.
- All Feature discovery paths now use one semantic/capability projection.
  requiredOps is evaluated during discovery and again before invocation. Info,
  Preview, menus, decorations and search do not receive empty capability sets by
  accident. Actual preview candidates are selected for the current resource;
  their cache includes its identity and capability snapshot.
- Recent has private source aliases. Applications user entries stream into the
  private store. Trash denies ordinary reads, writes and generic export/delete.
  Symlink identity survives metadata projection instead of becoming an ordinary
  file in the plugin boundary.
- Transfer copies use 256 KiB bounded reads and retain source files on conflict,
  skip, cancellation or incomplete commit. Move cleanup rechecks the complete
  source tree, then removes leaves before directories. Rollback records only
  successful mutations and preserves concurrent destination edits. If remote
  rollback fails, original bytes and a recovery manifest are retained privately.
- Known-length plugin transfers send their size to the storage provider.
  Unknown-length plugin output is privately spooled under an aggregate 100 GiB
  limit, checked for destination changes, then streamed at commit. Created file
  and directory IDs are rebound from the provider's returned opaque node IDs.
- USB reads are bounded on NTFS/ext4/APFS/HFS+/Btrfs. Device generations include
  device identity and Linux disk sequence; ejection invalidates old bindings.
  Reads/writes recheck their generation. Reentrant per-device gates serialize
  raw-driver operations; Linux holds an O_EXCL device handle to prevent OS mount
  races. Raw devices remain fail-closed on platforms without a safe device broker.
- Export uses adapter capability policy rather than source URI branches. Native
  paths are an adapter optimization. Other sources stage recursively with lease,
  byte/item quotas and duplicate-name isolation. Leases survive the gesture;
  private filesystem locks protect other Fiex processes from orphan cleanup.
- SDK FileReader rejects premature EOF and obeys the opened snapshot length.
  SDK releases use Rust package version 0.3.0, while the browser SDK remains
  0.1.1. Both consuming repositories pin the same SDK revision.

## Remaining architectural work / release gate

1. Desktop create/rename/delete/search and the old VirtualResolver storage/action
   interface still have parallel source dispatch. FsAdapter is not yet the only
   operation implementation. Old local/USB transfer helpers retain callers.
2. FsError is still represented by strings. Explicit uniform mount registration,
   provider-node replacement generations, connection reconfiguration invalidation
   and fully independent capability snapshots need completion.
3. Unknown-length spool quotas are implemented, but the transfer API still reports
   item progress; export preparation has no complete user cancellation/progress
   protocol. Rollback retention is not a full crash-replay transaction journal.
4. Windows/macOS use the existing native drag dependency with staged real paths.
   A first-class promised-file bridge is absent. Physical cross-application drag,
   external drops into all writable providers, and X11/Wayland interoperability
   are not established by compilation or headless CI.
5. macOS/Windows raw block-device access is intentionally denied without a scoped
   platform device broker. It must not be replaced by global device chmod.
6. Sparse 10 GiB offset reads are covered, but an end-to-end 10 GiB transfer/export
   simulation and real hotplug/network-loss matrix remain incomplete.

Latest-HEAD CI is required for all three PRs. A green run establishes the checks
it actually executed; it does not establish physical device/OS interoperability.
Do not merge/tag a stable release as "all plan requirements complete" while the
above implementation and verification gaps remain.
