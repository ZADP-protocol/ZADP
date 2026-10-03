//! Zallet's supported PCZT RPC, restricted to authenticated local loopback.
use crate::local::read_protected;
use anyhow::{ensure, Result};
use serde_json::{json, Value};
use std::{path::Path, time::Duration};
use zeroize::Zeroizing;

pub struct LocalWallet {
    client: reqwest::Client,
    url: url::Url,
    user: Zeroizing<String>,
    password: Zeroizing<String>,
}
impl LocalWallet {
    pub fn connect(url: &str, auth: &Path) -> Result<Self> {
        let url = url::Url::parse(url).map_err(|_| anyhow::anyhow!("invalid wallet RPC URL"))?;
        ensure!(
            url.scheme() == "http"
                && matches!(url.host_str(), Some("127.0.0.1" | "[::1]"))
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none(),
            "wallet RPC must be an explicit loopback IP over HTTP without URL credentials"
        );
        let bytes = read_protected(auth)?;
        let auth = std::str::from_utf8(&bytes)
            .map_err(|_| anyhow::anyhow!("invalid wallet authentication file"))?;
        let (user, password) = auth.trim_end().split_once(':').ok_or_else(|| {
            anyhow::anyhow!("wallet authentication file must contain username:password")
        })?;
        ensure!(
            !user.is_empty()
                && !password.is_empty()
                && !user.contains('\n')
                && !password.contains('\n'),
            "invalid wallet authentication file"
        );
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(300))
            .build()?;
        Ok(Self {
            client,
            url,
            user: Zeroizing::new(user.into()),
            password: Zeroizing::new(password.into()),
        })
    }
    pub async fn rpc(&self, method: &str, params: Value) -> Result<Value> {
        let response=self.client.post(self.url.clone()).basic_auth(self.user.as_str(),Some(self.password.as_str()))
            .json(&json!({"jsonrpc":"2.0","id":"zadp","method":method,"params":params})).send().await
            .map_err(|_|anyhow::anyhow!("local wallet RPC transport failed; consult wallet locally, do not retry broadcast blindly"))?;
        ensure!(
            response.status().is_success(),
            "local wallet RPC HTTP error"
        );
        let value: Value = response
            .json()
            .await
            .map_err(|_| anyhow::anyhow!("invalid local wallet RPC JSON"))?;
        ensure!(value.get("error").is_none_or(Value::is_null),"local wallet RPC {method} failed; inspect wallet locally (response suppressed to avoid leaking wallet data)");
        value
            .get("result")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing RPC result"))
    }
}

/// Verify the wallet-created unsigned proposal before any signing method is called.
pub fn inspect_fee(value: &Value, address: &str, listing_value_zat: u64) -> Result<u64> {
    ensure!(
        value["wallet_created"] == true && value["privacy_policy"] == "FullPrivacy",
        "wallet proposal is not a wallet-created FullPrivacy transaction"
    );
    ensure!(
        value["transparent"]["inputs"]
            .as_array()
            .is_some_and(Vec::is_empty)
            && value["transparent"]["outputs"]
                .as_array()
                .is_some_and(Vec::is_empty),
        "transparent wallet inputs or outputs are forbidden"
    );
    ensure!(
        value["sapling"]["spends"] == 0
            && value["sapling"]["outputs"]
                .as_array()
                .is_some_and(Vec::is_empty),
        "MVP requires a single Orchard-family shielded pool"
    );
    let mut matches = 0;
    for pool in ["orchard", "ironwood"] {
        for output in value[pool]["outputs"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("missing shielded output inspection"))?
        {
            if output["user_address"] == address {
                ensure!(
                    output["value_zat"].as_u64() == Some(listing_value_zat),
                    "proposal changed listing value"
                );
                matches += 1;
            } else {
                ensure!(
                    output["user_address"].is_null(),
                    "proposal contains an unexpected external recipient"
                );
            }
        }
    }
    ensure!(
        matches == 1,
        "proposal must contain exactly one payment to the board"
    );
    value["fee_zat"]
        .as_u64()
        .filter(|v| *v > 0 && *v <= 2_100_000_000_000_000)
        .ok_or_else(|| anyhow::anyhow!("wallet did not supply a positive fee"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn preview() -> Value {
        json!({"wallet_created":true,"privacy_policy":"FullPrivacy","fee_zat":12345,"transparent":{"inputs":[],"outputs":[]},"sapling":{"spends":0,"outputs":[]},"orchard":{"outputs":[{"user_address":"board","value_zat":1000},{}]},"ironwood":{"outputs":[]}})
    }
    #[test]
    fn fee_is_wallet_supplied_and_modified_recipient_is_rejected() {
        let mut p = preview();
        assert_eq!(inspect_fee(&p, "board", 1000).unwrap(), 12345);
        p["orchard"]["outputs"][0]["value_zat"] = json!(999);
        assert!(inspect_fee(&p, "board", 1000).is_err());
        p = preview();
        p["transparent"]["inputs"] = json!([{}]);
        assert!(inspect_fee(&p, "board", 1000).is_err());
        p = preview();
        p["privacy_policy"] = json!("NoPrivacy");
        assert!(inspect_fee(&p, "board", 1000).is_err());
        p = preview();
        p["fee_zat"] = json!(-1);
        assert!(inspect_fee(&p, "board", 1000).is_err());
    }
}
