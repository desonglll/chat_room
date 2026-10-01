//! TG-803: the chat info panel's shared-content tabs — `/files?kind=media|document|voice|gif`
//! and the lazily built link index behind `/links` — on SQLite and on PostgreSQL.

mod privacy_support;
mod voice_support;

use chat_room::{config::AppConfig, state::AppState};
use futures_util::SinkExt;
use privacy_support::{
    migration_support::{create_postgres_scratch, drop_postgres_scratch, postgres_admin_pool},
    next_json, start_server, start_server_on, Account, TestServer,
};
use reqwest::{multipart, StatusCode};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;
use voice_support::{send_voice, VoiceForm};

async fn upload(server: &TestServer, account: &Account, chat: Uuid, name: &str, mime: &str) {
    let part = multipart::Part::bytes(b"tg803 payload".to_vec())
        .file_name(name.to_string())
        .mime_str(mime)
        .unwrap();
    let status = server
        .client
        .post(server.url(&format!("/api/chats/{chat}/attachments")))
        .bearer_auth(&account.token)
        .multipart(multipart::Form::new().part("file", part))
        .send()
        .await
        .unwrap()
        .status();
    assert!(status.is_success(), "upload {name}: {status}");
}

async fn get(server: &TestServer, account: &Account, path: &str) -> (StatusCode, Value) {
    let response = server
        .client
        .get(server.url(path))
        .bearer_auth(&account.token)
        .send()
        .await
        .unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

async fn file_names(server: &TestServer, account: &Account, chat: Uuid, kind: &str) -> Vec<String> {
    let (status, page) = get(
        server,
        account,
        &format!("/api/chats/{chat}/files?kind={kind}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{kind}: {page}");
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            item["attachment"]["file_name"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect()
}

async fn file_tabs_are_classified_on_the_server(server: &TestServer) {
    let owner = server.account("sc-owner").await;
    let stranger = server.account("sc-stranger").await;
    let chat = server.create_group(&owner, "sc-files").await;
    upload(server, &owner, chat, "photo.png", "image/png").await;
    upload(server, &owner, chat, "clip.mp4", "video/mp4").await;
    upload(server, &owner, chat, "dance.gif", "image/gif").await;
    upload(server, &owner, chat, "report.pdf", "application/pdf").await;
    upload(server, &owner, chat, "song.mp3", "audio/mpeg").await;
    let (status, body) = send_voice(server, chat, &owner, VoiceForm::webm(1_000, 31)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    assert_eq!(
        file_names(server, &owner, chat, "media").await,
        ["clip.mp4", "photo.png"]
    );
    assert_eq!(
        file_names(server, &owner, chat, "document").await,
        ["report.pdf"]
    );
    assert_eq!(
        file_names(server, &owner, chat, "music").await,
        ["song.mp3"]
    );
    assert_eq!(file_names(server, &owner, chat, "voice").await.len(), 1);
    assert_eq!(file_names(server, &owner, chat, "gif").await, ["dance.gif"]);
    assert_eq!(file_names(server, &owner, chat, "all").await.len(), 6);

    let (status, _) = get(
        server,
        &owner,
        &format!("/api/chats/{chat}/files?kind=nonsense"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = get(
        server,
        &stranger,
        &format!("/api/chats/{chat}/files?kind=media"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "non-members see no tab");
}

async fn frame(socket: &mut privacy_support::Socket, value: Value, wait_for: &str) {
    socket.send(Message::Text(value.to_string())).await.unwrap();
    loop {
        if next_json(socket).await["type"] == wait_for {
            return;
        }
    }
}

async fn links(server: &TestServer, account: &Account, chat: Uuid, query: &str) -> Value {
    let (status, page) = get(server, account, &format!("/api/chats/{chat}/links{query}")).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    page
}

fn urls(page: &Value) -> Vec<String> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["url"].as_str().unwrap().to_string())
        .collect()
}

async fn the_link_tab_backfills_follows_edits_and_pages(server: &TestServer) {
    let owner = server.account("sl-owner").await;
    let member = server.account("sl-member").await;
    let stranger = server.account("sl-stranger").await;
    let chat = server.create_group(&owner, "sc-links").await;
    server.join(chat, &member).await;

    // History that exists before the index does: the first read backfills it.
    server
        .send_message(chat, &owner, "first https://one.example/a")
        .await;
    server.send_message(chat, &member, "no link here").await;
    let edited = server
        .send_message(
            chat,
            &owner,
            "two links https://two.example/ and http://three.example/x.",
        )
        .await;
    let recalled = server
        .send_message(chat, &member, "gone https://gone.example/")
        .await;

    let page = links(server, &member, chat, "").await;
    assert_eq!(
        urls(&page),
        [
            "https://gone.example/",
            "https://two.example/",
            "http://three.example/x",
            "https://one.example/a"
        ]
    );
    assert_eq!(page["items"][1]["sender"], "sl-owner");

    // A recall hides the row; an edit replaces the message's links; new messages appear.
    let (mut socket, _) = server.open_chat(chat, &member).await;
    frame(
        &mut socket,
        json!({ "type": "recall", "message_id": recalled }),
        "message_recalled",
    )
    .await;
    server.close_and_settle(chat, &member, socket).await;
    let (mut socket, _) = server.open_chat(chat, &owner).await;
    frame(
        &mut socket,
        json!({ "type": "edit", "message_id": edited, "content": "now https://four.example/" }),
        "message_edited",
    )
    .await;
    server.close_and_settle(chat, &owner, socket).await;
    server
        .send_message(chat, &member, "latest https://five.example/")
        .await;

    let first = links(server, &member, chat, "?limit=2").await;
    assert_eq!(
        urls(&first),
        ["https://five.example/", "https://four.example/"]
    );
    let cursor = first["next"].as_str().unwrap().to_string();
    let second = links(server, &member, chat, &format!("?limit=2&before={cursor}")).await;
    assert_eq!(urls(&second), ["https://one.example/a"]);
    assert!(second.get("next").is_none(), "{second}");

    let (status, _) = get(server, &stranger, &format!("/api/chats/{chat}/links")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = get(
        server,
        &member,
        &format!("/api/chats/{chat}/links?before=nonsense"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let empty = server.create_group(&owner, "sc-empty").await;
    assert!(urls(&links(server, &owner, empty, "").await).is_empty());
}

async fn run_all(server: &TestServer) {
    file_tabs_are_classified_on_the_server(server).await;
    the_link_tab_backfills_follows_edits_and_pages(server).await;
}

#[tokio::test]
async fn sqlite_shared_content_tabs() {
    let server = start_server().await;
    run_all(&server).await;
}

#[tokio::test]
async fn postgres_shared_content_tabs() {
    let Some((admin_url, admin_pool)) = postgres_admin_pool("postgres_shared_content_tabs").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "shared_content").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    {
        let server = start_server_on(state).await;
        run_all(&server).await;
        server.state.postgres_pool().unwrap().close().await;
    }
    drop_postgres_scratch(&admin_pool, &scratch).await;
}
