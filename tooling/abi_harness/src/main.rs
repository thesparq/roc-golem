//! Runs a built roc-golem component outside Golem.
//!
//! Golem's runtime is the only other consumer of these components, so anything
//! the host bridge gets wrong (string layout, Result tagging, allocator
//! bookkeeping) shows up here as a wasm trap or a garbled payload rather than as
//! a remote deployment failure. Run `./tooling/build.sh all` first, then point
//! this at each component in `build/`.

use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::process::ExitCode;
use wasmtime::component::{bindgen, Component, Linker};
use wasmtime::{Config, Engine, Store};
use wasmtime::component::ResourceTable;

mod stubs;

bindgen!({
    path: "wit",
    world: "harness-world",
});

use exports::golem::agent::guest::Principal;
use golem::core::types::{
    DataValue, ElementValue, TextReference, TextSource, WitNode, WitValue,
};

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
pub fn method_name(method: &wasi::http::types::Method) -> String {
    match method {
        wasi::http::types::Method::Get => "GET",
        wasi::http::types::Method::Head => "HEAD",
        wasi::http::types::Method::Post => "POST",
        wasi::http::types::Method::Put => "PUT",
        wasi::http::types::Method::Delete => "DELETE",
        wasi::http::types::Method::Connect => "CONNECT",
        wasi::http::types::Method::Options => "OPTIONS",
        wasi::http::types::Method::Trace => "TRACE",
        wasi::http::types::Method::Patch => "PATCH",
        wasi::http::types::Method::Other(other) => other.as_str(),
    }
    .to_string()
}

/// A stand-in WebSocket connection that records what the agent sends.
struct StubConnection {
    sent: Vec<String>,
    closed: bool,
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

impl golem::agent::common::Host for Ctx {}

impl wasi::clocks::monotonic_clock::Host for Ctx {
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

impl wasi::clocks::wall_clock::Host for Ctx {
    fn now(&mut self) -> wasi::clocks::wall_clock::Datetime {
        wasi::clocks::wall_clock::Datetime {
            seconds: 0,
            nanoseconds: 0,
        }
    }
    fn resolution(&mut self) -> wasi::clocks::wall_clock::Datetime {
        wasi::clocks::wall_clock::Datetime {
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

    fn receive(
        &mut self,
        _self_: wasmtime::component::Resource<golem::websocket::client::WebsocketConnection>,
    ) -> Result<golem::websocket::client::Message, golem::websocket::client::Error> {
        Ok(golem::websocket::client::Message::Text(String::from(
            "stub-reply",
        )))
    }

    fn receive_with_timeout(
        &mut self,
        _self_: wasmtime::component::Resource<golem::websocket::client::WebsocketConnection>,
        _timeout_ms: u64,
    ) -> Result<Option<golem::websocket::client::Message>, golem::websocket::client::Error> {
        Ok(Some(golem::websocket::client::Message::Text(
            String::from("stub-reply"),
        )))
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

    fn subscribe(
        &mut self,
        _self_: wasmtime::component::Resource<golem::websocket::client::WebsocketConnection>,
    ) -> wasmtime::component::Resource<wasi::io::poll::Pollable> {
        unsafe { wasmtime::component::Resource::new_own(0) }
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

    fn make_agent_id(
        &mut self,
        agent_type_name: String,
        _input: golem::agent::common::DataValue,
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
            golem::agent::common::DataValue,
            Option<golem::core::types::Uuid>,
        ),
        golem::agent::common::AgentError,
    > {
        // Accept whatever the agent passes and treat it as the stub target, so
        // the call reaches `invoke-and-await` below.
        Ok((
            agent_id,
            golem::agent::common::DataValue::Tuple(Vec::new()),
            None,
        ))
    }

    fn create_webhook(&mut self, _promise_id: golem::core::types::PromiseId) -> String {
        String::from("http://stub/webhook")
    }

    fn get_config_value(
        &mut self,
        _key: Vec<String>,
        _expected_type: golem::core::types::WitType,
    ) -> golem::core::types::WitValue {
        unimplemented!("get-config-value is not used by roc-golem components")
    }
}

impl golem::agent::host::HostWasmRpc for Ctx {
    fn new(
        &mut self,
        agent_type_name: String,
        _constructor: golem::agent::common::DataValue,
        _phantom_id: Option<golem::core::types::Uuid>,
        _agent_config: Vec<golem::agent::common::TypedAgentConfigValue>,
    ) -> wasmtime::component::Resource<golem::agent::host::WasmRpc> {
        let rep = self.rpc_calls.len() as u32;
        self.rpc_calls.push(format!("new({agent_type_name})"));
        unsafe { wasmtime::component::Resource::new_own(rep) }
    }

    fn invoke_and_await(
        &mut self,
        _self_: wasmtime::component::Resource<golem::agent::host::WasmRpc>,
        method_name: String,
        _input: golem::agent::common::DataValue,
    ) -> Result<golem::agent::common::DataValue, golem::agent::host::RpcError> {
        self.rpc_calls.push(format!("invoke-and-await({method_name})"));
        Ok(golem::agent::common::DataValue::Tuple(vec![
            golem::core::types::ElementValue::UnstructuredText(
                golem::core::types::TextReference::Inline(golem::core::types::TextSource {
                    data: String::from("stub-rpc-reply"),
                    text_type: None,
                }),
            ),
        ]))
    }

    fn invoke(
        &mut self,
        _self_: wasmtime::component::Resource<golem::agent::host::WasmRpc>,
        method_name: String,
        _input: golem::agent::common::DataValue,
    ) -> Result<(), golem::agent::host::RpcError> {
        self.rpc_calls.push(format!("invoke({method_name})"));
        Ok(())
    }

    fn async_invoke_and_await(
        &mut self,
        _self_: wasmtime::component::Resource<golem::agent::host::WasmRpc>,
        method_name: String,
        _input: golem::agent::common::DataValue,
    ) -> wasmtime::component::Resource<golem::agent::host::FutureInvokeResult> {
        self.rpc_calls.push(format!("async-invoke-and-await({method_name})"));
        unsafe { wasmtime::component::Resource::new_own(0) }
    }

    fn schedule_invocation(
        &mut self,
        _self_: wasmtime::component::Resource<golem::agent::host::WasmRpc>,
        _scheduled_time: wasi::clocks::wall_clock::Datetime,
        _method_name: String,
        _input: golem::agent::common::DataValue,
    ) {
        unimplemented!("schedule-invocation is not used by roc-golem components")
    }

    fn schedule_cancelable_invocation(
        &mut self,
        _self_: wasmtime::component::Resource<golem::agent::host::WasmRpc>,
        _scheduled_time: wasi::clocks::wall_clock::Datetime,
        _method_name: String,
        _input: golem::agent::common::DataValue,
    ) -> wasmtime::component::Resource<golem::agent::host::CancellationToken> {
        unimplemented!("schedule-cancelable-invocation is not used by roc-golem components")
    }

    fn drop(
        &mut self,
        _rep: wasmtime::component::Resource<golem::agent::host::WasmRpc>,
    ) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl golem::agent::host::HostFutureInvokeResult for Ctx {
    fn subscribe(
        &mut self,
        _self_: wasmtime::component::Resource<golem::agent::host::FutureInvokeResult>,
    ) -> wasmtime::component::Resource<wasi::io::poll::Pollable> {
        unsafe { wasmtime::component::Resource::new_own(0) }
    }

    fn get(
        &mut self,
        _self_: wasmtime::component::Resource<golem::agent::host::FutureInvokeResult>,
    ) -> Option<Result<golem::agent::common::DataValue, golem::agent::host::RpcError>> {
        None
    }

    fn cancel(&mut self, _self_: wasmtime::component::Resource<golem::agent::host::FutureInvokeResult>) {}

    fn drop(
        &mut self,
        _rep: wasmtime::component::Resource<golem::agent::host::FutureInvokeResult>,
    ) -> wasmtime::Result<()> {
        Ok(())
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
}

fn usage() -> String {
    String::from(
        "usage: abi_harness <component.wasm> [--type NAME] [--method NAME] \
         [--message TEXT] [--reply SUBSTRING] [--then-message TEXT] \
         [--then-reply SUBSTRING] [--expect-log SUBSTRING] [--expect-sent SUBSTRING] \\
         [--config TEXT] [--expect-http SUBSTRING] [--expect-rpc SUBSTRING]",
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

/// A single component-model `string`, which is what the platform declares in
/// its agent method schemas (Golem type-checks arguments against them).
fn text_value(s: &str) -> DataValue {
    DataValue::Tuple(vec![ElementValue::ComponentModel(WitValue {
        nodes: vec![WitNode::PrimString(s.to_string())],
    })])
}

fn text_of(value: &DataValue) -> Option<String> {
    match value {
        DataValue::Tuple(elements) => match elements.first() {
            Some(ElementValue::ComponentModel(wit)) => match wit.nodes.first() {
                Some(WitNode::PrimString(text)) => Some(text.clone()),
                _ => None,
            },
            Some(ElementValue::UnstructuredText(TextReference::Inline(source))) => {
                Some(source.data.clone())
            }
            Some(ElementValue::UnstructuredText(TextReference::Url(url))) => Some(url.clone()),
            _ => None,
        },
        DataValue::Multimodal(elements) => match elements.first() {
            Some((_, ElementValue::UnstructuredText(TextReference::Inline(source)))) => {
                Some(source.data.clone())
            }
            _ => None,
        },
    }
}

fn main() -> Result<ExitCode> {
    let expectation = parse_args()?;

    let mut config = Config::new();
    config.wasm_component_model(true);
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
    HarnessWorld::add_to_linker::<Ctx, Ctx>(&mut linker, |ctx: &mut Ctx| ctx)?;

    let mut store = Store::new(
        &engine,
        Ctx {
            table: Default::default(),
            logs: Vec::new(),
            sent: Vec::new(),
            connections: Vec::new(),
            rpc_calls: Vec::new(),
            fields_table: Vec::new(),
            requests_table: Vec::new(),
            body_slots: Vec::new(),
            request_body: Vec::new(),
            response_body: Vec::new(),
            response_cursor: 0,
            http_request_lines: Vec::new(),
        },
    );
    let bindings = HarnessWorld::instantiate(&mut store, &component, &linker).map_err(|err| {
        anyhow::anyhow!(
            "failed to instantiate {}: {err}",
            expectation.component.display()
        )
    })?;

    let guest = bindings.golem_agent_guest();
    let principal = Principal::Anonymous;
    let mut failures: Vec<String> = Vec::new();

    // 1. Agent definition: exercises the metadata hook, which only uses strings
    //    longer than the 11-byte small-string limit.
    let definition = guest.call_get_definition(&mut store)?;
    let methods: Vec<String> = definition.methods.iter().map(|m| m.name.clone()).collect();
    println!("[get-definition] type-name = {:?}", definition.type_name);
    println!("[get-definition] methods   = {:?}", methods);
    if let Some(expected) = &expectation.agent_type {
        if &definition.type_name != expected {
            failures.push(format!(
                "agent type name is {:?}, expected {:?} (the host falls back to \"roc-agent\" when the app's metadata cannot be read)",
                definition.type_name, expected
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
    match guest.call_discover_agent_types(&mut store)? {
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
    match guest.call_initialize(&mut store, "harness", &text_value(&agent_config), &principal) {
        Ok(Ok(())) => println!("[initialize]               = ok"),
        Ok(Err(err)) => {
            println!("[initialize]               = agent error {:?}", err);
            failures.push(format!("initialize returned an error: {:?}", err));
        }
        Err(err) => {
            println!("[initialize]               = trap\n{}", err);
            failures.push(String::from("initialize trapped"));
        }
    }

    // 4. Message round-trips. `invoke` returns the app's reply text, so replies
    //    are matched directly; a second call shows whether state survived.
    let second_message = expectation
        .then_message
        .clone()
        .unwrap_or_else(|| expectation.message.clone());
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
        if expected.is_none() {
            continue;
        }
        match guest.call_invoke(&mut store, "handle-message", &text_value(&message), &principal) {
            Ok(Ok(value)) => {
                let raw = text_of(&value).unwrap_or_default();
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
            Ok(Err(err)) => {
                println!("[{}] = agent error {:?}", label, err);
                failures.push(format!("{} returned an error: {:?}", label, err));
            }
            Err(err) => {
                println!("[{}] = trap\n{}", label, err);
                failures.push(format!("{} trapped", label));
            }
        }
    }

    // 5. Effects the agent reached for: log lines and WebSocket sends are
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
