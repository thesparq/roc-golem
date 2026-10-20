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

use core::cell::RefCell;
use core::mem::MaybeUninit;
use core::ptr;
use std::boxed::Box;
use std::collections::HashMap;
use std::thread::LocalKey;
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
    AgentTypeKind, CachePolicy, CorsOptions, ExactFileMapping, FieldSource, FileMapping,
    FileResponseHeader, HttpEndpointDetails, HttpMethod, HttpMountDetails, InputSchema, NamedField,
    OutputSchema, PathSegment, PathVariable, ReadOnlyConfig, Snapshotting, SubtreeFileMapping,
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
    let mut openapi_document: Option<String> = None;
    let mut metadata_error: Option<String> = None;

    if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&meta_json) {
        // The agent kind decides the rest: a router registers no application
        // methods and has no constructor parameters.
        is_router = meta.get("agentKind").and_then(|v| v.as_str()) == Some("http-router");
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
                } else if mount.static_bindings.is_empty() && openapi_document.is_none() {
                    kind_error = Some(
                        "an http-router mount must serve staticBindings or an OpenAPI document"
                            .to_string(),
                    );
                }
            }
        }
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

    // A router's only method is the OpenAPI provider, which Golem calls to build
    // the deployment's `/openapi.yaml`.
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
            schema: string_schema_graph(),
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
    table.with(|slots| {
        let mut slots = slots.borrow_mut();
        if let Some(index) = slots.iter().position(|slot| slot.is_none()) {
            slots[index] = Some(value);
            (index + 1) as u32
        } else {
            slots.push(Some(value));
            slots.len() as u32
        }
    })
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
    let high = id.high_bits;
    let low = id.low_bits;
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        high >> 32,
        (high >> 16) & 0xffff,
        high & 0xffff,
        low >> 48,
        low & 0xffff_ffff_ffff,
    )
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
/// Creating one that exists is not an error — the store keeps a directory entry
/// and a second call leaves it alone.
#[no_mangle]
pub unsafe extern "C" fn rocFxBlobCreateContainer(name: RocStr) -> RocResult<(), RocStr> {
    let name = name.as_str();
    // The host reads the container back to report its creation time, which the
    // effect does not expose: drop the handle and keep the outcome.
    unit_result(wasi::blobstore::blobstore::create_container(name).map(|_container| ()))
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
}
