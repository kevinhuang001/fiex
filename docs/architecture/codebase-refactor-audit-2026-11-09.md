# Fiex codebase structure and verification audit (2026-11-09)

Baseline: Core `a32c699`, SDK `ab21f3a`, Official Plugins `51da0f5`.

This is a structural and representative source audit, **not** a line-by-line proof that all legacy code has been eliminated. Full local cross-platform builds were not run during the audit.

## Existing strengths

- Core already separates `crates/foundation`, `crates/builtins`, and `crates/platform`.
- The SDK contracts crate owns typed event and file-operation DTOs.
- ProviderRegistry, ResourceBroker, Operation Scope, preflight and directory mutation overlay already provide useful boundaries.
- The current CI already runs frontend tests, browser E2E, Rust workspace tests and platform-matrix Clippy.

These structures should be retained rather than replaced wholesale.

## Confirmed findings

### ARC-001: Unreachable USB driver copies (P0)

`src-tauri/src/usbfs/mod.rs` declares only `mod tree` and reexports `fiex_usb_fs`. Six files from the pre-extraction implementation were not declared Rust modules:

- `src-tauri/src/usbfs/access.rs`
- `src-tauri/src/usbfs/identity.rs`
- `src-tauri/src/usbfs/mount.rs`
- `src-tauri/src/usbfs/util.rs`
- `src-tauri/src/usbfs/tree/drivers.rs`
- `src-tauri/src/usbfs/tree/stream.rs`

Remove these copies. Retain `src-tauri/src/usbfs/tree.rs` as a Core adapter and the dedicated `fiex-usb-fs` crate as the driver owner.

### ARC-002: Missing file-size enforcement (P0)

`scripts/check-file-size.mjs` scans only `src` and `src-tauri/src`, excluding extracted Rust crates, SDK and official plugins. Core CI does not invoke `npm run check:size`. The old baseline also contained `src/lib/demo/handlers/plugins.ts`, which no longer exists.

Extend coverage, establish a measured non-growth baseline and enforce it in CI. **Do not enable the existing script unchanged:** it contains unbaselined oversized modules and would block unrelated PRs.

### ARC-003: SDK contracts tests omitted from the declared matrix (P0)

SDK CI tests `rust/Cargo.toml` but does not explicitly test the separate `rust/contracts/Cargo.toml` package. The latter contains ProviderMatcher unit tests. Add its own fmt, test and Clippy checks to the SDK pipeline.

### ARC-004: Overloaded modules (P1)

Examples include `src/hooks/explorer/useListing.ts` (approximately 638 lines), `src-tauri/src/core/fs/preflight.rs` (497 lines), `src-tauri/src/plugins/resource/fs_access/scope.rs` (438 lines), `src-tauri/src/core/volumes/mounts.rs` (688 lines), SDK `rust/src/sdk_tests.rs` (959 lines) and Archive `official/archive/src/main.rs` (766 lines).

Split by responsibility, state machine and I/O ownership; do not split merely to satisfy a line limit.

### ARC-005: Cross-language type-safety gap (P1)

Core uses TypeScript `strict: true`, but `allowJs: true` and `checkJs: false`. `src/lib/api.ts` also exposes `call<T>(command: string, args)`, which allows callers to claim an arbitrary return type without validating the runtime response.

Generate IPC DTOs from Rust Serde contracts, introduce a typed command map and validate untrusted IPC/plugin data at the actual boundary. Keep dynamic plugin RPC behind an explicit validated adapter.

### ARC-006: Resource and scope invariants (P1)

`plugins/resource/fs_access/scope.rs` handles resource binding, path authorization and references. `core/fs/mod.rs` converts JSON values into `ResourceRef` during broker resolution.

Move pure authorization predicates and state transitions into testable modules. Prefer typed validated resource identifiers after the wire boundary. Prove that binding, derivation, revocation and stale references cannot amplify authority.

### ARC-007: Test coverage and placement (P1)

Frontend tests are mostly pure utility tests. `e2e/explorer.spec.ts` and `e2e/fixtures/handlers/plugins.ts` are large. Official plugin integration executables live in `integration/examples`; this is not equivalent to cargo test discovering an integration test.

Keep idiomatic Rust colocated `#[cfg(test)]` tests. Organize black-box tests separately and add React hook/component, IPC protocol, provider lifecycle, failure injection and OS-boundary tests.

### ARC-008: No formal-verification pipeline (P1)

Current workflows do not explicitly invoke Kani, Verus, property testing or coverage collection. Add a small set of pure proof targets before trying to verify host-wide I/O or concurrency.

### ARC-009: Rust edition (P2)

Manifests use edition 2021 with Rust 1.90. Trial migration to edition 2024 after checking macros, dependencies, warnings and every platform. Edition 2021 is not itself a defect.

## Proposed ownership

```text
fiex/
  src/
    app/                 # application composition
    features/
      explorer/
      transfers/
      plugins/
      settings/
    shared/
      ipc/               # typed command facade and generated DTOs
      cache/
      i18n/
      ui/
    test/                # shared unit/component fixtures
  e2e/
    explorer/
    transfers/
    plugins/
    builtins/
    fixtures/
  src-tauri/
    crates/
      foundation/        # keep existing resource/fs/storage/runtime
      builtins/          # keep existing built-in providers
      platform/          # keep native OS implementations
      application/       # only acyclic pure policy/operations/cache
    src/
      core/              # orchestration during gradual migration
      plugins/           # plugin runtime and provider host
      desktop/           # Tauri adapters, no domain authorization
      usbfs/             # display and ejection adapter only
```

The SDK is the canonical source for shared contracts. Official plugins must remain independent. Do not introduce unnecessary crates or cyclic dependencies.

## Type and module conventions

- Rust: prefer private modules and `pub(crate)` over broad `pub`. Use newtypes for identifiers, typed errors, enums for lifecycle states, explicit cancellation and generics when they enforce invariants.
- TypeScript: keep `strict`; incrementally enable `noUncheckedIndexedAccess` and `exactOptionalPropertyTypes`; prefer discriminated unions, `satisfies`, exhaustive switches and readonly domain values.
- Generate shared types from Rust and validate at runtime. A TypeScript generic return annotation is not runtime validation.
- Separate directory fetch, event subscriptions, mutation reconciliation and presentation in `useListing`; preserve cache-before-revalidate behavior and avoid flicker.
- Do not convert one monolith into another global utility crate.

## Test layout policy

- Rust private unit tests: colocated `src/<module>/tests.rs` under `#[cfg(test)] mod tests;` is idiomatic.
- Rust black-box tests: package-specific `tests/*.rs` with public contracts, or local test modules where private access is required.
- TypeScript unit and component tests: colocated `*.test.ts(x)` with setup and shared fixtures in `src/test`.
- Browser E2E: `e2e/<feature>/*.spec.ts` with per-test isolated fixtures.
- Official plugin integration: use `integration/tests` for test-harness-compatible cases, or `integration/src/bin` for environment-driven standalone binaries.

## Formal and property verification targets

- **Scope and capability:** binding, delegation and revocation never amplify authorization.
- **Resource identity:** path normalization never escapes the trusted root and stale refs do not regain access.
- **Write session:** no write after abort, no double commit, no leftover uncommitted state.
- **Directory overlay:** stale provider listings do not erase committed local mutations before reconciliation.
- **Transfer planner:** source cleanup cannot precede successful destination commit.
- **JSON-RPC:** responses belong to the correct request; missing capability cannot trigger a privileged fallback.

For pure, bounded Rust state machines, use Kani proof harnesses and property tests. For a small pure normalization algorithm, evaluate Verus as a separate experiment. Kani does not replace filesystem race or multithreaded integration testing.

## Implementation sequence

1. **Hygiene:** delete proven-unreachable code, remove stale baselines, and explicitly test the SDK contracts package.
2. **Gates:** expand file-size checking, add frontend lint and component/hook tests, and isolate E2E fixtures.
3. **Contracts:** generate typed IPC models, validate wire inputs and move to typed internal reference/state representations.
4. **Extraction:** isolate policy, transfer planning and cache reconciliation with explicit dependency directions.
5. **Verification:** add Kani harnesses, property tests, injected failures, coverage and mutation testing.
6. **Final cleanup:** trial edition 2024, remove unused dependencies and obsolete branches, and retire size exemptions.

Every subsequent PR should pass relevant `fmt`, `clippy`, `test`, frontend build, Playwright and native platform checks. Do not claim a gate passed until it has actually run.
