use super::*;

#[test]
fn verifies_signature_and_rejects_tampering() {
    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::Value;
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("payload.bin"), b"payload").unwrap();
    let key = SigningKey::from_bytes(&[7u8; 32]);
    let public = key.verifying_key().to_bytes();
    let publisher = PluginPublisher {
        id: "com.example.test".into(),
        name: "Test".into(),
        key_id: key_id(&public),
        public_key: STANDARD.encode(public),
    };
    let files = BTreeMap::from([(
        "payload.bin".to_string(),
        hash_file(&temp.path().join("payload.bin")).unwrap(),
    )]);
    let manifest = PluginManifest {
        locales: Default::default(),
        schema_version: 2,
        id: "com.example.test".into(),
        name: "Test".into(),
        version: "1.0.0".into(),
        description: String::new(),
        icon: None,
        runtime: crate::plugins::host::PluginRuntime::default(),
        permissions: crate::plugins::host::PluginPermissions::default(),
        extensions: Value::Null,
        author: None,
        publisher: Some(publisher),
        files,
    };
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    let mut signed = DOMAIN.to_vec();
    signed.extend_from_slice(&manifest_bytes);
    std::fs::write(
        temp.path().join(SIGNATURE_FILE),
        STANDARD.encode(key.sign(&signed).to_bytes()),
    )
    .unwrap();
    let result = verify(&manifest_bytes, &manifest, temp.path()).unwrap();
    assert!(result.signed);
    assert!(!result.trusted);
    std::fs::write(temp.path().join("payload.bin"), b"changed").unwrap();
    assert!(verify(&manifest_bytes, &manifest, temp.path()).is_err());
}
#[test]
fn fingerprint_is_grouped_and_stable() {
    let key = [0u8; 32];
    let expected = key_id(&key)
        .as_bytes()
        .chunks(4)
        .map(|chunk| String::from_utf8_lossy(chunk).to_uppercase())
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(fingerprint(&key), expected);
    assert_eq!(key_id(&key).len(), 64);
}
