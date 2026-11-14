# Changelog

All notable changes to roc-golem are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.0] — 2026-11-14

### Added
- Real wasm32 host effect implementations for `createPromise`, `blobPut`, and `rpcSchedule`.
- `HttpRequest` typed record and `Golem.httpRequest` for strongly-typed HTTP dispatch.
- Tool parameter helpers: `numberParam`, `boolParam`, and `typedParam`.
- `bundleAll` helper function for bundling arbitrary lists of type-erased agents.
- Comprehensive test coverage for `spec_builder` and `bundle` composition primitives (`tests/spec_builder.roc`, `tests/bundle.roc`).
- Control character escaping support and test suite for `Json.escape` (U+0000..U+001F, `\b`, `\f`).
- Opaque nominal union `SecretHandle` preventing raw handle leakage.
- Security hardening in `normalize_path` preventing path traversal (`..` rejection).
- Human-readable error message formatters for RPC, WebSocket, SQL Database, and Config operations.

### Changed
- Standardized C-ABI argument ordering with out-pointer at position 0 across all host effect exports.
- `RocStr` clone on host now increments the atomic reference count instead of performing deep copies.
- 1-based indexing unified across all resource handle tables (WebSocket, Promises, Webhooks, Streams).
- Removed automatic `roc fmt` mutation from `build.sh` application build pipeline.
- Improved `test_consumer.sh` archive resolution using glob pattern detection.
- Documented Golem Cloud wasm32 limitation on `getOplogIndex` and `revertAgent` with explicit developer notices.

### Fixed
- Fixed `agentWith` initialization error propagation so invalid JSON configs surface actionable errors.
- Fixed `build` path missing-handler error diagnostics.
- Updated `wit/world.wit` Golem API dependency packages to `@1.6.x`.
- Fixed `examples/counter/main.roc` to use JSON serialization for counter states.
- Fixed `examples/canary_release/ReleaseCoordinator.roc` to schedule rollback against self using `Golem.workerId`.
- Added `/.audit-*/` to `.gitignore` to prevent committing local audit snapshots.

## [1.1.0] — 2026-11-12

### Added
- Golem Cloud 1.6.3 async Component Model upgrade with `[async-lift]` and `[callback][async-lift]` exports.
- Pinned `wit-bindgen` to commit `c0eb9d2ff0dac16cbf04f6206fc9e86cfa70f5bd`.
- Wasm32-wasip2 target compilation support and bundled `libc.a`.
- Upgraded `wasm-tools` support to 1.263.0.
- Automated consumer verification suite in `tooling/test_consumer.sh`.

## [1.0.0] — 2026-10-31

### Added
- Initial GA release of the `roc-golem` platform.
- Golem Cloud Agent Guest Component Model 2.0.0 integration.
- Typed agent definitions and composable bundles (`Agent`, `bundle`, `bundle2`).
- Wasm-RPC, Key-Value, Blobstore, WASI HTTP, Database, and WebSocket effect primitives.
