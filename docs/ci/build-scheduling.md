# Build scheduling

The frontend checks produce a single `desktop-web-assets` artifact containing the production `dist` tree and generated native icons. All desktop installers consume that artifact. The CI-only Tauri config override clears `beforeBuildCommand`; ordinary local builds still run the frontend build normally.

Browser E2E and the three native desktop jobs start after frontend checks. Browser failure still blocks release. Rust tests, IPC examples and Clippy retain their platform coverage. Desktop jobs use `CARGO_BUILD_TARGET` consistently so debug dependencies can be reused across tests, examples and Clippy. The SDK Rust cache points at its real `sdk/rust/target` directory.

Cheap frontend/syntax/format failures now stop the native jobs before platform dependency installation and compilation. Previous CI exposed a dangling OR operator in the shortcut modal guard; that syntax error is fixed without changing its intended condition.

No local test or build suite was run for this change, as requested. CI timings are not yet measured. The expected savings are repeated frontend/icon builds and duplicate Rust target compilation, rather than a guaranteed percentage reduction.
