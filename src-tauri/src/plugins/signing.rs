//! Ed25519 manifest signatures for `.fiexplugin` packages.
//!
//! The signature covers the exact `manifest.json` bytes. The manifest carries a
//! SHA-256 digest for every other file in the package, so verifying the
//! signature and then every file proves the complete package was signed.
use crate::plugins::host::{PluginManifest, PluginPublisher};
use base64::{engine::general_purpose::STANDARD, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const SIGNATURE_FILE: &str = "manifest.sig";
const MANIFEST_FILE: &str = "manifest.json";
const DOMAIN: &[u8] = b"fiex-plugin-manifest-v1\0";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Verification {
    pub signed: bool,
    pub trusted: bool,
    pub publisher: Option<String>,
    pub key_id: Option<String>,
    pub publisher_key: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrustedPublisher {
    id: String,
    key_id: String,
    public_key: String,
}

#[derive(Default, Serialize, Deserialize)]
struct TrustStore {
    #[serde(default)]
    publishers: Vec<TrustedPublisher>,
}

fn trust_path() -> Result<PathBuf, String> {
    dirs::data_local_dir()
        .map(|path| path.join("fiex").join("plugin-trust.json"))
        .ok_or("application data directory unavailable".into())
}

fn load_trust() -> Result<TrustStore, String> {
    match fs::read(trust_path()?) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|error| format!("plugin trust store invalid: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(TrustStore::default()),
        Err(error) => Err(error.to_string()),
    }
}

fn save_trust(store: &TrustStore) -> Result<(), String> {
    let path = trust_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(
        &path,
        serde_json::to_vec_pretty(store).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

pub fn key_id(public_key: &[u8]) -> String {
    hex::encode(Sha256::digest(public_key))
}

pub fn fingerprint(public_key: &[u8]) -> String {
    key_id(public_key)
        .as_bytes()
        .chunks(4)
        .map(|chunk| String::from_utf8_lossy(chunk).to_uppercase())
        .collect::<Vec<_>>()
        .join(" ")
}

fn decode_key(value: &str) -> Result<VerifyingKey, String> {
    let bytes = STANDARD
        .decode(value.trim())
        .map_err(|error| format!("publisher key is not valid base64: {error}"))?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "publisher key must be 32 bytes".to_string())?;
    VerifyingKey::from_bytes(&bytes).map_err(|error| error.to_string())
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("sha256:{}", hex::encode(digest.finalize())))
}

fn collect_files(
    root: &Path,
    current: &Path,
    files: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    for entry in fs::read_dir(current).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if meta.file_type().is_symlink() {
            return Err("plugin package contains a symbolic link".into());
        }
        if meta.is_dir() {
            collect_files(root, &path, files)?;
        } else if meta.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| "plugin file escaped its package".to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            if relative == MANIFEST_FILE || relative == SIGNATURE_FILE {
                continue;
            }
            files.insert(relative, hash_file(&path)?);
        }
    }
    Ok(())
}

/// Verify a package's manifest signature and file digests. Unsigned packages
/// are returned as `signed = false`; malformed signed packages are errors.
pub fn verify(
    manifest_bytes: &[u8],
    manifest: &PluginManifest,
    root: &Path,
) -> Result<Verification, String> {
    let signature_path = root.join(SIGNATURE_FILE);
    if !signature_path.is_file() {
        return Ok(Verification {
            signed: false,
            trusted: false,
            publisher: manifest.publisher.as_ref().map(|value| value.id.clone()),
            key_id: manifest
                .publisher
                .as_ref()
                .map(|value| value.key_id.clone()),
            publisher_key: manifest
                .publisher
                .as_ref()
                .map(|value| value.public_key.clone()),
        });
    }
    let publisher = manifest
        .publisher
        .clone()
        .ok_or("signed plugin has no publisher")?;
    if manifest.files.is_empty() {
        return Err("signed plugin has no file digests".into());
    }
    let key = decode_key(&publisher.public_key)?;
    let computed_id = key_id(key.as_bytes());
    if publisher.key_id != computed_id {
        return Err("publisher key id does not match the public key".into());
    }
    let signature = STANDARD
        .decode(
            fs::read_to_string(&signature_path)
                .map_err(|error| error.to_string())?
                .trim(),
        )
        .map_err(|error| format!("manifest signature is not valid base64: {error}"))?;
    let signature = Signature::from_slice(&signature)
        .map_err(|error| format!("manifest signature is invalid: {error}"))?;
    let mut signed = Vec::with_capacity(DOMAIN.len() + manifest_bytes.len());
    signed.extend_from_slice(DOMAIN);
    signed.extend_from_slice(manifest_bytes);
    key.verify(&signed, &signature)
        .map_err(|_| "manifest signature verification failed".to_string())?;

    let mut actual = BTreeMap::new();
    collect_files(root, root, &mut actual)?;
    if actual != manifest.files {
        return Err("plugin file digests do not match the signed manifest".into());
    }

    let trusted = load_trust()?.publishers.iter().any(|item| {
        item.id == publisher.id
            && item.key_id == publisher.key_id
            && item.public_key == publisher.public_key
    });
    Ok(Verification {
        signed: true,
        trusted,
        publisher: Some(publisher.id),
        key_id: Some(publisher.key_id),
        publisher_key: Some(publisher.public_key),
    })
}

/// Re-check a signed package after installation, before spawning it.
pub fn verify_installed(manifest: &PluginManifest, root: &Path) -> Result<Verification, String> {
    let bytes = fs::read(root.join(MANIFEST_FILE))
        .map_err(|error| format!("installed manifest is unreadable: {error}"))?;
    verify(&bytes, manifest, root)
}

/// Remember a third-party publisher after the user explicitly trusts it.
pub fn trust_publisher(publisher: &PluginPublisher) -> Result<(), String> {
    let key = decode_key(&publisher.public_key)?;
    let id = key_id(key.as_bytes());
    if id != publisher.key_id {
        return Err("publisher key id does not match the public key".into());
    }
    let mut store = load_trust()?;
    if let Some(existing) = store
        .publishers
        .iter_mut()
        .find(|item| item.id == publisher.id)
    {
        if existing.key_id != publisher.key_id {
            return Err("publisher key changed; remove the old trust entry first".into());
        }
        existing.public_key = publisher.public_key.clone();
    } else {
        store.publishers.push(TrustedPublisher {
            id: publisher.id.clone(),
            key_id: publisher.key_id.clone(),
            public_key: publisher.public_key.clone(),
        });
    }
    save_trust(&store)
}

#[cfg(test)]
mod tests {
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
}
