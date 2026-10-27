# Builtin context-menu and decoration providers

Context menus and entry decorations keep their feature-specific decisions out
of the generic menu/list/tile/column renderers.

## Context menus

`src/components/explorer/context-menu/items.tsx` is the builtin provider registry
and collector. Each provider has an ID, order, predicate, resolver and optional
`tail` placement. The collector captures the effective selection once: a click
inside the current selection targets that selection; a click outside it targets
only the clicked entry. Providers receive both `paths` and `multiple`.

The builtin providers are opening/inspection, macOS tags, connection cache/sync,
single rename, batch rename, clipboard/duplicate, terminal, copy path, and Trash.
Batch rename's predicate requires more than one effective target. Connection
operations require all targets to belong to remote connections; a mixed
local/remote selection does not offer operations that would fail halfway.
Delete/restore actions explicitly declare tail placement; translations never
control grouping or item identity.

These builtin providers execute in the frontend because their commands include
opening dialogs, clipboard state, preview and activity UI. Their action closures
reuse the existing controller and native command boundaries. They resolve
synchronously and do not introduce another IPC round trip or download files
while discovering a menu. Adding a builtin requires a provider module and a
registry entry, not a new conditional in the renderer.

Plugin menu providers retain the native `contextMenu.resolve` EventRouter,
validation, expiring menu sessions, lazy submenus and `contextMenu.execute`
authorization. `PluginContextActions` remains between builtin items and the tail.
The frontend registry does not grant plugin permissions or expose builtin
command closures over plugin RPC. This is a frontend presentation-provider
registry, separate from Rust's plugin EventRouter.

## Decorations

`src/lib/decorations` resolves independent tag, read-only and connection-status
providers from existing listing metadata. They return the same `EntryDecoration`
shape used by native plugin results. The connection provider owns cached,
uncached, pinned, pending, conflict and uploaded badges; uploaded is independent
of cache/pin mode. The tag provider keeps the five-dot limit.

`useEntryDecorations` immediately resolves builtins and merges the native
`plugin_decorations` response when it arrives. Builtins remain visible during
plugin loading, failure and search. Responses for an older listing are ignored,
and an older response is never painted onto a new listing. Icon publication and
cleanup are owned by this hook rather than the listing controller.

Builtin provider priorities define their merge order. Validated plugin output
is applied afterward: it can override labels, descriptions, icons and style
properties; badges append and per-column fields survive. Builtin badges do not
consume Core's existing plugin badge allowance. The inputs are never mutated.
Only currently listed paths are admitted to the collected result.

`NameCell`/`Badges` render the collected DTO, and all views use those renderers.
They contain no connection-state, upload-state, tag or read-only policy. Optional
local badge presentation fields (`dot`, `status`, `icon`) preserve the existing
visuals. Rust's plugin decoration decoder strips those fields, so this does not
change the plugin SDK/wire contract or weaken native sanitization.

Run `npm test`, `npm run build`, `npm run check:i18n`, and the Playwright suite to
validate selection matching, cache commands, presentation merging and UI flows.
