//! Background execution of TG-907 social actions (contacts, pins, forwarding).

use tokio::sync::mpsc;

use crate::client_api::ApiClient;
use uuid::Uuid;

use super::model::AppEvent;
use super::social::{ContactsData, SocialAction, SocialEvent};

pub fn run(
    action: SocialAction,
    server: String,
    token: Option<Uuid>,
    sender: mpsc::UnboundedSender<AppEvent>,
) {
    tokio::spawn(async move {
        let api = ApiClient::new(&server, token);
        let event = match action {
            SocialAction::LoadContacts => SocialEvent::Contacts(load_contacts(&api).await),
            SocialAction::Respond { user_id, accept } => {
                SocialEvent::ContactsChanged(api.respond_request(user_id, accept).await.map(|()| {
                    if accept {
                        "Request accepted"
                    } else {
                        "Request declined"
                    }
                    .to_string()
                }))
            }
            SocialAction::AddContact(username) => {
                SocialEvent::ContactsChanged(api.add_contact(&username).await)
            }
            SocialAction::OpenDirectChat(user_id) => {
                SocialEvent::DirectChatOpened(api.open_direct_chat(user_id).await)
            }
            SocialAction::LoadPins(room_id) => SocialEvent::Pins {
                room_id,
                result: api.pins(room_id).await,
            },
            SocialAction::SetPinned {
                room_id,
                message_id,
                pinned,
            } => SocialEvent::PinChanged {
                room_id,
                pinned,
                result: api.set_pinned(room_id, message_id, pinned).await,
            },
            SocialAction::Forward {
                message_id,
                target,
                title,
            } => SocialEvent::Forwarded {
                title,
                result: api.forward(message_id, target).await,
            },
            SocialAction::Vote {
                message_id,
                options,
            } => SocialEvent::Voted {
                message_id,
                result: api.vote(message_id, &options).await,
            },
            SocialAction::LoadFolders => SocialEvent::Folders(api.folders().await),
        };
        let _ = sender.send(AppEvent::Social(event));
    });
}

async fn load_contacts(api: &ApiClient) -> crate::client_api::ApiResult<ContactsData> {
    let friends = api.friends().await?;
    let requests = api.incoming_requests().await?;
    // Presence is a nicety: a server without `/api/friends/statuses` still lists contacts.
    let statuses = api
        .friend_statuses()
        .await
        .map(|entries| {
            entries
                .into_iter()
                .map(|entry| (entry.user_id, entry.status))
                .collect()
        })
        .unwrap_or_default();
    Ok(ContactsData {
        friends,
        statuses,
        requests,
    })
}
