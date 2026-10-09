use chrono::{DateTime, Duration, Utc};
use zadp_protocol::{commerce_origin, public_endpoint_ip, EndpointProof, SellerKeyPair};
fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1_800_000_000, 0).unwrap()
}
#[test]
fn offline_proof_binds_seller_origin_and_all_fields() {
    let key = SellerKeyPair::from_secret_bytes(&[7; 32]);
    let proof = EndpointProof::sign(
        &key,
        "https://seller.example",
        now(),
        now() + Duration::days(30),
    )
    .unwrap();
    proof
        .verify(&key.id, "https://SELLER.example:443/service", now())
        .unwrap();
    let other = SellerKeyPair::from_secret_bytes(&[8; 32]);
    assert!(proof.verify(&other.id, &proof.origin, now()).is_err());
    assert!(proof
        .verify(&key.id, "https://other.example", now())
        .is_err());
    assert!(proof
        .verify(&key.id, &proof.origin, proof.expires_at)
        .is_err());
    assert!(proof
        .verify(
            &key.id,
            &proof.origin,
            proof.expires_at - Duration::seconds(1)
        )
        .is_ok());
    assert!(proof
        .verify(&key.id, &proof.origin, now() - Duration::seconds(1))
        .is_err());
    let json = serde_json::to_value(&proof).unwrap();
    for (field, value) in [
        ("seller", serde_json::json!(other.id)),
        ("origin", serde_json::json!("https://other.example")),
        ("issued_at", serde_json::json!(now() - Duration::seconds(1))),
        ("expires_at", serde_json::json!(now() + Duration::days(31))),
        ("zadp", serde_json::json!(2)),
        ("type", serde_json::json!("offer")),
        ("signature", serde_json::json!("bad")),
    ] {
        let mut modified = json.clone();
        modified[field] = value;
        if let Ok(p) = serde_json::from_value::<EndpointProof>(modified) {
            assert!(p.verify(&key.id, &proof.origin, now()).is_err(), "{field}");
        }
    }
    for bad in ["{}", "null", r#"{"zadp":1,"type":"commerce-endpoint"}"#] {
        assert!(serde_json::from_str::<EndpointProof>(bad).is_err());
    }
    let mut extra = json.clone();
    extra["private_key"] = serde_json::json!("not allowed");
    assert!(serde_json::from_value::<EndpointProof>(extra).is_err());
    assert_eq!(json.as_object().unwrap().len(), 7);
    assert!(!serde_json::to_string(&proof).unwrap().contains("private"));
    assert!(proof
        .signing_bytes()
        .unwrap()
        .starts_with(b"ZADP-ENDPOINT-PROOF/1\0"));
    assert_eq!(
        serde_json::from_value::<EndpointProof>(json)
            .unwrap()
            .signing_bytes()
            .unwrap(),
        proof.signing_bytes().unwrap()
    );
    assert!(EndpointProof::sign(
        &key,
        "https://seller.example/",
        now(),
        now() + Duration::days(1)
    )
    .is_err());
}
#[test]
fn endpoint_and_dns_addresses_fail_closed() {
    for url in [
        "http://seller.example",
        "https://user:pass@seller.example",
        "https://@seller.example",
        "https://localhost",
        "https://x.local",
        "https://x.internal",
        "https://127.0.0.1",
        "https://10.1.2.3",
        "https://172.16.0.1",
        "https://192.168.1.1",
        "https://169.254.169.254",
        "https://100.64.0.1",
        "https://198.18.0.1",
        "https://[::1]",
        "https://[fc00::1]",
        "https://[fe80::1]",
        "https://[::ffff:8.8.8.8]",
        "https://[64:ff9b::a00:1]",
        "https://[2001:db8::1]",
        "https://2130706433",
        "https://seller.example/#x",
        "https://seller.example\\@localhost",
        " https://seller.example",
    ] {
        assert!(commerce_origin(url).is_err(), "{url}");
    }
    for ip in [
        "127.0.0.1",
        "10.0.0.1",
        "169.254.169.254",
        "192.0.0.8",
        "198.19.0.1",
        "240.0.0.1",
        "::",
        "::ffff:127.0.0.1",
        "2001:db8::1",
        "2002:a00:1::",
    ] {
        assert!(!public_endpoint_ip(ip.parse().unwrap()), "{ip}");
    }
    for ip in ["8.8.8.8", "1.1.1.1", "2606:4700:4700::1111"] {
        assert!(public_endpoint_ip(ip.parse().unwrap()));
    }
    assert_eq!(
        commerce_origin("https://SELLER.example:443/path?x=y").unwrap(),
        "https://seller.example"
    );
    assert_eq!(
        commerce_origin("https://seller.example:8443/service").unwrap(),
        "https://seller.example:8443"
    );
}
