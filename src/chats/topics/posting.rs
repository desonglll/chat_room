//! The one rule every send path applies before writing a message into a forum:
//! which topic it lands in, and whether the sender may post there.

use uuid::Uuid;

use super::model::TopicError;
use super::store::TopicRow;
use crate::message_store::MessagePlacement;
use crate::state::AppState;

impl AppState {
    /// A topic administrator: holds `chat.topics` **and** is the owner or an administrator.
    /// A member who holds `chat.topics` through the default permissions may create topics,
    /// but neither manages other people's topics nor posts into closed ones (Telegram).
    pub async fn is_topic_admin(&self, room_id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error> {
        let role = self.membership_identity(room_id, user_id).await?;
        let Some((status, role)) = role else {
            return Ok(false);
        };
        if status != "active" || !matches!(role.as_str(), "owner" | "admin") {
            return Ok(false);
        }
        self.has_chat_permission(room_id, user_id, "chat.topics")
            .await
    }

    /// The topic a message lands in, checked for the sender. `requested` absent (or General's
    /// id) means General. Answers the value to store in `messages.topic_id` (`None` =
    /// General / not a forum).
    ///
    /// - chat is not a forum: only General is valid (`Invalid` otherwise);
    /// - unknown topic, or another chat's: `NotFound`;
    /// - closed topic (General included) and the sender is not a topic admin: `Closed`.
    pub async fn resolve_post_topic(
        &self,
        room_id: Uuid,
        sender_id: Uuid,
        requested: Option<Uuid>,
    ) -> Result<Option<Uuid>, TopicError> {
        let Some(is_forum) = self.chat_is_forum(room_id).await? else {
            return Err(TopicError::NotFound);
        };
        if !is_forum {
            return match requested {
                None => Ok(None),
                Some(_) => Err(TopicError::Invalid("the chat is not a forum")),
            };
        }
        let topic = self.post_target(room_id, requested).await?;
        if topic.closed_at.is_some() && !self.is_topic_admin(room_id, sender_id).await? {
            return Err(TopicError::Closed);
        }
        Ok(topic.stored_id())
    }

    /// [`Self::resolve_post_topic`] packaged with the reply target, for the attachment paths.
    pub async fn placement(
        &self,
        room_id: Uuid,
        sender_id: Uuid,
        reply_to: Option<Uuid>,
        requested_topic: Option<Uuid>,
    ) -> Result<MessagePlacement, TopicError> {
        let topic_id = self
            .resolve_post_topic(room_id, sender_id, requested_topic)
            .await?;
        Ok(MessagePlacement { reply_to, topic_id })
    }

    async fn post_target(
        &self,
        room_id: Uuid,
        requested: Option<Uuid>,
    ) -> Result<TopicRow, TopicError> {
        let general = self.ensure_general_topic(room_id).await?;
        match requested {
            None => Ok(general),
            Some(id) if id == general.id => Ok(general),
            Some(id) => self
                .load_topic(room_id, id)
                .await?
                .ok_or(TopicError::NotFound),
        }
    }
}
