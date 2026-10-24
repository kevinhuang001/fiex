# VFS Contribution / Connection routing refactor validation

## Result

- Core generates and resolves `fiex://mount/<id>/` identities. Standalone trees explicitly declare `contributions.vfs`; connection factories implicitly provide their own instance tree. Toolbar declarations do not confer provider eligibility.
- Git creates immutable, lightweight per-commit views on demand. There is no startup repository discovery, full-tree snapshot, or connection sync for Git.
- Connection navigation paths are distinct from cache paths. Metadata listing, offline pins, exact-file snapshot uploads and external materialization retain the existing Core pipeline.
- Removed manifest/category `capabilities` and the connection `sync` / `writeMode` / `conflictControl` switches. Core and all four official connection providers use `fs.stat` and `fs.writeFile`. Google Drive keeps resumable HTTP chunking inside its provider.
- IPC invocation checks provider eligibility and rejects public direct provider calls. Owner/contribution/resource scopes, mount-bound entries, URI validation and revocation are enforced in Core.

## Local validation

| Check                                                                                     | Result                                                                                         |
| ----------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| Core `cargo test --no-default-features --lib`                                             | 94 passed                                                                                      |
| Core no-default-features all-targets clippy, warnings denied                              | Passed                                                                                         |
| Core default desktop `cargo check` (Rust 1.92, native sysroot)                            | Passed                                                                                         |
| SDK Rust tests and doctest                                                                | 25 + 1 passed                                                                                  |
| SDK all-targets clippy                                                                    | Passed                                                                                         |
| Frontend Vitest                                                                           | 32 passed                                                                                      |
| Frontend TypeScript/Vite build                                                            | Passed                                                                                         |
| Git UI TypeScript/Vite build                                                              | Passed                                                                                         |
| Git Rust unit tests / clippy                                                              | Passed                                                                                         |
| Drive Rust unit tests / clippy                                                            | Passed                                                                                         |
| Real Git process with deterministic host broker                                           | Passed: independent commits, binary content, opaque nodes, Unicode filenames, invalid contexts |
| Real Drive process with deterministic HTTP/file broker                                    | Passed: reads, bounded upload, empty files, stale revisions, precommit conflicts, cleanup      |
| Real WebDAV/OpenList processes with deterministic HTTP broker                             | Passed: offline retry, version guards, exact reader handle uploads                             |
| Rust formatting for Core, SDK, Git, Drive and integration; changed TS/Markdown formatting | Passed                                                                                         |

Tests in `scripts/test-git-vfs.py` explicitly use a deterministic host broker; they do not replace Core sandbox authorization integration. The updated `integration/examples/plugin_git_e2e.rs` retains the actual Core-host test with multi-commit and permission-negative cases, and the existing live WebDAV/SMB CI jobs remain enabled against the matching Core commit.

## Environment limits and remaining constraints

- Local actual Core-host plugin execution was blocked by the environment: Landlock reported `ruleset not enforced` and the child closed its control channel. No sandbox bypass was added. Actual Core-host and live SMB/WebDAV end-to-end success is not claimed.
- Browser E2E could not start because the Chromium download returned invalid/truncated archives. Frontend tests/build passed; browser E2E success is not claimed.
- Windows/macOS runtime checks remain for CI. No OS-level filesystem mounting is implemented.
- Standalone VFS exports use the existing bounded `readSmall` contract: complete files up to 1 MiB, a maximum of 64 temporary exports, explicit errors for larger files, cleanup at normal application exit. Connection large-file streaming is unaffected.
- The existing live SMB write Access Denied issue is not hidden by this refactor; no SMB write implementation was changed beyond removing redundant manifest metadata.

## Workflow

Applied codex-token-saver through focused symbol searches, bounded logs, reused source context and targeted verification. No token-savings percentage is claimed.
