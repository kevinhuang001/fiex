# Global error dialog policy

Fiex has one user-facing runtime error channel: `reportError(cause, source)` from `src/lib/errorCenter.ts`, rendered by `ErrorDialogHost` in `App.tsx`. Native/Core errors are polled into this same channel.

## Rules

- Report failed user operations and plugin/backend runtime errors with `reportError`. Do not display raw exceptions in inline banners, text fields, terminal status, or iframes.
- Preserve workflow state when operations fail. The global dialog shows the specific error; failed content areas may show a generic fallback and retry state.
- Keep required-field validation, invalid rename templates, permissions and risk notices, loading, successful operations, and unsupported-format advice inline.
- `ui_error_report` persists plugin errors through Core. Avoid reporting the same failure locally when Core already queues it. Only report locally if the Core reporting path fails.
- Handle async UI failures explicitly. Suppress normal cancel and recoverable fallback attempts rather than causing repeated error dialogs.

## Audited flows

- Plugin manager: catalog, installation, upgrades, uninstall, activation, permissions, preferences, and plugin settings.
- Plugin UI: connection views, preview iframe, panel bridge, and `ui_error_report`.
- Application: connection setup, credential backend, background selection, file information, batch rename, file picker, preview, PDF, terminal, and explorer dialogs.

## Regression checks

1. Failed plugin installation and permission writes show one global dialog and no raw inline error.
2. Failed connection configuration load prevents saving partial settings.
3. Failed PDF render, plugin panel, and file picker show the dialog and a generic fallback.
4. Invalid rename regex shows local validation without the global dialog.
5. Cancelled pickers do not report errors; Core plugin reports produce only one dialog.
