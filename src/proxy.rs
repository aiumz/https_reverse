use async_trait::async_trait;
use pingora::prelude::*;
use pingora::proxy::{http_proxy_service, ProxyHttp, Session};
use pingora_core::server::configuration::Opt;
use pingora_core::server::Server;
use pingora_core::upstreams::peer::HttpPeer;
use pingora_http::RequestHeader;
use pingora_http::ResponseHeader;
use std::collections::HashMap;
use std::str;
use std::sync::Arc;

use crate::config::{ProxyConfig, Rule};

pub struct ProxyCtx {
    upstream_url: Option<String>,
    upstream_host: Option<String>,
    upstream_port: Option<u16>,
    upstream_use_tls: Option<bool>,
    is_websocket: bool,
}

impl ProxyCtx {
    fn new() -> Self {
        Self {
            upstream_url: None,
            upstream_host: None,
            upstream_port: None,
            upstream_use_tls: None,
            is_websocket: false,
        }
    }
}

pub struct ProxyService {
    routes: Arc<HashMap<String, String>>,
}

impl ProxyService {
    pub fn new(rules: &[Rule]) -> Self {
        let mut routes_map: HashMap<String, String> = HashMap::new();
        rules.iter().for_each(|rule| {
            let target = rule.proxy_pass.to_url();
            routes_map.insert(rule.location.clone(), target.clone());
        });
        Self {
            routes: Arc::new(routes_map),
        }
    }

    fn find_upstream(&self, path: &str) -> Option<String> {
        let mut upstream = String::new();
        let mut matched_prefix_len = 0;

        for (prefix, target) in self.routes.iter() {
            if path.starts_with(prefix) && prefix.len() > matched_prefix_len {
                upstream = target.clone();
                matched_prefix_len = prefix.len();
            }
        }

        if upstream.is_empty() {
            None
        } else {
            Some(upstream)
        }
    }

    fn parse_upstream_url(url: &str) -> (String, u16, bool) {
        let use_tls = url.starts_with("https://");
        let url = url
            .trim_start_matches("http://")
            .trim_start_matches("https://");
        let (host, port) = if let Some(pos) = url.find(':') {
            let host = url[..pos].to_string();
            let rest = &url[pos + 1..];
            if let Some(port_str) = rest.split('/').next() {
                let port = port_str.parse().unwrap_or(if use_tls { 443 } else { 80 });
                (host, port)
            } else {
                (host, if use_tls { 443 } else { 80 })
            }
        } else {
            (
                url.split('/').next().unwrap_or("127.0.0.1").to_string(),
                if use_tls { 443 } else { 80 },
            )
        };
        (host, port, use_tls)
    }

    fn set_cors_headers(req_header: &RequestHeader, resp: &mut ResponseHeader) {
        if let Some(origin) = req_header.headers.get("origin") {
            if let Ok(origin_str) = str::from_utf8(origin.as_bytes()) {
                resp.insert_header("Access-Control-Allow-Origin", origin_str)
                    .unwrap();
            }
        } else {
            resp.insert_header("Access-Control-Allow-Origin", "*")
                .unwrap();
        }

        resp.insert_header(
            "Access-Control-Allow-Methods",
            "GET, POST, PUT, DELETE, OPTIONS",
        )
        .unwrap();
        resp.insert_header(
            "Access-Control-Allow-Headers",
            "Authorization, Content-Type, X-Requested-With",
        )
        .unwrap();
        resp.insert_header("Access-Control-Allow-Credentials", "true")
            .unwrap();
    }
}

#[async_trait]
impl ProxyHttp for ProxyService {
    type CTX = ProxyCtx;

    fn new_ctx(&self) -> Self::CTX {
        ProxyCtx::new()
    }

    async fn request_filter(&self, session: &mut Session, ctx: &mut Self::CTX) -> Result<bool>
    where
        Self::CTX: Send + Sync,
    {
        let req_header = session.req_header();
        let path = req_header.uri.path();

        if req_header.method == "OPTIONS" {
            let mut resp = ResponseHeader::build(200, None).unwrap();
            ProxyService::set_cors_headers(req_header, &mut resp);
            session.write_response_header(Box::new(resp), false).await?;
            return Ok(true);
        }

        if let Some(upgrade) = req_header.headers.get("upgrade") {
            if let Ok(upgrade_str) = str::from_utf8(upgrade.as_bytes()) {
                if upgrade_str.eq_ignore_ascii_case("websocket") {
                    ctx.is_websocket = true;
                    println!("[PROXY WEBSOCKET] WebSocket upgrade request: {}", path);
                }
            }
        }

        let upstream = match self.find_upstream(path) {
            Some(upstream) => upstream,
            None => {
                let mut resp = ResponseHeader::build(404, None).unwrap();
                ProxyService::set_cors_headers(req_header, &mut resp);
                session.write_response_header(Box::new(resp), false).await?;
                return Ok(true);
            }
        };
        let (host, port, use_tls) = Self::parse_upstream_url(&upstream);
        ctx.upstream_url = Some(upstream.clone());
        ctx.upstream_host = Some(host.clone());
        ctx.upstream_port = Some(port);
        ctx.upstream_use_tls = Some(use_tls);

        println!("[PROXY HTTP] {} -> {}", path, upstream);
        Ok(false)
    }

    async fn upstream_peer(
        &self,
        _session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>>
    where
        Self::CTX: Send + Sync,
    {
        let host = ctx.upstream_host.as_ref().unwrap();
        let port = ctx.upstream_port.unwrap();
        let use_tls = ctx.upstream_use_tls.unwrap();
        let address = format!("{}:{}", host, port);
        let peer = HttpPeer::new(address, use_tls, host.clone());
        Ok(Box::new(peer))
    }

    async fn upstream_request_filter(
        &self,
        session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()>
    where
        Self::CTX: Send + Sync,
    {
        let req_header = session.req_header();

        if let Some(xff) = req_header.headers.get("x-forwarded-for") {
            if let Ok(xff_str) = str::from_utf8(xff.as_bytes()) {
                upstream_request
                    .insert_header("X-Forwarded-For", xff_str)
                    .unwrap();
            }
        }

        let scheme = "https";
        upstream_request
            .insert_header("X-Forwarded-Proto", scheme)
            .unwrap();

        if let Some(host) = req_header.headers.get("host") {
            if let Ok(host_str) = str::from_utf8(host.as_bytes()) {
                upstream_request.insert_header("Host", host_str).unwrap();
            }
        }

        if ctx.is_websocket {}

        Ok(())
    }

    fn upstream_response_filter(
        &self,
        session: &mut Session,
        upstream_response: &mut ResponseHeader,
        _ctx: &mut Self::CTX,
    ) -> Result<()> {
        let req_header = session.req_header();

        // 隐藏上游的 CORS 头
        upstream_response.remove_header("access-control-allow-origin");
        upstream_response.remove_header("access-control-allow-methods");
        upstream_response.remove_header("access-control-allow-headers");

        // 添加我们自己的 CORS 头，不覆盖某些服务还是会报 CORS 错误
        ProxyService::set_cors_headers(&req_header, upstream_response);

        Ok(())
    }

    async fn response_filter(
        &self,
        session: &mut Session,
        upstream_response: &mut ResponseHeader,
        _ctx: &mut Self::CTX,
    ) -> Result<()>
    where
        Self::CTX: Send + Sync,
    {
        let req_header = session.req_header();
        ProxyService::set_cors_headers(req_header, upstream_response);
        Ok(())
    }
}

pub fn start_proxy(proxy_config: ProxyConfig) -> std::io::Result<()> {
    let opt = Opt::default();
    let mut my_server = Server::new(Some(opt)).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to create server: {}", e),
        )
    })?;
    my_server.bootstrap();

    let rules = proxy_config.get_rules();
    let port = proxy_config.get_port();
    let proxy_service = ProxyService::new(rules);
    let mut service = http_proxy_service(&my_server.configuration, proxy_service);

    let cert_path = "./tmp/local.crt";
    let key_path = "./tmp/local.key";
    let mut tls_settings = pingora_core::listeners::tls::TlsSettings::intermediate(
        cert_path, key_path,
    )
    .map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Failed to load TLS settings: {}", e),
        )
    })?;
    tls_settings.enable_h2();
    service.add_tls_with_settings(&format!("0.0.0.0:{}", port), None, tls_settings);

    my_server.add_service(service);

    println!("==============================================");
    println!("HTTPS reverse proxy running on port {} ☺ ☺ ☺", port);
    println!("==============================================");
    my_server.run_forever();
}
