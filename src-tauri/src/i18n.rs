//! Application locale. Plugin RPC captures this value independently per request.
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU8, Ordering},
        LazyLock,
    },
};
static LOCALE: AtomicU8 = AtomicU8::new(0);
static EN: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../src/i18n/messages/en.json")).expect("English catalog")
});
static TRADITIONAL: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../src/i18n/messages/zh-TW.json"))
        .expect("Traditional Chinese catalog")
});
pub fn locale() -> &'static str {
    match LOCALE.load(Ordering::Relaxed) {
        1 => "zh-TW",
        2 => "en",
        _ => "zh-CN",
    }
}
pub fn set_locale(value: &str) -> Result<(), String> {
    let value = match value {
        "zh-CN" => 0,
        "zh-TW" => 1,
        "en" => 2,
        _ => return Err("unsupported locale".into()),
    };
    LOCALE.store(value, Ordering::Relaxed);
    Ok(())
}
pub fn t(source: &'static str) -> &'static str {
    match locale() {
        "en" => EN.get(source),
        "zh-TW" => TRADITIONAL.get(source),
        _ => None,
    }
    .map(String::as_str)
    .unwrap_or(source)
}

pub fn text(source: &str) -> String {
    match locale() {
        "en" => EN.get(source),
        "zh-TW" => TRADITIONAL.get(source),
        _ => None,
    }
    .map(String::as_str)
    .unwrap_or(source)
    .to_string()
}

/// Localize the template before interpolating untrusted filenames or server text.
pub fn format(source: &'static str, values: &[(&str, String)]) -> String {
    fiex_plugin_sdk::i18n::format(t(source), values)
}
