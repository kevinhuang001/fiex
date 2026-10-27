# Builtin context-menu and decoration providers

Context menus and entry decorations keep feature-specific decisions out of the
generic menu/list/tile/column renderers.

## Context menus

Builtin and plugin context menus share one Rust pipeline in
`src-tauri/src/plugins/context_menu/`. `builtin.rs` owns the Core menu policy:
opening/inspection, macOS tags, connection cache/sync, single rename, batch
rename, clipboard/duplicate, terminal, copy path and Trash. Its providers are
registered with the same `EventRouter` used for plugins, so ordering, match
rules and execute authorization are identical.

`query()` captures the effective selection once (a click inside the current
selection targets that selection; a click outside targets only the clicked
entry), projects resource refs and asks every matching provider for items. Menu
sessions expire, reject disabled/unknown item ids and can only execute through
the provider that produced them. Submenus are lazy for plugins and validated
for depth, duplicate ids and item count.

Builtin executions return a frontend-only `coreAction` (open, preview, info,
tags, rename, clipboard, trash, …) or a `remoteAction` handled by
`context_menu/actions.rs`. Plugin executions may open a panel. The frontend
`FileContextMenu` renders the returned items in tail/non-tail order,
`PluginContextActions` inserts plugin submenus, and `execute.ts` applies the
returned core action. The frontend does not reconstruct availability,
selection or connection policy.

Adding a builtin means adding policy to `builtin.rs` (and the matching core
action in `execute.ts` if it is a new command), not a new conditional in every
renderer.

## Decorations

`src-tauri/src/plugins/decorations/` resolves builtin tag, read-only and
connection-status providers from the same listing metadata used by plugins. The
connection provider owns cached, uncached, pinned, pending, conflict and
uploaded badges; uploaded is independent of cache/pin mode. The tag provider
keeps the five-dot limit.

Builtin and plugin results are merged in deterministic priority order. Plugin
output can override labels, descriptions, icons and style properties; badges
append and per-column fields survive. Builtin badges do not consume Core's
plugin badge allowance. Inputs are never mutated and only currently listed
paths are admitted.

`NameCell`/`Badges` render the collected DTO, and all views use those renderers.
They contain no connection-state, upload-state, tag or read-only policy.

Run `npm test`, `npm run build`, `npm run check:i18n`, and the Playwright suite to
validate selection matching, cache commands, presentation merging and UI flows.
