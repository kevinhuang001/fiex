use super::*;
fn params() -> Value {
    json!({"authorizationEndpoint":"https://accounts.google.com/o/oauth2/v2/auth","clientId":"client","scopes":["https://www.googleapis.com/auth/drive"],"timeoutMs":1000})
}
#[test]
fn security_fields_cannot_be_overridden_and_pkce_is_s256() {
    let mut p = params();
    let url = authorization_url(
        &p,
        "http://127.0.0.1:1234/oauth/callback",
        "state",
        "verifier",
    )
    .unwrap();
    let query = url.query_pairs().collect::<BTreeMap<_, _>>();
    assert_eq!(query["code_challenge_method"], "S256");
    assert_eq!(
        query["code_challenge"],
        URL_SAFE_NO_PAD.encode(Sha256::digest(b"verifier"))
    );
    p["parameters"] = json!({"state":"attacker"});
    assert!(authorization_url(&p, "redirect", "state", "verifier").is_err());
    p["authorizationEndpoint"] = json!("http://accounts.google.com/auth");
    assert!(endpoint_host(&p).is_err());
}
#[test]
fn callbacks_require_exact_state_host_path_and_unique_parameters() {
    let redirect = "http://127.0.0.1:1234/oauth/callback";
    let request =
        |target: &str, host: &str| format!("GET {target} HTTP/1.1\r\nHost: {host}\r\n\r\n");
    assert_eq!(
        parse_callback(
            &request("/oauth/callback?state=s&code=ok", "127.0.0.1:1234"),
            redirect,
            "s"
        )
        .unwrap(),
        Some("ok".into())
    );
    for target in [
        "/oauth/callback?state=wrong&code=ok",
        "/other?state=s&code=ok",
        "/oauth/callback?state=s&state=s&code=ok",
        "//evil.test/oauth/callback?state=s&code=ok",
    ] {
        assert!(
            parse_callback(&request(target, "127.0.0.1:1234"), redirect, "s")
                .unwrap()
                .is_none()
        );
    }
    assert!(parse_callback(
        &request("/oauth/callback?state=s&code=ok", "evil.test"),
        redirect,
        "s"
    )
    .unwrap()
    .is_none());
    assert!(parse_callback(
        &request(
            "/oauth/callback?state=s&error=access_denied",
            "127.0.0.1:1234"
        ),
        redirect,
        "s"
    )
    .is_err());
}
#[test]
fn real_loopback_flow_returns_code_and_closes_listener() {
    let reply = authorize_with(
        &params(),
        || true,
        |url| {
            let query = url.query_pairs().collect::<BTreeMap<_, _>>();
            let redirect = Url::parse(&query["redirect_uri"]).unwrap();
            let state = query["state"].clone().into_owned();
            std::thread::spawn(move || {
                let authority = format!("127.0.0.1:{}", redirect.port().unwrap());
                let mut stream = TcpStream::connect(&authority).unwrap();
                write!(
                    stream,
                    "GET {}?code=one-use&state={} HTTP/1.1\r\nHost: {}\r\n\r\n",
                    redirect.path(),
                    state,
                    authority
                )
                .unwrap();
                let mut response = String::new();
                stream.read_to_string(&mut response).unwrap();
                assert!(response.contains("200 OK"));
                assert!(!response.contains("one-use"));
            });
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(reply["code"], "one-use");
    assert_eq!(reply["codeVerifier"].as_str().unwrap().len(), 43);
    let url = Url::parse(reply["redirectUri"].as_str().unwrap()).unwrap();
    // The accept loop exits on its own thread; poll briefly instead of
    // assuming the socket is already gone at this exact instant.
    let port = url.port().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if TcpStream::connect(("127.0.0.1", port)).is_err() {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "loopback listener stayed open after the flow"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
