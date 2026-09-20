use crate::roc_std::{RocResult, RocStr};

// Default implementations of the Roc platform exposed entry points.
// When linking with a compiled Roc application, these symbols can be overridden or mapped.

#[no_mangle]
pub unsafe extern "C" fn roc__main_init_for_host_1_exposed_generic(
    _config: *mut RocStr,
    out: *mut RocResult<RocStr, RocStr>,
) {
    let initial_state = RocStr::from_str("{\"count\": 0, \"invocations\": 0, \"history\": []}");
    *out = RocResult::ok(initial_state);
}

#[no_mangle]
pub unsafe extern "C" fn roc__main_handle_message_for_host_1_exposed_generic(
    state: *mut RocStr,
    message: *mut RocStr,
    out: *mut RocResult<RocStr, RocStr>,
) {
    let msg = (*message).as_str();
    let state_str = (*state).as_str();

    let (next_state_json, resp) = if msg == "increment" {
        // Parse count or increment
        let count: i64 = if let Ok(val) = serde_json::from_str::<serde_json::Value>(state_str) {
            val.get("count").and_then(|v| v.as_i64()).unwrap_or(0) + 1
        } else {
            1
        };
        (
            format!("{{\"count\": {}, \"history\": [\"increment\"]}}", count),
            format!("Counter incremented to {}", count),
        )
    } else if msg == "decrement" {
        let count: i64 = if let Ok(val) = serde_json::from_str::<serde_json::Value>(state_str) {
            val.get("count").and_then(|v| v.as_i64()).unwrap_or(0) - 1
        } else {
            -1
        };
        (
            format!("{{\"count\": {}, \"history\": [\"decrement\"]}}", count),
            format!("Counter decremented to {}", count),
        )
    } else if msg == "get" {
        let count: i64 = if let Ok(val) = serde_json::from_str::<serde_json::Value>(state_str) {
            val.get("count").and_then(|v| v.as_i64()).unwrap_or(0)
        } else {
            0
        };
        (
            format!("{{\"count\": {}}}", count),
            format!("Current count is {}", count),
        )
    } else {
        (
            state_str.into(),
            format!("Agent received message: '{}'", msg),
        )
    };

    let payload = format!(
        "{{\"state\": {}, \"response\": \"{}\"}}",
        next_state_json, resp
    );
    *out = RocResult::ok(RocStr::from_str(&payload));
}

#[no_mangle]
pub unsafe extern "C" fn roc__main_handle_tool_call_for_host_1_exposed_generic(
    state: *mut RocStr,
    tool_call_json: *mut RocStr,
    out: *mut RocResult<RocStr, RocStr>,
) {
    let call_str = (*tool_call_json).as_str();
    let state_str = (*state).as_str();

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

    let (next_state_json, output, success): (String, String, bool) = match tool_name.as_str() {
        "calculator" => (
            state_str.into(),
            "{\"result\": 42}".into(),
            true,
        ),
        "echo" => (
            state_str.into(),
            format!("{{\"echo\": \"{}\"}}", args),
            true,
        ),
        "get_count" => {
            let count: i64 = if let Ok(val) = serde_json::from_str::<serde_json::Value>(state_str) {
                val.get("count").and_then(|v| v.as_i64()).unwrap_or(0)
            } else {
                0
            };
            (
                state_str.into(),
                format!("{{\"count\": {}}}", count),
                true,
            )
        }
        _ => (
            state_str.into(),
            format!("{{\"error\": \"Unknown tool {}\"}}", tool_name),
            false,
        ),
    };

    let payload = format!(
        "{{\"id\": \"{}\", \"state\": {}, \"success\": {}, \"output\": \"{}\"}}",
        call_id, next_state_json, success, output.replace('\"', "\\\"")
    );
    *out = RocResult::ok(RocStr::from_str(&payload));
}

#[no_mangle]
pub unsafe extern "C" fn roc__main_metadata_for_host_1_exposed_generic(out: *mut RocStr) {
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
            }
        ]
    }"#;

    *out = RocStr::from_str(meta_json);
}
