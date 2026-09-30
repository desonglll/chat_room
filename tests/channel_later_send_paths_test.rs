//! TG-202 × TG-404: the scheduled-send path, which landed after the channel post gate, obeys
//! it too — a subscriber can neither schedule nor have a scheduled post delivered, while an
//! administrator holding `message.post` can. (Voice and GIF sends reach the same decision
//! through `has_chat_permission`, which maps `message.send` to `message.post` in a channel.)

mod channel_support;
mod poll_support;
mod scheduled_support;

use channel_support::*;
use reqwest::StatusCode;
use serde_json::json;

#[tokio::test]
async fn only_post_holders_schedule_in_a_channel() {
    let server = serve_memory().await;
    let base = &server.base;
    let owner = register(base, "ch-sched-owner").await;
    let bob = register(base, "ch-sched-bob").await;
    let editor = register(base, "ch-sched-editor").await;
    let channel = id_of(&create_channel(base, &owner, "scheduled posts", false).await);
    for account in [&bob, &editor] {
        assert_eq!(subscribe(base, account, channel).await.0, StatusCode::OK);
    }
    appoint(base, &owner, channel, &editor, &["message.post"]).await;

    let body = json!({ "content": "later", "scheduled_at": scheduled_support::in_seconds(3600) });
    let (status, response) = scheduled_support::schedule(base, &bob, channel, body.clone()).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "subscriber scheduled: {response}"
    );

    let (status, response) = scheduled_support::schedule(base, &editor, channel, body).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "post holder refused: {response}"
    );
}
