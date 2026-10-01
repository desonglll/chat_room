//! TG-408 link previews: the SSRF cases from the task card (`127.0.0.1`, `169.254.169.254`,
//! `file://`, DNS rebinding, redirect into the internal network) are all refused, the fetch is
//! bounded (redirect count, content type), and end to end a send is never held up by its card,
//! the card arrives afterwards, and the sender can hide it.

mod poll_support;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::{http::header, response::IntoResponse, routing::get, Router};
use chat_room::config::AppConfig;
use chat_room::messages::link_previews::fetch::{fetch_preview, FetchError};
use chat_room::messages::link_previews::ssrf::{Policy, Refusal};
use chat_room::state::AppState;
use poll_support::{call, create_chat, history, next_type, open_socket, register, serve};
use reqwest::{Method, StatusCode, Url};
use serde_json::json;
use tokio::net::TcpListener;

const PAGE: &str = r#"<html><head><title>Fixture</title>
<meta property="og:title" content="Fixture page"><meta property="og:description" content="About it">
</head><body>hello</body></html>"#;

async fn fixture() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let port = address.port();
    let app = Router::new()
        .route(
            "/page",
            get(|| async { ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], PAGE) }),
        )
        .route(
            "/json",
            get(|| async { ([(header::CONTENT_TYPE, "application/json")], "{}") }),
        )
        .route(
            "/to-metadata",
            get(|| async {
                (
                    StatusCode::FOUND,
                    [(header::LOCATION, "http://169.254.169.254/latest/meta-data")],
                )
                    .into_response()
            }),
        )
        .route(
            "/to-loopback",
            get(|| async {
                (
                    StatusCode::FOUND,
                    [(header::LOCATION, "http://127.0.0.1/admin")],
                )
                    .into_response()
            }),
        )
        .route(
            "/slow",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(3)).await;
                ([(header::CONTENT_TYPE, "text/html")], PAGE)
            }),
        )
        .route(
            "/to-rebind",
            get(move || async move {
                (
                    StatusCode::FOUND,
                    [(header::LOCATION, format!("http://rebind.test:{port}/page"))],
                )
                    .into_response()
            }),
        )
        .route(
            "/loop",
            get(|| async { (StatusCode::FOUND, [(header::LOCATION, "/loop")]).into_response() }),
        );
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    address
}

fn exempting(address: SocketAddr) -> Policy {
    let mut policy = Policy::default();
    policy.ports.push(address.port());
    policy.exempt.push(address);
    policy
}

async fn no_dns(_: String, _: u16) -> std::io::Result<Vec<SocketAddr>> {
    panic!("a refused URL must not even be resolved")
}

#[tokio::test]
async fn the_task_card_ssrf_cases_are_refused() {
    let strict = Policy::default();
    for (url, expected) in [
        ("http://127.0.0.1/", "private"),
        ("http://169.254.169.254/latest/meta-data/", "private"),
        ("http://[::ffff:127.0.0.1]/", "private"),
        ("http://2130706433/", "private"),
        ("file:///etc/passwd", "scheme"),
        ("gopher://example.com/", "scheme"),
        ("http://example.com:6379/", "port"),
    ] {
        let result = fetch_preview(Url::parse(url).unwrap(), &strict, no_dns).await;
        match expected {
            "private" => assert!(
                matches!(result, Err(FetchError::Refused(Refusal::PrivateAddress(_)))),
                "{url}: {result:?}"
            ),
            "scheme" => assert_eq!(result, Err(FetchError::Refused(Refusal::Scheme)), "{url}"),
            _ => assert_eq!(result, Err(FetchError::Refused(Refusal::Port)), "{url}"),
        }
    }
    // A name whose answers include any internal address is refused before connecting.
    let mixed = |_: String, port: u16| async move {
        Ok(vec![
            SocketAddr::new("93.184.216.34".parse().unwrap(), port),
            SocketAddr::new("10.0.0.5".parse().unwrap(), port),
        ])
    };
    let result = fetch_preview(Url::parse("http://mixed.test/").unwrap(), &strict, mixed).await;
    assert!(
        matches!(result, Err(FetchError::Refused(Refusal::PrivateAddress(_)))),
        "{result:?}"
    );
}

#[tokio::test]
async fn redirects_are_rechecked_hop_by_hop_and_dns_is_pinned() {
    let server = fixture().await;
    let policy = exempting(server);
    let base = |path: &str| Url::parse(&format!("http://{server}{path}")).unwrap();
    let local = move |_: String, _: u16| async move { Ok(vec![server]) };

    let preview = fetch_preview(base("/page"), &policy, local)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(preview.title, "Fixture page");
    assert_eq!(preview.description, "About it");

    for path in ["/to-metadata", "/to-loopback"] {
        let result = fetch_preview(base(path), &policy, local).await;
        assert!(
            matches!(result, Err(FetchError::Refused(Refusal::PrivateAddress(_)))),
            "{path}: {result:?}"
        );
    }
    assert_eq!(
        fetch_preview(base("/loop"), &policy, local).await,
        Err(FetchError::TooManyRedirects)
    );
    assert_eq!(
        fetch_preview(base("/json"), &policy, local).await,
        Err(FetchError::NotHtml)
    );

    // DNS rebinding: `rebind.test` answers the fixture first, then the metadata address. The
    // first hop connects to the checked answer only; the redirect's fresh lookup is checked
    // again and refused. The resolver is consulted exactly once per hop.
    let calls = Arc::new(AtomicUsize::new(0));
    let rebinding = {
        let calls = calls.clone();
        move |_: String, port: u16| {
            let call = calls.fetch_add(1, Ordering::SeqCst);
            async move {
                Ok(vec![if call == 0 {
                    server
                } else {
                    SocketAddr::new("169.254.169.254".parse().unwrap(), port)
                }])
            }
        }
    };
    let start = Url::parse(&format!("http://rebind.test:{}/to-rebind", server.port())).unwrap();
    let result = fetch_preview(start, &policy, rebinding).await;
    assert!(
        matches!(result, Err(FetchError::Refused(Refusal::PrivateAddress(_)))),
        "{result:?}"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_send_is_never_held_up_and_the_card_follows_and_can_be_hidden() {
    let fixture = fixture().await;
    let mut config = AppConfig::default();
    config.link_preview.unsafe_allow_sockets = vec![fixture.to_string()];
    let server = serve(Arc::new(AppState::new_with_config(&config).await.unwrap())).await;
    let base = &server.base;
    let alice = register(base, "lp-alice").await;
    let chat = create_chat(base, &alice, "links").await;
    let mut socket = open_socket(base, chat, &alice).await;

    // A slow site: the message is delivered at once, without a card.
    socket_send(&mut socket, &format!("slow http://{fixture}/slow")).await;
    let delivered =
        tokio::time::timeout(Duration::from_secs(1), next_type(&mut socket, "broadcast"))
            .await
            .expect("the send must not wait for the preview");
    assert!(delivered.get("link_preview").is_none(), "{delivered}");

    // A quick site: the card arrives (embedded by the live poller, or as its own frame).
    socket_send(&mut socket, &format!("look http://{fixture}/page.")).await;
    let mut message_id = String::new();
    let mut titled = false;
    while !titled {
        let frame = tokio::time::timeout(Duration::from_secs(10), next_any(&mut socket))
            .await
            .expect("card");
        if frame["type"] == "broadcast"
            && frame["content"].as_str().unwrap_or("").starts_with("look")
        {
            message_id = frame["message_id"].as_str().unwrap().to_string();
            titled = frame["link_preview"]["title"] == "Fixture page";
        } else if frame["type"] == "link_preview_updated"
            && frame["message_id"] == message_id.as_str()
        {
            titled = frame["preview"]["title"] == "Fixture page";
        }
    }
    let stored = history(base, &alice, chat).await;
    let card = stored
        .iter()
        .find(|message| message["id"] == message_id.as_str())
        .unwrap();
    assert_eq!(card["link_preview"]["title"], "Fixture page");

    // An internal link sends normally and simply gets no card.
    socket_send(&mut socket, "metadata http://169.254.169.254/latest").await;
    loop {
        let frame = next_type(&mut socket, "broadcast").await;
        if frame["content"]
            .as_str()
            .unwrap_or("")
            .starts_with("metadata")
        {
            assert!(frame.get("link_preview").is_none());
            break;
        }
    }

    let (status, _) = call(
        Method::DELETE,
        format!("{base}/api/chats/{chat}/messages/{message_id}/link-preview"),
        &alice.token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let hidden = next_type(&mut socket, "link_preview_updated").await;
    assert!(hidden["preview"].is_null());
    let after = history(base, &alice, chat).await;
    let card = after
        .iter()
        .find(|message| message["id"] == message_id.as_str())
        .unwrap();
    assert!(card.get("link_preview").is_none(), "{card}");

    // The composer endpoint answers the same way, and refuses internal targets.
    let (status, preview) = call(
        Method::GET,
        format!("{base}/api/link-preview?url=http://{fixture}/page"),
        &alice.token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    let (status, _) = call(
        Method::GET,
        format!("{base}/api/link-preview?url=http://127.0.0.1:9/"),
        &alice.token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

async fn next_any(socket: &mut poll_support::Socket) -> serde_json::Value {
    use futures_util::StreamExt;
    loop {
        if let Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) = socket.next().await {
            return serde_json::from_str(&text).unwrap();
        }
    }
}

async fn socket_send(socket: &mut poll_support::Socket, content: &str) {
    use futures_util::SinkExt;
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            json!({ "type": "message", "content": content }).to_string(),
        ))
        .await
        .unwrap();
}
