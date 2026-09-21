use crate::roc_std::{RocResult, RocStr};
use core::ptr;

// Mock implementations for testing purposes only.

#[no_mangle]
pub unsafe extern "C" fn main_init_for_host_1_exposed_generic(
    out: *mut RocResult<RocStr, RocStr>,
    _config: *const RocStr,
) {
    let initial_state = RocStr::from_str("{\"count\": 0, \"invocations\": 0, \"history\": [], \"connected\": false, \"handle\": 0, \"sent\": 0}");
    ptr::write(out, RocResult::ok(initial_state));
}

#[no_mangle]
pub unsafe extern "C" fn main_handle_message_for_host_1_exposed_generic(
    out: *mut RocResult<RocStr, RocStr>,
    state: *const RocStr,
    message: *const RocStr,
) {
    let msg = if message.is_null() { "" } else { (*message).as_str() };
    let state_str = if state.is_null() { "{}" } else { (*state).as_str() };

    let (next_state_json, resp) = if msg == "increment" {
        let count: i64 = if let Ok(val) = serde_json::from_str::<serde_json::Value>(state_str) {
            val.get("count").and_then(|v| v.as_i64()).unwrap_or(0) + 1
        } else {
            1
        };
        (
            serde_json::json!({ "count": count, "history": ["increment"] }),
            format!("Counter incremented to {}", count),
        )
    } else if msg == "decrement" {
        let count: i64 = if let Ok(val) = serde_json::from_str::<serde_json::Value>(state_str) {
            val.get("count").and_then(|v| v.as_i64()).unwrap_or(0) - 1
        } else {
            -1
        };
        (
            serde_json::json!({ "count": count, "history": ["decrement"] }),
            format!("Counter decremented to {}", count),
        )
    } else if msg == "get" {
        let count: i64 = if let Ok(val) = serde_json::from_str::<serde_json::Value>(state_str) {
            val.get("count").and_then(|v| v.as_i64()).unwrap_or(0)
        } else {
            0
        };
        (
            serde_json::json!({ "count": count }),
            format!("Current count is {}", count),
        )
    } else if msg == "connect" {
        (
            serde_json::json!({ "connected": true, "handle": 1, "sent": 0 }),
            String::from("WebSocket connected (handle=1)"),
        )
    } else {
        let parsed_state = serde_json::from_str::<serde_json::Value>(state_str)
            .unwrap_or_else(|_| serde_json::json!({}));
        (
            parsed_state,
            format!("Agent received message: '{}'", msg),
        )
    };

    let payload_val = serde_json::json!({
        "state": next_state_json,
        "response": resp,
    });
    let payload_str = payload_val.to_string();
    ptr::write(out, RocResult::ok(RocStr::from_str(&payload_str)));
}

#[no_mangle]
pub unsafe extern "C" fn main_handle_tool_call_for_host_1_exposed_generic(
    out: *mut RocResult<RocStr, RocStr>,
    state: *const RocStr,
    tool_call_json: *const RocStr,
) {
    let call_str = if tool_call_json.is_null() { "{}" } else { (*tool_call_json).as_str() };
    let state_str = if state.is_null() { "{}" } else { (*state).as_str() };

    let mut tool_name = String::from("default");
    let mut call_id = String::from("call-1");
    let mut args = String::from("{}");

    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(call_str) {
        if let Some(n) = parsed.get("name").and_then(|v| v.as_str()) {
            tool_name = n.into();
        }
        if let Some(id) = parsed.get("id").and_then(|v| v.as_str()) {
            call_id = id.into();
        }
        if let Some(a) = parsed.get("arguments").and_then(|v| v.as_str()) {
            args = a.into();
        }
    }

    let parsed_state = serde_json::from_str::<serde_json::Value>(state_str)
        .unwrap_or_else(|_| serde_json::json!({}));

    let (next_state_json, output, success) = match tool_name.as_str() {
        "calculator" => (
            parsed_state,
            String::from("{\"result\": 42}"),
            true,
        ),
        "echo" => (
            parsed_state,
            serde_json::json!({ "echo": args }).to_string(),
            true,
        ),
        "get_count" => {
            let count: i64 = parsed_state
                .get("count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            (
                parsed_state,
                serde_json::json!({ "count": count }).to_string(),
                true,
            )
        }
        "stream_message" => (
            parsed_state,
            String::from("{\"status\": \"stream_active\"}"),
            true,
        ),
        _ => (
            parsed_state,
            serde_json::json!({ "error": format!("Unknown tool {}", tool_name) }).to_string(),
            false,
        ),
    };

    let payload_val = serde_json::json!({
        "id": call_id,
        "state": next_state_json,
        "success": success,
        "output": output,
    });
    let payload_str = payload_val.to_string();
    ptr::write(out, RocResult::ok(RocStr::from_str(&payload_str)));
}

#[no_mangle]
pub unsafe extern "C" fn main_metadata_for_host_1_exposed_generic(out: *mut RocStr) {
    let meta_json = r#"{
        "name": "roc-golem-agent",
        "version": "1.0.0",
        "description": "Durable Roc Agent on Golem Cloud",
        "tools": [
            {
                "name": "calculator",
                "description": "Evaluates mathematical expressions safely",
                "parameters": [
                    {
                        "name": "expression",
                        "description": "Math expression to evaluate",
                        "type": "string",
                        "required": true
                    }
                ]
            },
            {
                "name": "echo",
                "description": "Echoes back the input arguments",
                "parameters": [
                    {
                        "name": "text",
                        "description": "Text to echo",
                        "type": "string",
                        "required": true
                    }
                ]
            },
            {
                "name": "get_count",
                "description": "Returns current counter value",
                "parameters": []
            },
            {
                "name": "stream_message",
                "description": "Streams message via WebSocket",
                "parameters": [
                    {
                        "name": "content",
                        "description": "Message to stream",
                        "type": "string",
                        "required": true
                    }
                ]
            }
        ]
    }"#;

    ptr::write(out, RocStr::from_str(meta_json));
}
