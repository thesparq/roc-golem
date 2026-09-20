#[cfg(feature = "stub-guest")]
pub mod guest_bridge;
pub mod roc_std;

use core::cell::RefCell;
use core::mem::MaybeUninit;
use core::ptr;
use roc_std::{RocResult, RocStr};

#[cfg(feature = "stub-guest")]
use guest_bridge::*;

#[cfg(not(feature = "stub-guest"))]
extern "C" {
    pub fn roc__main_init_for_host_1_exposed_generic(
        config: *mut RocStr,
        out: *mut RocResult<RocStr, RocStr>,
    );
    pub fn roc__main_handle_message_for_host_1_exposed_generic(
        state: *mut RocStr,
        message: *mut RocStr,
        out: *mut RocResult<RocStr, RocStr>,
    );
    pub fn roc__main_handle_tool_call_for_host_1_exposed_generic(
        state: *mut RocStr,
        tool_call_json: *mut RocStr,
        out: *mut RocResult<RocStr, RocStr>,
    );
    pub fn roc__main_metadata_for_host_1_exposed_generic(out: *mut RocStr);
}


// Generate WIT bindings for the golem-agent world
wit_bindgen::generate!({
    world: "golem-agent",
    path: "../wit",
});

use exports::golem::agent_platform::agent_api::{
    AgentMetadata, Guest, ToolCall, ToolResult,
};
use golem::agent_platform::agent_types::{ToolDefinition, ToolParameter};
use golem::agent_platform::host_api::{LogLevel, PersistenceMode};

// Global worker state managed in linear memory (automatically persisted by Golem)
struct WorkerState {
    current_state_json: Option<String>,
}

thread_local! {
    static WORKER_STATE: RefCell<WorkerState> = const {
        RefCell::new(WorkerState {
            current_state_json: None,
        })
    };
}

fn get_current_state() -> String {
    WORKER_STATE.with(|s| {
        s.borrow()
            .current_state_json
            .clone()
            .unwrap_or_else(|| String::from("{}"))
    })
}

fn set_current_state(state: String) {
    WORKER_STATE.with(|s| {
        s.borrow_mut().current_state_json = Some(state);
    });
}

// Host effect functions exposed to the Roc guest runtime via C-ABI

#[no_mangle]
pub unsafe extern "C" fn rocFxLog(level: u8, msg: *const RocStr) {
    let log_level = match level {
        0 => LogLevel::Trace,
        1 => LogLevel::Debug,
        2 => LogLevel::Info,
        3 => LogLevel::Warn,
        _ => LogLevel::Error,
    };
    let message = if msg.is_null() { "" } else { (*msg).as_str() };
    host_log(log_level, message);
}

#[no_mangle]
pub unsafe extern "C" fn rocFxGetWorkerId(out: *mut RocStr) {
    let worker_id = golem::agent_platform::host_api::get_worker_id();
    ptr::write(out, RocStr::from_str(&worker_id));
}

#[no_mangle]
pub unsafe extern "C" fn rocFxRpcInvoke(
    target: *const RocStr,
    function_name: *const RocStr,
    payload: *const RocStr,
    out: *mut RocResult<RocStr, RocStr>,
) {
    let target_str = if target.is_null() { "" } else { (*target).as_str() };
    let fn_str = if function_name.is_null() { "" } else { (*function_name).as_str() };
    let payload_str = if payload.is_null() { "" } else { (*payload).as_str() };

    match golem::agent_platform::host_api::rpc_invoke(target_str, fn_str, payload_str) {
        Ok(res) => {
            ptr::write(out, RocResult::ok(RocStr::from_str(&res)));
        }
        Err(err) => {
            ptr::write(out, RocResult::err(RocStr::from_str(&err)));
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxSetPersistence(mode: u8) {
    let p_mode = match mode {
        0 => PersistenceMode::PersistNothing,
        1 => PersistenceMode::PersistStateOnly,
        _ => PersistenceMode::PersistEverything,
    };
    golem::agent_platform::host_api::set_persistence_mode(p_mode);
}

#[no_mangle]
pub unsafe extern "C" fn rocFxHttpRequest(
    req_json: *const RocStr,
    out: *mut RocResult<RocStr, RocStr>,
) {
    let req_str = if req_json.is_null() { "{}" } else { (*req_json).as_str() };
    match golem::agent_platform::host_api::http_request(req_str) {
        Ok(res) => {
            ptr::write(out, RocResult::ok(RocStr::from_str(&res)));
        }
        Err(err) => {
            ptr::write(out, RocResult::err(RocStr::from_str(&err)));
        }
    }
}

// WebSocket Effect C-ABI
#[no_mangle]
pub unsafe extern "C" fn rocFxWsConnect(
    url: *const RocStr,
    out: *mut RocResult<u32, RocStr>,
) {
    let url_str = if url.is_null() { "" } else { (*url).as_str() };
    match golem::agent_platform::websocket_api::connect(url_str) {
        Ok(handle) => {
            ptr::write(out, RocResult::ok(handle));
        }
        Err(err) => {
            ptr::write(out, RocResult::err(RocStr::from_str(&err)));
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxWsSend(
    handle: u32,
    msg: *const RocStr,
    out: *mut RocResult<(), RocStr>,
) {
    let msg_str = if msg.is_null() { "" } else { (*msg).as_str() };
    match golem::agent_platform::websocket_api::send(handle, msg_str) {
        Ok(()) => {
            ptr::write(out, RocResult::ok(()));
        }
        Err(err) => {
            ptr::write(out, RocResult::err(RocStr::from_str(&err)));
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxWsReceive(
    handle: u32,
    out: *mut RocResult<RocStr, RocStr>,
) {
    match golem::agent_platform::websocket_api::receive(handle) {
        Ok(res) => {
            ptr::write(out, RocResult::ok(RocStr::from_str(&res)));
        }
        Err(err) => {
            ptr::write(out, RocResult::err(RocStr::from_str(&err)));
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxWsClose(
    handle: u32,
    out: *mut RocResult<(), RocStr>,
) {
    match golem::agent_platform::websocket_api::close(handle) {
        Ok(()) => {
            ptr::write(out, RocResult::ok(()));
        }
        Err(err) => {
            ptr::write(out, RocResult::err(RocStr::from_str(&err)));
        }
    }
}

// Timer Effect C-ABI
#[no_mangle]
pub unsafe extern "C" fn rocFxSleepMillis(millis: u64) {
    golem::agent_platform::timer_api::sleep_millis(millis);
}

#[no_mangle]
pub unsafe extern "C" fn rocFxNowMillis() -> u64 {
    golem::agent_platform::timer_api::now_millis()
}

pub fn host_log(level: LogLevel, message: &str) {
    golem::agent_platform::host_api::log(level, message);
}

struct GolemAgentHost;

impl Guest for GolemAgentHost {
    fn init(config: String) -> Result<String, String> {
        let mut roc_config = RocStr::from_str(&config);
        let mut roc_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();

        unsafe {
            roc__main_init_for_host_1_exposed_generic(&mut roc_config, roc_out.as_mut_ptr());
            let res = roc_out.assume_init().into_result();
            match res {
                Ok(new_state) => {
                    let state_str = new_state.to_string();
                    set_current_state(state_str.clone());
                    Ok(state_str)
                }
                Err(err) => Err(err.to_string()),
            }
        }
    }

    fn handle_message(message: String) -> Result<String, String> {
        let state_str = get_current_state();

        let mut roc_state = RocStr::from_str(&state_str);
        let mut roc_message = RocStr::from_str(&message);
        let mut roc_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();

        unsafe {
            roc__main_handle_message_for_host_1_exposed_generic(
                &mut roc_state,
                &mut roc_message,
                roc_out.as_mut_ptr(),
            );
            let res = roc_out.assume_init().into_result();
            match res {
                Ok(response_payload) => {
                    let payload_str = response_payload.as_str();
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(payload_str) {
                        if let Some(next_state) = parsed.get("state") {
                            set_current_state(next_state.to_string());
                        }
                        if let Some(resp_val) = parsed.get("response") {
                            let resp_text = if let Some(s) = resp_val.as_str() {
                                s.into()
                            } else {
                                resp_val.to_string()
                            };
                            return Ok(resp_text);
                        }
                    }
                    Ok(payload_str.into())
                }
                Err(err) => Err(err.to_string()),
            }
        }
    }

    fn handle_tool_call(call: ToolCall) -> Result<ToolResult, String> {
        let state_str = get_current_state();

        let tool_call_json = serde_json::json!({
            "id": call.id,
            "name": call.name,
            "arguments": call.arguments_json,
        })
        .to_string();

        let mut roc_state = RocStr::from_str(&state_str);
        let mut roc_call = RocStr::from_str(&tool_call_json);
        let mut roc_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();

        unsafe {
            roc__main_handle_tool_call_for_host_1_exposed_generic(
                &mut roc_state,
                &mut roc_call,
                roc_out.as_mut_ptr(),
            );
            let res = roc_out.assume_init().into_result();
            match res {
                Ok(result_payload) => {
                    let payload_str = result_payload.as_str();
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(payload_str) {
                        if let Some(next_state) = parsed.get("state") {
                            set_current_state(next_state.to_string());
                        }
                        let success = parsed
                            .get("success")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true);
                        let output = if let Some(out_val) = parsed.get("output") {
                            if let Some(s) = out_val.as_str() {
                                s.into()
                            } else {
                                out_val.to_string()
                            }
                        } else {
                            String::from("{}")
                        };

                        Ok(ToolResult {
                            id: call.id,
                            success,
                            output_json: output,
                        })
                    } else {
                        Ok(ToolResult {
                            id: call.id,
                            success: true,
                            output_json: payload_str.into(),
                        })
                    }
                }
                Err(err) => Err(err.to_string()),
            }
        }
    }

    fn get_state() -> Result<String, String> {
        Ok(get_current_state())
    }

    fn get_metadata() -> AgentMetadata {
        let mut roc_out = MaybeUninit::<RocStr>::uninit();
        unsafe {
            roc__main_metadata_for_host_1_exposed_generic(roc_out.as_mut_ptr());
            let meta_roc = roc_out.assume_init();
            let meta_json = meta_roc.as_str();

            if let Ok(val) = serde_json::from_str::<serde_json::Value>(meta_json) {
                let name = val
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("roc-agent")
                    .into();
                let version = val
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("1.0.0")
                    .into();
                let description = val
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .into();

                let mut tools = Vec::new();
                if let Some(tools_arr) = val.get("tools").and_then(|v| v.as_array()) {
                    for t in tools_arr {
                        let t_name = t.get("name").and_then(|v| v.as_str()).unwrap_or("").into();
                        let t_desc = t
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .into();
                        let mut params = Vec::new();
                        if let Some(parr) = t.get("parameters").and_then(|v| v.as_array()) {
                            for p in parr {
                                params.push(ToolParameter {
                                    name: p
                                        .get("name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .into(),
                                    description: p
                                        .get("description")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .into(),
                                    param_type: p
                                        .get("type")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("string")
                                        .into(),
                                    required: p
                                        .get("required")
                                        .and_then(|v| v.as_bool())
                                        .unwrap_or(false),
                                });
                            }
                        }
                        tools.push(ToolDefinition {
                            name: t_name,
                            description: t_desc,
                            parameters: params,
                        });
                    }
                }

                AgentMetadata {
                    name,
                    version,
                    description,
                    tools,
                }
            } else {
                AgentMetadata {
                    name: String::from("roc-agent"),
                    version: String::from("1.0.0"),
                    description: String::from("Roc Golem Agent"),
                    tools: Vec::new(),
                }
            }
        }
    }
}

export!(GolemAgentHost);

#[cfg(test)]
mod tests {
    use super::guest_bridge::*;
    use super::roc_std::{RocResult, RocStr};
    use core::mem::MaybeUninit;

    #[test]
    fn test_agent_init() {
        let mut config = RocStr::from_str("{}");
        let mut out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();

        unsafe {
            roc__main_init_for_host_1_exposed_generic(&mut config, out.as_mut_ptr());
            let res = out.assume_init().into_result();
            assert!(res.is_ok());
            let state = res.unwrap().to_string();
            assert!(state.contains("\"count\": 0"));
        }
    }

    #[test]
    fn test_counter_message_flow() {
        let mut state = RocStr::from_str("{\"count\": 0}");
        let mut msg_inc = RocStr::from_str("increment");
        let mut out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();

        unsafe {
            roc__main_handle_message_for_host_1_exposed_generic(
                &mut state,
                &mut msg_inc,
                out.as_mut_ptr(),
            );
            let res = out.assume_init().into_result().expect("handle message failed");
            let payload = res.to_string();
            assert!(payload.contains("Counter incremented to 1"));
            assert!(payload.contains("\"count\":1") || payload.contains("\"count\": 1"));
        }
    }

    #[test]
    fn test_ai_tool_invocation() {
        let mut state = RocStr::from_str("{\"invocations\": 0}");
        let mut tool_call = RocStr::from_str(
            "{\"id\": \"call-calc-1\", \"name\": \"calculator\", \"arguments\": \"{\\\"expr\\\": \\\"2+2\\\"}\"}",
        );
        let mut out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();

        unsafe {
            roc__main_handle_tool_call_for_host_1_exposed_generic(
                &mut state,
                &mut tool_call,
                out.as_mut_ptr(),
            );
            let res = out.assume_init().into_result().expect("handle tool failed");
            let payload = res.to_string();
            assert!(payload.contains("\"success\":true") || payload.contains("\"success\": true"));
            assert!(payload.contains("42"));
        }
    }

    #[test]
    fn test_agent_metadata_schema() {
        let mut out = MaybeUninit::<RocStr>::uninit();
        unsafe {
            roc__main_metadata_for_host_1_exposed_generic(out.as_mut_ptr());
            let meta_str = out.assume_init().to_string();
            assert!(meta_str.contains("roc-golem-agent"));
            assert!(meta_str.contains("calculator"));
            assert!(meta_str.contains("get_count"));
        }
    }
}
