use axum::{
    body::{Body, Bytes},
    extract::Request,
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::Response,
};
use futures::{StreamExt, stream};
use std::sync::LazyLock;
use std::{collections::HashMap, sync::Arc, time::Instant};
use tokio::sync::Mutex;
use crate::core::catalog::revision::Revision;

const MAX_ENTRIES: usize = 64;
const MAX_BODY: usize = 512 * 1024;
static CACHE: LazyLock<Mutex<HashMap<String, (Instant, Arc<Mutex<Option<Entry>>>)>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

struct Entry {
    revision: u64,
    headers: HeaderMap,
    bytes: Bytes,
}

pub async fn cached_catalog(request: Request, next: Next) -> Response {
    cached_with_revision(request, next, Revision::current).await
}

async fn cached_with_revision(request: Request, next: Next, current_revision: fn() -> u64) -> Response {
    if request.method() != Method::GET
        || request
            .uri()
            .path_and_query()
            .is_some_and(|uri| uri.as_str().len() > 4096)
    {
        return next.run(request).await;
    }
    
    let path = request
        .uri()
        .path()
        .strip_prefix("/api/")
        .map_or_else(|| request.uri().path().to_string(), |path| format!("/{path}"));
    
    // Root and /api aliases return the same public data. Homepage/info ignore queries.
    let key = match (path.as_str(), request.uri().query()) {
        ("/homepage" | "/info", _) | (_, None) => path.clone(),
        (_, Some(query)) => format!("{path}?{query}"),
    };
    
    let slot = {
        let mut cache = CACHE.lock().await;
        if !cache.contains_key(&key) && cache.len() >= MAX_ENTRIES {
            // Do not evict in-flight slots: their waiters must keep sharing one calculation.
            if let Some(oldest) = cache
                .iter()
                .filter(|(_, (_, slot))| Arc::strong_count(slot) == 1)
                .min_by_key(|(_, (at, _))| *at)
                .map(|(key, _)| key.clone())
            {
                cache.remove(&oldest);
            } else {
                drop(cache);
                return next.run(request).await;
            }
        }
        let item = cache
            .entry(key)
            .or_insert_with(|| (Instant::now(), Arc::new(Mutex::new(None))));
        item.0 = Instant::now();
        item.1.clone()
    };
    
    // Requests for the same URL share a single calculation; different URLs run concurrently.
    let mut entry = slot.lock().await;
    let revision = current_revision();
    if let Some(hit) = entry.as_ref().filter(|hit| hit.revision == revision) {
        let mut response = Response::new(Body::from(hit.bytes.clone()));
        *response.headers_mut() = hit.headers.clone();
        response
            .headers_mut()
            .insert("x-flagship-cache", HeaderValue::from_static("HIT"));
        return response;
    }
    
    let response = next.run(request).await;
    if response.status() != StatusCode::OK
        || response.headers().contains_key(header::SET_COOKIE)
        || response.headers().contains_key(header::VARY)
        || response.headers().get_all(header::CACHE_CONTROL).iter().any(|value| {
            value.to_str().is_ok_and(|value| {
                value.split(',').any(|directive| {
                    matches!(
                        directive
                            .trim()
                            .split('=')
                            .next()
                            .unwrap_or("")
                            .to_ascii_lowercase()
                            .as_str(),
                        "private" | "no-store" | "no-cache"
                    )
                })
            })
        })
    {
        return response;
    }
    
    let (mut parts, body) = response.into_parts();
    let mut stream = body.into_data_stream();
    let mut buffered = Vec::new();
    
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(chunk) if buffered.len().saturating_add(chunk.len()) <= MAX_BODY => buffered.extend_from_slice(&chunk),
            chunk => {
                // Preserve large/streaming bodies without buffering them in full.
                let prefix = stream::iter([Ok(Bytes::from(buffered)), chunk]);
                parts
                    .headers
                    .insert("x-flagship-cache", HeaderValue::from_static("BYPASS"));
                return Response::from_parts(parts, Body::from_stream(prefix.chain(stream)));
            }
        }
    }
    
    let bytes = Bytes::from(buffered);
    if revision == current_revision() {
        *entry = Some(Entry {
            revision,
            headers: parts.headers.clone(),
            bytes: bytes.clone(),
        });
    }
    
    parts.headers.insert("x-flagship-cache", HeaderValue::from_static("MISS"));
    Response::from_parts(parts, Body::from(bytes))
}
