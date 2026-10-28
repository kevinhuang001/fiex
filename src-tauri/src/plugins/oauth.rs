//! Host-owned external-browser OAuth with loopback redirect and PKCE S256.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::Url;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::Mutex,
    time::{Duration, Instant},
};

fn endpoint(params: &Value) -> Result<Url, String> {
    let value = params["authorizationEndpoint"]
        .as_str()
        .ok_or("missing authorizationEndpoint")?;
    if value.len() > 2048 {
        return Err("OAuth endpoint too long".into());
    }
    let url = Url::parse(value).map_err(|_| "invalid OAuth endpoint")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "OAuth endpoint must be an HTTPS URL without credentials, query or fragment".into(),
        );
    }
    Ok(url)
}
pub(crate) fn endpoint_host(params: &Value) -> Result<String, String> {
    Ok(endpoint(params)?.host_str().unwrap().to_string())
}
fn random() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| "secure randomness unavailable")?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
fn authorization_url(
    params: &Value,
    redirect: &str,
    state: &str,
    verifier: &str,
) -> Result<Url, String> {
    let mut url = endpoint(params)?;
    let client = params["clientId"]
        .as_str()
        .filter(|v| !v.is_empty() && v.len() <= 1024)
        .ok_or("missing or invalid clientId")?;
    let scopes = params["scopes"]
        .as_array()
        .filter(|v| !v.is_empty() && v.len() <= 32)
        .ok_or("missing or invalid scopes")?;
    let scopes = scopes
        .iter()
        .map(|v| {
            v.as_str()
                .filter(|v| !v.is_empty() && v.len() <= 256 && !v.chars().any(char::is_whitespace))
                .ok_or("invalid OAuth scope")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let mut query = url.query_pairs_mut();
    query
        .append_pair("client_id", client)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect)
        .append_pair("state", state)
        .append_pair("scope", &scopes.join(" "))
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256");
    if let Some(extras) = params.get("parameters") {
        let extras = extras
            .as_object()
            .filter(|v| v.len() <= 16)
            .ok_or("invalid OAuth parameters")?;
        for (key, value) in extras {
            if [
                "client_id",
                "response_type",
                "redirect_uri",
                "state",
                "scope",
                "code_challenge",
                "code_challenge_method",
                "response_mode",
            ]
            .contains(&key.as_str())
            {
                return Err("OAuth parameters cannot override host security fields".into());
            }
            let value = value
                .as_str()
                .filter(|v| v.len() <= 1024)
                .ok_or("invalid OAuth parameter")?;
            if key.is_empty() || key.len() > 80 {
                return Err("invalid OAuth parameter name".into());
            }
            query.append_pair(key, value);
        }
    }
    drop(query);
    Ok(url)
}

pub(crate) fn authorize(params: &Value, active: impl Fn() -> bool) -> Result<Value, String> {
    authorize_with(params, active, |url| {
        open::that(url.as_str()).map_err(|_| "could not open system browser".into())
    })
}
fn authorize_with(
    params: &Value,
    active: impl Fn() -> bool,
    open: impl FnOnce(&Url) -> Result<(), String>,
) -> Result<Value, String> {
    static FLOW: Mutex<()> = Mutex::new(());
    let _guard = FLOW
        .try_lock()
        .map_err(|_| "another browser authorization is already in progress")?;
    let listener =
        TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let redirect = format!(
        "http://{}/oauth/callback",
        listener.local_addr().map_err(|e| e.to_string())?
    );
    let state = random()?;
    let verifier = random()?;
    let url = authorization_url(params, &redirect, &state, &verifier)?;
    if !active() {
        return Err("OAuth operation cancelled".into());
    }
    open(&url)?;
    let timeout = Duration::from_millis(
        params["timeoutMs"]
            .as_u64()
            .unwrap_or(300_000)
            .clamp(1_000, 600_000),
    );
    let started = Instant::now();
    while started.elapsed() < timeout && active() {
        match listener.accept() {
            Ok((mut stream, peer)) => {
                if !peer.ip().is_loopback() {
                    continue;
                }
                match callback(&mut stream, &redirect, &state) {
                    Ok(Some(code)) => {
                        return Ok(
                            json!({"code":code,"redirectUri":redirect,"codeVerifier":verifier}),
                        )
                    }
                    Ok(None) => {}
                    Err(error) => return Err(error),
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50))
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    Err(if active() {
        "OAuth browser authorization timed out"
    } else {
        "OAuth operation cancelled"
    }
    .into())
}
fn callback(stream: &mut TcpStream, redirect: &str, state: &str) -> Result<Option<String>, String> {
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_millis(500)))
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 1024];
    while bytes.len() < 16384 && !bytes.windows(4).any(|w| w == b"\r\n\r\n") {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => bytes.extend_from_slice(&buffer[..n]),
            Err(_) => break,
        }
    }
    let request = String::from_utf8_lossy(&bytes);
    let result = parse_callback(&request, redirect, state);
    let (status, body) = match &result {
        Ok(Some(_)) => (
            "200 OK",
            "Authorization received. You can close this tab and return to Fiex.",
        ),
        Ok(None) => ("400 Bad Request", "Invalid authorization callback."),
        Err(_) => (
            "400 Bad Request",
            "Authorization was not completed. Return to Fiex.",
        ),
    };
    let response = format!("HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; frame-ancestors 'none'\r\nConnection: close\r\n\r\n{body}", body.len());
    let _ = stream.write_all(response.as_bytes());
    result
}
fn parse_callback(request: &str, redirect: &str, state: &str) -> Result<Option<String>, String> {
    if request.len() >= 16384 || !request.contains("\r\n\r\n") {
        return Ok(None);
    }
    let expected = Url::parse(redirect).map_err(|_| "invalid redirect")?;
    let parts = request
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace()
        .collect::<Vec<_>>();
    if parts.len() != 3 || parts[0] != "GET" || !parts[1].starts_with('/') {
        return Ok(None);
    }
    let host = request
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("host"))
        .map(|(_, value)| value.trim());
    let authority = format!(
        "{}:{}",
        expected.host_str().unwrap_or(""),
        expected.port().unwrap_or(80)
    );
    if host != Some(authority.as_str()) {
        return Ok(None);
    }
    let url = expected
        .join(parts[1])
        .map_err(|_| "invalid callback URL")?;
    if url.origin() != expected.origin() || url.path() != expected.path() {
        return Ok(None);
    }
    let mut params = BTreeMap::new();
    for (key, value) in url.query_pairs() {
        if params
            .insert(key.into_owned(), value.into_owned())
            .is_some()
        {
            return Ok(None);
        }
    }
    if params.get("state").map(String::as_str) != Some(state) {
        return Ok(None);
    }
    if params.contains_key("error") {
        return Err("OAuth authorization was denied by the user or provider".into());
    }
    Ok(params
        .remove("code")
        .filter(|code| !code.is_empty() && code.len() <= 8192))
}
#[cfg(test)]
mod tests;
