//! Best-effort system proxy discovery for the plugin marketplace.
//!
//! reqwest's `system-proxy` feature already handles Windows/macOS, but we also
//! expose a small status object and cover Linux environment proxies explicitly.
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyInfo {
    pub enabled: bool,
    pub source: String,
    pub proxy: Option<String>,
}

fn mask(value: &str) -> String {
    let Ok(mut url) = reqwest::Url::parse(value) else {
        return value.to_string();
    };
    if !url.username().is_empty() {
        let _ = url.set_username("***");
    }
    if url.password().is_some() {
        let _ = url.set_password(Some("***"));
    }
    url.to_string()
}

fn url(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let candidate = if value.contains("://") {
        value.to_string()
    } else {
        format!("http://{value}")
    };
    reqwest::Url::parse(&candidate).ok()?;
    Some(candidate)
}

#[cfg(windows)]
fn detect() -> Option<(String, String)> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")
        .ok()?;
    let enabled: u32 = key.get_value("ProxyEnable").unwrap_or(0);
    if enabled == 0 {
        return None;
    }
    let server: String = key.get_value("ProxyServer").ok()?;
    let server = server.trim();
    if server.is_empty() {
        return None;
    }
    let chosen = if server.contains('=') {
        server
            .split(';')
            .find_map(|entry| {
                let (kind, value) = entry.split_once('=')?;
                matches!(kind.trim(), "https" | "http" | "socks")
                    .then_some((kind.trim(), value.trim()))
            })
            .map(|(kind, value)| {
                if kind == "socks" {
                    format!("socks5://{value}")
                } else {
                    value.to_string()
                }
            })
    } else {
        Some(server.to_string())
    }?;
    url(&chosen).map(|proxy| ("windows-system".into(), proxy))
}

#[cfg(target_os = "macos")]
fn detect() -> Option<(String, String)> {
    let output = std::process::Command::new("scutil")
        .arg("--proxy")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let value = |key: &str| {
        text.lines()
            .find_map(|line| line.trim().strip_prefix(key))
            .and_then(|value| value.trim().strip_prefix(':'))
            .map(str::trim)
            .filter(|value| !value.is_empty())
    };
    let enabled = |key: &str| value(key) == Some("1");
    let proxy = if enabled("HTTPSEnable") {
        format!("http://{}:{}", value("HTTPSProxy")?, value("HTTPSPort")?)
    } else if enabled("HTTPEnable") {
        format!("http://{}:{}", value("HTTPProxy")?, value("HTTPPort")?)
    } else if enabled("SOCKSEnable") {
        format!("socks5://{}:{}", value("SOCKSProxy")?, value("SOCKSPort")?)
    } else {
        return None;
    };
    url(&proxy).map(|proxy| ("macos-system".into(), proxy))
}

#[cfg(not(any(windows, target_os = "macos")))]
fn detect() -> Option<(String, String)> {
    for key in [
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
        "ALL_PROXY",
        "all_proxy",
    ] {
        if let Ok(value) = std::env::var(key) {
            if let Some(proxy) = url(&value) {
                return Some(("environment".into(), proxy));
            }
        }
    }
    None
}

pub fn proxy_url() -> Option<String> {
    detect().map(|(_, proxy)| proxy)
}

pub fn info() -> ProxyInfo {
    match detect() {
        Some((source, proxy)) => ProxyInfo {
            enabled: true,
            source,
            proxy: Some(mask(&proxy)),
        },
        None => ProxyInfo {
            enabled: false,
            source: if cfg!(windows) {
                "windows-system".into()
            } else if cfg!(target_os = "macos") {
                "macos-system".into()
            } else {
                "environment".into()
            },
            proxy: None,
        },
    }
}
