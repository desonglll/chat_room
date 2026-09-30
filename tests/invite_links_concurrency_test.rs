//! TG-205 acceptance: `usage_count` never oversells. Forty accounts hit one link limited to
//! five at the same moment, over real HTTP against a multi-connection pool — file-backed
//! SQLite (three connections, WAL) and PostgreSQL. Exactly five get in; the other thirty-five
//! are told `limit_reached`; the counter, the roster and `member_count` agree. And one account
//! firing ten concurrent joins at an unlimited link consumes exactly one use.

use std::sync::Arc;

use chat_room::state::AppState;
use futures_util::future::join_all;
use reqwest::{Method, StatusCode};
use serde_json::{json, Value};

mod chat_admin_support;

use chat_admin_support::{serve, with_postgres, Account, Server};

const JOINERS: usize = 40;
const LIMIT: i64 = 5;

/// Accounts created through the domain (not HTTP) so the per-IP registration limit does not
/// throttle the fixture.
async fn accounts(state: &AppState, prefix: &str, count: usize) -> Vec<Account> {
    let mut accounts = Vec::with_capacity(count);
    for index in 0..count {
        let user = state
            .register_user(&format!("{prefix}{index}"), "x", None)
            .await
            .unwrap();
        let session = state.create_session(user.clone()).await.unwrap();
        accounts.push(Account {
            id: user.id.to_string(),
            token: session.token.to_string(),
        });
    }
    accounts
}

async fn create_link(server: &Server, base: &str, owner: &Account, body: Value) -> Value {
    let (status, link) = server
        .call(
            Method::POST,
            &format!("{base}/invite-links"),
            &owner.token,
            Some(body),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{link}");
    link
}

async fn concurrent_joins_scenario(state: Arc<AppState>) {
    let server = serve(state.clone()).await;
    let owner = server.register("tg205-race-owner").await;
    let chat_id = server.create_group(&owner, "并发加入").await;
    let base = format!("/api/chats/{chat_id}");
    let link = create_link(&server, &base, &owner, json!({ "usage_limit": LIMIT })).await;
    let token = link["token"].as_str().unwrap().to_string();
    let joiners = accounts(&state, "tg205race", JOINERS).await;

    let barrier = Arc::new(tokio::sync::Barrier::new(JOINERS));
    let attempts = joiners.iter().map(|account| {
        let barrier = barrier.clone();
        let url = format!("{}/api/invite-links/{token}/join", server.base);
        let client = server.client.clone();
        let bearer = account.token.clone();
        async move {
            barrier.wait().await;
            let response = client.post(url).bearer_auth(bearer).send().await.unwrap();
            let status = response.status();
            (status, response.json::<Value>().await.unwrap())
        }
    });
    let results = join_all(attempts).await;
    let admitted = results
        .iter()
        .filter(|(status, _)| *status == StatusCode::OK)
        .count();
    let refused = results
        .iter()
        .filter(|(status, body)| *status == StatusCode::GONE && body["error"] == "limit_reached")
        .count();
    assert_eq!(admitted, LIMIT as usize, "{results:?}");
    assert_eq!(refused, JOINERS - LIMIT as usize, "{results:?}");

    let (_, view) = server
        .get(&format!("{base}/invite-links"), &owner.token)
        .await;
    let listed = view["links"]
        .as_array()
        .unwrap()
        .iter()
        .find(|candidate| candidate["id"] == link["id"])
        .unwrap()
        .clone();
    assert_eq!(listed["usage_count"], LIMIT);
    assert_eq!(listed["state"], "limit_reached");
    let (_, joined) = server
        .get(
            &format!(
                "{base}/invite-links/{}/members",
                link["id"].as_str().unwrap()
            ),
            &owner.token,
        )
        .await;
    assert_eq!(joined.as_array().unwrap().len(), LIMIT as usize);
    assert_eq!(server.member_count(&chat_id, &owner.token).await, 1 + LIMIT);

    // One account, ten simultaneous joins through an unlimited link: one use.
    let open = create_link(&server, &base, &owner, json!({})).await;
    let open_token = open["token"].as_str().unwrap().to_string();
    let solo = accounts(&state, "tg205solo", 1).await.remove(0);
    let repeats = (0..10).map(|_| {
        let url = format!("{}/api/invite-links/{open_token}/join", server.base);
        let client = server.client.clone();
        let bearer = solo.token.clone();
        async move {
            client
                .post(url)
                .bearer_auth(bearer)
                .send()
                .await
                .unwrap()
                .status()
        }
    });
    let statuses = join_all(repeats).await;
    assert!(
        statuses.iter().all(|status| *status == StatusCode::OK),
        "{statuses:?}"
    );
    let (_, view) = server
        .get(&format!("{base}/invite-links"), &owner.token)
        .await;
    let listed = view["links"]
        .as_array()
        .unwrap()
        .iter()
        .find(|candidate| candidate["id"] == open["id"])
        .unwrap()
        .clone();
    assert_eq!(listed["usage_count"], 1);
    assert_eq!(server.member_count(&chat_id, &owner.token).await, 2 + LIMIT);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_joins_never_oversell_on_sqlite() {
    // Not `migration_support`: it and `chat_admin_support` both load `service_skip`.
    let database = std::env::temp_dir().join(format!("tg205-race-{}.db", uuid::Uuid::new_v4()));
    let state = Arc::new(AppState::open(&database).await.unwrap());
    concurrent_joins_scenario(state.clone()).await;
    state.pool().close().await;
    drop(state);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", database.display()));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_joins_never_oversell_on_postgres() {
    with_postgres(
        "concurrent_joins_never_oversell_on_postgres",
        concurrent_joins_scenario,
    )
    .await;
}
