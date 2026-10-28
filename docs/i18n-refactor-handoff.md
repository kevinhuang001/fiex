# i18n and modularization handoff

The latest instruction is to submit the changes without further tests. No test, typecheck, build, lint, or CI wait was started after that instruction. Formatting and static code review continued. The final commits are not a claim that the complete project acceptance criteria have passed.

## Changes delivered

- SDK native source catalogs use the captured request/task locale, with source fallback and single-pass placeholder interpolation.
- All 15 official manifests have English and Traditional Chinese display catalogs. Shared resources cover React panels, standalone Archive/Data/Office HTML panels, SSH status copy and native source-message call sites. The checker covers duplicate keys, placeholders, missing and unused translations, JSX/attributes and native Chinese display messages.
- HTML previews use the SDK bridge and opaque ResourceRefs instead of names. Archive password flow uses a response flag. Drive reauthorization uses a structured error reason instead of an English substring.
- Core native display messages and format templates use the existing i18n module; the frontend checker also detects duplicate catalog keys and hardcoded JSX copy.
- Desktop file/plugin commands, remote listing/download/cache/upload/store, VFS exports/mutations, mount URIs/lifecycle, transfer execution, marketplace descriptions/catalogs, process startup/invocation/lifecycle, platform sandbox launchers, USB filesystem adapters, terminal IDE integration and tests are separated by responsibility.
- The terminal view belongs to the terminal domain. The obsolete demo SQLite provider/protocol and duplicate unreachable GPG delete branch are removed.

## Remaining work and limits

The entire original brief is **not yet fully audited or accepted**. In particular, the remaining frontend modules below, demo display/fixture boundaries, English native diagnostic errors, and a full Provider/Resolver call-chain review need follow-up. Fixture file names, table contents and user data must remain untranslated.

The final edits have not been tested. Cross-platform desktop compilation, live locale switching in real plugin panels, Google OAuth/token behavior, FS discovery, sandbox/process communication and the final CI result are for the user's testing pass. Previous passing checks were against earlier intermediate trees, not the final commit.

## Files still over 300 lines

| File                                                    | Lines | Reason/status                                                     |
| ------------------------------------------------------- | ----: | ----------------------------------------------------------------- |
| `src-tauri/src/core/volumes/mounts.rs`                  |   478 | Existing module; further responsibility-based extraction remains. |
| `src-tauri/src/plugins/resolvers/context_menu/tests.rs` |   447 | Test cases; explicit size exception.                              |
| `src/components/ConnectionDialog.tsx`                   |   337 | Existing module; further responsibility-based extraction remains. |
| `src/components/Preview.tsx`                            |   442 | Existing module; further responsibility-based extraction remains. |
| `src/components/explorer/ExplorerToolbar.tsx`           |   326 | Existing module; further responsibility-based extraction remains. |
| `src/components/file-picker/usePicker.ts`               |   445 | Existing module; further responsibility-based extraction remains. |
| `src/components/plugins/panel/usePanelBridge.ts`        |   423 | Existing module; further responsibility-based extraction remains. |
| `src/components/settings/SidebarSettings.tsx`           |   326 | Existing module; further responsibility-based extraction remains. |
| `src/hooks/explorer/useListing.ts`                      |   329 | Existing module; further responsibility-based extraction remains. |
| `src/hooks/useAppController.tsx`                        |   438 | Existing module; further responsibility-based extraction remains. |
| `src/lib/demo/handlers/plugins.ts`                      |   800 | Existing module; further responsibility-based extraction remains. |
