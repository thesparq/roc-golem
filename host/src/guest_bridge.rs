use crate::roc_std::{RocResult, RocStr};
use core::ptr;

// Stub guest implementations of the Roc platform exposed entry points for testing.

#[no_mangle]
pub unsafe extern "C" fn roc__main_init_for_host_1_exposed_generic(
    config: *mut RocStr,
    out: *mut RocResult<RocStr, RocStr>,
) {
    let cfg_str = if config.is_null() { "{}" } else { (*config).as_str() };
    let agent_type = if let Ok(val) = serde_json::from_str::<serde_json::Value>(cfg_str) {
        val.get("agent_type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    } else {
        String::new()
    };

    if agent_type == "diagnostics-worker" {
        ptr::write(out, RocResult::ok(RocStr::from_str("{}")));
    } else {
        let initial_state = RocStr::from_str(
            "{\"count\": 0, \"invocations\": 0, \"history\": [], \"connected\": false, \"handle\": 0, \"sent\": 0}",
        );
        ptr::write(out, RocResult::ok(initial_state));
    }
}

#[no_mangle]
pub unsafe extern "C" fn roc__main_handle_message_for_host_1_exposed_generic(
    state: *mut RocStr,
    message: *mut RocStr,
    out: *mut RocResult<RocStr, RocStr>,
) {
    let msg_str = if message.is_null() { "" } else { (*message).as_str() };
    let state_str = if state.is_null() { "{}" } else { (*state).as_str() };

    let mut agent_type = String::new();
    let mut method = String::new();
    let mut input = msg_str.to_string();

    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(msg_str) {
        if let Some(at) = parsed.get("agent_type").and_then(|v| v.as_str()) {
            agent_type = at.to_string();
        }
        if let Some(m) = parsed.get("method").and_then(|v| v.as_str()) {
            method = m.to_string();
        }
        if let Some(inp) = parsed.get("input").and_then(|v| v.as_str()) {
            input = inp.to_string();
        }
    }

    if method.is_empty() {
        method = input.clone();
    }

    if agent_type == "diagnostics-worker" || method == "run_diagnostics" {
        // Ephemeral agent: zero state returned ("state": "")
        let payload_val = serde_json::json!({
            "state": "",
            "response": "{\"healthy\": true, \"faults\": 0}",
        });
        ptr::write(out, RocResult::ok(RocStr::from_str(&payload_val.to_string())));
        return;
    }

    // Stateful agent
    let (next_state_json, resp) = if method == "increment" {
        let count: i64 = if let Ok(val) = serde_json::from_str::<serde_json::Value>(state_str) {
            val.get("count").and_then(|v| v.as_i64()).unwrap_or(0) + 1
        } else {
            1
        };
        (
            serde_json::json!({ "count": count, "history": ["increment"] }).to_string(),
            format!("Counter incremented to {}", count),
        )
    } else if method == "decrement" {
        let count: i64 = if let Ok(val) = serde_json::from_str::<serde_json::Value>(state_str) {
            val.get("count").and_then(|v| v.as_i64()).unwrap_or(0) - 1
        } else {
            -1
        };
        (
            serde_json::json!({ "count": count, "history": ["decrement"] }).to_string(),
            format!("Counter decremented to {}", count),
        )
    } else if method == "get" || method == "get_status" {
        // READ-ONLY BYPASS: return empty state string ""!
        let count: i64 = if let Ok(val) = serde_json::from_str::<serde_json::Value>(state_str) {
            val.get("count").and_then(|v| v.as_i64()).unwrap_or(0)
        } else {
            0
        };
        (
            String::new(), // EMPTY STATE -> READ-ONLY BYPASS
            format!("Current count is {}", count),
        )
    } else if method == "connect" {
        (
            serde_json::json!({ "connected": true, "handle": 1, "sent": 0 }).to_string(),
            String::from("WebSocket connected (handle=1)"),
        )
    } else if method == "start_canary" {
        let is_unhealthy = input.contains("PANIC") || input.contains("OOM");
        if is_unhealthy {
            (
                serde_json::json!({
                    "service": "auth-service",
                    "activeVersion": "v1.0.0",
                    "canaryVersion": "",
                    "weight": 0,
                    "region": "us-east-1",
                    "pendingTimerHandle": 0,
                    "approvalWebhookUrl": "",
                    "events": ["coordinator_initialized", "canary_aborted: faults detected"]
                }).to_string(),
                serde_json::json!({
                    "status": "aborted_unhealthy",
                    "version": "v1.1.0",
                    "weight": 0,
                    "approvalWebhookUrl": "",
                    "faults": 1
                }).to_string(),
            )
        } else {
            (
                serde_json::json!({
                    "service": "auth-service",
                    "activeVersion": "v1.0.0",
                    "canaryVersion": "v1.1.0",
                    "weight": 10,
                    "region": "us-east-1",
                    "pendingTimerHandle": 42,
                    "approvalWebhookUrl": "https://golem.cloud/webhooks/canary/auth-service/gate-approve",
                    "events": ["coordinator_initialized", "canary_started: v1.1.0 at 10%"]
                }).to_string(),
                serde_json::json!({
                    "status": "canary_started",
                    "version": "v1.1.0",
                    "weight": 10,
                    "approvalWebhookUrl": "https://golem.cloud/webhooks/canary/auth-service/gate-approve",
                    "faults": 0
                }).to_string(),
            )
        }
    } else if method == "approve_gate" {
        let is_approved = input.contains("\"approved\":true") || input.contains("\"approved\": true");
        if is_approved {
            (
                serde_json::json!({
                    "service": "auth-service",
                    "activeVersion": "v1.1.0",
                    "canaryVersion": "",
                    "weight": 100,
                    "region": "us-east-1",
                    "pendingTimerHandle": 0,
                    "approvalWebhookUrl": "",
                    "events": ["coordinator_initialized", "promoted v1.1.0 to 100%"]
                }).to_string(),
                String::from("Promoted v1.1.0 to 100% (approved by alice)"),
            )
        } else {
            (
                serde_json::json!({
                    "service": "auth-service",
                    "activeVersion": "v1.0.0",
                    "canaryVersion": "",
                    "weight": 0,
                    "region": "us-east-1",
                    "pendingTimerHandle": 0,
                    "approvalWebhookUrl": "",
                    "events": ["coordinator_initialized", "canary rejected"]
                }).to_string(),
                String::from("Canary rejected by bob"),
            )
        }
    } else if method == "emergency_rollback" {
        (
            serde_json::json!({
                "service": "auth-service",
                "activeVersion": "v1.0.0",
                "canaryVersion": "",
                "weight": 0,
                "region": "us-east-1",
                "pendingTimerHandle": 0,
                "approvalWebhookUrl": "",
                "events": ["coordinator_initialized", "emergency_rollback: critical issue"]
            }).to_string(),
            String::from("Rolled back to 0%: critical issue"),
        )
    } else if method == "analyze_logs" || method == "analyze-logs" || method == "calculator" || method == "echo" || method == "get_count" || method == "stream_message" {
        ptr::write(out, RocResult::err(RocStr::from_str(&format!("Method '{}' not found, trying tool", method))));
        return;
    } else {
        (
            String::new(), // Unchanged
            format!("Agent received message: '{}'", method),
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
pub unsafe extern "C" fn roc__main_handle_tool_call_for_host_1_exposed_generic(
    state: *mut RocStr,
    tool_call_json: *mut RocStr,
    out: *mut RocResult<RocStr, RocStr>,
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
            String::new(), // read-only tool, zero state re-serialization
            String::from("{\"result\": 42}"),
            true,
        ),
        "echo" => (
            String::new(),
            serde_json::json!({ "echo": args }).to_string(),
            true,
        ),
        "get_count" => {
            let count: i64 = parsed_state
                .get("count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            (
                String::new(), // read-only tool
                serde_json::json!({ "count": count }).to_string(),
                true,
            )
        }
        "stream_message" => (
            String::new(),
            String::from("{\"status\": \"stream_active\"}"),
            true,
        ),
        "analyze_logs" | "analyze-logs" => {
            let panic_count = if args.contains("PANIC") { 1 } else { 0 };
            let oom_count = if args.contains("OOM") { 1 } else { 0 };
            let faults = panic_count + oom_count;
            (
                String::new(),
                serde_json::json!({ "faults": faults }).to_string(),
                true,
            )
        }
        _ => (
            String::new(),
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
pub unsafe extern "C" fn roc__main_metadata_for_host_1_exposed_generic(out: *mut RocStr) {
    let meta_json = r#"{
        "agents": [
            {
                "name": "roc-golem-agent",
                "version": "1.0.0",
                "description": "Durable Roc Agent on Golem Cloud",
                "mode": "durable",
                "methods": [
                    {
                        "name": "handle-message",
                        "description": "Handles text or JSON messages",
                        "prompt_hint": "",
                        "read_only": false
                    },
                    {
                        "name": "increment",
                        "description": "Increments the counter",
                        "prompt_hint": "Call to increment",
                        "read_only": false
                    },
                    {
                        "name": "get_status",
                        "description": "Returns current counter status",
                        "prompt_hint": "Call to get status",
                        "read_only": true
                    }
                ],
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
            },
            {
                "name": "canary-release",
                "version": "1.0.0",
                "description": "Canary rollout orchestrator: typed methods, a gate, a live dashboard.",
                "mode": "durable",
                "methods": [
                    {
                        "name": "start_canary",
                        "description": "Starts a new canary deployment for a given service.",
                        "prompt_hint": "Call this to initialize a canary with initial traffic.",
                        "read_only": false
                    },
                    {
                        "name": "approve_gate",
                        "description": "Approves or rejects promotion of the canary traffic weight.",
                        "prompt_hint": "Call to advance canary to next rollout stage.",
                        "read_only": false
                    },
                    {
                        "name": "emergency_rollback",
                        "description": "Rolls the canary back to 0% of traffic.",
                        "prompt_hint": "Call this to take the canary out of rotation immediately.",
                        "read_only": false
                    },
                    {
                        "name": "get_status",
                        "description": "Returns the current rollout status.",
                        "prompt_hint": "Query current canary status.",
                        "read_only": true
                    }
                ],
                "tools": [
                    {
                        "name": "analyze_logs",
                        "description": "Scans log lines for panics and OOMs",
                        "parameters": [
                            {
                                "name": "logs",
                                "description": "Raw log text to analyze",
                                "type": "string",
                                "required": true
                            }
                        ]
                    }
                ]
            },
            {
                "name": "diagnostics-worker",
                "version": "1.0.0",
                "description": "Stateless diagnostics worker for parallel log analysis",
                "mode": "ephemeral",
                "methods": [
                    {
                        "name": "run_diagnostics",
                        "description": "Scans logs without state",
                        "prompt_hint": "Call to run diagnostics",
                        "read_only": false
                    }
                ],
                "tools": []
            }
        ]
    }"#;

    ptr::write(out, RocStr::from_str(meta_json));
}
