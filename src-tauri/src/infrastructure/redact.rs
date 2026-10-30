//! Shared, linear-time scrubber for every diagnostic sink, including historical records.
use regex::Regex;
use std::sync::OnceLock;
fn patterns() -> &'static Vec<Regex> {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| [
        // Complete header values, including folded continuation lines.
        r#"(?im)((?:authorization|proxy-authorization|cookie|set-cookie|x-[\w-]*(?:key|token|secret))\s*[:=]\s*)[^\r\n]*(?:\r?\n[ \t]+[^\r\n]*)*"#,
        // JSON, configuration assignments and URL queries. Quoted values can contain whitespace.
        r#"(?i)((?:[\w-]*(?:token|secret|password|passwd|credential)[\w-]*|api[_-]?key|key|code)\s*["']?\s*[:=]\s*)(?:"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|[^\s&,;}\]"']+)"#,
        r#"(?i)(\b(?:bearer|basic)\s+)[a-z0-9+/=_\-.]+"#,
    ].into_iter().map(|p| Regex::new(p).expect("static redaction expression")).collect())
}
pub fn clean(value: &str, limit: usize) -> String {
    // Reject oversized untrusted diagnostics as a whole; never expose an unscanned suffix.
    if value.len() > 256 * 1024 {
        return "[diagnostic omitted: size limit]".into();
    }
    let mut value = value.to_owned();
    for pattern in patterns() {
        value = pattern.replace_all(&value, "${1}[REDACTED]").into_owned();
    }
    value
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .take(limit)
        .collect::<String>()
        .trim()
        .into()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scrubs_headers_json_queries_and_multiline_credentials_before_truncation() {
        for input in [
            "Authorization: Bearer HIDDEN\n continuationHIDDEN",
            "Cookie: session=HIDDEN; user=HIDDEN",
            "Set-Cookie: auth=HIDDEN; HttpOnly",
            "X-Api-Key: HIDDEN",
            "Basic HIDDEN",
            r#"{"password": "HIDDEN with spaces", "refresh_token":
"HIDDEN"}"#,
            "https://service/?code=HIDDEN&token=HIDDEN&key=HIDDEN",
            "client_secret = HIDDEN",
            "authorization=HIDDEN",
        ] {
            assert!(!clean(input, 4000).contains("HIDDEN"), "{input}");
        }
    }
}
