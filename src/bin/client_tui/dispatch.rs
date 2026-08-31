//! Background execution of TUI actions.

use std::future::Future;

use tokio::sync::mpsc;

use crate::{client_api::ApiClient, client_chat, client_media};

use super::model::{Action, App, AppEvent};

pub fn action(app: &mut App, action: Action, sender: mpsc::UnboundedSender<AppEvent>) {
    if let Action::Chat(command) = action {
        if let Some(chat) = &app.chat {
            if chat.send(command).is_err() {
                app.status = "Chat connection is closed".into();
            }
        } else {
            app.status = "Open a conversation first".into();
        }
        return;
    }
    let server = app.server.clone();
    let token = app.token;
    match action {
        Action::ValidateSession => spawn(sender, async move {
            AppEvent::SessionValidated(ApiClient::new(&server, token).validate_session().await)
        }),
        Action::Authenticate {
            register,
            username,
            password,
        } => spawn(sender, async move {
            AppEvent::Authenticated(
                ApiClient::new(&server, None)
                    .authenticate(register, &username, &password)
                    .await,
            )
        }),
        Action::Logout => spawn(sender, async move {
            AppEvent::LoggedOut(ApiClient::new(&server, token).logout().await)
        }),
        Action::LoadConversations => spawn(sender, async move {
            AppEvent::Conversations(ApiClient::new(&server, token).conversations().await)
        }),
        Action::LoadRooms => spawn(sender, async move {
            AppEvent::Rooms(ApiClient::new(&server, token).discover_rooms().await)
        }),
        Action::CreateRoom { name, password } => {
            let saved_password = (!password.is_empty()).then_some(password.clone());
            spawn(sender, async move {
                AppEvent::RoomCreated {
                    password: saved_password,
                    result: ApiClient::new(&server, token)
                        .create_room(&name, Some(&password))
                        .await,
                }
            });
        }
        Action::JoinRoom { room_id, password } => {
            let request_password = password.clone();
            spawn(sender, async move {
                AppEvent::RoomJoined {
                    room_id,
                    password,
                    result: ApiClient::new(&server, token)
                        .join_room(room_id, request_password.as_deref())
                        .await,
                }
            });
        }
        Action::ConnectRoom {
            room_id,
            password,
            target_message,
        } => {
            let Some(token) = token else {
                app.status = "Sign in before opening a conversation".into();
                return;
            };
            tokio::spawn(async move {
                match client_chat::connect(&server, room_id, token, password.as_deref()).await {
                    Ok(mut connection) => {
                        if sender
                            .send(AppEvent::ChatConnected {
                                room_id,
                                target_message,
                                result: Ok((connection.room_name, connection.sender)),
                            })
                            .is_err()
                        {
                            return;
                        }
                        while let Some(event) = connection.events.recv().await {
                            if sender.send(AppEvent::Chat { room_id, event }).is_err() {
                                break;
                            }
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(AppEvent::ChatConnected {
                            room_id,
                            target_message,
                            result: Err(error.to_string()),
                        });
                    }
                }
            });
        }
        Action::Upload {
            room_id,
            password,
            path,
        } => {
            let Some(token) = token else {
                app.status = "Sign in before uploading".into();
                return;
            };
            spawn(sender, async move {
                AppEvent::Uploaded(
                    client_media::upload(&server, room_id, token, password.as_deref(), &path)
                        .await
                        .map_err(|error| error.to_string()),
                )
            });
        }
        Action::Download { attachment, path } => spawn(sender, async move {
            AppEvent::Downloaded(
                client_media::download(&server, &attachment, Some(&path))
                    .await
                    .map_err(|error| error.to_string()),
            )
        }),
        Action::UpdatePreferences { room_id, patch } => spawn(sender, async move {
            AppEvent::PreferencesUpdated(
                ApiClient::new(&server, token)
                    .update_preferences(room_id, patch)
                    .await,
            )
        }),
        Action::Search(query) => spawn(sender, async move {
            AppEvent::Search(ApiClient::new(&server, token).search(&query).await)
        }),
        Action::LoadNotifications => spawn(sender, async move {
            AppEvent::Notifications(ApiClient::new(&server, token).notifications().await)
        }),
        Action::ReadNotification(id) => spawn(sender, async move {
            AppEvent::NotificationRead(
                ApiClient::new(&server, token)
                    .mark_notification_read(&id)
                    .await,
            )
        }),
        Action::ReadAllNotifications => spawn(sender, async move {
            AppEvent::NotificationRead(
                ApiClient::new(&server, token)
                    .mark_all_notifications_read()
                    .await,
            )
        }),
        Action::LoadFavorites => spawn(sender, async move {
            AppEvent::Favorites(ApiClient::new(&server, token).favorites().await)
        }),
        Action::SaveFavorite {
            id,
            version,
            title,
            content,
        } => spawn(sender, async move {
            let api = ApiClient::new(&server, token);
            let result = match id {
                Some(id) => api.update_favorite(id, version, &title, &content).await,
                None => api.create_favorite(&title, &content).await,
            };
            AppEvent::FavoriteSaved(result)
        }),
        Action::FavoriteMessage(message_id) => spawn(sender, async move {
            AppEvent::MessageFavorited(
                ApiClient::new(&server, token)
                    .favorite_message(message_id)
                    .await,
            )
        }),
        Action::DeleteFavorite(id) => spawn(sender, async move {
            AppEvent::FavoriteDeleted(ApiClient::new(&server, token).delete_favorite(id).await)
        }),
        Action::LoadAiThreads => spawn(sender, async move {
            AppEvent::AiThreads(ApiClient::new(&server, token).ai_threads().await)
        }),
        Action::LoadAiMessages(thread_id) => spawn(sender, async move {
            AppEvent::AiMessages {
                thread_id,
                result: ApiClient::new(&server, token).ai_messages(thread_id).await,
            }
        }),
        Action::AskAi {
            thread_id,
            question,
            room_id,
            room_password,
        } => spawn_ai(
            sender,
            ApiClient::new(&server, token),
            thread_id,
            question,
            room_id,
            room_password,
        ),
        Action::Chat(_) | Action::Quit => unreachable!("handled before dispatch"),
    }
}

fn spawn<F>(sender: mpsc::UnboundedSender<AppEvent>, future: F)
where
    F: Future<Output = AppEvent> + Send + 'static,
{
    tokio::spawn(async move {
        let event = future.await;
        let _ = sender.send(event);
    });
}

fn spawn_ai(
    sender: mpsc::UnboundedSender<AppEvent>,
    api: ApiClient,
    thread_id: Option<uuid::Uuid>,
    question: String,
    room_id: Option<uuid::Uuid>,
    room_password: Option<String>,
) {
    tokio::spawn(async move {
        let result = async {
            let thread = match thread_id {
                Some(id) => match api
                    .ai_threads()
                    .await?
                    .into_iter()
                    .find(|thread| thread.id == id)
                {
                    Some(thread) => thread,
                    None => api.create_ai_thread(room_id).await?,
                },
                None => api.create_ai_thread(room_id).await?,
            };
            let run = api
                .create_ai_run(thread.id, &question, room_id, room_password.as_deref())
                .await?;
            Ok((thread, run))
        }
        .await;
        let Ok((thread, run)) = &result else {
            let _ = sender.send(AppEvent::AiRunStarted(result));
            return;
        };
        let thread_id = thread.id;
        let run_id = run.id;
        if sender.send(AppEvent::AiRunStarted(result)).is_err() {
            return;
        }
        let _ = sender.send(AppEvent::AiMessages {
            thread_id,
            result: api.ai_messages(thread_id).await,
        });
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(750)).await;
            let result = api.ai_run(run_id).await;
            let terminal = result
                .as_ref()
                .is_ok_and(|run| matches!(run.status.as_str(), "completed" | "failed"));
            if sender.send(AppEvent::AiRunPolled(result)).is_err() {
                return;
            }
            if terminal {
                let _ = sender.send(AppEvent::AiMessages {
                    thread_id,
                    result: api.ai_messages(thread_id).await,
                });
                return;
            }
        }
    });
}
