# Cache, diagnostics and notification contract

Implementation baseline: Core PR #27, `79f72cf776b544ec212466d5fa19b2667b883ec8`.
This follow-up supersedes the navigation-only enrollment description in that PR.

## Resource lists and presentation

`ResourceBroker::desktop_listing` authorizes the requesting scope before joining
`list_service`. A VFS list flight reads every page through the existing bounded
Mount pagination implementation. Only a successful complete enumeration commits
an unfiltered snapshot. `ResourceBroker::list`, plugin search fallback, system
VFS search, Explorer, Columns and FilePicker use this boundary. Provider-native
search hits never become directory snapshots. The duplicate search refresh-tree
worker has been removed.

Local FS and USBFS bypass structure snapshots on both read and write. They use
live adapter listing/search. A request-local in-flight promise is not a directory
cache. Short-lived preview/info caches and offline byte caches remain separate.

`directory-v2` isolates old mixed FS/VFS directory-v1 snapshots; v1 is never read
or written. Users may remove the obsolete version directory. Disk budget remains
64 MiB / 2,000 files / 4 MiB per snapshot; structure memory remains bounded to
10,000 directories / 500,000 entries. Disk hits populate this bounded memory.
Snapshot identity includes public connection configuration, account revision and
mount identity without retrieving keyring secrets.

Structure and Decoration are independent disk records. Only `directory_open`
creates presentation snapshots. Generic `plugin_decorations`, recursive listing
and search do not write them. Directory views combine structure and presentation
before sending a frame. Unchanged entry presentations survive revalidation;
changed entries are resolved again. Locale and enabled/approved provider versions
and preferences invalidate presentation. Local and USB presentation is transient.
Cached VFS presentation can paint before a live provider stat/list; live failures
retain the authorized cached display. Columns and FilePicker use the same view
service. The independent list flight is not owned by one consumer's cancellation.

Successful broker create, mkdir, remove, trash, rename and stream commits, plus
completed transfer execution, publish through `changes::committed`. Failed stream
commits do not publish. Affected directory/parent snapshots and graph-reachable
children are invalidated; preview/info keys also include a mutation revision.
Flights check connection identity and mutation revision before committing data.
Existing specialized Applications/connection invalidations remain active.

## State

StateStore owns persistent user intent; cache eviction never removes it.
Explorer preference defaults remain global. Explicit column resizing is scoped
per directory under `layout.columns.<SHA256(path)>`, including the name column.
Sidebar locations migrate once from localStorage into StateStore; first-paint
reuse is process-local. Picker keeps separate open/save locations and sort/group
state. Existing emergency legacy preference persistence on backend failure is
retained to avoid losing edits; normal successful persistence has one writer.

## Diagnostic and error boundaries

All Rust log ingestion, error notification text, provider error details and
history reads share `infrastructure::redact`. Headers, folded continuation lines,
JSON/config credentials and OAuth query values are scrubbed before truncation.
Oversized diagnostics are omitted wholesale. Frontend command errors and log
exports have an additional scrubber. Logs use a new `logs-v2` directory so old
unscrubbed history cannot reappear in the viewer. No RPC parameter/body/command
argument payload is added to lifecycle logging.

One bounded writer thread owns append/rotation/clear ordering. The queue holds
at most 2,048 jobs, preserves warn/error over low-priority queued records when
possible, and attaches merged dropped counts. Plugin limits are independent by
severity/channel, so stderr/debug cannot consume error quota. Core-generated
lifecycle diagnostics bypass plugin emission quotas. Memory has a global source
budget of 128 and a per-source budget of 500. Disk pruning bounds all sources to
64 MiB / 256 JSONL files; each source rotates at about 1 MiB. Exit performs a
750 ms maximum best-effort flush. Disk failure never recursively logs.

Records have stable IDs, optional host-assigned operation IDs and durations.
RPC start/completion and SDK log calls use the same authenticated owner context;
plugins cannot submit another plugin identity. SDK's existing simple log/error
API is retained; no SDK or official plugin contract change is necessary.
`plugin_logs(after)` returns an incremental page and cursor. UI pause stops
polling without stopping collection; the viewer bounds retained rows to 500 and
supports level/text filtering, copy/export and clear.

Error policy never searches provider message text for control-flow keywords.
Exact Core-owned cancellation/fallback/authorization codes suppress notifications;
plugin-provided codes are namespaced and cannot impersonate these controls.
The transfer cancellation wire code is `Cancelled:`. ErrorCenter retains a bounded
five-minute / 50-event journal, and windows read using independent cursors instead
of draining a shared queue. Duplicates aggregate across the retained journal;
per-source distinct-error budgets limit storms. Unbound plugin notifications are
broadcast to active window subscribers. Command failures remain owned by their
initiating UI, avoiding automatic backend-plus-frontend double presentation.

The public writer accepting a caller-provided plugin identity has been removed.
UI frame diagnostics use `ui_error_report`, whose source is derived from the
calling WebView. Runtime plugin logs/errors continue through authenticated Host
RPC. A log at error level alone never opens a modal. `file_info_details` now
returns a real failure rather than an empty success. Error notifications have a
linked diagnostic ID; no retry/elevation action is inferred from message text.

## Verification and platform limits

Locally verified: 272 Rust core tests (including USB image adapters), 66 frontend
tests, TypeScript/Vite production build, Rust formatting, frontend formatting,
i18n check, and headless Rust Clippy with warnings denied. Regression coverage
includes local directory bypass, empty complete VFS snapshots, graph invalidation,
Decoration metadata mismatch, independent error readers, redaction and ordered
log clearing.

Desktop native build and Windows/macOS smoke require repository CI. The local
runtime lacks GTK/WebKit/pkg-config development dependencies. The existing
repository-wide file-size check already reports unrelated oversized legacy
modules; this follow-up splits the broker adapter contract and stream handles,
and keeps all new infrastructure modules below 300 lines.

Remaining acceptance beyond unit checks: real remote provider pagination failure,
multiple desktop panes and windows, Windows sandbox stderr under pressure,
account revocation during navigation, disk-full/multiple-process log rotation,
and actual application icon first-paint behavior. Do not treat green headless
tests as cross-platform runtime acceptance or publish this branch automatically.
