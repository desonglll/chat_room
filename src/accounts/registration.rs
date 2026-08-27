//! Registration policy, one-time invitation consumption, and HTTP entry point.

use axum::{extract::State, http::StatusCode, Json};
use chrono::Utc;
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    admin::system_admins::token_hash,
    models::{AuthSession, User},
    state::{with_pool, AppState, SharedState},
};

use super::user_handlers::{hash_password, normalize_credentials};

#[derive(Debug, Deserialize, ToSchema)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub invite_token: Option<String>,
}

#[derive(Debug)]
pub enum RegistrationError {
    Database(sqlx::Error),
    Disabled,
    InvitationRequired,
}

impl From<sqlx::Error> for RegistrationError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

impl AppState {
    pub async fn register_user(
        &self,
        username: &str,
        password_hash: &str,
        invite_token: Option<&str>,
    ) -> Result<User, RegistrationError> {
        let invitation_hash = match self.registration_mode() {
            "open" => None,
            "disabled" => return Err(RegistrationError::Disabled),
            "invite_only" => Some(token_hash(
                invite_token
                    .filter(|token| !token.trim().is_empty())
                    .ok_or(RegistrationError::InvitationRequired)?,
            )),
            _ => return Err(RegistrationError::Disabled),
        };
        let user = User {
            id: Uuid::new_v4(),
            username: username.to_string(),
            avatar_emoji: String::new(),
            display_name: String::new(),
            signature: String::new(),
            homepage: String::new(),
            created_at: Utc::now(),
        };
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            sqlx::query(
                "INSERT INTO users (id, username, password_hash, avatar_emoji, created_at) \
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(user.id)
            .bind(&user.username)
            .bind(password_hash)
            .bind(&user.avatar_emoji)
            .bind(user.created_at)
            .execute(&mut *transaction)
            .await?;
            if let Some(hash) = &invitation_hash {
                let consumed = sqlx::query(
                    "UPDATE registration_invites SET used_by = $1, used_at = $2 \
                     WHERE token_hash = $3 AND used_at IS NULL AND expires_at > $2",
                )
                .bind(user.id)
                .bind(user.created_at)
                .bind(hash)
                .execute(&mut *transaction)
                .await?
                .rows_affected();
                if consumed == 0 {
                    return Err(RegistrationError::InvitationRequired);
                }
            }
            transaction.commit().await?;
            Ok::<_, RegistrationError>(())
        })?;
        Ok(user)
    }
}

/// Register an account and immediately issue a login session.
#[utoipa::path(
    post,
    path = "/api/users/register",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "Account registered", body = AuthSession),
        (status = 400, description = "Invalid username or password"),
        (status = 403, description = "Registration disabled or invitation invalid"),
        (status = 409, description = "Username already exists")
    )
)]
pub async fn register(
    State(state): State<SharedState>,
    Json(request): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<AuthSession>), StatusCode> {
    let (username, password) = normalize_credentials(request.username, request.password)?;
    let password_hash = hash_password(password).await?;
    let user = match state
        .register_user(&username, &password_hash, request.invite_token.as_deref())
        .await
    {
        Ok(user) => user,
        Err(RegistrationError::Database(sqlx::Error::Database(error)))
            if error.is_unique_violation() =>
        {
            return Err(StatusCode::CONFLICT)
        }
        Err(RegistrationError::Disabled | RegistrationError::InvitationRequired) => {
            return Err(StatusCode::FORBIDDEN)
        }
        Err(RegistrationError::Database(error)) => {
            tracing::error!("register user failed: {error}");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    state
        .create_session(user)
        .await
        .map(|session| (StatusCode::CREATED, Json(session)))
        .map_err(|error| {
            tracing::error!("create registration session failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}
