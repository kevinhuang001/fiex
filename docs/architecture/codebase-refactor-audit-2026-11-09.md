# Fiex codebase structure and verification audit (2026-11-09)

> Baseline: Core main `a32c699`, SDK main `ab21f3a`, Official Plugins main `51da0f5`. This document is a structural/representative source inspection, **not** a line-by-line proof that the full codebase is free of legacy code. No full local multi-platform build was run as part of this audit.

## Existing architectural strengths

- Core already uses the `crates/foundation`, `crates/builtins`, and `crates/platform` workspace layout. Retain these boundaries.
- The provider-contracts crate owns shared typed event/FS DTOs; the runtime uses typed provider registries.
- The resource broker, operation scope, directory mutation overlay, preflight and plugin hosting code already represent meaningful domain boundaries.
- CI runs browser E2E, frontend tests, Core Rust workspace tests and platform-matrix Clippy; do not replace working coverage in one large PR.

## Verified findings

| ID | Priority | Evidence | Recommended change |
| --- | --- | --- | --- |
| ARC-001 | P0 | `src-tauri/src/usbfs/mod.rs` declares only `mod tree` and reexports `fiex_usb_fs`. Six files from the previous local driver implementation are not declared as Rust modules. | Remove the six unreachable files; retain `src-tauri/src/usbfs/tree.rs` as the Core adaptation layer and the new crate as the driver owner. |
| ARC-002 | P0 | `scripts/check-file-size.mjs` scans only `src` and `src-tauri/src`; `npm run check:size` is missing from the Core CI workflow. `scripts/file-size-baseline.json` refers to removed `src/lib/demo/handlers/plugins.ts`. | Extend coverage to all owned code, create a measured non-growth baseline, run it in CI, and gradually eliminate exceptions. Do **not** enable the existing script in CI unchanged: it misses crates and currently has unbaselined oversized modules. |
| ARC-003 | P0 | SDK CI invokes `cargo test --manifest-path rust/Cargo.toml` but does not explicitly test the separate `rust/contracts/Cargo.toml` package, which has matcher unit tests. | Add contracts fmt/test/Clippy jobs. Track this in the SDK PR. |
| ARC-004 | P1 | `src/hooks/explorer/useListing.ts` (~638 lines), `src-tauri/src/core/fs/preflight.rs` (~497 lines), `src-tauri/src/plugins/resource/fs_access/scope.rs` (~438 lines), `src-tauri/src/core/volumes/mounts.rs` (~688 lines), SDK `rust/src/sdk_tests.rs` (~959 lines), Archive `official/archive/src/main.rs` (~766 lines). | Extract policy, state transitions, orchestration and I/O into cohesive modules; never split solely by line count. |
| ARC-005 | P1 | Core `tsconfig.json` uses `strict: true` but `allowJs: true, checkJs: false`; `src/lib/api.ts` exposes unvalidated generic `call<T>(command: string, args)`. | Generate IPC DTOs from Rust contracts and introduce a typed command map + runtime validation for untrusted responses. Avoid inaccurate claims that `call<T>` is runtime safe. |
| ARC-006 | P1 | `src-tauri/src/plugins/resource/fs_access/scope.rs` exposes JSON-value conversions at the authorization boundary; `src-tauri/src/core/fs/mod.rs` invokes serde deserialization in `ResourceBroker::resolve`. | Use typed validated resource identifiers internally; separate parse/bind/authorize/execute/revoke phases; verify non-escalation and stale-ref invariants. |
| ARC-007 | P1 | UI unit tests are primarily utility `.test.ts` files, while `e2e/explorer.spec.ts` and `e2e/fixtures/handlers/plugins.ts` are very large; official plugins place integration executables in `integration/examples`. | Add component, hook, contract, process and cross-platform boundary tests; reorganize large fixtures by feature. Retain legitimate Rust `#[cfg(test)]` colocated tests. |
| ARC-008 | P1 | No explicit Kani, Verus, coverage, or property-testing stage in current three-workspace CI configuration. | Add pure core invariants with Kani, property-based generators, filesystem integration tests, then coverage/mutation quality gates. |
| ARC-009 | P2 | Rust manifests use edition 2021 with `rust-version = 1.90`. | Trial edition 2024 with compiler and dependency checks, then migrate workspace-by-workspace; the edition itself is not a bug. |

## Target ownership (no unnecessary crate proliferation)

```text
fiex/
  src/
    app/                 # composition / main application
    features/
      explorer/          # view, listing state, actions, feature tests
      transfers/         # progress, conflicts, controller
      plugins/           # panels, marketplace, settings
      settings/
    shared/
      ipc/               # generated types, typed call wrapper, runtime guards
      ui/
      i18n/
      cache/
    test/                # reusable Vitest harness and fixtures
  e2e/
    explorer/
    transfers/
    plugins/
    builtins/
    fixtures/            # isolated, deterministic mocked host
  src-tauri/
    crates/
      foundation/        # existing fiex-resource/fs/storage/runtime
      builtins/          # existing applications/recent/trash/etc.
      platform/          # native OS calls
      application/       # extract ONLY acyclic policy/operations/cache components
    src/
      core/              # application orchestration while migration continues
      plugins/           # provider lifecycle / host orchestration
      desktop/           # Tauri command adapters, no domain policy
      usbfs/             # Core-only display/ejection adapter, no driver copies
```

The external `fiex-sdk` continues to be the canonical shared contract source, and `fiex-official-plugins` stays independent. Never copy contract DTO definitions into multiple hand-maintained locations.

### Rust boundaries

1. **Domain/contracts:** immutable typed references and data; pure functions without `tauri`, subprocesses or global runtime state.
2. **Application:** operations, scopes, preflight, conflict resolution and transaction state machines. Dependencies point inward only.
3. **Infrastructure:** filesystem, IPC, remote provider adapters, caches, process runners; implement domain-facing traits.
4. **Presentation:** Tauri commands and React app adapters. No storage permission decisions here.

Use `pub(crate)` or private modules by default. Prefer newtypes for `MountId`, `ResourceId`, `SessionId` and validated `Uri`; enums for protocol states, `Result` with typed errors, sealed capabilities where useful, and explicit cancellation. Keep generics/typestate where they enforce an invariant; avoid ornamental abstraction or broad `dyn Any` registries.

### TypeScript boundaries

- Keep `strict`, phase in `noUncheckedIndexedAccess` and `exactOptionalPropertyTypes` after measuring failures.
- Generate command/event DTOs from Rust Serde contracts (evaluate `ts-rs` or equivalent); use runtime validation where external IPC or plugin messages enter the process.
- Replace unbounded `call<T>(string)` in feature code with typed methods; allow dynamic custom methods only in a quarantined, validated SDK bridge.
- Prefer discriminated unions, exhaustive switches, `satisfies`, narrow readonly domain values, and generic helpers where they remove casts.
- Split `useListing` by read pipeline, event subscription, mutation reconciliation and presentation; preserve the existing cache-before-revalidate no-flicker semantics.

## Test placement policy

- Rust private unit tests: `src/<module>/tests.rs` and `#[cfg(test)] mod tests;`; valid, idiomatic and not inherently misplaced.
- Rust external contract and black-box tests: `crates/<crate>/tests/*.rs`, `sdk/rust/contracts/tests/*.rs`, or `src-tauri/tests/*.rs` where an executable host is required. Only move tests once public/private access constraints are understood.
- TypeScript pure-unit and React tests: colocated `*.test.ts(x)`; shared test setup under `src/test`; Playwright cross-feature tests under `e2e/<feature>/*.spec.ts`.
- Official integration harnesses: dedicated `integration/tests` for cargo test-compatible cases, or `integration/src/bin` for environment-driven standalone binaries, not falsely counted as ordinary unit tests.

## Required invariants and proof plan

| Target | Invariant | Technique |
| --- | --- | --- |
| Scope / capability policy | authorization cannot be amplified by bind/derive/revoke; stale/revoked references stay unusable | Pure transition model + Kani bounded harnesses, followed by integration tests |
| URI / ResourceRef | normalized child path never escapes root; parse/serialize round trips | proptest, then Kani or Verus for a pure parser/normalizer |
| Write session | at most one commit, no write after abort, uncommitted cleanup | typestate + property/model checks |
| Cache overlay | stale list may not erase a confirmed local mutation before reconciliation; identity changes invalidate snapshots | deterministic state-machine/property tests |
| Transfer planner | no source deletion before destination commit, error always triggers cleanup | property tests + fault-injected process integration tests |
| JSON-RPC | unique request IDs and correct response attribution; no silent permission fallback | protocol conformance/mutation tests; concurrency harnesses outside Kani |
| Provider registry | only registered typed capabilities are invoked | contract tests + invalid/absent-provider cases |

Kani is suitable for **bounded, pure** state and safety checks, but does not replace multi-threaded integration or filesystem race tests. Verus should be explored on one pure core algorithm, not applied wholesale to the Tauri/runtime code.

## Rollout (small, independently verifiable PRs)

1. **P0 hygiene:** delete proved-unreachable copied sources, fix SDK contracts CI, remove stale baselines; do not modify user-visible functionality.
2. **Quality gates:** implement full-workspace source-size non-regression check and baseline; add frontend lint and tests for hooks/IPC; split E2E fixtures without changing assertions.
3. **Type contracts:** introduce generated DTOs and typed RPC facade; introduce internal newtypes; migrate one domain at a time with snapshot/contract tests.
4. **Domain extraction:** pull pure scope policy, transfer planner and cache reconciliation into independently testable modules/crates; enforce dependency rules.
5. **Verification:** start with 2-4 Kani harnesses for scope and write state machine; add proptest, deterministic failures, coverage and mutation testing.
6. **Edition/toolchain and final cleanup:** test edition 2024, remove adapters only when call sites are migrated, delete unused dependencies/examples, and remove file-size exceptions.

### Merge gate for every follow-up PR

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, appropriate `cargo test` and SDK contracts tests, `npm run build`, `npm test`, targeted Playwright, platform matrix, and a diff review for legacy paths. Do not claim these passed unless jobs ran. Keep functional behavior stable until specific contract/integration coverage proves the replacement.
