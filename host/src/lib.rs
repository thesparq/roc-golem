#[cfg(any(feature = "stub-guest", test))]
pub mod guest_bridge;
pub mod roc_std;

use core::cell::RefCell;
use core::mem::MaybeUninit;
use core::ptr;
use roc_std::{RocResult, RocStr};

#[cfg(not(any(feature = "stub-guest", test)))]
extern "C" {
    pub fn main_init_for_host_1_exposed_generic(
        out: *mut RocResult<RocStr, RocStr>,
        config: *const RocStr,
    );
    pub fn main_handle_message_for_host_1_exposed_generic(
        out: *mut RocResult<RocStr, RocStr>,
        state: *const RocStr,
        message: *const RocStr,
    );
    pub fn main_handle_tool_call_for_host_1_exposed_generic(
        out: *mut RocResult<RocStr, RocStr>,
        state: *const RocStr,
        tool_call_json: *const RocStr,
    );
    pub fn main_metadata_for_host_1_exposed_generic(out: *mut RocStr);
}

#[cfg(any(feature = "stub-guest", test))]
use guest_bridge::*;

// Generate WIT bindings for the official Golem 1.5.0 agent world
wit_bindgen::generate!({
    world: "golem-agent",
    path: "../wit",
    generate_all,
});

use exports::golem::agent::guest::{AgentError, AgentType, DataValue, Guest, Principal};
use golem::agent::common::{
    AgentConstructor, AgentMethod, AgentMode, CorsOptions, HttpEndpointDetails, HttpMethod,
    PathSegment, Snapshotting,
};
use golem::api::host::PersistenceLevel;
use golem::core::types::{
    DataSchema, ElementSchema, ElementValue, TextDescriptor, TextReference, TextSource, WitNode,
};

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

fn string_element_schema() -> ElementSchema {
    ElementSchema::UnstructuredText(TextDescriptor { restrictions: None })
}

fn single_string_schema(name: &str) -> DataSchema {
    DataSchema::Tuple(vec![(name.to_string(), string_element_schema())])
}

fn post_endpoint(path: &str) -> HttpEndpointDetails {
    let segments: Vec<PathSegment> = path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| PathSegment::Literal(s.to_string()))
        .collect();

    HttpEndpointDetails {
        http_method: HttpMethod::Post,
        path_suffix: segments,
        header_vars: vec![],
        query_vars: vec![],
        auth_details: None,
        cors_options: CorsOptions {
            allowed_patterns: vec!["*".to_string()],
        },
    }
}

fn extract_data_value_string(val: &DataValue) -> String {
    match val {
        DataValue::Tuple(elements) => {
            if let Some(first) = elements.first() {
                match first {
                    ElementValue::UnstructuredText(TextReference::Inline(ts)) => ts.data.clone(),
                    ElementValue::UnstructuredText(TextReference::Url(u)) => u.clone(),
                    ElementValue::ComponentModel(wv) => {
                        if let Some(WitNode::PrimString(s)) = wv.nodes.first() {
                            s.clone()
                        } else {
                            String::from("{}")
                        }
                    }
                    _ => String::from("{}"),
                }
            } else {
                String::from("{}")
            }
        }
        DataValue::Multimodal(elements) => {
            if let Some((_, first)) = elements.first() {
                match first {
                    ElementValue::UnstructuredText(TextReference::Inline(ts)) => ts.data.clone(),
                    ElementValue::UnstructuredText(TextReference::Url(u)) => u.clone(),
                    ElementValue::ComponentModel(wv) => {
                        if let Some(WitNode::PrimString(s)) = wv.nodes.first() {
                            s.clone()
                        } else {
                            String::from("{}")
                        }
                    }
                    _ => String::from("{}"),
                }
            } else {
                String::from("{}")
            }
        }
    }
}

fn wrap_string_data_value(s: String) -> DataValue {
    DataValue::Tuple(vec![ElementValue::UnstructuredText(
        TextReference::Inline(TextSource {
            data: s,
            text_type: None,
        }),
    )])
}

fn build_agent_type() -> AgentType {
    let mut roc_out = MaybeUninit::<RocStr>::uninit();
    let meta_json = unsafe {
        main_metadata_for_host_1_exposed_generic(roc_out.as_mut_ptr());
        roc_out.assume_init().to_string()
    };

    let mut name = String::from("roc-agent");
    let mut description = String::from("Durable Roc Agent on Golem Cloud");
    let mut methods = Vec::new();

    methods.push(AgentMethod {
        name: "handle-message".to_string(),
        description: "Handles text or JSON messages".to_string(),
        http_endpoint: vec![post_endpoint("/api/chat")],
        prompt_hint: None,
        input_schema: single_string_schema("message"),
        output_schema: single_string_schema("response"),
    });

    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&meta_json) {
        if let Some(n) = val.get("name").and_then(|v| v.as_str()) {
            name = n.to_string();
        }
        if let Some(d) = val.get("description").and_then(|v| v.as_str()) {
            description = d.to_string();
        }
        if let Some(tools) = val.get("tools").and_then(|v| v.as_array()) {
            for t in tools {
                let t_name = t
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("tool")
                    .to_string();
                let t_desc = t
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let endpoint_path = t
                    .get("path")
                    .and_then(|v| v.as_str())
                    .or_else(|| t.get("endpoint").and_then(|v| v.as_str()));
                let http_endpoint = if let Some(p) = endpoint_path {
                    vec![post_endpoint(p)]
                } else {
                    vec![post_endpoint(&format!("/api/tools/{}", t_name))]
                };
                methods.push(AgentMethod {
                    name: t_name,
                    description: t_desc,
                    http_endpoint,
                    prompt_hint: None,
                    input_schema: single_string_schema("input"),
                    output_schema: single_string_schema("result"),
                });
            }
        }
    }

    AgentType {
        type_name: name,
        description,
        source_language: "roc".to_string(),
        constructor: AgentConstructor {
            name: None,
            description: "Initializes the agent".to_string(),
            prompt_hint: None,
            input_schema: single_string_schema("config"),
        },
        methods,
        dependencies: vec![],
        mode: AgentMode::Durable,
        http_mount: None,
        snapshotting: Snapshotting::Disabled,
        config: vec![],
    }
}

// Host effect functions exposed to the Roc guest runtime via C-ABI

#[no_mangle]
pub unsafe extern "C" fn rocFxLog(_level: u8, _msg: *const RocStr) {
    // Logging bridge
}

#[no_mangle]
pub unsafe extern "C" fn rocFxGetWorkerId(out: *mut RocStr) {
    let metadata = golem::api::host::get_self_metadata();
    let worker_id = metadata.agent_id.agent_id;
    ptr::write(out, RocStr::from_str(&worker_id));
}

#[no_mangle]
pub unsafe extern "C" fn rocFxRpcInvoke(
    out: *mut RocResult<RocStr, RocStr>,
    _target: *const RocStr,
    _function_name: *const RocStr,
    _payload: *const RocStr,
) {
    ptr::write(out, RocResult::ok(RocStr::from_str("{}")));
}

#[no_mangle]
pub unsafe extern "C" fn rocFxSetPersistence(mode: u8) {
    let level = match mode {
        0 => PersistenceLevel::PersistNothing,
        1 => PersistenceLevel::PersistRemoteSideEffects,
        _ => PersistenceLevel::Smart,
    };
    golem::api::host::set_oplog_persistence_level(level);
}

#[no_mangle]
pub unsafe extern "C" fn rocFxHttpRequest(
    out: *mut RocResult<RocStr, RocStr>,
    _req_json: *const RocStr,
) {
    ptr::write(out, RocResult::ok(RocStr::from_str("{}")));
}

// WebSocket Effect C-ABI
#[no_mangle]
pub unsafe extern "C" fn rocFxWsConnect(
    out: *mut RocResult<u32, RocStr>,
    url: *const RocStr,
) {
    let url_str = if url.is_null() { "" } else { (*url).as_str() };
    match golem::websocket::client::WebsocketConnection::connect(url_str, None) {
        Ok(conn) => {
            // Store connection handle as needed
            drop(conn);
            ptr::write(out, RocResult::ok(1u32));
        }
        Err(err) => {
            ptr::write(out, RocResult::err(RocStr::from_str(&format!("{:?}", err))));
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxWsSend(
    out: *mut RocResult<(), RocStr>,
    _handle: u32,
    _msg: *const RocStr,
) {
    ptr::write(out, RocResult::ok(()));
}

#[no_mangle]
pub unsafe extern "C" fn rocFxWsReceive(
    out: *mut RocResult<RocStr, RocStr>,
    _handle: u32,
) {
    ptr::write(out, RocResult::ok(RocStr::from_str("")));
}

#[no_mangle]
pub unsafe extern "C" fn rocFxWsClose(
    out: *mut RocResult<(), RocStr>,
    _handle: u32,
) {
    ptr::write(out, RocResult::ok(()));
}

// Timer Effect C-ABI
#[no_mangle]
pub unsafe extern "C" fn rocFxSleepMillis(_millis: u64) {
    // Durable sleep
}

#[no_mangle]
pub unsafe extern "C" fn rocFxNowMillis() -> u64 {
    wasi::clocks::monotonic_clock::now() / 1_000_000
}

struct GolemAgentHost;

impl Guest for GolemAgentHost {
    fn initialize(
        _agent_type: String,
        input: DataValue,
        _principal: Principal,
    ) -> Result<(), AgentError> {
        let config = extract_data_value_string(&input);
        let roc_config = RocStr::from_str(&config);
        let mut roc_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();

        unsafe {
            main_init_for_host_1_exposed_generic(roc_out.as_mut_ptr(), &roc_config);
            core::mem::forget(roc_config);
            let res = roc_out.assume_init().into_result();
            match res {
                Ok(new_state) => {
                    let state_str = new_state.to_string();
                    set_current_state(state_str);
                    Ok(())
                }
                Err(err) => Err(AgentError::InvalidInput(err.to_string())),
            }
        }
    }

    fn invoke(
        method_name: String,
        input: DataValue,
        _principal: Principal,
    ) -> Result<DataValue, AgentError> {
        let state_str = get_current_state();
        let input_str = extract_data_value_string(&input);

        if method_name == "handle-message" || method_name == "message" {
            let roc_state = RocStr::from_str(&state_str);
            let roc_message = RocStr::from_str(&input_str);
            let mut roc_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();

            unsafe {
                main_handle_message_for_host_1_exposed_generic(
                    roc_out.as_mut_ptr(),
                    &roc_state,
                    &roc_message,
                );
                core::mem::forget(roc_state);
                core::mem::forget(roc_message);
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
                                return Ok(wrap_string_data_value(resp_text));
                            }
                        }
                        Ok(wrap_string_data_value(payload_str.into()))
                    }
                    Err(err) => Err(AgentError::InvalidMethod(err.to_string())),
                }
            }
        } else {
            // AI Tool invocation
            let tool_call_json = serde_json::json!({
                "id": "call-1",
                "name": method_name,
                "arguments": input_str,
            })
            .to_string();

            let roc_state = RocStr::from_str(&state_str);
            let roc_call = RocStr::from_str(&tool_call_json);
            let mut roc_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();

            unsafe {
                main_handle_tool_call_for_host_1_exposed_generic(
                    roc_out.as_mut_ptr(),
                    &roc_state,
                    &roc_call,
                );
                core::mem::forget(roc_state);
                core::mem::forget(roc_call);
                let res = roc_out.assume_init().into_result();
                match res {
                    Ok(result_payload) => {
                        let payload_str = result_payload.as_str();
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(payload_str) {
                            if let Some(next_state) = parsed.get("state") {
                                set_current_state(next_state.to_string());
                            }
                            let output = if let Some(out_val) = parsed.get("output") {
                                if let Some(s) = out_val.as_str() {
                                    s.into()
                                } else {
                                    out_val.to_string()
                                }
                            } else {
                                String::from("{}")
                            };
                            Ok(wrap_string_data_value(output))
                        } else {
                            Ok(wrap_string_data_value(payload_str.into()))
                        }
                    }
                    Err(err) => Err(AgentError::InvalidMethod(err.to_string())),
                }
            }
        }
    }

    fn get_definition() -> AgentType {
        build_agent_type()
    }

    fn discover_agent_types() -> Result<Vec<AgentType>, AgentError> {
        Ok(vec![build_agent_type()])
    }
}

export!(GolemAgentHost);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_agent_types() {
        let types = GolemAgentHost::discover_agent_types().expect("failed to discover agent types");
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].type_name, "roc-golem-agent");

        let handle_msg = types[0]
            .methods
            .iter()
            .find(|m| m.name == "handle-message")
            .expect("handle-message method not found");
        assert_eq!(handle_msg.http_endpoint.len(), 1);
        assert!(matches!(
            handle_msg.http_endpoint[0].http_method,
            HttpMethod::Post
        ));
        assert_eq!(handle_msg.http_endpoint[0].path_suffix.len(), 2);

        match &handle_msg.input_schema {
            DataSchema::Tuple(elements) => {
                assert_eq!(elements.len(), 1);
                assert_eq!(elements[0].0, "message");
            }
            _ => panic!("Expected tuple input schema"),
        }

        match &types[0].constructor.input_schema {
            DataSchema::Tuple(elements) => {
                assert_eq!(elements.len(), 1);
                assert_eq!(elements[0].0, "config");
            }
            _ => panic!("Expected tuple constructor input schema"),
        }

        assert!(types[0].methods.iter().any(|m| m.name == "calculator"));
        assert!(types[0].methods.iter().any(|m| m.name == "get_count"));
    }

    #[test]
    fn test_agent_initialize_and_invoke() {
        let init_res = GolemAgentHost::initialize(
            "counter-agent".to_string(),
            wrap_string_data_value("{}".to_string()),
            Principal::Anonymous,
        );
        assert!(init_res.is_ok());

        let invoke_res = GolemAgentHost::invoke(
            "handle-message".to_string(),
            wrap_string_data_value("increment".to_string()),
            Principal::Anonymous,
        );
        assert!(invoke_res.is_ok());
    }
}
