//! Hosted presentation rules v1. Never affect canonical protocol admission.
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Approved,
    PendingReview,
    Rejected,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Decision {
    pub outcome: Outcome,
    pub signals: Vec<String>,
}
fn pattern(s: &str) -> Regex {
    Regex::new(s).expect("static moderation pattern")
}
static EMAIL: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b"));
// Explicit international prefix + grouping: deliberately excludes bare numeric IDs.
static PHONE: LazyLock<Regex> = LazyLock::new(|| {
    pattern(r"(?:^|[^\w])\+[1-9][0-9]{0,2}[ .()-]+[0-9][0-9 .()-]{6,}[0-9](?:$|[^\w])")
});
static ID: LazyLock<Regex> = LazyLock::new(|| {
    pattern(
        r"(?i)\b(?:ssn|social security|passport|national id|government id)\s*(?:number\s*)?[:#]\s*[A-Z0-9][A-Z0-9 -]{5,20}\b",
    )
});
static AMBIGUOUS: LazyLock<Regex> = LazyLock::new(|| {
    pattern(
        r"(?i)\b(?:email|e-mail)\s*[:=]\s*\S+\s*(?:\[at\]|\(at\))\s*\S+|\b(?:phone|call|whatsapp|telephone)\s*[:=]\s*[0-9][0-9 ()-]{6,}[0-9]",
    )
});
/// All email addresses in listing text are excluded; domains belong in the signed HTTPS endpoint.
/// Generic words (e.g. drugs, weapons) are intentionally NOT banned: context is unreliable.
pub fn assess(text: &str) -> Decision {
    let mut signals = Vec::new();
    let normalized = normalized(text);
    if [
        "stolen credit card numbers for sale",
        "buy stolen passwords",
        "child sexual abuse material for sale",
    ]
    .iter()
    .any(|phrase| normalized.contains(phrase))
    {
        signals.push("explicit_prohibited_content".into());
    }
    if EMAIL.is_match(text) {
        signals.push("email_address".into());
    }
    if PHONE
        .find_iter(text)
        .any(|m| (9..=15).contains(&m.as_str().chars().filter(char::is_ascii_digit).count()))
    {
        signals.push("international_phone".into());
    }
    if ID.is_match(text) {
        signals.push("labelled_government_id".into());
    }
    if !signals.is_empty() {
        return Decision {
            outcome: Outcome::Rejected,
            signals,
        };
    }
    if AMBIGUOUS.is_match(text) {
        return Decision {
            outcome: Outcome::PendingReview,
            signals: vec!["ambiguous_contact".into()],
        };
    }
    Decision {
        outcome: Outcome::Approved,
        signals,
    }
}
pub fn offer_text(o: &zadp_protocol::Offer) -> String {
    format!(
        "{}\n{}\n{}\n{}",
        o.title,
        o.description.as_deref().unwrap_or(""),
        o.capability,
        o.commerce
            .as_ref()
            .map(|c| c.endpoint.as_str())
            .unwrap_or("")
    )
}
pub fn fingerprint(o: &zadp_protocol::Offer) -> String {
    format!(
        "{}|{:?}|{:?}|{:?}",
        normalized(&offer_text(o)),
        o.price,
        o.commerce,
        o.geo
    )
}
pub fn normalized(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub report_id: String,
    pub offer_id: String,
    pub board_id: String,
    pub seller: String,
    pub reason: String,
    pub explanation: String,
    pub created_at: i64,
    pub status: String,
}
pub fn validate_report(key: &str, reason: &str, explanation: &str) -> bool {
    (16..=80).contains(&key.len())
        && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && [
            "illegal_content",
            "scam_fraud",
            "personal_information",
            "spam",
            "other",
        ]
        .contains(&reason)
        && explanation.len() <= 2000
        && !explanation.chars().any(|c| c.is_control() && c != '\n')
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conservative_rules() {
        for text in [
            "Consulting $3.00 0.001 ZEC",
            "u1abcdef0123456789",
            "ed25519:abc123",
            "TX 0123456789abcdef",
            "Product 123-45-6789",
            "Acme Smith Ltd",
            "Translation for government ID documents",
        ] {
            assert_eq!(assess(text).outcome, Outcome::Approved, "{text}");
        }
        for text in [
            "hello jane@example.org",
            "Call +358 40 123 4567",
            "SSN: 123-45-6789",
        ] {
            assert_eq!(assess(text).outcome, Outcome::Rejected, "{text}");
        }
        assert_eq!(
            assess("email: jane [at] example.org").outcome,
            Outcome::PendingReview
        );
        assert_eq!(
            assess("phone: 040 123 4567").outcome,
            Outcome::PendingReview
        );
        assert!(!validate_report("short", "spam", ""));
        assert!(validate_report("abcdefghijklmnop", "spam", "duplicate"));
    }
}
#[cfg(test)]
mod fixture_tests {
    #[test]
    fn public_browser_fixture_parity() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/moderation.json")).unwrap();
        for f in fixtures.as_array().unwrap() {
            let d = super::assess(f["text"].as_str().unwrap());
            assert_eq!(serde_json::to_value(d).unwrap()["outcome"], f["outcome"]);
        }
    }
}
