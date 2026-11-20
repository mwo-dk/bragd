//! The API over HTTP. TLS is the host's proxy's; this listens on the host only.
//!
//! Each connection carries one request and has [`DEADLINE`] to send it and read the answer, so a
//! client that announces a body and never sends it, or never reads, is dropped. Reading a request
//! takes no worker: at most `threads` requests reach the database at once.

use std::sync::Arc;
use std::time::Duration;

use http_body_util::{BodyExt, Full, Limited};
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::sync::Semaphore;

use crate::api::{MAX_BODY, Service};

/// How long one connection may take, from its first byte to the end of the answer.
pub const DEADLINE: Duration = Duration::from_secs(20);

/// Serves `service` on `addr` with `threads` workers, until the program ends.
pub fn serve(service: Service, addr: &str, threads: usize) -> Result<(), String> {
    let (runtime, listener) = bind(addr)?;
    runtime.block_on(run(listener, Arc::new(service), threads, DEADLINE));
    Ok(())
}

/// The address `serve` listens on, for a test that asked for port 0.
pub fn start(service: Service, addr: &str) -> Result<String, String> {
    start_with(service, addr, 1, DEADLINE)
}

/// [`start`] with `threads` workers and a `deadline` of its own.
pub fn start_with(service: Service, addr: &str, threads: usize, deadline: Duration) -> Result<String, String> {
    let (runtime, listener) = bind(addr)?;
    let bound = listener.local_addr().map_err(|e| e.to_string())?.to_string();
    std::thread::spawn(move || runtime.block_on(run(listener, Arc::new(service), threads, deadline)));
    Ok(bound)
}

fn bind(addr: &str) -> Result<(tokio::runtime::Runtime, std::net::TcpListener), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().map_err(|e| e.to_string())?;
    let listener = std::net::TcpListener::bind(addr).map_err(|e| format!("{addr}: {e}"))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    Ok((runtime, listener))
}

async fn run(listener: std::net::TcpListener, service: Arc<Service>, threads: usize, deadline: Duration) {
    let listener = tokio::net::TcpListener::from_std(listener).expect("a listener inside the runtime");
    let gate = Arc::new(Semaphore::new(threads.max(1)));
    loop {
        // Out of file descriptors, say: wait a moment rather than spin.
        let Ok((stream, _)) = listener.accept().await else {
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        };
        let (service, gate) = (service.clone(), gate.clone());
        tokio::spawn(async move {
            let answer = hyper::service::service_fn(move |req| answer(service.clone(), gate.clone(), req));
            let conn = hyper::server::conn::http1::Builder::new().keep_alive(false).serve_connection(TokioIo::new(stream), answer);
            let _ = tokio::time::timeout(deadline, conn).await;
        });
    }
}

async fn answer(service: Arc<Service>, gate: Arc<Semaphore>, req: Request<Incoming>) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let method = req.method().as_str().to_string();
    let path = req.uri().path_and_query().map(|p| p.as_str().to_string()).unwrap_or_default();
    let auth = req.headers().get(hyper::header::AUTHORIZATION).and_then(|v| v.to_str().ok()).map(str::to_string);
    let body = Limited::new(req.into_body(), MAX_BODY).collect().await.ok().and_then(|b| String::from_utf8(b.to_bytes().to_vec()).ok());
    let (status, value) = match body {
        None => (413, serde_json::json!({ "error": "bad_request" })),
        Some(body) => {
            let permit = gate.acquire_owned().await.expect("the gate is never closed");
            // The permit goes with the work, which finishes even when the client is gone.
            tokio::task::spawn_blocking(move || {
                let _permit = permit;
                service.handle(&method, &path, auth.as_deref(), &body)
            })
            .await
            .unwrap_or((500, serde_json::json!({ "error": "server" })))
        }
    };
    let mut r = Response::new(Full::new(Bytes::from(value.to_string())));
    *r.status_mut() = hyper::StatusCode::from_u16(status).unwrap_or(hyper::StatusCode::INTERNAL_SERVER_ERROR);
    r.headers_mut().insert(hyper::header::CONTENT_TYPE, hyper::header::HeaderValue::from_static("application/json"));
    Ok(r)
}
