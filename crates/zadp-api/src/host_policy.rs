//! Operator presentation policy, independent of signed board admission and canonical storage.
use std::collections::HashSet;
use zadp_protocol::SellerId;

#[derive(Debug, Clone, Default)]
pub enum HostPolicy {
    #[default]
    Unrestricted,
    Allowlist(HashSet<SellerId>),
    ManualFile(std::path::PathBuf),
    Manual(std::sync::Arc<ManualPolicy>),
}
impl HostPolicy {
    /// Missing mode is unrestricted for local development; hosted units must explicitly set allowlist.
    pub fn from_config(mode: Option<&str>, entries: Option<&str>) -> Result<Self, &'static str> {
        match mode.unwrap_or("unrestricted") {
            "unrestricted" => Ok(Self::Unrestricted),
            "allowlist" => {
                let entries = entries.filter(|s| !s.trim().is_empty()).unwrap_or("[]");
                let sellers: Vec<SellerId> = serde_json::from_str(entries)
                    .map_err(|_| "invalid HOST_SELLER_ALLOWLIST JSON")?;
                for seller in &sellers {
                    seller
                        .verifying_key()
                        .map_err(|_| "invalid host seller identity")?;
                }
                Ok(Self::Allowlist(sellers.into_iter().collect()))
            }
            _ => Err("unknown HOST_POLICY_MODE"),
        }
    }
    pub fn from_env() -> Result<Self, &'static str> {
        let mode = std::env::var("HOST_POLICY_MODE").map_err(|e| match e {
            std::env::VarError::NotPresent => "missing",
            _ => "invalid HOST_POLICY_MODE encoding",
        });
        let mode = match mode {
            Ok(v) => Some(v),
            Err("missing") => None,
            Err(e) => return Err(e),
        };
        let entries = match std::env::var("HOST_SELLER_ALLOWLIST") {
            Ok(v) => Some(v),
            Err(std::env::VarError::NotPresent) => None,
            Err(_) => return Err("invalid HOST_SELLER_ALLOWLIST encoding"),
        };
        if mode.as_deref() == Some("manual") {
            let path = std::path::PathBuf::from(
                std::env::var("HOST_POLICY_FILE").map_err(|_| "missing HOST_POLICY_FILE")?,
            );
            Self::manual_file(path)
        } else {
            Self::from_config(mode.as_deref(), entries.as_deref())
        }
    }
    pub fn manual_file(path: std::path::PathBuf) -> Result<Self, &'static str> {
        read_manual(&path)?;
        Ok(Self::ManualFile(path))
    }
    pub fn snapshot(&self) -> Self {
        match self {
            Self::ManualFile(path) => read_manual(path)
                .map(|p| Self::Manual(std::sync::Arc::new(p)))
                .unwrap_or_else(|_| Self::Allowlist(HashSet::new())),
            _ => self.clone(),
        }
    }
    pub fn permits_record(&self, record: &zadp_indexer::OfferRecord) -> bool {
        match self {
            Self::Manual(p) => p.permits(record),
            Self::ManualFile(_) => self.snapshot().permits_record(record),
            _ => self.permits(&record.offer.seller),
        }
    }
    pub fn permits(&self, seller: &SellerId) -> bool {
        match self {
            Self::Unrestricted => true,
            Self::Allowlist(sellers) => sellers.contains(seller),
            Self::ManualFile(_) | Self::Manual(_) => false,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManualPolicy {
    pub version: u8,
    pub sellers: Vec<SellerRule>,
    pub offers: Vec<OfferRule>,
    pub demo_auto_approved_sellers: HashSet<SellerId>,
    pub categories: HashSet<String>,
    pub capability_categories: std::collections::HashMap<String, String>,
    /// Host floor on received board value, not an assertion of actual tx fee.
    pub minimum_publication_payment_zat: u64,
}
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SellerRule {
    pub seller: SellerId,
    pub status: SellerStatus,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SellerStatus {
    Approved,
    Suspended,
    Blocked,
}
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfferRule {
    pub board_id: String,
    pub seller: SellerId,
    pub offer_id: String,
    pub status: OfferDecision,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OfferDecision {
    Approved,
    Pending,
    Blocked,
}
impl ManualPolicy {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.version != 1
            || self.minimum_publication_payment_zat > 2_100_000_000_000_000
            || self.sellers.len() > 10000
            || self.offers.len() > 100000
        {
            return Err("invalid manual host policy");
        }
        let mut sellers = HashSet::new();
        for r in &self.sellers {
            r.seller
                .verifying_key()
                .map_err(|_| "invalid host seller identity")?;
            if !sellers.insert(&r.seller) {
                return Err("duplicate seller decision");
            }
        }
        for seller in &self.demo_auto_approved_sellers {
            seller
                .verifying_key()
                .map_err(|_| "invalid demo seller identity")?;
        }
        let mut offers = HashSet::new();
        for r in &self.offers {
            r.seller
                .verifying_key()
                .map_err(|_| "invalid offer seller identity")?;
            if r.board_id.is_empty()
                || r.offer_id.is_empty()
                || !offers.insert((&r.board_id, &r.seller, &r.offer_id))
            {
                return Err("invalid or duplicate offer decision");
            }
        }
        if self.categories.is_empty()
            || self.categories.iter().any(|c| {
                c.is_empty()
                    || c.len() > 64
                    || !c.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
            })
            || self
                .capability_categories
                .values()
                .any(|c| !self.categories.contains(c))
        {
            return Err("invalid category policy");
        }
        Ok(())
    }
    fn permits(&self, r: &zadp_indexer::OfferRecord) -> bool {
        let offer = &r.offer;
        if zadp_protocol::verify_offer(offer, chrono::Utc::now()).is_err() {
            return false;
        }
        if r.chain.as_ref().is_none_or(|c| {
            c.listing_value_zat < self.minimum_publication_payment_zat || c.offer_status != "active"
        }) {
            return false;
        }
        let decision = self
            .offers
            .iter()
            .find(|p| {
                p.board_id == offer.board_id.as_deref().unwrap_or("")
                    && p.seller == offer.seller
                    && p.offer_id == offer.id
            })
            .map(|p| p.status);
        if matches!(
            decision,
            Some(OfferDecision::Blocked | OfferDecision::Pending)
        ) {
            return false;
        }
        if self
            .sellers
            .iter()
            .find(|p| p.seller == offer.seller)
            .is_none_or(|p| p.status != SellerStatus::Approved)
        {
            return false;
        }
        if decision != Some(OfferDecision::Approved)
            && !self.demo_auto_approved_sellers.contains(&offer.seller)
        {
            return false;
        }
        if offer.title.chars().count() > 120
            || offer
                .description
                .as_ref()
                .is_some_and(|d| d.chars().count() > 2000)
            || [&offer.title, offer.description.as_deref().unwrap_or("")]
                .iter()
                .any(|s| {
                    s.chars()
                        .any(|c| c == '<' || c == '>' || c.is_control() && c != '\n' && c != '\t')
                })
        {
            return false;
        }
        let category = self
            .capability_categories
            .get(&offer.capability)
            .map(String::as_str)
            .unwrap_or(&offer.capability);
        if !self.categories.contains(category) {
            return false;
        }
        true
    }
}
fn read_manual(path: &std::path::Path) -> Result<ManualPolicy, &'static str> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| "manual policy unavailable")?;
    if !metadata.is_file() || metadata.len() > 16 * 1024 * 1024 {
        return Err("invalid manual policy file");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o022 != 0 {
            return Err("manual policy must not be group/world writable");
        }
    }
    let policy: ManualPolicy =
        serde_json::from_slice(&std::fs::read(path).map_err(|_| "manual policy unavailable")?)
            .map_err(|_| "invalid manual policy JSON")?;
    policy.validate()?;
    Ok(policy)
}

#[cfg(test)]
mod manual_tests {
    use super::*;
    use chrono::{Duration, Utc};
    use zadp_indexer::{ChainObservation, OfferRecord};
    use zadp_protocol::{bind_offer, OfferDraft, Price, SellerKeyPair};
    fn record(key: &SellerKeyPair, title: &str, endpoint: &str) -> OfferRecord {
        let now = Utc::now();
        let offer = bind_offer(
            OfferDraft::new_ulid(
                key.id.clone(),
                "repair",
                title,
                Price::zec("0.00001".parse().unwrap()),
                endpoint,
                now + Duration::days(7),
                "AAAAAAAAAAAAAAAAAAAAAA==",
            )
            .sign(key)
            .unwrap(),
            "fixture",
            now,
            key,
        )
        .unwrap();
        OfferRecord {
            offer,
            source_txid: zadp_zcash::TxId("fixture".into()),
            indexed_at: now,
            chain: Some(ChainObservation {
                network: zadp_protocol::ZcashNetwork::Mainnet,
                board_id: "fixture".into(),
                block_height: 1,
                block_hash: "00".repeat(32),
                transaction_id: "00".repeat(32),
                pool: "orchard".into(),
                output_index: 0,
                listing_value_zat: 1000,
                scanned_height: 4,
                confirmations: 4,
                confirmation_status: "confirmed".into(),
                offer_status: "active".into(),
            }),
        }
    }
    fn policy(key: &SellerKeyPair, r: &OfferRecord) -> ManualPolicy {
        serde_json::from_value(serde_json::json!({"version":1,"sellers":[{"seller":key.id,"status":"approved"}],"offers":[{"board_id":"fixture","seller":key.id,"offer_id":r.offer.id,"status":"approved"}],"demo_auto_approved_sellers":[],"categories":["repair"],"capability_categories":{},"minimum_publication_payment_zat":1000})).unwrap()
    }
    #[test]
    fn payment_is_not_distribution_and_blocking_wins() {
        let key = SellerKeyPair::generate();
        let r = record(&key, "Bike repair", "https://service.example");
        let mut p = policy(&key, &r);
        p.validate().unwrap();
        assert!(p.permits(&r));
        p.offers.clear();
        assert!(!p.permits(&r));
        p.demo_auto_approved_sellers.insert(key.id.clone());
        assert!(p.permits(&r));
        p.offers = policy(&key, &r).offers;
        p.offers[0].status = OfferDecision::Pending;
        assert!(!p.permits(&r));
        p.offers[0].status = OfferDecision::Blocked;
        assert!(!p.permits(&r));
        p.offers[0].status = OfferDecision::Approved;
        for status in [SellerStatus::Suspended, SellerStatus::Blocked] {
            p.sellers[0].status = status;
            assert!(!p.permits(&r));
        }
        p.sellers[0].status = SellerStatus::Approved;
        let mut unpaid = r.clone();
        unpaid.chain.as_mut().unwrap().listing_value_zat = 999;
        assert!(!p.permits(&unpaid));
        unpaid.chain = None;
        assert!(!p.permits(&unpaid));
        let mut expired = r.clone();
        expired.offer.expires_at = Utc::now() - Duration::seconds(1);
        assert!(!p.permits(&expired));
        let other = record(
            &SellerKeyPair::generate(),
            "Repair",
            "https://service.example",
        );
        assert!(!p.permits(&other));
    }
    #[test]
    fn mechanical_restrictions_are_host_policy_not_protocol() {
        let key = SellerKeyPair::generate();
        let r = record(&key, "<script>bad</script>", "https://service.example");
        assert!(!policy(&key, &r).permits(&r));
        let r = record(&key, &"x".repeat(121), "https://service.example");
        assert!(!policy(&key, &r).permits(&r));
        let r = record(&key, "Repair", "https://service.example");
        let mut p = policy(&key, &r);
        p.categories = HashSet::from(["software".into()]);
        assert!(!p.permits(&r));
        for endpoint in [
            "http://public.example",
            "https://10.0.0.1",
            "https://169.254.169.254",
            "https://[::1]",
            "https://[fc00::1]",
            "https://user:pass@public.example",
            "https://host.local",
            "not a URL",
        ] {
            assert!(
                zadp_protocol::commerce_origin(endpoint).is_err(),
                "{endpoint}"
            );
        }
    }
    #[test]
    fn endpoint_ownership_does_not_replace_manual_host_approval_or_visibility() {
        let key = SellerKeyPair::generate();
        let r = record(&key, "Repair", "https://127.0.0.1");
        let mut p = policy(&key, &r);
        assert!(p.permits(&r)); // Visible discovery; unsafe commerce stays disabled separately.
        let v = crate::EndpointVerifier::default().status(&r.offer);
        assert_eq!(v.status, crate::CommerceStatus::Unverified);
        p.sellers[0].status = SellerStatus::Blocked;
        assert!(!p.permits(&r));
    }
    #[test]
    fn atomic_file_reload_and_restart_remain_fail_closed() {
        let key = SellerKeyPair::generate();
        let r = record(&key, "Repair", "https://service.example");
        let p = policy(&key, &r);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("policy.json");
        let value = serde_json::json!({"version":1,"sellers":[{"seller":key.id,"status":"approved"}],"offers":[{"board_id":"fixture","seller":key.id,"offer_id":r.offer.id,"status":"approved"}],"demo_auto_approved_sellers":[],"categories":["repair"],"capability_categories":{},"minimum_publication_payment_zat":1000});
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let live = HostPolicy::manual_file(path.clone()).unwrap();
        assert!(live.snapshot().permits_record(&r));
        assert!(p.permits(&r));
        let mut blocked = value;
        blocked["offers"][0]["status"] = "blocked".into();
        let replacement = directory.path().join("replacement");
        std::fs::write(&replacement, serde_json::to_vec(&blocked).unwrap()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&replacement, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        std::fs::rename(&replacement, &path).unwrap();
        assert!(!live.snapshot().permits_record(&r));
        assert!(!HostPolicy::manual_file(path.clone())
            .unwrap()
            .snapshot()
            .permits_record(&r));
        std::fs::write(&path, "invalid").unwrap();
        assert!(!live.snapshot().permits_record(&r));
        assert!(HostPolicy::manual_file(path).is_err());
    }
}
