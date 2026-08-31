//! State and event contracts for the terminal client.

use std::{collections::HashMap, path::PathBuf};

use uuid::Uuid;

use crate::{
    client_api::{
        AiRun, AiThread, AiThreadMessage, ApiResult, AuthSession, Conversation, Favorite,
        Notification, NotificationPage, PreferencePatch, RoomSummary, SearchPage, SearchResult,
    },
    client_auth::UserConfig,
    client_chat::{ChatCommand, ChatEvent, ChatMessage, ChatSender},
    client_media::Attachment,
};

use super::input::TextField;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Screen {
    SignIn,
    Main,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthMode {
    Login,
    Register,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum View {
    Chats,
    Search,
    Notifications,
    Favorites,
    Ai,
}

impl View {
    pub const ALL: [Self; 5] = [
        Self::Chats,
        Self::Search,
        Self::Notifications,
        Self::Favorites,
        Self::Ai,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Chats => "Chats",
            Self::Search => "Search",
            Self::Notifications => "Notifications",
            Self::Favorites => "Favorites",
            Self::Ai => "AI",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|candidate| *candidate == self)
            .unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Focus {
    List,
    Content,
    Input,
}

#[derive(Clone, Debug)]
pub enum PromptKind {
    RoomPassword {
        room_id: Uuid,
        target_message: Option<Uuid>,
    },
    RoomJoinPassword(Uuid),
    Upload,
    Download(Attachment),
    EditMessage(Uuid),
    Reaction(Uuid),
}

#[derive(Clone, Debug)]
pub enum ConfirmKind {
    RecallMessage(Uuid),
    DeleteFavorite(Uuid),
}

#[derive(Clone, Debug)]
pub enum Dialog {
    Help,
    Prompt {
        title: String,
        kind: PromptKind,
        input: TextField,
    },
    CreateRoom {
        name: TextField,
        password: TextField,
        field: usize,
    },
    Rooms {
        items: Vec<RoomSummary>,
        selected: usize,
    },
    FavoriteEditor {
        id: Option<Uuid>,
        version: i64,
        title: TextField,
        content: TextField,
        field: usize,
    },
    Confirm {
        title: String,
        kind: ConfirmKind,
    },
}

#[derive(Clone, Debug)]
pub enum Action {
    ValidateSession,
    Authenticate {
        register: bool,
        username: String,
        password: String,
    },
    Logout,
    LoadConversations,
    LoadRooms,
    CreateRoom {
        name: String,
        password: String,
    },
    JoinRoom {
        room_id: Uuid,
        password: Option<String>,
    },
    ConnectRoom {
        room_id: Uuid,
        password: Option<String>,
        target_message: Option<Uuid>,
    },
    Chat(ChatCommand),
    Upload {
        room_id: Uuid,
        password: Option<String>,
        path: PathBuf,
    },
    Download {
        attachment: Attachment,
        path: PathBuf,
    },
    UpdatePreferences {
        room_id: Uuid,
        patch: PreferencePatch,
    },
    Search(String),
    LoadNotifications,
    ReadNotification(String),
    ReadAllNotifications,
    LoadFavorites,
    SaveFavorite {
        id: Option<Uuid>,
        version: i64,
        title: String,
        content: String,
    },
    FavoriteMessage(Uuid),
    DeleteFavorite(Uuid),
    LoadAiThreads,
    LoadAiMessages(Uuid),
    AskAi {
        thread_id: Option<Uuid>,
        question: String,
        room_id: Option<Uuid>,
        room_password: Option<String>,
    },
    Quit,
}

pub enum AppEvent {
    SessionValidated(ApiResult<()>),
    Authenticated(ApiResult<AuthSession>),
    LoggedOut(ApiResult<()>),
    Conversations(ApiResult<Vec<Conversation>>),
    Rooms(ApiResult<Vec<RoomSummary>>),
    RoomCreated {
        password: Option<String>,
        result: ApiResult<RoomSummary>,
    },
    RoomJoined {
        room_id: Uuid,
        password: Option<String>,
        result: ApiResult<crate::client_api::RoomMembership>,
    },
    ChatConnected {
        room_id: Uuid,
        target_message: Option<Uuid>,
        result: Result<(String, ChatSender), String>,
    },
    Chat {
        room_id: Uuid,
        event: ChatEvent,
    },
    Uploaded(Result<Attachment, String>),
    Downloaded(Result<PathBuf, String>),
    PreferencesUpdated(ApiResult<crate::client_api::ConversationPreferences>),
    Search(ApiResult<SearchPage>),
    Notifications(ApiResult<NotificationPage>),
    NotificationRead(ApiResult<()>),
    Favorites(ApiResult<Vec<Favorite>>),
    FavoriteSaved(ApiResult<Favorite>),
    MessageFavorited(ApiResult<Vec<Favorite>>),
    FavoriteDeleted(ApiResult<()>),
    AiThreads(ApiResult<Vec<AiThread>>),
    AiMessages {
        thread_id: Uuid,
        result: ApiResult<Vec<AiThreadMessage>>,
    },
    AiRunStarted(ApiResult<(AiThread, AiRun)>),
    AiRunPolled(ApiResult<AiRun>),
}

pub struct App {
    pub server: String,
    pub screen: Screen,
    pub auth_mode: AuthMode,
    pub auth_username: TextField,
    pub auth_password: TextField,
    pub auth_field: usize,
    pub username: String,
    pub token: Option<Uuid>,
    pub view: View,
    pub focus: Focus,
    pub conversations: Vec<Conversation>,
    pub conversation_index: usize,
    pub active_room: Option<Uuid>,
    pub active_room_name: String,
    pub room_passwords: HashMap<Uuid, String>,
    pub messages: Vec<ChatMessage>,
    pub message_index: usize,
    pub chat: Option<ChatSender>,
    pub compose: TextField,
    pub reply_to: Option<Uuid>,
    pub pending_message: Option<Uuid>,
    pub typing_user: Option<String>,
    pub search_input: TextField,
    pub search_results: Vec<SearchResult>,
    pub search_index: usize,
    pub notifications: Vec<Notification>,
    pub notification_index: usize,
    pub favorites: Vec<Favorite>,
    pub favorite_index: usize,
    pub ai_threads: Vec<AiThread>,
    pub ai_thread_index: usize,
    pub ai_messages: Vec<AiThreadMessage>,
    pub ai_message_index: usize,
    pub ai_input: TextField,
    pub ai_running: bool,
    pub dialog: Option<Dialog>,
    pub status: String,
    pub busy: bool,
    pub initial_room: Option<(Uuid, Option<String>)>,
}

impl App {
    pub fn new(
        server: String,
        config: UserConfig,
        initial_room: Option<(Uuid, Option<String>)>,
    ) -> Self {
        let signed_in = !config.username.is_empty() && config.token.is_some();
        Self {
            server,
            screen: if signed_in {
                Screen::Main
            } else {
                Screen::SignIn
            },
            auth_mode: AuthMode::Login,
            auth_username: TextField::new(config.username.clone()),
            auth_password: TextField::password(),
            auth_field: 0,
            username: config.username,
            token: config.token,
            view: View::Chats,
            focus: Focus::List,
            conversations: Vec::new(),
            conversation_index: 0,
            active_room: None,
            active_room_name: String::new(),
            room_passwords: HashMap::new(),
            messages: Vec::new(),
            message_index: 0,
            chat: None,
            compose: TextField::default(),
            reply_to: None,
            pending_message: None,
            typing_user: None,
            search_input: TextField::default(),
            search_results: Vec::new(),
            search_index: 0,
            notifications: Vec::new(),
            notification_index: 0,
            favorites: Vec::new(),
            favorite_index: 0,
            ai_threads: Vec::new(),
            ai_thread_index: 0,
            ai_messages: Vec::new(),
            ai_message_index: 0,
            ai_input: TextField::default(),
            ai_running: false,
            dialog: None,
            status: if signed_in {
                "Validating saved session...".into()
            } else {
                "Sign in to continue".into()
            },
            busy: signed_in,
            initial_room,
        }
    }

    pub fn startup_actions(&self) -> Vec<Action> {
        self.token
            .map(|_| vec![Action::ValidateSession])
            .unwrap_or_default()
    }

    pub fn selected_conversation(&self) -> Option<&Conversation> {
        self.conversations.get(self.conversation_index)
    }

    pub fn selected_message(&self) -> Option<&ChatMessage> {
        self.messages.get(self.message_index)
    }

    pub fn selected_favorite(&self) -> Option<&Favorite> {
        self.favorites.get(self.favorite_index)
    }

    pub fn selected_ai_thread(&self) -> Option<&AiThread> {
        self.ai_threads.get(self.ai_thread_index)
    }
}
