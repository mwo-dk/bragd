//! The API over HTTP. TLS is the host's proxy's; this listens on the host only.

use std::io::Read;
use std::sync::Arc;

use crate::api::{MAX_BODY, Service};

/// Serves `service` on `addr` with `threads` workers, until the program ends.
pub fn serve(service: Service, addr: &str, threads: usize) -> Result<(), String> {
    let server = Arc::new(tiny_http::Server::http(addr).map_err(|e| format!("{addr}: {e}"))?);
    let service = Arc::new(service);
    let workers: Vec<_> = (0..threads.max(1))
        .map(|_| {
            let (server, service) = (server.clone(), service.clone());
            std::thread::spawn(move || {
                while let Ok(req) = server.recv() {
                    answer(&service, req);
                }
            })
        })
        .collect();
    for w in workers {
        let _ = w.join();
    }
    Ok(())
}

/// The address `serve` listens on, for a test that asked for port 0.
pub fn start(service: Service, addr: &str) -> Result<String, String> {
    let server = tiny_http::Server::http(addr).map_err(|e| format!("{addr}: {e}"))?;
    let bound = server.server_addr().to_ip().map(|a| a.to_string()).unwrap_or_default();
    std::thread::spawn(move || {
        while let Ok(req) = server.recv() {
            answer(&service, req);
        }
    });
    Ok(bound)
}

fn answer(service: &Service, mut req: tiny_http::Request) {
    let auth = req.headers().iter().find(|h| h.field.equiv("Authorization")).map(|h| h.value.as_str().to_string());
    let mut body = String::new();
    let read = req.as_reader().take(MAX_BODY as u64 + 1).read_to_string(&mut body);
    let (status, value) = if read.is_err() || body.len() > MAX_BODY {
        (413, serde_json::json!({ "error": "bad_request" }))
    } else {
        service.handle(req.method().as_str(), req.url(), auth.as_deref(), &body)
    };
    let json = tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).expect("a header");
    let _ = req.respond(tiny_http::Response::from_string(value.to_string()).with_status_code(status).with_header(json));
}
