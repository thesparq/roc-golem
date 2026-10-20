//! Runs a built roc-golem component outside Golem.
//!
//! Golem's runtime is the only other consumer of these components, so anything
//! the host bridge gets wrong (string layout, Result tagging, allocator
//! bookkeeping) shows up here as a wasm trap or a garbled payload rather than as
//! a remote deployment failure. Run `./tooling/build.sh all` first, then point
//! this at each component in `build/`.

use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context as TaskContext, Poll};
use std::path::PathBuf;
use std::process::ExitCode;
use wasmtime::component::{bindgen, Access, Component, Linker, Resource, Source, StreamConsumer, StreamReader, StreamResult};
use wasmtime::{Config, Engine, Store, StoreContextMut};
use wasmtime::component::ResourceTable;

use crate::wasi::filesystem0_2_6::types::{Descriptor, DescriptorStat, DescriptorType};

mod stubs;

bindgen!({
    path: "wit",
    world: "harness-world",
    debug: true,
    imports: {
        // Both of these move a component-model stream. Attaching a host-side
        // producer or consumer to a stream needs the store, so the generated
        // `WithStore` variants are what the stubs implement (`store`); the
        // rest of the two interfaces is plain (`trappable` is the default).
        "wasi:blobstore/types.[method]outgoing-value.outgoing-value-write-body": store | trappable,
        "wasi:blobstore/container.[method]container.list-objects": store | trappable,
    },
});

use exports::golem::agent::guest::Principal;
use golem::agent::common::PathSegment;
use golem::core::types::{SchemaValueNode, SchemaValueTree};

/// Renders a mount's path prefix the way an app author writes it: literal
/// segments verbatim, captures in braces.
fn render_mount_path(segments: &[PathSegment]) -> String {
    segments
        .iter()
        .map(|segment| match segment {
            PathSegment::Literal(literal) => literal.clone(),
            PathSegment::PathVariable(variable) => format!("{{{}}}", variable.variable_name),
            PathSegment::RemainingPathVariable(variable) => {
                format!("{{{}...}}", variable.variable_name)
            }
            PathSegment::SystemVariable(variable) => format!("<{variable:?}>"),
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Dummy host state. The linked component needs a monotonic clock, an empty poll
/// set, a logging sink and WebSocket connections; the recorded effects are what
/// the checks below assert on.
struct Ctx {
    table: ResourceTable,
    logs: Vec<String>,
    sent: Vec<String>,
    connections: Vec<StubConnection>,
    /// Remote calls the agent made through `rpc!`.
    rpc_calls: Vec<String>,
    /// Quota host calls (`golem:quota/types`) the agent made.
    quota_calls: Vec<String>,
    /// Retry-policy host calls (`golem:api/retry`) the agent made.
    retry_calls: Vec<String>,
    /// Oplog and execution-control host calls (`golem:api/host`) the agent made:
    /// `trap`, `oplog-commit`, the resolvers, the idempotency key …
    host_api_calls: Vec<String>,
    /// Key-value host calls (`wasi:keyvalue`) the agent made, for `--expect-kv`.
    kv_calls: Vec<String>,
    /// The harness's own key-value store: `(bucket, key)` → value. Buckets are
    /// implicit here exactly as they are in the real store: a write makes one.
    kv_store: std::collections::HashMap<(String, String), Vec<u8>>,
    /// Bucket handles: rep → bucket name.
    kv_buckets: Vec<String>,
    /// Outgoing values the guest filled with `outgoing-value-write-body-sync`:
    /// rep → body.
    kv_outgoing: Vec<Vec<u8>>,
    /// Incoming values a `get` answered with: rep → body (consumed once).
    kv_incoming: Vec<Vec<u8>>,
    /// Blobstore host calls (`wasi:blobstore`) the agent made, for
    /// `--expect-blob`.
    blob_calls: Vec<String>,
    /// The harness's own containers: container name → object name → body.
    blob_containers: BTreeMap<String, BTreeMap<String, Vec<u8>>>,
    /// Container handles: rep → container name.
    blob_container_reps: Vec<String>,
    /// Outgoing values the guest streamed a body into: rep → body. The body is
    /// shared with the stream consumer that fills it while the guest writes.
    blob_outgoing: Vec<Arc<Mutex<Vec<u8>>>>,
    /// Incoming values a `get-data` answered with: rep → body.
    blob_incoming: Vec<Vec<u8>>,
    /// Database host calls (`golem:rdbms`) the agent made, for `--expect-db`.
    db_calls: Vec<String>,
    /// The stub's relational store: the committed `(id, note)` rows, and one
    /// copy per open transaction (a transaction's rep indexes this list).
    db_rows: Vec<(i32, String)>,
    db_transactions: Vec<Vec<(i32, String)>>,
    /// The next connection rep; connections carry no state of their own.
    db_connections: u32,
    /// The postgres `lazy-db-value` / `lazy-db-column-type` resources the stub
    /// handed out, by rep.
    lazy_db_values: Vec<golem::rdbms::postgres::DbValue>,
    lazy_db_column_types: Vec<golem::rdbms::postgres::DbColumnType>,
    /// Filesystem host calls (`wasi:filesystem@0.2.6`) the agent made, for
    /// `--expect-fs`.
    fs_calls: Vec<String>,
    /// The harness's own agent filesystem: a directory path to its entries. The
    /// root is `""`, and paths are relative to it — the spelling the p2 calls
    /// use once the component has resolved the app's path against the root.
    fs_entries: BTreeMap<String, BTreeMap<String, StubKind>>,
    /// Contents of the stub filesystem's regular files, by path.
    fs_contents: BTreeMap<String, Vec<u8>>,
    /// Its open descriptors: rep → what the descriptor names.
    fs_descriptors: Vec<StubDescriptor>,
    /// Directory listings a `read-directory` started: rep → the entries still
    /// to hand out, in the order a `read-directory-entry` answers them.
    fs_listings: Vec<std::collections::VecDeque<(String, StubKind)>>,
    /// HTTP state used by the `wasi:http` stubs in `stubs.rs`.
    fields_table: Vec<std::collections::HashMap<String, Vec<Vec<u8>>>>,
    requests_table: Vec<StubRequest>,
    body_slots: Vec<()>,
    request_body: Vec<u8>,
    response_body: Vec<u8>,
    response_cursor: usize,
    /// Request lines the HTTP stubs saw, for `--expect-http`.
    http_request_lines: Vec<String>,
}

/// An outgoing request as recorded by the `wasi:http` stubs.
#[derive(Clone, Default)]
pub struct StubRequest {
    pub method: String,
    pub scheme: String,
    pub authority: String,
    pub path: String,
}

/// Name of a `wasi:http/types` method, as it appears on the wire.
pub fn method_name(method: &wasi::http0_2_6::types::Method) -> String {
    match method {
        wasi::http0_2_6::types::Method::Get => "GET",
        wasi::http0_2_6::types::Method::Head => "HEAD",
        wasi::http0_2_6::types::Method::Post => "POST",
        wasi::http0_2_6::types::Method::Put => "PUT",
        wasi::http0_2_6::types::Method::Delete => "DELETE",
        wasi::http0_2_6::types::Method::Connect => "CONNECT",
        wasi::http0_2_6::types::Method::Options => "OPTIONS",
        wasi::http0_2_6::types::Method::Trace => "TRACE",
        wasi::http0_2_6::types::Method::Patch => "PATCH",
        wasi::http0_2_6::types::Method::Other(other) => other.as_str(),
    }
    .to_string()
}

/// A stand-in WebSocket connection that records what the agent sends.
struct StubConnection {
    sent: Vec<String>,
    closed: bool,
}

/// What the stub filesystem holds at a path. The effects never create a symlink,
/// so the stub does not model one.
#[derive(Clone, Copy, PartialEq)]
pub enum StubKind {
    File,
    Directory,
}

/// An open descriptor of the stub filesystem. The root is `""` (the path Golem
/// preopens the agent's sandbox under), and every other path is relative to it.
#[derive(Clone)]
pub struct StubDescriptor {
    pub path: String,
    pub directory: bool,
}

/// Splits a stub filesystem path into its parent directory and last segment. The
/// root (`""`) has no parent.
fn fs_split(path: &str) -> Option<(String, String)> {
    if path.is_empty() {
        return None;
    }
    match path.rsplit_once('/') {
        Some((parent, name)) => Some((parent.to_string(), name.to_string())),
        None => Some((String::new(), path.to_string())),
    }
}

impl Ctx {
    /// What the stub filesystem holds at `path`; the root is always there.
    fn fs_kind(&self, path: &str) -> Option<StubKind> {
        if path.is_empty() {
            return Some(StubKind::Directory);
        }
        let (parent, name) = fs_split(path)?;
        self.fs_entries.get(&parent)?.get(&name).copied()
    }

    fn fs_insert(&mut self, path: &str, kind: StubKind) {
        if let Some((parent, name)) = fs_split(path) {
            self.fs_entries.entry(parent).or_default().insert(name, kind);
        }
    }

    fn fs_remove(&mut self, path: &str) -> Option<StubKind> {
        let (parent, name) = fs_split(path)?;
        self.fs_entries.get_mut(&parent)?.remove(&name)
    }

    /// A new descriptor of the stub filesystem at `path`.
    fn fs_descriptor(&mut self, path: String, directory: bool) -> Resource<Descriptor> {
        let rep = self.fs_descriptors.len() as u32;
        self.fs_descriptors.push(StubDescriptor { path, directory });
        unsafe { Resource::new_own(rep) }
    }

    fn fs_descriptor_at(&self, rep: Resource<Descriptor>) -> Option<&StubDescriptor> {
        self.fs_descriptors.get(rep.rep() as usize)
    }

    /// The path a descriptor-relative call names, joined the way the p2
    /// interface resolves a relative path (the component already applied `.`,
    /// `..` and the leading `/`).
    fn fs_join(&self, rep: Resource<Descriptor>, path: &str) -> Option<String> {
        let descriptor = self.fs_descriptor_at(rep)?;
        Some(if descriptor.path.is_empty() {
            path.to_string()
        } else {
            format!("{}/{}", descriptor.path, path)
        })
    }
}

/// The `descriptor-type` the stub reports for one of its entries.
pub fn fs_descriptor_type(kind: StubKind) -> DescriptorType {
    match kind {
        StubKind::File => DescriptorType::RegularFile,
        StubKind::Directory => DescriptorType::Directory,
    }
}

/// A `descriptor-stat` of an entry of that size; the timestamps the stub does not
/// model are absent.
fn fs_stat(kind: StubKind, size: u64) -> DescriptorStat {
    DescriptorStat {
        type_: fs_descriptor_type(kind),
        link_count: 1,
        size,
        data_access_timestamp: None,
        data_modification_timestamp: None,
        status_change_timestamp: None,
    }
}

impl wasmtime::component::HasData for Ctx {
    type Data<'a> = &'a mut Ctx;
}

impl golem::core::types::Host for Ctx {
    fn parse_uuid(&mut self, _s: String) -> Result<golem::core::types::Uuid, String> {
        Err(String::from("parse-uuid is not used by roc-golem components"))
    }
    fn uuid_to_string(&mut self, _u: golem::core::types::Uuid) -> String {
        unimplemented!("uuid-to-string is not used by roc-golem components")
    }
}

// Capability resources a component may import but roc-golem agents do not use
// yet: the harness only needs them to link.
impl golem::core::types::HostQuotaToken for Ctx {
    fn drop(
        &mut self,
        _rep: wasmtime::component::Resource<golem::core::types::QuotaToken>,
    ) -> wasmtime::Result<()> {
        Ok(())
    }
}

/// The quota interface, stubbed: every call is recorded so a check can see what
/// the component asked the host for. The tokens and reservations the agent
/// holds are plain rep numbers — nothing here enforces a limit, and a
/// reservation is never rejected (the e2e run is where a real resource's
/// `enforcementAction` shows up).
impl golem::quota::types::Host for Ctx {
    fn new_token(
        &mut self,
        resource_name: String,
        expected_use: u64,
    ) -> wasmtime::component::Resource<golem::quota::types::QuotaToken> {
        let rep = self.quota_calls.len() as u32 + 1;
        self.quota_calls.push(format!(
            "new-token({resource_name}, expected-use={expected_use})"
        ));
        wasmtime::component::Resource::new_own(rep)
    }

    fn reserve(
        &mut self,
        _token: wasmtime::component::Resource<golem::quota::types::QuotaToken>,
        amount: u64,
    ) -> Result<
        wasmtime::component::Resource<golem::quota::types::Reservation>,
        golem::quota::types::FailedReservation,
    > {
        let rep = self.quota_calls.len() as u32 + 1;
        self.quota_calls.push(format!("reserve(amount={amount})"));
        Ok(wasmtime::component::Resource::new_own(rep))
    }

    fn split(
        &mut self,
        _token: wasmtime::component::Resource<golem::quota::types::QuotaToken>,
        child_expected_use: u64,
    ) -> wasmtime::component::Resource<golem::quota::types::QuotaToken> {
        let rep = self.quota_calls.len() as u32 + 1;
        self.quota_calls
            .push(format!("split(child-expected-use={child_expected_use})"));
        wasmtime::component::Resource::new_own(rep)
    }

    fn merge(
        &mut self,
        _token: wasmtime::component::Resource<golem::quota::types::QuotaToken>,
        _other: wasmtime::component::Resource<golem::quota::types::QuotaToken>,
    ) {
        self.quota_calls.push(String::from("merge()"));
    }
}

impl golem::quota::types::HostReservation for Ctx {
    fn commit(
        &mut self,
        _this: wasmtime::component::Resource<golem::quota::types::Reservation>,
        used: u64,
    ) {
        self.quota_calls.push(format!("commit(used={used})"));
    }

    fn drop(
        &mut self,
        _rep: wasmtime::component::Resource<golem::quota::types::Reservation>,
    ) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl golem::core::types::HostSecret for Ctx {
    fn drop(
        &mut self,
        _rep: wasmtime::component::Resource<golem::core::types::Secret>,
    ) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl golem::core::types::HostPermissionCard for Ctx {
    fn drop(
        &mut self,
        _rep: wasmtime::component::Resource<golem::core::types::PermissionCard>,
    ) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl golem::core::types::HostSchemaValueStream for Ctx {
    fn drop(
        &mut self,
        _rep: wasmtime::component::Resource<golem::core::types::SchemaValueStream>,
    ) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl golem::core::types::HostSchemaValueStreamWithStore<Ctx> for Ctx {
    fn wrap(
        _accessor: &wasmtime::component::Accessor<Ctx, Ctx>,
        _reader: wasmtime::component::StreamReader<SchemaValueTree>,
    ) -> impl std::future::Future<
        Output = wasmtime::component::Resource<golem::core::types::SchemaValueStream>,
    > + Send {
        async move { unsafe { wasmtime::component::Resource::new_own(0) } }
    }

    fn unwrap(
        _accessor: &wasmtime::component::Accessor<Ctx, Ctx>,
        _value: wasmtime::component::Resource<golem::core::types::SchemaValueStream>,
    ) -> impl std::future::Future<
        Output = wasmtime::component::StreamReader<SchemaValueTree>,
    > + Send {
        async move { unimplemented!("schema-value-stream is not used by roc-golem components") }
    }
}

impl golem::agent::common::Host for Ctx {}

impl golem::tool::common::Host for Ctx {}

impl golem::tool::streams::Host for Ctx {}

impl golem::tool::streams::HostToolOutputWriter for Ctx {
    fn drop(
        &mut self,
        _rep: wasmtime::component::Resource<golem::tool::streams::ToolOutputWriter>,
    ) -> wasmtime::Result<()> {
        Ok(())
    }
}

/// The writer's operations are async imports; the stub accepts everything.
impl golem::tool::streams::HostToolOutputWriterWithStore<Ctx> for Ctx {
    fn write(
        _accessor: &wasmtime::component::Accessor<Ctx, Ctx>,
        _self_: wasmtime::component::Resource<golem::tool::streams::ToolOutputWriter>,
        _bytes: Vec<u8>,
    ) -> impl std::future::Future<
        Output = Result<(), golem::tool::streams::StreamWriteError>,
    > + Send {
        async move { Ok(()) }
    }

    fn finish(
        _accessor: &wasmtime::component::Accessor<Ctx, Ctx>,
        _self_: wasmtime::component::Resource<golem::tool::streams::ToolOutputWriter>,
    ) -> impl std::future::Future<
        Output = Result<(), golem::tool::streams::StreamWriteError>,
    > + Send {
        async move { Ok(()) }
    }

    fn fail(
        _accessor: &wasmtime::component::Accessor<Ctx, Ctx>,
        _self_: wasmtime::component::Resource<golem::tool::streams::ToolOutputWriter>,
        _reason: golem::tool::streams::ByteStreamFailure,
    ) -> impl std::future::Future<
        Output = Result<(), golem::tool::streams::StreamWriteError>,
    > + Send {
        async move { Ok(()) }
    }
}

impl wasi::clocks0_3_0::types::Host for Ctx {}

impl wasi::clocks0_3_0::system_clock::Host for Ctx {
    fn now(&mut self) -> wasi::clocks0_3_0::system_clock::Instant {
        wasi::clocks0_3_0::system_clock::Instant {
            seconds: 0,
            nanoseconds: 0,
        }
    }

    fn get_resolution(&mut self) -> wasi::clocks0_3_0::system_clock::Duration {
        1_000_000
    }
}

impl wasi::clocks0_2_6::monotonic_clock::Host for Ctx {
    fn now(&mut self) -> u64 {
        0
    }
    fn resolution(&mut self) -> u64 {
        1_000_000
    }
    fn subscribe_duration(
        &mut self,
        _when: u64,
    ) -> wasmtime::component::Resource<wasi::io::poll::Pollable> {
        unsafe { wasmtime::component::Resource::new_own(0) }
    }
    fn subscribe_instant(
        &mut self,
        _when: u64,
    ) -> wasmtime::component::Resource<wasi::io::poll::Pollable> {
        unsafe { wasmtime::component::Resource::new_own(0) }
    }
}

impl wasi::clocks0_2_6::wall_clock::Host for Ctx {
    fn now(&mut self) -> wasi::clocks0_2_6::wall_clock::Datetime {
        wasi::clocks0_2_6::wall_clock::Datetime {
            seconds: 0,
            nanoseconds: 0,
        }
    }
    fn resolution(&mut self) -> wasi::clocks0_2_6::wall_clock::Datetime {
        wasi::clocks0_2_6::wall_clock::Datetime {
            seconds: 1,
            nanoseconds: 0,
        }
    }
}

impl wasi::io::poll::Host for Ctx {
    fn poll(
        &mut self,
        _in: Vec<wasmtime::component::Resource<wasi::io::poll::Pollable>>,
    ) -> Vec<u32> {
        Vec::new()
    }
}

impl wasi::io::poll::HostPollable for Ctx {
    fn ready(&mut self, _self_: wasmtime::component::Resource<wasi::io::poll::Pollable>) -> bool {
        true
    }
    fn block(&mut self, _self_: wasmtime::component::Resource<wasi::io::poll::Pollable>) {}
    fn drop(
        &mut self,
        _rep: wasmtime::component::Resource<wasi::io::poll::Pollable>,
    ) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl wasi::logging::logging::Host for Ctx {
    fn log(
        &mut self,
        level: wasi::logging::logging::Level,
        context: String,
        message: String,
    ) {
        self.logs.push(format!("{:?} {} {}", level, context, message));
    }
}

impl golem::websocket::client::Host for Ctx {}

impl golem::websocket::client::HostWebsocketConnection for Ctx {
    fn connect(
        &mut self,
        url: String,
        _headers: Option<Vec<(String, String)>>,
        // Golem 1.6.0 added the connection's reconstruction policy; the stub
        // reconnects implicitly, so the policy is not modelled here.
        _reconstruction_policy: Option<golem::websocket::client::ReconstructionPolicy>,
    ) -> Result<
        wasmtime::component::Resource<golem::websocket::client::WebsocketConnection>,
        golem::websocket::client::Error,
    > {
        self.logs.push(format!("CONNECT {url}"));
        let rep = self.connections.len() as u32;
        self.connections.push(StubConnection {
            sent: Vec::new(),
            closed: false,
        });
        // The stub tracks connection state itself, so the handle only needs to
        // carry the table index.
        Ok(unsafe {
            wasmtime::component::Resource::new_own(rep)
        })
    }

    fn send(
        &mut self,
        self_: wasmtime::component::Resource<golem::websocket::client::WebsocketConnection>,
        message: golem::websocket::client::Message,
    ) -> Result<(), golem::websocket::client::Error> {
        let text = match message {
            golem::websocket::client::Message::Text(text) => text,
            golem::websocket::client::Message::Binary(bytes) => {
                String::from_utf8_lossy(&bytes).into()
            }
        };
        self.sent.push(text.clone());
        match self.connections.get_mut(self_.rep() as usize) {
            Some(connection) => {
                connection.sent.push(text);
                Ok(())
            }
            None => Err(golem::websocket::client::Error::Other(format!(
                "unknown connection handle {}",
                self_.rep()
            ))),
        }
    }

    fn close(
        &mut self,
        self_: wasmtime::component::Resource<golem::websocket::client::WebsocketConnection>,
        _code: Option<u16>,
        _reason: Option<String>,
    ) -> Result<(), golem::websocket::client::Error> {
        match self.connections.get_mut(self_.rep() as usize) {
            Some(connection) => {
                connection.closed = true;
                Ok(())
            }
            None => Err(golem::websocket::client::Error::Other(format!(
                "unknown connection handle {}",
                self_.rep()
            ))),
        }
    }

    fn drop(
        &mut self,
        rep: wasmtime::component::Resource<golem::websocket::client::WebsocketConnection>,
    ) -> wasmtime::Result<()> {
        let index = rep.rep() as usize;
        if let Some(connection) = self.connections.get_mut(index) {
            connection.closed = true;
        }
        Ok(())
    }
}

/// The async websocket imports live in the `WithStore` companion trait; the
/// stub answers immediately with the canned message the old sync stub used.
impl golem::websocket::client::HostWebsocketConnectionWithStore<Ctx> for Ctx {
    fn receive(
        _accessor: &wasmtime::component::Accessor<Ctx, Ctx>,
        _self_: wasmtime::component::Resource<golem::websocket::client::WebsocketConnection>,
    ) -> impl std::future::Future<
        Output = Result<golem::websocket::client::Message, golem::websocket::client::Error>,
    > + Send {
        async move {
            Ok(golem::websocket::client::Message::Text(String::from(
                "stub-reply",
            )))
        }
    }

    fn receive_with_timeout(
        _accessor: &wasmtime::component::Accessor<Ctx, Ctx>,
        _self_: wasmtime::component::Resource<golem::websocket::client::WebsocketConnection>,
        _timeout_ms: u64,
    ) -> impl std::future::Future<
        Output = Result<
            Option<golem::websocket::client::Message>,
            golem::websocket::client::Error,
        >,
    > + Send {
        async move {
            Ok(Some(golem::websocket::client::Message::Text(
                String::from("stub-reply"),
            )))
        }
    }
}

// ---------------------------------------------------------------------------
// Key-value (`wasi:keyvalue`) and blobstore (`wasi:blobstore`) stubs
// ---------------------------------------------------------------------------
//
// On a real server both stores are shared by the whole environment and durable;
// here each one is a plain in-memory map, and every call is recorded so a check
// can see what the component asked the host for. A round trip driven by the
// component has to come back out of *this* store changed, which is what makes
// the checks more than "the call was made".

impl Ctx {
    /// The bucket a handle names; the effects open the bucket first, so a rep
    /// this stub never handed out is not reachable through them.
    fn kv_bucket_name(&self, rep: u32) -> String {
        self.kv_buckets
            .get(rep as usize)
            .cloned()
            .unwrap_or_else(|| String::from("<unknown>"))
    }

    /// The container a handle names, and the same caveat as `kv_bucket_name`.
    fn blob_container_name(&self, rep: u32) -> String {
        self.blob_container_reps
            .get(rep as usize)
            .cloned()
            .unwrap_or_else(|| String::from("<unknown>"))
    }

    /// What `blobPut!` streamed into this outgoing value (empty for a handle
    /// that was never written to).
    fn blob_outgoing_body(&self, rep: u32) -> Vec<u8> {
        self.blob_outgoing
            .get(rep as usize)
            .map(|body| body.lock().unwrap().clone())
            .unwrap_or_default()
    }
}

/// `wasi:keyvalue/wasi-keyvalue-error`: the error resource, which the effects
/// render with `trace`. Nothing in this harness raises one.
impl wasi::keyvalue::wasi_keyvalue_error::HostError for Ctx {
    fn trace(
        &mut self,
        _self_: wasmtime::component::Resource<wasi::keyvalue::wasi_keyvalue_error::Error>,
    ) -> String {
        String::from("stub key-value error")
    }

    fn drop(
        &mut self,
        _rep: wasmtime::component::Resource<wasi::keyvalue::wasi_keyvalue_error::Error>,
    ) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl wasi::keyvalue::wasi_keyvalue_error::Host for Ctx {}

/// Buckets are implicit in the host, and here too: opening one only hands back
/// a handle to the name.
impl wasi::keyvalue::types::HostBucket for Ctx {
    fn open_bucket(
        &mut self,
        name: String,
    ) -> Result<
        wasmtime::component::Resource<wasi::keyvalue::types::Bucket>,
        wasmtime::component::Resource<wasi::keyvalue::wasi_keyvalue_error::Error>,
    > {
        self.kv_calls.push(format!("open-bucket(name={name})"));
        let rep = self.kv_buckets.len() as u32;
        self.kv_buckets.push(name);
        Ok(unsafe { wasmtime::component::Resource::new_own(rep) })
    }

    fn drop(
        &mut self,
        _rep: wasmtime::component::Resource<wasi::keyvalue::types::Bucket>,
    ) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl wasi::keyvalue::types::HostOutgoingValue for Ctx {
    fn new_outgoing_value(&mut self) -> wasmtime::component::Resource<wasi::keyvalue::types::OutgoingValue> {
        let rep = self.kv_outgoing.len() as u32;
        self.kv_outgoing.push(Vec::new());
        unsafe { wasmtime::component::Resource::new_own(rep) }
    }

    fn outgoing_value_write_body_async(
        &mut self,
        _self_: wasmtime::component::Resource<wasi::keyvalue::types::OutgoingValue>,
        _data: StreamReader<u8>,
    ) -> Result<(), wasmtime::component::Resource<wasi::keyvalue::wasi_keyvalue_error::Error>> {
        unimplemented!("harness stub: wasi:keyvalue/types.outgoing-value-write-body-async")
    }

    fn outgoing_value_write_body_sync(
        &mut self,
        self_: wasmtime::component::Resource<wasi::keyvalue::types::OutgoingValue>,
        value: Vec<u8>,
    ) -> Result<(), wasmtime::component::Resource<wasi::keyvalue::wasi_keyvalue_error::Error>> {
        if let Some(body) = self.kv_outgoing.get_mut(self_.rep() as usize) {
            body.extend_from_slice(&value);
        }
        Ok(())
    }

    fn drop(
        &mut self,
        _rep: wasmtime::component::Resource<wasi::keyvalue::types::OutgoingValue>,
    ) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl wasi::keyvalue::types::HostIncomingValue for Ctx {
    fn incoming_value_consume_sync(
        &mut self,
        self_: wasmtime::component::Resource<wasi::keyvalue::types::IncomingValue>,
    ) -> Result<Vec<u8>, wasmtime::component::Resource<wasi::keyvalue::wasi_keyvalue_error::Error>> {
        let rep = self_.rep() as usize;
        Ok(match self.kv_incoming.get_mut(rep) {
            Some(body) => std::mem::take(body),
            None => Vec::new(),
        })
    }

    fn incoming_value_consume_async(
        &mut self,
        _self_: wasmtime::component::Resource<wasi::keyvalue::types::IncomingValue>,
    ) -> Result<StreamReader<u8>, wasmtime::component::Resource<wasi::keyvalue::wasi_keyvalue_error::Error>> {
        unimplemented!("harness stub: wasi:keyvalue/types.incoming-value-consume-async")
    }

    fn incoming_value_size(
        &mut self,
        self_: wasmtime::component::Resource<wasi::keyvalue::types::IncomingValue>,
    ) -> Result<u64, wasmtime::component::Resource<wasi::keyvalue::wasi_keyvalue_error::Error>> {
        Ok(self
            .kv_incoming
            .get(self_.rep() as usize)
            .map(|body| body.len() as u64)
            .unwrap_or(0))
    }

    fn drop(
        &mut self,
        _rep: wasmtime::component::Resource<wasi::keyvalue::types::IncomingValue>,
    ) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl wasi::keyvalue::types::Host for Ctx {}

/// The `eventual` CRUD surface, over the harness's own map.
impl wasi::keyvalue::eventual::Host for Ctx {
    fn get(
        &mut self,
        bucket: wasmtime::component::Resource<wasi::keyvalue::types::Bucket>,
        key: String,
    ) -> Result<
        Option<wasmtime::component::Resource<wasi::keyvalue::types::IncomingValue>>,
        wasmtime::component::Resource<wasi::keyvalue::wasi_keyvalue_error::Error>,
    > {
        let bucket = self.kv_bucket_name(bucket.rep());
        self.kv_calls.push(format!("get(bucket={bucket}, key={key})"));
        match self.kv_store.get(&(bucket, key)) {
            Some(value) => {
                let rep = self.kv_incoming.len() as u32;
                self.kv_incoming.push(value.clone());
                Ok(Some(unsafe { wasmtime::component::Resource::new_own(rep) }))
            }
            None => Ok(None),
        }
    }

    fn set(
        &mut self,
        bucket: wasmtime::component::Resource<wasi::keyvalue::types::Bucket>,
        key: String,
        outgoing_value: wasmtime::component::Resource<wasi::keyvalue::types::OutgoingValue>,
    ) -> Result<(), wasmtime::component::Resource<wasi::keyvalue::wasi_keyvalue_error::Error>> {
        let bucket = self.kv_bucket_name(bucket.rep());
        let value = self
            .kv_outgoing
            .get(outgoing_value.rep() as usize)
            .cloned()
            .unwrap_or_default();
        self.kv_calls
            .push(format!("set(bucket={bucket}, key={key}, {} bytes)", value.len()));
        self.kv_store.insert((bucket, key), value);
        Ok(())
    }

    fn delete(
        &mut self,
        bucket: wasmtime::component::Resource<wasi::keyvalue::types::Bucket>,
        key: String,
    ) -> Result<(), wasmtime::component::Resource<wasi::keyvalue::wasi_keyvalue_error::Error>> {
        let bucket = self.kv_bucket_name(bucket.rep());
        self.kv_calls.push(format!("delete(bucket={bucket}, key={key})"));
        self.kv_store.remove(&(bucket, key));
        Ok(())
    }

    fn exists(
        &mut self,
        bucket: wasmtime::component::Resource<wasi::keyvalue::types::Bucket>,
        key: String,
    ) -> Result<bool, wasmtime::component::Resource<wasi::keyvalue::wasi_keyvalue_error::Error>> {
        let bucket = self.kv_bucket_name(bucket.rep());
        self.kv_calls.push(format!("exists(bucket={bucket}, key={key})"));
        Ok(self.kv_store.contains_key(&(bucket, key)))
    }
}

/// The container-level blobstore surface. A container that was never created is
/// missing here exactly as it is on the host, `get-container` included: the
/// real one answers `Container not found`.
impl wasi::blobstore::blobstore::Host for Ctx {
    fn create_container(
        &mut self,
        name: String,
    ) -> Result<wasmtime::component::Resource<wasi::blobstore::container::Container>, String> {
        self.blob_calls.push(format!("create-container(name={name})"));
        self.blob_containers.entry(name.clone()).or_default();
        let rep = self.blob_container_reps.len() as u32;
        self.blob_container_reps.push(name);
        Ok(unsafe { wasmtime::component::Resource::new_own(rep) })
    }

    fn get_container(
        &mut self,
        name: String,
    ) -> Result<wasmtime::component::Resource<wasi::blobstore::container::Container>, String> {
        self.blob_calls.push(format!("get-container(name={name})"));
        if !self.blob_containers.contains_key(&name) {
            return Err(String::from("Container not found"));
        }
        let rep = self.blob_container_reps.len() as u32;
        self.blob_container_reps.push(name);
        Ok(unsafe { wasmtime::component::Resource::new_own(rep) })
    }

    fn delete_container(&mut self, name: String) -> Result<(), String> {
        self.blob_calls.push(format!("delete-container(name={name})"));
        self.blob_containers.remove(&name);
        Ok(())
    }

    fn container_exists(&mut self, name: String) -> Result<bool, String> {
        self.blob_calls.push(format!("container-exists(name={name})"));
        Ok(self.blob_containers.contains_key(&name))
    }

    fn copy_object(
        &mut self,
        _src: wasi::blobstore::types::ObjectId,
        _dest: wasi::blobstore::types::ObjectId,
    ) -> Result<(), String> {
        unimplemented!("harness stub: wasi:blobstore/blobstore.copy-object")
    }

    fn move_object(
        &mut self,
        _src: wasi::blobstore::types::ObjectId,
        _dest: wasi::blobstore::types::ObjectId,
    ) -> Result<(), String> {
        unimplemented!("harness stub: wasi:blobstore/blobstore.move-object")
    }
}

impl wasi::blobstore::container::HostContainer for Ctx {
    fn name(
        &mut self,
        self_: wasmtime::component::Resource<wasi::blobstore::container::Container>,
    ) -> Result<String, String> {
        Ok(self.blob_container_name(self_.rep()))
    }

    fn info(
        &mut self,
        self_: wasmtime::component::Resource<wasi::blobstore::container::Container>,
    ) -> Result<wasi::blobstore::types::ContainerMetadata, String> {
        Ok(wasi::blobstore::types::ContainerMetadata {
            name: self.blob_container_name(self_.rep()),
            created_at: 0,
        })
    }

    fn get_data(
        &mut self,
        self_: wasmtime::component::Resource<wasi::blobstore::container::Container>,
        name: String,
        start: u64,
        end: u64,
    ) -> Result<wasmtime::component::Resource<wasi::blobstore::types::IncomingValue>, String>
    {
        let container = self.blob_container_name(self_.rep());
        self.blob_calls.push(format!(
            "get-data(container={container}, object={name}, {start}..={end})"
        ));
        let object = self
            .blob_containers
            .get(&container)
            .and_then(|objects| objects.get(&name))
            .cloned()
            .ok_or_else(|| String::from("Object does not exist"))?;
        // The store's rule, inclusive on both ends: the range has to sit inside
        // the object (an empty object has no range at all).
        if end < start || end as usize >= object.len() {
            return Err(format!(
                "no bytes {start}..={end} in the {} bytes of {name:?}",
                object.len()
            ));
        }
        let rep = self.blob_incoming.len() as u32;
        self.blob_incoming
            .push(object[start as usize..=end as usize].to_vec());
        Ok(unsafe { wasmtime::component::Resource::new_own(rep) })
    }

    fn write_data(
        &mut self,
        self_: wasmtime::component::Resource<wasi::blobstore::container::Container>,
        name: String,
        data: wasmtime::component::Resource<wasi::blobstore::types::OutgoingValue>,
    ) -> Result<(), String> {
        let container = self.blob_container_name(self_.rep());
        let body = self.blob_outgoing_body(data.rep());
        self.blob_calls.push(format!(
            "write-data(container={container}, object={name}, {} bytes)",
            body.len()
        ));
        self.blob_containers
            .entry(container)
            .or_default()
            .insert(name, body);
        Ok(())
    }

    fn delete_object(
        &mut self,
        self_: wasmtime::component::Resource<wasi::blobstore::container::Container>,
        name: String,
    ) -> Result<(), String> {
        let container = self.blob_container_name(self_.rep());
        self.blob_calls
            .push(format!("delete-object(container={container}, object={name})"));
        if let Some(objects) = self.blob_containers.get_mut(&container) {
            objects.remove(&name);
        }
        Ok(())
    }

    fn delete_objects(
        &mut self,
        _self_: wasmtime::component::Resource<wasi::blobstore::container::Container>,
        _names: Vec<String>,
    ) -> Result<(), String> {
        unimplemented!("harness stub: wasi:blobstore/container.delete-objects")
    }

    fn has_object(
        &mut self,
        self_: wasmtime::component::Resource<wasi::blobstore::container::Container>,
        name: String,
    ) -> Result<bool, String> {
        let container = self.blob_container_name(self_.rep());
        self.blob_calls
            .push(format!("has-object(container={container}, object={name})"));
        Ok(self
            .blob_containers
            .get(&container)
            .map(|objects| objects.contains_key(&name))
            .unwrap_or(false))
    }

    fn object_info(
        &mut self,
        self_: wasmtime::component::Resource<wasi::blobstore::container::Container>,
        name: String,
    ) -> Result<wasi::blobstore::types::ObjectMetadata, String> {
        let container = self.blob_container_name(self_.rep());
        let size = self
            .blob_containers
            .get(&container)
            .and_then(|objects| objects.get(&name))
            .map(|body| body.len() as u64)
            .ok_or_else(|| String::from("Object does not exist"))?;
        self.blob_calls.push(format!(
            "object-info(container={container}, object={name}, {size} bytes)"
        ));
        Ok(wasi::blobstore::types::ObjectMetadata {
            name,
            container,
            created_at: 0,
            size,
        })
    }

    fn clear(
        &mut self,
        self_: wasmtime::component::Resource<wasi::blobstore::container::Container>,
    ) -> Result<(), String> {
        let container = self.blob_container_name(self_.rep());
        self.blob_calls.push(format!("clear(container={container})"));
        if let Some(objects) = self.blob_containers.get_mut(&container) {
            objects.clear();
        }
        Ok(())
    }

    fn drop(
        &mut self,
        _rep: wasmtime::component::Resource<wasi::blobstore::container::Container>,
    ) -> wasmtime::Result<()> {
        Ok(())
    }
}

/// Returning a stream needs the store, so `list-objects` is one of the two
/// functions this harness marks `store`. The names are served from the harness's
/// own container, and the whole stream is available immediately.
impl wasi::blobstore::container::HostContainerWithStore<Ctx> for Ctx {
    fn list_objects(
        mut host: Access<Ctx, Self>,
        self_: wasmtime::component::Resource<wasi::blobstore::container::Container>,
    ) -> wasmtime::Result<Result<StreamReader<String>, String>> {
        let container = host.get().blob_container_name(self_.rep());
        let names: Vec<String> = host
            .get()
            .blob_containers
            .get(&container)
            .map(|objects| objects.keys().cloned().collect())
            .unwrap_or_default();
        host.get()
            .blob_calls
            .push(format!("list-objects(container={container}, {} names)", names.len()));
        Ok(Ok(StreamReader::new(&mut host, names)?))
    }
}

impl wasi::blobstore::container::Host for Ctx {}

impl wasi::blobstore::types::HostOutgoingValue for Ctx {
    fn new_outgoing_value(&mut self) -> wasmtime::component::Resource<wasi::blobstore::types::OutgoingValue> {
        let rep = self.blob_outgoing.len() as u32;
        self.blob_outgoing.push(Arc::new(Mutex::new(Vec::new())));
        unsafe { wasmtime::component::Resource::new_own(rep) }
    }

    fn drop(
        &mut self,
        _rep: wasmtime::component::Resource<wasi::blobstore::types::OutgoingValue>,
    ) -> wasmtime::Result<()> {
        Ok(())
    }
}

/// The other stream-carrying function: the guest writes the object body into
/// this stream, and the consumer appends it to the outgoing value's body, which
/// `write-data` then copies into the harness's container.
impl wasi::blobstore::types::HostOutgoingValueWithStore<Ctx> for Ctx {
    fn outgoing_value_write_body(
        mut host: Access<Ctx, Self>,
        self_: wasmtime::component::Resource<wasi::blobstore::types::OutgoingValue>,
        data: StreamReader<u8>,
    ) -> wasmtime::Result<Result<(), ()>> {
        let Some(body) = host.get().blob_outgoing.get(self_.rep() as usize).cloned() else {
            return Ok(Err(()));
        };
        data.pipe(&mut host, ObjectBodyConsumer { body })?;
        Ok(Ok(()))
    }
}

/// Collects the bytes a guest streams into an outgoing value, the way the host
/// appends them to the value's body buffer.
struct ObjectBodyConsumer {
    body: Arc<Mutex<Vec<u8>>>,
}

impl StreamConsumer<Ctx> for ObjectBodyConsumer {
    type Item = u8;

    fn poll_consume(
        self: Pin<&mut Self>,
        _cx: &mut TaskContext<'_>,
        store: StoreContextMut<Ctx>,
        source: Source<'_, u8>,
        _finish: bool,
    ) -> Poll<wasmtime::Result<StreamResult>> {
        let mut source = source.as_direct(store);
        let bytes = source.remaining();
        if bytes.is_empty() {
            return Poll::Ready(Ok(StreamResult::Completed));
        }
        let len = bytes.len();
        self.body.lock().unwrap().extend_from_slice(bytes);
        source.mark_read(len);
        Poll::Ready(Ok(StreamResult::Completed))
    }
}

impl wasi::blobstore::types::HostIncomingValue for Ctx {
    fn incoming_value_consume_sync(
        &mut self,
        self_: wasmtime::component::Resource<wasi::blobstore::types::IncomingValue>,
    ) -> Result<Vec<u8>, String> {
        let rep = self_.rep() as usize;
        Ok(match self.blob_incoming.get_mut(rep) {
            Some(body) => std::mem::take(body),
            None => Vec::new(),
        })
    }

    fn incoming_value_consume_async(
        &mut self,
        _self_: wasmtime::component::Resource<wasi::blobstore::types::IncomingValue>,
    ) -> Result<StreamReader<u8>, String> {
        unimplemented!("harness stub: wasi:blobstore/types.incoming-value-consume-async")
    }

    fn size(
        &mut self,
        self_: wasmtime::component::Resource<wasi::blobstore::types::IncomingValue>,
    ) -> u64 {
        self.blob_incoming
            .get(self_.rep() as usize)
            .map(|body| body.len() as u64)
            .unwrap_or(0)
    }

    fn drop(
        &mut self,
        _rep: wasmtime::component::Resource<wasi::blobstore::types::IncomingValue>,
    ) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl wasi::blobstore::types::Host for Ctx {}

/// A stand-in for the host's RPC client, recording what the agent invoked.
struct StubRpc {
    target: String,
}

impl golem::agent::host::Host for Ctx {
    fn get_all_agent_types(&mut self) -> Vec<golem::agent::common::RegisteredAgentType> {
        Vec::new()
    }

    fn get_agent_type(
        &mut self,
        _agent_type_name: String,
    ) -> Option<golem::agent::common::RegisteredAgentType> {
        None
    }

    fn get_agent_type_by_agent_id(
        &mut self,
        _agent_id: String,
    ) -> Option<golem::agent::common::RegisteredAgentType> {
        None
    }

    fn make_agent_id(
        &mut self,
        agent_type_name: String,
        _input: SchemaValueTree,
        _phantom_id: Option<golem::core::types::Uuid>,
    ) -> Result<String, golem::agent::common::AgentError> {
        Ok(format!("{agent_type_name}(\"{{}}\")"))
    }

    fn parse_agent_id(
        &mut self,
        agent_id: String,
    ) -> Result<
        (
            String,
            golem::core::types::TypedSchemaValue,
            Option<golem::core::types::Uuid>,
        ),
        golem::agent::common::AgentError,
    > {
        // Accept whatever the agent passes and treat it as the stub target, so
        // the call reaches `invoke-and-await` below.
        Ok((
            agent_id,
            golem::core::types::TypedSchemaValue {
                graph: golem::core::types::SchemaGraph {
                    type_nodes: Vec::new(),
                    defs: Vec::new(),
                    root: 0,
                },
                value: text_value("{}"),
            },
            None,
        ))
    }

    fn create_webhook(
        &mut self,
        _promise_id: golem::core::types::PromiseId,
    ) -> Result<String, golem::agent::host::WebhookError> {
        Ok(String::from("http://stub/webhook"))
    }

    fn get_config_value(
        &mut self,
        _key: Vec<String>,
        _expected: golem::core::types::SchemaGraph,
    ) -> Result<SchemaValueTree, golem::agent::host::ConfigValueError> {
        Ok(SchemaValueTree {
            value_nodes: vec![SchemaValueNode::StringValue(String::from("stub-config"))],
            root: 0,
        })
    }
}

impl golem::agent::host::HostWasmRpc for Ctx {
    fn new(
        &mut self,
        agent_type_name: String,
        _constructor: SchemaValueTree,
        _phantom_id: Option<golem::core::types::Uuid>,
        _agent_config: Vec<golem::agent::common::TypedAgentConfigValue>,
    ) -> wasmtime::component::Resource<golem::agent::host::WasmRpc> {
        let rep = self.rpc_calls.len() as u32;
        self.rpc_calls.push(format!("new({agent_type_name})"));
        unsafe { wasmtime::component::Resource::new_own(rep) }
    }

    fn create(
        &mut self,
        agent_type_name: String,
        _constructor: SchemaValueTree,
        _phantom_id: Option<golem::core::types::Uuid>,
        _agent_config: Vec<golem::agent::common::TypedAgentConfigValue>,
    ) -> Result<wasmtime::component::Resource<golem::agent::host::WasmRpc>, golem::agent::host::RpcError>
    {
        let rep = self.rpc_calls.len() as u32;
        self.rpc_calls.push(format!("new({agent_type_name})"));
        Ok(unsafe { wasmtime::component::Resource::new_own(rep) })
    }

    fn invoke_and_await(
        &mut self,
        _self_: wasmtime::component::Resource<golem::agent::host::WasmRpc>,
        method_name: String,
        _input: SchemaValueTree,
        _scope_card: Option<wasmtime::component::Resource<golem::core::types::PermissionCard>>,
    ) -> Result<golem::agent::host::InvocationResultWithMetadata, golem::agent::host::RpcError> {
        self.rpc_calls.push(format!("invoke-and-await({method_name})"));
        Ok(golem::agent::host::InvocationResultWithMetadata {
            metadata: stub_invocation_metadata(),
            result: Some(SchemaValueTree {
                value_nodes: vec![SchemaValueNode::StringValue(String::from("stub-rpc-reply"))],
                root: 0,
            }),
        })
    }

    fn invoke(
        &mut self,
        _self_: wasmtime::component::Resource<golem::agent::host::WasmRpc>,
        method_name: String,
        _input: SchemaValueTree,
        _scope_card: Option<wasmtime::component::Resource<golem::core::types::PermissionCard>>,
    ) -> Result<golem::agent::host::InvocationMetadata, golem::agent::host::RpcError> {
        self.rpc_calls.push(format!("invoke({method_name})"));
        Ok(stub_invocation_metadata())
    }

    fn async_invoke_and_await(
        &mut self,
        _self_: wasmtime::component::Resource<golem::agent::host::WasmRpc>,
        method_name: String,
        _input: SchemaValueTree,
        _scope_card: Option<wasmtime::component::Resource<golem::core::types::PermissionCard>>,
    ) -> golem::agent::host::AsyncInvocationWithMetadata {
        self.rpc_calls
            .push(format!("async-invoke-and-await({method_name})"));
        golem::agent::host::AsyncInvocationWithMetadata {
            metadata: stub_invocation_metadata(),
            future: unsafe { wasmtime::component::Resource::new_own(0) },
        }
    }

    fn schedule_invocation(
        &mut self,
        _self_: wasmtime::component::Resource<golem::agent::host::WasmRpc>,
        _scheduled_time: golem::agent::host::Datetime,
        _method_name: String,
        _input: SchemaValueTree,
        _scope_card: Option<wasmtime::component::Resource<golem::core::types::PermissionCard>>,
    ) -> Result<golem::agent::host::ScheduledInvocationReceipt, golem::agent::host::RpcError> {
        unimplemented!("schedule-invocation is not used by roc-golem components")
    }

    fn schedule_cancelable_invocation(
        &mut self,
        _self_: wasmtime::component::Resource<golem::agent::host::WasmRpc>,
        _scheduled_time: golem::agent::host::Datetime,
        _method_name: String,
        _input: SchemaValueTree,
        _scope_card: Option<wasmtime::component::Resource<golem::core::types::PermissionCard>>,
    ) -> Result<
        golem::agent::host::CancelableScheduledInvocationReceipt,
        golem::agent::host::RpcError,
    > {
        unimplemented!("schedule-cancelable-invocation is not used by roc-golem components")
    }

    fn drop(
        &mut self,
        _rep: wasmtime::component::Resource<golem::agent::host::WasmRpc>,
    ) -> wasmtime::Result<()> {
        Ok(())
    }
}

fn stub_invocation_metadata() -> golem::agent::host::InvocationMetadata {
    golem::agent::host::InvocationMetadata {
        agent_id: String::from("stub-agent"),
        idempotency_key: String::from("stub-idempotency-key"),
    }
}

impl golem::agent::host::HostFutureInvokeResult for Ctx {
    fn cancel(
        &mut self,
        _self_: wasmtime::component::Resource<golem::agent::host::FutureInvokeResult>,
    ) {
    }

    fn drop(
        &mut self,
        _rep: wasmtime::component::Resource<golem::agent::host::FutureInvokeResult>,
    ) -> wasmtime::Result<()> {
        Ok(())
    }
}

/// The async half of `future-invoke-result` lives in the `WithStore` trait.
impl golem::agent::host::HostFutureInvokeResultWithStore<Ctx> for Ctx {
    fn get(
        _accessor: &wasmtime::component::Accessor<Ctx, Ctx>,
        _self_: wasmtime::component::Resource<golem::agent::host::FutureInvokeResult>,
    ) -> impl std::future::Future<
        Output = Result<Option<SchemaValueTree>, golem::agent::host::RpcError>,
    > + Send {
        async move { Ok(None) }
    }
}

impl golem::agent::host::HostCancellationToken for Ctx {
    fn cancel(&mut self, _self_: wasmtime::component::Resource<golem::agent::host::CancellationToken>) {}

    fn drop(
        &mut self,
        _rep: wasmtime::component::Resource<golem::agent::host::CancellationToken>,
    ) -> wasmtime::Result<()> {
        Ok(())
    }
}

/// The retry interface, stubbed. The environment would hold the named policies
/// and the host would evaluate them; the harness holds one canned policy and
/// records every call, so a check can see that the component read it, resolved
/// it and walked its schedule.
const STUB_RETRY_POLICY: &str = "harness-retry";

fn stub_named_policy() -> golem::api::retry::NamedRetryPolicy {
    golem::api::retry::NamedRetryPolicy {
        name: String::from(STUB_RETRY_POLICY),
        priority: 100,
        predicate: golem::api::retry::RetryPredicate {
            nodes: vec![golem::api::retry::PredicateNode::PropEq(
                golem::api::retry::PropertyComparison {
                    property_name: String::from("verb"),
                    value: golem::api::retry::PredicateValue::Text(String::from("send")),
                },
            )],
        },
        policy: golem::api::retry::RetryPolicy {
            nodes: vec![
                golem::api::retry::PolicyNode::CountBox(golem::api::retry::CountBoxConfig {
                    max_retries: 2,
                    inner: 1,
                }),
                golem::api::retry::PolicyNode::Periodic(100_000_000),
            ],
        },
    }
}

/// `property=value` pairs of a retry context, so a recording keeps its shape.
fn render_properties(
    properties: &[(String, golem::api::retry::PredicateValue)],
) -> String {
    properties
        .iter()
        .map(|(name, value)| {
            let rendered = match value {
                golem::api::retry::PredicateValue::Text(text) => text.clone(),
                golem::api::retry::PredicateValue::Integer(number) => number.to_string(),
                golem::api::retry::PredicateValue::Boolean(flag) => flag.to_string(),
            };
            format!("{name}={rendered}")
        })
        .collect::<Vec<_>>()
        .join(",")
}

impl golem::api::retry::Host for Ctx {
    fn get_retry_policies(&mut self) -> Vec<golem::api::retry::NamedRetryPolicy> {
        self.retry_calls.push(String::from("get-retry-policies"));
        vec![stub_named_policy()]
    }

    fn get_retry_policy_by_name(
        &mut self,
        name: String,
    ) -> Option<golem::api::retry::NamedRetryPolicy> {
        self.retry_calls.push(format!("get-retry-policy-by-name({name})"));
        (name == STUB_RETRY_POLICY).then(stub_named_policy)
    }

    fn resolve_retry_policy(
        &mut self,
        verb: String,
        noun_uri: String,
        properties: Vec<(String, golem::api::retry::PredicateValue)>,
    ) -> Result<Option<golem::api::retry::RetryPolicy>, String> {
        self.retry_calls.push(format!(
            "resolve(verb={verb}, noun-uri={noun_uri}, properties={})",
            render_properties(&properties)
        ));
        // The stub's policy is `propEq(verb, "send")`, so it applies to exactly
        // that verb.
        Ok((verb == "send").then(|| stub_named_policy().policy))
    }

    fn next_retry_delay(
        &mut self,
        policy: Option<golem::api::retry::RetryPolicy>,
        state: Vec<u8>,
        elapsed: golem::api::retry::Duration,
        verb: String,
        noun_uri: String,
        properties: Vec<(String, golem::api::retry::PredicateValue)>,
    ) -> Result<golem::api::retry::RetryDecision, String> {
        self.retry_calls.push(format!(
            "next-retry-delay(policy={} nodes, state={} bytes, elapsed={elapsed}ns, verb={verb}, noun-uri={noun_uri}, properties={})",
            policy.as_ref().map_or(0, |policy| policy.nodes.len()),
            state.len(),
            render_properties(&properties),
        ));
        Ok(golem::api::retry::RetryDecision {
            // Non-empty, so a continuation that the component dropped on the
            // way shows up as `state=0 bytes` on the next call.
            state: vec![1, 2, 3],
            delay: Some(100_000_000),
        })
    }

    fn set_retry_policy(&mut self, policy: golem::api::retry::NamedRetryPolicy) {
        self.retry_calls
            .push(format!("set-retry-policy({})", policy.name));
    }

    fn remove_retry_policy(&mut self, name: String) {
        self.retry_calls
            .push(format!("remove-retry-policy({name})"));
    }
}

// The `golem:rdbms` database stubs
//
// The three database interfaces are synchronous host imports, so the harness
// implements them: `open` answers a connection resource (and reports an address
// that is not a URL the way Golem does — `connection-failure`), a statement runs
// against a tiny relational store the stub keeps, and a transaction works on its
// own copy of that store until it commits (which publishes it) or rolls back
// (which drops it). It is not a SQL engine: it recognizes the statement shapes
// the counter's `db` command sends — `create table …`, `insert into …`,
// `select count(*) …`, `select … where id = $1`, `drop table …` — and answers
// with the column shapes postgres would, which is what makes the component's
// params and its rendering of the result a real round trip through the store.

/// One statement as the stub's store understands it.
enum StubSql {
    Create,
    Insert,
    Select,
    Count,
    Drop,
    Unknown,
}

fn stub_sql(statement: &str) -> StubSql {
    let normalized = statement.trim_start().to_ascii_lowercase();
    if normalized.starts_with("create table") {
        StubSql::Create
    } else if normalized.starts_with("insert into") {
        StubSql::Insert
    } else if normalized.starts_with("select count") {
        StubSql::Count
    } else if normalized.starts_with("select") {
        StubSql::Select
    } else if normalized.starts_with("drop table") {
        StubSql::Drop
    } else {
        StubSql::Unknown
    }
}

impl Ctx {
    /// The rows a statement on a connection (`None`) or on an open transaction
    /// sees: a transaction works on the copy it took when it began.
    fn db_visible_rows(&self, transaction: Option<u32>) -> Vec<(i32, String)> {
        match transaction {
            None => self.db_rows.clone(),
            Some(rep) => self
                .db_transactions
                .get(rep as usize)
                .cloned()
                .unwrap_or_default(),
        }
    }

    fn db_push_row(&mut self, transaction: Option<u32>, row: (i32, String)) {
        match transaction {
            None => self.db_rows.push(row),
            Some(rep) => {
                if let Some(rows) = self.db_transactions.get_mut(rep as usize) {
                    rows.push(row);
                }
            }
        }
    }

    fn db_begin(&mut self, engine: &str) -> u32 {
        self.db_calls.push(format!("{engine} begin"));
        let rep = self.db_transactions.len() as u32;
        self.db_transactions.push(self.db_rows.clone());
        rep
    }

    fn db_commit(&mut self, engine: &str, rep: u32) {
        self.db_calls.push(format!("{engine} commit"));
        if let Some(rows) = self.db_transactions.get_mut(rep as usize) {
            self.db_rows = std::mem::take(rows);
        }
    }

    fn db_rollback(&mut self, engine: &str, rep: u32) {
        self.db_calls.push(format!("{engine} rollback"));
        if let Some(rows) = self.db_transactions.get_mut(rep as usize) {
            rows.clear();
        }
    }

    /// Runs one `execute` against the stub's store. `row` is what an insert
    /// would add — the engine's own params decoded by its caller — and the
    /// classifier decides whether the statement is one the stub knows at all.
    fn db_execute(
        &mut self,
        transaction: Option<u32>,
        statement: &str,
        row: Option<(i32, String)>,
    ) -> Result<u64, String> {
        match stub_sql(statement) {
            StubSql::Create | StubSql::Drop => Ok(0),
            StubSql::Insert => match row {
                Some(row) => {
                    self.db_push_row(transaction, row);
                    Ok(1)
                }
                None => Err(String::from("an insert needs an int id and a text note")),
            },
            _ => Err(format!(
                "the harness stub does not execute the statement {statement:?}"
            )),
        }
    }

    /// Runs one `query` against the stub's store, answering `id`/`note` columns
    /// for a select and a `count` column for a count.
    fn db_query_rows(
        &mut self,
        transaction: Option<u32>,
        statement: &str,
        id: Option<i32>,
    ) -> Result<StubResult, String> {
        let rows = self.db_visible_rows(transaction);
        match stub_sql(statement) {
            StubSql::Count => Ok(StubResult::Count(rows.len())),
            StubSql::Select => Ok(StubResult::Rows(match id {
                Some(id) => rows.into_iter().filter(|(row_id, _)| *row_id == id).collect(),
                None => rows,
            })),
            StubSql::Unknown => Err(format!(
                "the harness stub does not know the statement {statement:?}"
            )),
            _ => Err(format!(
                "the statement {statement:?} answers no rows"
            )),
        }
    }
}

/// What a stub query answered: the count a `select count(*)` reports, or the
/// `(id, note)` rows a select reports.
enum StubResult {
    Count(usize),
    Rows(Vec<(i32, String)>),
}

fn stub_open(calls: &mut Vec<String>, engine: &str, address: &str) -> Result<(), String> {
    if !address.contains("://") {
        return Err(format!("invalid address {address:?}"));
    }
    calls.push(format!("{engine} open(address={address})"));
    Ok(())
}

impl golem::rdbms::postgres::Host for Ctx {}

impl golem::rdbms::postgres::HostDbConnection for Ctx {
    fn open(
        &mut self,
        address: String,
    ) -> Result<Resource<golem::rdbms::postgres::DbConnection>, golem::rdbms::postgres::Error>
    {
        stub_open(&mut self.db_calls, "postgres", &address)
            .map_err(golem::rdbms::postgres::Error::ConnectionFailure)?;
        let rep = self.db_connections;
        self.db_connections += 1;
        Ok(unsafe { Resource::new_own(rep) })
    }

    fn query(
        &mut self,
        _connection: Resource<golem::rdbms::postgres::DbConnection>,
        statement: String,
        params: Vec<golem::rdbms::postgres::DbValue>,
    ) -> Result<golem::rdbms::postgres::DbResult, golem::rdbms::postgres::Error> {
        self.db_calls.push(format!(
            "postgres query(statement={statement}, params={})",
            postgres_params_text(&params)
        ));
        let result = self.db_query_rows(None, &statement, postgres_param_id(&params));
        postgres_result(result)
    }

    fn query_stream(
        &mut self,
        _connection: Resource<golem::rdbms::postgres::DbConnection>,
        statement: String,
        _params: Vec<golem::rdbms::postgres::DbValue>,
    ) -> Result<Resource<golem::rdbms::postgres::DbResultStream>, golem::rdbms::postgres::Error>
    {
        Err(golem::rdbms::postgres::Error::Other(format!(
            "query-stream is not used by roc-golem components ({statement})"
        )))
    }

    fn execute(
        &mut self,
        _connection: Resource<golem::rdbms::postgres::DbConnection>,
        statement: String,
        params: Vec<golem::rdbms::postgres::DbValue>,
    ) -> Result<u64, golem::rdbms::postgres::Error> {
        self.db_calls.push(format!(
            "postgres execute(statement={statement}, params={})",
            postgres_params_text(&params)
        ));
        self.db_execute(None, &statement, postgres_param_row(&params))
            .map_err(golem::rdbms::postgres::Error::Other)
    }

    fn begin_transaction(
        &mut self,
        _connection: Resource<golem::rdbms::postgres::DbConnection>,
    ) -> Result<Resource<golem::rdbms::postgres::DbTransaction>, golem::rdbms::postgres::Error>
    {
        let rep = self.db_begin("postgres");
        Ok(unsafe { Resource::new_own(rep) })
    }

    fn drop(&mut self, _rep: Resource<golem::rdbms::postgres::DbConnection>) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl golem::rdbms::postgres::HostDbTransaction for Ctx {
    fn query(
        &mut self,
        transaction: Resource<golem::rdbms::postgres::DbTransaction>,
        statement: String,
        params: Vec<golem::rdbms::postgres::DbValue>,
    ) -> Result<golem::rdbms::postgres::DbResult, golem::rdbms::postgres::Error> {
        self.db_calls.push(format!(
            "postgres transaction query(statement={statement}, params={})",
            postgres_params_text(&params)
        ));
        let result = self.db_query_rows(Some(transaction.rep()), &statement, postgres_param_id(&params));
        postgres_result(result)
    }

    fn query_stream(
        &mut self,
        _transaction: Resource<golem::rdbms::postgres::DbTransaction>,
        statement: String,
        _params: Vec<golem::rdbms::postgres::DbValue>,
    ) -> Result<Resource<golem::rdbms::postgres::DbResultStream>, golem::rdbms::postgres::Error>
    {
        Err(golem::rdbms::postgres::Error::Other(format!(
            "query-stream is not used by roc-golem components ({statement})"
        )))
    }

    fn execute(
        &mut self,
        transaction: Resource<golem::rdbms::postgres::DbTransaction>,
        statement: String,
        params: Vec<golem::rdbms::postgres::DbValue>,
    ) -> Result<u64, golem::rdbms::postgres::Error> {
        self.db_calls.push(format!(
            "postgres transaction execute(statement={statement}, params={})",
            postgres_params_text(&params)
        ));
        self.db_execute(Some(transaction.rep()), &statement, postgres_param_row(&params))
            .map_err(golem::rdbms::postgres::Error::Other)
    }

    fn commit(
        &mut self,
        transaction: Resource<golem::rdbms::postgres::DbTransaction>,
    ) -> Result<(), golem::rdbms::postgres::Error> {
        self.db_commit("postgres", transaction.rep());
        Ok(())
    }

    fn rollback(
        &mut self,
        transaction: Resource<golem::rdbms::postgres::DbTransaction>,
    ) -> Result<(), golem::rdbms::postgres::Error> {
        self.db_rollback("postgres", transaction.rep());
        Ok(())
    }

    fn drop(&mut self, _rep: Resource<golem::rdbms::postgres::DbTransaction>) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl golem::rdbms::postgres::HostDbResultStream for Ctx {
    fn get_columns(
        &mut self,
        _stream: Resource<golem::rdbms::postgres::DbResultStream>,
    ) -> Result<Vec<golem::rdbms::postgres::DbColumn>, golem::rdbms::postgres::Error> {
        Err(golem::rdbms::postgres::Error::Other(String::from(
            "query-stream is not used by roc-golem components",
        )))
    }

    fn get_next(
        &mut self,
        _stream: Resource<golem::rdbms::postgres::DbResultStream>,
    ) -> Result<Option<Vec<golem::rdbms::postgres::DbRow>>, golem::rdbms::postgres::Error> {
        Err(golem::rdbms::postgres::Error::Other(String::from(
            "query-stream is not used by roc-golem components",
        )))
    }

    fn drop(&mut self, _rep: Resource<golem::rdbms::postgres::DbResultStream>) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl golem::rdbms::postgres::HostLazyDbValue for Ctx {
    /// The stub keeps the wrapped value and hands it back on the next read: the
    /// platform's JSON mirror constructs one lazy value per wrapped value, reads
    /// it once and drops it, which is exactly one read. The type is a resource
    /// handle rather than `Clone`, so a second read answers `null` instead of
    /// duplicating the value — nothing in this harness reads one twice.
    fn new(
        &mut self,
        value: golem::rdbms::postgres::DbValue,
    ) -> Resource<golem::rdbms::postgres::LazyDbValue> {
        let rep = self.lazy_db_values.len() as u32;
        self.lazy_db_values.push(value);
        unsafe { Resource::new_own(rep) }
    }

    fn get(
        &mut self,
        value: Resource<golem::rdbms::postgres::LazyDbValue>,
    ) -> golem::rdbms::postgres::DbValue {
        self.lazy_db_values
            .get_mut(value.rep() as usize)
            .map(|slot| std::mem::replace(slot, golem::rdbms::postgres::DbValue::Null))
            .unwrap_or(golem::rdbms::postgres::DbValue::Null)
    }

    fn drop(&mut self, _rep: Resource<golem::rdbms::postgres::LazyDbValue>) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl golem::rdbms::postgres::HostLazyDbColumnType for Ctx {
    fn new(
        &mut self,
        value: golem::rdbms::postgres::DbColumnType,
    ) -> Resource<golem::rdbms::postgres::LazyDbColumnType> {
        let rep = self.lazy_db_column_types.len() as u32;
        self.lazy_db_column_types.push(value);
        unsafe { Resource::new_own(rep) }
    }

    fn get(
        &mut self,
        value: Resource<golem::rdbms::postgres::LazyDbColumnType>,
    ) -> golem::rdbms::postgres::DbColumnType {
        self.lazy_db_column_types
            .get_mut(value.rep() as usize)
            .map(|slot| std::mem::replace(slot, golem::rdbms::postgres::DbColumnType::Text))
            .unwrap_or(golem::rdbms::postgres::DbColumnType::Text)
    }

    fn drop(
        &mut self,
        _rep: Resource<golem::rdbms::postgres::LazyDbColumnType>,
    ) -> wasmtime::Result<()> {
        Ok(())
    }
}

/// The params as the recorded call prints them: the common cases by name, and
/// anything else through `Debug` so an unexpected shape is still visible.
fn postgres_params_text(params: &[golem::rdbms::postgres::DbValue]) -> String {
    let rendered: Vec<String> = params
        .iter()
        .map(|param| match param {
            golem::rdbms::postgres::DbValue::Int4(number) => format!("int4({number})"),
            golem::rdbms::postgres::DbValue::Int8(number) => format!("int8({number})"),
            golem::rdbms::postgres::DbValue::Text(text) => format!("text({text:?})"),
            golem::rdbms::postgres::DbValue::Boolean(flag) => format!("boolean({flag})"),
            golem::rdbms::postgres::DbValue::Null => String::from("null"),
            other => format!("{other:?}"),
        })
        .collect();
    format!("[{}]", rendered.join(", "))
}

/// The `int4` a select filters on, when its first param is one.
fn postgres_param_id(params: &[golem::rdbms::postgres::DbValue]) -> Option<i32> {
    match params.first() {
        Some(golem::rdbms::postgres::DbValue::Int4(id)) => Some(*id),
        _ => None,
    }
}

/// The row an insert's params describe.
fn postgres_param_row(params: &[golem::rdbms::postgres::DbValue]) -> Option<(i32, String)> {
    match params {
        [
            golem::rdbms::postgres::DbValue::Int4(id),
            golem::rdbms::postgres::DbValue::Text(note),
        ] => Some((*id, note.clone())),
        _ => None,
    }
}

fn postgres_result(
    result: Result<StubResult, String>,
) -> Result<golem::rdbms::postgres::DbResult, golem::rdbms::postgres::Error> {
    use golem::rdbms::postgres::{DbColumn, DbColumnType, DbResult, DbRow, DbValue};

    let result = result.map_err(golem::rdbms::postgres::Error::Other)?;
    Ok(match result {
        StubResult::Count(count) => DbResult {
            columns: vec![DbColumn {
                ordinal: 0,
                name: String::from("count"),
                db_type: DbColumnType::Int8,
                db_type_name: String::from("INT8"),
            }],
            rows: vec![DbRow {
                values: vec![DbValue::Int8(count as i64)],
            }],
        },
        StubResult::Rows(rows) => DbResult {
            columns: vec![
                DbColumn {
                    ordinal: 0,
                    name: String::from("id"),
                    db_type: DbColumnType::Int4,
                    db_type_name: String::from("INT4"),
                },
                DbColumn {
                    ordinal: 1,
                    name: String::from("note"),
                    db_type: DbColumnType::Text,
                    db_type_name: String::from("TEXT"),
                },
            ],
            rows: rows
                .into_iter()
                .map(|(id, note)| DbRow {
                    values: vec![DbValue::Int4(id), DbValue::Text(note)],
                })
                .collect(),
        },
    })
}

impl golem::rdbms::mysql::Host for Ctx {}

impl golem::rdbms::mysql::HostDbConnection for Ctx {
    fn open(
        &mut self,
        address: String,
    ) -> Result<Resource<golem::rdbms::mysql::DbConnection>, golem::rdbms::mysql::Error> {
        stub_open(&mut self.db_calls, "mysql", &address)
            .map_err(golem::rdbms::mysql::Error::ConnectionFailure)?;
        let rep = self.db_connections;
        self.db_connections += 1;
        Ok(unsafe { Resource::new_own(rep) })
    }

    fn query(
        &mut self,
        _connection: Resource<golem::rdbms::mysql::DbConnection>,
        statement: String,
        params: Vec<golem::rdbms::mysql::DbValue>,
    ) -> Result<golem::rdbms::mysql::DbResult, golem::rdbms::mysql::Error> {
        self.db_calls.push(format!(
            "mysql query(statement={statement}, params={})",
            mysql_params_text(&params)
        ));
        mysql_result(self.db_query_rows(None, &statement, mysql_param_id(&params)))
    }

    fn query_stream(
        &mut self,
        _connection: Resource<golem::rdbms::mysql::DbConnection>,
        statement: String,
        _params: Vec<golem::rdbms::mysql::DbValue>,
    ) -> Result<Resource<golem::rdbms::mysql::DbResultStream>, golem::rdbms::mysql::Error> {
        Err(golem::rdbms::mysql::Error::Other(format!(
            "query-stream is not used by roc-golem components ({statement})"
        )))
    }

    fn execute(
        &mut self,
        _connection: Resource<golem::rdbms::mysql::DbConnection>,
        statement: String,
        params: Vec<golem::rdbms::mysql::DbValue>,
    ) -> Result<u64, golem::rdbms::mysql::Error> {
        self.db_calls.push(format!(
            "mysql execute(statement={statement}, params={})",
            mysql_params_text(&params)
        ));
        self.db_execute(None, &statement, mysql_param_row(&params))
            .map_err(golem::rdbms::mysql::Error::Other)
    }

    fn begin_transaction(
        &mut self,
        _connection: Resource<golem::rdbms::mysql::DbConnection>,
    ) -> Result<Resource<golem::rdbms::mysql::DbTransaction>, golem::rdbms::mysql::Error> {
        let rep = self.db_begin("mysql");
        Ok(unsafe { Resource::new_own(rep) })
    }

    fn drop(&mut self, _rep: Resource<golem::rdbms::mysql::DbConnection>) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl golem::rdbms::mysql::HostDbTransaction for Ctx {
    fn query(
        &mut self,
        transaction: Resource<golem::rdbms::mysql::DbTransaction>,
        statement: String,
        params: Vec<golem::rdbms::mysql::DbValue>,
    ) -> Result<golem::rdbms::mysql::DbResult, golem::rdbms::mysql::Error> {
        self.db_calls.push(format!(
            "mysql transaction query(statement={statement}, params={})",
            mysql_params_text(&params)
        ));
        mysql_result(self.db_query_rows(Some(transaction.rep()), &statement, mysql_param_id(&params)))
    }

    fn query_stream(
        &mut self,
        _transaction: Resource<golem::rdbms::mysql::DbTransaction>,
        statement: String,
        _params: Vec<golem::rdbms::mysql::DbValue>,
    ) -> Result<Resource<golem::rdbms::mysql::DbResultStream>, golem::rdbms::mysql::Error> {
        Err(golem::rdbms::mysql::Error::Other(format!(
            "query-stream is not used by roc-golem components ({statement})"
        )))
    }

    fn execute(
        &mut self,
        transaction: Resource<golem::rdbms::mysql::DbTransaction>,
        statement: String,
        params: Vec<golem::rdbms::mysql::DbValue>,
    ) -> Result<u64, golem::rdbms::mysql::Error> {
        self.db_calls.push(format!(
            "mysql transaction execute(statement={statement}, params={})",
            mysql_params_text(&params)
        ));
        self.db_execute(Some(transaction.rep()), &statement, mysql_param_row(&params))
            .map_err(golem::rdbms::mysql::Error::Other)
    }

    fn commit(
        &mut self,
        transaction: Resource<golem::rdbms::mysql::DbTransaction>,
    ) -> Result<(), golem::rdbms::mysql::Error> {
        self.db_commit("mysql", transaction.rep());
        Ok(())
    }

    fn rollback(
        &mut self,
        transaction: Resource<golem::rdbms::mysql::DbTransaction>,
    ) -> Result<(), golem::rdbms::mysql::Error> {
        self.db_rollback("mysql", transaction.rep());
        Ok(())
    }

    fn drop(&mut self, _rep: Resource<golem::rdbms::mysql::DbTransaction>) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl golem::rdbms::mysql::HostDbResultStream for Ctx {
    fn get_columns(
        &mut self,
        _stream: Resource<golem::rdbms::mysql::DbResultStream>,
    ) -> Result<Vec<golem::rdbms::mysql::DbColumn>, golem::rdbms::mysql::Error> {
        Err(golem::rdbms::mysql::Error::Other(String::from(
            "query-stream is not used by roc-golem components",
        )))
    }

    fn get_next(
        &mut self,
        _stream: Resource<golem::rdbms::mysql::DbResultStream>,
    ) -> Result<Option<Vec<golem::rdbms::mysql::DbRow>>, golem::rdbms::mysql::Error> {
        Err(golem::rdbms::mysql::Error::Other(String::from(
            "query-stream is not used by roc-golem components",
        )))
    }

    fn drop(&mut self, _rep: Resource<golem::rdbms::mysql::DbResultStream>) -> wasmtime::Result<()> {
        Ok(())
    }
}

fn mysql_params_text(params: &[golem::rdbms::mysql::DbValue]) -> String {
    let rendered: Vec<String> = params
        .iter()
        .map(|param| match param {
            golem::rdbms::mysql::DbValue::Int(number) => format!("int({number})"),
            golem::rdbms::mysql::DbValue::Bigint(number) => format!("bigint({number})"),
            golem::rdbms::mysql::DbValue::Varchar(text) => format!("varchar({text:?})"),
            golem::rdbms::mysql::DbValue::Boolean(flag) => format!("boolean({flag})"),
            golem::rdbms::mysql::DbValue::Null => String::from("null"),
            other => format!("{other:?}"),
        })
        .collect();
    format!("[{}]", rendered.join(", "))
}

fn mysql_param_id(params: &[golem::rdbms::mysql::DbValue]) -> Option<i32> {
    match params.first() {
        Some(golem::rdbms::mysql::DbValue::Int(id)) => Some(*id),
        _ => None,
    }
}

fn mysql_param_row(params: &[golem::rdbms::mysql::DbValue]) -> Option<(i32, String)> {
    match params {
        [golem::rdbms::mysql::DbValue::Int(id), golem::rdbms::mysql::DbValue::Varchar(note)] => {
            Some((*id, note.clone()))
        }
        _ => None,
    }
}

fn mysql_result(
    result: Result<StubResult, String>,
) -> Result<golem::rdbms::mysql::DbResult, golem::rdbms::mysql::Error> {
    use golem::rdbms::mysql::{DbColumn, DbColumnType, DbResult, DbRow, DbValue};

    let result = result.map_err(golem::rdbms::mysql::Error::Other)?;
    Ok(match result {
        StubResult::Count(count) => DbResult {
            columns: vec![DbColumn {
                ordinal: 0,
                name: String::from("count"),
                db_type: DbColumnType::Bigint,
                db_type_name: String::from("BIGINT"),
            }],
            rows: vec![DbRow {
                values: vec![DbValue::Bigint(count as i64)],
            }],
        },
        StubResult::Rows(rows) => DbResult {
            columns: vec![
                DbColumn {
                    ordinal: 0,
                    name: String::from("id"),
                    db_type: DbColumnType::Int,
                    db_type_name: String::from("INT"),
                },
                DbColumn {
                    ordinal: 1,
                    name: String::from("note"),
                    db_type: DbColumnType::Varchar,
                    db_type_name: String::from("VARCHAR"),
                },
            ],
            rows: rows
                .into_iter()
                .map(|(id, note)| DbRow {
                    values: vec![DbValue::Int(id), DbValue::Varchar(note)],
                })
                .collect(),
        },
    })
}

impl golem::rdbms::ignite2::Host for Ctx {}

impl golem::rdbms::ignite2::HostDbConnection for Ctx {
    fn open(
        &mut self,
        address: String,
    ) -> Result<Resource<golem::rdbms::ignite2::DbConnection>, golem::rdbms::ignite2::Error> {
        stub_open(&mut self.db_calls, "ignite", &address)
            .map_err(golem::rdbms::ignite2::Error::ConnectionFailure)?;
        let rep = self.db_connections;
        self.db_connections += 1;
        Ok(unsafe { Resource::new_own(rep) })
    }

    fn query(
        &mut self,
        _connection: Resource<golem::rdbms::ignite2::DbConnection>,
        statement: String,
        params: Vec<golem::rdbms::ignite2::DbValue>,
    ) -> Result<golem::rdbms::ignite2::DbResult, golem::rdbms::ignite2::Error> {
        self.db_calls.push(format!(
            "ignite query(statement={statement}, params={})",
            ignite_params_text(&params)
        ));
        ignite_result(self.db_query_rows(None, &statement, ignite_param_id(&params)))
    }

    fn query_stream(
        &mut self,
        _connection: Resource<golem::rdbms::ignite2::DbConnection>,
        statement: String,
        _params: Vec<golem::rdbms::ignite2::DbValue>,
    ) -> Result<Resource<golem::rdbms::ignite2::DbResultStream>, golem::rdbms::ignite2::Error> {
        Err(golem::rdbms::ignite2::Error::Other(format!(
            "query-stream is not used by roc-golem components ({statement})"
        )))
    }

    fn execute(
        &mut self,
        _connection: Resource<golem::rdbms::ignite2::DbConnection>,
        statement: String,
        params: Vec<golem::rdbms::ignite2::DbValue>,
    ) -> Result<i64, golem::rdbms::ignite2::Error> {
        self.db_calls.push(format!(
            "ignite execute(statement={statement}, params={})",
            ignite_params_text(&params)
        ));
        self.db_execute(None, &statement, ignite_param_row(&params))
            .map(|count| count as i64)
            .map_err(golem::rdbms::ignite2::Error::Other)
    }

    fn begin_transaction(
        &mut self,
        _connection: Resource<golem::rdbms::ignite2::DbConnection>,
    ) -> Result<Resource<golem::rdbms::ignite2::DbTransaction>, golem::rdbms::ignite2::Error> {
        let rep = self.db_begin("ignite");
        Ok(unsafe { Resource::new_own(rep) })
    }

    fn drop(&mut self, _rep: Resource<golem::rdbms::ignite2::DbConnection>) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl golem::rdbms::ignite2::HostDbTransaction for Ctx {
    fn query(
        &mut self,
        transaction: Resource<golem::rdbms::ignite2::DbTransaction>,
        statement: String,
        params: Vec<golem::rdbms::ignite2::DbValue>,
    ) -> Result<golem::rdbms::ignite2::DbResult, golem::rdbms::ignite2::Error> {
        self.db_calls.push(format!(
            "ignite transaction query(statement={statement}, params={})",
            ignite_params_text(&params)
        ));
        ignite_result(self.db_query_rows(Some(transaction.rep()), &statement, ignite_param_id(&params)))
    }

    fn query_stream(
        &mut self,
        _transaction: Resource<golem::rdbms::ignite2::DbTransaction>,
        statement: String,
        _params: Vec<golem::rdbms::ignite2::DbValue>,
    ) -> Result<Resource<golem::rdbms::ignite2::DbResultStream>, golem::rdbms::ignite2::Error> {
        Err(golem::rdbms::ignite2::Error::Other(format!(
            "query-stream is not used by roc-golem components ({statement})"
        )))
    }

    fn execute(
        &mut self,
        transaction: Resource<golem::rdbms::ignite2::DbTransaction>,
        statement: String,
        params: Vec<golem::rdbms::ignite2::DbValue>,
    ) -> Result<i64, golem::rdbms::ignite2::Error> {
        self.db_calls.push(format!(
            "ignite transaction execute(statement={statement}, params={})",
            ignite_params_text(&params)
        ));
        self.db_execute(Some(transaction.rep()), &statement, ignite_param_row(&params))
            .map(|count| count as i64)
            .map_err(golem::rdbms::ignite2::Error::Other)
    }

    fn commit(
        &mut self,
        transaction: Resource<golem::rdbms::ignite2::DbTransaction>,
    ) -> Result<(), golem::rdbms::ignite2::Error> {
        self.db_commit("ignite", transaction.rep());
        Ok(())
    }

    fn rollback(
        &mut self,
        transaction: Resource<golem::rdbms::ignite2::DbTransaction>,
    ) -> Result<(), golem::rdbms::ignite2::Error> {
        self.db_rollback("ignite", transaction.rep());
        Ok(())
    }

    fn drop(&mut self, _rep: Resource<golem::rdbms::ignite2::DbTransaction>) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl golem::rdbms::ignite2::HostDbResultStream for Ctx {
    fn get_columns(
        &mut self,
        _stream: Resource<golem::rdbms::ignite2::DbResultStream>,
    ) -> Result<Vec<golem::rdbms::ignite2::DbColumn>, golem::rdbms::ignite2::Error> {
        Err(golem::rdbms::ignite2::Error::Other(String::from(
            "query-stream is not used by roc-golem components",
        )))
    }

    fn get_next(
        &mut self,
        _stream: Resource<golem::rdbms::ignite2::DbResultStream>,
    ) -> Result<Option<Vec<golem::rdbms::ignite2::DbRow>>, golem::rdbms::ignite2::Error> {
        Err(golem::rdbms::ignite2::Error::Other(String::from(
            "query-stream is not used by roc-golem components",
        )))
    }

    fn drop(&mut self, _rep: Resource<golem::rdbms::ignite2::DbResultStream>) -> wasmtime::Result<()> {
        Ok(())
    }
}

fn ignite_params_text(params: &[golem::rdbms::ignite2::DbValue]) -> String {
    let rendered: Vec<String> = params
        .iter()
        .map(|param| match param {
            golem::rdbms::ignite2::DbValue::DbInt(number) => format!("dbInt({number})"),
            golem::rdbms::ignite2::DbValue::DbLong(number) => format!("dbLong({number})"),
            golem::rdbms::ignite2::DbValue::DbString(text) => format!("dbString({text:?})"),
            golem::rdbms::ignite2::DbValue::DbBoolean(flag) => format!("dbBoolean({flag})"),
            golem::rdbms::ignite2::DbValue::DbNull => String::from("dbNull"),
            other => format!("{other:?}"),
        })
        .collect();
    format!("[{}]", rendered.join(", "))
}

fn ignite_param_id(params: &[golem::rdbms::ignite2::DbValue]) -> Option<i32> {
    match params.first() {
        Some(golem::rdbms::ignite2::DbValue::DbInt(id)) => Some(*id),
        _ => None,
    }
}

fn ignite_param_row(params: &[golem::rdbms::ignite2::DbValue]) -> Option<(i32, String)> {
    match params {
        [
            golem::rdbms::ignite2::DbValue::DbInt(id),
            golem::rdbms::ignite2::DbValue::DbString(note),
        ] => Some((*id, note.clone())),
        _ => None,
    }
}

fn ignite_result(
    result: Result<StubResult, String>,
) -> Result<golem::rdbms::ignite2::DbResult, golem::rdbms::ignite2::Error> {
    use golem::rdbms::ignite2::{DbColumn, DbResult, DbRow, DbValue};

    let result = result.map_err(golem::rdbms::ignite2::Error::Other)?;
    Ok(match result {
        StubResult::Count(count) => DbResult {
            columns: vec![DbColumn {
                ordinal: 0,
                name: String::from("count"),
            }],
            rows: vec![DbRow {
                values: vec![DbValue::DbLong(count as i64)],
            }],
        },
        StubResult::Rows(rows) => DbResult {
            columns: vec![
                DbColumn {
                    ordinal: 0,
                    name: String::from("id"),
                },
                DbColumn {
                    ordinal: 1,
                    name: String::from("note"),
                },
            ],
            rows: rows
                .into_iter()
                .map(|(id, note)| DbRow {
                    values: vec![DbValue::DbInt(id), DbValue::DbString(note)],
                })
                .collect(),
        },
    })
}

impl golem::rdbms::types::Host for Ctx {}

/// The component reference and the component id the `golem:api/host` stubs
/// resolve, plus the UUID a stubbed idempotency key comes back as. A real server
/// looks the reference up in the registry; here one name is known and anything
/// else resolves to nothing, which is what the counter's resolve commands report
/// back (the positive and the negative case).
const STUB_COMPONENT_REFERENCE: &str = "roc-golem:counter";
const STUB_UUID: golem::core::types::Uuid = golem::core::types::Uuid {
    high_bits: 0x0102_0304_0506_0708,
    low_bits: 0x090a_0b0c_0d0e_0f10,
};

fn stub_component_id() -> golem::core::types::ComponentId {
    golem::core::types::ComponentId { uuid: STUB_UUID }
}

fn stub_agent_id(agent_id: String) -> golem::core::types::AgentId {
    golem::core::types::AgentId {
        component_id: stub_component_id(),
        agent_id,
    }
}

/// The error an operation the harness cannot model reports, so a component that
/// reaches for one fails visibly instead of being handed invented data.
fn harness_unavailable(operation: &str) -> golem::api::host::AgentOperationError {
    golem::api::host::AgentOperationError::BackendError(format!(
        "{operation} is not available in the ABI harness"
    ))
}

/// `golem:api/host`, stubbed. The synchronous calls the examples make are
/// recorded for `--expect-host-call`; `trap` panics because the WIT function has
/// no result a host error could travel in, and the harness catches that panic
/// itself for `--expect-trap` (wasmtime lets a host panic unwind on the
/// concurrent path instead of turning it into a guest trap). Everything
/// operator-facing (`update-agent`, `revert-agent`, `fork-agent`,
/// `get-agent-metadata`) reports `harness-unavailable`, and `fork` — which would
/// have to copy this store's agent — does too.
impl golem::api::host::Host for Ctx {
    fn create_promise(&mut self) -> golem::core::types::PromiseId {
        self.host_api_calls.push(String::from("create-promise"));
        golem::core::types::PromiseId {
            agent_id: stub_agent_id(String::from("stub-agent(\"harness\")")),
            oplog_idx: 1,
        }
    }

    fn get_promise(
        &mut self,
        _promise_id: golem::core::types::PromiseId,
    ) -> wasmtime::component::Resource<golem::api::host::GetPromiseResult> {
        self.host_api_calls.push(String::from("get-promise"));
        // The stub promise is already complete; the body is empty and the
        // rep does not have to name anything.
        wasmtime::component::Resource::new_own(0)
    }

    fn complete_promise(
        &mut self,
        _promise_id: golem::core::types::PromiseId,
        _data: Vec<u8>,
    ) -> bool {
        self.host_api_calls.push(String::from("complete-promise"));
        true
    }

    fn oplog_commit(&mut self, replicas: u8) {
        self.host_api_calls
            .push(format!("oplog-commit(replicas={replicas})"));
    }

    fn trap(&mut self, reason: String) {
        self.host_api_calls.push(format!("trap(reason={reason})"));
        panic!("guest-requested trap: {reason}");
    }

    fn generate_idempotency_key(&mut self) -> golem::core::types::Uuid {
        self.host_api_calls
            .push(String::from("generate-idempotency-key"));
        STUB_UUID
    }

    fn get_self_metadata(
        &mut self,
    ) -> Result<golem::api::host::AgentMetadata, golem::api::host::AgentOperationError> {
        self.host_api_calls.push(String::from("get-self-metadata"));
        Ok(golem::api::host::AgentMetadata {
            agent_id: stub_agent_id(String::from("stub-agent(\"harness\")")),
            args: Vec::new(),
            env: Vec::new(),
            config: Vec::new(),
            status: golem::api::host::AgentStatus::Idle,
            component_revision: 1,
            retry_count: 0,
            environment_id: golem::api::host::EnvironmentId { uuid: STUB_UUID },
        })
    }

    fn get_agent_metadata(
        &mut self,
        agent_id: golem::core::types::AgentId,
    ) -> Option<golem::api::host::AgentMetadata> {
        self.host_api_calls
            .push(format!("get-agent-metadata(agent={})", agent_id.agent_id));
        None
    }

    fn update_agent(
        &mut self,
        _agent_id: golem::core::types::AgentId,
        _target_version: u64,
        _mode: golem::api::host::UpdateMode,
    ) -> Result<(), golem::api::host::AgentOperationError> {
        self.host_api_calls.push(String::from("update-agent"));
        Err(harness_unavailable("update-agent"))
    }

    fn fork_agent(
        &mut self,
        _source_agent_id: golem::core::types::AgentId,
        _target_agent_id: golem::core::types::AgentId,
        _oplog_idx_cut_off: u64,
    ) -> Result<(), golem::api::host::AgentOperationError> {
        self.host_api_calls.push(String::from("fork-agent"));
        Err(harness_unavailable("fork-agent"))
    }

    fn revert_agent(
        &mut self,
        _agent_id: golem::core::types::AgentId,
        _revert_target: golem::api::host::RevertAgentTarget,
    ) -> Result<(), golem::api::host::AgentOperationError> {
        self.host_api_calls.push(String::from("revert-agent"));
        Err(harness_unavailable("revert-agent"))
    }

    fn resolve_component_id(
        &mut self,
        component_reference: String,
    ) -> Option<golem::core::types::ComponentId> {
        self.host_api_calls.push(format!(
            "resolve-component-id(component={component_reference})"
        ));
        (component_reference == STUB_COMPONENT_REFERENCE).then(stub_component_id)
    }

    fn resolve_agent_id(
        &mut self,
        component_reference: String,
        agent_id: String,
    ) -> Option<golem::core::types::AgentId> {
        self.host_api_calls.push(format!(
            "resolve-agent-id(component={component_reference}, agent={agent_id})"
        ));
        (component_reference == STUB_COMPONENT_REFERENCE).then(|| stub_agent_id(agent_id))
    }

    fn resolve_agent_id_strict(
        &mut self,
        component_reference: String,
        agent_name: String,
    ) -> Option<golem::core::types::AgentId> {
        self.host_api_calls.push(format!(
            "resolve-agent-id-strict(component={component_reference}, agent={agent_name})"
        ));
        (component_reference == STUB_COMPONENT_REFERENCE).then(|| stub_agent_id(agent_name))
    }

    fn fork(
        &mut self,
    ) -> Result<golem::api::host::ForkResult, golem::api::host::AgentOperationError> {
        self.host_api_calls.push(String::from("fork"));
        Err(harness_unavailable("fork"))
    }
}

impl golem::api::host::HostGetAgents for Ctx {
    fn new(
        &mut self,
        _component_id: golem::core::types::ComponentId,
        _filter: Option<golem::api::host::AgentAnyFilter>,
        _precise: bool,
    ) -> wasmtime::component::Resource<golem::api::host::GetAgents> {
        wasmtime::component::Resource::new_own(0)
    }

    fn get_next(
        &mut self,
        _self_: wasmtime::component::Resource<golem::api::host::GetAgents>,
    ) -> Result<
        Option<Vec<golem::api::host::AgentMetadata>>,
        golem::api::host::AgentOperationError,
    > {
        Ok(None)
    }

    fn drop(
        &mut self,
        _rep: wasmtime::component::Resource<golem::api::host::GetAgents>,
    ) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl golem::api::host::HostGetPromiseResult for Ctx {
    fn drop(
        &mut self,
        _rep: wasmtime::component::Resource<golem::api::host::GetPromiseResult>,
    ) -> wasmtime::Result<()> {
        Ok(())
    }
}

/// `get-promise-result.get` is the one async function in `golem:api/host`; the
/// stub promise is already complete, so it answers with an empty body.
impl golem::api::host::HostGetPromiseResultWithStore<Ctx> for Ctx {
    fn get(
        _accessor: &wasmtime::component::Accessor<Ctx, Ctx>,
        _self_: wasmtime::component::Resource<golem::api::host::GetPromiseResult>,
    ) -> impl std::future::Future<Output = Vec<u8>> + Send {
        async move { Vec::new() }
    }
}

/// What the component under test is expected to report and do.
struct Expectation {
    component: PathBuf,
    agent_type: Option<String>,
    method: Option<String>,
    /// First message to send, and the reply text it should contain.
    message: String,
    reply_contains: Option<String>,
    /// Second message (defaults to the first), for flows that need two steps
    /// such as connect-then-send, plus the reply it should contain.
    then_message: Option<String>,
    then_reply_contains: Option<String>,
    /// Substring expected in a log line the agent emitted.
    expect_log: Option<String>,
    /// Substring expected in a WebSocket message the agent sent.
    expect_sent: Option<String>,
    /// Agent config passed to `initialize`; defaults to the stub HTTP server URL
    /// when `--http-stub` is set, otherwise `{}`.
    config: Option<String>,
    /// Substring expected in a request line the stub HTTP server received.
    expect_http: Option<String>,
    /// Substring expected in the remote calls the agent made through `rpc!`.
    expect_rpc: Option<String>,
    /// Substring expected in the quota calls (`new-token`, `reserve`, …) the
    /// agent made through the `golem:quota/types` interface.
    expect_quota: Option<String>,
    /// Substring expected in the retry-policy calls the agent made through the
    /// `golem:api/retry` interface.
    expect_retry: Option<String>,
    /// Substring expected in the oplog/execution-control calls the agent made
    /// through `golem:api/host` (`trap`, `oplog-commit`, `resolve-…`).
    expect_host_call: Option<String>,
    /// Substring expected in the key-value calls the agent made through
    /// `wasi:keyvalue` (`open-bucket`, `set`, `get`, …).
    expect_kv: Option<String>,
    /// Substring expected in the blobstore calls the agent made through
    /// `wasi:blobstore` (`create-container`, `write-data`, …).
    expect_blob: Option<String>,
    /// Substring expected in the filesystem calls the agent made through
    /// `wasi:filesystem@0.2.6` (`open-at`, `write`, `read-directory`, …).
    expect_fs: Option<String>,
    /// Substring expected in the database calls the agent made through
    /// `golem:rdbms` (`postgres open`, `postgres execute`, `mysql query`, …).
    expect_db: Option<String>,
    /// The first message is expected to trap, with the substring in the trap's
    /// text. The message/reply checks are skipped in that case: a trap is a
    /// failed invocation, not a reply, and the instance is not asked for
    /// another one afterwards.
    expect_trap: Option<String>,
    /// A tool (registered as a method) to invoke, and the substring its result
    /// should contain.
    tool: Option<String>,
    tool_reply: Option<String>,
    /// The mount path the agent type should publish, rendered as a URL path
    /// (`site/{id}`): literal segments verbatim, captures in braces.
    mount: Option<String>,
    /// The agent type kind the app should register (`regular` or
    /// `http-router`).
    kind: Option<String>,
}

fn usage() -> String {
    String::from(
        "usage: abi_harness <component.wasm> [--type NAME] [--method NAME] \
         [--message TEXT] [--reply SUBSTRING] [--then-message TEXT] \
         [--then-reply SUBSTRING] [--expect-log SUBSTRING] [--expect-sent SUBSTRING] \
         [--config TEXT] [--expect-http SUBSTRING] [--expect-rpc SUBSTRING] \
         [--expect-quota SUBSTRING] [--expect-retry SUBSTRING] \
         [--expect-host-call SUBSTRING] [--expect-kv SUBSTRING] \
         [--expect-blob SUBSTRING] [--expect-fs SUBSTRING] [--expect-db SUBSTRING] \
         [--expect-trap SUBSTRING] \
         [--tool NAME] [--tool-reply SUBSTRING] [--expect-mount PATH] \
         [--expect-kind regular|http-router]",
    )
}

fn parse_args() -> Result<Expectation> {
    let mut args = std::env::args().skip(1);
    let mut expectation = Expectation {
        component: PathBuf::new(),
        agent_type: None,
        method: None,
        message: String::from("increment"),
        reply_contains: None,
        then_message: None,
        then_reply_contains: None,
        expect_log: None,
        expect_sent: None,
        config: None,
        expect_http: None,
        expect_rpc: None,
        expect_quota: None,
        expect_retry: None,
        expect_host_call: None,
        expect_kv: None,
        expect_blob: None,
        expect_fs: None,
        expect_db: None,
        expect_trap: None,
        tool: None,
        tool_reply: None,
        mount: None,
        kind: None,
    };

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--type" => expectation.agent_type = args.next(),
            "--method" => expectation.method = args.next(),
            "--message" => {
                expectation.message = args.next().context("--message needs a value")?
            }
            "--reply" => expectation.reply_contains = args.next(),
            "--then-message" => expectation.then_message = args.next(),
            "--then-reply" => expectation.then_reply_contains = args.next(),
            "--expect-log" => expectation.expect_log = args.next(),
            "--expect-sent" => expectation.expect_sent = args.next(),
            "--config" => expectation.config = args.next(),
            "--expect-http" => expectation.expect_http = args.next(),
            "--expect-rpc" => expectation.expect_rpc = args.next(),
            "--expect-quota" => expectation.expect_quota = args.next(),
            "--expect-retry" => expectation.expect_retry = args.next(),
            "--expect-host-call" => expectation.expect_host_call = args.next(),
            "--expect-kv" => expectation.expect_kv = args.next(),
            "--expect-blob" => expectation.expect_blob = args.next(),
            "--expect-fs" => expectation.expect_fs = args.next(),
            "--expect-db" => expectation.expect_db = args.next(),
            "--expect-trap" => expectation.expect_trap = args.next(),
            "--tool" => expectation.tool = args.next(),
            "--tool-reply" => expectation.tool_reply = args.next(),
            "--expect-mount" => expectation.mount = args.next(),
            "--expect-kind" => expectation.kind = args.next(),
            "-h" | "--help" => bail!("{}", usage()),
            other => {
                if expectation.component.as_os_str().is_empty() {
                    expectation.component = PathBuf::from(other);
                } else {
                    bail!("unexpected argument `{}`\n{}", other, usage());
                }
            }
        }
    }

    if expectation.component.as_os_str().is_empty() {
        bail!("{}", usage());
    }
    Ok(expectation)
}

/// A single component-model `string` as an input parameter list: the root is a
/// record with one field, which is what the platform declares in its agent
/// method schemas (Golem type-checks arguments against them).
fn text_value(s: &str) -> SchemaValueTree {
    SchemaValueTree {
        value_nodes: vec![
            SchemaValueNode::RecordValue(vec![1]),
            SchemaValueNode::StringValue(s.to_string()),
        ],
        root: 0,
    }
}

/// Extracts the single string from an output value tree (root is the string).
fn text_of(value: &SchemaValueTree) -> Option<String> {
    match value.value_nodes.get(value.root as usize) {
        Some(SchemaValueNode::StringValue(text)) => Some(text.clone()),
        Some(SchemaValueNode::RecordValue(items)) | Some(SchemaValueNode::TupleValue(items)) => {
            match items.first().and_then(|i| value.value_nodes.get(*i as usize)) {
                Some(SchemaValueNode::StringValue(text)) => Some(text.clone()),
                _ => None,
            }
        }
        _ => None,
    }
}

fn main() -> Result<ExitCode> {
    pollster::block_on(run())
}

async fn run() -> Result<ExitCode> {
    let expectation = parse_args()?;

    let mut config = Config::new();
    config.async_support(true);
    config.concurrency_support(true);
    config.wasm_component_model(true);
    config.wasm_component_model_async(true);
    let engine = Engine::new(&config)?;
    let component = Component::from_file(&engine, &expectation.component).map_err(|err| {
        anyhow::anyhow!("failed to load {}: {err}", expectation.component.display())
    })?;

    let agent_config = expectation
        .config
        .clone()
        .unwrap_or_else(|| String::from("{}"));

    // Every import the components use is implemented by this harness (see
    // `stubs.rs`), so no wasmtime-wasi linker is involved: wasmtime's own
    // `wasi:http` implementation tracks a newer WASI version than Golem pins.
    let mut linker = Linker::<Ctx>::new(&engine);
    HarnessWorld::add_to_linker::<Ctx, Ctx>(
        &mut linker,
        &LinkOptions::default(),
        |ctx: &mut Ctx| ctx,
    )?;

    let mut store = Store::new(
        &engine,
        Ctx {
            table: Default::default(),
            logs: Vec::new(),
            sent: Vec::new(),
            connections: Vec::new(),
            rpc_calls: Vec::new(),
            quota_calls: Vec::new(),
            retry_calls: Vec::new(),
            host_api_calls: Vec::new(),
            kv_calls: Vec::new(),
            kv_store: Default::default(),
            kv_buckets: Vec::new(),
            kv_outgoing: Vec::new(),
            kv_incoming: Vec::new(),
            blob_calls: Vec::new(),
            blob_containers: Default::default(),
            blob_container_reps: Vec::new(),
            blob_outgoing: Vec::new(),
            blob_incoming: Vec::new(),
            fs_calls: Vec::new(),
            db_calls: Vec::new(),
            db_rows: Vec::new(),
            db_transactions: Vec::new(),
            db_connections: 0,
            lazy_db_values: Vec::new(),
            lazy_db_column_types: Vec::new(),
            fs_entries: Default::default(),
            fs_contents: Default::default(),
            fs_descriptors: Vec::new(),
            fs_listings: Vec::new(),
            fields_table: Vec::new(),
            requests_table: Vec::new(),
            body_slots: Vec::new(),
            request_body: Vec::new(),
            response_body: Vec::new(),
            response_cursor: 0,
            http_request_lines: Vec::new(),
        },
    );
    let bindings = HarnessWorld::instantiate_async(&mut store, &component, &linker)
        .await
        .map_err(|err| {
            anyhow::anyhow!(
                "failed to instantiate {}: {err}",
                expectation.component.display()
            )
        })?;

    let guest = bindings.golem_agent_guest();
    let mut failures: Vec<String> = Vec::new();

    // 1. Agent definition: exercises the metadata hook, which only uses strings
    //    longer than the 11-byte small-string limit. Components that import an
    //    async host function (the streaming agent's websocket receive) make the
    //    store require async calls, so even the sync exports are driven through
    //    `call_concurrent` here.
    let definition = {
        let callee = guest.func_get_definition();
        let (definition,) = store
            .run_concurrent(async |accessor| callee.call_concurrent(accessor, ()).await)
            .await??;
        definition
    };
    let methods: Vec<String> = definition.methods.iter().map(|m| m.name.clone()).collect();
    println!("[get-definition] type-name = {:?}", definition.type_name);
    println!("[get-definition] kind      = {:?}", definition.kind);
    println!("[get-definition] methods   = {:?}", methods);
    if let Some(mount) = &definition.http_mount {
        println!(
            "[get-definition] http-mount = {:?} (static bindings: {}, filesystem bindings: {}, file response headers: {})",
            render_mount_path(&mount.path_prefix),
            mount.static_bindings.len(),
            mount.filesystem_bindings.len(),
            mount.file_response_headers.len(),
        );
    }
    if let Some(expected) = &expectation.agent_type {
        if &definition.type_name != expected {
            failures.push(format!(
                "agent type name is {:?}, expected {:?} (the host falls back to \"roc-agent\" when the app's metadata cannot be read)",
                definition.type_name, expected
            ));
        }
    }
    if let Some(expected) = &expectation.kind {
        let actual = match definition.kind {
            golem::agent::common::AgentTypeKind::Regular => "regular",
            golem::agent::common::AgentTypeKind::HttpRouter => "http-router",
        };
        if actual != expected {
            failures.push(format!(
                "agent type kind is {actual:?}, expected {expected:?}"
            ));
        }
    }
    if let Some(expected) = &expectation.mount {
        let actual = definition
            .http_mount
            .as_ref()
            .map(|mount| render_mount_path(&mount.path_prefix));
        if actual.as_deref() != Some(expected.as_str()) {
            failures.push(format!(
                "http mount path is {:?}, expected {:?}",
                actual, expected
            ));
        }
    }
    if let Some(expected) = &expectation.method {
        if !methods.iter().any(|m| m == expected) {
            failures.push(format!(
                "method {:?} missing from the agent definition (have {:?})",
                expected, methods
            ));
        }
    }

    // 2. Agent discovery is what golem-cli calls at deploy time; a duplicated
    //    fallback name here makes multi-component apps undeployable.
    let discovered = {
        let callee = guest.func_discover_agent_types();
        let (discovered,) = store
            .run_concurrent(async |accessor| callee.call_concurrent(accessor, ()).await)
            .await??;
        discovered
    };
    match discovered {
        Ok(types) => {
            let names: Vec<String> = types.iter().map(|t| t.type_name.clone()).collect();
            println!("[discover-agent-types]     = {:?}", names);
            if let Some(expected) = &expectation.agent_type {
                if !names.iter().any(|n| n == expected) {
                    failures.push(format!(
                        "discover-agent-types reported {:?}, expected {:?}",
                        names, expected
                    ));
                }
            }
        }
        Err(err) => {
            println!("[discover-agent-types]     = error {:?}", err);
            failures.push(format!("discover-agent-types returned an error: {:?}", err));
        }
    }

    // 3. initialize: the first call that runs app code and allocates. A trap
    //    here poisons the instance, so every later call fails too.
    let init_result = store
        .run_concurrent(async |accessor| {
            guest
                .call_initialize(
                    accessor,
                    String::from("harness"),
                    text_value(&agent_config),
                    Principal::Anonymous,
                )
                .await
        })
        .await;
    match init_result {
        Ok(Ok(Ok(()))) => println!("[initialize]               = ok"),
        Ok(Ok(Err(err))) => {
            println!("[initialize]               = agent error {:?}", err);
            failures.push(format!("initialize returned an error: {:?}", err));
        }
        Ok(Err(err)) => {
            println!("[initialize]               = trap\n{}", err);
            failures.push(String::from("initialize trapped"));
        }
        Err(err) => {
            println!("[initialize]               = executor error\n{err:?}");
            failures.push(String::from("initialize could not run"));
        }
    }

    // 4. Message round-trips. `invoke` returns the app's reply text, so replies
    //    are matched directly; a second call shows whether state survived.
    //    `--expect-trap` inverts that for one call: `trap!` fails the invocation,
    //    so what has to match is the trap's text, and there is no reply — nor a
    //    second call on an instance whose invocation just failed.
    let second_message = expectation
        .then_message
        .clone()
        .unwrap_or_else(|| expectation.message.clone());

    if let Some(expected_reason) = &expectation.expect_trap {
        // A host-side `trap` is an uncatchable failure by design, and on the
        // concurrent path wasmtime lets a panic inside a host call unwind
        // instead of turning it into a guest trap the way it does for a wasm
        // `unreachable`. The harness therefore catches the panic itself and
        // reads it as the failed invocation `trap!` produces on a real server;
        // nothing is asked of this store afterwards.
        let trap_result = {
            // The panic is expected and is reported below; the default hook's
            // "thread 'main' panicked" would read like a failure.
            let default_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(|_| {}));
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                pollster::block_on(store.run_concurrent(async |accessor| {
                    guest
                        .call_invoke(
                            accessor,
                            String::from("handle-message"),
                            text_value(&expectation.message),
                            Principal::Anonymous,
                        )
                        .await
                }))
            }));
            std::panic::set_hook(default_hook);
            result
        };

        let trapped_with = match trap_result {
            // The panic message carries the reason the guest asked for.
            Err(panic) => panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|text| text.to_string())),
            // A guest-side trap is reported by wasmtime as the call's error.
            Ok(Ok(Err(err))) => Some(format!("{err:?}")),
            Ok(result) => {
                failures.push(match result {
                    Ok(Ok(Ok(value))) => format!(
                        "expected the invocation to trap, but it returned {:?}",
                        value.as_ref().and_then(text_of).unwrap_or_default()
                    ),
                    _ => format!("expected the invocation to trap, but it could not run: {result:?}"),
                });
                None
            }
        };

        if let Some(text) = trapped_with {
            println!("[invoke handle-message] = trap (expected)\n{text}");
            if !text.contains(expected_reason.as_str()) {
                failures.push(format!(
                    "the trap text {text:?} does not contain {expected_reason:?}"
                ));
            }
        }
    }

    for (label, message, expected) in [
        (
            "invoke handle-message",
            expectation.message.clone(),
            expectation.reply_contains.clone(),
        ),
        (
            "invoke handle-message again",
            second_message,
            expectation.then_reply_contains.clone(),
        ),
    ] {
        if expected.is_none() || expectation.expect_trap.is_some() {
            continue;
        }
        let invoke_result = store
            .run_concurrent(async |accessor| {
                guest
                    .call_invoke(
                        accessor,
                        String::from("handle-message"),
                        text_value(&message),
                        Principal::Anonymous,
                    )
                    .await
            })
            .await;
        match invoke_result {
            Ok(Ok(Ok(value))) => {
                let raw = value.as_ref().and_then(text_of).unwrap_or_default();
                println!("[{}] = {:?}", label, raw);
                if let Some(expected) = expected {
                    if !raw.contains(expected.as_str()) {
                        failures.push(format!(
                            "{}: reply {:?} does not contain {:?}",
                            label, raw, expected
                        ));
                    }
                }
            }
            Ok(Ok(Err(err))) => {
                println!("[{}] = agent error {:?}", label, err);
                failures.push(format!("{} returned an error: {:?}", label, err));
            }
            Ok(Err(err)) => {
                println!("[{}] = trap\n{}", label, err);
                failures.push(format!("{} trapped", label));
            }
            Err(err) => {
                println!("[{label}] = executor error\n{err:?}");
                failures.push(format!("{} could not run", label));
            }
        }
    }

    // 5. Tool calls take a different path through the host (they go to the
    //    app's `handleToolCall!` and come back as a result payload). A check
    //    that expects a trap stops here: its invocation failed on purpose, and
    //    this store is not asked for more work afterwards.
    if expectation.expect_trap.is_none() {
        if let Some(tool) = &expectation.tool {
        let tool_result = store
            .run_concurrent(async |accessor| {
                guest
                    .call_invoke(
                        accessor,
                        tool.clone(),
                        text_value("{}"),
                        Principal::Anonymous,
                    )
                    .await
            })
            .await;
        match tool_result {
            Ok(Ok(Ok(value))) => {
                let raw = value.as_ref().and_then(text_of).unwrap_or_default();
                println!("[invoke {}] = {:?}", tool, raw);
                if let Some(expected) = &expectation.tool_reply {
                    if !raw.contains(expected.as_str()) {
                        failures.push(format!(
                            "tool {}: result {:?} does not contain {:?}",
                            tool, raw, expected
                        ));
                    }
                }
            }
            Ok(Ok(Err(err))) => {
                println!("[invoke {}] = agent error {:?}", tool, err);
                failures.push(format!("tool {} returned an error: {:?}", tool, err));
            }
            Ok(Err(err)) => {
                println!("[invoke {}] = trap\n{}", tool, err);
                failures.push(format!("tool {} trapped", tool));
            }
            Err(err) => {
                println!("[invoke {tool}] = executor error\n{err:?}");
                failures.push(format!("tool {} could not run", tool));
            }
        }
        }
    }

    // 6. Effects the agent reached for: log lines and WebSocket sends are
    //    recorded by the stubs above, so their wiring is checked, not just the
    //    reply text.
    {
        let ctx = store.data();
        if let Some(expected) = &expectation.expect_log {
            let hit = ctx.logs.iter().any(|line| line.contains(expected.as_str()));
            println!("[logs] {:?}", ctx.logs);
            if !hit {
                failures.push(format!(
                    "no log line contains {:?} (recorded: {:?})",
                    expected, ctx.logs
                ));
            }
        }
        if let Some(expected) = &expectation.expect_http {
            let requests = ctx.http_request_lines.clone();
            let hit = requests.iter().any(|line| line.contains(expected.as_str()));
            println!("[http requests] {:?}", requests);
            if !hit {
                failures.push(format!(
                    "no HTTP request line contains {:?} (received: {:?})",
                    expected, requests
                ));
            }
        }
        if let Some(expected) = &expectation.expect_rpc {
            let hit = ctx.rpc_calls.iter().any(|call| call.contains(expected.as_str()));
            println!("[rpc calls] {:?}", ctx.rpc_calls);
            if !hit {
                failures.push(format!(
                    "no rpc call contains {:?} (made: {:?})",
                    expected, ctx.rpc_calls
                ));
            }
        }
        if let Some(expected) = &expectation.expect_quota {
            let hit = ctx
                .quota_calls
                .iter()
                .any(|call| call.contains(expected.as_str()));
            println!("[quota calls] {:?}", ctx.quota_calls);
            if !hit {
                failures.push(format!(
                    "no quota call contains {:?} (made: {:?})",
                    expected, ctx.quota_calls
                ));
            }
        }
        if let Some(expected) = &expectation.expect_retry {
            let hit = ctx
                .retry_calls
                .iter()
                .any(|call| call.contains(expected.as_str()));
            println!("[retry calls] {:?}", ctx.retry_calls);
            if !hit {
                failures.push(format!(
                    "no retry-policy call contains {:?} (made: {:?})",
                    expected, ctx.retry_calls
                ));
            }
        }
        if let Some(expected) = &expectation.expect_host_call {
            let hit = ctx
                .host_api_calls
                .iter()
                .any(|call| call.contains(expected.as_str()));
            println!("[host api calls] {:?}", ctx.host_api_calls);
            if !hit {
                failures.push(format!(
                    "no golem:api/host call contains {:?} (made: {:?})",
                    expected, ctx.host_api_calls
                ));
            }
        }
        if let Some(expected) = &expectation.expect_kv {
            let calls = ctx.kv_calls.clone();
            let hit = calls.iter().any(|call| call.contains(expected.as_str()));
            println!("[key-value calls] {:?}", calls);
            if !hit {
                failures.push(format!(
                    "no key-value call contains {:?} (made: {:?})",
                    expected, calls
                ));
            }
        }
        if let Some(expected) = &expectation.expect_blob {
            let calls = ctx.blob_calls.clone();
            let hit = calls.iter().any(|call| call.contains(expected.as_str()));
            println!("[blobstore calls] {:?}", calls);
            if !hit {
                failures.push(format!(
                    "no blobstore call contains {:?} (made: {:?})",
                    expected, calls
                ));
            }
        }
        if let Some(expected) = &expectation.expect_fs {
            let calls = ctx.fs_calls.clone();
            let hit = calls.iter().any(|call| call.contains(expected.as_str()));
            println!("[filesystem calls] {:?}", calls);
            if !hit {
                failures.push(format!(
                    "no filesystem call contains {:?} (made: {:?})",
                    expected, calls
                ));
            }
        }
        if let Some(expected) = &expectation.expect_db {
            let calls = ctx.db_calls.clone();
            let hit = calls.iter().any(|call| call.contains(expected.as_str()));
            println!("[database calls] {:?}", calls);
            if !hit {
                failures.push(format!(
                    "no database call contains {:?} (made: {:?})",
                    expected, calls
                ));
            }
        }
        if let Some(expected) = &expectation.expect_sent {
            let hit = ctx.sent.iter().any(|msg| msg.contains(expected.as_str()));
            println!("[websocket sent] {:?}", ctx.sent);
            if !hit {
                failures.push(format!(
                    "no websocket message contains {:?} (sent: {:?})",
                    expected, ctx.sent
                ));
            }
        }
    }

    if failures.is_empty() {
        println!("\nPASS {}", expectation.component.display());
        Ok(ExitCode::SUCCESS)
    } else {
        println!("\nFAIL {}", expectation.component.display());
        for failure in &failures {
            println!("  - {}", failure);
        }
        Ok(ExitCode::FAILURE)
    }
}
