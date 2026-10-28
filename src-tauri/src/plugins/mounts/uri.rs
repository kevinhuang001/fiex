#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Uri {
    pub mount_id: String,
    pub relative: String,
}

pub fn handles(path: &str) -> bool {
    path.get(..5)
        .is_some_and(|s| s.eq_ignore_ascii_case("fiex:"))
}

pub fn segment(part: &str) -> Result<(), String> {
    if part.is_empty()
        || part.len() > 1024
        || matches!(part, "." | "..")
        || part.contains(['/', '\\', '\0', ':'])
        || part.chars().any(char::is_control)
    {
        return Err("InvalidVfsUri: invalid path segment".into());
    }
    // Reject nested encodings of traversal/separators, while allowing a literal %.
    let lower = part.to_ascii_lowercase();
    if ["%2e", "%2f", "%5c", "%00", "%25"]
        .iter()
        .any(|v| lower.contains(v))
    {
        return Err("InvalidVfsUri: nested path encoding".into());
    }
    Ok(())
}

pub fn parse(path: &str) -> Result<Uri, String> {
    if path.len() > 32768 {
        return Err("InvalidVfsUri: URI too long".into());
    }
    let rest = path
        .strip_prefix("fiex://mount/")
        .ok_or("InvalidVfsUri: expected fiex://mount/")?;
    let (id, relative) = rest
        .split_once('/')
        .ok_or("InvalidVfsUri: missing root slash")?;
    if id.is_empty()
        || id.len() > 128
        || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err("InvalidVfsUri: invalid mount ID".into());
    }
    let mut decoded = Vec::new();
    if !relative.is_empty() {
        if relative.split('/').count() > 256 {
            return Err("InvalidVfsUri: too many path segments".into());
        }
        for part in relative.split('/') {
            let bytes = part.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'%' {
                    if i + 2 >= bytes.len()
                        || !bytes[i + 1].is_ascii_hexdigit()
                        || !bytes[i + 2].is_ascii_hexdigit()
                    {
                        return Err("InvalidVfsUri: invalid encoding".into());
                    }
                    i += 2;
                }
                i += 1;
            }
            if part.contains(['?', '#', '\\']) {
                return Err("InvalidVfsUri: unescaped delimiter".into());
            }
            let value = urlencoding::decode(part)
                .map_err(|_| "InvalidVfsUri: invalid UTF-8")?
                .into_owned();
            segment(&value)?;
            decoded.push(value);
        }
    }
    Ok(Uri {
        mount_id: id.into(),
        relative: decoded.join("/"),
    })
}

pub fn uri(id: &str, relative: &str) -> Result<String, String> {
    let mut result = format!("fiex://mount/{id}/");
    if !relative.is_empty() {
        let mut encoded = Vec::new();
        for part in relative.split('/') {
            segment(part)?;
            encoded.push(urlencoding::encode(part).into_owned());
        }
        result.push_str(&encoded.join("/"));
    }
    parse(&result)?;
    Ok(result)
}

pub fn new_id() -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let token = tempfile::Builder::new()
        .rand_bytes(24)
        .tempfile()
        .map_err(|e| e.to_string())?;
    Ok(hex::encode(Sha256::digest(
        token.path().to_string_lossy().as_bytes(),
    )))
}

pub fn reject_local(path: &str) -> Result<(), String> {
    if handles(path) {
        parse(path)?;
        return Err(
            "VFS resource requires Core routing; this local operation is unavailable".into(),
        );
    }
    if path.starts_with("history://") {
        return Err(crate::i18n::t("旧历史地址已失效，请重新选择提交并保存标签页/书签").into());
    }
    Ok(())
}
