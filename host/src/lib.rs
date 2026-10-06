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
    DataSchema, ElementSchema, ElementValue, NamedWitTypeNode, TextDescriptor, TextReference,
    TextSource, WitNode, WitType, WitTypeNode, WitValue,
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
    // A plain `string`, not unstructured text: Golem's CLI renders results (and
    // validates arguments) for component-model values, while unstructured text
    // comes back as an empty tuple with "Non-ComponentModel output schema not
    // supported for result rendering".
    ElementSchema::ComponentModel(WitType {
        nodes: vec![NamedWitTypeNode {
            name: None,
            owner: None,
            type_: WitTypeNode::PrimStringType,
        }],
    })
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
    DataValue::Tuple(vec![ElementValue::ComponentModel(WitValue {
        nodes: vec![WitNode::PrimString(s)],
    })])
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
pub unsafe extern "C" fn rocFxLog(level: u8, msg: *const RocStr) {
    let text = if msg.is_null() { "" } else { (*msg).as_str() };
    let level = match level {
        0 => wasi::logging::logging::Level::Trace,
        1 => wasi::logging::logging::Level::Debug,
        2 => wasi::logging::logging::Level::Info,
        3 => wasi::logging::logging::Level::Warn,
        _ => wasi::logging::logging::Level::Error,
    };
    wasi::logging::logging::log(level, "roc-agent", text);
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
    target: *const RocStr,
    function_name: *const RocStr,
    payload: *const RocStr,
) {
    let target_str = if target.is_null() { "" } else { (*target).as_str() };
    let method = if function_name.is_null() {
        ""
    } else {
        (*function_name).as_str()
    };
    let payload_str = if payload.is_null() { "{}" } else { (*payload).as_str() };

    // The target is an agent id, as produced by `make-agent-id` / the Golem CLI
    // (for example `counter-agent("{}")`).
    let result = match golem::agent::host::parse_agent_id(target_str) {
        Ok((agent_type_name, constructor, phantom_id)) => {
            let rpc = golem::agent::host::WasmRpc::new(&agent_type_name, &constructor, phantom_id, &[]);
            let input = wrap_string_data_value(String::from(payload_str));
            match rpc.invoke_and_await(method, &input) {
                Ok(value) => Ok(extract_data_value_string(&value)),
                Err(err) => Err(format!("rpc to {} failed: {:?}", target_str, err)),
            }
        }
        Err(err) => Err(format!("invalid agent id {:?}: {:?}", target_str, err)),
    };

    match result {
        Ok(text) => ptr::write(out, RocResult::ok(RocStr::from_str(&text))),
        Err(err) => ptr::write(out, RocResult::err(RocStr::from_str(&err))),
    }
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

fn parse_url(url: &str) -> Result<(wasi::http::types::Scheme, String, String), String> {
    let (scheme, rest) = if let Some(stripped) = url.strip_prefix("https://") {
        (wasi::http::types::Scheme::Https, stripped)
    } else if let Some(stripped) = url.strip_prefix("http://") {
        (wasi::http::types::Scheme::Http, stripped)
    } else if let Some(idx) = url.find("://") {
        let (s, r) = url.split_at(idx);
        (wasi::http::types::Scheme::Other(s.to_string()), &r[3..])
    } else {
        (wasi::http::types::Scheme::Https, url)
    };

    let (authority, path_with_query) = if let Some(idx) = rest.find('/') {
        let (auth, pq) = rest.split_at(idx);
        (auth.to_string(), pq.to_string())
    } else {
        (rest.to_string(), "/".to_string())
    };

    if authority.is_empty() {
        return Err(format!("Invalid URL (empty authority): {}", url));
    }

    Ok((scheme, authority, path_with_query))
}

fn parse_method(m: &str) -> wasi::http::types::Method {
    match m.to_uppercase().as_str() {
        "GET" => wasi::http::types::Method::Get,
        "POST" => wasi::http::types::Method::Post,
        "PUT" => wasi::http::types::Method::Put,
        "DELETE" => wasi::http::types::Method::Delete,
        "PATCH" => wasi::http::types::Method::Patch,
        "HEAD" => wasi::http::types::Method::Head,
        "OPTIONS" => wasi::http::types::Method::Options,
        "CONNECT" => wasi::http::types::Method::Connect,
        "TRACE" => wasi::http::types::Method::Trace,
        other => wasi::http::types::Method::Other(other.to_string()),
    }
}

pub fn execute_http_request(req_str: &str) -> Result<String, String> {
    use wasi::http::outgoing_handler;
    use wasi::http::types::{Fields, IncomingBody, OutgoingBody, OutgoingRequest};
    use wasi::io::streams::StreamError;

    let mut method = wasi::http::types::Method::Get;
    let mut url = String::new();
    let mut custom_headers: Vec<(String, String)> = Vec::new();
    let mut body_bytes: Option<Vec<u8>> = None;

    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(req_str) {
        if let Some(obj) = parsed.as_object() {
            if let Some(u) = obj.get("url").and_then(|v| v.as_str()).or_else(|| obj.get("uri").and_then(|v| v.as_str())) {
                url = u.to_string();
            }
            if let Some(m) = obj.get("method").and_then(|v| v.as_str()) {
                method = parse_method(m);
            }
            if let Some(h) = obj.get("headers") {
                if let Some(h_obj) = h.as_object() {
                    for (k, v) in h_obj {
                        let val_str = if let Some(s) = v.as_str() {
                            s.to_string()
                        } else {
                            v.to_string()
                        };
                        custom_headers.push((k.clone(), val_str));
                    }
                } else if let Some(h_arr) = h.as_array() {
                    for item in h_arr {
                        if let Some(pair) = item.as_array() {
                            if pair.len() >= 2 {
                                let k = pair[0].as_str().unwrap_or("").to_string();
                                let v = pair[1].as_str().unwrap_or("").to_string();
                                if !k.is_empty() {
                                    custom_headers.push((k, v));
                                }
                            }
                        }
                    }
                }
            }
            if let Some(b) = obj.get("body") {
                if let Some(s) = b.as_str() {
                    body_bytes = Some(s.as_bytes().to_vec());
                } else if !b.is_null() {
                    body_bytes = serde_json::to_vec(b).ok();
                }
            }
        }
    }

    if url.is_empty() {
        if req_str.starts_with("http://") || req_str.starts_with("https://") {
            url = req_str.trim().to_string();
        } else {
            return Err(format!("Invalid HTTP request payload (missing 'url'): {}", req_str));
        }
    }

    let (scheme, authority, path_with_query) = parse_url(&url)?;

    let fields = Fields::new();
    for (name, val) in custom_headers {
        let _ = fields.set(&name, &[val.as_bytes().to_vec()]);
    }

    let request = OutgoingRequest::new(fields);
    request.set_method(&method).map_err(|_| "Failed to set HTTP method".to_string())?;
    request.set_scheme(Some(&scheme)).map_err(|_| "Failed to set HTTP scheme".to_string())?;
    request.set_authority(Some(&authority)).map_err(|_| "Failed to set HTTP authority".to_string())?;
    request.set_path_with_query(Some(&path_with_query)).map_err(|_| "Failed to set HTTP path and query".to_string())?;

    if let Some(data) = body_bytes {
        if !data.is_empty() {
            if let Ok(outgoing_body) = request.body() {
                if let Ok(stream) = outgoing_body.write() {
                    let _ = stream.blocking_write_and_flush(&data);
                    drop(stream);
                }
                let _ = OutgoingBody::finish(outgoing_body, None);
            }
        } else if let Ok(outgoing_body) = request.body() {
            let _ = OutgoingBody::finish(outgoing_body, None);
        }
    } else if let Ok(outgoing_body) = request.body() {
        let _ = OutgoingBody::finish(outgoing_body, None);
    }

    let future_response = outgoing_handler::handle(request, None)
        .map_err(|e| format!("WASI outgoing-handler failed: {:?}", e))?;

    let pollable = future_response.subscribe();
    pollable.block();
    drop(pollable);

    let incoming_response = match future_response.get() {
        Some(Ok(Ok(resp))) => resp,
        Some(Ok(Err(err))) => return Err(format!("HTTP request error code: {:?}", err)),
        Some(Err(())) => return Err("HTTP response already consumed".to_string()),
        None => return Err("HTTP response timeout".to_string()),
    };

    let status = incoming_response.status();

    let headers_resource = incoming_response.headers();
    let header_entries = headers_resource.entries();
    let mut headers_map = serde_json::Map::new();
    for (k, v) in header_entries {
        let val_str = String::from_utf8_lossy(&v).to_string();
        headers_map.insert(k, serde_json::Value::String(val_str));
    }
    drop(headers_resource);

    let body_str = match incoming_response.consume() {
        Ok(incoming_body) => {
            let body_bytes = match incoming_body.stream() {
                Ok(stream) => {
                    let mut all_bytes = Vec::new();
                    loop {
                        match stream.blocking_read(65536) {
                            Ok(bytes) => {
                                if bytes.is_empty() {
                                    break;
                                }
                                all_bytes.extend(bytes);
                            }
                            Err(StreamError::Closed) => break,
                            Err(StreamError::LastOperationFailed(_)) => break,
                        }
                    }
                    drop(stream);
                    all_bytes
                }
                Err(_) => Vec::new(),
            };
            let _ = IncomingBody::finish(incoming_body);
            String::from_utf8_lossy(&body_bytes).to_string()
        }
        Err(_) => String::new(),
    };

    let result_json = serde_json::json!({
        "status": status,
        "headers": headers_map,
        "body": body_str,
    })
    .to_string();

    Ok(result_json)
}

#[no_mangle]
pub unsafe extern "C" fn rocFxHttpRequest(
    out: *mut RocResult<RocStr, RocStr>,
    req_json: *const RocStr,
) {
    let req_str = if req_json.is_null() { "{}" } else { (*req_json).as_str() };
    match execute_http_request(req_str) {
        Ok(resp_json) => {
            ptr::write(out, RocResult::ok(RocStr::from_str(&resp_json)));
        }
        Err(err) => {
            ptr::write(out, RocResult::err(RocStr::from_str(&err)));
        }
    }
}

// WebSocket Effect C-ABI
//
// Handles are indices into this table, offset by one so that 0 is never a valid
// connection (Roc's `websocketConnect!` returns a `U32`).
thread_local! {
    static WS_CONNECTIONS: RefCell<Vec<Option<golem::websocket::client::WebsocketConnection>>> =
        const { RefCell::new(Vec::new()) };
}

fn ws_store(connection: golem::websocket::client::WebsocketConnection) -> u32 {
    WS_CONNECTIONS.with(|list| {
        let mut list = list.borrow_mut();
        if let Some(index) = list.iter().position(|slot| slot.is_none()) {
            list[index] = Some(connection);
            (index + 1) as u32
        } else {
            list.push(Some(connection));
            list.len() as u32
        }
    })
}

fn ws_take(handle: u32) -> Option<golem::websocket::client::WebsocketConnection> {
    if handle == 0 {
        return None;
    }
    WS_CONNECTIONS.with(|list| {
        let mut list = list.borrow_mut();
        list.get_mut((handle - 1) as usize).and_then(|slot| slot.take())
    })
}

fn ws_with<R>(
    handle: u32,
    f: impl FnOnce(&golem::websocket::client::WebsocketConnection) -> R,
) -> Option<R> {
    if handle == 0 {
        return None;
    }
    WS_CONNECTIONS.with(|list| {
        let list = list.borrow();
        list.get((handle - 1) as usize)
            .and_then(|slot| slot.as_ref())
            .map(f)
    })
}

fn ws_message_text(message: golem::websocket::client::Message) -> String {
    match message {
        golem::websocket::client::Message::Text(text) => text,
        golem::websocket::client::Message::Binary(bytes) => String::from_utf8_lossy(&bytes).into(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxWsConnect(
    out: *mut RocResult<u32, RocStr>,
    url: *const RocStr,
) {
    let url_str = if url.is_null() { "" } else { (*url).as_str() };
    match golem::websocket::client::WebsocketConnection::connect(url_str, None) {
        Ok(connection) => {
            let handle = ws_store(connection);
            ptr::write(out, RocResult::ok(handle));
        }
        Err(err) => {
            ptr::write(out, RocResult::err(RocStr::from_str(&format!("{:?}", err))));
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxWsSend(
    out: *mut RocResult<(), RocStr>,
    handle: u32,
    msg: *const RocStr,
) {
    let text = if msg.is_null() { "" } else { (*msg).as_str() };
    let sent = ws_with(handle, |connection| {
        connection.send(&golem::websocket::client::Message::Text(String::from(text)))
    });
    match sent {
        Some(Ok(())) => ptr::write(out, RocResult::ok(())),
        Some(Err(err)) => ptr::write(out, RocResult::err(RocStr::from_str(&format!("{:?}", err)))),
        None => ptr::write(
            out,
            RocResult::err(RocStr::from_str(&format!("unknown websocket handle {}", handle))),
        ),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxWsReceive(
    out: *mut RocResult<RocStr, RocStr>,
    handle: u32,
) {
    let received = ws_with(handle, |connection| connection.receive());
    match received {
        Some(Ok(message)) => {
            let text = ws_message_text(message);
            ptr::write(out, RocResult::ok(RocStr::from_str(&text)));
        }
        Some(Err(err)) => ptr::write(out, RocResult::err(RocStr::from_str(&format!("{:?}", err)))),
        None => ptr::write(
            out,
            RocResult::err(RocStr::from_str(&format!("unknown websocket handle {}", handle))),
        ),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxWsClose(
    out: *mut RocResult<(), RocStr>,
    handle: u32,
) {
    match ws_take(handle) {
        Some(connection) => {
            let _ = connection.close(None, None);
            drop(connection);
            ptr::write(out, RocResult::ok(()));
        }
        None => ptr::write(
            out,
            RocResult::err(RocStr::from_str(&format!("unknown websocket handle {}", handle))),
        ),
    }
}

// Timer Effect C-ABI
#[no_mangle]
pub unsafe extern "C" fn rocFxSleepMillis(millis: u64) {
    // Blocking on a WASI clock is the only sleep primitive Golem exposes; the
    // runtime records the wait in the agent's oplog, so a resumed agent does not
    // repeat it. Timer-style callbacks (rather than a blocking sleep) would use
    // the agent host's `schedule-invocation`, which this platform does not expose
    // yet.
    let nanos = millis.saturating_mul(1_000_000);
    let pollable = wasi::clocks::monotonic_clock::subscribe_duration(nanos);
    pollable.block();
    drop(pollable);
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

    #[test]
    fn test_parse_url() {
        let (scheme, auth, path) = parse_url("https://api.example.com/v1/chat?q=test").unwrap();
        assert!(matches!(scheme, wasi::http::types::Scheme::Https));
        assert_eq!(auth, "api.example.com");
        assert_eq!(path, "/v1/chat?q=test");

        let (scheme, auth, path) = parse_url("http://localhost:8080").unwrap();
        assert!(matches!(scheme, wasi::http::types::Scheme::Http));
        assert_eq!(auth, "localhost:8080");
        assert_eq!(path, "/");

        let (scheme, auth, path) = parse_url("custom://my-host/path").unwrap();
        assert!(matches!(scheme, wasi::http::types::Scheme::Other(s) if s == "custom"));
        assert_eq!(auth, "my-host");
        assert_eq!(path, "/path");
    }

    #[test]
    fn test_parse_method() {
        assert!(matches!(parse_method("get"), wasi::http::types::Method::Get));
        assert!(matches!(parse_method("POST"), wasi::http::types::Method::Post));
        assert!(matches!(parse_method("put"), wasi::http::types::Method::Put));
        assert!(matches!(parse_method("DELETE"), wasi::http::types::Method::Delete));
        assert!(matches!(parse_method("PATCH"), wasi::http::types::Method::Patch));
        assert!(matches!(parse_method("custom"), wasi::http::types::Method::Other(s) if s == "CUSTOM"));
    }
}
