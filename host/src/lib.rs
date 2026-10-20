#[cfg(any(feature = "stub-guest", test))]
pub mod guest_bridge;
pub mod roc_std;

use core::cell::RefCell;
use core::mem::MaybeUninit;
use core::ptr;
use std::boxed::Box;
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
    pub fn main_handle_request_for_host_1_exposed_generic(
        out: *mut RocResult<RocStr, RocStr>,
        state: *const RocStr,
        request: *const RocStr,
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
    AgentTypeKind, CachePolicy, CorsOptions, ExactFileMapping, FieldSource, FileMapping,
    FileResponseHeader, HttpEndpointDetails, HttpMethod, HttpMountDetails, InputSchema, NamedField,
    OutputSchema, PathSegment, PathVariable, ReadOnlyConfig, Snapshotting, SubtreeFileMapping,
};
use golem::core::types::{
    MetadataEnvelope, NamedFieldType, SchemaGraph, SchemaTypeBody, SchemaTypeNode, SchemaValueNode,
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

fn type_node(body: SchemaTypeBody) -> SchemaTypeNode {
    SchemaTypeNode {
        body,
        metadata: empty_metadata(),
    }
}

/// The endpoint a router's `handle` method must declare: Golem accepts exactly
/// one handler endpoint, on any method, with no path suffix and no per-parameter
/// policy.
fn any_endpoint() -> HttpEndpointDetails {
    HttpEndpointDetails {
        http_method: HttpMethod::Any,
        path_suffix: vec![],
        header_vars: vec![],
        query_vars: vec![],
        auth_details: None,
        cors_options: CorsOptions {
            allowed_patterns: vec![],
        },
        durable_streams: None,
    }
}

/// The schema graph a router's `handle` method declares. Golem requires the
/// request and response to be exactly this shape (`request_schema` /
/// `response_schema` in its own `schema/agent/http` module), so the graph is
/// built to match: header lists, an optional query, and `stream<list<u8>>`
/// bodies.
struct RouterSchema {
    graph: SchemaGraph,
    request: i32,
    response: i32,
}

fn router_schema_graph() -> RouterSchema {
    let string = 0i32;
    let u8_index = 1i32;
    let bytes = 2i32; // list<u8>
    let header = 3i32; // record { name, value }
    let headers = 4i32; // list<header>
    let query = 5i32; // option<string>
    let body = 6i32; // stream<list<u8>>
    let request = 7i32;
    let status = 8i32; // u16
    let response = 9i32;

    let named = |name: &str, schema: i32| NamedFieldType {
        name: name.to_string(),
        body: schema,
        metadata: empty_metadata(),
    };

    let graph = SchemaGraph {
        type_nodes: vec![
            type_node(SchemaTypeBody::StringType),
            type_node(SchemaTypeBody::U8Type(None)),
            type_node(SchemaTypeBody::ListType(u8_index)),
            type_node(SchemaTypeBody::RecordType(vec![
                named("name", string),
                named("value", bytes),
            ])),
            type_node(SchemaTypeBody::ListType(header)),
            type_node(SchemaTypeBody::OptionType(string)),
            type_node(SchemaTypeBody::StreamType(Some(bytes))),
            type_node(SchemaTypeBody::RecordType(vec![
                named("method", string),
                named("scheme", string),
                named("authority", string),
                named("path", string),
                named("query", query),
                named("headers", headers),
                named("body", body),
            ])),
            type_node(SchemaTypeBody::U16Type(None)),
            type_node(SchemaTypeBody::RecordType(vec![
                named("status", status),
                named("headers", headers),
                named("body", body),
            ])),
        ],
        defs: vec![],
        root: request,
    };

    RouterSchema {
        graph,
        request,
        response,
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

/// Parses the app's `httpMount` metadata into the registered agent type's mount
/// details. Golem binds a regular agent's constructor parameter through the
/// mount, so such a mount captures exactly one path variable — the agent's
/// identity parameter — while a router is parameterless and its mount must be
/// literal-only.
fn parse_http_mount(
    mount: &serde_json::Value,
    is_router: bool,
) -> Result<HttpMountDetails, String> {
    let segments = mount
        .get("pathPrefix")
        .and_then(|v| v.as_array())
        .ok_or("httpMount.pathPrefix must be a list of path segments")?;

    let mut path_prefix = Vec::new();
    let mut captures = Vec::new();
    for segment in segments {
        let segment = segment
            .as_str()
            .ok_or("httpMount.pathPrefix must contain only strings")?;
        match parse_mount_segment(segment)? {
            PathSegment::PathVariable(variable) => {
                captures.push(variable.variable_name.clone());
                path_prefix.push(PathSegment::PathVariable(variable));
            }
            literal => path_prefix.push(literal),
        }
    }

    // This platform's agents take exactly one constructor parameter, and Golem
    // requires every constructor parameter to be captured by the mount, so a
    // mount with any other number of captures cannot be deployed. Routers have
    // no constructor at all and must not capture anything.
    if is_router {
        if let Some(name) = captures.first() {
            return Err(format!(
                "an http-router mount cannot capture path variables (found {{{name}}}): routers \
                 have no constructor parameters"
            ));
        }
    } else {
        match captures.as_slice() {
            [_] => {}
            [] => {
                return Err(
                    "httpMount.pathPrefix must capture the agent's constructor parameter, \
                     for example [\"site\", \"{id}\"]"
                        .to_string(),
                );
            }
            _ => {
                return Err(format!(
                    "httpMount.pathPrefix captures {} variables ({}), but a Roc agent has a \
                     single constructor parameter",
                    captures.len(),
                    captures.join(", ")
                ));
            }
        }
    }

    let filesystem_bindings = parse_file_bindings(mount, "filesystemBindings")?;
    let file_response_headers = parse_file_response_headers(mount)?;
    let cors_allowed_patterns = string_list(mount, "corsAllowedPatterns")?;

    let details = HttpMountDetails {
        path_prefix,
        auth_details: None,
        phantom_agent: false,
        phantom_id_binding: None,
        cors_options: CorsOptions {
            allowed_patterns: cors_allowed_patterns,
        },
        webhook_suffix: vec![],
        // Static bindings are served from files uploaded at deploy time and only
        // exist on `http-router` agent types; filesystem bindings are served
        // from the agent's own durable filesystem on regular durable agents.
        static_bindings: parse_file_bindings(mount, "staticBindings")?,
        filesystem_bindings,
        file_response_headers,
        openapi_provider_method: None,
    };
    Ok(details)
}

/// One mount path segment: `{name}` captures the constructor parameter, anything
/// else is a URL literal.
fn parse_mount_segment(segment: &str) -> Result<PathSegment, String> {
    let is_capture = segment.starts_with('{') || segment.ends_with('}');
    if is_capture {
        let name = segment
            .strip_prefix('{')
            .and_then(|rest| rest.strip_suffix('}'))
            .ok_or_else(|| {
                format!("httpMount path segment {segment:?} is not a valid {{name}} capture")
            })?;
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(format!(
                "httpMount capture {segment:?} must be a plain name (letters, digits, '-' or '_')"
            ));
        }
        Ok(PathSegment::PathVariable(PathVariable {
            variable_name: name.to_string(),
        }))
    } else if valid_path_segment(segment) {
        Ok(PathSegment::Literal(segment.to_string()))
    } else {
        Err(format!(
            "httpMount path segment {segment:?} is not a valid URL path segment"
        ))
    }
}

/// Mirrors the server's `valid_decoded_segment`: decoded, non-empty, no `.`/`..`,
/// no separators or control characters.
fn valid_path_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && !segment
            .bytes()
            .any(|byte| byte <= 0x1f || byte == 0x7f || byte == b'/' || byte == b'\\')
}

/// Mirrors the server's `validate_file_mapping` target rules so a bad mapping
/// fails at build time instead of as a deploy-time validation error.
fn valid_file_target(target: &str, allow_root: bool) -> bool {
    target.starts_with('/')
        && !target.contains('$')
        && !target.chars().any(char::is_control)
        && (target != "/" || allow_root)
        && (target == "/"
            || target[1..].split('/').all(valid_path_segment))
}

fn string_list(value: &serde_json::Value, key: &str) -> Result<Vec<String>, String> {
    match value.get(key) {
        None | Some(serde_json::Value::Null) => Ok(vec![]),
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(|s| s.to_string())
                    .ok_or_else(|| format!("httpMount.{key} must contain only strings"))
            })
            .collect(),
        Some(_) => Err(format!("httpMount.{key} must be a list")),
    }
}

fn parse_file_bindings(
    mount: &serde_json::Value,
    key: &str,
) -> Result<Vec<FileMapping>, String> {
    let mut bindings = Vec::new();
    for binding in string_list_of_objects(mount, key)? {
        if let Some(exact) = binding.get("exact").and_then(|v| v.as_object()) {
            bindings.push(FileMapping::Exact(ExactFileMapping {
                public_path: segments_from(exact, key, "publicPath")?,
                file_path: target_from(exact, key, "filePath", false)?,
            }));
        } else if let Some(subtree) = binding.get("subtree").and_then(|v| v.as_object()) {
            bindings.push(FileMapping::Subtree(SubtreeFileMapping {
                public_prefix: segments_from(subtree, key, "publicPrefix")?,
                filesystem_root: target_from(subtree, key, "filesystemRoot", true)?,
            }));
        } else {
            return Err(format!(
                "httpMount.{key} entries must be an `Exact` or a `Subtree` binding"
            ));
        }
    }
    Ok(bindings)
}

fn string_list_of_objects<'a>(
    value: &'a serde_json::Value,
    key: &str,
) -> Result<Vec<&'a serde_json::Map<String, serde_json::Value>>, String> {
    match value.get(key) {
        None | Some(serde_json::Value::Null) => Ok(vec![]),
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_object()
                    .ok_or_else(|| format!("httpMount.{key} must contain only objects"))
            })
            .collect(),
        Some(_) => Err(format!("httpMount.{key} must be a list")),
    }
}

fn segments_from(
    binding: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    field: &str,
) -> Result<Vec<String>, String> {
    let list = string_list(&serde_json::Value::Object(binding.clone()), field)?;
    if list.is_empty() {
        return Err(format!("httpMount.{key}.{field} must not be empty"));
    }
    if let Some(bad) = list.iter().find(|s| !valid_path_segment(s)) {
        return Err(format!(
            "httpMount.{key}.{field} contains {bad:?}, which is not a valid path segment"
        ));
    }
    Ok(list)
}

fn target_from(
    binding: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    field: &str,
    allow_root: bool,
) -> Result<String, String> {
    let value = binding
        .get(field)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("httpMount.{key}.{field} must be a string"))?;
    if !valid_file_target(value, allow_root) {
        return Err(format!(
            "httpMount.{key}.{field} must be an absolute filesystem path inside the agent's \
             filesystem (got {value:?})"
        ));
    }
    Ok(value.to_string())
}

fn parse_file_response_headers(mount: &serde_json::Value) -> Result<Vec<FileResponseHeader>, String> {
    let mut headers = Vec::new();
    for header in string_list_of_objects(mount, "fileResponseHeaders")? {
        let name = header
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("httpMount.fileResponseHeaders entries need a `name`")?;
        let value = header
            .get("value")
            .and_then(|v| v.as_str())
            .ok_or("httpMount.fileResponseHeaders entries need a `value`")?;
        headers.push(FileResponseHeader {
            name: name.to_string(),
            value: value.to_string(),
        });
    }
    Ok(headers)
}

/// The name of the agent's constructor (identity) parameter. A mount binds it
/// through its path capture, so the capture's name is the parameter's public
/// name; without a mount the parameter keeps the platform's default name.
fn constructor_param_name(mount: &Option<HttpMountDetails>) -> String {
    mount
        .as_ref()
        .and_then(|mount| {
            mount.path_prefix.iter().find_map(|segment| match segment {
                PathSegment::PathVariable(variable) => Some(variable.variable_name.clone()),
                _ => None,
            })
        })
        .unwrap_or_else(|| "config".to_string())
}

/// One request routed to a router's mount, decoded from the value tree Golem
/// passes as the handler's `request` argument.
struct RouterRequest {
    method: String,
    scheme: String,
    authority: String,
    path: String,
    query: Option<String>,
    headers: Vec<(String, String)>,
    /// The body arrives as a `stream<list<u8>>`; the caller drains it.
    body: SchemaValueNode,
}

impl RouterRequest {
    /// Reads the handler's `request` argument out of the value tree, taking
    /// ownership of the body stream handle (a resource, so it cannot be cloned).
    fn from_value_tree(tree: &mut SchemaValueTree) -> Result<Self, AgentError> {
        // `invoke` inputs are a parameter record whose single field is the
        // request record.
        let request = match tree.value_nodes.get(tree.root as usize) {
            Some(SchemaValueNode::RecordValue(fields)) => fields.first().copied(),
            _ => None,
        }
        .and_then(|index| tree.value_nodes.get(index as usize))
        .ok_or_else(|| AgentError::InvalidInput("missing request argument".to_string()))?;
        let fields = match request {
            SchemaValueNode::RecordValue(fields) => fields.clone(),
            _ => {
                return Err(AgentError::InvalidInput(
                    "the request argument is not a record".to_string(),
                ))
            }
        };
        let node = |index: usize| fields.get(index).and_then(|i| tree.value_nodes.get(*i as usize));
        let text = |index: usize| match node(index) {
            Some(SchemaValueNode::StringValue(value)) => Ok(value.clone()),
            _ => Err(AgentError::InvalidInput(format!(
                "request field {} is missing or not a string",
                index
            ))),
        };

        let query = match node(4) {
            Some(SchemaValueNode::OptionValue(Some(index))) => match tree
                .value_nodes
                .get(*index as usize)
            {
                Some(SchemaValueNode::StringValue(value)) => Some(value.clone()),
                _ => None,
            },
            _ => None,
        };

        let mut headers = Vec::new();
        if let Some(SchemaValueNode::ListValue(entries)) = node(5) {
            for entry in entries {
                if let Some(SchemaValueNode::RecordValue(parts)) = tree
                    .value_nodes
                    .get(*entry as usize)
                {
                    let name = parts.first().and_then(|i| tree.value_nodes.get(*i as usize));
                    let value = parts.get(1).and_then(|i| tree.value_nodes.get(*i as usize));
                    if let (Some(SchemaValueNode::StringValue(name)), Some(value)) = (name, value) {
                        headers.push((name.clone(), bytes_of_tree(tree, value)));
                    }
                }
            }
        }

        let method = text(0)?;
        let scheme = text(1)?;
        let authority = text(2)?;
        let path = text(3)?;
        let body_index = fields
            .get(6)
            .copied()
            .ok_or_else(|| AgentError::InvalidInput("request body is missing".to_string()))?;
        let body = std::mem::replace(
            tree.value_nodes.get_mut(body_index as usize).ok_or_else(|| {
                AgentError::InvalidInput("request body is missing".to_string())
            })?,
            SchemaValueNode::OptionValue(None),
        );

        Ok(RouterRequest {
            method,
            scheme,
            authority,
            path,
            query,
            headers,
            body,
        })
    }
}

/// The bytes of a `list<u8>` value.
fn bytes_of_tree(tree: &SchemaValueTree, value: &SchemaValueNode) -> String {
    let mut bytes = Vec::new();
    if let SchemaValueNode::ListValue(items) = value {
        for item in items {
            if let Some(SchemaValueNode::U8Value(byte)) = tree.value_nodes.get(*item as usize) {
                bytes.push(*byte);
            }
        }
    }
    String::from_utf8_lossy(&bytes).to_string()
}

/// Drains the request's body stream. The stream carries `list<u8>` chunks.
async fn read_request_body(body: SchemaValueNode) -> Result<String, AgentError> {
    let SchemaValueNode::StreamValue(handle) = body else {
        return Err(AgentError::InvalidInput(
            "request body is not a stream".to_string(),
        ));
    };
    let mut reader = golem::core::types::SchemaValueStream::unwrap(handle).await;
    let chunks = reader.collect().await;
    let mut text = String::new();
    for chunk in chunks {
        if let Some(value) = chunk.value_nodes.get(chunk.root as usize) {
            text.push_str(&bytes_of_tree(&chunk, value));
        }
    }
    Ok(text)
}

/// Serves one request routed to a router's mount: hands the request to the app
/// and returns its response, with the body wrapped in the `stream<list<u8>>`
/// the schema declares.
async fn handle_router_request(
    input: &mut SchemaValueTree,
) -> Result<Option<SchemaValueTree>, AgentError> {
    let request = RouterRequest::from_value_tree(input)?;
    let body = read_request_body(request.body).await?;
    let headers_json = serde_json::Value::Array(
        request
            .headers
            .iter()
            .map(|(name, value)| serde_json::json!({ "name": name, "value": value }))
            .collect(),
    )
    .to_string();
    let request_json = serde_json::json!({
        "method": request.method,
        "scheme": request.scheme,
        "authority": request.authority,
        "path": request.path,
        "query": request.query.clone().unwrap_or_default(),
        "headersJson": headers_json,
        "body": body,
    })
    .to_string();

    let state_str = get_current_state();
    let roc_state = RocStr::from_str(&state_str);
    let roc_request = RocStr::from_str(&request_json);
    let mut roc_out = MaybeUninit::<RocResult<RocStr, RocStr>>::uninit();

    let response_json = unsafe {
        main_handle_request_for_host_1_exposed_generic(roc_out.as_mut_ptr(), &roc_state, &roc_request);
        core::mem::forget(roc_state);
        core::mem::forget(roc_request);
        match roc_out.assume_init().into_result() {
            Ok(payload) => payload.to_string(),
            Err(err) => return Err(AgentError::InvalidMethod(err.to_string())),
        }
    };

    let parsed: serde_json::Value = serde_json::from_str(&response_json).map_err(|err| {
        AgentError::InvalidMethod(format!("the app's response is not valid JSON: {err}"))
    })?;
    if let Some(state) = parsed.get("state") {
        set_current_state(state.to_string());
    }
    let status = parsed
        .get("status")
        .and_then(|v| v.as_u64())
        .unwrap_or(200) as u16;
    let mut header_nodes = Vec::new();
    let mut value_nodes = vec![SchemaValueNode::RecordValue(vec![1, 2, 3])];
    value_nodes.push(SchemaValueNode::U16Value(status));
    // Replaced with the header list once its entries exist.
    value_nodes.push(SchemaValueNode::ListValue(Vec::new()));
    if let Some(headers) = parsed.get("headers").and_then(|v| v.as_array()) {
        for header in headers {
            let name = header.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let value = header.get("value").and_then(|v| v.as_str()).unwrap_or("");
            let name_index = value_nodes.len() as i32;
            value_nodes.push(SchemaValueNode::StringValue(name.to_string()));
            let mut byte_nodes = Vec::new();
            for byte in value.bytes() {
                byte_nodes.push(value_nodes.len() as i32);
                value_nodes.push(SchemaValueNode::U8Value(byte));
            }
            let value_index = value_nodes.len() as i32;
            value_nodes.push(SchemaValueNode::ListValue(byte_nodes));
            let entry_index = value_nodes.len() as i32;
            value_nodes.push(SchemaValueNode::RecordValue(vec![name_index, value_index]));
            header_nodes.push(entry_index);
        }
    }
    value_nodes[2] = SchemaValueNode::ListValue(header_nodes);

    // The response body is a stream of `list<u8>` chunks. The runtime starts
    // reading it only after this call returns, so the write runs as a task the
    // export's executor keeps driving until the body is delivered — that is
    // what the fork's `spawn_local` is for ("execution after returning"). A
    // write that still cannot complete is reported through the log rather than
    // silently truncating the body.
    let (mut body_tx, body_rx) = wit_stream::new();
    let body_text = parsed.get("body").and_then(|v| v.as_str()).unwrap_or("");
    let mut body_nodes = Vec::new();
    for byte in body_text.bytes() {
        body_nodes.push(value_nodes.len() as i32);
        value_nodes.push(SchemaValueNode::U8Value(byte));
    }
    let chunk = SchemaValueTree {
        value_nodes: vec![SchemaValueNode::ListValue(body_nodes)],
        root: 0,
    };
    // The order matters: handing the read end to the runtime first (`wrap`) lets
    // it start draining the stream, which is what the write below waits for. The
    // Rust SDK's router does the same, and a body written *before* the transfer
    // deadlocks (the stream is a rendezvous).
    let handle = golem::core::types::SchemaValueStream::wrap(body_rx).await;
    let leftover = body_tx.write_all(vec![chunk]).await;
    drop(body_tx);
    if !leftover.is_empty() {
        crate::wasi::logging::logging::log(
            crate::wasi::logging::logging::Level::Warn,
            "roc-agent",
            "the response body was not fully delivered to the runtime",
        );
    }
    let body_index = value_nodes.len() as i32;
    value_nodes.push(SchemaValueNode::StreamValue(handle));
    value_nodes[0] = SchemaValueNode::RecordValue(vec![1, 2, body_index]);

    Ok(Some(SchemaValueTree {
        value_nodes,
        root: 0,
    }))
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
    let mut http_mount: Option<HttpMountDetails> = None;
    let mut mount_error: Option<String> = None;
    let mut is_router = false;
    let mut wants_handler = false;
    let mut openapi_document: Option<String> = None;
    let mut metadata_error: Option<String> = None;

    if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&meta_json) {
        // The agent kind decides the rest: a router registers no application
        // methods and has no constructor parameters.
        is_router = meta.get("agentKind").and_then(|v| v.as_str()) == Some("http-router");
        wants_handler = meta.get("httpHandler").and_then(|v| v.as_str()) == Some("handler");
        if let Some(document) = meta.get("openapiProvider").and_then(|v| v.as_str()) {
            if !document.is_empty() {
                openapi_document = Some(document.to_string());
            }
        }
        if is_router {
            methods.clear();
        }
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
                if is_router {
                    metadata_error = Some(
                        "an http-router agent cannot declare tools: routers register only their \
                         OpenAPI provider method"
                            .to_string(),
                    );
                    break;
                }
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
        match meta.get("httpMount") {
            None | Some(serde_json::Value::Null) => {}
            Some(mount) => match parse_http_mount(mount, is_router) {
                Ok(details) => http_mount = Some(details),
                Err(err) => mount_error = Some(err),
            },
        }
    }

    // What the mount is allowed to serve depends on the kind: routers serve
    // uploaded static files and/or an OpenAPI document and must stay
    // parameterless and literal-mounted, while filesystem bindings need the
    // durable regular agent that owns the filesystem.
    let mut kind_error: Option<String> = None;
    if is_router {
        match &http_mount {
            None => {
                kind_error = Some("an http-router agent needs an httpMount".to_string());
            }
            Some(mount) => {
                if mount
                    .path_prefix
                    .iter()
                    .any(|segment| !matches!(segment, PathSegment::Literal(_)))
                {
                    kind_error = Some(
                        "an http-router mount cannot capture path variables: routers have no \
                         constructor parameters"
                            .to_string(),
                    );
                } else if !mount.filesystem_bindings.is_empty() {
                    kind_error = Some(
                        "filesystem bindings need a regular durable agent; a router serves \
                         staticBindings"
                            .to_string(),
                    );
                } else if mount.static_bindings.is_empty()
                    && openapi_document.is_none()
                    && !wants_handler
                {
                    kind_error = Some(
                        "an http-router mount must serve staticBindings, an OpenAPI document or \
                         requests (httpHandler)"
                            .to_string(),
                    );
                }
            }
        }
    } else if wants_handler {
        kind_error = Some(
            "httpHandler needs the http-router agent kind: only a router receives requests"
                .to_string(),
        );
    } else if http_mount
        .as_ref()
        .is_some_and(|mount| !mount.static_bindings.is_empty())
    {
        kind_error = Some(
            "staticBindings need the http-router agent kind (they are served from files \
             uploaded at deploy time, not from an agent filesystem)"
                .to_string(),
        );
    }

    // A router's methods are the OpenAPI provider and, when the app handles
    // requests, the handler Golem routes everything the static bindings do not
    // match to.
    let mut router_schema: Option<RouterSchema> = None;
    if is_router {
        let schema = router_schema_graph();
        if wants_handler {
            methods.push(AgentMethod {
                name: "handle".to_string(),
                description: "Handles an HTTP request routed to this mount".to_string(),
                http_endpoint: vec![any_endpoint()],
                prompt_hint: None,
                input_schema: InputSchema::Parameters(vec![NamedField {
                    name: "request".to_string(),
                    source: FieldSource::UserSupplied,
                    schema: schema.request,
                    metadata: empty_metadata(),
                }]),
                output_schema: OutputSchema::Single(schema.response),
                read_only: None,
            });
        }
        router_schema = Some(schema);
    }

    // A router's only other method is the OpenAPI provider, which Golem calls to
    // build the deployment's `/openapi.yaml`.
    let openapi_provider_method = if is_router && openapi_document.is_some() {
        if let Some(document) = &openapi_document {
            if let Err(err) = validate_openapi_document(document) {
                metadata_error = Some(format!("openapiProvider is not usable: {err}"));
            }
        }
        methods.push(AgentMethod {
            name: "openapi".to_string(),
            description: "Returns this router's OpenAPI document".to_string(),
            http_endpoint: vec![],
            prompt_hint: None,
            input_schema: InputSchema::Parameters(vec![]),
            output_schema: OutputSchema::Single(0),
            read_only: None,
        });
        Some("openapi".to_string())
    } else {
        None
    };
    if let Some(mount) = http_mount.as_mut() {
        mount.openapi_provider_method = openapi_provider_method;
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
    } else if let Some(err) = mount_error {
        Err(AgentError::InvalidType(format!(
            "app metadata has an invalid httpMount: {err}"
        )))
    } else if let Some(err) = metadata_error {
        Err(AgentError::InvalidType(format!("app metadata is invalid: {err}")))
    } else if let Some(err) = kind_error {
        Err(AgentError::InvalidType(format!("app metadata is invalid: {err}")))
    } else {
        Ok(AgentType {
            type_name: name_str,
            kind: if is_router {
                AgentTypeKind::HttpRouter
            } else {
                AgentTypeKind::Regular
            },
            description: description_str,
            source_language: "roc".to_string(),
            schema: match &router_schema {
                Some(schema) => schema.graph.clone(),
                None => string_schema_graph(),
            },
            constructor: AgentConstructor {
                name: None,
                description: "Initializes the agent".to_string(),
                prompt_hint: None,
                input_schema: InputSchema::Parameters(if is_router {
                    // Routers are parameterless: their mount is literal-only, so
                    // there is nothing to bind a constructor parameter to.
                    vec![]
                } else {
                    vec![string_named_field(&constructor_param_name(&http_mount))]
                }),
            },
            methods,
            dependencies: vec![],
            mode: if is_router {
                AgentMode::Ephemeral
            } else {
                AgentMode::Durable
            },
            http_mount,
            snapshotting: Snapshotting::Disabled,
            config: config_decls,
        })
    };
    agent_type
}

/// Golem parses the provider's document as strict OpenAPI 3.1 JSON and answers a
/// request for the deployment's spec with a 502 when it cannot, so check the
/// parts it insists on here: the deployment fails with a message that names the
/// app instead of a route that breaks at request time.
fn validate_openapi_document(document: &str) -> Result<(), String> {
    let value: serde_json::Value = serde_json::from_str(document)
        .map_err(|err| format!("it is not valid JSON: {err}"))?;
    if value.get("openapi").and_then(|v| v.as_str()) != Some("3.1.0") {
        return Err("it must declare \"openapi\": \"3.1.0\"".to_string());
    }
    if !value.get("paths").is_some_and(|paths| paths.is_object()) {
        return Err("it must have a \"paths\" object".to_string());
    }
    Ok(())
}

/// The OpenAPI document the app declared, read from the metadata hook the same
/// way the agent type is built. The provider method returns it verbatim.
fn openapi_document() -> Option<String> {
    let mut roc_out = MaybeUninit::<RocStr>::uninit();
    let meta_json = unsafe {
        main_metadata_for_host_1_exposed_generic(roc_out.as_mut_ptr());
        roc_out.assume_init().to_string()
    };
    serde_json::from_str::<serde_json::Value>(&meta_json)
        .ok()?
        .get("openapiProvider")?
        .as_str()
        .filter(|document| !document.is_empty())
        .map(|document| document.to_string())
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

fn parse_method(m: &str) -> wasi::http0_3_0::types::Method {
    use wasi::http0_3_0::types::Method;
    match m.to_uppercase().as_str() {
        "GET" => Method::Get,
        "POST" => Method::Post,
        "PUT" => Method::Put,
        "DELETE" => Method::Delete,
        "PATCH" => Method::Patch,
        "HEAD" => Method::Head,
        "OPTIONS" => Method::Options,
        "CONNECT" => Method::Connect,
        "TRACE" => Method::Trace,
        other => Method::Other(other.to_string()),
    }
}

fn parse_url(url: &str) -> Result<(wasi::http0_3_0::types::Scheme, String, String), String> {
    use wasi::http0_3_0::types::Scheme;
    let (scheme, rest) = if let Some(stripped) = url.strip_prefix("https://") {
        (Scheme::Https, stripped)
    } else if let Some(stripped) = url.strip_prefix("http://") {
        (Scheme::Http, stripped)
    } else if let Some(idx) = url.find("://") {
        let (s, r) = url.split_at(idx);
        (Scheme::Other(s.to_string()), &r[3..])
    } else {
        (Scheme::Https, url)
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

/// One parsed `http!` request: what a WASI p3 request needs, already decoded.
struct HttpRequestParts {
    method: wasi::http0_3_0::types::Method,
    scheme: wasi::http0_3_0::types::Scheme,
    authority: String,
    path_with_query: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

/// Parses the effect's JSON payload (`{url, method, headers, body}`), accepting a
/// bare URL string as well.
fn parse_http_request(req_str: &str) -> Result<HttpRequestParts, String> {
    let mut method_name = String::from("GET");
    let mut url = String::new();
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut body_bytes: Option<Vec<u8>> = None;

    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(req_str) {
        if let Some(obj) = parsed.as_object() {
            if let Some(u) = obj
                .get("url")
                .and_then(|v| v.as_str())
                .or_else(|| obj.get("uri").and_then(|v| v.as_str()))
            {
                url = u.to_string();
            }
            if let Some(m) = obj.get("method").and_then(|v| v.as_str()) {
                method_name = m.to_string();
            }
            if let Some(h) = obj.get("headers") {
                if let Some(h_obj) = h.as_object() {
                    for (k, v) in h_obj {
                        let val_str = if let Some(s) = v.as_str() {
                            s.to_string()
                        } else {
                            v.to_string()
                        };
                        headers.push((k.clone(), val_str));
                    }
                } else if let Some(h_arr) = h.as_array() {
                    for item in h_arr {
                        if let Some(pair) = item.as_array() {
                            if pair.len() >= 2 {
                                let k = pair[0].as_str().unwrap_or("").to_string();
                                let v = pair[1].as_str().unwrap_or("").to_string();
                                if !k.is_empty() {
                                    headers.push((k, v));
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
            return Err(format!(
                "Invalid HTTP request payload (missing 'url'): {}",
                req_str
            ));
        }
    }

    let (scheme, authority, path_with_query) = parse_url(&url)?;
    Ok(HttpRequestParts {
        method: parse_method(&method_name),
        scheme,
        authority,
        path_with_query,
        headers,
        body: body_bytes.unwrap_or_default(),
    })
}

/// Runs one `http!` request. Golem 1.6.0 dropped the p2 WASI HTTP interfaces
/// (`wasi:http/outgoing-handler@0.2.6`), so this goes through WASI p3
/// (`wasi:http/client@0.3.0`) instead.
pub fn execute_http_request(req_str: &str) -> Result<String, String> {
    let parts = parse_http_request(req_str)?;
    // The Roc side calls this effect synchronously, so `block_on` drives the
    // async p3 call (and the body upload running alongside it) to completion.
    wit_bindgen::block_on(send_http_request(parts))
}

/// Sends one buffered request and renders the response the way the effect
/// promises: `{status, headers, body}` as JSON.
async fn send_http_request(parts: HttpRequestParts) -> Result<String, String> {
    use wasi::http0_3_0::client;
    use wasi::http0_3_0::types::{Fields, Request, Response};

    let fields = Fields::new();
    for (name, value) in &parts.headers {
        fields
            .append(name, value.as_bytes())
            .map_err(|err| format!("invalid HTTP header {name:?}: {err:?}"))?;
    }

    // The body is a p3 stream: `send` consumes it while we write into it, so the
    // two run concurrently (the stream has a bounded buffer, so writing first
    // would deadlock on a large body).
    let (mut body_tx, body_rx) = wit_stream::new();
    // No trailers: dropping the writer leaves the future resolved to `Ok(None)`.
    let (trailers_tx, trailers_rx) = wit_future::new(|| Ok(None));
    let (request, transmitted) = Request::new(fields, Some(body_rx), trailers_rx, None);
    drop(trailers_tx);

    request
        .set_method(&parts.method)
        .map_err(|()| "invalid HTTP method".to_string())?;
    request
        .set_scheme(Some(&parts.scheme))
        .map_err(|()| "invalid HTTP scheme".to_string())?;
    request
        .set_authority(Some(parts.authority.as_str()))
        .map_err(|()| "invalid HTTP authority".to_string())?;
    request
        .set_path_with_query(Some(parts.path_with_query.as_str()))
        .map_err(|()| "invalid HTTP path and query".to_string())?;

    let body = parts.body;
    let upload = async move {
        let remaining = body_tx.write_all(body).await;
        drop(body_tx);
        remaining
    };

    let (response, remaining) = join2(client::send(request), upload).await;
    if !remaining.is_empty() {
        return Err(format!(
            "HTTP request body was not fully sent ({} bytes left)",
            remaining.len()
        ));
    }
    transmitted
        .await
        .map_err(|err| format!("HTTP request transmission failed: {err:?}"))?;
    let response = response.map_err(|err| format!("HTTP request failed: {err:?}"))?;

    let status = response.get_status_code();
    let mut headers_map = serde_json::Map::new();
    for (name, value) in response.get_headers().copy_all() {
        headers_map.insert(
            name,
            serde_json::Value::String(String::from_utf8_lossy(&value).to_string()),
        );
    }

    let (res_tx, res_rx) = wit_future::new(|| Ok(()));
    let (body_reader, trailers) = Response::consume_body(response, res_rx);
    drop(res_tx);
    let body_bytes = body_reader.collect().await;
    // A failed body stream means the bytes above may be truncated; surface it
    // rather than returning a silently short body.
    if let Err(err) = trailers.await {
        return Err(format!("HTTP response body failed: {err:?}"));
    }

    Ok(serde_json::json!({
        "status": status,
        "headers": headers_map,
        "body": String::from_utf8_lossy(&body_bytes).to_string(),
    })
    .to_string())
}

/// Polls two futures to completion concurrently, yielding both outputs.
///
/// `http!` needs this to upload a request body while the host transmits the
/// request. The component model's `spawn` is not an option here: our async
/// export executor is Golem's `wit-bindgen` fork, and a task spawned by another
/// wit-bindgen version is never polled by it.
fn join2<A, B>(a: A, b: B) -> Join2<A, B>
where
    A: core::future::Future,
    B: core::future::Future,
    A::Output: Unpin,
    B::Output: Unpin,
{
    Join2 {
        a: Some(Box::pin(a)),
        b: Some(Box::pin(b)),
        out_a: None,
        out_b: None,
    }
}

struct Join2<A, B>
where
    A: core::future::Future,
    B: core::future::Future,
    A::Output: Unpin,
    B::Output: Unpin,
{
    a: Option<core::pin::Pin<Box<A>>>,
    b: Option<core::pin::Pin<Box<B>>>,
    out_a: Option<A::Output>,
    out_b: Option<B::Output>,
}

impl<A, B> core::future::Future for Join2<A, B>
where
    A: core::future::Future,
    B: core::future::Future,
    A::Output: Unpin,
    B::Output: Unpin,
{
    type Output = (A::Output, B::Output);

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        let this = self.get_mut();
        if this.out_a.is_none() {
            if let Some(future) = this.a.as_mut() {
                if let core::task::Poll::Ready(value) = future.as_mut().poll(cx) {
                    this.out_a = Some(value);
                    this.a = None;
                }
            }
        }
        if this.out_b.is_none() {
            if let Some(future) = this.b.as_mut() {
                if let core::task::Poll::Ready(value) = future.as_mut().poll(cx) {
                    this.out_b = Some(value);
                    this.b = None;
                }
            }
        }
        match (this.out_a.take(), this.out_b.take()) {
            (Some(a), Some(b)) => core::task::Poll::Ready((a, b)),
            (a, b) => {
                this.out_a = a;
                this.out_b = b;
                core::task::Poll::Pending
            }
        }
    }
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
    // Golem 1.6.0 added a third parameter (the connection's reconstruction
    // policy); `None` keeps the default, which reconnects automatically.
    match golem::websocket::client::WebsocketConnection::connect(url_str, None, None) {
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

        // A router's handler: Golem hands us the request and takes back the
        // response, both as schema values with streamed bodies.
        if method_name == "handle" {
            let mut input = input;
            return handle_router_request(&mut input).await;
        }

        let input_str = extract_input_string(&input);

        // A router's provider method: Golem calls it to build the deployment's
        // `/openapi.yaml`, and it returns the app's document verbatim.
        if method_name == "openapi" {
            return match openapi_document() {
                Some(document) => Ok(Some(single_string_value_tree(&document))),
                None => Err(AgentError::InvalidMethod(
                    "this agent type declares no OpenAPI document".to_string(),
                )),
            };
        }

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

    /// The mount metadata decides three things the deploy-time validator
    /// checks: the path captures that bind the constructor parameter, the
    /// file mappings, and the response headers. Pin all three.
    #[test]
    fn test_parse_http_mount() {
        let mount: serde_json::Value = serde_json::json!({
            "pathPrefix": ["site", "{id}"],
            "filesystemBindings": [
                {"subtree": {"publicPrefix": ["assets"], "filesystemRoot": "/assets"}},
                {"exact": {"publicPath": ["index.html"], "filePath": "/index.html"}},
            ],
            "fileResponseHeaders": [{"name": "x-frame-options", "value": "DENY"}],
            "corsAllowedPatterns": ["https://example.com"],
        });

        let details = parse_http_mount(&mount, false).expect("mount should parse");
        // The mount's capture is what names the constructor parameter.
        assert_eq!(constructor_param_name(&Some(details.clone())), "id");
        assert_eq!(constructor_param_name(&None), "config");
        assert_eq!(details.path_prefix.len(), 2);
        assert!(matches!(
            &details.path_prefix[0],
            PathSegment::Literal(literal) if literal == "site"
        ));
        assert!(matches!(
            &details.path_prefix[1],
            PathSegment::PathVariable(variable) if variable.variable_name == "id"
        ));
        assert_eq!(details.filesystem_bindings.len(), 2);
        assert!(matches!(
            &details.filesystem_bindings[0],
            FileMapping::Subtree(subtree)
                if subtree.public_prefix == vec!["assets".to_string()]
                    && subtree.filesystem_root == "/assets"
        ));
        assert!(matches!(
            &details.filesystem_bindings[1],
            FileMapping::Exact(exact)
                if exact.public_path == vec!["index.html".to_string()]
                    && exact.file_path == "/index.html"
        ));
        assert_eq!(details.file_response_headers.len(), 1);
        assert_eq!(details.file_response_headers[0].name, "x-frame-options");
        assert_eq!(
            details.cors_options.allowed_patterns,
            vec!["https://example.com".to_string()]
        );
        // Static bindings and an OpenAPI provider need a router agent type,
        // which this platform does not emit yet.
        assert!(details.static_bindings.is_empty());
        assert!(details.openapi_provider_method.is_none());
    }

    #[test]
    fn test_parse_http_mount_rejects_unbound_mounts() {
        // Golem binds every constructor parameter through the mount, so a
        // mount that captures nothing (or more than one value) cannot be
        // deployed; refuse it with an explanation instead.
        let no_capture: serde_json::Value = serde_json::json!({
            "pathPrefix": ["site"],
            "filesystemBindings": [],
        });
        let err = parse_http_mount(&no_capture, false).expect_err("mount without a capture");
        assert!(err.contains("must capture"), "unexpected error: {err}");

        let two_captures: serde_json::Value = serde_json::json!({
            "pathPrefix": ["{a}", "{b}"],
            "filesystemBindings": [],
        });
        let err = parse_http_mount(&two_captures, false).expect_err("mount with two captures");
        assert!(err.contains("single"), "unexpected error: {err}");

        // A router's mount is literal-only, and it must not capture anything.
        let router_capture: serde_json::Value = serde_json::json!({
            "pathPrefix": ["static", "{id}"],
            "staticBindings": [],
        });
        let err = parse_http_mount(&router_capture, true).expect_err("router with a capture");
        assert!(
            err.contains("cannot capture path variables"),
            "unexpected error: {err}"
        );
        let router_ok: serde_json::Value = serde_json::json!({
            "pathPrefix": ["static"],
            "staticBindings": [{"exact": {"publicPath": ["index.html"], "filePath": "/index.html"}}],
        });
        let details = parse_http_mount(&router_ok, true).expect("router mount should parse");
        assert_eq!(details.static_bindings.len(), 1);

        let bad_segment: serde_json::Value = serde_json::json!({
            "pathPrefix": ["site", "{id}"],
            "filesystemBindings": [{"subtree": {"publicPrefix": ["../etc"], "filesystemRoot": "/assets"}}],
        });
        let err = parse_http_mount(&bad_segment, false).expect_err("bad public segment");
        assert!(
            err.contains("not a valid path segment"),
            "unexpected error: {err}"
        );

        let bad_target: serde_json::Value = serde_json::json!({
            "pathPrefix": ["site", "{id}"],
            "filesystemBindings": [{"exact": {"publicPath": ["a.txt"], "filePath": "a.txt"}}],
        });
        let err = parse_http_mount(&bad_target, false).expect_err("relative file target");
        assert!(err.contains("absolute"), "unexpected error: {err}");
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
    fn test_parse_http_request() {
        // The effect's payload is JSON; a bare URL is accepted too.
        let parts = parse_http_request(
            r#"{"url":"http://example.test/a?b=1","method":"post","headers":{"x-token":"t","n":3},"body":"hi"}"#,
        )
        .expect("payload should parse");
        assert!(matches!(parts.method, wasi::http0_3_0::types::Method::Post));
        assert!(matches!(parts.scheme, wasi::http0_3_0::types::Scheme::Http));
        assert_eq!(parts.authority, "example.test");
        assert_eq!(parts.path_with_query, "/a?b=1");
        assert_eq!(
            parts.headers,
            vec![
                ("n".to_string(), "3".to_string()),
                ("x-token".to_string(), "t".to_string())
            ]
        );
        assert_eq!(parts.body, b"hi".to_vec());

        let parts = parse_http_request("https://example.test").expect("bare URL should parse");
        assert!(matches!(parts.method, wasi::http0_3_0::types::Method::Get));
        assert_eq!(parts.path_with_query, "/");
        assert!(parts.body.is_empty());

        assert!(parse_http_request("not a url").is_err());
    }

    #[test]
    fn test_validate_openapi_document() {
        assert!(validate_openapi_document(
            r#"{"openapi":"3.1.0","paths":{"/static/index.html":{"get":{}}}}"#
        )
        .is_ok());
        // What Golem rejects with a 502 when the deployment serves its spec.
        assert!(validate_openapi_document("not json").is_err());
        assert!(validate_openapi_document(r#"{"openapi":"3.0.0","paths":{}}"#).is_err());
        assert!(validate_openapi_document(r#"{"openapi":"3.1.0"}"#).is_err());
    }

    #[test]
    fn test_parse_url() {
        let (scheme, auth, path) = parse_url("https://api.example.com/v1/chat?q=test").unwrap();
        assert!(matches!(scheme, wasi::http0_3_0::types::Scheme::Https));
        assert_eq!(auth, "api.example.com");
        assert_eq!(path, "/v1/chat?q=test");

        let (scheme, auth, path) = parse_url("http://localhost:8080").unwrap();
        assert!(matches!(scheme, wasi::http0_3_0::types::Scheme::Http));
        assert_eq!(auth, "localhost:8080");
        assert_eq!(path, "/");

        let (scheme, auth, path) = parse_url("custom://my-host/path").unwrap();
        assert!(matches!(scheme, wasi::http0_3_0::types::Scheme::Other(s) if s == "custom"));
        assert_eq!(auth, "my-host");
        assert_eq!(path, "/path");
    }

    #[test]
    fn test_parse_method() {
        assert!(matches!(parse_method("get"), wasi::http0_3_0::types::Method::Get));
        assert!(matches!(parse_method("POST"), wasi::http0_3_0::types::Method::Post));
        assert!(matches!(parse_method("put"), wasi::http0_3_0::types::Method::Put));
        assert!(matches!(parse_method("DELETE"), wasi::http0_3_0::types::Method::Delete));
        assert!(matches!(parse_method("PATCH"), wasi::http0_3_0::types::Method::Patch));
        assert!(matches!(parse_method("custom"), wasi::http0_3_0::types::Method::Other(s) if s == "CUSTOM"));
    }
}
