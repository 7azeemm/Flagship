use axum::body::Body;
use axum::http::HeaderMap;
use axum::{
    extract::ConnectInfo,
    http::{Request, StatusCode},
    middleware::Next,
    response::IntoResponse,
    response::Response,
};
use std::net::{IpAddr, SocketAddr};
use std::sync::OnceLock;
use std::time::Instant;

static TRUSTED_PROXIES: OnceLock<Vec<IpAddr>> = OnceLock::new();

pub fn configure_proxies() -> Result<(), String> {
    let value = std::env::var("TRUSTED_PROXY_IPS").unwrap_or_default();
    let addresses = value
        .split(',')
        .filter(|v| !v.trim().is_empty())
        .map(|v| {
            v.trim()
                .parse::<IpAddr>()
                .map_err(|_| "TRUSTED_PROXY_IPS must contain comma-separated IP addresses".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let _ = TRUSTED_PROXIES.set(addresses);
    Ok(())
}

// Trust only a single header overwritten by an explicitly configured proxy.
// Never accept a client-supplied forwarding chain or guess which hop to use.
fn resolve_client_ip(peer: IpAddr, headers: &HeaderMap, trusted_proxies: &[IpAddr]) -> Result<IpAddr, &'static str> {
    if !trusted_proxies.contains(&peer) {
        return Ok(peer);
    }
    let mut values = headers.get_all("x-real-ip").iter();
    let value = values.next().ok_or("Trusted proxy must set X-Real-IP")?;
    if values.next().is_some() {
        return Err("Expected one X-Real-IP address");
    }
    value.to_str().ok().and_then(|v| v.parse().ok()).ok_or("Invalid X-Real-IP address")
}

pub async fn logging_middleware(ConnectInfo(addr): ConnectInfo<SocketAddr>, mut req: Request<Body>, next: Next) -> Response {
    let ip = match resolve_client_ip(addr.ip(), req.headers(), TRUSTED_PROXIES.get().map(Vec::as_slice).unwrap_or_default()) {
        Ok(ip) => ip,
        Err(message) => return (StatusCode::BAD_REQUEST, message).into_response(),
    };
    let addr = SocketAddr::new(ip, addr.port());
    req.extensions_mut().insert(ConnectInfo(addr));
    let method = req.method().clone();
    let uri = req.uri().path().to_string();

    let start = Instant::now();
    let response = next.run(req).await;
    let elapsed = start.elapsed();

    let status = response.status().as_u16();

    if response.status().is_client_error() || response.status().is_server_error() {
        tracing::warn!(
            target: "api",
            method = %method,
            uri = %uri,
            remote_addr = %addr,
            status,
            duration_ms = elapsed.as_millis(),
            "Request failed"
        );
    }

    response
}
