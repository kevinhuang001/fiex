use super::*;

pub(super) struct PluginSession {
    owner: String,
    pub(super) input: Arc<Mutex<VecDeque<u8>>>,
    pub(super) size: Arc<Mutex<(u16, u16)>>,
    pub(super) channel: Channel<SshEvent>,
    pub(super) closed: Arc<AtomicBool>,
}

pub(super) fn plugin_sessions() -> &'static Mutex<HashMap<String, PluginSession>> {
    static VALUES: OnceLock<Mutex<HashMap<String, PluginSession>>> = OnceLock::new();
    VALUES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) struct PluginParts {
    input: Arc<Mutex<VecDeque<u8>>>,
    size: Arc<Mutex<(u16, u16)>>,
    closed: Arc<AtomicBool>,
    channel: Channel<SshEvent>,
}

pub(super) fn plugin_session(owner: &str, id: &str) -> Result<PluginParts, String> {
    let values = plugin_sessions()
        .lock()
        .map_err(|_| crate::i18n::t("终端会话表不可用"))?;
    let session = values
        .get(id)
        .filter(|session| session.owner == owner)
        .ok_or(crate::i18n::t("终端会话不存在"))?;
    Ok(PluginParts {
        input: session.input.clone(),
        size: session.size.clone(),
        closed: session.closed.clone(),
        channel: session.channel.clone(),
    })
}

pub(crate) fn plugin_terminal_ready(owner: &str, params: &serde_json::Value) -> Result<(), String> {
    let id = params
        .get("sessionId")
        .and_then(serde_json::Value::as_str)
        .ok_or("missing sessionId")?;
    plugin_session(owner, id)?
        .channel
        .send(SshEvent::Ready)
        .map_err(|error| error.to_string())
}

pub(crate) fn plugin_terminal_output(
    owner: &str,
    params: &serde_json::Value,
) -> Result<(), String> {
    let id = params
        .get("sessionId")
        .and_then(serde_json::Value::as_str)
        .ok_or("missing sessionId")?;
    let data = params
        .get("dataBase64")
        .and_then(serde_json::Value::as_str)
        .ok_or("missing dataBase64")?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|error| error.to_string())?;
    let session = plugin_session(owner, id)?;
    session
        .channel
        .send(SshEvent::Data { data: bytes })
        .map_err(|error| error.to_string())
}

pub(crate) fn plugin_terminal_input(
    owner: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let id = params
        .get("sessionId")
        .and_then(serde_json::Value::as_str)
        .ok_or("missing sessionId")?;
    let max = params
        .get("maxBytes")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(8192)
        .clamp(1, 1024 * 1024) as usize;
    let session = plugin_session(owner, id)?;
    let mut queue = session
        .input
        .lock()
        .map_err(|_| crate::i18n::t("终端输入不可用"))?;
    let size = queue.len().min(max);
    let bytes = queue.drain(..size).collect::<Vec<_>>();
    Ok(serde_json::json!({
        "dataBase64": base64::engine::general_purpose::STANDARD.encode(bytes),
        "eof": session.closed.load(Ordering::Acquire),
    }))
}

pub(crate) fn plugin_terminal_size(
    owner: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let id = params
        .get("sessionId")
        .and_then(serde_json::Value::as_str)
        .ok_or("missing sessionId")?;
    let session = plugin_session(owner, id)?;
    let (cols, rows) = *session
        .size
        .lock()
        .map_err(|_| crate::i18n::t("终端尺寸不可用"))?;
    Ok(serde_json::json!({"cols": cols, "rows": rows}))
}

pub(crate) fn plugin_terminal_closed(
    owner: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let id = params
        .get("sessionId")
        .and_then(serde_json::Value::as_str)
        .ok_or("missing sessionId")?;
    let session = plugin_session(owner, id)?;
    Ok(serde_json::json!({"closed": session.closed.load(Ordering::Acquire)}))
}

pub(crate) fn plugin_terminal_end(owner: &str, params: &serde_json::Value) -> Result<(), String> {
    let id = params
        .get("sessionId")
        .and_then(serde_json::Value::as_str)
        .ok_or("missing sessionId")?;
    let mut values = plugin_sessions()
        .lock()
        .map_err(|_| crate::i18n::t("终端会话表不可用"))?;
    if !values.get(id).is_some_and(|session| session.owner == owner) {
        return Err(crate::i18n::t("终端会话不存在").into());
    }
    let session = values.remove(id).ok_or(crate::i18n::t("终端会话不存在"))?;
    session.closed.store(true, Ordering::Release);
    if let Some(message) = params.get("error").and_then(serde_json::Value::as_str) {
        let _ = session.channel.send(SshEvent::Error {
            message: message.into(),
        });
    }
    let _ = session.channel.send(SshEvent::Exit { code: None });
    Ok(())
}


