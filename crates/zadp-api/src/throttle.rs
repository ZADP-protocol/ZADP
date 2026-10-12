//! Process-wide discovery request budget. No client identifiers, forwarded-header trust or IP logs.
use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Json, Router,
};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct DiscoveryThrottle(Arc<Mutex<Bucket>>);
struct Bucket {
    rate: u32,
    capacity: u32,
    tokens: u32,
    updated: Instant,
}
impl DiscoveryThrottle {
    pub fn new(rate: u32, burst: u32) -> Result<Self, &'static str> {
        if !(1..=1000).contains(&rate) || !(1..=10000).contains(&burst) {
            return Err("invalid discovery throttle limits");
        }
        Ok(Self(Arc::new(Mutex::new(Bucket {
            rate,
            capacity: burst,
            tokens: burst,
            updated: Instant::now(),
        }))))
    }
    pub fn from_env() -> Result<Self, &'static str> {
        fn value(name: &str, default: u32) -> Result<u32, &'static str> {
            match std::env::var(name) {
                Ok(v) => v.parse().map_err(|_| "invalid throttle configuration"),
                Err(std::env::VarError::NotPresent) => Ok(default),
                Err(_) => Err("invalid throttle configuration"),
            }
        }
        Self::new(
            value("ZADP_API_RATE_PER_SECOND", 10)?,
            value("ZADP_API_RATE_BURST", 30)?,
        )
    }
    fn acquire(&self, now: Instant) -> bool {
        let Ok(mut b) = self.0.lock() else {
            return false;
        };
        let seconds = now.saturating_duration_since(b.updated).as_secs();
        if seconds > 0 {
            b.tokens =
                b.capacity.min(b.tokens.saturating_add(
                    seconds.saturating_mul(b.rate as u64).min(u32::MAX as u64) as u32,
                ));
            b.updated += Duration::from_secs(seconds);
        }
        if b.tokens == 0 {
            return false;
        }
        b.tokens -= 1;
        true
    }
    pub fn apply(self, router: Router) -> Router {
        router.layer(middleware::from_fn_with_state(self, limit))
    }
}
async fn limit(State(budget): State<DiscoveryThrottle>, request: Request, next: Next) -> Response {
    if request.uri().path() == "/healthz" || budget.acquire(Instant::now()) {
        return next.run(request).await;
    }
    (
        StatusCode::TOO_MANY_REQUESTS,
        [
            (header::RETRY_AFTER, "1"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        Json(serde_json::json!({"error":"rate_limited"})),
    )
        .into_response()
}
#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, routing::get};
    use tower::ServiceExt;
    #[test]
    fn burst_refill_and_shared_budget() {
        let b = DiscoveryThrottle::new(2, 3).unwrap();
        let start = Instant::now();
        assert!(b.acquire(start));
        assert!(b.clone().acquire(start));
        assert!(b.acquire(start));
        assert!(!b.acquire(start));
        assert!(b.acquire(start + Duration::from_secs(1)));
        assert!(b.acquire(start + Duration::from_secs(1)));
        assert!(!b.acquire(start + Duration::from_secs(1)));
        assert!(DiscoveryThrottle::new(0, 1).is_err());
        assert!(DiscoveryThrottle::new(1, 0).is_err());
    }
    #[tokio::test]
    async fn http_429_headers_health_and_spoofed_forwarding() {
        let router = DiscoveryThrottle::new(1, 1).unwrap().apply(
            Router::new()
                .route("/offers", get(|| async { "offers" }))
                .route("/healthz", get(|| async { "ok" })),
        );
        let request = || {
            Request::builder()
                .uri("/offers")
                .header("X-Forwarded-For", "203.0.113.1")
                .body(Body::empty())
                .unwrap()
        };
        assert_eq!(
            router.clone().oneshot(request()).await.unwrap().status(),
            StatusCode::OK
        );
        let response = router.clone().oneshot(request()).await.unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()[header::RETRY_AFTER], "1");
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
