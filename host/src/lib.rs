#![allow(clippy::missing_safety_doc)]

#[cfg(feature = "stub-guest")]
pub mod guest_bridge;
pub mod roc_std;

use core::cell::RefCell;
use core::mem::MaybeUninit;
use core::ptr;
use roc_std::{RocList, RocResult, RocStr};
use std::collections::HashMap;

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

// Generate WIT bindings for the official Golem 1.5.0 agent world
wit_bindgen::generate!({
    world: "golem-agent",
    path: "../wit",
    generate_all,
});

use exports::golem::agent::guest::{AgentError, AgentType, DataValue, Guest, Principal};
use golem::agent::common::{
    AgentConstructor, AgentMethod, AgentMode, CorsOptions, HttpEndpointDetails, HttpMethod,
    HttpMountDetails, PathSegment, PathVariable, Snapshotting,
};
use golem::api::host::PersistenceLevel;
use golem::core::types::{
    DataSchema, ElementSchema, ElementValue, TextDescriptor, TextReference, TextSource, WitNode,
};

#[cfg(target_arch = "wasm32")]
fn parse_url_parts(url: &str) -> (Option<wasi::http::types::Scheme>, String, String) {
    let (scheme, rest) = if let Some(stripped) = url.strip_prefix("https://") {
        (Some(wasi::http::types::Scheme::Https), stripped)
    } else if let Some(stripped) = url.strip_prefix("http://") {
        (Some(wasi::http::types::Scheme::Http), stripped)
    } else {
        (None, url)
    };

    let (authority, path_and_query) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };

    (scheme, authority.to_string(), path_and_query.to_string())
}

#[cfg(target_arch = "wasm32")]
fn parse_http_method(m: &str) -> wasi::http::types::Method {
    match m.to_uppercase().as_str() {
        "GET" => wasi::http::types::Method::Get,
        "POST" => wasi::http::types::Method::Post,
        "PUT" => wasi::http::types::Method::Put,
        "DELETE" => wasi::http::types::Method::Delete,
        "HEAD" => wasi::http::types::Method::Head,
        "PATCH" => wasi::http::types::Method::Patch,
        "OPTIONS" => wasi::http::types::Method::Options,
        "TRACE" => wasi::http::types::Method::Trace,
        "CONNECT" => wasi::http::types::Method::Connect,
        other => wasi::http::types::Method::Other(other.to_string()),
    }
}

#[cfg(target_arch = "wasm32")]
fn perform_wasi_http_request(req_str: &str, body_slice: &[u8]) -> Result<(u16, Vec<u8>), String> {
    let val: serde_json::Value = serde_json::from_str(req_str)
        .map_err(|e| format!("invalid request json: {}", e))?;

    let url_str = val.get("url").and_then(|u| u.as_str()).unwrap_or("");
    let (scheme, authority, path_and_query) = parse_url_parts(url_str);

    let method_str = val.get("method").and_then(|m| m.as_str()).unwrap_or("GET");
    let method = parse_http_method(method_str);

    let fields = wasi::http::types::Fields::new();

    if let Some(headers_obj) = val.get("headers").and_then(|h| h.as_object()) {
        for (k, v) in headers_obj {
            if let Some(s) = v.as_str() {
                let key_string = k.to_string();
                let _ = fields.set(&key_string, &[s.as_bytes().to_vec()]);
            }
        }
    }

    if let Some(headers_arr) = val.get("secretHeaders").and_then(|h| h.as_array()) {
        for h in headers_arr {
            let name = h.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let prefix = h.get("prefix").and_then(|v| v.as_str()).unwrap_or("");
            let handle = h.get("handle").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            if let Some(entry) = get_secret(handle) {
                let name_string = name.to_string();
                let full_value = format!("{}{}", prefix, entry.value);
                let _ = fields.set(&name_string, &[full_value.as_bytes().to_vec()]);
            }
        }
    }

    let request = wasi::http::types::OutgoingRequest::new(fields);
    let _ = request.set_method(&method);
    let _ = request.set_scheme(scheme.as_ref());
    let _ = request.set_authority(Some(&authority));
    let _ = request.set_path_with_query(Some(&path_and_query));

    let req_body_bytes: Vec<u8> = if !body_slice.is_empty() {
        body_slice.to_vec()
    } else if let Some(body_val) = val.get("body") {
        if let Some(s) = body_val.as_str() {
            s.as_bytes().to_vec()
        } else {
            body_val.to_string().into_bytes()
        }
    } else {
        Vec::new()
    };

    if !req_body_bytes.is_empty() {
        if let Ok(body) = request.body() {
            if let Ok(stream) = body.write() {
                let _ = stream.blocking_write_and_flush(&req_body_bytes);
            }
            let _ = wasi::http::types::OutgoingBody::finish(body, None);
        }
    } else if let Ok(body) = request.body() {
        let _ = wasi::http::types::OutgoingBody::finish(body, None);
    }

    let future = wasi::http::outgoing_handler::handle(request, None)
        .map_err(|e| format!("{:?}", e))?;
    let pollable = future.subscribe();
    pollable.block();

    let response = match future.get() {
        Some(Ok(Ok(resp))) => resp,
        Some(Ok(Err(err))) => return Err(format!("{:?}", err)),
        Some(Err(())) => return Err("response already consumed".to_string()),
        None => return Err("response not ready".to_string()),
    };

    let status = response.status();
    let body_stream = response.consume()
        .map_err(|_| "failed to consume response body".to_string())?
        .stream()
        .map_err(|_| "failed to acquire response body stream".to_string())?;

    let mut response_bytes = Vec::new();
    loop {
        match body_stream.blocking_read(65536) {
            Ok(chunk) => {
                if chunk.is_empty() {
                    break;
                }
                response_bytes.extend_from_slice(&chunk);
            }
            Err(wasi::io::streams::StreamError::Closed) => break,
            Err(wasi::io::streams::StreamError::LastOperationFailed(err)) => {
                return Err(format!("{:?}", err));
            }
        }
    }

    Ok((status, response_bytes))
}

#[cfg(target_arch = "wasm32")]
fn kv_get_wasm32(bucket_name: &str, key: &str) -> Result<Vec<u8>, String> {
    let bucket = wasi::keyvalue::types::Bucket::open_bucket(bucket_name)
        .map_err(|e| format!("{:?}", e))?;
    let key_string = key.to_string();
    let res = wasi::keyvalue::eventual::get(&bucket, &key_string)
        .map_err(|e| format!("{:?}", e))?;
    match res {
        Some(incoming) => {
            incoming.incoming_value_consume_sync()
                .map_err(|e| format!("{:?}", e))
        }
        None => Err("key not found".to_string()),
    }
}

#[cfg(target_arch = "wasm32")]
fn kv_set_wasm32(bucket_name: &str, key: &str, bytes: &[u8]) -> Result<(), String> {
    let bucket = wasi::keyvalue::types::Bucket::open_bucket(bucket_name)
        .map_err(|e| format!("{:?}", e))?;
    let outgoing = wasi::keyvalue::types::OutgoingValue::new_outgoing_value();
    let vec_bytes = bytes.to_vec();
    outgoing.outgoing_value_write_body_sync(&vec_bytes)
        .map_err(|e| format!("{:?}", e))?;
    let key_string = key.to_string();
    wasi::keyvalue::eventual::set(&bucket, &key_string, &outgoing)
        .map_err(|e| format!("{:?}", e))?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn blob_get_wasm32(container_name: &str, object_name: &str) -> Result<Vec<u8>, String> {
    let c_name = container_name.to_string();
    let container = wasi::blobstore::blobstore::get_container(&c_name)
        .map_err(|e| format!("{:?}", e))?;
    let o_name = object_name.to_string();
    let info = container.object_info(&o_name)
        .map_err(|e| format!("{:?}", e))?;
    let incoming = container.get_data(&o_name, 0, info.size)
        .map_err(|e| format!("{:?}", e))?;
    incoming.incoming_value_consume_sync()
        .map_err(|e| format!("{:?}", e))
}

#[cfg(target_arch = "wasm32")]
fn blob_put_wasm32(container_name: &str, object_name: &str, bytes: &[u8]) -> Result<(), String> {
    let c_name = container_name.to_string();
    let container = match wasi::blobstore::blobstore::get_container(&c_name) {
        Ok(c) => c,
        Err(_) => wasi::blobstore::blobstore::create_container(&c_name)
            .map_err(|e| format!("{:?}", e))?,
    };
    let outgoing = wasi::blobstore::types::OutgoingValue::new_outgoing_value();
    let stream = outgoing.outgoing_value_write_body()
        .map_err(|e| format!("{:?}", e))?;
    stream.blocking_write_and_flush(bytes)
        .map_err(|e| format!("{:?}", e))?;
    let o_name = object_name.to_string();
    container.write_data(&o_name, &outgoing)
        .map_err(|e| format!("{:?}", e))?;
    Ok(())
}

// Global worker state managed in linear memory (automatically persisted by Golem)
struct WorkerState {
    active_agent_type: Option<String>,
    current_state_json: Option<String>,
    is_ephemeral: bool,
}

#[derive(Clone, Debug)]
pub struct SecretEntry {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct MockPromise {
    pub completed: bool,
    pub payload: Vec<u8>,
}

thread_local! {
    static WORKER_STATE: RefCell<WorkerState> = const {
        RefCell::new(WorkerState {
            active_agent_type: None,
            current_state_json: None,
            is_ephemeral: false,
        })
    };
    static KV_STORE: RefCell<HashMap<(String, String), Vec<u8>>> =
        RefCell::new(HashMap::new());
    static BLOB_STORE: RefCell<HashMap<(String, String), Vec<u8>>> =
        RefCell::new(HashMap::new());
    static SECRETS: RefCell<Vec<Option<SecretEntry>>> =
        const { RefCell::new(Vec::new()) };
    pub static SECRET_ENV: RefCell<HashMap<String, String>> =
        RefCell::new(HashMap::new());
    static WS_TABLE: RefCell<Vec<Option<golem::websocket::client::WebsocketConnection>>> =
        const { RefCell::new(Vec::new()) };
    static WS_MOCK_MESSAGES: RefCell<HashMap<u32, Vec<String>>> =
        RefCell::new(HashMap::new());
    static PROMISE_TABLE: RefCell<Vec<Option<golem::core::types::PromiseId>>> =
        const { RefCell::new(Vec::new()) };
    static MOCK_PROMISES: RefCell<HashMap<u64, MockPromise>> =
        RefCell::new(HashMap::new());
}

pub fn store_secret(name: String, value: String) -> u32 {
    SECRETS.with(|table| {
        let mut slots = table.borrow_mut();
        for (idx, slot) in slots.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(SecretEntry { name, value });
                return (idx + 1) as u32;
            }
        }
        slots.push(Some(SecretEntry { name, value }));
        slots.len() as u32
    })
}

pub fn get_secret(handle: u32) -> Option<SecretEntry> {
    if handle == 0 {
        return None;
    }
    SECRETS.with(|table| {
        let slots = table.borrow();
        slots.get((handle - 1) as usize).and_then(|slot| slot.clone())
    })
}

#[allow(dead_code)]
pub fn set_mock_secret(name: &str, value: &str) {
    SECRET_ENV.with(|env| {
        env.borrow_mut().insert(name.to_string(), value.to_string());
    });
}

#[allow(dead_code)]
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
        let mut st = s.borrow_mut();
        if !st.is_ephemeral {
            st.current_state_json = Some(state);
        }
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

fn parse_single_agent_type(val: &serde_json::Value) -> AgentType {
    let mut name = String::from("roc-agent");
    let mut description = String::from("Durable Roc Agent on Golem Cloud");
    let mut methods = Vec::new();
    let mut mode = AgentMode::Durable;

    if let Some(n) = val.get("name").and_then(|v| v.as_str()) {
        name = n.to_string();
    }
    if let Some(d) = val.get("description").and_then(|v| v.as_str()) {
        description = d.to_string();
    }
    if let Some(m) = val.get("mode").and_then(|v| v.as_str()) {
        if m == "ephemeral" {
            mode = AgentMode::Ephemeral;
        }
    }

    // Declared typed methods
    if let Some(declared_methods) = val.get("methods").and_then(|v| v.as_array()) {
        for m in declared_methods {
            let m_name = m
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("method")
                .to_string();
            let m_desc = m
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let prompt_hint = m
                .get("prompt_hint")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            let endpoint_path = format!("/api/{}/{}", name, m_name);
            methods.push(AgentMethod {
                name: m_name,
                description: m_desc,
                http_endpoint: vec![post_endpoint(&endpoint_path)],
                prompt_hint,
                input_schema: single_string_schema("input"),
                output_schema: single_string_schema("result"),
            });
        }
    }

    // Declared tools
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
                .or_else(|| t.get("endpoint").and_then(|v| v.as_str()))
                .map(|p| p.to_string())
                .unwrap_or_else(|| format!("/api/tools/{}/{}", name, t_name));

            methods.push(AgentMethod {
                name: t_name.clone(),
                description: t_desc.clone(),
                http_endpoint: vec![post_endpoint(&endpoint_path)],
                prompt_hint: None,
                input_schema: single_string_schema("input"),
                output_schema: single_string_schema("result"),
            });

            // Golem 1.6: Normalize kebab-case / snake_case tool declarations
            if t_name.contains('_') {
                let kebab = t_name.replace('_', "-");
                let kebab_endpoint = format!("/api/tools/{}/{}", name, kebab);
                methods.push(AgentMethod {
                    name: kebab,
                    description: t_desc,
                    http_endpoint: vec![post_endpoint(&kebab_endpoint)],
                    prompt_hint: None,
                    input_schema: single_string_schema("input"),
                    output_schema: single_string_schema("result"),
                });
            }
        }
    }

    // Default handle-message if not present
    if !methods.iter().any(|m| m.name == "handle-message") {
        methods.push(AgentMethod {
            name: "handle-message".to_string(),
            description: "Handles text or JSON messages".to_string(),
            http_endpoint: vec![post_endpoint(&format!("/api/{}/chat", name))],
            prompt_hint: None,
            input_schema: single_string_schema("message"),
            output_schema: single_string_schema("response"),
        });
    }

    let http_mount = if name == "canary-release"
        || val.get("httpMount").is_some()
        || val.get("http_mount").is_some()
    {
        Some(HttpMountDetails {
            path_prefix: vec![
                PathSegment::Literal("canary".to_string()),
                PathSegment::PathVariable(PathVariable {
                    variable_name: "id".to_string(),
                }),
            ],
            auth_details: None,
            phantom_agent: false,
            cors_options: CorsOptions {
                allowed_patterns: vec!["*".to_string()],
            },
            webhook_suffix: vec![
                PathSegment::Literal("webhooks".to_string()),
            ],
        })
    } else {
        None
    };

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
        mode,
        http_mount,
        snapshotting: Snapshotting::Disabled,
        config: vec![],
    }
}

fn query_agent_types() -> Vec<AgentType> {
    let mut roc_out = MaybeUninit::<RocStr>::uninit();
    let meta_json = unsafe {
        roc__main_metadata_for_host_1_exposed_generic(roc_out.as_mut_ptr());
        roc_out.assume_init().to_string()
    };

    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&meta_json) {
        if let Some(agents) = val.get("agents").and_then(|v| v.as_array()) {
            return agents.iter().map(parse_single_agent_type).collect();
        }
        return vec![parse_single_agent_type(&val)];
    }

    vec![parse_single_agent_type(&serde_json::json!({}))]
}

// Host effect functions exposed to the Roc guest runtime via C-ABI

#[no_mangle]
pub unsafe extern "C" fn rocFxLog(level: u8, msg: *const RocStr) {
    let m = if msg.is_null() { "" } else { (*msg).as_str() };
    let tag = match level {
        0 => "[TRACE]",
        1 => "[DEBUG]",
        2 => "[INFO]",
        3 => "[WARN]",
        4 => "[ERROR]",
        _ => "[LOG]",
    };
    println!("{} {}", tag, m);
}

#[no_mangle]
pub unsafe extern "C" fn rocFxGetWorkerId(out: *mut RocStr) {
    #[cfg(target_arch = "wasm32")]
    {
        let metadata = golem::api::host::get_self_metadata();
        let worker_id = metadata.agent_id.agent_id;
        ptr::write(out, RocStr::from_str(&worker_id));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        ptr::write(out, RocStr::from_str("worker-local-01"));
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxRpcInvoke(
    target: *const RocStr,
    function_name: *const RocStr,
    payload: *const RocStr,
    out: *mut RocResult<RocStr, RocStr>,
) {
    let t = if target.is_null() { "" } else { (*target).as_str() };
    let f = if function_name.is_null() { "" } else { (*function_name).as_str() };
    let p = if payload.is_null() { "{}" } else { (*payload).as_str() };

    #[cfg(target_arch = "wasm32")]
    {
        let constructor_val = wrap_string_data_value("{}".to_string());
        let rpc_client = golem::agent::host::WasmRpc::new(t, &constructor_val, None, &[]);
        let input_val = wrap_string_data_value(p.to_string());
        match rpc_client.invoke_and_await(f, &input_val) {
            Ok(res_val) => {
                let res_str = extract_data_value_string(&res_val);
                ptr::write(out, RocResult::ok(RocStr::from_str(&res_str)));
            }
            Err(err) => {
                ptr::write(out, RocResult::err(RocStr::from_str(&format!("{:?}", err))));
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let resp = serde_json::json!({
            "status": "rpc_invoked",
            "target": t,
            "method": f,
            "payload": p,
        })
        .to_string();
        ptr::write(out, RocResult::ok(RocStr::from_str(&resp)));
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxRpcSchedule(
    target: *const RocStr,
    function_name: *const RocStr,
    payload: *const RocStr,
    delay_millis: u64,
    out: *mut RocResult<RocStr, RocStr>,
) {
    let t = if target.is_null() { "" } else { (*target).as_str() };
    let f = if function_name.is_null() { "" } else { (*function_name).as_str() };
    let p = if payload.is_null() { "{}" } else { (*payload).as_str() };

    let resp = serde_json::json!({
        "status": "scheduled",
        "target": t,
        "method": f,
        "payload": p,
        "delayMillis": delay_millis,
    })
    .to_string();
    ptr::write(out, RocResult::ok(RocStr::from_str(&resp)));
}

#[no_mangle]
pub unsafe extern "C" fn rocFxRpcFireAndForget(
    target: *const RocStr,
    function_name: *const RocStr,
    payload: *const RocStr,
    out: *mut RocResult<RocStr, RocStr>,
) {
    let t = if target.is_null() { "" } else { (*target).as_str() };
    let f = if function_name.is_null() { "" } else { (*function_name).as_str() };
    let p = if payload.is_null() { "{}" } else { (*payload).as_str() };

    #[cfg(target_arch = "wasm32")]
    {
        let constructor_val = wrap_string_data_value("{}".to_string());
        let rpc_client = golem::agent::host::WasmRpc::new(t, &constructor_val, None, &[]);
        let input_val = wrap_string_data_value(p.to_string());
        match rpc_client.invoke(f, &input_val) {
            Ok(()) => {
                ptr::write(out, RocResult::ok(RocStr::from_str("{\"status\":\"ok\"}")));
            }
            Err(err) => {
                ptr::write(out, RocResult::err(RocStr::from_str(&format!("{:?}", err))));
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let resp = serde_json::json!({
            "status": "rpc_fired",
            "target": t,
            "method": f,
            "payload": p,
        })
        .to_string();
        ptr::write(out, RocResult::ok(RocStr::from_str(&resp)));
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxGenerateIdempotencyKey(out: *mut RocResult<RocStr, RocStr>) {
    #[cfg(target_arch = "wasm32")]
    {
        let uuid = golem::api::host::generate_idempotency_key();
        let key_str = format!("{:016x}{:016x}", uuid.high_bits, uuid.low_bits);
        ptr::write(out, RocResult::ok(RocStr::from_str(&key_str)));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let key_str = "00000000-0000-0000-0000-000000000001".to_string();
        ptr::write(out, RocResult::ok(RocStr::from_str(&key_str)));
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxGetOplogIndex() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        golem::api::host::get_oplog_index()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        1
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxOplogCommit(replicas: u8) {
    #[cfg(target_arch = "wasm32")]
    {
        golem::api::host::oplog_commit(replicas);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = replicas;
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxCreatePromise(out: *mut RocResult<u64, RocStr>) {
    #[cfg(target_arch = "wasm32")]
    {
        let pid = golem::api::host::create_promise();
        let handle = PROMISE_TABLE.with(|table| {
            let mut slots = table.borrow_mut();
            slots.push(Some(pid));
            slots.len() as u64
        });
        ptr::write(out, RocResult::ok(handle));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let handle = MOCK_PROMISES.with(|m| {
            let mut map = m.borrow_mut();
            let next_id = (map.len() + 1) as u64;
            map.insert(next_id, MockPromise { completed: false, payload: Vec::new() });
            next_id
        });
        ptr::write(out, RocResult::ok(handle));
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxCompletePromise(
    promise_handle: u64,
    data_bytes: *const RocList<u8>,
    out: *mut RocResult<bool, RocStr>,
) {
    let slice = if data_bytes.is_null() { &[] } else { (*data_bytes).as_slice() };
    #[cfg(target_arch = "wasm32")]
    {
        let pid = PROMISE_TABLE.with(|table| {
            let slots = table.borrow();
            if promise_handle == 0 || promise_handle as usize > slots.len() {
                None
            } else {
                slots.get((promise_handle - 1) as usize).and_then(|p| p.clone())
            }
        });
        match pid {
            Some(id) => {
                let res = golem::api::host::complete_promise(&id, slice);
                ptr::write(out, RocResult::ok(res));
            }
            None => ptr::write(out, RocResult::err(RocStr::from_str("invalid promise handle"))),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let res = MOCK_PROMISES.with(|m| {
            let mut map = m.borrow_mut();
            if let Some(p) = map.get_mut(&promise_handle) {
                p.completed = true;
                p.payload = slice.to_vec();
                Ok(true)
            } else {
                Err("invalid promise handle")
            }
        });
        match res {
            Ok(b) => ptr::write(out, RocResult::ok(b)),
            Err(e) => ptr::write(out, RocResult::err(RocStr::from_str(e))),
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxGetPromise(
    promise_handle: u64,
    out: *mut RocResult<RocList<u8>, RocStr>,
) {
    #[cfg(target_arch = "wasm32")]
    {
        let pid = PROMISE_TABLE.with(|table| {
            let slots = table.borrow();
            if promise_handle == 0 || promise_handle as usize > slots.len() {
                None
            } else {
                slots.get((promise_handle - 1) as usize).and_then(|p| p.clone())
            }
        });
        match pid {
            Some(id) => {
                let promise_res = golem::api::host::get_promise(&id);
                match promise_res.get() {
                    Some(bytes) => ptr::write(out, RocResult::ok(RocList::from_slice(&bytes))),
                    None => ptr::write(out, RocResult::err(RocStr::from_str("promise not yet completed"))),
                }
            }
            None => ptr::write(out, RocResult::err(RocStr::from_str("invalid promise handle"))),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let payload = MOCK_PROMISES.with(|m| {
            let map = m.borrow();
            match map.get(&promise_handle) {
                Some(p) if p.completed => Some(Ok(p.payload.clone())),
                Some(_) => Some(Err("promise not yet completed")),
                None => None,
            }
        });
        match payload {
            Some(Ok(bytes)) => ptr::write(out, RocResult::ok(RocList::from_slice(&bytes))),
            Some(Err(err)) => ptr::write(out, RocResult::err(RocStr::from_str(err))),
            None => ptr::write(out, RocResult::err(RocStr::from_str("invalid promise handle"))),
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxCreateWebhook(
    promise_handle: u64,
    out: *mut RocResult<RocStr, RocStr>,
) {
    #[cfg(target_arch = "wasm32")]
    {
        let pid = PROMISE_TABLE.with(|table| {
            let slots = table.borrow();
            if promise_handle == 0 || promise_handle as usize > slots.len() {
                None
            } else {
                slots.get((promise_handle - 1) as usize).and_then(|p| p.clone())
            }
        });
        match pid {
            Some(id) => {
                let url = golem::agent::host::create_webhook(&id);
                ptr::write(out, RocResult::ok(RocStr::from_str(&url)));
            }
            None => ptr::write(out, RocResult::err(RocStr::from_str("invalid promise handle"))),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let valid = MOCK_PROMISES.with(|m| m.borrow().contains_key(&promise_handle));
        if valid {
            let url = format!("https://golem.cloud/webhooks/{}", promise_handle);
            ptr::write(out, RocResult::ok(RocStr::from_str(&url)));
        } else {
            ptr::write(out, RocResult::err(RocStr::from_str("invalid promise handle")));
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxSetPersistence(mode: u8) {
    let level = match mode {
        0 => PersistenceLevel::PersistNothing,
        1 => PersistenceLevel::PersistRemoteSideEffects,
        _ => PersistenceLevel::Smart,
    };
    #[cfg(target_arch = "wasm32")]
    {
        golem::api::host::set_oplog_persistence_level(level);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = level;
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxGetSecretHandle(
    name: *const RocStr,
    out: *mut RocResult<u32, RocStr>,
) {
    let n = if name.is_null() { "" } else { (*name).as_str() };
    if n.is_empty() {
        ptr::write(out, RocResult::err(RocStr::from_str("empty secret name")));
        return;
    }

    let existing_handle = SECRETS.with(|table| {
        let slots = table.borrow();
        for (idx, slot) in slots.iter().enumerate() {
            if let Some(entry) = slot {
                if entry.name == n {
                    return Some((idx + 1) as u32);
                }
            }
        }
        None
    });

    if let Some(h) = existing_handle {
        ptr::write(out, RocResult::ok(h));
        return;
    }

    let secret_val = SECRET_ENV.with(|env| env.borrow().get(n).cloned())
        .or_else(|| std::env::var(n).ok());

    match secret_val {
        Some(val) => {
            let h = store_secret(n.to_string(), val);
            ptr::write(out, RocResult::ok(h));
        }
        None => {
            ptr::write(out, RocResult::err(RocStr::from_str(&format!("secret '{}' not found", n))));
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxRevealSecret(
    handle: u32,
    out: *mut RocResult<RocStr, RocStr>,
) {
    match get_secret(handle) {
        Some(entry) => {
            ptr::write(out, RocResult::ok(RocStr::from_str(&entry.value)));
        }
        None => {
            ptr::write(
                out,
                RocResult::err(RocStr::from_str(&format!("invalid secret handle {}", handle))),
            );
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxHttpRequest(
    req_json: *const RocStr,
    out: *mut RocResult<RocStr, RocStr>,
) {
    let req_str = if req_json.is_null() { "{}" } else { (*req_json).as_str() };
    #[cfg(target_arch = "wasm32")]
    {
        match perform_wasi_http_request(req_str, &[]) {
            Ok((status, body)) => {
                let body_str = String::from_utf8_lossy(&body);
                let resp_payload = serde_json::json!({
                    "status": status,
                    "body": body_str,
                })
                .to_string();
                ptr::write(out, RocResult::ok(RocStr::from_str(&resp_payload)));
            }
            Err(err) => ptr::write(out, RocResult::err(RocStr::from_str(&err))),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut injected_headers: Vec<(String, String)> = Vec::new();

        if let Ok(val) = serde_json::from_str::<serde_json::Value>(req_str) {
            if let Some(headers) = val.get("secretHeaders").and_then(|h| h.as_array()) {
                for h in headers {
                    let name = h.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let prefix = h.get("prefix").and_then(|v| v.as_str()).unwrap_or("");
                    let handle = h.get("handle").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    if let Some(entry) = get_secret(handle) {
                        injected_headers.push((name, format!("{}{}", prefix, entry.value)));
                    }
                }
            }
        }

        let resp_payload = serde_json::json!({
            "status": 200,
            "injected_headers_count": injected_headers.len(),
        })
        .to_string();
        ptr::write(out, RocResult::ok(RocStr::from_str(&resp_payload)));
    }
}

// WebSocket Effect C-ABI
#[no_mangle]
pub unsafe extern "C" fn rocFxWsConnect(
    url: *const RocStr,
    out: *mut RocResult<u32, RocStr>,
) {
    let _url_str = if url.is_null() { "" } else { (*url).as_str() };
    #[cfg(target_arch = "wasm32")]
    {
        match golem::websocket::client::WebsocketConnection::connect(_url_str, None) {
            Ok(conn) => {
                let handle = WS_TABLE.with(|table| {
                    let mut slots = table.borrow_mut();
                    slots.push(Some(conn));
                    slots.len() as u32
                });
                ptr::write(out, RocResult::ok(handle));
            }
            Err(err) => {
                ptr::write(out, RocResult::err(RocStr::from_str(&format!("{:?}", err))));
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let handle = WS_MOCK_MESSAGES.with(|m| {
            let mut map = m.borrow_mut();
            let next_id = (map.len() + 1) as u32;
            map.insert(next_id, Vec::new());
            next_id
        });
        ptr::write(out, RocResult::ok(handle));
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxWsSend(
    handle: u32,
    msg: *const RocStr,
    out: *mut RocResult<(), RocStr>,
) {
    let msg_str = if msg.is_null() { "" } else { (*msg).as_str() };
    #[cfg(target_arch = "wasm32")]
    {
        let res = WS_TABLE.with(|table| {
            let slots = table.borrow();
            if handle == 0 || handle as usize > slots.len() {
                return Err("invalid websocket handle".to_string());
            }
            if let Some(conn) = slots.get((handle - 1) as usize).and_then(|c| c.as_ref()) {
                conn.send(&golem::websocket::client::Message::Text(msg_str.to_string()))
                    .map_err(|e| format!("{:?}", e))
            } else {
                Err("websocket connection closed".to_string())
            }
        });
        match res {
            Ok(()) => ptr::write(out, RocResult::ok(())),
            Err(e) => ptr::write(out, RocResult::err(RocStr::from_str(&e))),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let res = WS_MOCK_MESSAGES.with(|m| {
            if let Some(queue) = m.borrow_mut().get_mut(&handle) {
                queue.push(msg_str.to_string());
                Ok(())
            } else {
                Err("websocket connection closed")
            }
        });
        match res {
            Ok(()) => ptr::write(out, RocResult::ok(())),
            Err(e) => ptr::write(out, RocResult::err(RocStr::from_str(e))),
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxWsReceive(
    handle: u32,
    out: *mut RocResult<RocStr, RocStr>,
) {
    #[cfg(target_arch = "wasm32")]
    {
        let res = WS_TABLE.with(|table| {
            let slots = table.borrow();
            if handle == 0 || handle as usize > slots.len() {
                return Err("invalid websocket handle".to_string());
            }
            if let Some(conn) = slots.get((handle - 1) as usize).and_then(|c| c.as_ref()) {
                conn.receive()
                    .map_err(|e| format!("{:?}", e))
                    .map(|msg| match msg {
                        golem::websocket::client::Message::Text(t) => t,
                        golem::websocket::client::Message::Binary(b) => String::from_utf8_lossy(&b).into_owned(),
                    })
            } else {
                Err("websocket connection closed".to_string())
            }
        });
        match res {
            Ok(s) => ptr::write(out, RocResult::ok(RocStr::from_str(&s))),
            Err(e) => ptr::write(out, RocResult::err(RocStr::from_str(&e))),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let res = WS_MOCK_MESSAGES.with(|m| {
            let mut map = m.borrow_mut();
            if let Some(queue) = map.get_mut(&handle) {
                if !queue.is_empty() {
                    Ok(queue.remove(0))
                } else {
                    Ok(String::new())
                }
            } else {
                Err("websocket connection closed")
            }
        });
        match res {
            Ok(msg) => ptr::write(out, RocResult::ok(RocStr::from_str(&msg))),
            Err(e) => ptr::write(out, RocResult::err(RocStr::from_str(e))),
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxWsClose(
    handle: u32,
    out: *mut RocResult<(), RocStr>,
) {
    #[cfg(target_arch = "wasm32")]
    {
        WS_TABLE.with(|table| {
            let mut slots = table.borrow_mut();
            if handle > 0 && (handle as usize) <= slots.len() {
                if let Some(conn) = slots[(handle - 1) as usize].take() {
                    let _ = conn.close(None, None);
                }
            }
        });
        ptr::write(out, RocResult::ok(()));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        WS_MOCK_MESSAGES.with(|m| {
            m.borrow_mut().remove(&handle);
        });
        ptr::write(out, RocResult::ok(()));
    }
}

// Timer Effect C-ABI
#[no_mangle]
pub unsafe extern "C" fn rocFxSleepMillis(millis: u64) {
    if millis > 0 {
        #[cfg(target_arch = "wasm32")]
        {
            let pollable = wasi::clocks::monotonic_clock::subscribe_duration(millis.saturating_mul(1_000_000));
            pollable.block();
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::thread::sleep(std::time::Duration::from_millis(millis.min(10)));
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxNowMillis() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        wasi::clocks::monotonic_clock::now() / 1_000_000
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

// Binary and Text I/O Effects (Filesystem, Key-Value, Blobstore, and HTTP)

#[no_mangle]
pub unsafe extern "C" fn rocFxReadFileBytes(
    path: *const RocStr,
    out: *mut RocResult<RocList<u8>, RocStr>,
) {
    let p = if path.is_null() { "" } else { (*path).as_str() };
    match std::fs::read(p) {
        Ok(bytes) => ptr::write(out, RocResult::ok(RocList::from_slice(&bytes))),
        Err(err) => ptr::write(out, RocResult::err(RocStr::from_str(&err.to_string()))),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxWriteFileBytes(
    path: *const RocStr,
    bytes: *const RocList<u8>,
    out: *mut RocResult<(), RocStr>,
) {
    let p = if path.is_null() { "" } else { (*path).as_str() };
    let slice = if bytes.is_null() { &[] } else { (*bytes).as_slice() };
    match std::fs::write(p, slice) {
        Ok(()) => ptr::write(out, RocResult::ok(())),
        Err(err) => ptr::write(out, RocResult::err(RocStr::from_str(&err.to_string()))),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxAppendFileBytes(
    path: *const RocStr,
    bytes: *const RocList<u8>,
    out: *mut RocResult<(), RocStr>,
) {
    use std::io::Write;
    let p = if path.is_null() { "" } else { (*path).as_str() };
    let slice = if bytes.is_null() { &[] } else { (*bytes).as_slice() };
    match std::fs::OpenOptions::new().create(true).append(true).open(p) {
        Ok(mut f) => match f.write_all(slice) {
            Ok(()) => ptr::write(out, RocResult::ok(())),
            Err(err) => ptr::write(out, RocResult::err(RocStr::from_str(&err.to_string()))),
        },
        Err(err) => ptr::write(out, RocResult::err(RocStr::from_str(&err.to_string()))),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxReadFile(
    path: *const RocStr,
    out: *mut RocResult<RocStr, RocStr>,
) {
    let p = if path.is_null() { "" } else { (*path).as_str() };
    match std::fs::read_to_string(p) {
        Ok(s) => ptr::write(out, RocResult::ok(RocStr::from_str(&s))),
        Err(err) => ptr::write(out, RocResult::err(RocStr::from_str(&err.to_string()))),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxWriteFile(
    path: *const RocStr,
    content: *const RocStr,
    out: *mut RocResult<(), RocStr>,
) {
    let p = if path.is_null() { "" } else { (*path).as_str() };
    let s = if content.is_null() { "" } else { (*content).as_str() };
    match std::fs::write(p, s) {
        Ok(()) => ptr::write(out, RocResult::ok(())),
        Err(err) => ptr::write(out, RocResult::err(RocStr::from_str(&err.to_string()))),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxKvGetBytes(
    bucket: *const RocStr,
    key: *const RocStr,
    out: *mut RocResult<RocList<u8>, RocStr>,
) {
    let b = if bucket.is_null() { "" } else { (*bucket).as_str() };
    let k = if key.is_null() { "" } else { (*key).as_str() };
    #[cfg(target_arch = "wasm32")]
    {
        match kv_get_wasm32(b, k) {
            Ok(bytes) => ptr::write(out, RocResult::ok(RocList::from_slice(&bytes))),
            Err(e) => ptr::write(out, RocResult::err(RocStr::from_str(&e))),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let val = KV_STORE.with(|store| {
            store.borrow().get(&(b.to_string(), k.to_string())).cloned()
        });
        match val {
            Some(bytes) => ptr::write(out, RocResult::ok(RocList::from_slice(&bytes))),
            None => ptr::write(out, RocResult::err(RocStr::from_str("key not found"))),
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxKvSetBytes(
    bucket: *const RocStr,
    key: *const RocStr,
    bytes: *const RocList<u8>,
    out: *mut RocResult<(), RocStr>,
) {
    let b = if bucket.is_null() { "" } else { (*bucket).as_str() };
    let k = if key.is_null() { "" } else { (*key).as_str() };
    let slice = if bytes.is_null() { &[] } else { (*bytes).as_slice() };
    #[cfg(target_arch = "wasm32")]
    {
        match kv_set_wasm32(b, k, slice) {
            Ok(()) => ptr::write(out, RocResult::ok(())),
            Err(e) => ptr::write(out, RocResult::err(RocStr::from_str(&e))),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        KV_STORE.with(|store| {
            store.borrow_mut().insert((b.to_string(), k.to_string()), slice.to_vec());
        });
        ptr::write(out, RocResult::ok(()));
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxKvGet(
    bucket: *const RocStr,
    key: *const RocStr,
    out: *mut RocResult<RocStr, RocStr>,
) {
    let b = if bucket.is_null() { "" } else { (*bucket).as_str() };
    let k = if key.is_null() { "" } else { (*key).as_str() };
    #[cfg(target_arch = "wasm32")]
    {
        match kv_get_wasm32(b, k) {
            Ok(bytes) => {
                let s = String::from_utf8(bytes).unwrap_or_default();
                ptr::write(out, RocResult::ok(RocStr::from_str(&s)));
            }
            Err(e) => ptr::write(out, RocResult::err(RocStr::from_str(&e))),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let val = KV_STORE.with(|store| {
            store.borrow().get(&(b.to_string(), k.to_string())).cloned()
        });
        match val {
            Some(bytes) => {
                let s = String::from_utf8(bytes).unwrap_or_default();
                ptr::write(out, RocResult::ok(RocStr::from_str(&s)));
            }
            None => ptr::write(out, RocResult::err(RocStr::from_str("key not found"))),
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxKvSet(
    bucket: *const RocStr,
    key: *const RocStr,
    value: *const RocStr,
    out: *mut RocResult<(), RocStr>,
) {
    let b = if bucket.is_null() { "" } else { (*bucket).as_str() };
    let k = if key.is_null() { "" } else { (*key).as_str() };
    let v = if value.is_null() { "" } else { (*value).as_str() };
    #[cfg(target_arch = "wasm32")]
    {
        match kv_set_wasm32(b, k, v.as_bytes()) {
            Ok(()) => ptr::write(out, RocResult::ok(())),
            Err(e) => ptr::write(out, RocResult::err(RocStr::from_str(&e))),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        KV_STORE.with(|store| {
            store.borrow_mut().insert((b.to_string(), k.to_string()), v.as_bytes().to_vec());
        });
        ptr::write(out, RocResult::ok(()));
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxBlobGetBytes(
    container: *const RocStr,
    object: *const RocStr,
    out: *mut RocResult<RocList<u8>, RocStr>,
) {
    let c = if container.is_null() { "" } else { (*container).as_str() };
    let o = if object.is_null() { "" } else { (*object).as_str() };
    #[cfg(target_arch = "wasm32")]
    {
        match blob_get_wasm32(c, o) {
            Ok(bytes) => ptr::write(out, RocResult::ok(RocList::from_slice(&bytes))),
            Err(e) => ptr::write(out, RocResult::err(RocStr::from_str(&e))),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let val = BLOB_STORE.with(|store| {
            store.borrow().get(&(c.to_string(), o.to_string())).cloned()
        });
        match val {
            Some(bytes) => ptr::write(out, RocResult::ok(RocList::from_slice(&bytes))),
            None => ptr::write(out, RocResult::err(RocStr::from_str("blob not found"))),
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxBlobPutBytes(
    container: *const RocStr,
    object: *const RocStr,
    bytes: *const RocList<u8>,
    out: *mut RocResult<(), RocStr>,
) {
    let c = if container.is_null() { "" } else { (*container).as_str() };
    let o = if object.is_null() { "" } else { (*object).as_str() };
    let slice = if bytes.is_null() { &[] } else { (*bytes).as_slice() };
    #[cfg(target_arch = "wasm32")]
    {
        match blob_put_wasm32(c, o, slice) {
            Ok(()) => ptr::write(out, RocResult::ok(())),
            Err(e) => ptr::write(out, RocResult::err(RocStr::from_str(&e))),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        BLOB_STORE.with(|store| {
            store.borrow_mut().insert((c.to_string(), o.to_string()), slice.to_vec());
        });
        ptr::write(out, RocResult::ok(()));
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxBlobGet(
    container: *const RocStr,
    object: *const RocStr,
    out: *mut RocResult<RocStr, RocStr>,
) {
    let c = if container.is_null() { "" } else { (*container).as_str() };
    let o = if object.is_null() { "" } else { (*object).as_str() };
    #[cfg(target_arch = "wasm32")]
    {
        match blob_get_wasm32(c, o) {
            Ok(bytes) => {
                let s = String::from_utf8(bytes).unwrap_or_default();
                ptr::write(out, RocResult::ok(RocStr::from_str(&s)));
            }
            Err(e) => ptr::write(out, RocResult::err(RocStr::from_str(&e))),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let val = BLOB_STORE.with(|store| {
            store.borrow().get(&(c.to_string(), o.to_string())).cloned()
        });
        match val {
            Some(bytes) => {
                let s = String::from_utf8(bytes).unwrap_or_default();
                ptr::write(out, RocResult::ok(RocStr::from_str(&s)));
            }
            None => ptr::write(out, RocResult::err(RocStr::from_str("blob not found"))),
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxBlobPut(
    container: *const RocStr,
    object: *const RocStr,
    value: *const RocStr,
    out: *mut RocResult<(), RocStr>,
) {
    let c = if container.is_null() { "" } else { (*container).as_str() };
    let o = if object.is_null() { "" } else { (*object).as_str() };
    let v = if value.is_null() { "" } else { (*value).as_str() };
    #[cfg(target_arch = "wasm32")]
    {
        match blob_put_wasm32(c, o, v.as_bytes()) {
            Ok(()) => ptr::write(out, RocResult::ok(())),
            Err(e) => ptr::write(out, RocResult::err(RocStr::from_str(&e))),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        BLOB_STORE.with(|store| {
            store.borrow_mut().insert((c.to_string(), o.to_string()), v.as_bytes().to_vec());
        });
        ptr::write(out, RocResult::ok(()));
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxHttpRequestBytes(
    req_json: *const RocStr,
    body_bytes: *const RocList<u8>,
    out: *mut RocResult<RocList<u8>, RocStr>,
) {
    let _req_str = if req_json.is_null() { "{}" } else { (*req_json).as_str() };
    let slice = if body_bytes.is_null() { &[] } else { (*body_bytes).as_slice() };
    #[cfg(target_arch = "wasm32")]
    {
        match perform_wasi_http_request(_req_str, slice) {
            Ok((_status, body)) => {
                ptr::write(out, RocResult::ok(RocList::from_slice(&body)));
            }
            Err(err) => ptr::write(out, RocResult::err(RocStr::from_str(&err))),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        ptr::write(out, RocResult::ok(RocList::from_slice(slice)));
    }
}

// Configuration Store Effect C-ABI
#[no_mangle]
pub unsafe extern "C" fn rocFxConfigGet(
    key: *const RocStr,
    out: *mut RocResult<RocStr, RocStr>,
) {
    let k = if key.is_null() { "" } else { (*key).as_str() };
    #[cfg(target_arch = "wasm32")]
    {
        let key_string = k.to_string();
        match wasi::config::store::get(&key_string) {
            Ok(Some(v)) => ptr::write(out, RocResult::ok(RocStr::from_str(&v))),
            Ok(None) => ptr::write(out, RocResult::err(RocStr::from_str("config key not found"))),
            Err(e) => ptr::write(out, RocResult::err(RocStr::from_str(&format!("{:?}", e)))),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        match std::env::var(k) {
            Ok(v) => ptr::write(out, RocResult::ok(RocStr::from_str(&v))),
            Err(_) => ptr::write(out, RocResult::err(RocStr::from_str("config key not found"))),
        }
    }
}

// Distributed Tracing Context Effect C-ABI
#[no_mangle]
pub unsafe extern "C" fn rocFxTraceId(out: *mut RocStr) {
    #[cfg(target_arch = "wasm32")]
    {
        let trace = golem::api::context::current_context().trace_id();
        ptr::write(out, RocStr::from_str(&trace));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        ptr::write(out, RocStr::from_str("4bf92f3577b34da6a3ce929d0e0e4736"));
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxSpanId(out: *mut RocStr) {
    #[cfg(target_arch = "wasm32")]
    {
        let span = golem::api::context::current_context().span_id();
        ptr::write(out, RocStr::from_str(&span));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        ptr::write(out, RocStr::from_str("00f067aa0ba902b7"));
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxTraceHeaders(out: *mut RocResult<RocStr, RocStr>) {
    #[cfg(target_arch = "wasm32")]
    {
        let headers = golem::api::context::current_context().trace_context_headers();
        let mut map = serde_json::Map::new();
        for (k, v) in headers {
            map.insert(k, serde_json::Value::String(v));
        }
        let json_str = serde_json::Value::Object(map).to_string();
        ptr::write(out, RocResult::ok(RocStr::from_str(&json_str)));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mock_headers = serde_json::json!({
            "traceparent": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
        }).to_string();
        ptr::write(out, RocResult::ok(RocStr::from_str(&mock_headers)));
    }
}

// Idempotence & Oplog Revert Effect C-ABI
#[no_mangle]
pub unsafe extern "C" fn rocFxGetIdempotenceMode() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        golem::api::host::get_idempotence_mode()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        true
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxSetIdempotenceMode(mode: bool) {
    #[cfg(target_arch = "wasm32")]
    {
        golem::api::host::set_idempotence_mode(mode);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = mode;
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxRevertAgent(
    oplog_index: u64,
    out: *mut RocResult<(), RocStr>,
) {
    #[cfg(target_arch = "wasm32")]
    {
        let metadata = golem::api::host::get_self_metadata();
        let target = golem::api::host::RevertAgentTarget::RevertToOplogIndex(oplog_index);
        golem::api::host::revert_agent(&metadata.agent_id, target);
        ptr::write(out, RocResult::ok(()));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = oplog_index;
        ptr::write(out, RocResult::ok(()));
    }
}

// Durable RDBMS (PostgreSQL & MySQL) Effect C-ABI
#[no_mangle]
pub unsafe extern "C" fn rocFxDbExecute(
    conn_str: *const RocStr,
    sql: *const RocStr,
    out: *mut RocResult<u64, RocStr>,
) {
    let c = if conn_str.is_null() { "" } else { (*conn_str).as_str() };
    let s = if sql.is_null() { "" } else { (*sql).as_str() };
    #[cfg(target_arch = "wasm32")]
    {
        let c_str = c.to_string();
        let s_str = s.to_string();
        let conn_res = if c.starts_with("mysql://") {
            match golem::rdbms::mysql::DbConnection::open(&c_str) {
                Ok(conn) => match conn.execute(&s_str, &[]) {
                    Ok(count) => Ok(count),
                    Err(e) => Err(format!("{:?}", e)),
                },
                Err(e) => Err(format!("{:?}", e)),
            }
        } else {
            match golem::rdbms::postgres::DbConnection::open(&c_str) {
                Ok(conn) => match conn.execute(&s_str, Vec::new()) {
                    Ok(count) => Ok(count),
                    Err(e) => Err(format!("{:?}", e)),
                },
                Err(e) => Err(format!("{:?}", e)),
            }
        };
        match conn_res {
            Ok(count) => ptr::write(out, RocResult::ok(count)),
            Err(e) => ptr::write(out, RocResult::err(RocStr::from_str(&e))),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (c, s);
        ptr::write(out, RocResult::ok(1));
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxDbQuery(
    conn_str: *const RocStr,
    sql: *const RocStr,
    out: *mut RocResult<RocStr, RocStr>,
) {
    let c = if conn_str.is_null() { "" } else { (*conn_str).as_str() };
    let s = if sql.is_null() { "" } else { (*sql).as_str() };
    #[cfg(target_arch = "wasm32")]
    {
        let c_str = c.to_string();
        let s_str = s.to_string();
        let query_res = if c.starts_with("mysql://") {
            match golem::rdbms::mysql::DbConnection::open(&c_str) {
                Ok(conn) => match conn.query(&s_str, &[]) {
                    Ok(res) => {
                        let cols: Vec<String> = res.columns.into_iter().map(|col| col.name).collect();
                        let rows_count = res.rows.len();
                        Ok(serde_json::json!({
                            "columns": cols,
                            "rowCount": rows_count,
                        }).to_string())
                    }
                    Err(e) => Err(format!("{:?}", e)),
                },
                Err(e) => Err(format!("{:?}", e)),
            }
        } else {
            match golem::rdbms::postgres::DbConnection::open(&c_str) {
                Ok(conn) => match conn.query(&s_str, Vec::new()) {
                    Ok(res) => {
                        let cols: Vec<String> = res.columns.into_iter().map(|col| col.name).collect();
                        let rows_count = res.rows.len();
                        Ok(serde_json::json!({
                            "columns": cols,
                            "rowCount": rows_count,
                        }).to_string())
                    }
                    Err(e) => Err(format!("{:?}", e)),
                },
                Err(e) => Err(format!("{:?}", e)),
            }
        };
        match query_res {
            Ok(json_res) => ptr::write(out, RocResult::ok(RocStr::from_str(&json_res))),
            Err(e) => ptr::write(out, RocResult::err(RocStr::from_str(&e))),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (c, s);
        let mock_res = serde_json::json!({
            "columns": ["id", "status"],
            "rowCount": 1
        }).to_string();
        ptr::write(out, RocResult::ok(RocStr::from_str(&mock_res)));
    }
}

struct GolemAgentHost;

impl Guest for GolemAgentHost {
    fn initialize(
        agent_type: String,
        input: DataValue,
        _principal: Principal,
    ) -> Result<(), AgentError> {
        let already_init = WORKER_STATE.with(|s| s.borrow().active_agent_type.is_some());
        if already_init {
            return Err(AgentError::InvalidInput(
                "Worker instance already initialized".to_string(),
            ));
        }

        let discovered = query_agent_types();
        let matching = discovered.iter().find(|t| t.type_name == agent_type);
        let is_ephemeral = match matching {
            Some(t) => matches!(t.mode, AgentMode::Ephemeral),
            None => {
                if discovered.len() == 1 {
                    matches!(discovered[0].mode, AgentMode::Ephemeral)
                } else {
                    return Err(AgentError::InvalidType(format!(
                        "Agent type '{}' is not registered in this component",
                        agent_type
                    )));
                }
            }
        };

        let config = extract_data_value_string(&input);
        let envelope = serde_json::json!({
            "agent_type": agent_type,
            "config": config,
        })
        .to_string();

        let mut roc_config = RocStr::from_str(&envelope);
        let mut roc_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();

        unsafe {
            roc__main_init_for_host_1_exposed_generic(&mut roc_config, roc_out.as_mut_ptr());
            let res = roc_out.assume_init().into_result();
            match res {
                Ok(new_state) => {
                    let state_str = new_state.to_string();
                    WORKER_STATE.with(|s| {
                        let mut st = s.borrow_mut();
                        st.active_agent_type = Some(agent_type);
                        st.is_ephemeral = is_ephemeral;
                        if !is_ephemeral {
                            st.current_state_json = Some(state_str);
                        } else {
                            st.current_state_json = Some("{}".to_string());
                        }
                    });
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
        let (active_agent, is_ephemeral, state_str) = WORKER_STATE.with(|s| {
            let st = s.borrow();
            (
                st.active_agent_type.clone(),
                st.is_ephemeral,
                st.current_state_json
                    .clone()
                    .unwrap_or_else(|| String::from("{}")),
            )
        });

        let target_agent = active_agent.unwrap_or_else(|| String::from("roc-agent"));
        let input_str = extract_data_value_string(&input);

        // Envelope encoding for guest dispatch
        let envelope = serde_json::json!({
            "agent_type": target_agent,
            "method": method_name,
            "input": input_str,
        })
        .to_string();

        let pass_state = if is_ephemeral { "{}" } else { &state_str };

        let mut roc_state = RocStr::from_str(pass_state);
        let mut roc_message = RocStr::from_str(&envelope);
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
                        // ZERO-SERIALIZATION BYPASS:
                        // If "state" is non-empty and agent is durable, update host state.
                        // If "state" is empty (""), state was NOT modified - skip update completely!
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
                Err(err) => {
                    // Fallback to tool invocation (support both kebab-case and snake_case)
                    let tool_name = if method_name.contains('-') {
                        method_name.replace('-', "_")
                    } else {
                        method_name.clone()
                    };
                    let tool_call_json = serde_json::json!({
                        "id": "call-1",
                        "name": tool_name,
                        "arguments": input_str,
                        "agent_type": target_agent,
                    })
                    .to_string();

                    let mut roc_state = RocStr::from_str(pass_state);
                    let mut roc_call = RocStr::from_str(&tool_call_json);
                    let mut roc_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();

                    roc__main_handle_tool_call_for_host_1_exposed_generic(
                        &mut roc_state,
                        &mut roc_call,
                        roc_out.as_mut_ptr(),
                    );
                    let res = roc_out.assume_init().into_result();
                    match res {
                        Ok(result_payload) => {
                            let payload_str = result_payload.as_str();
                            if let Ok(parsed) =
                                serde_json::from_str::<serde_json::Value>(payload_str)
                            {
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
                        Err(_) => Err(AgentError::InvalidMethod(err.to_string())),
                    }
                }
            }
        }
    }

    fn get_definition() -> AgentType {
        let active = WORKER_STATE.with(|s| s.borrow().active_agent_type.clone());
        let mut types = query_agent_types();
        if let Some(name) = active {
            if let Some(idx) = types.iter().position(|t| t.type_name == name) {
                return types.remove(idx);
            }
        }
        if !types.is_empty() {
            return types.remove(0);
        }
        parse_single_agent_type(&serde_json::json!({
            "name": "roc-agent",
            "description": "Durable Roc Agent on Golem Cloud"
        }))
    }

    fn discover_agent_types() -> Result<Vec<AgentType>, AgentError> {
        Ok(query_agent_types())
    }
}

export!(GolemAgentHost);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_agent_types() {
        let types = GolemAgentHost::discover_agent_types().expect("failed to discover agent types");
        assert_eq!(types.len(), 3);
        assert_eq!(types[0].type_name, "roc-golem-agent");
        assert_eq!(types[1].type_name, "canary-release");
        assert_eq!(types[2].type_name, "diagnostics-worker");

        // Durable agent verification
        assert!(matches!(types[0].mode, AgentMode::Durable));
        let inc_msg = types[0]
            .methods
            .iter()
            .find(|m| m.name == "increment")
            .expect("increment method not found");
        assert_eq!(inc_msg.http_endpoint.len(), 1);

        // Canary release durable agent verification
        assert!(matches!(types[1].mode, AgentMode::Durable));
        let canary_start = types[1]
            .methods
            .iter()
            .find(|m| m.name == "start_canary")
            .expect("start_canary method not found");
        assert_eq!(canary_start.http_endpoint.len(), 1);
        let canary_ro = types[1]
            .methods
            .iter()
            .find(|m| m.name == "get_status")
            .expect("get_status method not found");
        assert_eq!(canary_ro.http_endpoint.len(), 1);

        // Verify http_mount for /canary/{id} and tool normalization
        assert!(types[1].http_mount.is_some());
        let mount = types[1].http_mount.as_ref().unwrap();
        assert_eq!(mount.path_prefix.len(), 2);
        assert!(types[1].methods.iter().any(|m| m.name == "analyze_logs"));
        assert!(types[1].methods.iter().any(|m| m.name == "analyze-logs"));

        // Ephemeral agent verification
        assert!(matches!(types[2].mode, AgentMode::Ephemeral));
        let diag_msg = types[2]
            .methods
            .iter()
            .find(|m| m.name == "run_diagnostics")
            .expect("run_diagnostics method not found");
        assert_eq!(diag_msg.http_endpoint.len(), 1);
    }

    #[test]
    fn test_agent_initialize_and_invoke() {
        // Reset state for test
        WORKER_STATE.with(|s| {
            let mut st = s.borrow_mut();
            st.active_agent_type = None;
            st.current_state_json = None;
            st.is_ephemeral = false;
        });

        let init_res = GolemAgentHost::initialize(
            "roc-golem-agent".to_string(),
            wrap_string_data_value("{}".to_string()),
            Principal::Anonymous,
        );
        assert!(init_res.is_ok());

        // Increment mutating call
        let invoke_res = GolemAgentHost::invoke(
            "increment".to_string(),
            wrap_string_data_value("".to_string()),
            Principal::Anonymous,
        );
        assert!(invoke_res.is_ok());
        let current_state = get_current_state();
        assert!(current_state.contains("\"count\":1"));

        // Read-only call (zero-serialization bypass - state must remain count=1 without rewrite)
        let get_res = GolemAgentHost::invoke(
            "get_status".to_string(),
            wrap_string_data_value("".to_string()),
            Principal::Anonymous,
        );
        assert!(get_res.is_ok());
        assert_eq!(get_current_state(), current_state);
    }

    #[test]
    fn test_ephemeral_worker_lifecycle() {
        WORKER_STATE.with(|s| {
            let mut st = s.borrow_mut();
            st.active_agent_type = None;
            st.current_state_json = None;
            st.is_ephemeral = false;
        });

        let init_res = GolemAgentHost::initialize(
            "diagnostics-worker".to_string(),
            wrap_string_data_value("{}".to_string()),
            Principal::Anonymous,
        );
        assert!(init_res.is_ok());

        let invoke_res = GolemAgentHost::invoke(
            "run_diagnostics".to_string(),
            wrap_string_data_value("{\"service\":\"web\",\"lines\":[]}".to_string()),
            Principal::Anonymous,
        );
        assert!(invoke_res.is_ok());
        let val_str = extract_data_value_string(&invoke_res.unwrap());
        assert!(val_str.contains("\"healthy\": true") || val_str.contains("\"healthy\":true"));

        // State remains empty/unit for ephemeral worker
        assert_eq!(get_current_state(), "{}");
    }

    #[test]
    fn test_canary_release_suite() {
        // Assertion 1: Multi-Agent Discovery
        let types = GolemAgentHost::discover_agent_types().expect("failed to discover agent types");
        assert!(types.iter().any(|t| t.type_name == "canary-release" && matches!(t.mode, AgentMode::Durable)));
        assert!(types.iter().any(|t| t.type_name == "diagnostics-worker" && matches!(t.mode, AgentMode::Ephemeral)));

        // Assertion 2: Ephemeral Direct Invocation
        WORKER_STATE.with(|s| {
            let mut st = s.borrow_mut();
            st.active_agent_type = None;
            st.current_state_json = None;
            st.is_ephemeral = false;
        });
        let diag_init = GolemAgentHost::initialize(
            "diagnostics-worker".to_string(),
            wrap_string_data_value("{}".to_string()),
            Principal::Anonymous,
        );
        assert!(diag_init.is_ok());
        let diag_run = GolemAgentHost::invoke(
            "run_diagnostics".to_string(),
            wrap_string_data_value("{\"service\":\"auth-service\",\"lines\":\"log line 1\\nlog line 2\"}".to_string()),
            Principal::Anonymous,
        );
        assert!(diag_run.is_ok());
        let diag_str = extract_data_value_string(&diag_run.unwrap());
        assert!(diag_str.contains("\"faults\": 0") || diag_str.contains("\"faults\":0"));

        // Assertion 3 & 4: Canary Release Coordinator (Durable) - Clean Start & Read-Only Status
        WORKER_STATE.with(|s| {
            let mut st = s.borrow_mut();
            st.active_agent_type = None;
            st.current_state_json = None;
            st.is_ephemeral = false;
        });
        let canary_init = GolemAgentHost::initialize(
            "canary-release".to_string(),
            wrap_string_data_value("{\"service\":\"auth-service\"}".to_string()),
            Principal::Anonymous,
        );
        assert!(canary_init.is_ok());

        // Invoke start_canary with healthy logs
        let start_res = GolemAgentHost::invoke(
            "start_canary".to_string(),
            wrap_string_data_value("{\"version\":\"v1.1.0\",\"weight\":10,\"logs\":\"all normal\",\"permissionCard\":\"\"}".to_string()),
            Principal::Anonymous,
        );
        assert!(start_res.is_ok());
        let start_out = extract_data_value_string(&start_res.unwrap());
        assert!(start_out.contains("canary_started"));
        assert!(start_out.contains("\"weight\":10") || start_out.contains("\"weight\": 10"));

        let state_after_start = get_current_state();
        assert!(state_after_start.contains("\"weight\":10") || state_after_start.contains("\"weight\": 10"));
        assert!(state_after_start.contains("\"canaryVersion\":\"v1.1.0\"") || state_after_start.contains("\"canaryVersion\": \"v1.1.0\""));

        // Assertion 4: Read-only get_status skips state re-serialization
        let status_res = GolemAgentHost::invoke(
            "get_status".to_string(),
            wrap_string_data_value("{}".to_string()),
            Principal::Anonymous,
        );
        assert!(status_res.is_ok());
        assert_eq!(get_current_state(), state_after_start);

        // Assertion 6: Gate Approval promotes canary
        let approve_res = GolemAgentHost::invoke(
            "approve_gate".to_string(),
            wrap_string_data_value("{\"approved\":true,\"reviewer\":\"alice\"}".to_string()),
            Principal::Anonymous,
        );
        assert!(approve_res.is_ok());
        let state_after_approve = get_current_state();
        assert!(state_after_approve.contains("\"weight\":100") || state_after_approve.contains("\"weight\": 100"));
        assert!(state_after_approve.contains("\"activeVersion\":\"v1.1.0\"") || state_after_approve.contains("\"activeVersion\": \"v1.1.0\""));

        // Assertion 6b: Emergency Rollback
        let rollback_res = GolemAgentHost::invoke(
            "emergency_rollback".to_string(),
            wrap_string_data_value("{\"reason\":\"critical memory leak detected\"}".to_string()),
            Principal::Anonymous,
        );
        assert!(rollback_res.is_ok());
        let state_after_rollback = get_current_state();
        assert!(state_after_rollback.contains("\"weight\":0") || state_after_rollback.contains("\"weight\": 0"));

        // Assertion 5: Unhealthy Log Abort
        let unhealthy_start_res = GolemAgentHost::invoke(
            "start_canary".to_string(),
            wrap_string_data_value("{\"version\":\"v1.2.0\",\"weight\":10,\"logs\":\"PANIC: fatal segfault\\nOOM\",\"permissionCard\":\"\"}".to_string()),
            Principal::Anonymous,
        );
        assert!(unhealthy_start_res.is_ok());
        let unhealthy_out = extract_data_value_string(&unhealthy_start_res.unwrap());
        assert!(unhealthy_out.contains("aborted_unhealthy"));
        // Weight remains 0
        let state_after_unhealthy = get_current_state();
        assert!(state_after_unhealthy.contains("\"weight\":0") || state_after_unhealthy.contains("\"weight\": 0"));

        // Assertion 7: Tool Surface Parity (analyze_logs)
        let tool_call_res = GolemAgentHost::invoke(
            "analyze_logs".to_string(),
            wrap_string_data_value("service log line\\nPANIC: out of memory\\nworker line".to_string()),
            Principal::Anonymous,
        );
        assert!(tool_call_res.is_ok());
        let tool_out = extract_data_value_string(&tool_call_res.unwrap());
        assert!(tool_out.contains("\"faults\":1") || tool_out.contains("\"faults\": 1"));

        // Kebab-case tool invocation normalization
        let kebab_tool_res = GolemAgentHost::invoke(
            "analyze-logs".to_string(),
            wrap_string_data_value("service log line\\nPANIC: fatal\\nOOM: heap limit".to_string()),
            Principal::Anonymous,
        );
        assert!(kebab_tool_res.is_ok());
        let kebab_out = extract_data_value_string(&kebab_tool_res.unwrap());
        assert!(kebab_out.contains("\"faults\":2") || kebab_out.contains("\"faults\": 2"));
    }

    #[test]
    fn test_double_initialization_fails() {
        WORKER_STATE.with(|s| {
            let mut st = s.borrow_mut();
            st.active_agent_type = None;
            st.current_state_json = None;
            st.is_ephemeral = false;
        });

        let init_1 = GolemAgentHost::initialize(
            "roc-golem-agent".to_string(),
            wrap_string_data_value("{}".to_string()),
            Principal::Anonymous,
        );
        assert!(init_1.is_ok());

        let init_2 = GolemAgentHost::initialize(
            "roc-golem-agent".to_string(),
            wrap_string_data_value("{}".to_string()),
            Principal::Anonymous,
        );
        assert!(init_2.is_err());
    }

    #[test]
    fn test_unknown_agent_type_fails() {
        WORKER_STATE.with(|s| {
            let mut st = s.borrow_mut();
            st.active_agent_type = None;
            st.current_state_json = None;
            st.is_ephemeral = false;
        });

        let init_res = GolemAgentHost::initialize(
            "non-existent-agent".to_string(),
            wrap_string_data_value("{}".to_string()),
            Principal::Anonymous,
        );
        assert!(init_res.is_err());
    }

    #[test]
    fn test_binary_io_effects() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("roc_binary_test.bin");
        let path_str = test_file.to_str().unwrap();

        let binary_payload = vec![0x00u8, 0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0xFF, 0xFE];
        let roc_path = RocStr::from_str(path_str);
        let roc_bytes = RocList::from_slice(&binary_payload);

        let mut write_out = MaybeUninit::<RocResult<(), RocStr>>::uninit();
        unsafe {
            rocFxWriteFileBytes(&roc_path, &roc_bytes, write_out.as_mut_ptr());
            assert!(write_out.assume_init().is_ok());
        }

        let mut read_out = MaybeUninit::<RocResult<RocList<u8>, RocStr>>::uninit();
        unsafe {
            rocFxReadFileBytes(&roc_path, read_out.as_mut_ptr());
            let res = read_out.assume_init().into_result().expect("read failed");
            assert_eq!(res.as_slice(), binary_payload.as_slice());
        }

        let _ = std::fs::remove_file(test_file);

        // Key-Value binary roundtrip
        let bucket = RocStr::from_str("test-bucket");
        let key = RocStr::from_str("binary-key");
        let mut kv_set_out = MaybeUninit::<RocResult<(), RocStr>>::uninit();
        unsafe {
            rocFxKvSetBytes(&bucket, &key, &roc_bytes, kv_set_out.as_mut_ptr());
            assert!(kv_set_out.assume_init().is_ok());
        }

        let mut kv_get_out = MaybeUninit::<RocResult<RocList<u8>, RocStr>>::uninit();
        unsafe {
            rocFxKvGetBytes(&bucket, &key, kv_get_out.as_mut_ptr());
            let res = kv_get_out.assume_init().into_result().expect("kv get failed");
            assert_eq!(res.as_slice(), binary_payload.as_slice());
        }

        // Blobstore binary roundtrip
        let container = RocStr::from_str("test-container");
        let object = RocStr::from_str("binary-blob");
        let mut blob_put_out = MaybeUninit::<RocResult<(), RocStr>>::uninit();
        unsafe {
            rocFxBlobPutBytes(&container, &object, &roc_bytes, blob_put_out.as_mut_ptr());
            assert!(blob_put_out.assume_init().is_ok());
        }

        let mut blob_get_out = MaybeUninit::<RocResult<RocList<u8>, RocStr>>::uninit();
        unsafe {
            rocFxBlobGetBytes(&container, &object, blob_get_out.as_mut_ptr());
            let res = blob_get_out.assume_init().into_result().expect("blob get failed");
            assert_eq!(res.as_slice(), binary_payload.as_slice());
        }
    }

    #[test]
    fn test_secret_handles_and_header_injection() {
        set_mock_secret("TEST_API_KEY", "super-secret-token-12345");

        // 1. Acquire secret handle
        let secret_name = RocStr::from_str("TEST_API_KEY");
        let mut handle_out = MaybeUninit::<RocResult<u32, RocStr>>::uninit();
        let handle = unsafe {
            rocFxGetSecretHandle(&secret_name, handle_out.as_mut_ptr());
            handle_out.assume_init().into_result().expect("failed to get secret handle")
        };
        assert!(handle > 0);

        // 2. Handle deduplication
        let mut handle_out_2 = MaybeUninit::<RocResult<u32, RocStr>>::uninit();
        let handle_2 = unsafe {
            rocFxGetSecretHandle(&secret_name, handle_out_2.as_mut_ptr());
            handle_out_2.assume_init().into_result().expect("failed to get existing handle")
        };
        assert_eq!(handle, handle_2);

        // 3. Unknown secret error
        let unknown_name = RocStr::from_str("NON_EXISTENT_SECRET");
        let mut unknown_out = MaybeUninit::<RocResult<u32, RocStr>>::uninit();
        unsafe {
            rocFxGetSecretHandle(&unknown_name, unknown_out.as_mut_ptr());
            assert!(unknown_out.assume_init().into_result().is_err());
        }

        // 4. Reveal secret
        let mut reveal_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();
        let revealed = unsafe {
            rocFxRevealSecret(handle, reveal_out.as_mut_ptr());
            reveal_out.assume_init().into_result().expect("failed to reveal secret")
        };
        assert_eq!(revealed.as_str(), "super-secret-token-12345");

        // 5. Reveal invalid handle fails
        let mut invalid_reveal_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();
        unsafe {
            rocFxRevealSecret(99999, invalid_reveal_out.as_mut_ptr());
            assert!(invalid_reveal_out.assume_init().into_result().is_err());
        }

        // 6. Host-mediated injection in HTTP request
        let http_req_json = serde_json::json!({
            "url": "https://api.vendor.com/v1/action",
            "method": "POST",
            "secretHeaders": [
                {
                    "name": "Authorization",
                    "prefix": "Bearer ",
                    "handle": handle
                }
            ]
        }).to_string();
        let roc_req = RocStr::from_str(&http_req_json);
        let mut http_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();
        let http_res = unsafe {
            rocFxHttpRequest(&roc_req, http_out.as_mut_ptr());
            http_out.assume_init().into_result().expect("http request failed")
        };
        assert!(http_res.as_str().contains("\"injected_headers_count\":1"));
    }

    #[test]
    fn test_promises_webhooks_and_idempotency() {
        // Idempotency key generation
        let mut key_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();
        let key = unsafe {
            rocFxGenerateIdempotencyKey(key_out.as_mut_ptr());
            key_out.assume_init().into_result().expect("failed to generate idempotency key")
        };
        assert!(!key.as_str().is_empty());

        // Oplog index and commit
        let oplog_idx = unsafe { rocFxGetOplogIndex() };
        assert!(oplog_idx > 0);
        unsafe { rocFxOplogCommit(2) };

        // Create promise
        let mut promise_out = MaybeUninit::<RocResult<u64, RocStr>>::uninit();
        let promise_handle = unsafe {
            rocFxCreatePromise(promise_out.as_mut_ptr());
            promise_out.assume_init().into_result().expect("failed to create promise")
        };
        assert!(promise_handle > 0);

        // Webhook creation from promise
        let mut webhook_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();
        let webhook_url = unsafe {
            rocFxCreateWebhook(promise_handle, webhook_out.as_mut_ptr());
            webhook_out.assume_init().into_result().expect("failed to create webhook")
        };
        assert!(webhook_url.as_str().contains(&format!("{}", promise_handle)));

        // Get promise before completion should return err (not yet completed)
        let mut get_out = MaybeUninit::<RocResult<RocList<u8>, RocStr>>::uninit();
        unsafe {
            rocFxGetPromise(promise_handle, get_out.as_mut_ptr());
            assert!(get_out.assume_init().into_result().is_err());
        }

        // Complete promise with payload
        let payload = vec![0x42u8, 0x13, 0x37];
        let roc_payload = RocList::from_slice(&payload);
        let mut complete_out = MaybeUninit::<RocResult<bool, RocStr>>::uninit();
        let completed = unsafe {
            rocFxCompletePromise(promise_handle, &roc_payload, complete_out.as_mut_ptr());
            complete_out.assume_init().into_result().expect("failed to complete promise")
        };
        assert!(completed);

        // Get promise after completion should yield payload bytes
        let mut get_out_completed = MaybeUninit::<RocResult<RocList<u8>, RocStr>>::uninit();
        let retrieved = unsafe {
            rocFxGetPromise(promise_handle, get_out_completed.as_mut_ptr());
            get_out_completed.assume_init().into_result().expect("failed to get completed promise")
        };
        assert_eq!(retrieved.as_slice(), payload.as_slice());

        // Invalid promise handle errors
        let mut invalid_get = MaybeUninit::<RocResult<RocList<u8>, RocStr>>::uninit();
        unsafe {
            rocFxGetPromise(99999, invalid_get.as_mut_ptr());
            assert!(invalid_get.assume_init().into_result().is_err());
        }
        let mut invalid_webhook = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();
        unsafe {
            rocFxCreateWebhook(99999, invalid_webhook.as_mut_ptr());
            assert!(invalid_webhook.assume_init().into_result().is_err());
        }
    }

    #[test]
    fn test_websockets_and_oplog() {
        let ws_url = RocStr::from_str("wss://gateway.golem.cloud/ws/canary");
        let mut connect_out = MaybeUninit::<RocResult<u32, RocStr>>::uninit();
        let handle = unsafe {
            rocFxWsConnect(&ws_url, connect_out.as_mut_ptr());
            connect_out.assume_init().into_result().expect("ws connect failed")
        };
        assert!(handle > 0);

        // Send message
        let msg = RocStr::from_str("hello websocket");
        let mut send_out = MaybeUninit::<RocResult<(), RocStr>>::uninit();
        unsafe {
            rocFxWsSend(handle, &msg, send_out.as_mut_ptr());
            assert!(send_out.assume_init().into_result().is_ok());
        }

        // Receive message
        let mut recv_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();
        let received = unsafe {
            rocFxWsReceive(handle, recv_out.as_mut_ptr());
            recv_out.assume_init().into_result().expect("ws receive failed")
        };
        assert_eq!(received.as_str(), "hello websocket");

        // Close websocket
        let mut close_out = MaybeUninit::<RocResult<(), RocStr>>::uninit();
        unsafe {
            rocFxWsClose(handle, close_out.as_mut_ptr());
            assert!(close_out.assume_init().into_result().is_ok());
        }

        // Subsequent send/receive fails
        let mut fail_send = MaybeUninit::<RocResult<(), RocStr>>::uninit();
        unsafe {
            rocFxWsSend(handle, &msg, fail_send.as_mut_ptr());
            assert!(fail_send.assume_init().into_result().is_err());
        }
        let mut fail_recv = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();
        unsafe {
            rocFxWsReceive(handle, fail_recv.as_mut_ptr());
            assert!(fail_recv.assume_init().into_result().is_err());
        }
    }

    #[test]
    fn test_rpc_and_timer_effects() {
        let target = RocStr::from_str("worker-auth-01");
        let method = RocStr::from_str("validate_token");
        let payload = RocStr::from_str("{\"token\":\"xyz\"}");

        // RPC Invoke
        let mut invoke_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();
        let inv_res = unsafe {
            rocFxRpcInvoke(&target, &method, &payload, invoke_out.as_mut_ptr());
            invoke_out.assume_init().into_result().expect("rpc invoke failed")
        };
        assert!(inv_res.as_str().contains("rpc_invoked"));

        // RPC Schedule
        let mut sched_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();
        let sched_res = unsafe {
            rocFxRpcSchedule(&target, &method, &payload, 5000, sched_out.as_mut_ptr());
            sched_out.assume_init().into_result().expect("rpc schedule failed")
        };
        assert!(sched_res.as_str().contains("scheduled"));
        assert!(sched_res.as_str().contains("5000"));

        // RPC Fire and Forget
        let mut fnf_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();
        let fnf_res = unsafe {
            rocFxRpcFireAndForget(&target, &method, &payload, fnf_out.as_mut_ptr());
            fnf_out.assume_init().into_result().expect("rpc fire and forget failed")
        };
        assert!(fnf_res.as_str().contains("rpc_fired"));

        // Timer effects
        unsafe {
            rocFxSleepMillis(5);
            let now_val = rocFxNowMillis();
            assert!(now_val > 0);
        }

        // Logging
        let log_msg = RocStr::from_str("test log message");
        unsafe {
            for level in 0..5 {
                rocFxLog(level, &log_msg);
            }
        }
    }

    #[test]
    fn test_config_tracing_idempotence_and_rdbms() {
        // 1. Config store
        std::env::set_var("GOLEM_CONFIG_TEST_KEY", "config_test_val_99");
        let conf_key = RocStr::from_str("GOLEM_CONFIG_TEST_KEY");
        let mut conf_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();
        let conf_val = unsafe {
            rocFxConfigGet(&conf_key, conf_out.as_mut_ptr());
            conf_out.assume_init().into_result().expect("failed to get config key")
        };
        assert_eq!(conf_val.as_str(), "config_test_val_99");

        let missing_key = RocStr::from_str("MISSING_KEY_XYZ");
        let mut missing_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();
        unsafe {
            rocFxConfigGet(&missing_key, missing_out.as_mut_ptr());
            assert!(missing_out.assume_init().into_result().is_err());
        }

        // 2. Distributed Tracing Context
        let mut trace_out = MaybeUninit::<RocStr>::uninit();
        let trace = unsafe {
            rocFxTraceId(trace_out.as_mut_ptr());
            trace_out.assume_init()
        };
        assert!(!trace.as_str().is_empty());

        let mut span_out = MaybeUninit::<RocStr>::uninit();
        let span = unsafe {
            rocFxSpanId(span_out.as_mut_ptr());
            span_out.assume_init()
        };
        assert!(!span.as_str().is_empty());

        let mut headers_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();
        let headers_res = unsafe {
            rocFxTraceHeaders(headers_out.as_mut_ptr());
            headers_out.assume_init().into_result().expect("failed to get trace headers")
        };
        assert!(headers_res.as_str().contains("traceparent"));

        // 3. Idempotence Mode
        let mode = unsafe { rocFxGetIdempotenceMode() };
        assert!(mode);
        unsafe { rocFxSetIdempotenceMode(false) };

        // 4. Oplog Revert
        let mut revert_out = MaybeUninit::<RocResult<(), RocStr>>::uninit();
        unsafe {
            rocFxRevertAgent(42, revert_out.as_mut_ptr());
            assert!(revert_out.assume_init().into_result().is_ok());
        }

        // 5. Durable RDBMS (PostgreSQL & MySQL)
        let pg_conn = RocStr::from_str("postgresql://user:pass@localhost:5432/db");
        let sql_stmt = RocStr::from_str("INSERT INTO test (id) VALUES (1)");
        let mut exec_out = MaybeUninit::<RocResult<u64, RocStr>>::uninit();
        let count = unsafe {
            rocFxDbExecute(&pg_conn, &sql_stmt, exec_out.as_mut_ptr());
            exec_out.assume_init().into_result().expect("db execute failed")
        };
        assert_eq!(count, 1);

        let sql_query = RocStr::from_str("SELECT id, status FROM test");
        let mut query_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();
        let query_res = unsafe {
            rocFxDbQuery(&pg_conn, &sql_query, query_out.as_mut_ptr());
            query_out.assume_init().into_result().expect("db query failed")
        };
        assert!(query_res.as_str().contains("\"columns\""));
        assert!(query_res.as_str().contains("\"rowCount\""));
    }
}
