# Specification: Unit 17 — Platform Correctness, Safety Hardening and DX Polish

- **Unit:** 17
- **File:** `docs/specs/17-platform-correctness-and-hardening.md`
- **Status:** Planned
- **Phase:** Phase 5 — Hardening & Ecosystem
- **Findings:** `KI-26` through `KI-55`

---

## 1. Goal

Address all findings from the post-v1.0.0 platform review across five areas:

1. **Critical Correctness (`KI-26`–`KI-35`)**: Fix silent failures and stub implementations in the Rust host that masquerade as working features on live Golem deployments — including unimplemented `createPromise`, broken `blobPut`, constant `getOplogIndex`, and no-op `rpcSchedule` on `wasm32`, plus a JSON escape gap that produces invalid envelopes.
2. **Safety Hardening (`KI-36`–`KI-42`)**: Fix a `RocStr` reference-count model mismatch, make `SecretHandle` opaque, unify handle-slot conventions across all resource tables, audit the C-ABI argument ordering for all `#[no_mangle]` exports, block path traversal in `normalize_path`, and add a `GlobalCell` thread-safety comment.
3. **Test Coverage (`KI-43`–`KI-50`)**: Extend the Rust unit test suite to cover all host effect functions, add Roc tests for the `define/build/register*` path and multi-agent `bundle` routing, remove the `|| [ $? -eq 2 ]` CI bypass that silently swallows failing `expect` blocks.
4. **DX Improvements (`KI-51`–`KI-56`)**: Group the `Golem.roc` export list by domain, add an `HttpRequest` record type, add `numberParam`/`boolParam`/`typedParam` helpers, improve RPC documentation, fix error message formatting in the host, and annotate the double-encode state protocol.
5. **Tooling & Housekeeping (`KI-57`–`KI-64`)**: Add `.gitignore`, remove the mutating `roc fmt` from `build.sh`, fix the `test_consumer.sh` archive-glob default, add `CHANGELOG.md`, fix the `Cargo.toml` author email, align `wit/world.wit` to Golem 1.6.x, add a `bundleAll` helper, and add `golem.yaml` naming comments.

---

## 2. Design & Architecture

### 2.1 Critical Correctness

#### 2.1.1 Real `createPromise` on wasm32 (`KI-26`)

`rocFxCreatePromise` currently writes to `MOCK_PROMISES` on both `wasm32` and native targets. The real Golem promise API (`golem::api::host::create_promise`) is never called, so agents using `Golem.createPromise` on a live server always receive a fake local promise ID that cannot be fulfilled externally.

```rust
// Before (both targets use the mock):
let handle = MOCK_PROMISES.with(|m| { ... mock insert ... });
ptr::write(out, RocResult::ok(handle));

// After — wasm32 path calls real API:
#[cfg(target_arch = "wasm32")]
{
    match golem::api::host::create_promise() {
        Ok(promise_id) => {
            let handle = PROMISE_TABLE.with(|t| {
                let mut slots = t.borrow_mut();
                slots.push(Some(promise_id));
                slots.len() as u64
            });
            ptr::write(out, RocResult::ok(handle));
        }
        Err(e) => {
            ptr::write(out, RocResult::err(RocStr::from_str(&format!("{}", e))));
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
{ /* existing mock */ }
```

The same split-cfg pattern applies to `rocFxGetPromise` and `rocFxCompletePromise`.

#### 2.1.2 Real `blob_put_wasm32` (`KI-27`)

`blob_put_wasm32` unconditionally returns `Err("blob_put not supported in async component model")`. The WASI blobstore spec supports synchronous writes. The correct implementation:

```rust
fn blob_put_wasm32(container_name: &str, object_name: &str, bytes: &[u8]) -> Result<(), String> {
    let c_name = container_name.to_string();
    let container = wasi::blobstore::blobstore::get_container(&c_name)
        .or_else(|_| wasi::blobstore::blobstore::create_container(&c_name))
        .map_err(|e| format!("{:?}", e))?;
    let outgoing = wasi::blobstore::types::OutgoingValue::new_outgoing_value();
    outgoing
        .outgoing_value_write_body_sync(&bytes.to_vec())
        .map_err(|e| format!("{:?}", e))?;
    let o_name = object_name.to_string();
    container
        .write_object(&o_name, &outgoing)
        .map_err(|e| format!("{:?}", e))
}
```

#### 2.1.3 Real `rocFxGetOplogIndex` on wasm32 (`KI-28`)

Currently always returns `0` on wasm32. Replace with:

```rust
#[cfg(target_arch = "wasm32")]
{
    golem::api::host::get_oplog_index()
}
```

#### 2.1.4 Real `rocFxRpcSchedule` on wasm32 (`KI-29`)

The string-only `rocFxRpcSchedule` returns a JSON stub instead of calling `rpc_client.schedule_invocation`. Mirror the existing `rocFxRpcScheduleBytes` implementation:

```rust
#[cfg(target_arch = "wasm32")]
{
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let (sched_secs, sched_nanos) =
        compute_scheduled_time(now.as_secs(), now.subsec_nanos(), delay_millis);
    let target_datetime = golem::agent::host::Datetime {
        seconds: sched_secs as i64,
        nanoseconds: sched_nanos,
    };
    let constructor_val = wrap_string_data_value(c.to_string());
    let rpc_client = golem::agent::host::WasmRpc::new(t, constructor_val, None, Vec::new());
    let input_val = wrap_string_data_value(p.to_string());
    let _ = rpc_client.schedule_invocation(target_datetime, f, input_val, None);
    let resp = serde_json::json!({ "status": "scheduled", "target": t, "method": f }).to_string();
    ptr::write(out, RocResult::ok(RocStr::from_str(&resp)));
}
```

#### 2.1.5 JSON escape — control characters U+0000–U+001F (`KI-30`)

`platform/Json.roc`'s `escape` function handles `\`, `"`, `\n`, `\r`, `\t` but silently passes all other ASCII control characters (U+0000–U+001C), producing RFC 8259-invalid JSON. Add a `Str.walk_utf8` pass to `\uXXXX`-escape any remaining bytes in `0x00`–`0x1F`:

```roc
escape : Str -> Str
escape = |input|
    input
        |> Str.replace_each("\\", "\\\\")
        |> Str.replace_each("\"", "\\\"")
        |> Str.replace_each("\n", "\\n")
        |> Str.replace_each("\r", "\\r")
        |> Str.replace_each("\t", "\\t")
        |> escapeControlChars

escapeControlChars : Str -> Str
escapeControlChars = |s|
    # Walk bytes; any codepoint < 0x20 not already handled gets \uXXXX encoding.
    # Implementation: iterate with Str.to_utf8, map, Str.from_utf8.
    ...
```

#### 2.1.6 `agentWith` init must propagate `Err` on bad config (`KI-31`)

```roc
# Before:
Err(_) => Ok((cfg.serializeState)(cfg.initial))

# After:
Err(e) => Err("init failed to parse config: ${e}")
```

#### 2.1.7 `build` path — surface error when no `handle-message` handler is registered (`KI-32`)

```roc
# Before:
Err(_) => Ok({ nextStateJson: "", stateChanged: False, response: "Agent received: ${msgStr}" })

# After:
Err(_) => Err("No handle-message handler registered. Use Golem.agent/simpleAgent or register one with registerEffectful.")
```

#### 2.1.8 `extract_json_field` — handle escaped quotes and non-string values (`KI-33`)

Replace the single-split fallback parser with a minimal value extractor that skips whitespace and correctly handles `\"` inside string values. The simplest safe fix is to treat any parse failure as `Err(NotFound)` and log it rather than producing a silently truncated value.

#### 2.1.9 `wit/world.wit` — update Golem API imports to `@1.6.x` (`KI-34`)

```wit
# Before:
import golem:api/host@1.5.0;
import golem:api/oplog@1.5.0;
import golem:api/context@1.5.0;
import golem:api/retry@1.5.0;

# After (match golem-schema = "1.6.3"):
import golem:api/host@1.6.0;
import golem:api/oplog@1.6.0;
import golem:api/context@1.6.0;
import golem:api/retry@1.6.0;
```

Verify the correct minor version suffix from the official Golem 1.6 WIT sources and re-run `wasm-tools validate` on all four component WASMs.

#### 2.1.10 `counter/main.roc` — `Str.inspect` on integers produces strings, not JSON numbers (`KI-35`)

```roc
# Before:
Ok("{\"count\":${Str.inspect(state.count)}}")
# Produces: {"count":"42"} (string — wrong)

# After:
Ok(Json.encode({ count: state.count }))
# Or: Ok("{\"count\":${Num.to_str(state.count)}}")
# Produces: {"count":42} (number — correct)
```

Apply the same fix to any other hand-crafted JSON in examples that uses `Str.inspect` on numeric types.

---

### 2.2 Safety Hardening

#### 2.2.1 `RocStr` clone — refcount increment instead of deep copy (`KI-36`)

The current `Clone` impl for large `RocStr` calls `RocStr::from_str(self.as_str())`, allocating a fresh heap buffer. This breaks Roc's shared-ownership assumption and makes every RPC payload copy O(n).

```rust
// After:
impl Clone for RocStr {
    fn clone(&self) -> Self {
        if self.is_small() {
            Self { words: self.words }
        } else {
            // Increment refcount header
            unsafe {
                let header_size = mem::size_of::<isize>();
                let data_ptr = self.words[0] as *mut u8;
                if !data_ptr.is_null() {
                    let ref_count = data_ptr.sub(header_size) as *mut isize;
                    *ref_count += 1;
                }
            }
            Self { words: self.words }
        }
    }
}
```

Add a matching `#[test]` that verifies two clones share the same allocation (pointer equality on the data word).

#### 2.2.2 `SecretHandle` — opaque type (`KI-37`)

```roc
# Before (platform/Types.roc):
SecretHandle : U32

# After:
SecretHandle := U32
```

Add `SecretHandle.unwrap : SecretHandle -> U32` for host-internal use and ensure all external-facing functions accept/return `SecretHandle`, not `U32`.

#### 2.2.3 Unify 1-based handle convention across all resource tables (`KI-38`)

Secret table uses 1-based handles (slot + 1). WebSocket, HTTP stream, and promise tables use 0-based slot indices, meaning handle `0` is both a valid slot and a "null handle" sentinel in some paths. Standardise all tables to 1-based: `handle = slot_index + 1`, with `handle == 0` always meaning "invalid / not set". Add an explicit guard at the top of every handle-consuming function:

```rust
if handle == 0 {
    ptr::write(out, RocResult::err(RocStr::from_str("invalid handle: 0")));
    return;
}
let slot_idx = (handle - 1) as usize;
```

#### 2.2.4 C-ABI argument ordering audit (`KI-39`)

Roc's C-ABI convention places the return-value out-pointer as the **first** argument. Audit every `#[no_mangle] pub unsafe extern "C" fn rocFx*` function in `host/src/lib.rs`. Current inconsistencies found:

| Function | Out-ptr position | Correct |
|---|---|---|
| `rocFxRpcInvoke` | Last | Should be first |
| `rocFxRpcFireAndForget` | Last | Should be first |
| `rocFxRpcSchedule` | First | ✓ |
| `rocFxLog` | No out | ✓ |
| `rocFxGetWorkerId` | First (out param) | ✓ |

For each function that is wrong: move the `out: *mut RocResult<...>` parameter to position 0, then update the matching declaration in `platform/Effect.roc` and `platform/main.roc` `hosted {}` block if needed. Add a Rust unit test after each fix that calls the function with a stack-allocated `MaybeUninit` out buffer and verifies the result.

#### 2.2.5 `normalize_path` — block `..` traversal (`KI-40`)

```rust
pub fn normalize_path(raw: &str) -> Result<String, String> {
    // existing whitespace/dot stripping...
    for part in s.split('/') {
        if part == ".." {
            return Err("path traversal rejected".to_string());
        }
        // ...
    }
    Ok(parts.join("/"))
}
```

Update all call sites to handle the `Result` and propagate an `Err` back to the Roc guest.

#### 2.2.6 `GlobalCell` — document single-thread assumption (`KI-41`)

```rust
// SAFETY: wasm32 is currently single-threaded. This `Sync` impl is unsound
// in a multi-threaded environment. If Golem ever supports multi-threaded
// component instances, replace with `once_cell::sync::OnceCell` or a mutex.
unsafe impl<T> Sync for GlobalCell<T> {}
```

File a tracking comment `// TODO(multi-thread): revisit GlobalCell` above the struct definition.

#### 2.2.7 `defineAgent` legacy — document `stateChanged: True` limitation (`KI-42`)

Add a prominent warning to the `defineAgent` doc comment:

```roc
## ⚠️ Backwards-compatible legacy single-agent definition.
## Note: This shim always sets `stateChanged: True` on every `handleMessage`
## call, which forces a full state serialisation even for read-only queries.
## For zero-cost read-only bypass, migrate to `Golem.agent` with `readOnly`.
```

---

### 2.3 Test Coverage

#### 2.3.1 Rust host unit tests for all effect functions (`KI-43`)

Add `#[cfg(test)]` blocks in `host/src/lib.rs` using the existing `stub-guest` feature. Each test uses a stack `MaybeUninit` out buffer and verifies the non-wasm32 stub path:

```rust
#[cfg(test)]
mod host_tests {
    use super::*;
    use core::mem::MaybeUninit;

    #[test]
    fn test_roc_fx_log_does_not_crash() { ... }

    #[test]
    fn test_roc_fx_get_worker_id_returns_local() { ... }

    #[test]
    fn test_rpc_invoke_stub_returns_ok() { ... }

    #[test]
    fn test_kv_round_trip() { ... }   // set then get

    #[test]
    fn test_compute_scheduled_time_basic() { ... }

    #[test]
    fn test_normalize_path_rejects_traversal() { ... }

    #[test]
    fn test_normalize_path_basic() { ... }

    #[test]
    fn test_store_and_get_secret() { ... }

    #[test]
    fn test_parse_single_agent_type_name_and_mode() { ... }

    #[test]
    fn test_parse_http_mount_path_variables() { ... }
}
```

#### 2.3.2 `RocStr` large-string clone / refcount tests (`KI-44`)

```rust
#[test]
fn test_rocstr_large_clone_shares_allocation() {
    let s = "x".repeat(100);
    let roc = RocStr::from_str(&s);
    let cloned = roc.clone();
    // After fix: data pointer should be identical
    assert_eq!(roc.words[0], cloned.words[0]);
    assert_eq!(roc.as_str(), cloned.as_str());
}

#[test]
fn test_rocstr_large_drop_decrements_refcount() { ... }
```

#### 2.3.3 `tests/spec_builder.roc` — `define/build/register*` path coverage (`KI-45`)

New test file covering the full typed pipeline:

```roc
app [main] { pf: platform "../platform/main.roc" }

import pf.Golem exposing [
    Agent, Method,
    define, registerTyped, registerReadOnly, registerReadOnlyTyped,
    registerEffectful, build, method,
]
import pf.Types exposing [Result]

CountState : { n : I64 }

addMethod : Method(I64, I64)
addMethod = method({ name: "add", description: "Adds n", promptHint: "" })

getMethod : Method({}, I64)
getMethod = method({ name: "get", description: "Returns n", promptHint: "" })

testSpec : Agent(CountState)
testSpec =
    define(
        { name: "spec-test", version: "1.0.0", description: "" },
        |_| Ok({ n: 0.I64 }),
        |st| Json.to_str(st),
        |json| Json.parse(json),
    )
        |> registerTyped(addMethod, |st, delta| Ok({ state: { n: st.n + delta }, out: st.n + delta }), Json.parse, Json.to_str)
        |> registerReadOnlyTyped(getMethod, |st, _| Ok(st.n), |_| Ok({}), Json.to_str)
        |> build

# Verify method metadata is populated
expect {
    erased = match testSpec.agents { [a, ..] => a _ => crash "empty" }
    List.len(erased.metadata.methods) == 2
}

# Verify mutating method: stateChanged = True, nextStateJson is non-empty
expect { ... }

# Verify read-only method: stateChanged = False, nextStateJson is empty
expect { ... }

# Verify unknown method returns Err
expect { ... }

main = testSpec
```

#### 2.3.4 `tests/bundle.roc` — multi-agent routing via `bundle2` (`KI-46`)

```roc
# Builds two named agents, bundles them, verifies find_agent_by_name routes correctly.
expect {
    bundled = bundle2(agentA, agentB)
    # Agent A is found by name
    # Agent B is found by name
    # Unknown name returns Err
}
```

#### 2.3.5 Remove `|| [ $? -eq 2 ]` from all `roc test` calls (`KI-47`)

In `.github/workflows/ci.yaml` Layer 2 step:

```yaml
# Before:
roc test tests/envelope.roc || [ $? -eq 2 ]
roc test tests/json_roundtrip.roc || [ $? -eq 2 ]
roc test tests/dx_primitives.roc || [ $? -eq 2 ]

# After:
roc test tests/envelope.roc
roc test tests/json_roundtrip.roc
roc test tests/dx_primitives.roc
roc test tests/spec_builder.roc
roc test tests/bundle.roc
```

Similarly for `roc check` calls.

#### 2.3.6 `escape` — add control-character unit tests (`KI-48`)

```roc
# In tests/json_roundtrip.roc or tests/envelope.roc:
expect Json.escape("\u(0000)\u(0001)\u(001f)") == "\\u0000\\u0001\\u001f"
expect Json.escape("hello\nworld") == "hello\\nworld"
```

---

### 2.4 DX Improvements

#### 2.4.1 Group `Golem.roc` exports with section comments (`KI-49`)

The 115-name export list gains grouping comments:

```roc
module [
    # --- Agent Builders ---
    Agent, ErasedAgent, Spec,
    agent, agentWith, simpleAgent,
    ephemeral, simpleEphemeral,
    define, defineEphemeral, defineAgent,
    build, bundle, bundle2, bundleAll, erase,

    # --- Method & Handler Registration ---
    Method, Handler,
    method, register, registerReadOnly, registerEphemeral,
    registerTyped, registerReadOnlyTyped, registerEphemeralTyped,
    registerEffectful, registerEffectfulTyped,
    withHttpMount,

    # --- Tool Helpers ---
    Tool, StatelessTool,
    tool, toolJson, statefulTool,
    statelessTool, statelessToolJson, statelessToTool,
    withTools, withToolHandler, withStatelessToolHandler,
    withEffectfulToolHandler, withEffectfulStatelessToolHandler,
    param, optionalParam, numberParam, boolParam, typedParam,
    dispatchTool, dispatchMessage, dispatchStatelessTool,

    # --- Response Primitives ---
    reply, readOnly, stay, fail, replyJson, readOnlyJson,

    # --- Logging ---
    logTrace, logDebug, logInfo, logWarn, logError,

    # --- Worker Identity ---
    workerId,

    # --- RPC ---
    rpc, rpcInstance, rpcSchedule, rpcScheduleInstance,
    rpcFireAndForget, rpcFireAndForgetInstance,
    rpcTyped, rpcTypedInstance, rpcScheduleTyped, rpcScheduleTypedInstance,
    rpcFireAndForgetTyped, rpcFireAndForgetTypedInstance,
    rpcBytes, rpcBytesInstance, rpcScheduleBytes, rpcScheduleBytesInstance,
    rpcFireAndForgetBytes, rpcFireAndForgetBytesInstance,

    # --- HTTP ---
    http, httpRequest, httpRequestBytes,
    httpStreamOpen, httpStreamReadChunk, httpStreamClose,

    # --- Storage: Filesystem ---
    readFile, writeFile, readFileBytes, writeFileBytes, appendFileBytes,

    # --- Storage: Key-Value ---
    kvGet, kvSet, kvGetBytes, kvSetBytes,

    # --- Storage: Blobstore ---
    blobGet, blobPut, blobGetBytes, blobPutBytes,

    # --- Secrets ---
    getSecretHandle, revealSecret,

    # --- Database ---
    dbExecute, dbExecuteParams, dbQuery, dbQueryParams,

    # --- WebSocket ---
    websocketConnect, websocketSend, websocketReceive, websocketClose,

    # --- Concurrency & Time ---
    sleep, now,

    # --- Golem Primitives ---
    setPersistence,
    generateIdempotencyKey, getOplogIndex, oplogCommit,
    getIdempotenceMode, setIdempotenceMode, revertAgent,
    createPromise, completePromise, getPromise, createWebhook,

    # --- Observability ---
    configGet, traceId, spanId, traceHeaders,
]
```

#### 2.4.2 `HttpRequest` record type + `httpRequest` overload (`KI-50`)

Add to `platform/Types.roc`:

```roc
HttpRequest : {
    url : Str,
    method : Str,
    headers : List({ name : Str, value : Str }),
    body : Str,
    secretHeaders : List(SecretHeader),
}
```

Add to `platform/Golem.roc`:

```roc
## Perform an outgoing HTTP request using a typed HttpRequest record.
## For raw JSON control use `Golem.http`.
httpRequest : HttpRequest -> Effect.Effect(Result(Str, Str))
httpRequest = |req|
    Effect.httpRequest(encodeHttpRequest(req))
```

Where `encodeHttpRequest` serialises the record to the JSON schema that `perform_wasi_http_request` already parses.

#### 2.4.3 Add `numberParam`, `boolParam`, `typedParam` helpers (`KI-51`)

```roc
numberParam : Str, Str -> ToolParameter
numberParam = |name, description| {
    name,
    description,
    paramType: "number",
    required: True,
}

boolParam : Str, Str -> ToolParameter
boolParam = |name, description| {
    name,
    description,
    paramType: "boolean",
    required: True,
}

typedParam : Str, Str, Str -> ToolParameter
typedParam = |name, description, paramType| {
    name,
    description,
    paramType,
    required: True,
}
```

#### 2.4.4 Fix error message formatting — `{:?}` → display-friendly (`KI-52`)

Replace all `format!("{:?}", err)` in `host/src/lib.rs` RPC, KV, blob, WS, and DB handlers with the most human-readable variant available. Where `Display` is implemented, use `format!("{}", err)`. Where only `Debug` is available, extract the inner error message string where possible:

```rust
// Before:
Err(err) => {
    ptr::write(out, RocResult::err(RocStr::from_str(&format!("{:?}", err))));
}

// After:
Err(err) => {
    let msg = err.to_string();  // or a custom extractor
    ptr::write(out, RocResult::err(RocStr::from_str(&msg)));
}
```

#### 2.4.5 Document the double-encode state protocol (`KI-53`)

Add inline comments in `platform/main.roc` and `host/src/lib.rs` where the double-encode/decode occurs:

```roc
## NOTE: The state is transmitted as an escaped JSON *string* embedded inside
## the outer envelope JSON object. The host first decodes the outer envelope,
## then decodes `state` as a second JSON parse. This avoids nested-object
## escaping at the cost of two parse steps. Do not change without updating
## the Rust host's `handle_invoke` decode logic simultaneously.
format_reply_envelope : Str, Str -> Str
```

#### 2.4.6 Fix streaming example to use actual `websocketConnect` or add clear comment (`KI-54`)

```roc
# Option A: make it real (preferred)
"connect" => {
    urlResult = Effect.run!(Golem.websocketConnect("wss://echo.example.com"))
    match urlResult {
        Ok(handle) => reply({ ..state, connected: True, handle }, "WebSocket connected")
        Err(e) => readOnly(state, "Connection failed: ${e}")
    }
}

# Option B: add a comment if keeping it simplified
"connect" =>
    # NOTE: Simplified example. Replace with Golem.websocketConnect call.
    reply({ ..state, connected: True, handle: 1.U32 }, "WebSocket connected (handle=1)")
```

#### 2.4.7 Fix `rpcSchedule` in canary example to use real worker URN (`KI-55`)

```roc
# Before:
rpcSchedule(state.service, emergencyRollback, ...) # service name, not a worker URN

# After — schedule against self using workerId:
Effect.and_then(
    Golem.workerId,
    |selfId|
        rpcSchedule(selfId, emergencyRollback, "{\"reason\":\"approval timeout expired\"}", 300000.U64),
)
```

---

### 2.5 Tooling & Housekeeping

#### 2.5.1 Add `.gitignore` (`KI-56`)

```gitignore
# Build artifacts
/build/

# Release archives (content-addressed tarballs and named copies)
/dist/*.tar.zst
/dist/*.tar.br
/dist/*.tar.gz

# Rust compiled targets
/tooling/pack_platform/target/
/host/target/

# Keep:
# platform/targets/wasm32/libroc_golem_host.a  — intentionally committed
# platform/targets/wasm32/libc.a               — intentionally committed
# platform/libhost.a                           — intentionally committed
```

#### 2.5.2 Remove `roc fmt` mutation from `build.sh` (`KI-57`)

```bash
# Remove from build_app():
# echo "==> Formatting Roc code ($APP_FILE)..."
# roc fmt "$APP_FILE" "$PLATFORM_DIR/main.roc"
```

Format checks remain in the CI `Layer 1: Static Checks` step as `roc fmt --check`. Developers run `roc fmt` manually or via editor integration. Add a note to the contributing section of `README.md`.

#### 2.5.3 Fix `test_consumer.sh` archive-glob default (`KI-58`)

```bash
# Before:
ARCHIVE="${1:-"$ROOT_DIR/dist/roc-golem-v1.1.0.tar.zst"}"

# After:
ARCHIVE="${1:-$(ls -t "$ROOT_DIR/dist/roc-golem-"*.tar.zst 2>/dev/null | head -1)}"
if [[ -z "$ARCHIVE" || ! -f "$ARCHIVE" ]]; then
    echo "==> No archive found in dist/. Building platform package first..."
    "$SCRIPT_DIR/build.sh" package
    ARCHIVE=$(ls -t "$ROOT_DIR/dist/roc-golem-"*.tar.zst 2>/dev/null | head -1)
fi
```

#### 2.5.4 Add `CHANGELOG.md` (`KI-59`)

Create `CHANGELOG.md` at the repository root following Keep-a-Changelog format:

```markdown
# Changelog

All notable changes to roc-golem are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)

## [Unreleased]
### Fixed
- ...

## [1.0.0] — 2026-10-xx
### Added
- Initial GA release ...
```

The release workflow step `Create GitHub Release` appends content from `dist/release_notes.md`; `CHANGELOG.md` is the human-maintained long-form record.

#### 2.5.5 Fix `Cargo.toml` author email (`KI-60`)

```toml
# Before:
authors = ["Samuel Iyeh <thesparq@vmi3258764.contaboserver.net>"]

# After:
authors = ["Samuel Iyeh <thesparq@users.noreply.github.com>"]
```

#### 2.5.6 `golem.yaml` — add naming convention comment (`KI-61`)

```yaml
# Component names use kebab-case matching the agent `name` field in Roc source.
# WASM output files use snake_case + _agent suffix (e.g. canary_release_agent.wasm).
components:
  roc-golem:canary-release:
    ...
```

#### 2.5.7 Add `bundleAll` helper (`KI-62`)

```roc
## Bundle any number of type-erased agents into a single Wasm component.
## Use `erase` to convert a typed `Agent(state)` to `Agent({})` before passing.
bundleAll : List(ErasedAgent) -> Agent({})
bundleAll = |erasedList| { agents: erasedList }
```

Add a usage example in `README.md` showing `bundleAll([erase(agentA), erase(agentB), erase(agentC)])`.

#### 2.5.8 `defineAgent` legacy — `stateChanged` and read-only bypass notice (`KI-63`)

Document the limitation in a code comment and in the `README.md` migration guide section:

```roc
## ⚠️ Backwards-compatible shim. `handleMessage` always triggers a full state
## serialisation (stateChanged: True) regardless of whether state actually
## changed. For zero-cost read-only bypass, migrate to `Golem.agent` +
## `readOnly`. See docs/migration-guide.md.
defineAgent : { ... } -> Agent(state)
```

---

## 3. Implementation Details

### 3.1 Phased Execution Order

Execute phases strictly in order. Each phase's tests must pass before the next begins.

#### Phase 1 (1–2 days): Critical Correctness
1. `KI-30`: Fix `escape` to handle U+0000–U+001F — write `escapeControlChars`, update `tests/json_roundtrip.roc` with control character tests.
2. `KI-31`: Fix `agentWith` init error propagation.
3. `KI-32`: Fix `build` path missing-handler error message.
4. `KI-34`: Update `wit/world.wit` golem API import versions; run `wasm-tools validate` on all four WASMs.
5. `KI-35`: Fix `Str.inspect` → `Num.to_str`/`Json.encode` in counter and any other examples.
6. `KI-26`–`KI-29`: Implement real wasm32 paths for `createPromise`, `blobPut`, `getOplogIndex`, `rpcSchedule`.
7. `KI-47`: Remove `|| [ $? -eq 2 ]` from CI `roc test` calls.

#### Phase 2 (1–2 days): Safety Hardening
8. `KI-39`: Audit and fix C-ABI argument ordering for all `rocFx*` exports — highest-risk item, requires careful per-function verification.
9. `KI-36`: Fix `RocStr` clone to increment refcount.
10. `KI-37`: Make `SecretHandle` opaque in `Types.roc`.
11. `KI-38`: Unify 1-based handle convention across all resource tables.
12. `KI-40`: Add `".."` rejection to `normalize_path`.
13. `KI-41`: Add `GlobalCell` thread-safety comment.
14. `KI-42`: Add `defineAgent` limitation doc comment.

#### Phase 3 (1–2 days): Test Coverage
15. `KI-43`: Add Rust unit tests for all host effect functions.
16. `KI-44`: Add `RocStr` large-string clone/refcount tests.
17. `KI-45`: Create `tests/spec_builder.roc`.
18. `KI-46`: Create `tests/bundle.roc`.
19. `KI-48`: Add control character escape tests to existing JSON test file.

#### Phase 4 (1 day): DX Improvements
20. `KI-49`: Group `Golem.roc` exports with section comments.
21. `KI-50`: Add `HttpRequest` type + `httpRequest` overload.
22. `KI-51`: Add `numberParam`, `boolParam`, `typedParam` helpers.
23. `KI-52`: Replace `{:?}` with display-friendly error formatting in host.
24. `KI-53`: Add double-encode protocol inline comments.
25. `KI-54`: Fix streaming example (real `websocketConnect` or comment).
26. `KI-55`: Fix canary `rpcSchedule` target to use `workerId`.

#### Phase 5 (half day): Tooling & Housekeeping
27. `KI-56`: Add `.gitignore`.
28. `KI-57`: Remove `roc fmt` mutation from `build.sh`.
29. `KI-58`: Fix `test_consumer.sh` archive-glob default.
30. `KI-59`: Add `CHANGELOG.md`.
31. `KI-60`: Fix `Cargo.toml` author email.
32. `KI-61`: Add `golem.yaml` naming comment.
33. `KI-62`: Add `bundleAll` helper.
34. `KI-63`: Add `defineAgent` stateChanged limitation notice.

---

## 4. Verification Checklist

| KI | Description | File(s) | Phase | Done |
|----|-------------|---------|-------|------|
| KI-26 | Real `createPromise` on wasm32 | `host/src/lib.rs` | 1 | [x] |
| KI-27 | Real `blob_put_wasm32` | `host/src/lib.rs` | 1 | [x] |
| KI-28 | Real `rocFxGetOplogIndex` on wasm32 | `host/src/lib.rs` | 1 | [x] |
| KI-29 | Real `rocFxRpcSchedule` on wasm32 | `host/src/lib.rs` | 1 | [x] |
| KI-30 | `escape` handles U+0000–U+001F control chars | `platform/Json.roc` | 1 | [x] |
| KI-31 | `agentWith` init propagates `Err` | `platform/Golem.roc` | 1 | [x] |
| KI-32 | `build` path surfaces missing-handler error | `platform/Golem.roc` | 1 | [x] |
| KI-33 | `extract_json_field` handles escaped quotes | `platform/main.roc` | 1 | [x] |
| KI-34 | `wit/world.wit` golem API imports updated to `@1.6.x` | `wit/world.wit` | 1 | [x] |
| KI-35 | Counter example uses `Num.to_str` not `Str.inspect` for JSON | `examples/counter/main.roc` | 1 | [x] |
| KI-36 | `RocStr` clone increments refcount | `host/src/roc_std.rs` | 2 | [x] |
| KI-37 | `SecretHandle` is opaque type | `platform/Types.roc` | 2 | [x] |
| KI-38 | Unified 1-based handle convention | `host/src/lib.rs` | 2 | [x] |
| KI-39 | C-ABI argument order audit & fix | `host/src/lib.rs` | 2 | [x] |
| KI-40 | `normalize_path` rejects `..` traversal | `host/src/lib.rs` | 2 | [x] |
| KI-41 | `GlobalCell` thread-safety comment | `host/src/lib.rs` | 2 | [x] |
| KI-42 | `defineAgent` limitation doc comment | `platform/Golem.roc` | 2 | [x] |
| KI-43 | Rust unit tests for all host effect functions | `host/src/lib.rs` | 3 | [x] |
| KI-44 | `RocStr` large-string clone/refcount tests | `host/src/roc_std.rs` | 3 | [x] |
| KI-45 | `tests/spec_builder.roc` created | new file | 3 | [x] |
| KI-46 | `tests/bundle.roc` created | new file | 3 | [x] |
| KI-47 | Remove `|| [ $? -eq 2 ]` from CI | `.github/workflows/ci.yaml` | 1 | [x] |
| KI-48 | Control character escape unit tests | `tests/json_roundtrip.roc` | 3 | [x] |
| KI-49 | `Golem.roc` exports grouped with section comments | `platform/Golem.roc` | 4 | [x] |
| KI-50 | `HttpRequest` type + `httpRequest` overload | `platform/Types.roc`, `platform/Golem.roc` | 4 | [x] |
| KI-51 | `numberParam`, `boolParam`, `typedParam` helpers | `platform/Golem.roc` | 4 | [x] |
| KI-52 | Replace `{:?}` with display-friendly error messages | `host/src/lib.rs` | 4 | [x] |
| KI-53 | Double-encode state protocol documented | `platform/main.roc`, `host/src/lib.rs` | 4 | [x] |
| KI-54 | Streaming example uses real `websocketConnect` or has comment | `examples/streaming_agent/main.roc` | 4 | [x] |
| KI-55 | Canary `rpcSchedule` target uses `workerId` | `examples/canary_release/ReleaseCoordinator.roc` | 4 | [x] |
| KI-56 | `.gitignore` added | root | 5 | [x] |
| KI-57 | `roc fmt` mutation removed from `build.sh` | `tooling/build.sh` | 5 | [x] |
| KI-58 | `test_consumer.sh` archive-glob default | `tooling/test_consumer.sh` | 5 | [x] |
| KI-59 | `CHANGELOG.md` added | root | 5 | [x] |
| KI-60 | `Cargo.toml` author email fixed | `host/Cargo.toml` | 5 | [x] |
| KI-61 | `golem.yaml` naming comment added | `golem.yaml` | 5 | [x] |
| KI-62 | `bundleAll` helper added | `platform/Golem.roc` | 5 | [x] |
| KI-63 | `defineAgent` `stateChanged` limitation notice | `platform/Golem.roc` | 5 | [x] |

---

## 5. Success Criteria

- All `roc test` runs in CI exit `0` with no `|| [ $? -eq 2 ]` bypass.
- `cargo test --features stub-guest --lib` passes all tests including the new host effect tests and `RocStr` clone test.
- `wasm-tools validate` succeeds on all four component WASMs after the WIT version update.
- `Golem.createPromise` → `createWebhook` → `getPromise` round-trip works on a live Golem 1.6.3 server.
- `Golem.blobPutBytes` and `Golem.blobGetBytes` round-trip works on a live Golem 1.6.3 server.
- A fresh `git clone` + `./tooling/build.sh counter` completes without formatting the source files.
- `docs/feature-checklist.md` updated marking all KI-26–KI-63 items as verified.
