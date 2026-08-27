use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::User;

#[derive(Debug)]
pub enum AdminRoleError {
    Database(sqlx::Error),
    UserNotFound,
    AdministratorNotFound,
    Forbidden,
    LastAdministrator,
    BootstrapUnavailable,
}

impl From<sqlx::Error> for AdminRoleError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemAdminView {
    pub user: User,
    pub granted_by: Option<Uuid>,
    pub grant_source: String,
    pub created_at: DateTime<Utc>,
}

#[derive(FromRow)]
pub(super) struct SystemAdminRow {
    pub(super) id: Uuid,
    pub(super) username: String,
    pub(super) avatar_emoji: String,
    pub(super) display_name: String,
    pub(super) signature: String,
    pub(super) homepage: String,
    pub(super) user_created_at: DateTime<Utc>,
    pub(super) granted_by: Option<Uuid>,
    pub(super) grant_source: String,
    pub(super) admin_created_at: DateTime<Utc>,
}

impl SystemAdminRow {
    pub(super) fn view(self) -> SystemAdminView {
        SystemAdminView {
            user: User {
                id: self.id,
                username: self.username,
                avatar_emoji: self.avatar_emoji,
                display_name: self.display_name,
                signature: self.signature,
                homepage: self.homepage,
                created_at: self.user_created_at,
            },
            granted_by: self.granted_by,
            grant_source: self.grant_source,
            created_at: self.admin_created_at,
        }
    }
}
