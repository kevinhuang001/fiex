# Unified navigation cache and transfer preflight

Based on `feat/unified-search-and-ui-polish` at `f69f565`.

## Navigation and presentation

Core FS owns one bounded, in-memory directory snapshot store for local FS,
USB FS, connection mounts, plugin views, Recent, Applications and Trash.
Only `ResourceBroker::desktop_listing` enrolls a visited directory. Stat,
transfer traversal and live search list calls do not enroll directories.
Empty snapshots are cache hits. Hidden/ignore preferences are applied when
reading an unfiltered snapshot, and directory capabilities stay in the snapshot.
USB generations isolate replaced devices.

The pane starts live list and cache lookup together. It paints the cache first,
then compares all entry metadata independent of enumeration order. Equal data
retains the existing entry array, selection and scroll. Refresh failure keeps
the cache visible. There is no startup recursive warm-up or `virtual_cache` IPC.

Frontend entry presentation snapshots retain icons, labels, badges, field/row
styles and descriptions by entry metadata and locale. They are read synchronously
when cached entries render. Decoration refresh does not clear the last snapshot.
Native icons expose synchronous resolved-cache lookup and keep the same image
while revalidating. A cold virtual application reserves its icon slot until its
real decoration resolves, avoiding a generic-app-to-real-icon swap.

## Search

VFS filename search first streams matches from the visited directory graph using
ripgrep's fixed-string, case-insensitive matcher. Hidden/ignored/deleted parent
edges cannot expose cached descendants. Search never reads file contents to
match a filename.

Live list refreshes of enrolled directories and live search run together.
Providers with native search stream paginated results; list fallback and builtin
views emit hits during traversal. Existing hits are updated by path, identical
hits do not append duplicates, and the terminal authoritative set removes stale
hits. An unchanged set emits completion without replacement. Cancelled queries
are ignored by the pane generation. A cache refresh stops between list calls
when cancelled; an in-flight provider RPC follows its existing timeout behavior.

The metadata cache does not provide content-search results. Existing content
search behavior and capability checks remain in force.

## Transfer policy

The SDK exposes `TransferPreflight`, `TransferMetadata`, `PreflightDecision`,
`TransferDirection` and `TransferAction`. Providers opt in with
`supports_preflight()` and implement `preflight(ctx, request)`. Handshake declares
`vfs.preflight`; Core invokes it only on opted-in owners. Provider requests contain
its own node ID and portable metadata, never another provider's credentials or
host paths. Unknown size and executable-shortcut classification stay optional.
A malformed reply or a hook failure rejects the transfer.

Capabilities and scope remain mandatory. Preflight can further deny a permitted
operation but cannot grant a missing operation. Copy/move planning reviews source
export and destination import before writes, then repeats review before execution
and move cleanup. Rename and external drag export also run review. Descendants
whose destination parents do not exist yet are reviewed by the nearest existing
destination ancestor; they cannot invent provider node IDs.

Applications denies system-application export outside its view and accepts
external imports only when Core can identify an executable shortcut. Local Windows
`.lnk`, Linux application `.desktop` entries and macOS Finder aliases have platform
classification. Unknown remote shortcut targets are denied by this Applications
policy. Internal system-application organization remains a mapping operation.
Recent continues to deny create/write through its capabilities, with no
Recent-specific preflight rule. Trash retains its dedicated restore/delete actions.

The final revision was submitted without further testing or CI waits at the
user's request. Earlier checks do not establish validation of the final revision.
