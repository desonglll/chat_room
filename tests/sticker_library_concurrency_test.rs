//! TG-302: concurrent install / uninstall / archive / reorder of one account's sticker
//! library keeps a consistent order on both adapters.

mod migration_support;

use std::{collections::HashSet, sync::Arc};

use chat_room::{
    config::AppConfig,
    state::AppState,
    stickers::models::{CreateStickerSetRequest, SetType},
};
use chrono::Utc;
use migration_support::{
    create_postgres_scratch, drop_postgres_scratch, postgres_admin_pool, remove_sqlite_files,
    sqlite_scratch_path,
};
use uuid::Uuid;

const SETS: usize = 12;

async fn user(state: &AppState, label: &str) -> Uuid {
    let id = Uuid::new_v4();
    let insert =
        "INSERT INTO users (id, username, password_hash, created_at) VALUES ($1, $2, 'x', $3)";
    let name = format!("{label}-{}", id.simple());
    match state.postgres_pool() {
        Some(pool) => {
            sqlx::query(insert)
                .bind(id)
                .bind(&name)
                .bind(Utc::now())
                .execute(pool)
                .await
                .unwrap();
        }
        None => {
            sqlx::query(insert)
                .bind(id)
                .bind(&name)
                .bind(Utc::now())
                .execute(state.pool())
                .await
                .unwrap();
        }
    }
    id
}

async fn sets(state: &AppState, owner: Uuid, prefix: &str) -> Vec<Uuid> {
    let mut ids = Vec::new();
    for index in 0..SETS {
        let request = CreateStickerSetRequest {
            short_name: format!("{prefix}_{index}"),
            title: format!("Set {index}"),
            set_type: SetType::Regular,
        };
        ids.push(state.create_sticker_set(owner, &request).await.unwrap().id);
    }
    ids
}

/// Positions as stored, top first, plus a check that they are pairwise distinct.
async fn stored_order(state: &AppState, user_id: Uuid) -> Vec<Uuid> {
    let query =
        "SELECT set_id, position FROM user_sticker_sets WHERE user_id = $1 ORDER BY position";
    let rows: Vec<(Uuid, i64)> = match state.postgres_pool() {
        Some(pool) => sqlx::query_as(query)
            .bind(user_id)
            .fetch_all(pool)
            .await
            .unwrap(),
        None => sqlx::query_as(query)
            .bind(user_id)
            .fetch_all(state.pool())
            .await
            .unwrap(),
    };
    let positions: HashSet<i64> = rows.iter().map(|(_, position)| *position).collect();
    assert_eq!(
        positions.len(),
        rows.len(),
        "two installed sets share a position: {rows:?}"
    );
    rows.into_iter().map(|(set_id, _)| set_id).collect()
}

async fn exercise(state: Arc<AppState>) {
    let owner = user(&state, "sticker-owner").await;
    let fan = user(&state, "sticker-fan").await;
    let prefix = format!("c{}", &Uuid::new_v4().simple().to_string()[..8]);
    let ids = sets(&state, owner, &prefix).await;

    // Round 1: every set installed at once, the same set installed eight times over, and
    // reorders racing the installs.
    let mut tasks = Vec::new();
    for id in ids.iter().copied().chain(std::iter::repeat_n(ids[0], 8)) {
        let state = state.clone();
        tasks.push(tokio::spawn(async move {
            state.install_sticker_set(fan, id).await.map(|_| ())
        }));
    }
    for round in 0..4 {
        let state = state.clone();
        let mut order = ids.clone();
        order.rotate_left(round * 3);
        tasks.push(tokio::spawn(async move {
            state.reorder_sticker_sets(fan, order).await.map(|_| ())
        }));
    }
    for task in tasks {
        task.await.unwrap().unwrap();
    }
    let order = stored_order(&state, fan).await;
    assert_eq!(order.len(), SETS, "each set installed exactly once");
    assert_eq!(
        order.iter().collect::<HashSet<_>>(),
        ids.iter().collect::<HashSet<_>>()
    );

    // Round 2: uninstall a third, archive a third and reorder, all concurrently.
    let mut tasks = Vec::new();
    for (index, id) in ids.iter().copied().enumerate() {
        let state = state.clone();
        tasks.push(tokio::spawn(async move {
            match index % 3 {
                0 => state.uninstall_sticker_set(fan, id).await.map(|_| ()),
                1 => state.archive_sticker_set(fan, id, true).await.map(|_| ()),
                _ => state.reorder_sticker_sets(fan, vec![id]).await.map(|_| ()),
            }
        }));
    }
    for task in tasks {
        task.await.unwrap().unwrap();
    }
    let order = stored_order(&state, fan).await;
    let expected: HashSet<Uuid> = ids
        .iter()
        .enumerate()
        .filter(|(index, _)| index % 3 != 0)
        .map(|(_, id)| *id)
        .collect();
    assert_eq!(order.iter().copied().collect::<HashSet<_>>(), expected);

    // Every write bumped the revision exactly once, and the API order matches storage.
    let library = state.installed_sticker_sets(fan).await.unwrap();
    assert_eq!(library.revision as usize, SETS + 8 + 4 + SETS);
    let listed: Vec<Uuid> = library.sets.iter().map(|set| set.id).collect();
    assert_eq!(listed, order);
    let archived = library.sets.iter().filter(|set| set.archived).count();
    assert_eq!(archived, SETS / 3);

    // A final explicit reorder is honoured exactly.
    let mut wanted = order.clone();
    wanted.reverse();
    let reordered = state
        .reorder_sticker_sets(fan, wanted.clone())
        .await
        .unwrap();
    assert_eq!(
        reordered.sets.iter().map(|set| set.id).collect::<Vec<_>>(),
        wanted
    );
    assert_eq!(stored_order(&state, fan).await, wanted);
}

#[tokio::test]
async fn sqlite_library_writes_serialise_per_account() {
    let database = sqlite_scratch_path("sticker-library");
    let state = Arc::new(AppState::open(&database).await.unwrap());
    exercise(state.clone()).await;
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn postgres_library_writes_serialise_per_account() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_library_writes_serialise_per_account").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "sticker_library").await;
    let state = Arc::new(
        AppState::open_postgres(&scratch.url, &AppConfig::default())
            .await
            .unwrap(),
    );
    exercise(state.clone()).await;
    state.postgres_pool().unwrap().close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}
