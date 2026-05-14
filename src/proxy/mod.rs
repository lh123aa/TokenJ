pub mod tls;

use crate::config::Config;
use crate::db::{Database, RequestRecord};
use crate::pricing::calculate_saving;
use crate::provider;
use crate::provider::Provider;
use anyhow::Result;
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ProxyEvent {
    pub provider: String,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_tokens: u64,
    pub saving_cents: f64,
    pub saving_rate: f64,
    pub cache_injected: bool,
    pub duration_ms: u64,
    pub cache_write_tokens: u64,
}

pub struct Proxy {
    config: Arc<Config>,
    db: Arc<Database>,
    event_tx: broadcast::Sender<ProxyEvent>,
}

impl Proxy {
    pub fn new(
        config: Arc<Config>,
        db: Arc<Database>,
        event_tx: broadcast::Sender<ProxyEvent>,
    ) -> Self {
        Self { config, db, event_tx }
    }

    pub async fn run(&self) -> Result<()> {
        let addr = SocketAddr::from(([127, 0, 0, 1], self.config.port));
        let listener = TcpListener::bind(addr).await?;

        info!("tokenJ proxy running on http://127.0.0.1:{}", self.config.port);

        let svc = self.clone();

        loop {
            let (stream, peer_addr) = listener.accept().await?;
            let svc = svc.clone();

            tokio::spawn(async move {
                let io = TokioIo::new(stream);
                if let Err(e) = http1::Builder::new()
                    .preserve_header_case(true)
                    .title_case_headers(true)
                    .serve_connection(io, service_fn(move |req| {
                        svc.clone().handle_request(req)
                    }))
                    .await
                {
                    warn!("Connection error from {}: {}", peer_addr, e);
                }
            });
        }
    }

    async fn handle_request(
        self,
        req: Request<Incoming>,
    ) -> Result<Response<Full<Bytes>>, hyper::Error> {
        if req.method() == Method::CONNECT {
            warn!("CONNECT method not supported in direct mode, use base_url config instead");
            let mut resp = Response::new(Full::from(Bytes::new()));
            *resp.status_mut() = StatusCode::BAD_REQUEST;
            return Ok(resp);
        }

        let host = req.uri().host().unwrap_or("").to_string();
        let provider = Provider::from_host(&host);

        // Read body
        let (parts, body) = req.into_parts();
        let body_bytes = body.collect().await.map(|b| b.to_bytes()).unwrap_or_default();

        // Inject cache headers
        let mut json_body: Option<serde_json::Value> = None;
        let injection = if let Ok(mut val) = serde_json::from_slice::<serde_json::Value>(&body_bytes)
        {
            let result = provider::inject_cache_headers(&provider, &mut val);
            json_body = Some(val);
            result
        } else {
            provider::CacheInjection { injected: false, details: vec![] }
        };

        let final_body = if let Some(ref val) = json_body {
            serde_json::to_vec(val).unwrap_or(body_bytes.to_vec())
        } else {
            body_bytes.to_vec()
        };

        let model = json_body
            .as_ref()
            .and_then(|v| v.get("model"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        // Forward the request
        let start = std::time::Instant::now();
        let url = format!("https://{}{}",
            host,
            parts.uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("")
        );

        let client = match reqwest::Client::builder()
            .no_proxy()
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to create HTTP client: {}", e);
                let mut resp = Response::new(Full::from(format!("Proxy error: {}", e)));
                *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                return Ok(resp);
            }
        };

        let mut req_builder = client.request(parts.method.clone(), &url);
        for (name, value) in &parts.headers {
            let n = name.as_str().to_lowercase();
            if n != "host" && n != "proxy-connection" && n != "transfer-encoding" {
                req_builder = req_builder.header(name, value);
            }
        }
        req_builder = req_builder.body(final_body);

        match req_builder.send().await {
            Ok(resp) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                let status = resp.status();
                let resp_headers = resp.headers().clone();
                let resp_body = resp.bytes().await.unwrap_or_default();

                let cache_result = if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&resp_body) {
                    provider::parse_cache_result(&provider, &val)
                } else {
                    provider::CacheResult::default()
                };

                let saving = calculate_saving(
                    provider.name(), &model,
                    cache_result.input_tokens, cache_result.output_tokens,
                    cache_result.cached_tokens, cache_result.cache_write_tokens,
                    &self.config,
                );

                let _ = self.db.insert_request(&RequestRecord {
                    id: Uuid::new_v4().to_string(),
                    session_id: "default".into(),
                    provider: provider.name().into(),
                    model: model.clone(),
                    input_tokens: cache_result.input_tokens,
                    output_tokens: cache_result.output_tokens,
                    cached_tokens: cache_result.cached_tokens,
                    cache_write_tokens: cache_result.cache_write_tokens,
                    actual_cost_cents: saving.actual_cost_cents,
                    saving_cents: saving.saving_cents,
                    saving_rate: saving.saving_rate,
                    cache_injected: injection.injected,
                    duration_ms,
                    created_at: chrono::Utc::now().to_rfc3339(),
                });

                let _ = self.event_tx.send(ProxyEvent {
                    provider: provider.name().into(),
                    model,
                    input_tokens: cache_result.input_tokens,
                    output_tokens: cache_result.output_tokens,
                    cached_tokens: cache_result.cached_tokens,
                    cache_write_tokens: cache_result.cache_write_tokens,
                    saving_cents: saving.saving_cents,
                    saving_rate: saving.saving_rate,
                    cache_injected: injection.injected,
                    duration_ms,
                });

                let mut response = Response::new(Full::from(resp_body.to_vec()));
                *response.status_mut() = status;
                for (name, value) in resp_headers.iter() {
                    let n = name.as_str().to_lowercase();
                    if n != "transfer-encoding" {
                        response.headers_mut().insert(name, value.clone());
                    }
                }
                Ok(response)
            }
            Err(e) => {
                warn!("Forward failed: {}", e);
                let body = format!("tokenJ proxy error: {}", e);
                let mut resp = Response::new(Full::from(body));
                *resp.status_mut() = StatusCode::BAD_GATEWAY;
                Ok(resp)
            }
        }
    }
}

impl Clone for Proxy {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            db: self.db.clone(),
            event_tx: self.event_tx.clone(),
        }
    }
}
