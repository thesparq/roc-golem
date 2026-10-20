//! A minimal HTTP router built with the official Rust SDK, to answer one
//! question: does a body written from a task spawned by the handler reach the
//! client? `handle` returns the response (and its body stream) immediately; the
//! write is queued with the SDK's `spawn_local`, exactly the shape the Roc
//! platform cannot make work.

use golem_rust::agentic::{spawn_local, AgentStream, Config, Header, HttpRequest, HttpResponse, HttpRouter};

struct ProbeRouter;

#[golem_rust::http_router(name = "SdkProbeRouter", mount = "/sdk-probe")]
impl HttpRouter for ProbeRouter {
    type Config = ();

    fn new(_config: Config<()>) -> Self {
        ProbeRouter
    }

    async fn handle(&self, request: HttpRequest) -> HttpResponse {
        let (mut writer, body) = AgentStream::<Vec<u8>>::new();
        let text = format!("sdk probe body for {}", request.path).into_bytes();
        spawn_local(async move {
            let _ = writer.write_one(text).await;
        });

        HttpResponse {
            status: 200,
            headers: vec![Header {
                name: "content-type".to_string(),
                value: b"text/plain".to_vec(),
            }],
            body,
        }
    }
}
