//! Private credentials produced during connection validation never reach UI.
use serde_json::{Map, Value};
pub(super) fn append_private_fields(
    factory: &Value,
    fields: &mut Vec<(String, String)>,
) -> Result<(), String> {
    let Some(keys) = factory.get("credentials") else {
        return Ok(());
    };
    let keys = keys
        .as_array()
        .filter(|v| v.len() <= 32)
        .ok_or("invalid private connection credentials")?;
    for key in keys {
        let key = key
            .as_str()
            .filter(|v| {
                !v.is_empty()
                    && v.len() <= 80
                    && v.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            })
            .ok_or("invalid private credential name")?;
        if fields.iter().any(|(id, _)| id == key) {
            return Err("duplicate connection credential".into());
        }
        fields.push((key.into(), "secret".into()));
    }
    Ok(())
}
pub(super) fn apply_validation(
    settings: &mut Map<String, Value>,
    secret_keys: &[String],
    reply: Value,
) -> Result<(), String> {
    if reply.get("valid").and_then(Value::as_bool) == Some(false) {
        return Err("provider rejected connection configuration".into());
    }
    let Some(credentials) = reply.get("credentials") else {
        return Ok(());
    };
    let credentials = credentials
        .as_object()
        .filter(|v| v.len() <= 32)
        .ok_or("invalid connection validation credentials")?;
    // Validate the whole reply before adding any value to the pending record.
    for (key, value) in credentials {
        if !secret_keys.contains(key)
            || value
                .as_str()
                .is_none_or(|v| v.is_empty() || v.len() > 65536)
        {
            return Err(
                "connection validation returned an undeclared or invalid credential".into(),
            );
        }
    }
    settings.extend(credentials.clone());
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn tokens_must_be_declared_and_are_only_applied_after_full_validation() {
        let mut fields = vec![("clientId".into(), "text".into())];
        append_private_fields(&json!({"credentials":["refreshToken"]}), &mut fields).unwrap();
        assert_eq!(fields[1], ("refreshToken".into(), "secret".into()));
        let mut settings = Map::new();
        assert!(apply_validation(
            &mut settings,
            &["refreshToken".into()],
            json!({"credentials":{"refreshToken":"token","unknown":"bad"}})
        )
        .is_err());
        assert!(settings.is_empty());
        apply_validation(
            &mut settings,
            &["refreshToken".into()],
            json!({"valid":true,"credentials":{"refreshToken":"token"}}),
        )
        .unwrap();
        assert_eq!(settings["refreshToken"], "token");
    }
}
