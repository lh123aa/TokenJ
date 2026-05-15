pub mod tls;

use crate::cert::CertManager;
use crate::config::Config;
use crate::db::{Database, RequestRecord};
use crate::pricing::calculate_saving;
use crate::provider;
use crate::provider::gemini_cache::GeminiCacheStore;
use crate::provider::Provider;
use anyhow::Result;
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_rustls::TlsAcceptor;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// 创建一个可复用的 HTTP 客户端，带合理的超时配置
fn build_http_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(120))
        .connect_timeout(std::time::Duration::from_secs(10))
        .pool_max_idle_per_host(32)
        .pool_idle_timeout(std::time::Duration::from_secs(90))
        .build()
}

/// 从字节缓冲区解析 CONNECT 请求，返回 (host, port)
fn parse_connect_request(buf: &[u8]) -> Option<(String, u16)> {
    let s = std::str::from_utf8(buf).ok()?;
    let first_line = s.lines().next()?;
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() < 2 || parts[0].to_uppercase() != "CONNECT" {
        return None;
    }
    let authority = parts[1];
    let (host, port) = tls::parse_host(authority);
    Some((host, port))
}

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
    cert_manager: Arc<CertManager>,
    gemini_cache: GeminiCacheStore,
    http_client: reqwest::Client,
}

impl Proxy {
    pub fn new(
        config: Arc<Config>,
        db: Arc<Database>,
        event_tx: broadcast::Sender<ProxyEvent>,
        cert_manager: Arc<CertManager>,
    ) -> Self {
        let http_client = build_http_client().unwrap_or_else(|e| {
            warn!("Failed to create HTTP client with custom config: {}, using default", e);
            reqwest::Client::builder().no_proxy().build().expect("Default HTTP client should work")
        });
        Self {
            config, db, event_tx, cert_manager,
            gemini_cache: GeminiCacheStore::new(),
            http_client,
        }
    }

    pub async fn run(&self) -> Result<()> {
        let addr = SocketAddr::from(([127, 0, 0, 1], self.config.port));
        let listener = TcpListener::bind(addr).await?;

        info!("TokenJ proxy running on http://127.0.0.1:{}", self.config.port);

        let svc = self.clone();
        let accept_loop = async {
            loop {
                match listener.accept().await {
                    Ok((stream, peer_addr)) => {
                        let svc = svc.clone();
                        tokio::spawn(async move {
                            let mut peek_buf = [0u8; 7];
                            match stream.peek(&mut peek_buf).await {
                                Ok(n) if n >= 7 && &peek_buf[..7] == b"CONNECT" => {
                                    if let Err(e) = svc.handle_connect_tunnel(stream).await {
                                        warn!("CONNECT tunnel error from {}: {}", peer_addr, e);
                                    }
                                }
                                _ => {
                                    let io = TokioIo::new(stream);
                                    if let Err(e) = http1::Builder::new()
                                        .preserve_header_case(true)
                                        .title_case_headers(true)
                                        .serve_connection(io, service_fn(move |req| {
                                            svc.clone().handle_direct_request(req)
                                        }))
                                        .await
                                    {
                                        warn!("Connection error from {}: {}", peer_addr, e);
                                    }
                                }
                            }
                        });
                    }
                    Err(e) => {
                        // 监听器被关闭时正常退出
                        if e.kind() == std::io::ErrorKind::ConnectionReset
                            || e.kind() == std::io::ErrorKind::NotConnected
                        {
                            break;
                        }
                        warn!("Accept error: {}", e);
                    }
                }
            }
            Ok::<_, anyhow::Error>(())
        };

        tokio::select! {
            result = accept_loop => {
                if let Err(e) = result {
                    warn!("Accept loop error: {}", e);
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Shutdown signal received, stopping proxy...");
                // 停止接受新连接，现有 in-flight 请求继续处理
                drop(listener);
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }

        info!("TokenJ proxy stopped.");
        Ok(())
    }

    /// 处理 CONNECT 隧道（HTTPS_PROXY 模式）
    /// LLM 域名走 MITM 拦截（缓存注入），其他域名透传
    async fn handle_connect_tunnel(&self, mut stream: tokio::net::TcpStream) -> Result<()> {
        // 用缓冲区读取完整请求头直到 \r\n\r\n
        let mut buf = vec![0u8; 4096];
        let mut pos = 0;

        loop {
            let n = stream.read(&mut buf[pos..]).await?;
            if n == 0 {
                return Ok(());
            }
            pos += n;
            if pos >= 4 && buf[..pos].windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
            if pos >= buf.len() {
                buf.resize(buf.len() * 2, 0);
            }
        }

        // 解析 CONNECT 请求
        let (host, port) = match parse_connect_request(&buf[..pos]) {
            Some(hp) => hp,
            None => {
                warn!("Malformed CONNECT request");
                let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
                return Ok(());
            }
        };

        let is_llm = tls::is_llm_domain(&host);

        // 对 LLM 域名做 MITM 拦截（需要 443 端口，否则降级透传）
        if is_llm && port == 443 {
            info!("LLM domain CONNECT MITM: {}:{}", host, port);
            stream
                .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                .await?;
            return self.handle_connect_mitm(stream, &host).await;
        }

        // 非 LLM 域名 → 透传隧道
        info!("CONNECT passthrough: {}:{}", host, port);
        let target_addr = format!("{}:{}", host, port);
        let target_stream = match tokio::net::TcpStream::connect(&target_addr).await {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to connect to {}: {}", target_addr, e);
                let _ = stream
                    .write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n")
                    .await;
                return Ok(());
            }
        };

        stream
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;

        let mut server_stream = target_stream;
        // 透传加 5 分钟超时，防止连接被僵尸占用
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(300),
            tokio::io::copy_bidirectional(&mut stream, &mut server_stream),
        )
        .await;
        match result {
            Ok(Ok(_)) => debug!("CONNECT passthrough closed: {}:{}", host, port),
            Ok(Err(e)) => warn!("CONNECT passthrough error for {}:{}: {}", host, port, e),
            Err(_) => warn!("CONNECT passthrough timed out for {}:{} after 300s", host, port),
        }
        Ok(())
    }

    /// TLS MITM 拦截：对 LLM 域名解密 TLS，注入缓存后转发
    async fn handle_connect_mitm(
        &self,
        stream: tokio::net::TcpStream,
        host: &str,
    ) -> Result<()> {
        // 1. 使用 CA 证书动态签发目标域名证书（异步生成，不阻塞 tokio 线程）
        let (cert_pem, key_pem) = self
            .cert_manager
            .get_or_create_domain_cert_pem_async(host)
            .await
            .map_err(|e| {
                warn!("Failed to generate cert for {}: {}", host, e);
                e
            })?;

        // 2. 构建 rustls ServerConfig
        let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_slice_iter(cert_pem.as_bytes())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("Failed to parse cert PEM: {}", e))?;
        let key = PrivateKeyDer::from_pem_slice(key_pem.as_bytes())
            .map_err(|e| anyhow::anyhow!("Failed to parse key PEM: {}", e))?;

        let server_config = Arc::new(
            rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(certs, key)
                .map_err(|e| anyhow::anyhow!("Failed to build TLS config: {}", e))?,
        );

        // 3. 接受 TLS 握手
        let acceptor = TlsAcceptor::from(server_config);
        let tls_stream = match acceptor.accept(stream).await {
            Ok(s) => s,
            Err(e) => {
                warn!("TLS handshake failed for {}: {}", host, e);
                return Ok(());
            }
        };

        // 4. 在 TLS 隧道内运行 HTTP 服务
        let io = TokioIo::new(tls_stream);

        let target_host = host.to_string();
        let config = self.config.clone();
        let db = self.db.clone();
        let event_tx = self.event_tx.clone();
        let http_client = self.http_client.clone();

        let service = service_fn(move |req: Request<Incoming>| {
            let target_host = target_host.clone();
            let config = config.clone();
            let db = db.clone();
            let event_tx = event_tx.clone();
            let http_client = http_client.clone();

            async move {
                let provider = Provider::from_host(&target_host);

                // 读取请求体
                let (parts, body) = req.into_parts();
                let body_bytes =
                    body.collect().await.map(|b| b.to_bytes()).unwrap_or_default();

                // 注入缓存标记（含 Gemini 特化处理）
                let mut json_body: Option<serde_json::Value> = None;
                let injection = if let Ok(mut val) =
                    serde_json::from_slice::<serde_json::Value>(&body_bytes)
                {
                    if provider == Provider::Gemini {
                        let api_key = GeminiCacheStore::extract_api_key(
                            &parts.uri.to_string()
                        ).unwrap_or_default();
                        let model = val.get("model")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let result = crate::provider::gemini_cache::handle_gemini_request(
                            &mut val, &api_key, &model, &self.gemini_cache
                        );
                        json_body = Some(val);
                        result
                    } else {
                        let result = provider::inject_cache_headers(&provider, &mut val);
                        json_body = Some(val);
                        result
                    }
                } else {
                    provider::CacheInjection {
                        injected: false,
                        details: vec![],
                    }
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

                // 转发到真实服务器
                let start = std::time::Instant::now();
                let url = format!(
                    "https://{}{}",
                    target_host,
                    parts
                        .uri
                        .path_and_query()
                        .map(|pq| pq.as_str())
                        .unwrap_or("")
                );

                let mut req_builder = http_client.request(parts.method.clone(), &url);
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

                        let cache_result = if let Ok(val) =
                            serde_json::from_slice::<serde_json::Value>(&resp_body)
                        {
                            provider::parse_cache_result(&provider, &val)
                        } else {
                            provider::CacheResult::default()
                        };

                        let saving = calculate_saving(
                            provider.name(),
                            &model,
                            cache_result.input_tokens,
                            cache_result.output_tokens,
                            cache_result.cached_tokens,
                            cache_result.cache_write_tokens,
                            &config,
                        );

                        let rec = RequestRecord {
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
                        };
                        tokio::spawn(async move {
                            let _ = crate::db::insert_request_blocking(&db, &rec).await;
                        });

                        if event_tx.send(ProxyEvent {
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
                        }).is_err() {
                            debug!("No active dashboard subscribers, dropping event");
                        }

                        let mut response: Response<Full<Bytes>> =
                            Response::new(Full::from(resp_body.to_vec()));
                        *response.status_mut() = status;
                        for (name, value) in resp_headers.iter() {
                            let n = name.as_str().to_lowercase();
                            if n != "transfer-encoding" {
                                response.headers_mut().insert(name, value.clone());
                            }
                        }
                        Ok::<_, hyper::Error>(response)
                    }
                    Err(e) => {
                        warn!("MITM forward failed for {}: {}", target_host, e);
                        let body = format!("TokenJ proxy error: {}", e);
                        let mut resp = Response::new(Full::from(body));
                        *resp.status_mut() = StatusCode::BAD_GATEWAY;
                        Ok(resp)
                    }
                }
            }
        });

        if let Err(e) = http1::Builder::new()
            .preserve_header_case(true)
            .title_case_headers(true)
            .serve_connection(io, service)
            .await
        {
            debug!("MITM connection closed for {}: {}", host, e);
        }

        Ok(())
    }

    /// 处理直连模式下的 HTTP 请求（非 CONNECT）
    async fn handle_direct_request(
        self,
        req: Request<Incoming>,
    ) -> Result<Response<Full<Bytes>>, hyper::Error> {
        // 直连模式下客户端把代理当目标服务器发请求，URI 地址是代理本身。
        // 目标服务器地址必须从 Host header 获取（方式 A 的标准做法）。
        // 若 URI 中带有绝对路径的 host（如 GET http://api.deepseek.com/...），
        // 也优先使用它（兼容某些 SDK 的透传模式）。
        let uri_host = req.uri().host().map(|s| s.to_string());
        let header_host = req
            .headers()
            .get("host")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(':').next().unwrap_or(s).to_string());

        // 如果 URI host 不是本地回环地址，说明 SDK 直接设置了目标 host（兼容模式）
        // 否则用 Host header（标准直连模式）
        let host = match &uri_host {
            Some(h) if h != "127.0.0.1" && h != "localhost" && h != "0.0.0.0" => h.clone(),
            _ => header_host.unwrap_or_default(),
        };

        let provider = Provider::from_host(&host);

        // Read body
        let (parts, body) = req.into_parts();
        let body_bytes = body.collect().await.map(|b| b.to_bytes()).unwrap_or_default();

        // 快速判断：只有需要注入的 Provider 才解析 JSON，其他直接透传
        let needs_json_processing = matches!(provider,
            Provider::Anthropic | Provider::OpenAI | Provider::Gemini
        );

        let mut json_body: Option<serde_json::Value> = None;
        let injection = if needs_json_processing {
            if let Ok(mut val) =
                serde_json::from_slice::<serde_json::Value>(&body_bytes)
            {
                if provider == Provider::Gemini {
                    let api_key = GeminiCacheStore::extract_api_key(
                        &parts.uri.to_string()
                    ).unwrap_or_default();
                    let model = val.get("model")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let result = crate::provider::gemini_cache::handle_gemini_request(
                        &mut val, &api_key, &model, &self.gemini_cache
                    );
                    json_body = Some(val);
                    result
                } else {
                    let result = provider::inject_cache_headers(&provider, &mut val);
                    json_body = Some(val);
                    result
                }
            } else {
                provider::CacheInjection {
                    injected: false,
                    details: vec![],
                }
            }
        } else {
            provider::CacheInjection {
                injected: false,
                details: vec![],
            }
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
        let url = format!(
            "https://{}{}",
            host,
            parts
                .uri
                .path_and_query()
                .map(|pq| pq.as_str())
                .unwrap_or("")
        );

        let client = self.http_client.clone();
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

                let cache_result =
                    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&resp_body) {
                        provider::parse_cache_result(&provider, &val)
                    } else {
                        provider::CacheResult::default()
                    };

                let saving = calculate_saving(
                    provider.name(),
                    &model,
                    cache_result.input_tokens,
                    cache_result.output_tokens,
                    cache_result.cached_tokens,
                    cache_result.cache_write_tokens,
                    &self.config,
                );

                let rec = RequestRecord {
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
                };
                let db = self.db.clone();
                tokio::spawn(async move {
                    let _ = crate::db::insert_request_blocking(&db, &rec).await;
                });

                if self.event_tx.send(ProxyEvent {
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
                }).is_err() {
                    debug!("No active dashboard subscribers, dropping event");
                }

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
                let body = format!("TokenJ proxy error: {}", e);
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
            cert_manager: self.cert_manager.clone(),
            event_tx: self.event_tx.clone(),
            gemini_cache: self.gemini_cache.clone(), // 共享 Gemini HTTP Client
            http_client: self.http_client.clone(), // reqwest::Client 用 Arc 内部共享
        }
    }
}
