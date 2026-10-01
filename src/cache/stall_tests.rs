//! TG-1207: a Redis that accepts connections but stops answering must cost at most one
//! command timeout per breaker cooldown, not one per call. Uses an in-process fake Redis so the
//! stall is deterministic and the test needs no external service.

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use uuid::Uuid;

use super::{MessageCacheLookup, RedisCache};
use crate::config::RedisConfig;

const COMMAND_TIMEOUT: Duration = Duration::from_millis(200);

/// Minimal RESP server with an in-memory key space: PING, GET and SET behave like Redis,
/// anything else answers OK. While `stalled` is set it holds every reply (in order, like an
/// overloaded real Redis) until cleared.
async fn fake_redis() -> (String, Arc<AtomicBool>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("redis://{}/", listener.local_addr().unwrap());
    let stalled = Arc::new(AtomicBool::new(false));
    let store: Arc<Mutex<HashMap<String, String>>> = Arc::default();
    let server_stalled = stalled.clone();
    tokio::spawn(async move {
        loop {
            let Ok((socket, _)) = listener.accept().await else {
                return;
            };
            let stalled = server_stalled.clone();
            let store = store.clone();
            tokio::spawn(async move {
                let (read, mut write) = socket.into_split();
                let mut reader = BufReader::new(read);
                while let Some(args) = read_command(&mut reader).await {
                    while stalled.load(Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                    let reply = respond(&store, &args);
                    if write.write_all(reply.as_bytes()).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    (url, stalled)
}

fn respond(store: &Mutex<HashMap<String, String>>, args: &[String]) -> String {
    let name = args.first().map(|name| name.to_ascii_uppercase());
    match (name.as_deref(), args) {
        (Some("PING"), _) => "+PONG\r\n".into(),
        (Some("GET"), [_, key, ..]) => match store.lock().unwrap().get(key) {
            Some(value) => format!("${}\r\n{value}\r\n", value.len()),
            None => "$-1\r\n".into(),
        },
        (Some("SET"), [_, key, value, ..]) => {
            store.lock().unwrap().insert(key.clone(), value.clone());
            "+OK\r\n".into()
        }
        _ => "+OK\r\n".into(),
    }
}

/// Reads one RESP array command and returns its arguments; `None` on EOF.
async fn read_command(
    reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>,
) -> Option<Vec<String>> {
    let mut line = String::new();
    if reader.read_line(&mut line).await.ok()? == 0 {
        return None;
    }
    let count: usize = line.trim().strip_prefix('*')?.parse().ok()?;
    let mut args = Vec::with_capacity(count);
    for _ in 0..count {
        line.clear();
        reader.read_line(&mut line).await.ok()?;
        let len: usize = line.trim().strip_prefix('$')?.parse().ok()?;
        let mut payload = vec![0; len + 2];
        reader.read_exact(&mut payload).await.ok()?;
        payload.truncate(len);
        args.push(String::from_utf8(payload).ok()?);
    }
    Some(args)
}

async fn connect(url: String) -> RedisCache {
    RedisCache::connect(&RedisConfig {
        enabled: true,
        url,
        connect_timeout_ms: 2_000,
        command_timeout_ms: COMMAND_TIMEOUT.as_millis() as u64,
        ..RedisConfig::default()
    })
    .await
    .expect("fake Redis accepts the handshake")
}

#[tokio::test]
async fn a_stalled_redis_costs_one_timeout_per_cooldown_not_one_per_call() {
    let (url, stalled) = fake_redis().await;
    let cache = connect(url).await;
    assert!(cache.get_session(Uuid::new_v4()).await.unwrap().is_none());

    stalled.store(true, Ordering::SeqCst);
    let started = Instant::now();
    let first = cache.get_session(Uuid::new_v4()).await;
    assert!(first.is_err(), "the call that hits the stall reports it");
    for _ in 0..10 {
        // Bypassed: database fallback, reported as a cache miss rather than an error.
        assert!(cache.get_session(Uuid::new_v4()).await.unwrap().is_none());
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed < COMMAND_TIMEOUT * 2,
        "11 calls against a stalled Redis took {elapsed:?}; expected one timeout ({COMMAND_TIMEOUT:?})"
    );
}

#[tokio::test]
async fn message_history_reads_skip_redis_while_the_breaker_is_open() {
    let (url, stalled) = fake_redis().await;
    let cache = connect(url).await;
    stalled.store(true, Ordering::SeqCst);
    let _ = cache.get_session(Uuid::new_v4()).await;

    let started = Instant::now();
    let lookup = cache
        .message_history(Uuid::new_v4(), 50, None, None)
        .await
        .unwrap();
    assert!(matches!(lookup, MessageCacheLookup::Bypass));
    assert!(cache.ping().await.is_err(), "health reports the bypass");
    assert!(started.elapsed() < COMMAND_TIMEOUT / 2);
}

#[tokio::test]
async fn after_recovery_history_pages_are_refreshed_not_served_until_one_ttl_passes() {
    let (url, stalled) = fake_redis().await;
    let cache = connect(url).await;
    let room_id = Uuid::new_v4();
    // A page cached before the outage: exactly what a lost invalidation would leave stale.
    let MessageCacheLookup::Miss(ticket) = cache
        .message_history(room_id, 50, None, None)
        .await
        .unwrap()
    else {
        panic!("empty fake starts with a miss");
    };
    cache.set_message_history(ticket, &[]).await.unwrap();
    assert!(matches!(
        cache
            .message_history(room_id, 50, None, None)
            .await
            .unwrap(),
        MessageCacheLookup::Hit(_)
    ));

    stalled.store(true, Ordering::SeqCst);
    let _ = cache.get_session(Uuid::new_v4()).await;
    stalled.store(false, Ordering::SeqCst);
    // Let the reply held during the stall drain, then open the probe window now instead of
    // sleeping through the real cooldown.
    tokio::time::sleep(Duration::from_millis(50)).await;
    cache
        .breaker
        .record(Instant::now() - super::breaker::OPEN_COOLDOWN, false);
    // The probe (version read) succeeds and closes the breaker, but the cached page is not
    // served: the outage may have swallowed this room's invalidation.
    let lookup = cache
        .message_history(room_id, 50, None, None)
        .await
        .unwrap();
    assert!(
        matches!(lookup, MessageCacheLookup::Miss(_)),
        "a page cached before the failure was served within one TTL of it"
    );
}
