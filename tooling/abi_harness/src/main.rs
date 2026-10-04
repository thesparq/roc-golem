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
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

bindgen!({
    path: "wit",
    world: "harness-world",
});

use exports::golem::agent::guest::Principal;
use golem::core::types::{DataValue, ElementValue, TextReference, TextSource};

/// Dummy host state. The linked component only needs a monotonic clock and an
/// empty poll set, and neither influences the ABI checks below.
struct Ctx {
    wasi: WasiCtx,
    table: wasmtime_wasi::ResourceTable,
}

impl WasiView for Ctx {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
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
        unimplemented!("not used by roc-golem components")
    }
    fn subscribe_instant(
        &mut self,
        _when: u64,
    ) -> wasmtime::component::Resource<wasi::io::poll::Pollable> {
        unimplemented!("not used by roc-golem components")
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

/// What the component under test is expected to report and do.
struct Expectation {
    component: PathBuf,
    agent_type: Option<String>,
    method: Option<String>,
    message: String,
    reply_contains: Option<String>,
    /// Reply expected from a second invocation of the same message, which shows
    /// that state survived between calls.
    then_reply_contains: Option<String>,
}

fn usage() -> String {
    String::from(
        "usage: abi_harness <component.wasm> [--type NAME] [--method NAME] \
         [--message TEXT] [--reply SUBSTRING] [--then-reply SUBSTRING]",
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
        then_reply_contains: None,
    };

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--type" => expectation.agent_type = args.next(),
            "--method" => expectation.method = args.next(),
            "--message" => {
                expectation.message = args.next().context("--message needs a value")?
            }
            "--reply" => expectation.reply_contains = args.next(),
            "--then-reply" => expectation.then_reply_contains = args.next(),
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

fn text_value(s: &str) -> DataValue {
    DataValue::Tuple(vec![ElementValue::UnstructuredText(
        TextReference::Inline(TextSource {
            data: s.to_string(),
            text_type: None,
        }),
    )])
}

fn text_of(value: &DataValue) -> Option<String> {
    match value {
        DataValue::Tuple(elements) => match elements.first() {
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

    let mut linker = Linker::<Ctx>::new(&engine);
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;
    HarnessWorld::add_to_linker::<Ctx, Ctx>(&mut linker, |ctx: &mut Ctx| ctx)?;

    let mut store = Store::new(
        &engine,
        Ctx {
            wasi: WasiCtx::builder().build(),
            table: Default::default(),
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
    match guest.call_initialize(&mut store, "harness", &text_value("{}"), &principal) {
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

    // 4. Message round-trips. `invoke` returns the app's reply text, so the
    //    reply is matched directly; a second call with the same message shows
    //    whether state survived.
    for (label, expected) in [
        ("invoke handle-message", &expectation.reply_contains),
        ("invoke handle-message again", &expectation.then_reply_contains),
    ] {
        if expected.is_none() {
            continue;
        }
        match guest.call_invoke(
            &mut store,
            "handle-message",
            &text_value(&expectation.message),
            &principal,
        ) {
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
