//! Bounded, memory-only client budgets plus an aggregate service budget. Never log IPs.
use axum::{
    extract::{ConnectInfo, Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Json, Router,
};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// HTTP adapter defaults. Application-specific budgets belong to their caller.
pub struct ThrottleDefaults {
    pub rate_per_second: u32,
    pub burst: u32,
    pub client_rate: u32,
    pub client_burst: u32,
    pub client_period_seconds: u32,
}
pub const CLIENT_IP_HEADER: &str = "x-zadp-client-ip";
#[derive(Clone)]
pub struct DiscoveryThrottle(Arc<Mutex<Budget>>, bool);
struct Budget {
    global: Bucket,
    client_rate: u32,
    client_capacity: u32,
    client_period: Duration,
    clients: HashMap<Option<IpAddr>, Client>,
    max_clients: usize,
    idle: Duration,
    swept: Instant,
}
struct Client {
    bucket: Bucket,
    seen: Instant,
}
struct Bucket {
    rate: u32,
    capacity: u32,
    tokens: u32,
    period: Duration,
    updated: Instant,
}
impl Bucket {
    fn new(rate: u32, capacity: u32, period: Duration, now: Instant) -> Self {
        Self {
            rate,
            capacity,
            tokens: capacity,
            period,
            updated: now,
        }
    }
    fn refill(&mut self, now: Instant) {
        let periods = now.saturating_duration_since(self.updated).as_secs() / self.period.as_secs();
        if periods > 0 {
            self.tokens = self.capacity.min(
                self.tokens.saturating_add(
                    periods
                        .saturating_mul(self.rate as u64)
                        .min(u32::MAX as u64) as u32,
                ),
            );
            self.updated += Duration::from_secs(periods * self.period.as_secs());
        }
    }
    fn wait(&self, now: Instant) -> u64 {
        self.period
            .saturating_sub(now.saturating_duration_since(self.updated))
            .as_secs()
            .saturating_add(1)
            .min(self.period.as_secs())
            .max(1)
    }
}
impl DiscoveryThrottle {
    /// Compatibility constructor: equal aggregate/client budgets for explicit callers.
    pub fn new(rate: u32, burst: u32) -> Result<Self, &'static str> {
        Self::configured(rate, burst, rate, burst, 1, false, 10000, 600)
    }
    #[allow(clippy::too_many_arguments)]
    fn configured(
        rate: u32,
        burst: u32,
        client_rate: u32,
        client_burst: u32,
        period: u64,
        trusted: bool,
        max_clients: usize,
        idle: u64,
    ) -> Result<Self, &'static str> {
        if rate == 0
            || rate > 1000
            || burst == 0
            || burst > 10000
            || client_rate == 0
            || client_rate > 1000
            || client_burst == 0
            || client_burst > 10000
            || !(1..=3600).contains(&period)
            || !(1..=100000).contains(&max_clients)
            || !(60..=3600).contains(&idle)
        {
            return Err("invalid throttle limits");
        }
        let now = Instant::now();
        Ok(Self(
            Arc::new(Mutex::new(Budget {
                global: Bucket::new(rate, burst, Duration::from_secs(1), now),
                client_rate,
                client_capacity: client_burst,
                client_period: Duration::from_secs(period),
                clients: HashMap::new(),
                max_clients,
                idle: Duration::from_secs(idle),
                swept: now,
            })),
            trusted,
        ))
    }
    pub fn from_env() -> Result<Self, &'static str> {
        Self::from_env_with_defaults(
            "ZADP_API",
            ThrottleDefaults {
                rate_per_second: 100,
                burst: 200,
                client_rate: 5,
                client_burst: 30,
                client_period_seconds: 1,
            },
        )
    }
    pub fn from_env_with_defaults(
        prefix: &str,
        defaults: ThrottleDefaults,
    ) -> Result<Self, &'static str> {
        fn value(name: &str, default: u32) -> Result<u32, &'static str> {
            match std::env::var(name) {
                Ok(v) => v.parse().map_err(|_| "invalid throttle configuration"),
                Err(std::env::VarError::NotPresent) => Ok(default),
                Err(_) => Err("invalid throttle configuration"),
            }
        }
        let trusted = match std::env::var("ZADP_TRUST_LOOPBACK_PROXY") {
            Ok(v) if v == "true" => true,
            Ok(v) if v == "false" => false,
            Err(std::env::VarError::NotPresent) => false,
            _ => return Err("invalid trusted proxy configuration"),
        };
        Self::configured(
            value(
                &format!("{prefix}_RATE_PER_SECOND"),
                defaults.rate_per_second,
            )?,
            value(&format!("{prefix}_RATE_BURST"), defaults.burst)?,
            value(&format!("{prefix}_CLIENT_RATE"), defaults.client_rate)?,
            value(&format!("{prefix}_CLIENT_BURST"), defaults.client_burst)?,
            value(
                &format!("{prefix}_CLIENT_PERIOD_SECONDS"),
                defaults.client_period_seconds,
            )? as u64,
            trusted,
            value("ZADP_RATE_MAX_CLIENTS", 10000)? as usize,
            value("ZADP_RATE_IDLE_SECONDS", 600)? as u64,
        )
    }
    fn client(&self, request: &Request) -> Result<Option<IpAddr>, ()> {
        let peer = request
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|p| p.0.ip());
        if !self.1 {
            return Ok(peer.map(normalize));
        }
        if !peer.is_some_and(|p| p.is_loopback()) {
            return Err(());
        }
        let mut values = request.headers().get_all(CLIENT_IP_HEADER).iter();
        let raw = values.next().ok_or(())?.to_str().map_err(|_| ())?;
        if values.next().is_some() || raw.len() > 45 {
            return Err(());
        }
        Ok(Some(normalize(raw.parse().map_err(|_| ())?)))
    }
    fn acquire(&self, client: Option<IpAddr>, now: Instant) -> Result<(), u64> {
        let Ok(mut b) = self.0.lock() else {
            return Err(1);
        };
        if now.saturating_duration_since(b.swept) >= Duration::from_secs(60) {
            let idle = b.idle;
            b.clients
                .retain(|_, c| now.saturating_duration_since(c.seen) < idle);
            b.swept = now;
        }
        if !b.clients.contains_key(&client) {
            if b.clients.len() >= b.max_clients {
                return Err(60);
            }
            let bucket = Bucket::new(b.client_rate, b.client_capacity, b.client_period, now);
            b.clients.insert(client, Client { bucket, seen: now });
        }
        let c = b.clients.get_mut(&client).expect("inserted client");
        c.seen = now;
        c.bucket.refill(now);
        if c.bucket.tokens == 0 {
            return Err(c.bucket.wait(now));
        }
        b.global.refill(now);
        if b.global.tokens == 0 {
            return Err(b.global.wait(now));
        }
        // Rejected clients do not drain the shared budget, and aggregate rejections do not drain client tokens.
        b.global.tokens -= 1;
        b.clients
            .get_mut(&client)
            .expect("inserted client")
            .bucket
            .tokens -= 1;
        Ok(())
    }
    pub fn apply(self, router: Router) -> Router {
        router.layer(middleware::from_fn_with_state(self, limit))
    }
}
fn normalize(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v) => {
            if let Some(v4) = v.to_ipv4_mapped() {
                return IpAddr::V4(v4);
            }
            let mut octets = v.octets();
            octets[8..].fill(0);
            IpAddr::V6(octets.into())
        }
        v => v,
    }
}
pub async fn limit(
    State(budget): State<DiscoveryThrottle>,
    request: Request,
    next: Next,
) -> Response {
    if request.uri().path() == "/healthz" {
        return next.run(request).await;
    }
    let client = match budget.client(&request) {
        Ok(c) => c,
        Err(()) => {
            return (
                StatusCode::FORBIDDEN,
                [(header::CACHE_CONTROL, "no-store")],
                Json(serde_json::json!({"error":"invalid_proxy_identity"})),
            )
                .into_response()
        }
    };
    match budget.acquire(client, Instant::now()) {
        Ok(()) => next.run(request).await,
        Err(wait) => (
            StatusCode::TOO_MANY_REQUESTS,
            [
                (header::RETRY_AFTER, wait.to_string()),
                (header::CACHE_CONTROL, "no-store".to_string()),
            ],
            Json(serde_json::json!({"error":"rate_limited"})),
        )
            .into_response(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, routing::get};
    use tower::ServiceExt;
    fn ip(s: &str) -> Option<IpAddr> {
        Some(normalize(s.parse().unwrap()))
    }
    #[test]
    fn isolation_shared_network_and_aggregate() {
        let b = DiscoveryThrottle::configured(3, 3, 1, 2, 1, false, 10, 600).unwrap();
        let t = Instant::now();
        assert!(b.acquire(ip("192.0.2.1"), t).is_ok());
        assert!(b.acquire(ip("192.0.2.1"), t).is_ok());
        assert!(b.acquire(ip("192.0.2.1"), t).is_err());
        assert!(b.acquire(ip("192.0.2.2"), t).is_ok());
        assert!(b.acquire(ip("192.0.2.3"), t).is_err());
        assert!(b
            .acquire(ip("192.0.2.3"), t + Duration::from_secs(1))
            .is_ok());
    }
    #[test]
    fn expiry_capacity_ipv6_and_reports() {
        let b = DiscoveryThrottle::configured(100, 200, 1, 3, 60, false, 1, 600).unwrap();
        let t = Instant::now();
        for _ in 0..3 {
            assert!(b.acquire(ip("2001:db8::1"), t).is_ok());
        }
        assert_eq!(b.acquire(ip("2001:db8::2"), t), Err(60));
        assert!(b.acquire(ip("192.0.2.1"), t).is_err());
        assert!(b
            .acquire(ip("2001:db8::3"), t + Duration::from_secs(60))
            .is_ok());
        assert!(b
            .acquire(ip("192.0.2.1"), t + Duration::from_secs(660))
            .is_ok());
        assert_eq!(ip("::ffff:192.0.2.1"), ip("192.0.2.1"));
        assert_eq!(b.0.lock().unwrap().clients.len(), 1);
    }
    #[tokio::test]
    async fn proxy_spoofing_and_health() {
        let b = DiscoveryThrottle::configured(100, 200, 1, 1, 1, true, 10, 600).unwrap();
        let router = b.apply(
            Router::new()
                .route("/offers", get(|| async { "ok" }))
                .route("/healthz", get(|| async { "ok" })),
        );
        let req = |peer: &str, header: &str| {
            let mut r = Request::builder()
                .uri("/offers")
                .header(CLIENT_IP_HEADER, header)
                .header("x-forwarded-for", "198.51.100.99")
                .body(Body::empty())
                .unwrap();
            r.extensions_mut()
                .insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
            r
        };
        assert_eq!(
            router
                .clone()
                .oneshot(req("198.51.100.1:1", "192.0.2.1"))
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            router
                .clone()
                .oneshot(req("127.0.0.1:1", "192.0.2.1"))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        let r = router
            .clone()
            .oneshot(req("127.0.0.1:1", "192.0.2.1"))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(r.headers()[header::RETRY_AFTER], "1");
        assert_eq!(
            router
                .clone()
                .oneshot(req("127.0.0.1:1", "192.0.2.2"))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        for value in ["bad", "192.0.2.1,192.0.2.2"] {
            assert_eq!(
                router
                    .clone()
                    .oneshot(req("127.0.0.1:1", value))
                    .await
                    .unwrap()
                    .status(),
                StatusCode::FORBIDDEN
            );
        }
        let r = Request::builder()
            .uri("/healthz")
            .body(Body::empty())
            .unwrap();
        assert_eq!(router.oneshot(r).await.unwrap().status(), StatusCode::OK);
    }
    #[test]
    fn invalid_limits_and_missing_duplicate_identity() {
        assert!(DiscoveryThrottle::new(0, 1).is_err());
        assert!(DiscoveryThrottle::configured(1, 1, 1, 1, 0, false, 1, 600).is_err());
        assert!(DiscoveryThrottle::configured(1, 1, 1, 1, 1, false, 0, 600).is_err());
        let budget = DiscoveryThrottle::configured(1, 1, 1, 1, 1, true, 10, 600).unwrap();
        let mut r = Request::builder().body(Body::empty()).unwrap();
        assert!(budget.client(&r).is_err());
        r.extensions_mut()
            .insert(ConnectInfo("127.0.0.1:10".parse::<SocketAddr>().unwrap()));
        assert!(budget.client(&r).is_err());
        r.headers_mut()
            .append(CLIENT_IP_HEADER, "192.0.2.1".parse().unwrap());
        r.headers_mut()
            .append(CLIENT_IP_HEADER, "192.0.2.2".parse().unwrap());
        assert!(budget.client(&r).is_err());
    }
    #[test]
    fn direct_mode_ignores_every_forwarded_header() {
        let b = DiscoveryThrottle::new(1, 1).unwrap();
        let mut r = Request::builder()
            .header(CLIENT_IP_HEADER, "192.0.2.99")
            .header("x-forwarded-for", "192.0.2.98")
            .body(Body::empty())
            .unwrap();
        r.extensions_mut()
            .insert(ConnectInfo("192.0.2.1:1".parse::<SocketAddr>().unwrap()));
        assert_eq!(b.client(&r), Ok(ip("192.0.2.1")));
    }
}
