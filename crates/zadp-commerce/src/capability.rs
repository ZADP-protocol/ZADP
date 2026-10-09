//! Seller-side service description, not an Offer field or protocol authority.
use serde_json::{json, Value};

pub fn supported(capability: &str) -> bool {
    matches!(capability, "text.uppercase" | "text.summarize")
}
pub fn describe(capability: &str, offer_id: &str) -> Value {
    json!({
        "zadp_service": 1, "offer_id": offer_id, "capability": capability,
        "content_types": {"request":"application/json", "response":"application/json"},
        "request_schema": {"type":"object","required":["text"],"additionalProperties":false,
            "properties":{"text":{"type":"string","minLength":1,"maxLength":4000}}},
        "response_schema": {"type":"string","maxLength":12000},
        "transport":{"create_order":"POST /orders","request_field":"request","poll_order":"GET /orders/{id}","result_field":"result"},
        "limits":{"input_utf8_bytes":4000,"result_utf8_bytes":12000},
        "semantics": if capability == "text.summarize" {
            "Deterministic extractive summary: first three punctuation-delimited sentences. No LLM or external provider."
        } else {"Unicode uppercase transformation."},
        "media": null,
        "authority":"Seller-supplied service metadata. Not part of the signed Offer; validate against the verified seller endpoint."
    })
}
pub fn execute(capability: &str, input: &str) -> anyhow::Result<String> {
    anyhow::ensure!(supported(capability), "unsupported capability");
    Ok(match capability {
        "text.uppercase" => input.to_uppercase(),
        _ => input
            .trim()
            .split_inclusive(['.', '!', '?'])
            .take(3)
            .collect::<String>()
            .trim()
            .to_owned(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_deterministic_services_and_descriptor() {
        assert_eq!(execute("text.uppercase", "hello").unwrap(), "HELLO");
        assert_eq!(
            execute("text.summarize", " One. Two! Three? Four.").unwrap(),
            "One. Two! Three?"
        );
        assert_eq!(
            execute("text.summarize", "No punctuation").unwrap(),
            "No punctuation"
        );
        assert!(execute("llm.chat", "text").is_err());
        let descriptor = describe("text.summarize", "offer-1");
        assert_eq!(descriptor["offer_id"], "offer-1");
        assert_eq!(descriptor["request_schema"]["additionalProperties"], false);
        assert!(descriptor["media"].is_null());
    }
}
