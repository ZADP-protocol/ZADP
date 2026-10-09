use chrono::{Duration, Utc};
use std::process::Command;
use zadp_protocol::{EndpointProof, SellerKeyPair};
#[test]
fn offline_command_outputs_only_public_proof_and_never_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    let identity = dir.path().join("seller.identity");
    let output = dir.path().join("proof.json");
    let key = SellerKeyPair::from_secret_bytes(&[17; 32]);
    zadp_seller::local::write_identity(&identity, &key).unwrap();
    let expiry =
        chrono::DateTime::from_timestamp(Utc::now().timestamp(), 0).unwrap() + Duration::days(1);
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_zadp-seller"))
            .arg("endpoint-proof")
            .arg("--identity")
            .arg(&identity)
            .args([
                "--origin",
                "https://seller.example",
                "--expires-at",
                &expiry.to_rfc3339(),
            ])
            .arg("--output")
            .arg(&output)
            .output()
            .unwrap()
    };
    let result = run();
    assert!(result.status.success());
    let original = std::fs::read(&output).unwrap();
    let proof: EndpointProof = serde_json::from_slice(&original).unwrap();
    proof
        .verify(&key.id, "https://seller.example", Utc::now())
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&original)
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        7
    );
    assert!(!String::from_utf8(result.stdout)
        .unwrap()
        .contains(&"11".repeat(32)));
    assert!(!run().status.success());
    assert_eq!(std::fs::read(output).unwrap(), original);
}
