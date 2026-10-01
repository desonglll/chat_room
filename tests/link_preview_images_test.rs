//! TG-1209: a link card's image is fetched by the server under the TG-408 SSRF policy and
//! served from this origin under an unguessable key — the third-party URL never reaches a
//! client — and the CSP admits exactly the configured map tile host, nothing else.

mod poll_support;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::{http::header, response::IntoResponse, routing::get, Router};
use chat_room::config::AppConfig;
use chat_room::messages::link_previews::fetch::{fetch_image, FetchError, MAX_IMAGE_BYTES};
use chat_room::messages::link_previews::ssrf::{Policy, Refusal};
use chat_room::state::AppState;
use poll_support::{call, create_chat, history, open_socket, register, serve};
use reqwest::{Method, StatusCode, Url};
use serde_json::{json, Value};
use tokio::net::TcpListener;

/// A real 1×1 PNG.
const PNG: [u8; 69] = [
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0,
    0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 0, 0, 3, 1,
    1, 0, 201, 254, 146, 239, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#;

async fn fixture() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let page = format!(
        r#"<html><head><meta property="og:title" content="With a cover">
<meta property="og:image" content="http://{address}/cover.png"></head></html>"#
    );
    let app = Router::new()
        .route(
            "/page",
            get(move || {
                let page = page.clone();
                async move { ([(header::CONTENT_TYPE, "text/html")], page) }
            }),
        )
        .route(
            "/cover.png",
            get(|| async { ([(header::CONTENT_TYPE, "image/png")], PNG.to_vec()) }),
        )
        // Claims PNG, is SVG: the bytes decide, and SVG is never served from this origin.
        .route(
            "/fake.png",
            get(|| async { ([(header::CONTENT_TYPE, "image/png")], SVG) }),
        )
        .route(
            "/big.png",
            get(|| async {
                let mut body = PNG.to_vec();
                body.resize(MAX_IMAGE_BYTES + 1, 0);
                ([(header::CONTENT_TYPE, "image/png")], body)
            }),
        )
        .route(
            "/to-metadata.png",
            get(|| async {
                (
                    StatusCode::FOUND,
                    [(header::LOCATION, "http://169.254.169.254/latest/meta-data")],
                )
                    .into_response()
            }),
        );
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    address
}

#[tokio::test]
async fn card_images_are_checked_like_pages_and_judged_by_their_bytes() {
    let server = fixture().await;
    let mut policy = Policy::default();
    policy.ports.push(server.port());
    policy.exempt.push(server);
    let local = move |_: String, _: u16| async move { Ok(vec![server]) };
    let url = |path: &str| Url::parse(&format!("http://{server}{path}")).unwrap();

    let image = fetch_image(url("/cover.png"), &policy, local)
        .await
        .unwrap();
    assert_eq!(image.content_type, "image/png");
    assert_eq!(image.bytes, PNG);

    assert_eq!(
        fetch_image(url("/fake.png"), &policy, local)
            .await
            .unwrap_err(),
        FetchError::NotImage
    );
    assert_eq!(
        fetch_image(url("/big.png"), &policy, local)
            .await
            .unwrap_err(),
        FetchError::TooLarge
    );
    assert!(matches!(
        fetch_image(url("/to-metadata.png"), &policy, local).await,
        Err(FetchError::Refused(Refusal::PrivateAddress(_)))
    ));
    // Without the fixture exemption, the image host is internal and refused before any socket.
    assert!(matches!(
        fetch_image(url("/cover.png"), &Policy::default(), local).await,
        Err(FetchError::Refused(_))
    ));
}

#[tokio::test]
async fn the_card_carries_a_same_origin_image_and_never_the_third_party_url() {
    let fixture = fixture().await;
    let mut config = AppConfig::default();
    config.link_preview.unsafe_allow_sockets = vec![fixture.to_string()];
    let server = serve(Arc::new(AppState::new_with_config(&config).await.unwrap())).await;
    let base = &server.base;
    let alice = register(base, "lpi-alice").await;
    let chat = create_chat(base, &alice, "covers").await;

    // The composer's preview.
    let (status, preview) = call(
        Method::GET,
        format!("{base}/api/link-preview?url=http://{fixture}/page"),
        &alice.token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    let image_url = preview["image_url"]
        .as_str()
        .expect("the card has an image");
    assert!(
        image_url.starts_with("/api/link-previews/images/"),
        "the browser must load the image from this origin: {image_url}"
    );
    assert!(!preview.to_string().contains("cover.png"), "{preview}");

    // The image itself: the stored bytes, typed by their content, loadable by an `<img>`
    // (no session header — the key is the capability, like an attachment's).
    let response = reqwest::get(format!("{base}{image_url}")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
    assert_eq!(response.bytes().await.unwrap().as_ref(), PNG);
    let unknown = reqwest::get(format!(
        "{base}/api/link-previews/images/{}",
        uuid::Uuid::new_v4()
    ))
    .await
    .unwrap();
    assert_eq!(unknown.status(), StatusCode::NOT_FOUND);

    // A message with the link: its card (live and in history) points at the same image.
    let mut socket = open_socket(base, chat, &alice).await;
    {
        use futures_util::SinkExt;
        socket
            .send(tokio_tungstenite::tungstenite::Message::Text(
                json!({ "type": "message", "content": format!("cover http://{fixture}/page") })
                    .to_string(),
            ))
            .await
            .unwrap();
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let card = loop {
        assert!(tokio::time::Instant::now() < deadline, "no card in history");
        let messages = history(base, &alice, chat).await;
        if let Some(card) = messages
            .iter()
            .find_map(|message| message.get("link_preview").filter(|card| card.is_object()))
        {
            break card.clone();
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    assert_eq!(card["image_url"].as_str(), Some(image_url), "{card}");
    let everything: Value = history(base, &alice, chat).await.into();
    assert!(
        !everything.to_string().contains("cover.png"),
        "{everything}"
    );
}

#[tokio::test]
async fn the_csp_admits_the_configured_tile_host_and_no_other_image_host() {
    let mut config = AppConfig::default();
    config.map.tile_url = "https://{s}.tiles.example.org/{z}/{x}/{y}.png".into();
    let server = serve(Arc::new(AppState::new_with_config(&config).await.unwrap())).await;
    let response = reqwest::get(format!("{}/api/config", server.base))
        .await
        .unwrap();
    let csp = response.headers()["content-security-policy"]
        .to_str()
        .unwrap()
        .to_string();
    let img_src = csp
        .split(';')
        .map(str::trim)
        .find(|directive| directive.starts_with("img-src"))
        .unwrap();
    assert_eq!(
        img_src,
        "img-src 'self' data: blob: https://*.tiles.example.org"
    );
    assert!(csp.contains("frame-ancestors 'none'"), "{csp}");
}
