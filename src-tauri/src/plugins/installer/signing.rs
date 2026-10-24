//! Publisher signature and trust checks. The cryptographic verification lives
//! in [`crate::plugin_signing`]; this module applies the installer's trust gate
//! and derives the preview fingerprint.
use crate::plugin_host::PluginManifest;
use crate::plugin_signing::Verification;

/// Enforce explicit consent for untrusted publishers and remember a publisher
/// once the user has accepted its fingerprint.
pub(super) fn trust_gate(
    verification: &Verification,
    manifest: &PluginManifest,
    allow_untrusted: bool,
) -> Result<(), String> {
    if !verification.trusted && !allow_untrusted {
        return Err("plugin publisher is not trusted; review the fingerprint and confirm".into());
    }
    if verification.signed && !verification.trusted {
        let publisher = manifest
            .publisher
            .as_ref()
            .ok_or("signed plugin has no publisher")?;
        crate::plugin_signing::trust_publisher(publisher)?;
    }
    Ok(())
}

/// Human-checkable fingerprint of the publisher key that signed the package.
pub(super) fn fingerprint(verification: &Verification) -> Option<String> {
    verification
        .publisher_key
        .as_deref()
        .and_then(|key| {
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, key).ok()
        })
        .map(|key| crate::plugin_signing::fingerprint(&key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_signing::Verification;

    fn verification(signed: bool, trusted: bool) -> Verification {
        Verification {
            signed,
            trusted,
            publisher: None,
            key_id: None,
            publisher_key: None,
        }
    }

    fn manifest() -> PluginManifest {
        PluginManifest {
            schema_version: 1,
            id: "org.fiex.test".into(),
            name: "Test".into(),
            version: "1.0.0".into(),
            description: String::new(),
            icon: None,
            runtime: crate::plugin_host::PluginRuntime::default(),
            permissions: crate::plugin_host::PluginPermissions::default(),
            contributions: serde_json::Value::Null,
            author: None,
            publisher: None,
            files: Default::default(),
        }
    }

    #[test]
    fn untrusted_publishers_require_explicit_consent() {
        let manifest = manifest();
        assert!(trust_gate(&verification(false, false), &manifest, false).is_err());
        assert!(trust_gate(&verification(false, false), &manifest, true).is_ok());
        assert!(trust_gate(&verification(true, true), &manifest, false).is_ok());
    }

    #[test]
    fn fingerprint_is_derived_from_the_publisher_key() {
        let key = [3u8; 32];
        let mut verification = verification(true, false);
        verification.publisher_key = Some(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            key,
        ));
        assert_eq!(
            fingerprint(&verification),
            Some(crate::plugin_signing::fingerprint(&key))
        );
    }
}
