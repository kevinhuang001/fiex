# Build scheduling

The frontend checks produce a single `desktop-web-assets` artifact containing the production `dist` tree and generated native icons. All desktop installers consume that artifact. The CI-only Tauri config override clears `beforeBuildCommand`; ordinary local builds still run the frontend build normally.

Browser E2E and the three native desktop jobs start after frontend checks. Browser failure still blocks release. Rust tests, IPC examples and Clippy retain their platform coverage. Desktop jobs use `CARGO_BUILD_TARGET` consistently so debug dependencies can be reused across tests, examples and Clippy. Core and the SDK mock now share `src-tauri/target`, avoiding separate dependency compilation and duplicate target caches.

Cheap frontend/syntax/format failures now stop the native jobs before platform dependency installation and compilation. Previous CI exposed a dangling OR operator in the shortcut modal guard; that syntax error is fixed without changing its intended condition.

No local test or build suite was run for this change, as requested. CI timings are not yet measured. The expected savings are repeated frontend/icon builds and duplicate Rust target compilation, rather than a guaranteed percentage reduction.

## Build caches

- Frontend: an exact input hash restores the production bundle and generated icons. Source, lockfile, SDK UI, Vite/TypeScript configuration, scripts and assets participate in the key. On a hit, formatting, i18n, unit tests and TypeScript checking still run. Bundling/icon generation is skipped only on an exact hit; there is no fallback to a different frontend source.
- Native dependencies: rust-cache keeps the Cargo registry/git cache, with target caching disabled to avoid duplicating the full native cache.
- Native compiler outputs: a separate cache preserves the complete shared target tree, including Core's own libraries, fingerprints, examples and release outputs. The ordinary rust-cache cleanup prunes workspace crates and example binaries, even when cache-all-crates is enabled (that setting controls registry cleanup).
- Native keys include OS/architecture, target, compiler/build environment, lockfiles, tracked native source/vendor/config/workflow IDs, the SDK gitlink and frontend/icon content. A same-toolchain/environment/lockfile fallback can reuse dependency artifacts after a source change. Cargo, tests and Clippy always run and validate restored outputs; an exact target-cache hit never bypasses these checks. Source timestamps are not rewritten.
- Native caches are saved before rust-cache's post-job cleanup, including after compilation failures so completed dependencies can be reused. Installer bundles are removed only after the installer-upload step and excluded from the saved compiler outputs. A cache result is written to the job summary.

The PR's first optimized run failed before native compilation, in Prettier: two files had been formatted with 3.6.2 while the repository lockfile installs 3.9.10. Those files are reformatted with the installed version. This failure supplies no native cache-hit measurements. Fresh checkout timestamps, linker work and Cargo invalidation can still require compilation after a target restore; full output preservation is not a guarantee of a zero-build run. Cache storage and runner timing should be assessed from the next run.
