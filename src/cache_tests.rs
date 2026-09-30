//! Unit tests for [`RedisCache`], split out of `src/cache.rs` to respect the file-size
//! budget (`scripts/check_file_sizes.py`), following the `src/knowledge/store_tests.rs`
//! precedent.

use std::io::Write;

use super::*;

/// Env vars honoured by [`redis_url_or_skip`], in precedence order — the same pair, in the
/// same order, as `tests/service_skip/mod.rs` and the CI workflow.
const REDIS_ENV: &str = "TEST_REDIS_URL";
const REDIS_FALLBACK_ENV: &str = "CHAT_ROOM_REDIS_URL";

/// TG-013's service-availability rule, replicated from `tests/service_skip/mod.rs` — that
/// module is the canonical pattern, but a src/ unit test cannot import it, so the few lines
/// live here too. Keep the two in sync:
///
/// * env unset → skip visibly: one frozen `SKIPPED: Redis not verified: …` marker written
///   straight to the stderr file descriptor, because libtest only captures the `eprintln!`
///   macro family and silently discards a passing test's output;
/// * env set → Redis is required: the caller panics when it is unreachable, so a broken
///   environment cannot hide behind a green run. There is deliberately no
///   probe-and-fall-back URL — a fallback that cannot connect disguises "misconfigured" as
///   "not configured", the exact TG-013 defect.
fn redis_url_or_skip(test_name: &str) -> Option<String> {
    match std::env::var(REDIS_ENV).or_else(|_| std::env::var(REDIS_FALLBACK_ENV)) {
        Ok(url) => Some(url),
        Err(_) => {
            let line = format!(
                "SKIPPED: Redis not verified: {test_name} — {REDIS_ENV} and \
                 {REDIS_FALLBACK_ENV} unset; docker-compose.local.yaml default: \
                 redis://127.0.0.1:6379/\n"
            );
            let mut stderr = std::io::stderr().lock();
            let _ = stderr.write_all(line.as_bytes());
            let _ = stderr.flush();
            None
        }
    }
}

#[tokio::test]
async fn live_ai_answers_keep_the_terminal_snapshot_until_ttl() {
    dotenvy::dotenv().ok();
    let test_name = "live_ai_answers_keep_the_terminal_snapshot_until_ttl";
    let Some(url) = redis_url_or_skip(test_name) else {
        return;
    };
    let config = RedisConfig {
        enabled: true,
        url: url.clone(),
        key_prefix: format!("chat-room-test:{}", Uuid::new_v4()),
        ..RedisConfig::default()
    };
    let cache = RedisCache::connect(&config).await.unwrap_or_else(|error| {
        panic!(
            "{test_name}: {REDIS_ENV} / {REDIS_FALLBACK_ENV} is set but Redis at {url} is \
             unreachable: {error:#}"
        )
    });
    let message_id = Uuid::new_v4();
    let source = AiCitationSource {
        label: "S1".into(),
        room_id: Uuid::new_v4(),
        message_id: Uuid::new_v4(),
        sender: "Ada".into(),
        sent_at: Utc::now(),
        excerpt: "The launch date is Friday".into(),
        score: Some(0.82),
        score_kind: "rerank".into(),
        attachment: None,
    };
    let answer = CachedAiAnswer {
        content: "partial answer".into(),
        context_message_count: 3,
        retrieved_message_count: 1,
        sources: vec![source.clone()],
        trace: Vec::new(),
        revision: 2,
        status: "completed".into(),
        stage: "completed".into(),
        stage_started_at: Some(Utc::now()),
        updated_at: Utc::now(),
    };

    cache.set_ai_answer(message_id, &answer, 60).await.unwrap();
    let cached = cache.ai_answer(message_id).await.unwrap().unwrap();
    assert_eq!(cached.content, "partial answer");
    assert_eq!(cached.retrieved_message_count, 1);
    assert_eq!(cached.sources, vec![source]);
    assert_eq!(cached.revision, 2);
    assert_eq!(cached.status, "completed");
    assert_eq!(cached.stage, "completed");
}
