# Themes and panel lifecycle

## Theme packages

Themes are installed and enabled through the same plugin flow. A pure theme has no `runtime`, executable, iframe, or special publisher privilege. Its `contributions.themes` is a list of `{ "path": "theme.json" }` entries. Paths must remain inside the package. Core rejects oversized files, duplicate IDs, invalid metadata, and token values containing CSS declarations or URLs.

```json
{
  "schemaVersion": 1,
  "id": "example.theme",
  "name": "Example",
  "version": "1.0.0",
  "permissions": {},
  "contributions": { "themes": [{ "path": "theme.json" }] }
}
```

```json
{
  "id": "light",
  "name": "Example Light",
  "mode": "light",
  "tokens": {
    "accent": "#0067c0",
    "surface": "#f3f3f3",
    "canvas": "#ffffff",
    "text": "#202020",
    "muted": "#626262",
    "line": "#dedede",
    "hover": "#e9e9e9",
    "selected": "#e5f1fb",
    "radius-control": "6px",
    "radius-panel": "12px"
  }
}
```

The settings identifier is `<plugin-id>:<theme-id>`. `mode` is `light` or `dark`. Optional tokens inherit Fiex defaults for that mode. Tokens include `danger`, `success`, `warning`, `on-accent`, `glass`, `shadow`, `overlay`, `sidebar-background`, `toolbar-background`, and `font-family`. No raw stylesheet or executable theme code is accepted. Disabling/removing the selected theme restores the default palette.

Core publishes the computed palette as `theme.changed` on each panel's private MessagePort, including at initial handshake. The SDK applies tokens as inline custom properties inside the opaque-origin iframe. Custom token names may be used by cooperating plugins; they do not create arbitrary CSS selectors or permissions.

## Panels

A client installs its message listener, then sends `fiex.bridge.listening`. The host probes readiness and transfers one MessagePort only after receiving that event from the specific iframe window. Nonce, session ID and method authorization remain enforced. No fixed visual delay or timed handshake retries remain; a 10-second connection timeout reports failure.

Up to three panels are retained per window, keyed by plugin, panel, directory and selection. Switching context creates an isolated session. Hidden panels receive `panel.visibility=false`; new side-effect requests are rejected while hidden, while existing task completion can still be reported. All retained sessions are destroyed when the plugin catalog/authorization changes. Eviction closes ports and finalizes owned activity entries.

Cancellation stops waiting in the UI and suppresses late bridge responses. It does not claim to terminate an already-running external command. Commands remain bounded by the host timeout.

## Native IPC

The current protocol remains JSON-RPC 2.0 with framed messages. File readers retain one 256 KiB read-ahead window; seeks inside it need no new request. Writers aggregate small writes up to 256 KiB and flush before seek/commit. Dropping a writer aborts its transaction. A failed flush poisons the writer so retry cannot duplicate uncertain output.

This reduces round trips; file chunk transport still uses Base64 and is not zero-copy. The native plugin protocol is serial per process because reverse Host requests share an invocation directory capability. Multiplexing it without explicit per-request resource contexts would change isolation semantics, so serialization is retained. Independent plugins have independent instance locks. RPC response waiting has a total 120-second deadline instead of restarting the timeout for each reverse request.

CI tests assert RPC counts, seek behavior, abort and failed-flush handling, readiness handshake, method rejection, and event delivery. Wall-clock latency gains depend on platform and workload and are not claimed without measurements.
