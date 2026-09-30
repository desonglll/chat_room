//! Persistence of privacy rules and exceptions, and the single-pair decision query.

use std::collections::HashSet;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{decide, ExceptionEffect, PrivacyFacts, PrivacyKey, PrivacyTier};
use crate::models::UserSummary;
use crate::state::{with_pool, AppState};

/// Upper bound on exceptions per dimension (allow + deny together). Telegram caps its lists
/// in the same order of magnitude; the bound keeps one PUT a bounded transaction.
pub const MAX_PRIVACY_EXCEPTIONS: usize = 1000;

/// One dimension as the settings page shows it.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PrivacyRuleView {
    pub key: PrivacyKey,
    pub tier: PrivacyTier,
    pub allow_users: Vec<UserSummary>,
    pub deny_users: Vec<UserSummary>,
}

/// `GET /api/users/me/privacy`: every dimension, always in [`PrivacyKey::ALL`] order.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PrivacySettings {
    pub rules: Vec<PrivacyRuleView>,
}

/// `PUT /api/users/me/privacy/:key` body. The lists replace the stored exceptions.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct PrivacyRuleWrite {
    pub tier: PrivacyTier,
    #[serde(default)]
    pub allow_user_ids: Vec<Uuid>,
    #[serde(default)]
    pub deny_user_ids: Vec<Uuid>,
}

/// Why a write was refused.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PrivacyWriteError {
    TooManyExceptions,
    UnknownUser,
}

impl PrivacyRuleWrite {
    /// Deduplicated `(target, effect)` pairs. The owner is dropped (a rule never applies to
    /// its owner) and an account named in both lists is denied: when the intent is
    /// ambiguous, the private answer wins.
    pub(crate) fn exceptions(&self, owner: Uuid) -> Vec<(Uuid, ExceptionEffect)> {
        let denied: HashSet<Uuid> = self.deny_user_ids.iter().copied().collect();
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        for (ids, effect) in [
            (&self.deny_user_ids, ExceptionEffect::Deny),
            (&self.allow_user_ids, ExceptionEffect::Allow),
        ] {
            for &id in ids {
                if id == owner || (effect == ExceptionEffect::Allow && denied.contains(&id)) {
                    continue;
                }
                if seen.insert(id) {
                    result.push((id, effect));
                }
            }
        }
        result
    }
}

impl AppState {
    /// The inputs of [`decide`] for one (owner, dimension, viewer) triple in one round trip.
    pub(crate) async fn privacy_facts(
        &self,
        owner: Uuid,
        key: PrivacyKey,
        viewer: Uuid,
    ) -> Result<PrivacyFacts, sqlx::Error> {
        if owner == viewer {
            return Ok(PrivacyFacts {
                is_self: true,
                ..PrivacyFacts::default()
            });
        }
        let (tier, effect, are_contacts, owner_blocked_viewer): (
            Option<String>,
            Option<String>,
            bool,
            bool,
        ) = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT \
                   (SELECT tier FROM user_privacy_rules \
                     WHERE user_id = $1 AND privacy_key = $2), \
                   (SELECT effect FROM user_privacy_exceptions \
                     WHERE user_id = $1 AND privacy_key = $2 AND target_user_id = $3), \
                   EXISTS(SELECT 1 FROM friendships WHERE status = 'accepted' AND \
                     ((user_low_id = $1 AND user_high_id = $3) OR \
                      (user_low_id = $3 AND user_high_id = $1))), \
                   EXISTS(SELECT 1 FROM user_blocks WHERE blocker_id = $1 AND blocked_id = $3)",
            )
            .bind(owner)
            .bind(key.as_str())
            .bind(viewer)
            .fetch_one(pool)
            .await
        })?;
        Ok(PrivacyFacts {
            is_self: false,
            owner_blocked_viewer,
            tier: tier.as_deref().map(PrivacyTier::parse).unwrap_or_default(),
            exception: effect.as_deref().and_then(ExceptionEffect::parse),
            are_contacts,
        })
    }

    /// Whether `owner`'s `key` rule admits `viewer`.
    pub async fn privacy_allows(
        &self,
        owner: Uuid,
        key: PrivacyKey,
        viewer: Uuid,
    ) -> Result<bool, sqlx::Error> {
        Ok(decide(self.privacy_facts(owner, key, viewer).await?))
    }

    pub(crate) async fn privacy_tier(
        &self,
        owner: Uuid,
        key: PrivacyKey,
    ) -> Result<PrivacyTier, sqlx::Error> {
        let tier: Option<String> = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT tier FROM user_privacy_rules WHERE user_id = $1 AND privacy_key = $2",
            )
            .bind(owner)
            .bind(key.as_str())
            .fetch_optional(pool)
            .await
        })?;
        Ok(tier.as_deref().map(PrivacyTier::parse).unwrap_or_default())
    }

    pub async fn privacy_settings(&self, owner: Uuid) -> Result<PrivacySettings, sqlx::Error> {
        let mut rules = Vec::with_capacity(PrivacyKey::ALL.len());
        for key in PrivacyKey::ALL {
            rules.push(self.privacy_rule(owner, key).await?);
        }
        Ok(PrivacySettings { rules })
    }

    pub async fn privacy_rule(
        &self,
        owner: Uuid,
        key: PrivacyKey,
    ) -> Result<PrivacyRuleView, sqlx::Error> {
        let tier = self.privacy_tier(owner, key).await?;
        let rows: Vec<(String, Uuid, String, String, String)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT exceptions.effect, users.id, users.username, users.avatar_emoji, \
                   users.display_name \
                 FROM user_privacy_exceptions AS exceptions \
                 JOIN users ON users.id = exceptions.target_user_id \
                 WHERE exceptions.user_id = $1 AND exceptions.privacy_key = $2 \
                 ORDER BY LOWER(users.username), users.id",
            )
            .bind(owner)
            .bind(key.as_str())
            .fetch_all(pool)
            .await
        })?;
        let mut view = PrivacyRuleView {
            key,
            tier,
            allow_users: Vec::new(),
            deny_users: Vec::new(),
        };
        for (effect, id, username, avatar_emoji, display_name) in rows {
            let summary = UserSummary {
                id,
                username,
                avatar_emoji,
                display_name,
            };
            match ExceptionEffect::parse(&effect) {
                Some(ExceptionEffect::Allow) => view.allow_users.push(summary),
                Some(ExceptionEffect::Deny) => view.deny_users.push(summary),
                None => {}
            }
        }
        Ok(view)
    }

    /// Replace one dimension's tier and exceptions atomically. An unknown target account
    /// rolls the whole write back, so a rule is never half-applied.
    pub(crate) async fn save_privacy_rule(
        &self,
        owner: Uuid,
        key: PrivacyKey,
        write: &PrivacyRuleWrite,
    ) -> Result<Result<PrivacyRuleView, PrivacyWriteError>, sqlx::Error> {
        let exceptions = write.exceptions(owner);
        if exceptions.len() > MAX_PRIVACY_EXCEPTIONS {
            return Ok(Err(PrivacyWriteError::TooManyExceptions));
        }
        let now = Utc::now();
        let stored = with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            sqlx::query(
                "INSERT INTO user_privacy_rules (user_id, privacy_key, tier, updated_at) \
                 VALUES ($1, $2, $3, $4) ON CONFLICT (user_id, privacy_key) DO UPDATE SET \
                 tier = excluded.tier, updated_at = excluded.updated_at",
            )
            .bind(owner)
            .bind(key.as_str())
            .bind(write.tier.as_str())
            .bind(now)
            .execute(&mut *transaction)
            .await?;
            sqlx::query(
                "DELETE FROM user_privacy_exceptions WHERE user_id = $1 AND privacy_key = $2",
            )
            .bind(owner)
            .bind(key.as_str())
            .execute(&mut *transaction)
            .await?;
            let mut all_known = true;
            for (target, effect) in &exceptions {
                let inserted = sqlx::query(
                    "INSERT INTO user_privacy_exceptions \
                     (user_id, privacy_key, target_user_id, effect, created_at) \
                     SELECT $1, $2, users.id, $4, $5 FROM users WHERE users.id = $3",
                )
                .bind(owner)
                .bind(key.as_str())
                .bind(*target)
                .bind(effect.as_str())
                .bind(now)
                .execute(&mut *transaction)
                .await?
                .rows_affected();
                if inserted == 0 {
                    all_known = false;
                    break;
                }
            }
            if all_known {
                transaction.commit().await?;
            } else {
                transaction.rollback().await?;
            }
            Ok::<_, sqlx::Error>(all_known)
        })?;
        if !stored {
            return Ok(Err(PrivacyWriteError::UnknownUser));
        }
        Ok(Ok(self.privacy_rule(owner, key).await?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_target_in_both_lists_is_denied_and_the_owner_is_dropped() {
        let (owner, both, allowed) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let write = PrivacyRuleWrite {
            tier: PrivacyTier::Contacts,
            allow_user_ids: vec![both, allowed, owner, allowed],
            deny_user_ids: vec![both, both],
        };
        assert_eq!(
            write.exceptions(owner),
            vec![
                (both, ExceptionEffect::Deny),
                (allowed, ExceptionEffect::Allow)
            ]
        );
    }
}
