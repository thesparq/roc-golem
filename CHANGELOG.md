# Changelog

All notable changes to roc-golem are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.4.2] — 2026-11-16

### Fixed
- **Outbound HTTP Headers JSON Serialization**: Fixed a serialization format mismatch where `encodeHttpRequest` in `platform/Golem.roc` serialized `headers` as a JSON array of `{"name": "...", "value": "..."}` objects, while the host runtime expected a JSON object map (`{"Header": "Value"}`). Consequently, headers were silently dropped by the WASI HTTP builder. `encodeHttpRequest` now correctly serializes headers as a JSON object map.
- **Defensive Outbound HTTP Headers Parsing in Host**: Updated `perform_wasi_http_request` in `host/src/lib.rs` to support both JSON key-value maps and JSON arrays of `{"name": "...", "value": "..."}` objects as a defensive fallback, ensuring complete backward compatibility with older compiled guest components.

### Added
- **Export of `encodeHttpRequest`**: Exported `encodeHttpRequest : HttpRequest -> Str` directly in `platform/Golem.roc` under the HTTP primitives section.
- **Test Coverage**: Added host unit tests (`test_http_request_headers_object_map` and `test_http_request_headers_array_fallback`) in `host/src/lib.rs`, and guest serialization validation in `tests/effectful_agent.roc`.

## [1.4.1] — 2026-11-16

### Added
- **Dynamic HTTP Webhook Endpoints (`withAgentHttpMount`)**: Added `withAgentHttpMount : Agent(state), HttpMount -> Agent(state)` in `platform/Golem.roc` to attach dynamic HTTP API deployment mounts and webhook endpoints to `Agent` containers (e.g. from `Golem.agent`, `Golem.agentWith`, `Golem.ephemeral`) cleanly satisfying Golem host requirements for `createWebhook`.
- **Direct Type Exposure in `pf.Golem`**: Re-exported `HttpMount`, `HttpRequest`, `SecretHeader`, `SecretHandle`, `ToolDefinition`, `ToolParameter`, `ToolCall`, and `ToolResult` directly from `pf.Golem` module header exports so developers do not need to import both `pf.Golem` and `pf.Types`.
- **Roc Package Delimiter Safety & Prefixed Releases**: Added a nonce avoidance loop in `tooling/pack_platform` guaranteeing all release BLAKE3 hashes are strictly free of `__` delimiters (which Roc's package resolver treats as a prefix delimiter). Platform packaging now produces both canonical bare hash archives (`{hash}.tar.zst`) and human-readable prefixed archives (`roc-golem-{tag}__{hash}.tar.zst`) that Roc cleanly indexes under `<hash>` in `~/.cache/roc/packages/`.

## [1.4.0] — 2026-11-16

### Added
- **Effectful High-Level Agent & Tool API**: Enabled host effects (`now`, `rpcSchedule`, `httpRequest`, `getSecretHandle`, etc.) inside `Golem.agent`, `Golem.agentWith`, `Golem.ephemeral`, and `Tool(state)` closures without compiler type mismatches.
- **Effectful Tool Constructors**: Added `effectfulTool`, `effectfulToolJson`, `effectfulStatelessTool`, and `effectfulStatelessToolJson` allowing tools to return `Effect.Effect(Result(...))` with automatic host execution.
- **Host Effect Execution Primitive (`runEffect!`)**: Added `runEffect! : Effect.Effect(a) => a` in `platform/Golem.roc`, allowing agent authors to execute host effects directly within `onMessage` or custom tool handlers.
- **Pure Tool Backward Compatibility**: Pure tool constructors (`tool`, `toolJson`, `statefulTool`, `statelessTool`, `statelessToolJson`) remain 100% supported and seamlessly unify with effectful tools in heterogeneous tool lists.
- **Test Suite**: Added `tests/effectful_agent.roc` covering canonical agent construction with mixed pure/effectful tools and effectful `onMessage`.
- **Architecture Specification**: Added design specification in `docs/specs/20-effectful-high-level-agent-api.md`.

## [1.3.0] — 2026-11-16

### Added
- **Multi-Agent Distinct Bundling Combinators**: Added `bundle3`, `bundle4`, `bundle5`, and `bundleWith` pipeline chaining combinator in `platform/Golem.roc` to prevent Roc compiler monomorphization explosion when bundling multiple agents sharing identical closure signatures.
- **Standardized Numerical Parsing Module (`pf.Parse` & `pf.Golem`)**: Added bounds-checked integer parsing (`parseU64`, `parseU32`, `parseU16`, `parseU8`, `parseI64`, `parseI32`, `parseI16`, `parseI8`), float parsing (`parseF64`), and safe fallback helpers (`parseU64Or`, `parseU32Or`, `parseI64Or`, `parseI32Or`, `parseF64Or`) with overflow detection, ASCII digit parsing, whitespace trimming, and optional sign (`+`, `-`) handling.
- **Test Suites**: Added `tests/parse_numbers.roc` (62 tests) and `tests/multi_agent_bundle.roc` (9 tests).
- **Architecture Specification**: Added design and root-cause analysis specification in `docs/specs/19-multi-agent-bundling-and-numeric-parsing.md`.

## [1.2.1] — 2026-11-15

### Added
- Live verification of inter-agent RPC (`start_canary` calling `diagnostics-worker` over Component Model RPC).
- Live verification of scheduled invocation (`rpcSchedule` delayed rollback execution).
- Live verification of asynchronous fire-and-forget RPC (`rpcFireAndForget`).
- Added `schedule_rollback` and `fire_rollback` methods to canary release coordinator example.

### Fixed
- Fixed WasmRpc constructor and method parameter wrapping: parameters are now wrapped in `SchemaValueNode::RecordValue` trees as required by Golem Cloud 1.6.3 Component Model RPC.
- Fixed wasm32 C-ABI variant tag union offset (offset 12) for `RocResult<u64, RocStr>` in `rocFxCreatePromise`.
- Fixed `rocFxRpcSchedule` and `rocFxRpcScheduleBytes` to properly match on and propagate `schedule_invocation` results instead of discarding with `let _`.
- Fixed constructor input normalization across host and guest: treats empty tuple `()`, empty object `{}`, empty array `[]`, and `null` as empty configuration.
- Sanitized worker IDs to bare agent type name in `rocFxGetWorkerId` and RPC target resolution.
- Fixed Rust code formatting diffs in `host/src/lib.rs` to satisfy `cargo fmt --check`.

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

## [1.0.0] — 2026-11-08

### Added
- Initial GA release of the `roc-golem` platform.
- Golem Cloud Agent Guest Component Model 2.0.0 integration.
- Typed agent definitions and composable bundles (`Agent`, `bundle`, `bundle2`).
- Wasm-RPC, Key-Value, Blobstore, WASI HTTP, Database, and WebSocket effect primitives.
