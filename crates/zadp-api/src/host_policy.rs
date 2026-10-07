//! Operator presentation policy, independent of signed board admission and canonical storage.
use std::collections::HashSet;
use zadp_protocol::SellerId;

#[derive(Debug, Clone, Default)]
pub enum HostPolicy {
    #[default]
    Unrestricted,
    Allowlist(HashSet<SellerId>),
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
        Self::from_config(mode.as_deref(), entries.as_deref())
    }
    pub fn permits(&self, seller: &SellerId) -> bool {
        match self {
            Self::Unrestricted => true,
            Self::Allowlist(sellers) => sellers.contains(seller),
        }
    }
}
