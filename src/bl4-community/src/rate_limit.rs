use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

use crate::state::AppState;

struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

impl Bucket {
    fn new(max: f64) -> Self {
        Self {
            tokens: max,
            last_refill: Instant::now(),
        }
    }

    fn try_consume(&mut self, max: f64, refill_per_sec: f64) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * refill_per_sec).min(max);
        self.last_refill = now;

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

pub struct RateLimiter {
    buckets: Mutex<HashMap<String, Bucket>>,
    max_tokens: f64,
    refill_per_sec: f64,
}

impl RateLimiter {
    pub fn new(requests_per_minute: u32) -> Self {
        let per_sec = requests_per_minute as f64 / 60.0;
        Self {
            buckets: Mutex::new(HashMap::new()),
            max_tokens: requests_per_minute as f64,
            refill_per_sec: per_sec,
        }
    }

    async fn check(&self, ip: &str) -> bool {
        let mut buckets = self.buckets.lock().await;
        let bucket = buckets
            .entry(ip.to_string())
            .or_insert_with(|| Bucket::new(self.max_tokens));
        bucket.try_consume(self.max_tokens, self.refill_per_sec)
    }
}

fn client_ip(req: &Request) -> String {
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0.ip().to_string())
        .or_else(|| {
            req.headers()
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.split(',').next().unwrap_or("").trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

pub async fn rate_limit_writes(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let ip = client_ip(&req);

    if state.allowed_ips.contains(&ip) {
        return Ok(next.run(req).await);
    }

    if state.write_limiter.check(&ip).await {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::TOO_MANY_REQUESTS)
    }
}

pub async fn require_allowed_ip(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let ip = client_ip(&req);

    if state.allowed_ips.contains(&ip) {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}
