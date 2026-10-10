//! Bounded, off-chain proof fetching. No arbitrary URL API, proxy, redirects or private DNS targets.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::Semaphore;
use zadp_protocol::{commerce_origin, public_endpoint_ip, verify_offer, EndpointProof, Offer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointStatus {
    NotApplicable,
    Verified,
    Unverified,
    Unavailable,
}
#[derive(Debug, Clone, Serialize)]
pub struct EndpointVerification {
    pub status: EndpointStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    /// Short host attestation lease; saved browser metadata cannot authorize new orders indefinitely.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<DateTime<Utc>>,
}
#[async_trait]
pub trait ProofFetcher: Send + Sync {
    /// Only called with a validated normalized origin from a signed visible Offer.
    async fn fetch(&self, origin: &str) -> Result<EndpointProof, EndpointStatus>;
}
struct HttpsFetcher;
#[async_trait]
impl ProofFetcher for HttpsFetcher {
    async fn fetch(&self, origin: &str) -> Result<EndpointProof, EndpointStatus> {
        let url = url::Url::parse(origin).map_err(|_| EndpointStatus::Unverified)?;
        let host = url.host_str().ok_or(EndpointStatus::Unverified)?;
        let port = url
            .port_or_known_default()
            .ok_or(EndpointStatus::Unverified)?;
        let addresses: Vec<_> = match url.host().ok_or(EndpointStatus::Unverified)? {
            url::Host::Ipv4(ip) => vec![std::net::SocketAddr::new(ip.into(), port)],
            url::Host::Ipv6(ip) => vec![std::net::SocketAddr::new(ip.into(), port)],
            url::Host::Domain(_) => tokio::net::lookup_host((host, port))
                .await
                .map_err(|_| EndpointStatus::Unavailable)?
                .take(33)
                .collect(),
        };
        if !addresses_are_public(&addresses) {
            return Err(EndpointStatus::Unverified);
        }
        // Pin the validated DNS snapshot. Never resolve again at connect time; no ambient proxy.
        let client = reqwest::Client::builder()
            .no_proxy()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(4))
            .connect_timeout(Duration::from_secs(2))
            .resolve_to_addrs(host, &addresses)
            .build()
            .map_err(|_| EndpointStatus::Unavailable)?;
        let mut response = client
            .get(format!("{origin}/.well-known/zadp"))
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|_| EndpointStatus::Unavailable)?;
        if response.status() != reqwest::StatusCode::OK {
            return Err(EndpointStatus::Unavailable);
        }
        if response.content_length().is_some_and(|n| n > 16384) {
            return Err(EndpointStatus::Unverified);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| EndpointStatus::Unavailable)?
        {
            if bytes.len() + chunk.len() > 16384 {
                return Err(EndpointStatus::Unverified);
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| EndpointStatus::Unverified)
    }
}
struct Entry {
    checked: Instant,
    result: Option<Result<EndpointProof, EndpointStatus>>,
}
/// In-memory, maximum 512 origins and four in-flight requests. Revalidate at most once per minute.
/// First lookup is unverified while the bounded background check runs; no discovery request waits on sellers.
#[derive(Clone)]
pub struct EndpointVerifier {
    cache: Arc<Mutex<HashMap<String, Entry>>>,
    slots: Arc<Semaphore>,
    fetcher: Arc<dyn ProofFetcher>,
}
impl Default for EndpointVerifier {
    fn default() -> Self {
        Self::new(Arc::new(HttpsFetcher))
    }
}
impl EndpointVerifier {
    pub fn new(fetcher: Arc<dyn ProofFetcher>) -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            slots: Arc::new(Semaphore::new(4)),
            fetcher,
        }
    }
    pub fn status(&self, offer: &Offer) -> EndpointVerification {
        let mut view = EndpointVerification {
            status: EndpointStatus::Unverified,
            origin: None,
            expires_at: None,
            valid_until: None,
        };
        if offer.commerce.is_none() {
            view.status = EndpointStatus::NotApplicable;
            return view;
        }
        if verify_offer(offer, Utc::now()).is_err() {
            return view;
        }
        let Ok(origin) = commerce_origin(&offer.commerce.as_ref().unwrap().endpoint) else {
            return view;
        };
        view.origin = Some(origin.clone());
        let mut cache = self.cache.lock().unwrap();
        if let Some(entry) = cache.get(&origin) {
            if entry.result.is_none() || entry.checked.elapsed() < Duration::from_secs(60) {
                match &entry.result {
                    Some(Ok(proof)) => {
                        if proof.verify(&offer.seller, &origin, Utc::now()).is_ok() {
                            view.status = EndpointStatus::Verified;
                            view.expires_at = Some(proof.expires_at);
                            view.valid_until = Some(
                                proof.expires_at.min(
                                    Utc::now()
                                        + chrono::Duration::from_std(
                                            Duration::from_secs(60)
                                                .saturating_sub(entry.checked.elapsed()),
                                        )
                                        .unwrap(),
                                ),
                            );
                        }
                    }
                    Some(Err(status)) => view.status = *status,
                    None => {}
                }
                return view;
            }
        }
        cache.retain(|_, e| e.result.is_none() || e.checked.elapsed() < Duration::from_secs(300));
        if cache.len() >= 512 && !cache.contains_key(&origin) {
            return view;
        }
        cache.insert(
            origin.clone(),
            Entry {
                checked: Instant::now(),
                result: None,
            },
        );
        let this = self.clone();
        tokio::spawn(async move {
            // At most 512 pending/cached origins; queue fairly rather than starving later Offers.
            let Ok(permit) = this.slots.clone().acquire_owned().await else {
                return;
            };
            // Includes DNS lookup and body reads in the deadline.
            let result = tokio::time::timeout(Duration::from_secs(5), this.fetcher.fetch(&origin))
                .await
                .unwrap_or(Err(EndpointStatus::Unavailable));
            this.cache.lock().unwrap().insert(
                origin,
                Entry {
                    checked: Instant::now(),
                    result: Some(result),
                },
            );
            drop(permit);
        });
        view
    }
}

fn addresses_are_public(addresses: &[std::net::SocketAddr]) -> bool {
    !addresses.is_empty()
        && addresses.len() <= 32
        && addresses.iter().all(|a| public_endpoint_ip(a.ip()))
}
#[cfg(test)]
mod transport_tests {
    use super::*;
    #[test]
    fn every_dns_answer_must_be_public_and_the_snapshot_is_bounded() {
        let public = "8.8.8.8:443".parse().unwrap();
        assert!(addresses_are_public(&[public]));
        assert!(!addresses_are_public(&[]));
        for blocked in [
            "127.0.0.1:443",
            "10.0.0.1:443",
            "169.254.169.254:443",
            "[::1]:443",
            "[fc00::1]:443",
        ] {
            assert!(!addresses_are_public(&[public, blocked.parse().unwrap()]));
        }
        assert!(!addresses_are_public(&[public; 33]));
    }
    #[tokio::test]
    async fn private_dns_target_is_rejected_before_http() {
        assert_eq!(
            HttpsFetcher.fetch("https://127.0.0.1").await.unwrap_err(),
            EndpointStatus::Unverified
        );
    }
}
