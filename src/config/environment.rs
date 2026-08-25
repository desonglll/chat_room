//! Environment overrides used by local Compose and container deployments.

use super::AppConfig;

pub(super) fn apply(config: &mut AppConfig) {
    apply_with(config, |name| std::env::var(name).ok());
}

fn apply_with(config: &mut AppConfig, mut value: impl FnMut(&str) -> Option<String>) {
    if let Some(usernames) = value("CHAT_ROOM_ADMIN_USERNAMES") {
        config.admin.usernames = usernames
            .split(',')
            .map(str::trim)
            .filter(|username| !username.is_empty())
            .map(str::to_string)
            .collect();
    }
    if let Some(database_url) = nonempty(value("CHAT_ROOM_DATABASE_URL")) {
        config.database.kind = "postgres".into();
        config.database.postgres_url = database_url;
    }
    if let Some(url) = value("CHAT_ROOM_REDIS_URL") {
        config.redis.enabled = !url.trim().is_empty();
        config.redis.url = url;
    }
    set_bool(&mut config.ai.enabled, value("CHAT_ROOM_AI_ENABLED"));
    set_string(&mut config.ai.provider, value("CHAT_ROOM_AI_PROVIDER"));
    set_string(&mut config.ai.model, value("CHAT_ROOM_AI_MODEL"));
    if let Some(base_url) = nonempty(value("CHAT_ROOM_AI_BASE_URL")) {
        config.ai.base_url = Some(base_url);
    }
    if let Some(fast_model) = nonempty(value("CHAT_ROOM_AI_FAST_MODEL")) {
        config.ai.fast_model = Some(fast_model);
    }
    set_bool(
        &mut config.vector_store.enabled,
        value("CHAT_ROOM_VECTOR_ENABLED"),
    );
    set_string(&mut config.vector_store.url, value("CHAT_ROOM_VECTOR_URL"));
    set_string(
        &mut config.vector_store.embedding_base_url,
        value("CHAT_ROOM_EMBEDDING_BASE_URL"),
    );
    set_string(
        &mut config.vector_store.embedding_model,
        value("CHAT_ROOM_EMBEDDING_MODEL"),
    );
    set_number(
        &mut config.vector_store.dimensions,
        value("CHAT_ROOM_EMBEDDING_DIMENSIONS"),
    );
    set_string(
        &mut config.vector_store.embedding_api_key_env,
        value("CHAT_ROOM_EMBEDDING_API_KEY_ENV"),
    );
    set_number(
        &mut config.vector_store.top_k,
        value("CHAT_ROOM_VECTOR_TOP_K"),
    );
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.filter(|candidate| !candidate.trim().is_empty())
}

fn set_string(target: &mut String, value: Option<String>) {
    if let Some(value) = nonempty(value) {
        *target = value;
    }
}

fn set_bool(target: &mut bool, value: Option<String>) {
    if let Some(value) = value.and_then(|value| value.parse().ok()) {
        *target = value;
    }
}

fn set_number(target: &mut usize, value: Option<String>) {
    if let Some(value) = value.and_then(|value| value.parse().ok()) {
        *target = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_and_vector_settings_can_come_from_the_environment() {
        let mut config = AppConfig::default();
        apply_with(&mut config, |name| {
            [
                ("CHAT_ROOM_AI_ENABLED", "true"),
                ("CHAT_ROOM_AI_MODEL", "model-from-env"),
                ("CHAT_ROOM_AI_BASE_URL", "https://ai.example/v1"),
                ("CHAT_ROOM_DATABASE_URL", "postgres://db/chatroom"),
                ("CHAT_ROOM_VECTOR_ENABLED", "true"),
                ("CHAT_ROOM_EMBEDDING_MODEL", "embed-from-env"),
                ("CHAT_ROOM_EMBEDDING_BASE_URL", "https://embed.example/v1"),
                ("CHAT_ROOM_EMBEDDING_DIMENSIONS", "768"),
            ]
            .into_iter()
            .find_map(|(key, value)| (key == name).then(|| value.to_owned()))
        });

        assert!(config.ai.enabled);
        assert_eq!(config.database.kind, "postgres");
        assert_eq!(config.database.postgres_url, "postgres://db/chatroom");
        assert_eq!(config.ai.model, "model-from-env");
        assert_eq!(config.ai.base_url.as_deref(), Some("https://ai.example/v1"));
        assert!(config.vector_store.enabled);
        assert_eq!(config.vector_store.embedding_model, "embed-from-env");
        assert_eq!(config.vector_store.dimensions, 768);
    }
}
