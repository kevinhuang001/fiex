# Global error dialog policy

Fiex has one user-facing runtime error channel: `reportError(cause, source)`
from `src/lib/errorCenter.ts`, rendered by the single `ErrorDialogHost`
mounted in `App.tsx`. Native/Core errors are polled into this same channel.

## Rules

- Report actionable, failed user operations and backend/plugin runtime errors using
  `reportError`. Do **not** render the exception string in a page banner,
  settings pane, text field, terminal status, or plugin iframe.
- Preserve the failing workflow state (e.g. retain an open form, stop a spinner,
  prevent a partially loaded connection from being saved). If the broken view
  cannot render, show a **generic** unavailable/retry placeholder without
  repeating the error text; the dialog supplies the specific diagnostic.
- Keep local input validation (required fields, invalid rename templates,
  invalid numeric values), permission/risk warnings, loading indicators,
  unsupported-format fallback advice, and successful-operation feedback inline.
  Normal cancel and recoverable cache/fallback attempts are not runtime errors.
- `ui_error_report` is the plugin-to-Core diagnostics channel. Core persists it
  and the global dialog host presents it. Do not additionally call
  `reportError` on the success path or the same failure can appear twice;
  use a local `reportError` fallback only if the report itself fails.
- Make all async UI handlers explicitly catch rejected promises. Avoid alert
  loops caused by error polling or intended fallback paths. Use source keys
  such as `plugins:installation`, `connections:configuration`, `file-picker`,
  and `preview:pdf` to identify the failing operation.

## Audited user-facing flows

- Plugin manager: catalog, import/install/upgrade/uninstall, activation,
  permissions, provider priorities, and plugin settings load/save.
- Plugin content: connection views, preview iframe, panel bridge, and
  plugin/Core `ui_error_report` diagnostics.
- General application: connection editor/setup, credential backend,
  background-image selection, file info, batch rename execution,
  file picker, file preview/PDF/media, terminal session failures, and
  explorer operation dialogs.

## Regression checks

1. Force a failed plugin install and permission write: one global error dialog,
   no embedded raw error message; the original controls remain available.
2. Force a connection configuration fetch failure: popup appears and the
   partially loaded form cannot be submitted.
3. Make a PDF renderer, plugin panel, or directory picker fail: show the global
   dialog and a non-diagnostic fallback state instead of raw exception text.
4. Enter an invalid batch-rename regex: show local field validation *without*
   a modal; cancelling a picker must not show an error.
5. Force a Core/SDK `ui_error_report`: the global dialog shows it once.
