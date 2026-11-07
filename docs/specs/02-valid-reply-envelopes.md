# Specification: Unit 02 — Valid Reply Envelopes & State Escaping

- **Unit:** 02
- **File:** `docs/specs/02-valid-reply-envelopes.md`
- **Status:** Spec written
- **Fixes:** `KI-03`, partial `KI-09`

---

## 1. Goal

Eliminate the JSON interpolation defect in `platform/main.roc` (`KI-03`) where unescaped state strings produce syntactically malformed reply envelopes that cause the Golem Rust host to silently discard state updates. Ensure that all reply envelopes (for standard messages, typed methods, and tool invocations) are 100% valid JSON documents across all edge cases (nested quotes, backslashes, newlines, unicode, and empty-string read-only bypass). Update the counter example and README to use a robust, non-lossy state codec (`KI-09`).

---

## 2. Design & Architecture

### 2.1 The Root Cause (`KI-03`)
In `platform/main.roc`, the host interaction handlers format response envelopes using Roc string interpolation:

```roc
# In main_handle_message_for_host!:
escaped_response = escape_json_str(res.response)
Ok("{\"state\":\"${res.nextStateJson}\",\"response\":\"${escaped_response}\"}")

# In main_handle_tool_call_for_host!:
success_str = if res.result.success "true" else "false"
escaped_output = escape_json_str(res.result.output)
Ok("{\"state\":\"${res.nextStateJson}\",\"success\":${success_str},\"output\":\"${escaped_output}\"}")
```

While `res.response` and `res.result.output` are escaped using `escape_json_str`, `res.nextStateJson` is interpolated **raw** inside double quotes (`"state":"${res.nextStateJson}"`).
Because `nextStateJson` is produced by serializing agent state to JSON (e.g., `{"count":1}`), the resulting envelope string is:

```json
{"state":"{"count":1}","response":"..."}
```

This string is **invalid JSON** (unquoted, unescaped delimiters inside a quoted string).

When this string is received by the Rust host in `host/src/lib.rs` (lines 2525 & 2570):
```rust
if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(payload_str) {
    if !is_ephemeral {
        if let Some(next_state) = parsed.get("state") {
            let next_state_str = if let Some(s) = next_state.as_str() {
                s.to_string()
            } else {
                next_state.to_string()
            };
            if !next_state_str.is_empty() {
                set_current_state(next_state_str);
            }
        }
    }
    // ...
}
```

Because `serde_json::from_str` fails on the malformed envelope, the entire `if let Ok(parsed)` block is skipped. `set_current_state` is **never called**, and all state mutations produced by Roc agents are silently lost!

### 2.2 The Solution
1. **Escape State Consistently in `platform/main.roc`:**
   Define centralized envelope formatting functions:
   ```roc
   format_reply_envelope : Str, Str -> Str
   format_reply_envelope = |next_state_json, response| {
       escaped_state = escape_json_str(next_state_json)
       escaped_response = escape_json_str(response)
       "{\"state\":\"${escaped_state}\",\"response\":\"${escaped_response}\"}"
   }

   format_tool_envelope : Str, Bool, Str -> Str
   format_tool_envelope = |next_state_json, success, output| {
       escaped_state = escape_json_str(next_state_json)
       escaped_output = escape_json_str(output)
       success_str = if success "true" else "false"
       "{\"state\":\"${escaped_state}\",\"success\":${success_str},\"output\":\"${escaped_output}\"}"
   }
   ```
2. **Preserve Host Decoupling & Read-Only Bypass:**
   - When an agent method executes without mutating state (e.g. `registerReadOnly`), `nextStateJson` is `""`.
   - `escaped_state` becomes `""`.
   - The resulting envelope is `{"state":"","response":"..."}`.
   - In Rust, `next_state.as_str()` yields `Some("")`. Since `!next_state_str.is_empty()` is false, the host skips `set_current_state`, preserving the zero-serialization bypass!
   - When state is mutated, `escaped_state` contains escaped JSON (e.g. `{\"count\":1}`). In Rust, `next_state.as_str()` unpacks the exact JSON string `{"count":1}` and commits it to worker storage.
3. **Robust Counter State Codec (`KI-09` partial):**
   - Update `examples/counter/main.roc` so `deserializeState` handles whitespace, negative numbers, and extra fields cleanly.
   - Update `README.md` to remove `deserializeState: |_json| Ok({ count: 0 })` which previously reset state on every invocation.

---

## 3. Test-Driven Development (TDD) Plan

In accordance with `docs/ai-workflow-rules.md` and `docs/testing.md`, we will follow strict TDD:

### Phase 1: Red (Write Tests First)
Before modifying `platform/main.roc`:
1. **Rust Host Unit Tests (`host/src/lib.rs` / `host/tests/`):**
   - `test_envelope_deserialization_escaped_state()`: Tests parsing of `{"state":"{\"count\":1}","response":"ok"}`. Verifies that `next_state_str` is extracted as `{"count":1}` and `set_current_state` stores it.
   - `test_envelope_read_only_bypass()`: Tests parsing of `{"state":"","response":"ok"}`. Verifies that host state remains untouched.
   - `test_envelope_complex_nested_state()`: Tests parsing of state containing quotes, backslashes, newlines, and unicode (`{"state":"{\"msg\":\"hello \\\"world\\\"\\nline2 🚀\"}","response":"done"}`). Verifies exact round-trip preservation.
   - `test_envelope_tool_call_payload()`: Tests parsing of `{"state":"{\"invocations\":1}","success":true,"output":"{\"val\":42}"}`.
   - `test_malformed_raw_state_envelope_fails()`: Confirms that `{"state":"{"count":1}","response":"ok"}` fails `serde_json::from_str`, explicitly demonstrating the bug that `KI-03` addresses.
   - `test_counter_state_transitions_multi_turn()`: Simulates successive calls: `init` (`count: 0`) -> `increment` (`count: 1`) -> `increment` (`count: 2`) -> `get` (`count: 2`, untouched) -> `decrement` (`count: 1`).
2. **Roc Unit Tests (`platform/Envelope.roc` or `tests/envelope.roc`):**
   - Tests using top-level `expect` verifying `escape_json_str`, `format_reply_envelope`, and `format_tool_envelope` output valid JSON matching golden strings for:
     - Basic state `{"count": 0}`.
     - Empty state `""`.
     - Strings containing `"`, `\`, `\n`, `\r`, `\t`, and unicode characters.

### Phase 2: Green (Implementation)
1. Implement `format_reply_envelope` and `format_tool_envelope` with `escape_json_str` in `platform/main.roc`.
2. Apply them in:
   - `main_handle_message_for_host!` (for both message handler and typed method handler).
   - `main_handle_tool_call_for_host!`.
3. Fix `examples/counter/main.roc` state serialization and deserialization.
4. Fix `README.md` counter snippet.

### Phase 3: Refactor & Verification
1. Run all Rust unit tests: `cargo test --manifest-path host/Cargo.toml --features stub-guest --lib`.
2. Run Roc tests: `roc test ...`.
3. Check platform and examples: `roc check platform/main.roc` and `roc check examples/counter/main.roc`.
4. Compile real component: `./tooling/build.sh counter`.
5. Validate component: `wasm-tools validate build/counter_agent.wasm`.

---

## 4. Deliverables & Modified Files

- `docs/specs/02-valid-reply-envelopes.md`: This specification file.
- `host/src/lib.rs`: TDD unit tests for envelope parsing, state persistence, bypass, and round-tripping.
- `platform/main.roc`: Escaped envelope construction in all host-facing entry points.
- `examples/counter/main.roc`: Robust state codec.
- `README.md`: Fix snippet to match durable state handling.
- `docs/progress-tracker.md`: Mark `KI-03` as resolved.
- `docs/specs/00-build-plan.md`: Update Unit 02 status to complete after verification.

---

## 5. Verification Criteria

- [ ] All new host envelope unit tests pass (`cargo test --manifest-path host/Cargo.toml --features stub-guest --lib`).
- [ ] Golden envelope round-trip tests pass without character corruption.
- [ ] `roc check platform/main.roc` passes with zero errors.
- [ ] `roc check examples/counter/main.roc` passes with zero errors.
- [ ] `./tooling/build.sh counter` compiles and links against `libhost.a` cleanly.
- [ ] `wasm-tools validate build/counter_agent.wasm` exits 0.
