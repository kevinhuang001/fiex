# Fiex 0.7.5

SSH now opens the official plugin's self-contained PluginView. Remote terminal rendering, transport and session state live in the SSH plugin; Core uses the same connection and UI bridge as every other plugin.

Removed the built-in SSH fallback, hard-coded SSH plugin routing and host-terminal API. Local shell PTYs remain in the terminal module. Saved connection autostart resolves navigation through each provider.
