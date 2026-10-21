#[cfg(any(feature = "stub-guest", test))]
pub mod guest_bridge;
/// The compiler-generated ABI layer (`roc glue` with the official
/// `RustGlue.roc`): the ABI-visible types, retain/release plans, the hosted
/// symbols the host must export, and owning wrappers for the provided
/// entrypoints. Regenerate with:
///
/// ```bash
/// curl -s -o /tmp/RustGlue.roc \
///   https://raw.githubusercontent.com/roc-lang/roc/<nightly-sha>/src/glue/src/RustGlue.roc
/// roc glue /tmp/RustGlue.roc host/src host/src/main.roc  # see tooling/glue/README.md
/// ```
///
/// It is checked in so the hand-written ABI in `roc_std.rs` can be compared
/// against it (see `tests::test_generated_abi_agrees_with_hand_written`), and so
/// the migration to the generated types can proceed incrementally.
///
/// While both layers exist, the generated `extern` declarations of the provided
/// entrypoints and this crate's own `sret`-style definitions describe the same C
/// ABI in two spellings; the definitions go away as the migration completes.
#[allow(clashing_extern_declarations)]
pub mod roc_platform_abi;
pub mod roc_std;
/// The JSON mirror of the retry policy/predicate trees (`golem:api/retry`).
mod retry_json;
/// The JSON mirror of the three `golem:rdbms` `db-value` vocabularies, and the
/// effects that carry them (`dbOpen!`, `dbQuery!`, …).
mod rdbms_json;

use core::cell::RefCell;
use core::mem::MaybeUninit;
use core::ptr;
use std::boxed::Box;
use std::collections::HashMap;
use std::thread::LocalKey;
use roc_std::{RocResult, RocStr, RocU64Result};
use wasi::filesystem0_2_6::preopens;
use wasi::filesystem0_2_6::types::{
    Descriptor, DescriptorFlags, DescriptorType, ErrorCode, OpenFlags, PathFlags,
};

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

    // The response body is a stream of `list<u8>` chunks. On Golem 1.6.0 a
    // *guest-created* stream placed here is accepted but never delivered: the
    // items do not reach the invocation session, while the same slot works for a
    // stream the runtime created itself (the measurements are in
    // `docs/router-handler-status.md`). The write stays — it is the shape the
    // platform needs once the runtime forwards such a stream — but an app's body
    // is reported rather than silently dropped, and the README says what a
    // handler can expect.
    let (mut body_tx, body_rx) = wit_stream::new();
    let body_text = parsed.get("body").and_then(|v| v.as_str()).unwrap_or("");
    if !body_text.is_empty() {
        crate::wasi::logging::logging::log(
            crate::wasi::logging::logging::Level::Warn,
            "roc-agent",
            "the app's response body cannot be delivered on Golem 1.6.0: a guest-created \
             stream in the response value is not forwarded to the invocation session, so \
             only the status and headers reach the client (docs/router-handler-status.md)",
        );
    }
    let mut body_nodes = Vec::new();
    for byte in body_text.bytes() {
        body_nodes.push(value_nodes.len() as i32);
        value_nodes.push(SchemaValueNode::U8Value(byte));
    }
    let chunk = SchemaValueTree {
        value_nodes: vec![SchemaValueNode::ListValue(body_nodes)],
        root: 0,
    };
    // Handing the read end to the runtime first (`wrap`) is what the SDK's
    // router does too; a write *before* the transfer deadlocks, because the
    // stream is a rendezvous.
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
pub unsafe extern "C" fn rocFxLog(level: u8, msg: RocStr) {
    // `msg` is owned by this function (Roc transfers ownership of hosted
    // arguments), so dropping it at the end of the call releases it.
    let text = msg.as_str();
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
pub unsafe extern "C" fn rocFxGetWorkerId() -> RocStr {
    let worker_id = match golem::api::host::get_self_metadata() {
        Ok(metadata) => metadata.agent_id.agent_id,
        Err(_) => String::from("unknown"),
    };
    RocStr::from_str(&worker_id)
}

// Resource handle tables
//
// Some effects hand a host-side resource (a websocket connection, a
// cancellation token) to the app as a `U32` handle: the index into a
// thread-local table, offset by one so that 0 is never a valid handle.

fn handle_store<T>(table: &'static LocalKey<RefCell<Vec<Option<T>>>>, value: T) -> u32 {
    table.with(|slots| slot_store(&mut slots.borrow_mut(), value))
}

/// The body of `handle_store`, for callers that already hold the table's
/// borrow: an effect that has to look one handle up *and* store another (the
/// transaction `dbBegin!` starts on a connection) must do both inside one
/// borrow, or the second `borrow_mut` panics.
fn slot_store<T>(slots: &mut Vec<Option<T>>, value: T) -> u32 {
    if let Some(index) = slots.iter().position(|slot| slot.is_none()) {
        slots[index] = Some(value);
        (index + 1) as u32
    } else {
        slots.push(Some(value));
        slots.len() as u32
    }
}

fn handle_with<T, R>(
    table: &'static LocalKey<RefCell<Vec<Option<T>>>>,
    handle: u32,
    f: impl FnOnce(&T) -> R,
) -> Option<R> {
    if handle == 0 {
        return None;
    }
    table.with(|slots| {
        slots
            .borrow()
            .get((handle - 1) as usize)
            .and_then(|slot| slot.as_ref())
            .map(f)
    })
}

fn handle_take<T>(table: &'static LocalKey<RefCell<Vec<Option<T>>>>, handle: u32) -> Option<T> {
    if handle == 0 {
        return None;
    }
    table.with(|slots| {
        slots
            .borrow_mut()
            .get_mut((handle - 1) as usize)
            .and_then(|slot| slot.take())
    })
}

thread_local! {
    static WS_CONNECTIONS: RefCell<Vec<Option<golem::websocket::client::WebsocketConnection>>> =
        const { RefCell::new(Vec::new()) };
    /// Cancellation tokens of scheduled invocations that have not run yet.
    static CANCELABLE_INVOCATIONS: RefCell<
        Vec<Option<golem::agent::host::CancellationToken>>,
    > = const { RefCell::new(Vec::new()) };
}

// Agent-to-agent calls
//
// `rpc!` is the invoke-and-await form. The rest of what `golem:agent/host`
// offers for calling another agent lives next to it: fire-and-forget,
// scheduled (optionally cancelable) invocations, and the agent-id helpers the
// RPC path resolves its target with.

/// Resolves an agent id into an RPC client the way `rpc!` always has. The
/// target is an agent id, as produced by `make-agent-id` / the Golem CLI (for
/// example `counter-agent("{}")`); `parse-agent-id` returns the target's
/// constructor parameters as a self-contained typed value, which the 2.0
/// `wasm-rpc` resource accepts directly. `WasmRpc::new` is the fail-fast
/// constructor, so an unusable target traps rather than returning an error.
fn rpc_client_for(target: &str) -> Result<golem::agent::host::WasmRpc, String> {
    match golem::agent::host::parse_agent_id(target) {
        Ok((agent_type_name, constructor, phantom_id)) => Ok(golem::agent::host::WasmRpc::new(
            &agent_type_name,
            constructor.value,
            phantom_id,
            Vec::new(),
        )),
        Err(err) => Err(format!("invalid agent id {:?}: {:?}", target, err)),
    }
}

/// Renders the identity Golem allocated for one outbound invocation
/// (`invocation-metadata`) as JSON, which is how the platform hands
/// multi-field results to Roc.
fn invocation_identity_json(metadata: &golem::agent::host::InvocationMetadata) -> String {
    serde_json::json!({
        "agentId": metadata.agent_id.as_str(),
        "idempotencyKey": metadata.idempotency_key.as_str(),
    })
    .to_string()
}

/// `now` plus `delay_millis`, as the `instant` the scheduling calls take
/// (`nanoseconds` stays below a second, which is what the host expects).
fn instant_after(
    now: golem::agent::host::Datetime,
    delay_millis: u64,
) -> golem::agent::host::Datetime {
    let nanos = (now.nanoseconds as u64).saturating_add(delay_millis.saturating_mul(1_000_000));
    golem::agent::host::Datetime {
        seconds: now.seconds.saturating_add((nanos / 1_000_000_000) as i64),
        nanoseconds: (nanos % 1_000_000_000) as u32,
    }
}

/// The instant `delay_millis` from now. Reading the clock is a durable host
/// call: Golem records it in the agent's oplog, so a replayed invocation
/// computes the same scheduled time instead of drifting by however long the
/// replay took.
fn scheduled_instant_in(delay_millis: u64) -> golem::agent::host::Datetime {
    instant_after(wasi::clocks0_3_0::system_clock::now(), delay_millis)
}

/// The `Try(Str, Str)` every RPC effect returns, from the `Result` its host
/// calls produce.
fn rpc_result(result: Result<String, String>) -> RocResult<RocStr, RocStr> {
    match result {
        Ok(text) => RocResult::ok(RocStr::from_str(&text)),
        Err(err) => RocResult::err(RocStr::from_str(&err)),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxRpcInvoke(
    target: RocStr,
    function_name: RocStr,
    payload: RocStr,
) -> RocResult<RocStr, RocStr> {
    // All three arguments are owned by this function and released when they drop.
    let target_str = target.as_str();
    let method = function_name.as_str();
    let payload_str = if payload.is_empty() { "{}" } else { payload.as_str() };

    let result = match rpc_client_for(target_str) {
        Ok(rpc) => match rpc.invoke_and_await(method, single_param_value_tree(payload_str), None) {
            Ok(invocation) => match invocation.result {
                Some(tree) => Ok(extract_input_string(&tree)),
                None => Ok(String::from("")),
            },
            Err(err) => Err(format!("rpc to {} failed: {:?}", target_str, err)),
        },
        Err(err) => Err(err),
    };

    rpc_result(result)
}

/// Triggers a remote method and returns its invocation identity right away,
/// instead of waiting for the reply the way `rocFxRpcInvoke` does.
#[no_mangle]
pub unsafe extern "C" fn rocFxRpcFireAndForget(
    target: RocStr,
    function_name: RocStr,
    payload: RocStr,
) -> RocResult<RocStr, RocStr> {
    let target_str = target.as_str();
    let method = function_name.as_str();
    let payload_str = if payload.is_empty() { "{}" } else { payload.as_str() };

    let result = match rpc_client_for(target_str) {
        Ok(rpc) => match rpc.invoke(method, single_param_value_tree(payload_str), None) {
            Ok(metadata) => Ok(invocation_identity_json(&metadata)),
            Err(err) => Err(format!("rpc to {} failed: {:?}", target_str, err)),
        },
        Err(err) => Err(err),
    };

    rpc_result(result)
}

/// Schedules a remote method for `delay_millis` from now and returns the
/// scheduled invocation's identity.
#[no_mangle]
pub unsafe extern "C" fn rocFxRpcSchedule(
    target: RocStr,
    function_name: RocStr,
    payload: RocStr,
    delay_millis: u64,
) -> RocResult<RocStr, RocStr> {
    let target_str = target.as_str();
    let method = function_name.as_str();
    let payload_str = if payload.is_empty() { "{}" } else { payload.as_str() };

    let result = match rpc_client_for(target_str) {
        Ok(rpc) => {
            let input = single_param_value_tree(payload_str);
            let scheduled_time = scheduled_instant_in(delay_millis);
            match rpc.schedule_invocation(scheduled_time, method, input, None) {
                Ok(receipt) => Ok(invocation_identity_json(&receipt.metadata)),
                Err(err) => Err(format!(
                    "scheduling {} on {} failed: {:?}",
                    method, target_str, err
                )),
            }
        }
        Err(err) => Err(err),
    };

    rpc_result(result)
}

/// The same as `rocFxRpcSchedule`, but the returned handle can cancel the
/// invocation up to its scheduled time (`rocFxRpcCancel`).
#[no_mangle]
pub unsafe extern "C" fn rocFxRpcScheduleCancelable(
    target: RocStr,
    function_name: RocStr,
    payload: RocStr,
    delay_millis: u64,
) -> RocResult<u32, RocStr> {
    let target_str = target.as_str();
    let method = function_name.as_str();
    let payload_str = if payload.is_empty() { "{}" } else { payload.as_str() };

    match rpc_client_for(target_str) {
        Ok(rpc) => {
            let input = single_param_value_tree(payload_str);
            let scheduled_time = scheduled_instant_in(delay_millis);
            match rpc.schedule_cancelable_invocation(scheduled_time, method, input, None) {
                Ok(receipt) => RocResult::ok(handle_store(
                    &CANCELABLE_INVOCATIONS,
                    receipt.cancellation_token,
                )),
                Err(err) => RocResult::err(RocStr::from_str(&format!(
                    "scheduling {} on {} failed: {:?}",
                    method, target_str, err
                ))),
            }
        }
        Err(err) => RocResult::err(RocStr::from_str(&err)),
    }
}

/// Cancels a scheduled invocation that has not run yet. The handle comes from
/// `rocFxRpcScheduleCancelable` and can be used once.
#[no_mangle]
pub unsafe extern "C" fn rocFxRpcCancel(handle: u32) -> RocResult<(), RocStr> {
    match handle_take(&CANCELABLE_INVOCATIONS, handle) {
        Some(token) => {
            token.cancel();
            drop(token);
            RocResult::ok(())
        }
        None => RocResult::err(RocStr::from_str(&format!(
            "unknown cancellation handle {}",
            handle
        ))),
    }
}

/// `parse-agent-id`: an agent id split into the agent type name, the
/// constructor parameter and the (usually absent) phantom id, as JSON.
#[no_mangle]
pub unsafe extern "C" fn rocFxParseAgentId(agent_id: RocStr) -> RocResult<RocStr, RocStr> {
    let id = agent_id.as_str();

    let result = match golem::agent::host::parse_agent_id(id) {
        Ok((agent_type_name, constructor, phantom_id)) => Ok(serde_json::json!({
            "agentType": agent_type_name.as_str(),
            "constructor": extract_input_string(&constructor.value),
            "phantomId": phantom_id.as_ref().map(uuid_string),
        })
        .to_string()),
        Err(err) => Err(format!("invalid agent id {:?}: {:?}", id, err)),
    };

    rpc_result(result)
}

/// `make-agent-id`: the inverse of `rocFxParseAgentId`, for the ids this
/// platform's agents have — one constructor parameter, no phantom id.
#[no_mangle]
pub unsafe extern "C" fn rocFxMakeAgentId(
    agent_type_name: RocStr,
    constructor_param: RocStr,
) -> RocResult<RocStr, RocStr> {
    let type_name = agent_type_name.as_str();
    let input = single_param_value_tree(constructor_param.as_str());

    let result = match golem::agent::host::make_agent_id(type_name, input, None) {
        Ok(agent_id) => Ok(agent_id),
        Err(err) => Err(format!(
            "cannot build an id for agent type {:?}: {:?}",
            type_name, err
        )),
    };

    rpc_result(result)
}

/// The canonical UUID spelling of Golem's two-word `uuid` type.
fn uuid_string(id: &golem::core::types::Uuid) -> String {
    rdbms_json::uuid_text(id.high_bits, id.low_bits)
}

// Webhook effects
//
// A webhook is a promise plus a URL Golem serves: a POST to the URL completes
// the promise with the request body, so the agent hands the URL to an external
// system and awaits the callback later. The promise id never leaves the host —
// the URL is the handle the app stores in its own state between invocations,
// and it is unique per webhook.

thread_local! {
    /// Promise ids of the webhooks this instance created, keyed by the callback
    /// URL `createWebhook!` returned. The map is part of the instance's memory,
    /// so it survives between invocations and comes back with the agent's state
    /// (restored from a snapshot, or rebuilt by replaying `createWebhook!`).
    static WEBHOOK_PROMISES: RefCell<HashMap<String, golem::core::types::PromiseId>> =
        RefCell::new(HashMap::new());
}

fn remember_webhook_promise(url: &str, promise_id: golem::core::types::PromiseId) {
    WEBHOOK_PROMISES.with(|promises| {
        promises.borrow_mut().insert(url.to_string(), promise_id);
    });
}

fn webhook_promise(url: &str) -> Option<golem::core::types::PromiseId> {
    WEBHOOK_PROMISES.with(|promises| promises.borrow().get(url).cloned())
}

/// The `webhook-error` variant rendered for the `Try(Str, Str)` error channel.
fn webhook_error_text(error: golem::agent::host::WebhookError) -> String {
    match error {
        golem::agent::host::WebhookError::PermissionDenied => "permission-denied".to_string(),
        golem::agent::host::WebhookError::InternalError(message) => {
            format!("internal-error: {message}")
        }
    }
}

/// `createWebhook!`: creates a promise, creates the webhook that completes it,
/// and returns the callback URL. Only agent types currently deployed through an
/// HTTP API may create webhooks; Golem reports anything else as the
/// `webhook-error` this effect carries.
#[no_mangle]
pub unsafe extern "C" fn rocFxCreateWebhook() -> RocResult<RocStr, RocStr> {
    let promise_id = golem::api::host::create_promise();
    match golem::agent::host::create_webhook(&promise_id) {
        Ok(url) => {
            remember_webhook_promise(&url, promise_id);
            RocResult::ok(RocStr::from_str(&url))
        }
        Err(err) => RocResult::err(RocStr::from_str(&webhook_error_text(err))),
    }
}

/// `awaitWebhook!`: waits for the POST that completes the URL's promise and
/// returns the body as lossy UTF-8, the convention `http!` uses for bytes. The
/// URL comes from `createWebhook!` in an earlier invocation (or a replay of
/// one); a URL this instance never created is an error, not a hang.
#[no_mangle]
pub unsafe extern "C" fn rocFxAwaitWebhook(url: RocStr) -> RocResult<RocStr, RocStr> {
    let url_str = url.as_str();
    let Some(promise_id) = webhook_promise(url_str) else {
        return RocResult::err(RocStr::from_str(&format!(
            "no webhook was created for URL {url_str:?}"
        )));
    };

    // `get` is an async host import (like `websocketReceive!`), so the
    // synchronous effect drives it with the fork's `block_on`.
    let result = golem::api::host::get_promise(&promise_id);
    let body = wit_bindgen::block_on(result.get());
    let text = String::from_utf8_lossy(&body);
    RocResult::ok(RocStr::from_str(&text))
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
pub unsafe extern "C" fn rocFxHttpRequest(req_json: RocStr) -> RocResult<RocStr, RocStr> {
    let req_str = if req_json.is_empty() { "{}" } else { req_json.as_str() };
    match execute_http_request(req_str) {
        Ok(resp_json) => RocResult::ok(RocStr::from_str(&resp_json)),
        Err(err) => RocResult::err(RocStr::from_str(&err)),
    }
}

// WebSocket Effect C-ABI

fn ws_message_text(message: golem::websocket::client::Message) -> String {
    match message {
        golem::websocket::client::Message::Text(text) => text,
        golem::websocket::client::Message::Binary(bytes) => String::from_utf8_lossy(&bytes).into(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxWsConnect(url: RocStr) -> RocResult<u32, RocStr> {
    let url_str = url.as_str();
    // Golem 1.6.0 added a third parameter (the connection's reconstruction
    // policy); `None` keeps the default, which reconnects automatically.
    match golem::websocket::client::WebsocketConnection::connect(url_str, None, None) {
        Ok(connection) => RocResult::ok(handle_store(&WS_CONNECTIONS, connection)),
        Err(err) => RocResult::err(RocStr::from_str(&format!("{:?}", err))),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxWsSend(handle: u32, msg: RocStr) -> RocResult<(), RocStr> {
    let text = msg.as_str();
    let sent = handle_with(&WS_CONNECTIONS, handle, |connection| {
        connection.send(&golem::websocket::client::Message::Text(String::from(text)))
    });
    match sent {
        Some(Ok(())) => RocResult::ok(()),
        Some(Err(err)) => RocResult::err(RocStr::from_str(&format!("{:?}", err))),
        None => RocResult::err(RocStr::from_str(&format!(
            "unknown websocket handle {}",
            handle
        ))),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxWsReceive(handle: u32) -> RocResult<RocStr, RocStr> {
    // `receive` became an async host import in Golem 1.6. The Roc side calls
    // this effect synchronously, so the fork's `block_on` drives the async call
    // to completion (it waits on the guest's waitable set while the host call is
    // in flight).
    let received = handle_with(&WS_CONNECTIONS, handle, |connection| {
        wit_bindgen::block_on(connection.receive())
    });
    match received {
        Some(Ok(message)) => {
            let text = ws_message_text(message);
            RocResult::ok(RocStr::from_str(&text))
        }
        Some(Err(err)) => RocResult::err(RocStr::from_str(&format!("{:?}", err))),
        None => RocResult::err(RocStr::from_str(&format!(
            "unknown websocket handle {}",
            handle
        ))),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rocFxWsClose(handle: u32) -> RocResult<(), RocStr> {
    match handle_take(&WS_CONNECTIONS, handle) {
        Some(connection) => {
            let _ = connection.close(None, None);
            drop(connection);
            RocResult::ok(())
        }
        None => RocResult::err(RocStr::from_str(&format!(
            "unknown websocket handle {}",
            handle
        ))),
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
pub unsafe extern "C" fn rocFxGetConfig(key: RocStr) -> RocResult<RocStr, RocStr> {
    let key_str = key.as_str();
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
        Ok(text) => RocResult::ok(RocStr::from_str(&text)),
        Err(err) => RocResult::err(RocStr::from_str(&err)),
    }
}

// Quota effects
//
// `golem:quota/types` is a synchronous host interface. `newQuotaToken!` asks the
// host for a capability on a resource the manifest declares, `quotaReserve!`
// waits for capacity (or returns the rejection the resource's enforcement
// action produced), and `quotaCommit!` reports what was actually used — a
// reservation that is never committed counts as zero usage. Tokens and
// reservations are host-side resources, so the app holds them as `U32` handles
// in the same kind of table the WebSocket effects use.

thread_local! {
    static QUOTA_TOKENS: RefCell<Vec<Option<golem::core::types::QuotaToken>>> =
        const { RefCell::new(Vec::new()) };
    static QUOTA_RESERVATIONS: RefCell<Vec<Option<golem::quota::types::Reservation>>> =
        const { RefCell::new(Vec::new()) };
}

/// The `failed-reservation` of a `reject`ing resource, rendered for the effect's
/// error channel. The host's estimate of how long the caller would have to wait
/// for capacity comes along when it has one (only rate limits can predict one).
fn failed_reservation_text(failure: &golem::quota::types::FailedReservation) -> String {
    match failure.estimated_wait_nanos {
        Some(nanos) => format!(
            "quota reservation failed (retry after {:?})",
            core::time::Duration::from_nanos(nanos)
        ),
        None => String::from("quota reservation failed"),
    }
}

/// The handle of a quota token, for the resource the manifest declares under
/// `resourceDefaults.<env>.<name>`.
#[no_mangle]
pub unsafe extern "C" fn rocFxNewQuotaToken(resource_name: RocStr, expected_use: u64) -> u32 {
    // `resource_name` is owned by this function and released when it drops.
    let name = resource_name.as_str();
    handle_store(
        &QUOTA_TOKENS,
        golem::quota::types::new_token(name, expected_use),
    )
}

/// Reserves `amount` from the token's allocation: `Ok` is the handle of a
/// reservation to commit (or drop, which commits zero), `Err` is the rejection a
/// `reject`ing resource answered with. A `throttle` resource waits here instead,
/// and a `terminate` one traps the invocation.
#[no_mangle]
pub unsafe extern "C" fn rocFxQuotaReserve(handle: u32, amount: u64) -> RocResult<u32, RocStr> {
    let reserved = handle_with(&QUOTA_TOKENS, handle, |token| {
        golem::quota::types::reserve(token, amount)
    });

    match reserved {
        Some(Ok(reservation)) => RocResult::ok(handle_store(&QUOTA_RESERVATIONS, reservation)),
        Some(Err(failure)) => RocResult::err(RocStr::from_str(&failed_reservation_text(&failure))),
        None => RocResult::err(RocStr::from_str(&format!(
            "unknown quota token handle {handle}"
        ))),
    }
}

/// Commits what the reserved work actually used; a handle is spent once. A
/// `Try` because an unknown or already-committed handle is the app's mistake to
/// see, the way `wsClose!` and `rpcCancel!` report theirs.
#[no_mangle]
pub unsafe extern "C" fn rocFxQuotaCommit(handle: u32, used: u64) -> RocResult<(), RocStr> {
    match handle_take(&QUOTA_RESERVATIONS, handle) {
        Some(reservation) => {
            golem::quota::types::Reservation::commit(reservation, used);
            RocResult::ok(())
        }
        None => RocResult::err(RocStr::from_str(&format!(
            "unknown quota reservation handle {handle}"
        ))),
    }
}

/// Splits `child_expected_use` off the token into a new one. The host traps if
/// the child would exceed the parent's expected use.
#[no_mangle]
pub unsafe extern "C" fn rocFxQuotaSplit(
    handle: u32,
    child_expected_use: u64,
) -> RocResult<u32, RocStr> {
    let child = handle_with(&QUOTA_TOKENS, handle, |token| {
        golem::quota::types::split(token, child_expected_use)
    });

    match child {
        Some(token) => RocResult::ok(handle_store(&QUOTA_TOKENS, token)),
        None => RocResult::err(RocStr::from_str(&format!(
            "unknown quota token handle {handle}"
        ))),
    }
}

/// Merges `other` into the token: `other` is consumed, and the host traps if the
/// two tokens name different resources.
#[no_mangle]
pub unsafe extern "C" fn rocFxQuotaMerge(handle: u32, other: u32) -> RocResult<(), RocStr> {
    let unknown = |handle: u32| {
        RocResult::err(RocStr::from_str(&format!(
            "unknown quota token handle {handle}"
        )))
    };

    // `merge` borrows the receiver and takes the other token, so both handles are
    // checked before either is consumed (merging a token with itself would take
    // the receiver away under the call).
    if handle == other {
        return RocResult::err(RocStr::from_str("cannot merge a quota token with itself"));
    }
    if handle_with(&QUOTA_TOKENS, handle, |_| ()).is_none() {
        return unknown(handle);
    }
    let Some(other_token) = handle_take(&QUOTA_TOKENS, other) else {
        return unknown(other);
    };

    handle_with(&QUOTA_TOKENS, handle, |token| {
        golem::quota::types::merge(token, other_token)
    });
    RocResult::ok(())
}

// Retry-policy effects
//
// `golem:api/retry` is synchronous too. Policies and predicates are flattened
// trees, which Roc can only see as text: `retry_json` renders them as JSON and
// parses the same shape back, and the effects below are the thin shell around
// that (`getRetryPolicies!` hands over an array of named policies). A policy the
// app reads can be fed straight back into `setRetryPolicy!` or
// `nextRetryDelay!`.

/// Every retry policy active for this agent, as a JSON array of named policies.
#[no_mangle]
pub unsafe extern "C" fn rocFxGetRetryPolicies() -> RocStr {
    RocStr::from_str(&retry_json::policies_to_json(
        &golem::api::retry::get_retry_policies(),
    ))
}

/// One named policy as JSON, or the empty string when no policy has that name
/// (the host answers with an `option`, and `resolveRetryPolicy!` reports "none"
/// the same way).
#[no_mangle]
pub unsafe extern "C" fn rocFxGetRetryPolicy(name: RocStr) -> RocResult<RocStr, RocStr> {
    let name_str = name.as_str();
    match golem::api::retry::get_retry_policy_by_name(name_str) {
        Some(policy) => RocResult::ok(RocStr::from_str(&retry_json::to_json(
            &retry_json::NamedPolicyJson::from(&policy),
        ))),
        None => RocResult::ok(RocStr::from_str("")),
    }
}

/// The first named policy whose predicate matches the context, as JSON — or the
/// empty string when none does. The host reports a predicate it could not
/// evaluate as an error, without trapping the invocation.
#[no_mangle]
pub unsafe extern "C" fn rocFxResolveRetryPolicy(
    verb: RocStr,
    noun_uri: RocStr,
    properties_json: RocStr,
) -> RocResult<RocStr, RocStr> {
    let properties = match retry_json::properties_from_json(properties_json.as_str()) {
        Ok(properties) => properties,
        Err(err) => return RocResult::err(RocStr::from_str(&err)),
    };

    let resolved =
        golem::api::retry::resolve_retry_policy(verb.as_str(), noun_uri.as_str(), &properties);
    match resolved {
        Ok(Some(policy)) => RocResult::ok(RocStr::from_str(&retry_json::to_json(
            &retry_json::PolicyJson::from(&policy),
        ))),
        Ok(None) => RocResult::ok(RocStr::from_str("")),
        Err(err) => RocResult::err(RocStr::from_str(&format!(
            "resolving a retry policy failed: {err}"
        ))),
    }
}

/// Advances the host's retry evaluator once: `policy_json` is a policy (empty
/// takes the agent's configured default), `state_json` the continuation the
/// previous decision returned (`[]` starts a sequence), `elapsed_nanos` the time
/// since the first attempt, and `context_json` `{verb, nounUri, properties}`.
/// The answer is `{state, delayNanos}`, with `null` for an exhausted policy.
#[no_mangle]
pub unsafe extern "C" fn rocFxNextRetryDelay(
    policy_json: RocStr,
    state_json: RocStr,
    elapsed_nanos: u64,
    context_json: RocStr,
) -> RocResult<RocStr, RocStr> {
    let policy = if policy_json.is_empty() {
        None
    } else {
        match retry_json::policy_from_json(policy_json.as_str()) {
            Ok(policy) => Some(policy),
            Err(err) => return RocResult::err(RocStr::from_str(&err)),
        }
    };
    let state = match retry_json::state_from_json(state_json.as_str()) {
        Ok(state) => state,
        Err(err) => return RocResult::err(RocStr::from_str(&err)),
    };
    let context = match retry_json::decision_context_from_json(context_json.as_str()) {
        Ok(context) => context,
        Err(err) => return RocResult::err(RocStr::from_str(&err)),
    };

    // A changed policy or a continuation the host cannot read comes back as an
    // error here, never as a reset retry budget.
    let decision = golem::api::retry::next_retry_delay(
        policy.as_ref(),
        &state,
        elapsed_nanos,
        &context.verb,
        &context.noun_uri,
        &context.properties,
    );
    match decision {
        Ok(decision) => RocResult::ok(RocStr::from_str(&retry_json::decision_to_json(&decision))),
        Err(err) => RocResult::err(RocStr::from_str(&format!(
            "the next retry delay could not be computed: {err}"
        ))),
    }
}

/// Adds or overwrites a named policy (the host records it in the agent's oplog,
/// so it survives a restart). The argument is the JSON `getRetryPolicies!`
/// prints.
#[no_mangle]
pub unsafe extern "C" fn rocFxSetRetryPolicy(policy_json: RocStr) -> RocResult<(), RocStr> {
    match retry_json::named_policy_from_json(policy_json.as_str()) {
        Ok(policy) => {
            golem::api::retry::set_retry_policy(&policy);
            RocResult::ok(())
        }
        Err(err) => RocResult::err(RocStr::from_str(&err)),
    }
}

/// Removes a named policy by name (recorded in the oplog as well).
#[no_mangle]
pub unsafe extern "C" fn rocFxRemoveRetryPolicy(name: RocStr) {
    golem::api::retry::remove_retry_policy(name.as_str());
}

// Oplog and execution-control effects
//
// `golem:api/host` is synchronous, so these effects call it directly. What
// remains of the 1.5 execution-control surface is small on purpose: Golem 1.6
// removed the guest-controlled oplog markers (`mark-begin-operation`,
// `mark-end-operation`), the oplog cursor (`get-oplog-index`,
// `set-oplog-index`) and the idempotence switch (`get-idempotence-mode`,
// `set-idempotence-mode`) — see the README's "Oplog and execution control".
// What is left is for the app: `trap!` to fail an invocation it must not
// continue, `idempotencyKey!` to make an external write duplicate-safe,
// `fork!` to copy the agent, `oplogCommit!` to wait for the oplog to reach
// replicas, and the two resolvers that turn a component reference into the ids
// `rpc!` works with. The operator-facing operations (`update-agent`,
// `revert-agent`, `get-agent-metadata`) stay on the CLI.

/// `agent-operation-error` rendered for the effect's error channel, the way
/// `webhook_error_text` renders `webhook-error`.
fn agent_operation_error_text(error: golem::api::host::AgentOperationError) -> String {
    match error {
        golem::api::host::AgentOperationError::PermissionDenied => String::from("permission-denied"),
        golem::api::host::AgentOperationError::BackendError(message) => {
            format!("backend-error: {message}")
        }
    }
}

/// A `fork-result` as `"original:<phantom-id>"` / `"forked:<phantom-id>"`: the
/// variant says which side of the fork this agent is, and the phantom id — the
/// same value on both sides — is what the forked copy's agent id carries.
fn fork_result_text(result: golem::api::host::ForkResult) -> String {
    let (side, details) = match result {
        golem::api::host::ForkResult::Original(details) => ("original", details),
        golem::api::host::ForkResult::Forked(details) => ("forked", details),
    };
    format!("{side}:{}", uuid_string(&details.forked_phantom_id))
}

/// An `agent-id` record as JSON. An app that resolved an agent needs both
/// halves: the component id (which the agent id string does not carry) and the
/// agent id string itself.
fn agent_id_json(id: &golem::core::types::AgentId) -> String {
    serde_json::json!({
        "componentId": uuid_string(&id.component_id.uuid),
        "agentId": id.agent_id,
    })
    .to_string()
}

/// What the resolve effects report for a reference that named no component: the
/// app asked for something that does not exist, which is its mistake to see —
/// the way a malformed agent id is for `parseAgentId!`.
fn unresolved_component_text(reference: &str) -> String {
    format!("no component named {reference:?}")
}

/// `trap!`: fails the current invocation with `reason`, and never returns. The
/// host turns the call into a trap, so the runtime reports it as an invocation
/// failure instead of a result the app could branch on; Golem treats a trap as
/// permanent, so a recoverable failure belongs in the app's own error handling.
#[no_mangle]
pub unsafe extern "C" fn rocFxTrap(reason: RocStr) {
    // The argument is owned by this function, and a real server traps inside this
    // call, so execution never returns here and the string is not released — the
    // invocation is over, and the runtime's copy of the reason is what the CLI
    // reports as the failure.
    golem::api::host::trap(reason.as_str());
}

/// `idempotencyKey!`: a durable key for an external write. Golem derives it from
/// the position of this call in the invocation's oplog, so a replay of the
/// invocation hands back the key the failed attempt already used — which is
/// what makes a retried write duplicate-safe at the peer the write goes to. The
/// peer has to enforce the key: an accompanying header alone is not a guarantee.
#[no_mangle]
pub unsafe extern "C" fn rocFxIdempotencyKey() -> RocStr {
    RocStr::from_str(&uuid_string(&golem::api::host::generate_idempotency_key()))
}

/// `fork!`: copies the current agent at this point in the invocation. Both
/// sides continue, and each gets its own `fork-result`; the forked copy runs
/// under the same agent id with the returned phantom id, and the invocation the
/// caller is waiting for is the original's.
#[no_mangle]
pub unsafe extern "C" fn rocFxFork() -> RocResult<RocStr, RocStr> {
    match golem::api::host::fork() {
        Ok(result) => RocResult::ok(RocStr::from_str(&fork_result_text(result))),
        Err(err) => RocResult::err(RocStr::from_str(&agent_operation_error_text(err))),
    }
}

/// `oplogCommit!`: blocks until the oplog is durable on `replicas` replicas (or
/// on all of them, when `replicas` is higher than the deployment has). It is a
/// replication barrier, not a transaction: it commits nothing about an external
/// system, and it undoes nothing either.
#[no_mangle]
pub unsafe extern "C" fn rocFxOplogCommit(replicas: u8) {
    golem::api::host::oplog_commit(replicas);
}

/// `resolveComponentId!`: the component id of a deployed component, looked up by
/// reference (in Golem OSS a component's name, as the manifest declares it).
#[no_mangle]
pub unsafe extern "C" fn rocFxResolveComponentId(
    component_reference: RocStr,
) -> RocResult<RocStr, RocStr> {
    let reference = component_reference.as_str();
    match golem::api::host::resolve_component_id(reference) {
        Some(component_id) => RocResult::ok(RocStr::from_str(&uuid_string(&component_id.uuid))),
        None => RocResult::err(RocStr::from_str(&unresolved_component_text(reference))),
    }
}

/// `resolveAgentId!`: the agent id record for `agent_name` in the referenced
/// component, as JSON (`{"componentId": …, "agentId": …}`). The host resolves
/// the reference and attaches the name; whether that agent exists is what
/// `resolve-agent-id-strict` would check, and this platform does not expose it.
#[no_mangle]
pub unsafe extern "C" fn rocFxResolveAgentId(
    component_reference: RocStr,
    agent_name: RocStr,
) -> RocResult<RocStr, RocStr> {
    let reference = component_reference.as_str();
    match golem::api::host::resolve_agent_id(reference, agent_name.as_str()) {
        Some(agent_id) => RocResult::ok(RocStr::from_str(&agent_id_json(&agent_id))),
        None => RocResult::err(RocStr::from_str(&unresolved_component_text(reference))),
    }
}

// Key-value and blobstore effects
//
// `wasi:keyvalue` and `wasi:blobstore` are the two shared stores Golem gives an
// environment: buckets of key-value pairs, and containers of named objects.
// Both belong to the environment rather than to one agent, both are
// permission-checked and recorded in the agent's oplog by the runtime, and both
// are synchronous interfaces — the blobstore's `list-objects` is the one call
// whose `stream<object-name>` the host has to drain itself.
//
// Values on the wire are bytes; the effects carry them as lossy-UTF-8 text, the
// convention `http!` and `awaitWebhook!` already use for bodies. Buckets, keys,
// containers and objects are named by strings, so no handle reaches the app.

/// What `kvGet!` answers when the bucket holds no such key: a missing key is an
/// ordinary answer rather than an error, and it has to be told apart from an
/// empty value. The value's text comes back verbatim (so an empty value is
/// `""`), and the JSON literal `null` marks the missing case. The one blind
/// spot is a value that is itself the text `null`; `kvExists!` is the check to
/// reach for when that matters.
const MISSING_KEY_TEXT: &str = "null";

/// The text of a `wasi:keyvalue` error: its error type is a resource whose only
/// reader is `trace`.
fn kv_error_text(error: wasi::keyvalue::wasi_keyvalue_error::Error) -> String {
    error.trace()
}

/// Opens the bucket an effect names. Buckets are implicit — the store makes one
/// the first time it is written to — so opening only fails where the store
/// itself refuses.
fn kv_bucket(name: &str) -> Result<wasi::keyvalue::types::Bucket, String> {
    wasi::keyvalue::types::Bucket::open_bucket(name).map_err(kv_error_text)
}

/// The container an object operation names. A container that does not exist is
/// reported as an error by the host (`Container not found`), which is the
/// effect's `Err`.
fn blob_container(name: &str) -> Result<wasi::blobstore::container::Container, String> {
    wasi::blobstore::blobstore::get_container(name)
}

/// The object names of a `list-objects` stream as the JSON array `blobList!`
/// promises: arbitrary text, so the encoder builds it rather than a hand-rolled
/// join.
fn names_json(names: &[String]) -> String {
    serde_json::to_string(names).unwrap_or_else(|_| String::from("[]"))
}

/// The `Try` an effect whose success value is text returns.
fn text_result(result: Result<String, String>) -> RocResult<RocStr, RocStr> {
    match result {
        Ok(text) => RocResult::ok(RocStr::from_str(&text)),
        Err(err) => RocResult::err(RocStr::from_str(&err)),
    }
}

/// The `Try` an effect whose success value is `{}` returns (`Try({}, Str)`).
fn unit_result(result: Result<(), String>) -> RocResult<(), RocStr> {
    match result {
        Ok(()) => RocResult::ok(()),
        Err(err) => RocResult::err(RocStr::from_str(&err)),
    }
}

/// The `Try` of `kvExists!` / `blobHas!`.
fn bool_result(result: Result<bool, String>) -> RocResult<bool, RocStr> {
    match result {
        Ok(value) => RocResult::ok(value),
        Err(err) => RocResult::err(RocStr::from_str(&err)),
    }
}

fn kv_get(bucket: &str, key: &str) -> Result<String, String> {
    let bucket = kv_bucket(bucket)?;
    let Some(value) = wasi::keyvalue::eventual::get(&bucket, key).map_err(kv_error_text)? else {
        return Ok(String::from(MISSING_KEY_TEXT));
    };
    let bytes = value
        .incoming_value_consume_sync()
        .map_err(kv_error_text)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn kv_set(bucket: &str, key: &str, value: &str) -> Result<(), String> {
    let bucket = kv_bucket(bucket)?;
    let outgoing = wasi::keyvalue::types::OutgoingValue::new_outgoing_value();
    outgoing
        .outgoing_value_write_body_sync(value.as_bytes())
        .map_err(kv_error_text)?;
    wasi::keyvalue::eventual::set(&bucket, key, &outgoing).map_err(kv_error_text)
}

fn kv_delete(bucket: &str, key: &str) -> Result<(), String> {
    let bucket = kv_bucket(bucket)?;
    wasi::keyvalue::eventual::delete(&bucket, key).map_err(kv_error_text)
}

fn kv_exists(bucket: &str, key: &str) -> Result<bool, String> {
    let bucket = kv_bucket(bucket)?;
    wasi::keyvalue::eventual::exists(&bucket, key).map_err(kv_error_text)
}

/// The whole object as lossy UTF-8. `get-data` takes an inclusive byte range,
/// so the size has to come from `object-info` first: the whole object is
/// `0..=size-1`, and a zero-length object is read without a range at all (the
/// store rejects an empty range rather than answering with no bytes).
fn blob_get(container: &str, object: &str) -> Result<String, String> {
    let container = blob_container(container)?;
    let size = container.object_info(object)?.size;
    let Some(last_byte) = size.checked_sub(1) else {
        return Ok(String::new());
    };
    let value = container.get_data(object, 0, last_byte)?;
    let bytes = value.incoming_value_consume_sync()?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn blob_put(container: &str, object: &str, value: &str) -> Result<(), String> {
    let container = blob_container(container)?;
    // An outgoing value's body is written through a stream: attaching the
    // stream returns right away and the host reads it while the guest writes
    // (the same shape `http!` uses for a request body), so the bytes have to be
    // in before `write-data` copies them.
    let outgoing = wasi::blobstore::types::OutgoingValue::new_outgoing_value();
    let (mut body_tx, body_rx) = wit_stream::new();
    outgoing
        .outgoing_value_write_body(body_rx)
        .map_err(|()| String::from("the host refused the object body stream"))?;
    let remaining = wit_bindgen::block_on(body_tx.write_all(value.as_bytes().to_vec()));
    drop(body_tx);
    if !remaining.is_empty() {
        return Err(format!(
            "the host did not take the whole object body ({} bytes left)",
            remaining.len()
        ));
    }
    container.write_data(object, &outgoing)
}

fn blob_create_container(name: &str) -> Result<(), String> {
    // The host reads the container back to report its creation time, which the
    // effect does not expose: drop the handle and keep the outcome. Creating a
    // container that is already there is not an error — the store keeps a
    // directory entry, and a second call leaves it alone.
    wasi::blobstore::blobstore::create_container(name).map(|_container| ())
}

fn blob_has(container: &str, object: &str) -> Result<bool, String> {
    let container = blob_container(container)?;
    container.has_object(object)
}

fn blob_list(container: &str) -> Result<String, String> {
    let container = blob_container(container)?;
    // The names arrive as a stream; the effect answers with the JSON array of
    // all of them, so the host drains the stream here.
    let names = wit_bindgen::block_on(container.list_objects()?.collect());
    Ok(names_json(&names))
}

fn blob_delete(container: &str, object: &str) -> Result<(), String> {
    let container = blob_container(container)?;
    container.delete_object(object)
}

/// `kvGet!`: the value at `key` in `bucket` as lossy UTF-8, or the literal
/// `null` when the key is absent (see `MISSING_KEY_TEXT`).
#[no_mangle]
pub unsafe extern "C" fn rocFxKvGet(bucket: RocStr, key: RocStr) -> RocResult<RocStr, RocStr> {
    // Both arguments are owned by this function and released when they drop.
    text_result(kv_get(bucket.as_str(), key.as_str()))
}

/// `kvSet!`: stores `value` (as UTF-8 bytes) at `key` in `bucket`, overwriting
/// whatever was there.
#[no_mangle]
pub unsafe extern "C" fn rocFxKvSet(
    bucket: RocStr,
    key: RocStr,
    value: RocStr,
) -> RocResult<(), RocStr> {
    unit_result(kv_set(bucket.as_str(), key.as_str(), value.as_str()))
}

/// `kvDelete!`: removes `key` from `bucket`; deleting a key that is not there
/// is not an error.
#[no_mangle]
pub unsafe extern "C" fn rocFxKvDelete(bucket: RocStr, key: RocStr) -> RocResult<(), RocStr> {
    unit_result(kv_delete(bucket.as_str(), key.as_str()))
}

/// `kvExists!`: whether `bucket` holds `key` — the direct check that does not
/// need the value read.
#[no_mangle]
pub unsafe extern "C" fn rocFxKvExists(bucket: RocStr, key: RocStr) -> RocResult<bool, RocStr> {
    bool_result(kv_exists(bucket.as_str(), key.as_str()))
}

/// `blobCreateContainer!`: creates the container if it is not there yet.
#[no_mangle]
pub unsafe extern "C" fn rocFxBlobCreateContainer(name: RocStr) -> RocResult<(), RocStr> {
    unit_result(blob_create_container(name.as_str()))
}

/// `blobPut!`: stores `value` (as UTF-8 bytes) as `object` in `container`,
/// replacing an object with that name.
#[no_mangle]
pub unsafe extern "C" fn rocFxBlobPut(
    container: RocStr,
    object: RocStr,
    value: RocStr,
) -> RocResult<(), RocStr> {
    unit_result(blob_put(
        container.as_str(),
        object.as_str(),
        value.as_str(),
    ))
}

/// `blobGet!`: the whole object as lossy UTF-8.
#[no_mangle]
pub unsafe extern "C" fn rocFxBlobGet(
    container: RocStr,
    object: RocStr,
) -> RocResult<RocStr, RocStr> {
    text_result(blob_get(container.as_str(), object.as_str()))
}

/// `blobHas!`: whether `container` holds an object with that name.
#[no_mangle]
pub unsafe extern "C" fn rocFxBlobHas(
    container: RocStr,
    object: RocStr,
) -> RocResult<bool, RocStr> {
    bool_result(blob_has(container.as_str(), object.as_str()))
}

/// `blobList!`: the names in `container` as a JSON array of strings (`[]` for
/// an empty container), in the store's own order — the interface promises none.
#[no_mangle]
pub unsafe extern "C" fn rocFxBlobList(container: RocStr) -> RocResult<RocStr, RocStr> {
    text_result(blob_list(container.as_str()))
}

/// `blobDelete!`: deletes the object; deleting one that is not there is not an
/// error.
#[no_mangle]
pub unsafe extern "C" fn rocFxBlobDelete(
    container: RocStr,
    object: RocStr,
) -> RocResult<(), RocStr> {
    unit_result(blob_delete(container.as_str(), object.as_str()))
}

// The agent's own filesystem effects
//
// `wasi:filesystem@0.2.6` is the interface Golem 1.6 registers for the agent's
// sandbox, next to the p3 revision (`wasi:filesystem@0.3.0`) whose descriptor
// calls are `async func`s over component-model streams. The effects below use
// the p2 shape because it is synchronous: its descriptor operations answer like
// the POSIX calls they mirror, which is what makes a host call of this platform
// a plain call rather than a stream setup, and what lets the ABI harness drive
// the effects with an in-memory stub. A p3 (stream-shaped) effect family is a
// follow-up for whoever needs the async shapes.
//
// The filesystem is the agent's own, not the environment's: the runtime
// provisions it from the manifest's per-agent `files:` list before the agent
// starts, records every write in the invocation's oplog, and replays those
// calls, so what one invocation wrote the next one reads. (HTTP mounts *serve*
// files out of the same filesystem; that is a separate, host-side surface.)
//
// Paths are named the way the sandbox reads: `/seed/a.txt` and `seed/a.txt` are
// the same file — the root is the directory Golem preopens under `/` — `.` and
// `..` normalize inside the root, and a path that would step above it is
// refused before the host is asked (`resolve_agent_path`). Bytes travel as
// lossy-UTF-8 text, the convention `http!` and the two stores use, and no
// descriptor reaches the app: every effect names its file and the host opens it.

/// One positioned `read`'s chunk size. The p2 call answers what the host
/// chooses to hand over (Golem caps one read at 64 KiB), so a whole-file loop
/// asks for this much at a time.
const READ_CHUNK: u64 = 64 * 1024;

/// The preopened root of the agent's own filesystem. Golem preopens the sandbox
/// root twice — under `/` and under `.` — and the effects resolve against it; an
/// agent the runtime started without a filesystem answers no preopen at all,
/// which is reported like any other failure of the effect.
fn filesystem_root() -> Result<Descriptor, String> {
    let mut directories = preopens::get_directories();
    if directories.is_empty() {
        return Err(String::from(
            "the agent has no filesystem (the runtime preopened no directory)",
        ));
    }
    let index = directories
        .iter()
        .position(|(_, guest_path)| guest_path == "/")
        .unwrap_or(0);
    let (root, _) = directories.swap_remove(index);
    Ok(root)
}

/// Resolves an app-facing path against the agent's filesystem root: a leading
/// `/` names the root (the spelling the manifest's `files:` targets use), empty
/// segments and `.` are dropped, and `..` steps back inside the root. A path
/// that would step above the root is refused here, with the app's own path in
/// the message, instead of reaching the host as a sandbox escape.
///
/// The result is relative to the root — the one spelling `open-at` and friends
/// accept — so the empty string is the root itself (`.`, `/`).
fn resolve_agent_path(path: &str) -> Result<String, String> {
    let mut segments: Vec<&str> = Vec::new();
    for segment in path.trim_start_matches('/').split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if segments.pop().is_none() {
                    return Err(format!("path {path} escapes the agent's filesystem root"));
                }
            }
            segment => segments.push(segment),
        }
    }
    Ok(segments.join("/"))
}

/// Reads a whole file through repeated `read(length, offset)` calls.
///
/// A p2 `read` is positioned: each call answers up to `length` bytes from
/// `offset` plus whether *that* chunk reached the end of the file, and a host
/// may cap a chunk (Golem at 64 KiB), so a file longer than one chunk takes the
/// loop. A chunk that is empty without the end flag would spin the loop, so it
/// is reported as the host failure it would be.
fn read_all_chunks(
    mut read: impl FnMut(u64, u64) -> Result<(Vec<u8>, bool), String>,
) -> Result<Vec<u8>, String> {
    let mut contents = Vec::new();
    loop {
        let (chunk, end_of_file) = read(READ_CHUNK, contents.len() as u64)?;
        if chunk.is_empty() && !end_of_file {
            return Err(String::from(
                "the filesystem read answered no bytes without reaching the end of the file",
            ));
        }
        contents.extend_from_slice(&chunk);
        if end_of_file {
            return Ok(contents);
        }
    }
}

/// Writes `bytes` at `offset` through repeated `write(buffer, offset)` calls,
/// advancing by what each one reports (a host may accept fewer bytes than it was
/// handed). A call that accepts nothing would spin the loop, so it is reported
/// as the host failure it would be.
fn write_all_chunks(
    mut write: impl FnMut(&[u8], u64) -> Result<u64, String>,
    bytes: &[u8],
    offset: u64,
) -> Result<(), String> {
    let mut written = 0usize;
    while written < bytes.len() {
        let accepted = write(&bytes[written..], offset + written as u64)?;
        if accepted == 0 {
            return Err(String::from(
                "the filesystem write accepted no bytes",
            ));
        }
        written += accepted as usize;
    }
    Ok(())
}

/// The kind `listDir!` reports for one directory entry's `descriptor-type`: the
/// file kinds the interface distinguishes collapse to `other`, the four names
/// the effect promises.
fn entry_kind_name(kind: DescriptorType) -> &'static str {
    match kind {
        DescriptorType::RegularFile => "file",
        DescriptorType::Directory => "dir",
        DescriptorType::SymbolicLink => "symlink",
        _ => "other",
    }
}

/// Renders a directory listing as the JSON array `listDir!` promises, sorted by
/// name so two listings of an unchanged directory read the same. Names are
/// arbitrary text, so the encoder builds the JSON rather than a hand-rolled
/// join.
fn directory_listing_json(entries: &[(String, DescriptorType)]) -> String {
    let mut entries = entries.to_vec();
    entries.sort_by(|(left, _), (right, _)| left.cmp(right));
    let rendered: Vec<serde_json::Value> = entries
        .into_iter()
        .map(|(name, kind)| serde_json::json!({ "name": name, "type": entry_kind_name(kind) }))
        .collect();
    serde_json::Value::Array(rendered).to_string()
}

/// The text a `wasi:filesystem` failure becomes in an effect's `Err` channel:
/// the WIT name of the `error-code` it answered with (`no-entry`,
/// `not-permitted`, …), the way the store effects report their host errors.
fn filesystem_error_text(code: ErrorCode) -> String {
    match code {
        ErrorCode::Access => "access",
        ErrorCode::WouldBlock => "would-block",
        ErrorCode::Already => "already",
        ErrorCode::BadDescriptor => "bad-descriptor",
        ErrorCode::Busy => "busy",
        ErrorCode::Deadlock => "deadlock",
        ErrorCode::Quota => "quota",
        ErrorCode::Exist => "exist",
        ErrorCode::FileTooLarge => "file-too-large",
        ErrorCode::IllegalByteSequence => "illegal-byte-sequence",
        ErrorCode::InProgress => "in-progress",
        ErrorCode::Interrupted => "interrupted",
        ErrorCode::Invalid => "invalid",
        ErrorCode::Io => "io",
        ErrorCode::IsDirectory => "is-directory",
        ErrorCode::Loop => "loop",
        ErrorCode::TooManyLinks => "too-many-links",
        ErrorCode::MessageSize => "message-size",
        ErrorCode::NameTooLong => "name-too-long",
        ErrorCode::NoDevice => "no-device",
        ErrorCode::NoEntry => "no-entry",
        ErrorCode::NoLock => "no-lock",
        ErrorCode::InsufficientMemory => "insufficient-memory",
        ErrorCode::InsufficientSpace => "insufficient-space",
        ErrorCode::NotDirectory => "not-directory",
        ErrorCode::NotEmpty => "not-empty",
        ErrorCode::NotRecoverable => "not-recoverable",
        ErrorCode::Unsupported => "unsupported",
        ErrorCode::NoTty => "no-tty",
        ErrorCode::NoSuchDevice => "no-such-device",
        ErrorCode::Overflow => "overflow",
        ErrorCode::NotPermitted => "not-permitted",
        ErrorCode::Pipe => "pipe",
        ErrorCode::ReadOnly => "read-only",
        ErrorCode::InvalidSeek => "invalid-seek",
        ErrorCode::TextFileBusy => "text-file-busy",
        ErrorCode::CrossDevice => "cross-device",
    }
    .to_string()
}

/// Opens `path` inside the agent's root with the given open flags, following
/// symlinks (the runtime refuses ones that would leave the sandbox).
fn filesystem_open(
    path: &str,
    open_flags: OpenFlags,
    descriptor_flags: DescriptorFlags,
) -> Result<Descriptor, String> {
    let root = filesystem_root()?;
    let resolved = resolve_agent_path(path)?;
    root.open_at(
        PathFlags::SYMLINK_FOLLOW,
        &resolved,
        open_flags,
        descriptor_flags,
    )
    .map_err(filesystem_error_text)
}

fn read_file(path: &str) -> Result<String, String> {
    let file = filesystem_open(path, OpenFlags::empty(), DescriptorFlags::READ)?;
    let bytes = read_all_chunks(|length, offset| {
        file.read(length, offset).map_err(filesystem_error_text)
    })?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Writes the whole file, creating it if it is not there and truncating what was
/// (the `writeFile!` effect's contract).
fn write_file(path: &str, value: &str) -> Result<(), String> {
    let file = filesystem_open(
        path,
        OpenFlags::CREATE | OpenFlags::TRUNCATE,
        DescriptorFlags::WRITE,
    )?;
    write_all_chunks(
        |buffer, offset| file.write(buffer, offset).map_err(filesystem_error_text),
        value.as_bytes(),
        0,
    )
}

/// Appends to the file, creating it if it is not there. `write` is positioned,
/// so the append reads the size the file has now and writes at its end.
fn append_file(path: &str, value: &str) -> Result<(), String> {
    let file = filesystem_open(path, OpenFlags::CREATE, DescriptorFlags::WRITE)?;
    let size = file.stat().map_err(filesystem_error_text)?.size;
    write_all_chunks(
        |buffer, offset| file.write(buffer, offset).map_err(filesystem_error_text),
        value.as_bytes(),
        size,
    )
}

/// Whether anything is there under `path`. A path that cannot be stat'ed
/// *because nothing is there* is the `false` answer; any other failure (a
/// sandbox escape, an unreadable parent, …) is reported.
fn file_exists(path: &str) -> Result<bool, String> {
    let root = filesystem_root()?;
    let resolved = resolve_agent_path(path)?;
    match root.stat_at(PathFlags::SYMLINK_FOLLOW, &resolved) {
        Ok(_) => Ok(true),
        Err(ErrorCode::NoEntry) => Ok(false),
        Err(code) => Err(filesystem_error_text(code)),
    }
}

fn file_size(path: &str) -> Result<u64, String> {
    let root = filesystem_root()?;
    let resolved = resolve_agent_path(path)?;
    root.stat_at(PathFlags::SYMLINK_FOLLOW, &resolved)
        .map(|stat| stat.size)
        .map_err(filesystem_error_text)
}

/// Removes the file at `path`; a directory is refused (`is-directory`), the way
/// `unlink-file-at` refuses it.
fn delete_file(path: &str) -> Result<(), String> {
    let root = filesystem_root()?;
    let resolved = resolve_agent_path(path)?;
    root.unlink_file_at(&resolved)
        .map_err(filesystem_error_text)
}

/// Creates the one directory `path` names; its parent has to exist, so the
/// effect is `mkdir`, not `mkdir -p`.
fn make_dir(path: &str) -> Result<(), String> {
    let root = filesystem_root()?;
    let resolved = resolve_agent_path(path)?;
    root.create_directory_at(&resolved)
        .map_err(filesystem_error_text)
}

/// The entries of the directory `path` names, as the JSON array `listDir!`
/// promises. The p2 stream is a resource the host reads entry by entry until it
/// answers `none`; each entry answers with its type, which is what the JSON
/// reports (folded to the four names the effect promises).
fn list_dir(path: &str) -> Result<String, String> {
    let directory = filesystem_open(path, OpenFlags::DIRECTORY, DescriptorFlags::READ)?;
    let stream = directory
        .read_directory()
        .map_err(filesystem_error_text)?;
    let mut entries = Vec::new();
    loop {
        match stream
            .read_directory_entry()
            .map_err(filesystem_error_text)?
        {
            Some(entry) => entries.push((entry.name, entry.type_)),
            None => break,
        }
    }
    Ok(directory_listing_json(&entries))
}

/// `readFile!`: the whole file as lossy UTF-8 text.
#[no_mangle]
pub unsafe extern "C" fn rocFxReadFile(path: RocStr) -> RocResult<RocStr, RocStr> {
    text_result(read_file(path.as_str()))
}

/// `writeFile!`: creates the file (truncating what was there) and writes the
/// text's UTF-8 bytes.
#[no_mangle]
pub unsafe extern "C" fn rocFxWriteFile(path: RocStr, value: RocStr) -> RocResult<(), RocStr> {
    unit_result(write_file(path.as_str(), value.as_str()))
}

/// `appendFile!`: writes the text's bytes at the end of the file, creating it
/// when it is not there yet.
#[no_mangle]
pub unsafe extern "C" fn rocFxAppendFile(path: RocStr, value: RocStr) -> RocResult<(), RocStr> {
    unit_result(append_file(path.as_str(), value.as_str()))
}

/// `fileExists!`: whether anything is there under the path.
#[no_mangle]
pub unsafe extern "C" fn rocFxFileExists(path: RocStr) -> RocResult<bool, RocStr> {
    bool_result(file_exists(path.as_str()))
}

/// `fileSize!`: the size in bytes.
#[no_mangle]
pub unsafe extern "C" fn rocFxFileSize(path: RocStr) -> RocU64Result {
    match file_size(path.as_str()) {
        Ok(size) => RocU64Result::ok(size),
        Err(err) => RocU64Result::err(RocStr::from_str(&err)),
    }
}

/// `deleteFile!`: removes the file. Unlike the store effects' deletes this is
/// not idempotent — `unlink-file-at` answers `no-entry` for a path that is not
/// there, and the effect reports it — so an app that wants idempotence checks
/// `fileExists!` first.
#[no_mangle]
pub unsafe extern "C" fn rocFxDeleteFile(path: RocStr) -> RocResult<(), RocStr> {
    unit_result(delete_file(path.as_str()))
}

/// `makeDir!`: creates the directory; the parent has to exist.
#[no_mangle]
pub unsafe extern "C" fn rocFxMakeDir(path: RocStr) -> RocResult<(), RocStr> {
    unit_result(make_dir(path.as_str()))
}

/// `listDir!`: the directory's entries as a JSON array of `{name, type}`.
#[no_mangle]
pub unsafe extern "C" fn rocFxListDir(path: RocStr) -> RocResult<RocStr, RocStr> {
    text_result(list_dir(path.as_str()))
}

// Database effects (`golem:rdbms`)
//
// Golem gives an agent three database interfaces, one per engine — postgres,
// MySQL and Apache Ignite 2.x — and all three are synchronous: a connection
// opens, a statement runs and a transaction commits with plain host calls (no
// `block_on`), which is why the ABI harness can drive them against recording
// stubs. `query-stream` is part of every one of the interfaces but not exposed
// yet: a result stream is a resource an app would have to read chunk by chunk,
// and this platform hands an app text.
//
// A connection and a transaction are host-side resources; the app carries them
// as `U32` handles in one table, tagged by engine and kind, so a query can run
// on either and a handle of the wrong kind is reported rather than misread.
// Handles are positional (the first connection is 1), so replaying an invocation
// hands out the same numbers again.
//
// Statements' values travel as the JSON mirror in `rdbms_json`: the params are a
// JSON array of `db-value` encodings, and a result is
// `{"columns": […], "rows": […]}`. An effect's `Err` is the interface's `error`
// variant rendered as its WIT case name plus the host's message
// (`connection-failure: …`), the way `webhook-error` and `agent-operation-error`
// are rendered.

thread_local! {
    /// The database connections and transactions this instance holds, in the
    /// order they were handed out: handle N is the entry at index N-1.
    static DB_HANDLES: RefCell<Vec<Option<DbHandle>>> = const { RefCell::new(Vec::new()) };
}

enum DbHandle {
    Connection(DbConnectionHandle),
    Transaction(DbTransactionHandle),
}

enum DbConnectionHandle {
    Postgres(golem::rdbms::postgres::DbConnection),
    Mysql(golem::rdbms::mysql::DbConnection),
    Ignite(golem::rdbms::ignite2::DbConnection),
}

enum DbTransactionHandle {
    Postgres(golem::rdbms::postgres::DbTransaction),
    Mysql(golem::rdbms::mysql::DbTransaction),
    Ignite(golem::rdbms::ignite2::DbTransaction),
}

/// The `error` variant of `golem:rdbms/postgres` for an effect's `Err` channel.
fn postgres_db_error_text(error: golem::rdbms::postgres::Error) -> String {
    use golem::rdbms::postgres::Error;
    match error {
        Error::ConnectionFailure(message) => format!("connection-failure: {message}"),
        Error::QueryParameterFailure(message) => format!("query-parameter-failure: {message}"),
        Error::QueryExecutionFailure(message) => format!("query-execution-failure: {message}"),
        Error::QueryResponseFailure(message) => format!("query-response-failure: {message}"),
        Error::Other(message) => format!("other: {message}"),
        Error::ResourceLost(message) => format!("resource-lost: {message}"),
    }
}

/// The `error` variant of `golem:rdbms/mysql` for an effect's `Err` channel.
fn mysql_db_error_text(error: golem::rdbms::mysql::Error) -> String {
    use golem::rdbms::mysql::Error;
    match error {
        Error::ConnectionFailure(message) => format!("connection-failure: {message}"),
        Error::QueryParameterFailure(message) => format!("query-parameter-failure: {message}"),
        Error::QueryExecutionFailure(message) => format!("query-execution-failure: {message}"),
        Error::QueryResponseFailure(message) => format!("query-response-failure: {message}"),
        Error::Other(message) => format!("other: {message}"),
        Error::ResourceLost(message) => format!("resource-lost: {message}"),
    }
}

/// The `error` variant of `golem:rdbms/ignite2` for an effect's `Err` channel.
fn ignite_db_error_text(error: golem::rdbms::ignite2::Error) -> String {
    use golem::rdbms::ignite2::Error;
    match error {
        Error::ConnectionFailure(message) => format!("connection-failure: {message}"),
        Error::QueryParameterFailure(message) => format!("query-parameter-failure: {message}"),
        Error::QueryExecutionFailure(message) => format!("query-execution-failure: {message}"),
        Error::QueryResponseFailure(message) => format!("query-response-failure: {message}"),
        Error::Other(message) => format!("other: {message}"),
        Error::ResourceLost(message) => format!("resource-lost: {message}"),
    }
}

fn unknown_db_handle(handle: u32) -> String {
    format!("unknown database handle {handle}")
}

fn db_query(handle: u32, statement: &str, params: &str) -> Result<String, String> {
    let params = rdbms_json::params_array(params)?;
    handle_with(&DB_HANDLES, handle, |entry| match entry {
        DbHandle::Connection(connection) => db_connection_query(connection, statement, &params),
        DbHandle::Transaction(transaction) => db_transaction_query(transaction, statement, &params),
    })
    .unwrap_or_else(|| Err(unknown_db_handle(handle)))
}

fn db_connection_query(
    connection: &DbConnectionHandle,
    statement: &str,
    params: &[serde_json::Value],
) -> Result<String, String> {
    match connection {
        DbConnectionHandle::Postgres(connection) => {
            let params = rdbms_json::postgres_params(params)?;
            connection
                .query(statement, params)
                .map(|result| rdbms_json::postgres_result(&result))
                .map_err(postgres_db_error_text)
        }
        DbConnectionHandle::Mysql(connection) => {
            let params = rdbms_json::mysql_params(params)?;
            connection
                .query(statement, &params)
                .map(|result| rdbms_json::mysql_result(&result))
                .map_err(mysql_db_error_text)
        }
        DbConnectionHandle::Ignite(connection) => {
            let params = rdbms_json::ignite_params(params)?;
            connection
                .query(statement, &params)
                .map(|result| rdbms_json::ignite_result(&result))
                .map_err(ignite_db_error_text)
        }
    }
}

fn db_transaction_query(
    transaction: &DbTransactionHandle,
    statement: &str,
    params: &[serde_json::Value],
) -> Result<String, String> {
    match transaction {
        DbTransactionHandle::Postgres(transaction) => {
            let params = rdbms_json::postgres_params(params)?;
            transaction
                .query(statement, params)
                .map(|result| rdbms_json::postgres_result(&result))
                .map_err(postgres_db_error_text)
        }
        DbTransactionHandle::Mysql(transaction) => {
            let params = rdbms_json::mysql_params(params)?;
            transaction
                .query(statement, &params)
                .map(|result| rdbms_json::mysql_result(&result))
                .map_err(mysql_db_error_text)
        }
        DbTransactionHandle::Ignite(transaction) => {
            let params = rdbms_json::ignite_params(params)?;
            transaction
                .query(statement, &params)
                .map(|result| rdbms_json::ignite_result(&result))
                .map_err(ignite_db_error_text)
        }
    }
}

fn db_execute(handle: u32, statement: &str, params: &str) -> Result<u64, String> {
    let params = rdbms_json::params_array(params)?;
    handle_with(&DB_HANDLES, handle, |entry| match entry {
        DbHandle::Connection(connection) => db_connection_execute(connection, statement, &params),
        DbHandle::Transaction(transaction) => db_transaction_execute(transaction, statement, &params),
    })
    .unwrap_or_else(|| Err(unknown_db_handle(handle)))
}

fn db_connection_execute(
    connection: &DbConnectionHandle,
    statement: &str,
    params: &[serde_json::Value],
) -> Result<u64, String> {
    match connection {
        DbConnectionHandle::Postgres(connection) => {
            let params = rdbms_json::postgres_params(params)?;
            connection.execute(statement, params).map_err(postgres_db_error_text)
        }
        DbConnectionHandle::Mysql(connection) => {
            let params = rdbms_json::mysql_params(params)?;
            connection.execute(statement, &params).map_err(mysql_db_error_text)
        }
        DbConnectionHandle::Ignite(connection) => {
            let params = rdbms_json::ignite_params(params)?;
            ignite_row_count(connection.execute(statement, &params).map_err(ignite_db_error_text)?)
        }
    }
}

fn db_transaction_execute(
    transaction: &DbTransactionHandle,
    statement: &str,
    params: &[serde_json::Value],
) -> Result<u64, String> {
    match transaction {
        DbTransactionHandle::Postgres(transaction) => {
            let params = rdbms_json::postgres_params(params)?;
            transaction.execute(statement, params).map_err(postgres_db_error_text)
        }
        DbTransactionHandle::Mysql(transaction) => {
            let params = rdbms_json::mysql_params(params)?;
            transaction.execute(statement, &params).map_err(mysql_db_error_text)
        }
        DbTransactionHandle::Ignite(transaction) => {
            let params = rdbms_json::ignite_params(params)?;
            ignite_row_count(transaction.execute(statement, &params).map_err(ignite_db_error_text)?)
        }
    }
}

/// Ignite's `execute` answers an `s64` where the other two answer a `u64`; an
/// affected-row count is never negative, so a negative one is a host failure
/// rather than a count to hand on.
fn ignite_row_count(count: i64) -> Result<u64, String> {
    u64::try_from(count)
        .map_err(|_| format!("the host reported {count} affected rows"))
}

fn db_begin(handle: u32) -> Result<u32, String> {
    // The connection is looked up and the transaction stored under one borrow of
    // the table: a `borrow_mut` while the lookup's borrow is alive would panic.
    DB_HANDLES.with(|slots| {
        let mut slots = slots.borrow_mut();
        if handle == 0 {
            return Err(unknown_db_handle(handle));
        }
        let Some(Some(entry)) = slots.get((handle - 1) as usize) else {
            return Err(unknown_db_handle(handle));
        };
        let transaction = match entry {
            DbHandle::Connection(connection) => db_connection_begin(connection)?,
            DbHandle::Transaction(_) => {
                return Err(format!(
                    "database handle {handle} is a transaction, not a connection"
                ))
            }
        };
        Ok(slot_store(&mut slots, DbHandle::Transaction(transaction)))
    })
}

/// Starts a transaction on the connection. It only reads the connection, so the
/// table's borrow is not re-entered here.
fn db_connection_begin(connection: &DbConnectionHandle) -> Result<DbTransactionHandle, String> {
    match connection {
        DbConnectionHandle::Postgres(connection) => connection
            .begin_transaction()
            .map(DbTransactionHandle::Postgres)
            .map_err(postgres_db_error_text),
        DbConnectionHandle::Mysql(connection) => connection
            .begin_transaction()
            .map(DbTransactionHandle::Mysql)
            .map_err(mysql_db_error_text),
        DbConnectionHandle::Ignite(connection) => connection
            .begin_transaction()
            .map(DbTransactionHandle::Ignite)
            .map_err(ignite_db_error_text),
    }
}

/// `dbCommit!` and `dbRollback!`: both consume the transaction handle, which is
/// why one helper serves the two effects.
fn db_finish_transaction(handle: u32, commit: bool) -> Result<(), String> {
    let Some(entry) = handle_take(&DB_HANDLES, handle) else {
        return Err(unknown_db_handle(handle));
    };
    let DbHandle::Transaction(transaction) = entry else {
        return Err(format!(
            "database handle {handle} is a connection, not a transaction"
        ));
    };
    match transaction {
        DbTransactionHandle::Postgres(transaction) => {
            let outcome = if commit {
                transaction.commit()
            } else {
                transaction.rollback()
            };
            outcome.map_err(postgres_db_error_text)
        }
        DbTransactionHandle::Mysql(transaction) => {
            let outcome = if commit {
                transaction.commit()
            } else {
                transaction.rollback()
            };
            outcome.map_err(mysql_db_error_text)
        }
        DbTransactionHandle::Ignite(transaction) => {
            let outcome = if commit {
                transaction.commit()
            } else {
                transaction.rollback()
            };
            outcome.map_err(ignite_db_error_text)
        }
    }
}

/// `dbOpen!`: opens a postgres connection. The address is
/// `postgresql://user[:password]@host[:port]/database`; Golem parses it when the
/// call is made and creates the pool on the first statement, so an address that
/// parses is `Ok` even with no server listening, while one that does not is
/// `Err` with `connection-failure`.
#[no_mangle]
pub unsafe extern "C" fn rocFxDbOpen(address: RocStr) -> RocResult<u32, RocStr> {
    // `address` is owned by this function and released when it drops.
    match golem::rdbms::postgres::DbConnection::open(address.as_str()) {
        Ok(connection) => RocResult::ok(handle_store(
            &DB_HANDLES,
            DbHandle::Connection(DbConnectionHandle::Postgres(connection)),
        )),
        Err(error) => RocResult::err(RocStr::from_str(&postgres_db_error_text(error))),
    }
}

/// `dbOpenMysql!`: opens a MySQL connection. The address is
/// `mysql://user[:password]@host[:port]/database`.
#[no_mangle]
pub unsafe extern "C" fn rocFxDbOpenMysql(address: RocStr) -> RocResult<u32, RocStr> {
    match golem::rdbms::mysql::DbConnection::open(address.as_str()) {
        Ok(connection) => RocResult::ok(handle_store(
            &DB_HANDLES,
            DbHandle::Connection(DbConnectionHandle::Mysql(connection)),
        )),
        Err(error) => RocResult::err(RocStr::from_str(&mysql_db_error_text(error))),
    }
}

/// `dbOpenIgnite!`: opens an Apache Ignite 2.x connection. The address is
/// `ignite://[user:password@]host[:port][?pool_size=N&tls=true]` (default port
/// 10800).
#[no_mangle]
pub unsafe extern "C" fn rocFxDbOpenIgnite(address: RocStr) -> RocResult<u32, RocStr> {
    match golem::rdbms::ignite2::DbConnection::open(address.as_str()) {
        Ok(connection) => RocResult::ok(handle_store(
            &DB_HANDLES,
            DbHandle::Connection(DbConnectionHandle::Ignite(connection)),
        )),
        Err(error) => RocResult::err(RocStr::from_str(&ignite_db_error_text(error))),
    }
}

/// `dbQuery!`: runs a statement and returns its result as JSON
/// (`{"columns": […], "rows": […]}`, see `rdbms_json`). `params` is a JSON
/// array of `db-value` encodings in the engine's own vocabulary; the handle is
/// a connection or a transaction from `dbOpen!` / `dbBegin!`.
#[no_mangle]
pub unsafe extern "C" fn rocFxDbQuery(
    handle: u32,
    statement: RocStr,
    params: RocStr,
) -> RocResult<RocStr, RocStr> {
    text_result(db_query(handle, statement.as_str(), params.as_str()))
}

/// `dbExecute!`: runs a statement and returns the affected-row count.
#[no_mangle]
pub unsafe extern "C" fn rocFxDbExecute(
    handle: u32,
    statement: RocStr,
    params: RocStr,
) -> RocU64Result {
    match db_execute(handle, statement.as_str(), params.as_str()) {
        Ok(count) => RocU64Result::ok(count),
        Err(err) => RocU64Result::err(RocStr::from_str(&err)),
    }
}

/// `dbBegin!`: starts a transaction on the connection and returns its handle.
#[no_mangle]
pub unsafe extern "C" fn rocFxDbBegin(handle: u32) -> RocResult<u32, RocStr> {
    match db_begin(handle) {
        Ok(transaction) => RocResult::ok(transaction),
        Err(err) => RocResult::err(RocStr::from_str(&err)),
    }
}

/// `dbCommit!`: commits the transaction and consumes its handle.
#[no_mangle]
pub unsafe extern "C" fn rocFxDbCommit(handle: u32) -> RocResult<(), RocStr> {
    unit_result(db_finish_transaction(handle, true))
}

/// `dbRollback!`: rolls the transaction back and consumes its handle.
#[no_mangle]
pub unsafe extern "C" fn rocFxDbRollback(handle: u32) -> RocResult<(), RocStr> {
    unit_result(db_finish_transaction(handle, false))
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
            // Tool invocation through the agent's own method surface: no
            // streams are attached here, so the app sees empty stdin and its
            // stdout/stderr text has nowhere to go (only the result payload
            // travels).
            match run_tool_call(&method_name, &input_str, "") {
                Ok(outcome) => Ok(Some(single_string_value_tree(&outcome.output))),
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

/// One tool invocation's answer from the app: the result payload, plus the
/// text the app wrote to the tool's stdout/stderr (`""` when it wrote
/// nothing). The `golem:tool/guest` surface sends the two through the writers
/// the runtime attached; the agent method surface has no writers, so it only
/// reports the payload.
struct ToolOutcome {
    output: String,
    stdout: String,
    stderr: String,
}

/// Runs the app's `handleToolCall!` hook for one tool invocation and returns
/// its answer, updating the persisted state as a side effect. `stdin` is the
/// text the caller piped into the tool (`""` for a call that carried none).
/// Shared by the agent method surface and the `golem:tool/guest` surface.
fn run_tool_call(tool_name: &str, args: &str, stdin: &str) -> Result<ToolOutcome, AgentError> {
    let state_str = get_current_state();
    let tool_call_json = serde_json::json!({
        "id": "call-1",
        "name": tool_name,
        "arguments": args,
        "stdin": stdin,
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
                    Ok(ToolOutcome {
                        output: json_text_field(&parsed, "output")
                            .unwrap_or_else(|| String::from("{}")),
                        stdout: json_text_field(&parsed, "stdout").unwrap_or_default(),
                        stderr: json_text_field(&parsed, "stderr").unwrap_or_default(),
                    })
                } else {
                    Ok(ToolOutcome {
                        output: payload_str.into(),
                        stdout: String::new(),
                        stderr: String::new(),
                    })
                }
            }
            Err(err) => Err(AgentError::InvalidMethod(err.to_string())),
        }
    }
}

/// Reads a text field out of a JSON payload the app produced. `None` when the
/// field is absent, which is what an app built before tool streams (or one
/// that only ever answers with a result payload) produces.
fn json_text_field(value: &serde_json::Value, field: &str) -> Option<String> {
    match value.get(field) {
        Some(serde_json::Value::String(text)) => Some(text.clone()),
        Some(other) => Some(other.to_string()),
        None => None,
    }
}

/// Folds the items drained from a caller's stdin stream into text: successful
/// chunks concatenate, and a failed item is reported instead of handing the
/// app half of its input (clean end-of-input is stream closure, not a failed
/// item).
fn tool_stdin_text(
    items: Vec<Result<Vec<u8>, golem::tool::streams::ByteStreamFailure>>,
) -> Result<String, String> {
    let mut text = String::new();
    for item in items {
        match item {
            Ok(chunk) => text.push_str(&String::from_utf8_lossy(&chunk)),
            Err(failure) => return Err(format!("the tool's stdin stream failed: {failure:?}")),
        }
    }
    Ok(text)
}

/// Drains the stdin stream the runtime attached to a tool invocation. The
/// export is async, so the reader is awaited directly (the platform's
/// synchronous effects drive their host calls with `wit_bindgen::block_on`
/// instead).
async fn drain_tool_stdin(
    reader: wit_bindgen::StreamReader<Result<Vec<u8>, golem::tool::streams::ByteStreamFailure>>,
) -> Result<String, String> {
    tool_stdin_text(reader.collect().await)
}

/// Writes one channel of the app's buffered output through the writer the
/// runtime attached, then finishes it — finishing is what turns the last write
/// into a clean end-of-stream for the reader, and an empty text still finishes
/// so a reader is never left waiting on a writer the guest abandoned.
///
/// Output is written *after* the app returns: the app answers with a whole
/// string, so there is nothing to stream while it runs (see the README).
async fn write_tool_output(
    writer: Option<golem::tool::streams::ToolOutputWriter>,
    text: &str,
    channel: &str,
) {
    let Some(writer) = writer else { return };
    if !text.is_empty() {
        match writer.write(text.as_bytes().to_vec()).await {
            Ok(()) => {}
            Err(err) => {
                note_tool_stream_stop(channel, err);
                return;
            }
        }
    }
    if let Err(err) = writer.finish().await {
        note_tool_stream_stop(channel, err);
    }
}

/// A writer that closed because the consumer cancelled is not a failure — the
/// reader went away (a CLI that stopped reading, say), and the invocation's
/// result still stands. Anything else is logged for the operator rather than
/// failing a call whose answer is valid.
fn note_tool_stream_stop(channel: &str, error: golem::tool::streams::StreamWriteError) {
    use golem::tool::streams::{ByteStreamCloseCause, StreamWriteError};

    if let StreamWriteError::Closed(ByteStreamCloseCause::ConsumerCancelled) = error {
        return;
    }
    wasi::logging::logging::log(
        wasi::logging::logging::Level::Warn,
        "roc-agent",
        &format!("could not write the tool's {channel}: {error:?}"),
    );
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
        stdin: Option<
            wit_bindgen::StreamReader<Result<Vec<u8>, golem::tool::streams::ByteStreamFailure>>,
        >,
        stdout: Option<golem::tool::streams::ToolOutputWriter>,
        stderr: Option<golem::tool::streams::ToolOutputWriter>,
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
        // stdin is drained before the app is called: the app reads it as one
        // string, so a stream that is still being written to has nothing to
        // hand it mid-call.
        let stdin_text = match stdin {
            Some(reader) => drain_tool_stdin(reader).await.map_err(
                exports::golem::tool::guest::ToolError::InvalidInput,
            )?,
            None => String::new(),
        };
        match run_tool_call(&app_name, &args, &stdin_text) {
            Ok(outcome) => {
                // The app's answer is written after it returns, through the
                // writers the runtime attached. `InvocationResult`'s own
                // `stdout`/`stderr` stay absent: the runtime rejects a guest
                // that returns attachment streams instead of using them.
                write_tool_output(stdout, &outcome.stdout, "stdout").await;
                write_tool_output(stderr, &outcome.stderr, "stderr").await;
                Ok(golem::tool::common::InvocationResult {
                    result: Some(golem::core::types::TypedSchemaValue {
                        graph: string_schema_graph(),
                        value: single_string_value_tree(&outcome.output),
                    }),
                    stdout: None,
                    stderr: None,
                })
            }
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
///
/// Every command declares stdin, stdout and stderr as *optional* streams: the
/// runtime refuses an attachment a command did not declare, and requires one it
/// declared as `required`, so `required: false` is what makes
/// `golem tool invoke --stdin … --stdout` work while a plain invocation
/// attaches nothing.
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
                    stdin: Some(tool_stream_spec(
                        "Text the caller pipes in; the app reads it as `toolCall.stdin`.",
                    )),
                    stdout: Some(tool_stream_spec(
                        "Text the app answers with in `ToolResult.stdout`.",
                    )),
                    stderr: Some(tool_stream_spec(
                        "Text the app answers with in `ToolResult.stderr`.",
                    )),
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

/// One of the three optional streams a tool command declares. The `mime` list
/// is metadata only (the platform's chunks are raw bytes); `required: false`
/// keeps the attachment up to the caller.
fn tool_stream_spec(doc: &str) -> golem::tool::common::StreamSpec {
    use golem::tool::common::{Doc, StreamSpec};

    StreamSpec {
        doc: Doc {
            summary: doc.to_string(),
            description: String::new(),
            examples: Vec::new(),
        },
        mime: vec![String::from("application/octet-stream")],
        required: false,
    }
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

    /// Every tool advertises the three streams the `golem:tool/guest` surface
    /// can attach. The runtime refuses an attachment a command does not
    /// declare, so a missing spec here is what would make
    /// `golem tool invoke --stdin … --stdout` fail before the app runs; a spec
    /// declared `required` would make a plain invocation fail instead.
    #[test]
    fn test_build_tools_declares_optional_streams() {
        let tools = build_tools();
        assert!(!tools.is_empty(), "the stub app declares tools");
        for (app_name, tool) in &tools {
            let body = tool.commands.nodes[0]
                .body
                .as_ref()
                .expect("every tool has a command body");
            for (channel, spec) in [
                ("stdin", &body.stdin),
                ("stdout", &body.stdout),
                ("stderr", &body.stderr),
            ] {
                let spec = spec
                    .as_ref()
                    .unwrap_or_else(|| panic!("{app_name}: no {channel} stream spec"));
                assert!(
                    !spec.required,
                    "{app_name}: {channel} must stay optional"
                );
                assert!(
                    !spec.doc.summary.is_empty(),
                    "{app_name}: {channel} should say what it carries"
                );
            }
        }
    }

    /// The tool call contract the Roc records describe, end to end: stdin goes
    /// into the call payload, and the app's answer carries the text for both
    /// output channels back out.
    #[test]
    fn test_tool_call_streams_round_trip() {
        let outcome =
            run_tool_call("echo", "{\"text\": \"hi\"}", "piped text").expect("tool call");
        assert!(outcome.output.contains("hi"), "output: {}", outcome.output);
        assert_eq!(outcome.stdout, "piped text");
        assert_eq!(outcome.stderr, "echo tool: read 10 bytes of stdin");

        // A tool that writes nothing answers with empty channels, and the
        // payload is still the result.
        let quiet = run_tool_call("get_count", "{}", "").expect("tool call");
        assert_eq!(quiet.stdout, "");
        assert_eq!(quiet.stderr, "");
        assert!(quiet.output.contains("count"));
    }

    /// Drained stdin items fold into one text; a failed item is reported
    /// rather than handed to the app as a truncated input.
    #[test]
    fn test_stdin_text_assembly() {
        use golem::tool::streams::ByteStreamFailure;

        assert_eq!(tool_stdin_text(vec![]).expect("empty stdin"), "");
        assert_eq!(
            tool_stdin_text(vec![Ok(b"piped ".to_vec()), Ok(b"text".to_vec())])
                .expect("two chunks"),
            "piped text"
        );
        // Chunks are bytes on the wire; a partial UTF-8 sequence is replaced
        // rather than failing the call.
        assert_eq!(
            tool_stdin_text(vec![Ok(vec![0x61, 0xff])]).expect("lossy chunk"),
            "a\u{fffd}"
        );
        let failure = tool_stdin_text(vec![
            Ok(b"half".to_vec()),
            Err(ByteStreamFailure::Abandoned),
        ])
        .expect_err("a failed item is an error");
        assert!(failure.contains("Abandoned"), "failure: {failure}");
    }

    /// A scheduled call's time is `now + delay`, with the nanoseconds field
    /// normalised: the host rejects an `instant` whose nanoseconds field does
    /// not fit below a second, and a delay crossing a second has to carry.
    #[test]
    fn test_instant_after() {
        let now = golem::agent::host::Datetime {
            seconds: 1_700_000_000,
            nanoseconds: 400_000_000,
        };

        let later = instant_after(now, 1_500);
        assert_eq!(later.seconds, 1_700_000_001);
        assert_eq!(later.nanoseconds, 900_000_000);

        let later = instant_after(now, 1_000);
        assert_eq!(later.seconds, 1_700_000_001);
        assert_eq!(later.nanoseconds, 400_000_000);

        let now_again = instant_after(now, 0);
        assert_eq!(now_again.seconds, now.seconds);
        assert_eq!(now_again.nanoseconds, now.nanoseconds);

        // A delay long enough to overflow the millisecond→nanosecond
        // conversion saturates rather than wrapping, and still produces a
        // well-formed instant.
        let far = instant_after(now, u64::MAX);
        assert_eq!(far.seconds, 1_700_000_000 + 18_446_744_073);
        assert_eq!(far.nanoseconds, 709_551_615);
    }

    /// The invocation identity the fire-and-forget and scheduling effects
    /// return is handed to Roc as JSON, which is how the platform reports
    /// multi-field results. Agent ids carry quotes, so escaping matters.
    #[test]
    fn test_invocation_identity_json() {
        let metadata = golem::agent::host::InvocationMetadata {
            agent_id: String::from("counter-agent(\"c-1\")"),
            idempotency_key: String::from("d372f3c4-0950-5dbd-b198-6570a53386f0"),
        };

        let json: serde_json::Value = serde_json::from_str(&invocation_identity_json(&metadata))
            .expect("the identity must be valid JSON");
        assert_eq!(json["agentId"], "counter-agent(\"c-1\")");
        assert_eq!(json["idempotencyKey"], "d372f3c4-0950-5dbd-b198-6570a53386f0");
    }

    /// Golem's `uuid` is two words; `parseAgentId!` reports a phantom id in the
    /// canonical spelling.
    #[test]
    fn test_uuid_string() {
        let id = golem::core::types::Uuid {
            high_bits: 0x0102_0304_0506_0708,
            low_bits: 0x090a_0b0c_0d0e_0f10,
        };
        assert_eq!(uuid_string(&id), "01020304-0506-0708-090a-0b0c0d0e0f10");
    }

    /// Handles are indices into a thread-local table, offset by one. Taking one
    /// frees its slot for the next resource, which is why a handle can only be
    /// used once; 0 is never a handle.
    #[test]
    fn test_handle_table() {
        thread_local! {
            static TEST_SLOTS: RefCell<Vec<Option<String>>> = const { RefCell::new(Vec::new()) };
        }

        let first = handle_store(&TEST_SLOTS, String::from("a"));
        let second = handle_store(&TEST_SLOTS, String::from("b"));
        assert_eq!((first, second), (1, 2));

        assert_eq!(
            handle_with(&TEST_SLOTS, first, |value| value.clone()),
            Some(String::from("a"))
        );
        assert_eq!(handle_with(&TEST_SLOTS, 0, |value| value.clone()), None);
        assert_eq!(handle_with(&TEST_SLOTS, 99, |value| value.clone()), None);

        assert_eq!(handle_take(&TEST_SLOTS, first), Some(String::from("a")));
        assert_eq!(handle_take(&TEST_SLOTS, first), None);
        assert_eq!(handle_take(&TEST_SLOTS, 0), None);
        assert_eq!(handle_store(&TEST_SLOTS, String::from("c")), first);
        assert_eq!(handle_take(&TEST_SLOTS, first), Some(String::from("c")));
    }

    /// A webhook's promise id stays host-side, keyed by the URL the app carries
    /// in its state; an unknown URL is an error rather than a lookup of someone
    /// else's promise.
    #[test]
    fn test_webhook_promise_table() {
        let url = "http://roc-golem.localhost:9882/webhooks/effects-agent/abc123";
        assert!(webhook_promise(url).is_none());

        remember_webhook_promise(
            url,
            golem::core::types::PromiseId {
                agent_id: golem::core::types::AgentId {
                    component_id: golem::core::types::ComponentId {
                        uuid: golem::core::types::Uuid {
                            high_bits: 0x0102_0304_0506_0708,
                            low_bits: 0x090a_0b0c_0d0e_0f10,
                        },
                    },
                    agent_id: String::from("effects-agent(\"e2e\")"),
                },
                oplog_idx: 7,
            },
        );

        let found = webhook_promise(url).expect("the URL should resolve to its promise");
        assert_eq!(found.oplog_idx, 7);
        assert_eq!(found.agent_id.agent_id, "effects-agent(\"e2e\")");
        assert_eq!(
            found.agent_id.component_id.uuid.high_bits,
            0x0102_0304_0506_0708
        );
        assert!(webhook_promise(
            "http://roc-golem.localhost:9882/webhooks/effects-agent/other"
        )
        .is_none());
    }

    /// The `webhook-error` variant is rendered for the effect's error channel:
    /// `permission-denied` and the host's message stay distinguishable.
    #[test]
    fn test_webhook_error_text() {
        assert_eq!(
            webhook_error_text(golem::agent::host::WebhookError::PermissionDenied),
            "permission-denied"
        );
        assert_eq!(
            webhook_error_text(golem::agent::host::WebhookError::InternalError(
                "not deployed as part of an http api".to_string()
            )),
            "internal-error: not deployed as part of an http api"
        );
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

    /// The router's `handle` method declares Golem's canonical request and
    /// response shapes: a request record with the seven envelope fields (its
    /// body a `stream<list<u8>>`) and a response record of status, headers and
    /// the same stream. Golem validates the graph when it registers the agent
    /// type, so a field out of place is a 502 rather than a compile error.
    #[test]
    fn test_router_schema_graph_is_golems_canonical_envelope() {
        let schema = router_schema_graph();
        let fields = |index: i32| match &schema.graph.type_nodes[index as usize].body {
            SchemaTypeBody::RecordType(fields) => fields.clone(),
            other => panic!("expected a record at {index}, got {other:?}"),
        };
        let names = |fields: &[NamedFieldType]| {
            fields
                .iter()
                .map(|field| field.name.clone())
                .collect::<Vec<_>>()
        };

        let request = fields(schema.request);
        let response = fields(schema.response);
        assert_eq!(
            names(&request),
            vec!["method", "scheme", "authority", "path", "query", "headers", "body"]
        );
        assert_eq!(names(&response), vec!["status", "headers", "body"]);
        assert_eq!(schema.graph.root, schema.request);

        // The response status is a `u16`, and both bodies are `stream<list<u8>>`:
        // Golem rebuilds the head from the record's first two fields and relays
        // the third as the body, so a field out of place is a 502.
        assert!(matches!(
            schema.graph.type_nodes[response[0].body as usize].body,
            SchemaTypeBody::U16Type(None)
        ));
        for body in [request[6].body, response[2].body] {
            match &schema.graph.type_nodes[body as usize].body {
                SchemaTypeBody::StreamType(Some(inner)) => assert!(matches!(
                    schema.graph.type_nodes[*inner as usize].body,
                    SchemaTypeBody::ListType(_)
                )),
                other => panic!("expected a typed stream at {body}, got {other:?}"),
            }
        }
    }

    /// The handler's request argument decodes from the parameter record Golem
    /// passes in, and taking the body out of the tree leaves the stream handle
    /// with exactly one owner — a resource cannot be cloned, so a second read
    /// has to find the slot emptied.
    #[test]
    fn test_router_request_decodes_and_takes_the_body_stream() {
        let mut tree = SchemaValueTree {
            value_nodes: vec![
                // 0: the parameter record; 1: the request record
                SchemaValueNode::RecordValue(vec![1]),
                SchemaValueNode::RecordValue(vec![2, 3, 4, 5, 6, 7, 8]),
                SchemaValueNode::StringValue(String::from("POST")),
                SchemaValueNode::StringValue(String::from("https")),
                SchemaValueNode::StringValue(String::from("example.test")),
                SchemaValueNode::StringValue(String::from("/static/echo")),
                SchemaValueNode::OptionValue(Some(9)),
                SchemaValueNode::ListValue(vec![11]),
                // 8: the body stream. The handle is leaked rather than dropped
                // below: outside wasm there is no host for the resource's drop
                // import to call.
                SchemaValueNode::StreamValue(unsafe {
                    golem::core::types::SchemaValueStream::from_handle(7)
                }),
                SchemaValueNode::StringValue(String::from("a=1")),
                SchemaValueNode::OptionValue(None),
                // 11: one header entry, 13 its name and 14-15 its bytes
                SchemaValueNode::RecordValue(vec![13, 14]),
                SchemaValueNode::OptionValue(None),
                SchemaValueNode::StringValue(String::from("x-token")),
                SchemaValueNode::ListValue(vec![15]),
                SchemaValueNode::U8Value(b't'),
            ],
            root: 0,
        };

        let mut request = RouterRequest::from_value_tree(&mut tree).expect("the request decodes");
        assert_eq!(request.method, "POST");
        assert_eq!(request.scheme, "https");
        assert_eq!(request.authority, "example.test");
        assert_eq!(request.path, "/static/echo");
        assert_eq!(request.query.as_deref(), Some("a=1"));
        assert_eq!(
            request.headers,
            vec![(String::from("x-token"), String::from("t"))]
        );
        let body = core::mem::replace(&mut request.body, SchemaValueNode::OptionValue(None));
        assert!(matches!(body, SchemaValueNode::StreamValue(_)));
        core::mem::forget(body);
        assert!(matches!(
            tree.value_nodes[8],
            SchemaValueNode::OptionValue(None)
        ));
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

    /// The hand-written ABI must agree with the compiler-generated one: same
    /// layout, and the same small-string, seamless-slice and refcount rules.
    /// This is the safety net while `roc_std.rs` is being replaced by the
    /// generated module (`roc glue` + the official `RustGlue.roc`).
    #[test]
    fn test_generated_abi_agrees_with_hand_written() {
        use crate::roc_platform_abi as generated;
        use crate::roc_std::RocStr as HandWritten;

        // Same committed layout.
        assert_eq!(
            core::mem::size_of::<generated::RocStr>(),
            core::mem::size_of::<HandWritten>()
        );
        assert_eq!(
            core::mem::align_of::<generated::RocStr>(),
            core::mem::align_of::<HandWritten>()
        );

        // A heap string built by the hand-written side reads the same through
        // the generated accessors, including the capacity encoding (shifted left
        // by one) and the absence of the slice tag.
        let text = "a string long enough to be heap allocated by both sides";
        let mine = HandWritten::from_str(text);
        let theirs = generated::RocStr {
            bytes: mine.words()[0] as *mut u8,
            capacity_or_alloc_ptr: mine.words()[1],
            length: mine.words()[2],
        };
        assert_eq!(theirs.len(), text.len());
        assert_eq!(theirs.as_slice(), text.as_bytes());
        assert!(!theirs.is_small_str());
        assert!(!theirs.is_seamless_slice());
        assert_eq!(theirs.capacity_or_alloc_ptr, text.len() << 1);

        // A seamless slice is recognised by both, and both follow it back to the
        // allocation it belongs to.
        let data_ptr = mine.words()[0];
        let slice = generated::RocStr {
            bytes: (data_ptr + 8) as *mut u8,
            capacity_or_alloc_ptr: data_ptr | 1,
            length: text.len() - 8,
        };
        assert!(slice.is_seamless_slice());
        assert_eq!(slice.as_slice(), &text.as_bytes()[8..]);
        let ours = HandWritten::from_parts(slice.bytes, slice.capacity_or_alloc_ptr, slice.length);
        assert_eq!(ours.as_str(), &text[8..]);
        assert_eq!(ours.allocation_ptr_for_test(), data_ptr as *mut u8);

        // And the refcount protocol: the word before the allocation, zero for
        // static data.
        unsafe {
            let refcount = *((data_ptr as *const isize).sub(1));
            assert_eq!(refcount, 1, "both sides start a fresh string at one");
        }
    }

    /// Hosted arguments are *owned* by the host: Roc transfers ownership, so the
    /// function must release them. These used to be taken as `*const RocStr` and
    /// never freed, which leaked every string the app passed to an effect.
    /// `rocFxWsSend` takes its message by value now, and dropping it decrements
    /// the refcount; an unknown handle keeps the call off the host APIs, which
    /// are stubbed outside wasm.
    #[test]
    fn test_hosted_argument_is_released() {
        let text = "a websocket message long enough to live on the heap";
        let msg = crate::roc_std::RocStr::from_str(text);
        let alloc_ptr = msg.words()[0];
        unsafe {
            let refcount = (alloc_ptr as *const isize).sub(1);
            assert_eq!(*refcount, 1, "a fresh string owns one reference");
            let _ = crate::rocFxWsSend(9999, msg);
            assert_eq!(
                *refcount, 0,
                "the hosted function released the argument it owned"
            );
        }
    }

    /// `blobList!` renders the names the stream produced as a JSON array of
    /// strings, which is what the effect promises the app.
    #[test]
    fn test_names_json() {
        assert_eq!(crate::names_json(&[]), "[]");
        assert_eq!(crate::names_json(&[String::from("one")]), "[\"one\"]");
        assert_eq!(
            crate::names_json(&[String::from("a\"b"), String::from("c\\d")]),
            "[\"a\\\"b\",\"c\\\\d\"]"
        );
    }

    /// The three result shapes the store effects use (`Try(Str, …)`,
    /// `Try({}, …)` and `Try(Bool, …)`) travel through the hand-written ABI the
    /// way the other effects' results do.
    #[test]
    fn test_store_result_helpers() {
        let ok = crate::text_result(Ok(String::from("value")));
        assert!(matches!(ok.into_result(), Ok(text) if text.as_str() == "value"));
        let err = crate::text_result(Err(String::from("store is down")));
        assert!(matches!(err.into_result(), Err(text) if text.as_str() == "store is down"));

        assert!(matches!(crate::unit_result(Ok(())).into_result(), Ok(())));
        assert!(
            matches!(crate::unit_result(Err(String::from("no"))).into_result(), Err(text) if text.as_str() == "no")
        );

        assert!(matches!(crate::bool_result(Ok(true)).into_result(), Ok(true)));
        assert!(matches!(crate::bool_result(Ok(false)).into_result(), Ok(false)));
        assert!(
            matches!(crate::bool_result(Err(String::from("no"))).into_result(), Err(text) if text.as_str() == "no")
        );
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

    /// A rejected reservation renders the host's `failed-reservation`: the
    /// estimated wait is only there when the resource can predict one (a rate
    /// limit can, a capacity limit cannot), and both halves stay readable in the
    /// effect's error channel.
    #[test]
    fn test_failed_reservation_text() {
        use golem::quota::types::FailedReservation;

        assert_eq!(
            failed_reservation_text(&FailedReservation {
                estimated_wait_nanos: None
            }),
            "quota reservation failed"
        );
        assert_eq!(
            failed_reservation_text(&FailedReservation {
                estimated_wait_nanos: Some(250_000_000)
            }),
            "quota reservation failed (retry after 250ms)"
        );
        assert_eq!(
            failed_reservation_text(&FailedReservation {
                estimated_wait_nanos: Some(1_500_000_000)
            }),
            "quota reservation failed (retry after 1.5s)"
        );
    }

    /// A fork result renders as `original:<phantom-id>` / `forked:<phantom-id>`
    /// from the record's phantom id — the same id on both sides of the fork, so
    /// an app can tell which branch it is on while still naming the copy.
    #[test]
    fn test_fork_result_text() {
        use golem::api::host::{ForkDetails, ForkResult};

        let details = ForkDetails {
            forked_phantom_id: test_uuid(),
        };
        assert_eq!(
            fork_result_text(ForkResult::Original(details.clone())),
            "original:01020304-0506-0708-090a-0b0c0d0e0f10"
        );
        assert_eq!(
            fork_result_text(ForkResult::Forked(details)),
            "forked:01020304-0506-0708-090a-0b0c0d0e0f10"
        );
    }

    /// The agent id `resolveAgentId!` returns is handed to Roc as JSON, because
    /// the two halves an app needs are both there: the agent id string (with its
    /// quotes, which have to survive) and the component id the string does not
    /// carry.
    #[test]
    fn test_agent_id_json() {
        let id = golem::core::types::AgentId {
            component_id: golem::core::types::ComponentId {
                uuid: test_uuid(),
            },
            agent_id: String::from("counter-agent(\"c-1\")"),
        };

        let json: serde_json::Value =
            serde_json::from_str(&agent_id_json(&id)).expect("the agent id must be valid JSON");
        assert_eq!(json["componentId"], "01020304-0506-0708-090a-0b0c0d0e0f10");
        assert_eq!(json["agentId"], "counter-agent(\"c-1\")");
    }

    /// `agent-operation-error` renders for the effect's error channel, the way
    /// `webhook-error` does for the webhook effects.
    #[test]
    fn test_agent_operation_error_text() {
        use golem::api::host::AgentOperationError;

        assert_eq!(
            agent_operation_error_text(AgentOperationError::PermissionDenied),
            "permission-denied"
        );
        assert_eq!(
            agent_operation_error_text(AgentOperationError::BackendError(
                "shard lost".to_string()
            )),
            "backend-error: shard lost"
        );
    }

    /// A reference that named no component is reported instead of looking like
    /// an empty result; the text names the reference the app asked for.
    #[test]
    fn test_unresolved_component_text() {
        assert_eq!(
            unresolved_component_text("roc-golem:nope"),
            "no component named \"roc-golem:nope\""
        );
    }

    /// The UUID the agent-id and fork-result tests share.
    fn test_uuid() -> golem::core::types::Uuid {
        golem::core::types::Uuid {
            high_bits: 0x0102_0304_0506_0708,
            low_bits: 0x090a_0b0c_0d0e_0f10,
        }
    }

    /// A handle this instance never handed out is reported, not passed to the
    /// host APIs (which are stubbed outside wasm, so a call would panic here the
    /// way it would fail on a real server).
    #[test]
    fn test_quota_unknown_handles() {
        unsafe {
            assert!(crate::rocFxQuotaReserve(9999, 1).is_err());
            assert!(crate::rocFxQuotaCommit(9999, 0).is_err());
            assert!(crate::rocFxQuotaSplit(9999, 1).is_err());
            assert!(crate::rocFxQuotaMerge(1, 2).is_err());
            // Merging a token with itself would consume the receiver, so it is
            // refused before either handle is looked up.
            assert!(crate::rocFxQuotaMerge(1, 1).is_err());
        }
    }

    /// The path rule the filesystem effects promise: a leading `/` names the
    /// agent's root (the spelling the manifest's `files:` targets use), `.` and
    /// empty segments are dropped, `..` steps back inside the root, and a path
    /// that would step above it is refused with the app's own path in the
    /// message.
    #[test]
    fn test_resolve_agent_path() {
        assert_eq!(
            crate::resolve_agent_path("seed/a.txt").unwrap(),
            "seed/a.txt"
        );
        assert_eq!(
            crate::resolve_agent_path("/seed/a.txt").unwrap(),
            "seed/a.txt"
        );
        assert_eq!(
            crate::resolve_agent_path("//seed//a.txt").unwrap(),
            "seed/a.txt"
        );
        assert_eq!(
            crate::resolve_agent_path("./seed/./a.txt").unwrap(),
            "seed/a.txt"
        );
        assert_eq!(
            crate::resolve_agent_path("seed/../seed/a.txt").unwrap(),
            "seed/a.txt"
        );
        // `/` and `.` are the root, the empty relative path the descriptor
        // calls read as "the base directory".
        assert_eq!(crate::resolve_agent_path("/").unwrap(), "");
        assert_eq!(crate::resolve_agent_path(".").unwrap(), "");

        assert_eq!(
            crate::resolve_agent_path("/.."),
            Err(String::from("path /.. escapes the agent's filesystem root"))
        );
        assert_eq!(
            crate::resolve_agent_path("seed/../../a.txt"),
            Err(String::from(
                "path seed/../../a.txt escapes the agent's filesystem root"
            ))
        );
    }

    /// The read loop assembles a file out of the chunks a p2 `read` answers: the
    /// boolean is the end-of-file signal, not "the host sent less than asked
    /// for", and the offset advances by what arrived.
    #[test]
    fn test_read_all_chunks() {
        // One chunk that reaches the end of the file.
        let mut chunks = vec![(b"hello".to_vec(), true)].into_iter();
        let mut calls = Vec::new();
        let contents = crate::read_all_chunks(|length, offset| {
            calls.push((length, offset));
            Ok(chunks.next().unwrap())
        })
        .unwrap();
        assert_eq!(contents, b"hello");
        assert_eq!(calls, vec![(crate::READ_CHUNK, 0)]);

        // A chunk that is not the end, however short: the loop asks again from
        // where the last one stopped.
        let mut chunks = vec![(b"abc".to_vec(), false), (b"def".to_vec(), true)].into_iter();
        let mut offsets = Vec::new();
        let contents = crate::read_all_chunks(|_, offset| {
            offsets.push(offset);
            Ok(chunks.next().unwrap())
        })
        .unwrap();
        assert_eq!(contents, b"abcdef");
        assert_eq!(offsets, vec![0, 3]);

        // An empty chunk without the end flag would spin the loop, so it is
        // reported as the broken host it would be.
        let error = crate::read_all_chunks(|_, _| Ok((Vec::new(), false))).unwrap_err();
        assert!(
            error.contains("without reaching the end of the file"),
            "unexpected error: {error}"
        );
    }

    /// The write loop advances by what each `write` reports, and a call that
    /// accepts nothing is reported instead of retried forever.
    #[test]
    fn test_write_all_chunks() {
        let mut accepted = vec![2u64, 3u64].into_iter();
        let mut calls: Vec<(Vec<u8>, u64)> = Vec::new();
        crate::write_all_chunks(
            |buffer, offset| {
                calls.push((buffer.to_vec(), offset));
                Ok(accepted.next().unwrap())
            },
            b"hello",
            10,
        )
        .unwrap();
        assert_eq!(calls, vec![(b"hello".to_vec(), 10), (b"llo".to_vec(), 12)]);

        let error = crate::write_all_chunks(|_, _| Ok(0), b"hello", 0).unwrap_err();
        assert!(error.contains("accepted no bytes"), "unexpected error: {error}");

        // Nothing to write takes no call at all.
        let mut called = false;
        crate::write_all_chunks(
            |_, _| {
                called = true;
                Ok(0)
            },
            b"",
            0,
        )
        .unwrap();
        assert!(!called);
    }

    /// A listing renders one `{name, type}` per entry, sorted by name so two
    /// listings of an unchanged directory read the same, with the interface's
    /// file kinds folded to the four names the effect promises and the names
    /// JSON-escaped (they are arbitrary text).
    #[test]
    fn test_directory_listing_json() {
        use wasi::filesystem0_2_6::types::DescriptorType;

        assert_eq!(crate::directory_listing_json(&[]), "[]");

        let entries = vec![
            (String::from("b.txt"), DescriptorType::RegularFile),
            (String::from("a\"dir"), DescriptorType::Directory),
            (String::from("link"), DescriptorType::SymbolicLink),
            (String::from("pipe"), DescriptorType::Fifo),
        ];
        assert_eq!(
            crate::directory_listing_json(&entries),
            "[{\"name\":\"a\\\"dir\",\"type\":\"dir\"},{\"name\":\"b.txt\",\"type\":\"file\"},{\"name\":\"link\",\"type\":\"symlink\"},{\"name\":\"pipe\",\"type\":\"other\"}]"
        );
    }

    /// A filesystem failure travels as the WIT name of its `error-code`, which is
    /// the vocabulary the interface's own documentation uses.
    #[test]
    fn test_filesystem_error_text() {
        use wasi::filesystem0_2_6::types::ErrorCode;

        assert_eq!(crate::filesystem_error_text(ErrorCode::NoEntry), "no-entry");
        assert_eq!(
            crate::filesystem_error_text(ErrorCode::NotPermitted),
            "not-permitted"
        );
        assert_eq!(crate::filesystem_error_text(ErrorCode::Exist), "exist");
        assert_eq!(
            crate::filesystem_error_text(ErrorCode::InsufficientSpace),
            "insufficient-space"
        );
    }

    /// The `db-value` JSON mirror, both ways: the common cases of each engine's
    /// vocabulary parse into the interface's own variant, and every parsed value
    /// renders back to the JSON it came from — so what `dbQuery!` prints can be
    /// handed straight back as a param.
    #[test]
    fn test_db_value_json_round_trip() {
        use crate::rdbms_json;

        let items: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
                {"tag":"int4","value":1},
                {"tag":"text","value":"hello"},
                {"tag":"boolean","value":true},
                {"tag":"numeric","value":"1.50"},
                {"tag":"bytea","value":[1,2,255]},
                {"tag":"uuid","value":"01020304-0506-0708-090a-0b0c0d0e0f10"},
                {"tag":"timestamp","value":{"date":{"year":2024,"month":1,"day":31},"time":{"hour":12,"minute":0,"second":5,"nanosecond":7}}},
                {"tag":"null"}
            ]"#,
        )
        .unwrap();
        let values = rdbms_json::postgres_params(&items).unwrap();
        assert_eq!(values.len(), items.len());
        assert!(matches!(values[0], golem::rdbms::postgres::DbValue::Int4(1)));
        assert!(
            matches!(&values[1], golem::rdbms::postgres::DbValue::Text(text) if text == "hello")
        );
        assert!(matches!(
            values[2],
            golem::rdbms::postgres::DbValue::Boolean(true)
        ));
        assert!(
            matches!(&values[3], golem::rdbms::postgres::DbValue::Numeric(number) if number == "1.50")
        );
        assert!(matches!(
            &values[4],
            golem::rdbms::postgres::DbValue::Bytea(bytes) if bytes == &[1, 2, 255]
        ));
        // A UUID is the one hand-mapped case: the WIT record's two words are the
        // canonical text.
        let golem::rdbms::postgres::DbValue::Uuid(uuid) = &values[5] else {
            panic!("the sixth value must be a uuid, got {:?}", values[5]);
        };
        assert_eq!(uuid.high_bits, 0x0102_0304_0506_0708);
        assert_eq!(uuid.low_bits, 0x090a_0b0c_0d0e_0f10);
        let golem::rdbms::postgres::DbValue::Timestamp(timestamp) = &values[6] else {
            panic!("the seventh value must be a timestamp, got {:?}", values[6]);
        };
        assert_eq!(timestamp.date.year, 2024);
        assert_eq!(timestamp.date.day, 31);
        assert_eq!(timestamp.time.nanosecond, 7);
        assert!(matches!(values[7], golem::rdbms::postgres::DbValue::Null));

        for (item, value) in items.iter().zip(&values) {
            assert_eq!(&rdbms_json::postgres_value_json(value), item);
        }

        // MySQL's vocabulary has its own names for the same shapes.
        let items: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
                {"tag":"int","value":1},
                {"tag":"bigintUnsigned","value":18446744073709551615},
                {"tag":"varchar","value":"hello"},
                {"tag":"decimal","value":"1.50"},
                {"tag":"bit","value":[true,false]},
                {"tag":"null"}
            ]"#,
        )
        .unwrap();
        let values = rdbms_json::mysql_params(&items).unwrap();
        assert!(matches!(values[0], golem::rdbms::mysql::DbValue::Int(1)));
        assert!(matches!(
            values[1],
            golem::rdbms::mysql::DbValue::BigintUnsigned(18446744073709551615)
        ));
        assert!(matches!(values[5], golem::rdbms::mysql::DbValue::Null));
        for (item, value) in items.iter().zip(&values) {
            assert_eq!(&rdbms_json::mysql_value_json(value), item);
        }

        // Ignite's names are the client's own (`db-…`), and its UUID is a tuple.
        let items: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
                {"tag":"dbInt","value":1},
                {"tag":"dbString","value":"hello"},
                {"tag":"dbUuid","value":"01020304-0506-0708-090a-0b0c0d0e0f10"},
                {"tag":"dbTimestamp","value":[1700000000000,5]},
                {"tag":"dbNull"}
            ]"#,
        )
        .unwrap();
        let values = rdbms_json::ignite_params(&items).unwrap();
        assert!(matches!(values[0], golem::rdbms::ignite2::DbValue::DbInt(1)));
        assert!(matches!(
            values[2],
            golem::rdbms::ignite2::DbValue::DbUuid((0x0102_0304_0506_0708, 0x090a_0b0c_0d0e_0f10))
        ));
        assert!(matches!(
            values[3],
            golem::rdbms::ignite2::DbValue::DbTimestamp((1700000000000, 5))
        ));
        assert!(matches!(values[4], golem::rdbms::ignite2::DbValue::DbNull));
        for (item, value) in items.iter().zip(&values) {
            assert_eq!(&rdbms_json::ignite_value_json(value), item);
        }
    }

    /// A params text that is not the encoding is reported with what is wrong, so
    /// an app can fix its JSON instead of reading a host error.
    #[test]
    fn test_db_params_json_errors() {
        use crate::rdbms_json;

        let error = rdbms_json::params_array("not json").unwrap_err();
        assert!(error.contains("not valid JSON"), "unexpected: {error}");
        let error = rdbms_json::params_array("{\"tag\":\"int4\"}").unwrap_err();
        assert!(
            error.contains("must be a JSON array"),
            "unexpected: {error}"
        );

        let items: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
                {"tag":"int5","value":1},
                {"tag":"text"},
                {"tag":"int4","value":"1"},
                {"tag":"uuid","value":"nope"},
                {"tag":"null","value":1}
            ]"#,
        )
        .unwrap();
        let errors: Vec<String> = items
            .iter()
            .map(|item| rdbms_json::postgres_params(std::slice::from_ref(item)).unwrap_err())
            .collect();
        assert!(
            errors[0].contains("unknown db-value tag \"int5\""),
            "unexpected: {}",
            errors[0]
        );
        assert!(errors[1].contains("needs a \"value\""), "unexpected: {}", errors[1]);
        assert!(
            errors[2].contains("needs an integer"),
            "unexpected: {}",
            errors[2]
        );
        assert!(errors[3].contains("not a UUID"), "unexpected: {}", errors[3]);
        assert!(
            errors[4].contains("takes no \"value\""),
            "unexpected: {}",
            errors[4]
        );

        // The floats JSON has no number for travel as text and come back as the
        // float they are.
        let value = rdbms_json::postgres_params(&[serde_json::json!({"tag": "float8", "value": "NaN"})])
            .unwrap();
        assert!(matches!(&value[0], golem::rdbms::postgres::DbValue::Float8(number) if number.is_nan()));
        let rendered = rdbms_json::postgres_value_json(&value[0]);
        assert_eq!(rendered, serde_json::json!({"tag": "float8", "value": "NaN"}));
    }

    /// A `db-result` renders as the columns and rows `dbQuery!` promises: the
    /// column carries its ordinal, name, WIT type and the database's own type
    /// name, and a row is the JSON array of its values in column order.
    #[test]
    fn test_db_result_json() {
        use crate::rdbms_json;
        use golem::rdbms::postgres;

        let result = postgres::DbResult {
            columns: vec![
                postgres::DbColumn {
                    ordinal: 0,
                    name: String::from("id"),
                    db_type: postgres::DbColumnType::Int4,
                    db_type_name: String::from("INT4"),
                },
                postgres::DbColumn {
                    ordinal: 1,
                    name: String::from("note"),
                    db_type: postgres::DbColumnType::Text,
                    db_type_name: String::from("TEXT"),
                },
            ],
            rows: vec![
                postgres::DbRow {
                    values: vec![
                        postgres::DbValue::Int4(1),
                        postgres::DbValue::Text(String::from("hello db")),
                    ],
                },
                postgres::DbRow {
                    values: vec![postgres::DbValue::Int4(2), postgres::DbValue::Null],
                },
            ],
        };
        assert_eq!(
            rdbms_json::postgres_result(&result),
            "{\"columns\":[{\"dbType\":{\"tag\":\"int4\"},\"dbTypeName\":\"INT4\",\"name\":\"id\",\"ordinal\":0},{\"dbType\":{\"tag\":\"text\"},\"dbTypeName\":\"TEXT\",\"name\":\"note\",\"ordinal\":1}],\"rows\":[[{\"tag\":\"int4\",\"value\":1},{\"tag\":\"text\",\"value\":\"hello db\"}],[{\"tag\":\"int4\",\"value\":2},{\"tag\":\"null\"}]]}"
        );

        // Ignite's `db-column` carries no type at all, so neither does the
        // column object.
        let result = golem::rdbms::ignite2::DbResult {
            columns: vec![golem::rdbms::ignite2::DbColumn {
                ordinal: 0,
                name: String::from("n"),
            }],
            rows: vec![golem::rdbms::ignite2::DbRow {
                values: vec![golem::rdbms::ignite2::DbValue::DbLong(7)],
            }],
        };
        assert_eq!(
            rdbms_json::ignite_result(&result),
            "{\"columns\":[{\"name\":\"n\",\"ordinal\":0}],\"rows\":[[{\"tag\":\"dbLong\",\"value\":7}]]}"
        );
    }

    /// A handle this instance never handed out is reported instead of reaching
    /// the host APIs (which are stubbed outside wasm, so a call would panic here
    /// the way it would fail on a real server).
    #[test]
    fn test_db_unknown_handles() {
        unsafe {
            let statement = || RocStr::from_str("select 1");
            let params = || RocStr::from_str("[]");

            assert!(crate::rocFxDbBegin(9999).is_err());
            assert!(crate::rocFxDbCommit(9999).is_err());
            assert!(crate::rocFxDbRollback(9999).is_err());
            assert!(crate::rocFxDbQuery(0, statement(), params()).is_err());
            assert!(crate::rocFxDbExecute(9999, statement(), params())
                .into_result()
                .is_err());

            let error = match crate::rocFxDbQuery(9999, statement(), params()).into_result() {
                Err(error) => error,
                Ok(_) => panic!("an unknown handle must be reported, not answered"),
            };
            assert_eq!(error.as_str(), "unknown database handle 9999");
        }
    }
}
