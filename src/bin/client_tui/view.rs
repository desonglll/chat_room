//! The main screen's tabs.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum View {
    Chats,
    Search,
    Notifications,
    Favorites,
    /// TG-907: friends (presence, open private chat) and incoming friend requests.
    Contacts,
    Ai,
}

impl View {
    pub const ALL: [Self; 6] = [
        Self::Chats,
        Self::Search,
        Self::Notifications,
        Self::Favorites,
        Self::Contacts,
        Self::Ai,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Chats => "Chats",
            Self::Search => "Search",
            Self::Notifications => "Notifications",
            Self::Favorites => "Favorites",
            Self::Contacts => "Contacts",
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
