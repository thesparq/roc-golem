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

// Generate WIT bindings for the official Golem 2.0 agent world. The `async`
// list names the exports that use the component-model async ABI (the fork
// requires this opt-in); `get-definition` and `discover-agent-types` stay
// synchronous.
wit_bindgen::generate!({
    world: "golem-agent",
    path: "../wit",
    async: [
        "export:golem:agent/guest@2.0.0#initialize",
        "export:golem:agent/guest@2.0.0#invoke",
        "export:golem:tool/guest@0.1.0#invoke",
        "export:golem:api/save-snapshot@1.5.0#save",
        "export:golem:api/load-snapshot@1.5.0#load",
    ],
    generate_all,
});

use exports::golem::agent::guest::{AgentError, AgentType, Guest, Principal};
use golem::agent::common::{
    AgentConfigDeclaration, AgentConfigSource, AgentConstructor, AgentMethod, AgentMode,
    AgentTypeKind, CachePolicy, CorsOptions, FieldSource, HttpEndpointDetails, HttpMethod,
    InputSchema, NamedField, OutputSchema, PathSegment, ReadOnlyConfig, Snapshotting,
};
use golem::core::types::{
    MetadataEnvelope, SchemaGraph, SchemaTypeBody, SchemaTypeNode, SchemaValueNode,
    SchemaValueTree,
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

fn empty_metadata() -> MetadataEnvelope {
    MetadataEnvelope {
        doc: None,
        aliases: vec![],
        examples: vec![],
        deprecated: None,
        role: None,
    }
}

/// The agent's schema graph: a single string type node. Every declared
/// parameter and result is a string in the app-facing contract, so all schema
/// roots point at node 0.
fn string_schema_graph() -> SchemaGraph {
    SchemaGraph {
        type_nodes: vec![SchemaTypeNode {
            body: SchemaTypeBody::StringType,
            metadata: empty_metadata(),
        }],
        defs: vec![],
        root: 0,
    }
}

fn string_named_field(name: &str) -> NamedField {
    NamedField {
        name: name.to_string(),
        source: FieldSource::UserSupplied,
        schema: 0,
        metadata: empty_metadata(),
    }
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
        durable_streams: None,
    }
}

/// Extracts the single string parameter from an input value tree.
///
/// Inputs encode the method/constructor parameter list: a record (or tuple)
/// whose first element is the string the Roc app receives. A bare string root
/// is also accepted, and anything else degrades to "{}" the way the old
/// data-value extraction did.
fn extract_input_string(tree: &SchemaValueTree) -> String {
    match tree.value_nodes.get(tree.root as usize) {
        Some(SchemaValueNode::RecordValue(items)) | Some(SchemaValueNode::TupleValue(items)) => {
            match items.first().and_then(|i| tree.value_nodes.get(*i as usize)) {
                Some(SchemaValueNode::StringValue(s)) => s.clone(),
                _ => String::from("{}"),
            }
        }
        Some(SchemaValueNode::StringValue(s)) => s.clone(),
        _ => String::from("{}"),
    }
}

/// Wraps a single string as an output value tree (root = the string node).
fn single_string_value_tree(s: &str) -> SchemaValueTree {
    SchemaValueTree {
        value_nodes: vec![SchemaValueNode::StringValue(s.to_string())],
        root: 0,
    }
}

/// Wraps a single string as a *parameter list* value tree: the root is a record
/// with one field, which is what both `invoke` inputs and RPC calls expect.
fn single_param_value_tree(s: &str) -> SchemaValueTree {
    SchemaValueTree {
        value_nodes: vec![
            SchemaValueNode::RecordValue(vec![1]),
            SchemaValueNode::StringValue(s.to_string()),
        ],
        root: 0,
    }
}

fn build_agent_type() -> Result<AgentType, AgentError> {
    let mut roc_out = MaybeUninit::<RocStr>::uninit();
    let meta_json = unsafe {
        main_metadata_for_host_1_exposed_generic(roc_out.as_mut_ptr());
        roc_out.assume_init().to_string()
    };

    // The registered type comes from the app's own metadata, which the platform
    // builds from `agent.metadata` ({"name", "version", "description", "tools"}).
    // A broken component used to fall back to a generic "roc-agent" type here,
    // silently hiding the failure and, in a multi-component app, surfacing at
    // deploy time as a duplicated `roc-agent` type (see the README's development
    // status). Refuse loudly instead, so Golem reports the actual problem.
    // Use "seen, non-empty" flags rather than empty-string sentinels: with a
    // `let … = String::from("")` initializer in this function, the compiled
    // component deterministically traps on a later *guest* call (seen
    // repeatedly while building this change — a Roc/WIT codegen quirk, see the
    // README's development status). Missing or unparseable metadata must fail
    // loudly instead of silently registering a fallback "roc-agent" type
    // (which in a multi-component app surfaces as a duplicated `roc-agent`
    // type at deploy time).
    let mut name_str = String::from("roc-agent");
    let mut name_seen = false;
    let mut description_str = String::from("Durable Roc Agent on Golem Cloud");
    let mut description_seen = false;
    let mut methods = Vec::new();

    methods.push(AgentMethod {
        name: "handle-message".to_string(),
        description: "Handles text or JSON messages".to_string(),
        http_endpoint: vec![post_endpoint("/api/chat")],
        prompt_hint: None,
        input_schema: InputSchema::Parameters(vec![string_named_field("message")]),
        output_schema: OutputSchema::Single(0),
        read_only: None,
    });

    let mut config_decls = Vec::new();

    if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&meta_json) {
        if let Some(n) = meta.get("name").and_then(|v| v.as_str()) {
            if !n.is_empty() {
                name_seen = true;
                name_str = n.to_string();
            }
        }
        if let Some(d) = meta.get("description").and_then(|v| v.as_str()) {
            if !d.is_empty() {
                description_seen = true;
                description_str = d.to_string();
            }
        }
        if let Some(config) = meta.get("config").and_then(|v| v.as_array()) {
            for c in config {
                let c_name = c
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if !c_name.is_empty() {
                    let secret = c
                        .get("secret")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    config_decls.push(AgentConfigDeclaration {
                        source: if secret {
                            AgentConfigSource::Secret
                        } else {
                            AgentConfigSource::Local
                        },
                        path: vec![c_name],
                        value_type: 0,
                    });
                }
            }
        }
        if let Some(tools) = meta.get("tools").and_then(|v| v.as_array()) {
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
                    input_schema: InputSchema::Parameters(vec![string_named_field("input")]),
                    output_schema: OutputSchema::Single(0),
                    read_only: t
                        .get("readOnly")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                        .then(|| ReadOnlyConfig {
                            cache_policy: CachePolicy::UntilWrite,
                            uses_principal: false,
                        }),
                });
            }
        }
    }

    // Build the result without early returns: errors and success are assembled
    // in one `else if` chain at the end, because early returns from this
    // function have tripped a compiler codegen bug in the past.
    let agent_type: Result<AgentType, AgentError> = if !name_seen {
        Err(AgentError::InvalidType(
            "app metadata is not valid JSON or has no non-empty \"name\" field".to_string(),
        ))
    } else if !description_seen {
        Err(AgentError::InvalidType(
            "app metadata is not valid JSON or has no non-empty \"description\" field"
                .to_string(),
        ))
    } else {
        Ok(AgentType {
            type_name: name_str,
            kind: AgentTypeKind::Regular,
            description: description_str,
            source_language: "roc".to_string(),
            schema: string_schema_graph(),
            constructor: AgentConstructor {
                name: None,
                description: "Initializes the agent".to_string(),
                prompt_hint: None,
                input_schema: InputSchema::Parameters(vec![string_named_field("config")]),
            },
            methods,
            dependencies: vec![],
            mode: AgentMode::Durable,
            http_mount: None,
            snapshotting: Snapshotting::Disabled,
            config: config_decls,
        })
    };
    agent_type
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
    let worker_id = match golem::api::host::get_self_metadata() {
        Ok(metadata) => metadata.agent_id.agent_id,
        Err(_) => String::from("unknown"),
    };
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
    // (for example `counter-agent("{}")`). `parse-agent-id` returns the target's
    // constructor parameters as a self-contained typed value, which the 2.0
    // `wasm-rpc` resource accepts directly.
    let result = match golem::agent::host::parse_agent_id(target_str) {
        Ok((agent_type_name, constructor, phantom_id)) => {
            let rpc = golem::agent::host::WasmRpc::new(
                &agent_type_name,
                constructor.value,
                phantom_id,
                Vec::new(),
            );
            let input = single_param_value_tree(payload_str);
            match rpc.invoke_and_await(method, input, None) {
                Ok(invocation) => match invocation.result {
                    Some(tree) => Ok(extract_input_string(&tree)),
                    None => Ok(String::from("")),
                },
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
    // Golem 1.6 removed persistence levels: durable agents are durable all the
    // way through. Kept as a no-op so pre-1.6 platform code still links.
    let _ = mode;
}

fn parse_url(url: &str) -> Result<(wasi::http0_2_6::types::Scheme, String, String), String> {
    let (scheme, rest) = if let Some(stripped) = url.strip_prefix("https://") {
        (wasi::http0_2_6::types::Scheme::Https, stripped)
    } else if let Some(stripped) = url.strip_prefix("http://") {
        (wasi::http0_2_6::types::Scheme::Http, stripped)
    } else if let Some(idx) = url.find("://") {
        let (s, r) = url.split_at(idx);
        (wasi::http0_2_6::types::Scheme::Other(s.to_string()), &r[3..])
    } else {
        (wasi::http0_2_6::types::Scheme::Https, url)
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

fn parse_method(m: &str) -> wasi::http0_2_6::types::Method {
    match m.to_uppercase().as_str() {
        "GET" => wasi::http0_2_6::types::Method::Get,
        "POST" => wasi::http0_2_6::types::Method::Post,
        "PUT" => wasi::http0_2_6::types::Method::Put,
        "DELETE" => wasi::http0_2_6::types::Method::Delete,
        "PATCH" => wasi::http0_2_6::types::Method::Patch,
        "HEAD" => wasi::http0_2_6::types::Method::Head,
        "OPTIONS" => wasi::http0_2_6::types::Method::Options,
        "CONNECT" => wasi::http0_2_6::types::Method::Connect,
        "TRACE" => wasi::http0_2_6::types::Method::Trace,
        other => wasi::http0_2_6::types::Method::Other(other.to_string()),
    }
}

pub fn execute_http_request(req_str: &str) -> Result<String, String> {
    use wasi::http0_2_6::outgoing_handler;
    use wasi::http0_2_6::types::{Fields, IncomingBody, OutgoingBody, OutgoingRequest};
    use wasi::io::streams::StreamError;

    let mut method = wasi::http0_2_6::types::Method::Get;
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
    // `receive` became an async host import in Golem 1.6. The Roc side calls
    // this effect synchronously, so the fork's `block_on` drives the async call
    // to completion (it waits on the guest's waitable set while the host call is
    // in flight).
    let received = ws_with(handle, |connection| wit_bindgen::block_on(connection.receive()));
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
    let pollable = wasi::clocks0_2_6::monotonic_clock::subscribe_duration(nanos);
    pollable.block();
    drop(pollable);
}

#[no_mangle]
pub unsafe extern "C" fn rocFxNowMillis() -> u64 {
    wasi::clocks0_2_6::monotonic_clock::now() / 1_000_000
}

#[no_mangle]
pub unsafe extern "C" fn rocFxGetConfig(out: *mut RocResult<RocStr, RocStr>, key: *const RocStr) {
    let key_str = if key.is_null() { "" } else { (*key).as_str() };
    // Only keys the agent type declares can be read; the host returns an error
    // otherwise (or `none` for an option-typed key, which this platform does not
    // declare yet).
    let result = match golem::agent::host::get_config_value(
        &[key_str.to_string()],
        &string_schema_graph(),
    ) {
        Ok(tree) => Ok(extract_input_string(&tree)),
        Err(err) => Err(format!("config {key_str:?} unavailable: {err:?}")),
    };

    match result {
        Ok(text) => ptr::write(out, RocResult::ok(RocStr::from_str(&text))),
        Err(err) => ptr::write(out, RocResult::err(RocStr::from_str(&err))),
    }
}

struct GolemAgentHost;

impl Guest for GolemAgentHost {
    async fn initialize(
        _agent_type: String,
        input: SchemaValueTree,
        _principal: Principal,
    ) -> Result<(), AgentError> {
        let config = extract_input_string(&input);
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

    async fn invoke(
        method_name: String,
        input: SchemaValueTree,
        _principal: Principal,
    ) -> Result<Option<SchemaValueTree>, AgentError> {
        let state_str = get_current_state();
        let input_str = extract_input_string(&input);

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
                                return Ok(Some(single_string_value_tree(&resp_text)));
                            }
                        }
                        Ok(Some(single_string_value_tree(payload_str)))
                    }
                    Err(err) => Err(AgentError::InvalidMethod(err.to_string())),
                }
            }
        } else {
            // Tool invocation through the agent's own method surface.
            match run_tool_call(&method_name, &input_str) {
                Ok(output) => Ok(Some(single_string_value_tree(&output))),
                Err(err) => Err(err),
            }
        }
    }

    fn get_definition() -> AgentType {
        match build_agent_type() {
            Ok(agent_type) => agent_type,
            Err(err) => panic!("Refusing to register an agent type: {:?}", err),
        }
    }

    fn discover_agent_types() -> Result<Vec<AgentType>, AgentError> {
        match build_agent_type() {
            Ok(agent_type) => Ok(vec![agent_type]),
            Err(err) => Err(err),
        }
    }
}

export!(GolemAgentHost);

/// Runs the app's `handleToolCall!` hook for one tool invocation and returns
/// the tool's output string, updating the persisted state as a side effect.
/// Shared by the agent method surface and the `golem:tool/guest` surface.
fn run_tool_call(tool_name: &str, args: &str) -> Result<String, AgentError> {
    let state_str = get_current_state();
    let tool_call_json = serde_json::json!({
        "id": "call-1",
        "name": tool_name,
        "arguments": args,
    })
    .to_string();

    let roc_state = RocStr::from_str(&state_str);
    let roc_call = RocStr::from_str(&tool_call_json);
    let mut roc_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();

    unsafe {
        main_handle_tool_call_for_host_1_exposed_generic(roc_out.as_mut_ptr(), &roc_state, &roc_call);
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
                    if let Some(out_val) = parsed.get("output") {
                        if let Some(s) = out_val.as_str() {
                            Ok(s.into())
                        } else {
                            Ok(out_val.to_string())
                        }
                    } else {
                        Ok(String::from("{}"))
                    }
                } else {
                    Ok(payload_str.into())
                }
            }
            Err(err) => Err(AgentError::InvalidMethod(err.to_string())),
        }
    }
}

/// The app's declared tools, exposed through `golem:tool/guest` so other agents
/// and tools can discover and invoke them by name.
impl exports::golem::tool::guest::Guest for GolemAgentHost {
    fn discover_tools()
    -> Result<Vec<golem::tool::common::Tool>, exports::golem::tool::guest::ToolError> {
        Ok(build_tools().into_iter().map(|(_, tool)| tool).collect())
    }

    fn get_tool(
        name: String,
    ) -> Result<golem::tool::common::Tool, exports::golem::tool::guest::ToolError> {
        match build_tools()
            .into_iter()
            .find(|(_, tool)| {
                tool.commands.nodes.first().map(|node| node.name.clone()) == Some(name.clone())
            })
        {
            Some((_, tool)) => Ok(tool),
            None => Err(exports::golem::tool::guest::ToolError::InvalidToolName(name)),
        }
    }

    async fn invoke(
        tool_name: String,
        _command_path: Vec<String>,
        input: golem::core::types::TypedSchemaValue,
        _stdin: Option<
            wit_bindgen::StreamReader<Result<Vec<u8>, golem::tool::streams::ByteStreamFailure>>,
        >,
        _stdout: Option<golem::tool::streams::ToolOutputWriter>,
        _stderr: Option<golem::tool::streams::ToolOutputWriter>,
        _principal: Principal,
    ) -> Result<golem::tool::common::InvocationResult, exports::golem::tool::guest::ToolError> {
        let args = extract_input_string(&input.value);
        let app_name = build_tools()
            .into_iter()
            .find(|(_, tool)| {
                tool.commands.nodes.first().map(|node| node.name.clone()) == Some(tool_name.clone())
            })
            .map(|(app_name, _)| app_name)
            .ok_or_else(|| {
                exports::golem::tool::guest::ToolError::InvalidToolName(tool_name.clone())
            })?;
        match run_tool_call(&app_name, &args) {
            Ok(output) => Ok(golem::tool::common::InvocationResult {
                result: Some(golem::core::types::TypedSchemaValue {
                    graph: string_schema_graph(),
                    value: single_string_value_tree(&output),
                }),
                stdout: None,
                stderr: None,
            }),
            Err(err) => Err(exports::golem::tool::guest::ToolError::InvalidInput(format!(
                "{err:?}"
            ))),
        }
    }
}

/// Tool command names are CLI-style: lowercase, digits and dashes only.
fn tool_command_name(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push('-');
        }
    }
    out
}

/// The app's tools as single-command tools with one optional string input and a
/// string result — the shape the platform declares everywhere else. Each entry
/// keeps the app's own tool name alongside the CLI-shaped tool.
fn build_tools() -> Vec<(String, golem::tool::common::Tool)> {
    use golem::tool::common::{
        CommandBody, CommandNode, CommandTree, Doc, Formatter, Globals, Positional, Positionals,
        ResultSpec, Tool,
    };

    let meta_json = read_metadata_json();
    let mut tools = Vec::new();

    if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&meta_json) {
        if let Some(list) = meta.get("tools").and_then(|v| v.as_array()) {
            for t in list {
                let name = t
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("tool")
                    .to_string();
                let description = t
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let doc = Doc {
                    summary: description,
                    description: String::new(),
                    examples: Vec::new(),
                };
                let body = CommandBody {
                    positionals: Positionals {
                        fixed: vec![Positional {
                            name: String::from("input"),
                            doc: doc.clone(),
                            value_name: None,
                            type_: 0,
                            default: Some(single_string_value_tree("{}")),
                            required: false,
                            accepts_stdio: false,
                        }],
                        tail: None,
                    },
                    options: Vec::new(),
                    flags: Vec::new(),
                    constraints: Vec::new(),
                    stdin: None,
                    stdout: None,
                    stderr: None,
                    result: Some(ResultSpec {
                        type_: 0,
                        doc: doc.clone(),
                        formatters: vec![Formatter {
                            name: String::from("json"),
                            doc: doc.clone(),
                        }],
                        default_formatter: String::from("json"),
                    }),
                    errors: Vec::new(),
                    annotations: None,
                };
                tools.push((
                    name.clone(),
                    Tool {
                        version: String::from("1.0.0"),
                        requires_filesystem: false,
                        commands: CommandTree {
                            nodes: vec![CommandNode {
                                name: tool_command_name(&name),
                                aliases: Vec::new(),
                                doc,
                                globals: Globals {
                                    options: Vec::new(),
                                    flags: Vec::new(),
                                },
                                subcommands: Vec::new(),
                                body: Some(body),
                            }],
                        },
                        schema: string_schema_graph(),
                    },
                ));
            }
        }
    }

    tools
}

/// Reads the app metadata JSON (the same source `build_agent_type` uses).
fn read_metadata_json() -> String {
    let mut roc_out = MaybeUninit::<RocStr>::uninit();
    unsafe {
        main_metadata_for_host_1_exposed_generic(roc_out.as_mut_ptr());
        roc_out.assume_init().to_string()
    }
}

// User-defined snapshots: the agent's state JSON is the payload, so
// `snapshot-based` updates work for Roc agents without app changes.
impl exports::golem::api::save_snapshot::Guest for GolemAgentHost {
    async fn save() -> exports::golem::api::save_snapshot::Snapshot {
        exports::golem::api::save_snapshot::Snapshot {
            payload: get_current_state().into_bytes(),
            mime_type: String::from("application/json"),
        }
    }
}

impl exports::golem::api::load_snapshot::Guest for GolemAgentHost {
    async fn load(snapshot: exports::golem::api::load_snapshot::Snapshot) -> Result<(), String> {
        match String::from_utf8(snapshot.payload) {
            Ok(state) => {
                set_current_state(state);
                Ok(())
            }
            Err(err) => Err(format!("snapshot payload is not UTF-8: {err}")),
        }
    }
}

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
            InputSchema::Parameters(fields) => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, "message");
            }
            _ => panic!("Expected parameters input schema"),
        }

        match &types[0].constructor.input_schema {
            InputSchema::Parameters(fields) => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, "config");
            }
            _ => panic!("Expected parameters constructor input schema"),
        }

        assert!(types[0].methods.iter().any(|m| m.name == "calculator"));
        assert!(types[0].methods.iter().any(|m| m.name == "get_count"));

        // The `readOnly` metadata flag maps to the method's read-only config.
        let calculator = types[0]
            .methods
            .iter()
            .find(|m| m.name == "calculator")
            .expect("calculator method not found");
        assert!(matches!(
            calculator.read_only,
            Some(ReadOnlyConfig {
                cache_policy: CachePolicy::UntilWrite,
                uses_principal: false,
            })
        ));
        let echo = types[0]
            .methods
            .iter()
            .find(|m| m.name == "echo")
            .expect("echo method not found");
        assert!(echo.read_only.is_none());

        // The `config` metadata list maps to the agent type's config
        // declarations (local keys by default).
        assert_eq!(types[0].config.len(), 1);
        assert_eq!(types[0].config[0].path, vec!["greeting".to_string()]);
        assert!(matches!(
            types[0].config[0].source,
            AgentConfigSource::Local
        ));
    }

    #[test]
    fn test_agent_initialize_and_invoke() {
        let init_res = pollster::block_on(GolemAgentHost::initialize(
            "counter-agent".to_string(),
            single_string_value_tree("{}"),
            Principal::Anonymous,
        ));
        assert!(init_res.is_ok());

        let invoke_res = pollster::block_on(GolemAgentHost::invoke(
            "handle-message".to_string(),
            single_string_value_tree("increment"),
            Principal::Anonymous,
        ));
        assert!(invoke_res.is_ok());
    }

    #[test]
    fn test_value_tree_codec() {
        // Parameter records round-trip through the input extractor.
        let tree = single_param_value_tree("hello");
        assert_eq!(extract_input_string(&tree), "hello");

        // A bare string root is accepted too (outputs use this shape).
        let bare = single_string_value_tree("world");
        assert_eq!(extract_input_string(&bare), "world");

        // Outputs are bare string nodes at the root.
        let out = single_string_value_tree("Counter incremented to 1");
        assert!(matches!(
            out.value_nodes.get(out.root as usize),
            Some(SchemaValueNode::StringValue(s)) if s == "Counter incremented to 1"
        ));

        // Anything else degrades to the empty JSON object, like the old
        // data-value extraction did.
        let empty = SchemaValueTree {
            value_nodes: vec![],
            root: 0,
        };
        assert_eq!(extract_input_string(&empty), "{}");
    }

    #[test]
    fn test_parse_url() {
        let (scheme, auth, path) = parse_url("https://api.example.com/v1/chat?q=test").unwrap();
        assert!(matches!(scheme, wasi::http0_2_6::types::Scheme::Https));
        assert_eq!(auth, "api.example.com");
        assert_eq!(path, "/v1/chat?q=test");

        let (scheme, auth, path) = parse_url("http://localhost:8080").unwrap();
        assert!(matches!(scheme, wasi::http0_2_6::types::Scheme::Http));
        assert_eq!(auth, "localhost:8080");
        assert_eq!(path, "/");

        let (scheme, auth, path) = parse_url("custom://my-host/path").unwrap();
        assert!(matches!(scheme, wasi::http0_2_6::types::Scheme::Other(s) if s == "custom"));
        assert_eq!(auth, "my-host");
        assert_eq!(path, "/path");
    }

    #[test]
    fn test_parse_method() {
        assert!(matches!(parse_method("get"), wasi::http0_2_6::types::Method::Get));
        assert!(matches!(parse_method("POST"), wasi::http0_2_6::types::Method::Post));
        assert!(matches!(parse_method("put"), wasi::http0_2_6::types::Method::Put));
        assert!(matches!(parse_method("DELETE"), wasi::http0_2_6::types::Method::Delete));
        assert!(matches!(parse_method("PATCH"), wasi::http0_2_6::types::Method::Patch));
        assert!(matches!(parse_method("custom"), wasi::http0_2_6::types::Method::Other(s) if s == "CUSTOM"));
    }
}
