//! HTTP surface of TG-203 channel comments. Translates protocol data and calls
//! `chats::discussion`; posting a comment reuses the text send path (`store_message`) and the
//! universal post gate (`resolve_post_topic`), so a comment obeys every rule a group message does.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::discussion::{LinkError, PostThread};
use super::membership_handlers::{require_permission, session_user};
use crate::admin_system_lock::require_chat_unlocked;
use crate::messages::reply_quotes::ReplyExtra;
use crate::models::{ChatMessage, StoredMessage};
use crate::state::{AppState, SharedState};

/// Longest comment accepted, the same ceiling as a chat message.
const MAX_COMMENT_CHARS: usize = 4096;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/chats/:id/discussion", put(put_discussion))
        .route(
            "/api/chats/:id/posts/:message_id/comments",
            get(list_comments).post(post_comment),
        )
}

/// `PUT /api/chats/:id/discussion` — `chat_id: null` unlinks.
#[derive(Debug, Deserialize, ToSchema)]
pub struct PutDiscussionRequest {
    pub chat_id: Option<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DiscussionLink {
    pub channel_id: Uuid,
    pub linked_chat_id: Option<Uuid>,
}

/// One post's comment section.
#[derive(Debug, Serialize, ToSchema)]
pub struct CommentThread {
    pub discussion_chat_id: Uuid,
    /// The post's copy in the group; comments reply to it (or to each other).
    pub discussion_message_id: Uuid,
    pub count: i64,
    /// Whether the caller may comment now (an active member allowed to send there).
    pub can_comment: bool,
    /// Oldest first, at most `discussion::THREAD_LIMIT`.
    pub messages: Vec<StoredMessage>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PostCommentRequest {
    pub content: String,
    /// A comment in the same thread to answer; the post itself when absent.
    pub reply_to: Option<Uuid>,
    pub client_message_id: Option<Uuid>,
}

fn internal(error: sqlx::Error) -> StatusCode {
    tracing::error!("channel discussion query failed: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}

/// Link or unlink a channel's discussion group. Needs `chat.info` in the channel and, to link,
/// in the group as well.
#[utoipa::path(put, path = "/api/chats/{id}/discussion", params(("id" = Uuid, description = "Channel id")),
    request_body = PutDiscussionRequest,
    responses((status = 200, description = "The link now in force", body = DiscussionLink),
        (status = 400, description = "The target is not a group or supergroup"),
        (status = 403, description = "Not an administrator of the channel or of the group"),
        (status = 404, description = "No such channel"),
        (status = 409, description = "The group already serves another channel")))]
pub async fn put_discussion(
    State(state): State<SharedState>,
    Path(channel_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<PutDiscussionRequest>,
) -> Result<Json<DiscussionLink>, StatusCode> {
    let user = session_user(&state, &headers).await?;
    if !state.is_channel(channel_id).await {
        return Err(StatusCode::NOT_FOUND);
    }
    require_chat_unlocked(&state, channel_id).await?;
    require_permission(&state, channel_id, user.id, "chat.info").await?;
    if let Some(group_id) = request.chat_id {
        if !state
            .can_read_chat(group_id, user.id)
            .await
            .map_err(internal)?
        {
            return Err(StatusCode::NOT_FOUND);
        }
        require_permission(&state, group_id, user.id, "chat.info").await?;
    }
    let previous = state
        .chat(channel_id)
        .await
        .and_then(|chat| chat.linked_chat_id);
    match state
        .set_discussion_group(channel_id, request.chat_id)
        .await
        .map_err(internal)?
    {
        Ok(()) => {}
        Err(LinkError::NotAGroup) => return Err(StatusCode::BAD_REQUEST),
        Err(LinkError::AlreadyLinked) => return Err(StatusCode::CONFLICT),
    }
    for id in [Some(channel_id), previous, request.chat_id]
        .into_iter()
        .flatten()
    {
        if let Some(mut chat) = state.chat(id).await {
            chat.membership_status = None;
            chat.membership_role = None;
            state.broadcast(id, ChatMessage::ChatUpdated { chat }).await;
        }
    }
    Ok(Json(DiscussionLink {
        channel_id,
        linked_chat_id: request.chat_id,
    }))
}

/// The channel reader's gate, then the post's thread.
async fn readable_thread(
    state: &SharedState,
    channel_id: Uuid,
    post_id: Uuid,
    user_id: Uuid,
) -> Result<PostThread, StatusCode> {
    if !state.is_channel(channel_id).await
        || !state
            .can_read_chat(channel_id, user_id)
            .await
            .map_err(internal)?
    {
        return Err(StatusCode::NOT_FOUND);
    }
    state
        .post_thread(channel_id, post_id)
        .await
        .map_err(internal)?
        .ok_or(StatusCode::NOT_FOUND)
}

/// A post's comments. Any reader of the channel may read them — joining the discussion group is
/// only needed to write (Telegram). The thread stays readable after the channel is unlinked.
#[utoipa::path(get, path = "/api/chats/{id}/posts/{message_id}/comments",
    params(("id" = Uuid, description = "Channel id"), ("message_id" = Uuid, description = "Post id")),
    responses((status = 200, description = "The comment thread", body = CommentThread),
        (status = 404, description = "No such post with comments, or not a reader of the channel")))]
pub async fn list_comments(
    State(state): State<SharedState>,
    Path((channel_id, post_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<CommentThread>, StatusCode> {
    let user = session_user(&state, &headers).await?;
    let thread = readable_thread(&state, channel_id, post_id, user.id).await?;
    let messages = state
        .thread_messages(thread, user.id)
        .await
        .map_err(internal)?;
    let count = state
        .comment_counts(&[post_id])
        .await
        .map_err(internal)?
        .get(&post_id)
        .copied()
        .unwrap_or(0);
    let can_comment = state
        .has_chat_permission(thread.discussion_chat_id, user.id, "message.send")
        .await
        .map_err(internal)?;
    Ok(Json(CommentThread {
        discussion_chat_id: thread.discussion_chat_id,
        discussion_message_id: thread.discussion_message_id,
        count,
        can_comment,
        messages,
    }))
}

/// Comment on a post: a reply in the discussion group, delivered there like any message.
#[utoipa::path(post, path = "/api/chats/{id}/posts/{message_id}/comments",
    params(("id" = Uuid, description = "Channel id"), ("message_id" = Uuid, description = "Post id")),
    request_body = PostCommentRequest,
    responses((status = 201, description = "The comment", body = StoredMessage),
        (status = 400, description = "Empty or too long, or reply_to is outside this thread"),
        (status = 403, description = "Not allowed to send in the discussion group (join it first), or slow mode"),
        (status = 404, description = "No such post with comments, or not a reader of the channel")))]
pub async fn post_comment(
    State(state): State<SharedState>,
    Path((channel_id, post_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(request): Json<PostCommentRequest>,
) -> Result<(StatusCode, Json<StoredMessage>), StatusCode> {
    let user = session_user(&state, &headers).await?;
    let thread = readable_thread(&state, channel_id, post_id, user.id).await?;
    let content = request.content.trim();
    if content.is_empty() || content.chars().count() > MAX_COMMENT_CHARS {
        return Err(StatusCode::BAD_REQUEST);
    }
    let room_id = thread.discussion_chat_id;
    require_permission(&state, room_id, user.id, "message.send").await?;
    let topic_id = state
        .resolve_post_topic(room_id, user.id, None)
        .await
        .map_err(StatusCode::from)?;
    let reply_to = request.reply_to.unwrap_or(thread.discussion_message_id);
    if !state.in_thread(thread, reply_to).await.map_err(internal)? {
        return Err(StatusCode::BAD_REQUEST);
    }
    let display_name = state.resolve_display_name(room_id, &user).await;
    let stored = state
        .store_message(
            room_id,
            user.id,
            &display_name,
            &user.avatar_emoji,
            content,
            Some(reply_to),
            request.client_message_id,
            &[],
            topic_id,
            false,
            &ReplyExtra::default(),
        )
        .await
        .map_err(internal)?;
    // A new row reaches the group through the live poller; a replayed client id is re-sent.
    if !stored.inserted {
        state
            .broadcast(
                room_id,
                crate::realtime::protocol::stored_message_to_chat(stored.message.clone()),
            )
            .await;
    }
    Ok((StatusCode::CREATED, Json(stored.message)))
}
