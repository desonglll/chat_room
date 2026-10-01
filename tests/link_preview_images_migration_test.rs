//! TG-1209: the link card image table (`20271201000001`) on both adapters, fresh and upgraded
//! from the schema before it. Checks what `messages::link_previews::images` relies on: one image
//! per cached page, unique access keys, and the cascade from `link_previews`.

mod migration_support;

use std::borrow::Cow;

use chat_room::{config::AppConfig, state::AppState};
use chrono::Utc;
use migration_support::{
    create_postgres_scratch, drop_postgres_scratch, migration_count, migrations_path,
    postgres_admin_pool, postgres_pool, remove_sqlite_files, sqlite_pool, sqlite_scratch_path,
};
use sqlx::migrate::Migrator;
use uuid::Uuid;

const IMAGES_VERSION: i64 = 20271201000001;

async fn everything_but_images(directory: &str) -> Migrator {
    let all = Migrator::new(migrations_path(directory).as_path())
        .await
        .unwrap();
    Migrator {
        migrations: Cow::Owned(
            all.iter()
                .filter(|migration| migration.version != IMAGES_VERSION)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    }
}

macro_rules! assert_image_semantics {
    ($pool:expr) => {{
        let now = Utc::now();
        let page = format!("https://a.example/{}", Uuid::new_v4().simple());
        sqlx::query(
            "INSERT INTO link_previews (url, ok, title, fetched_at) VALUES ($1, TRUE, 't', $2)",
        )
        .bind(&page)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        let key = Uuid::new_v4();
        let image = |url: &str, key: Uuid| {
            sqlx::query(
                "INSERT INTO link_preview_images (url, access_key, content_type, data, fetched_at) \
                 VALUES ($1, $2, 'image/png', $3, $4)",
            )
            .bind(url.to_string())
            .bind(key)
            .bind(vec![1_u8, 2, 3])
            .bind(now)
        };
        image(&page, key).execute($pool).await.unwrap();
        assert!(
            image(&page, Uuid::new_v4()).execute($pool).await.is_err(),
            "one image per page"
        );
        let other = format!("{page}/other");
        sqlx::query(
            "INSERT INTO link_previews (url, ok, title, fetched_at) VALUES ($1, TRUE, 't', $2)",
        )
        .bind(&other)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        assert!(
            image(&other, key).execute($pool).await.is_err(),
            "access keys are unique"
        );
        let data: Vec<u8> =
            sqlx::query_scalar("SELECT data FROM link_preview_images WHERE access_key = $1")
                .bind(key)
                .fetch_one($pool)
                .await
                .unwrap();
        assert_eq!(data, vec![1, 2, 3]);

        sqlx::query("DELETE FROM link_previews WHERE url = $1")
            .bind(&page)
            .execute($pool)
            .await
            .unwrap();
        let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM link_preview_images WHERE url = $1")
            .bind(&page)
            .fetch_one($pool)
            .await
            .unwrap();
        assert_eq!(left, 0, "dropping the cached page must drop its image");
    }};
}

#[tokio::test]
async fn sqlite_fresh_schema_has_link_preview_images() {
    let database = sqlite_scratch_path("lp-images-fresh");
    let state = AppState::open(&database).await.unwrap();
    assert_image_semantics!(state.pool());
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn sqlite_upgrade_gains_link_preview_images() {
    let database = sqlite_scratch_path("lp-images-upgrade");
    let pool = sqlite_pool(&database).await;
    everything_but_images("migrations")
        .await
        .run(&pool)
        .await
        .unwrap();
    pool.close().await;

    let state = AppState::open(&database).await.unwrap();
    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(state.pool())
        .await
        .unwrap();
    assert_eq!(
        applied,
        migration_count(&migrations_path("migrations")).await
    );
    assert_image_semantics!(state.pool());
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn postgres_fresh_schema_has_link_preview_images() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_fresh_schema_has_link_preview_images").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "lp_images_fresh").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    let postgres = state.postgres_pool().unwrap();
    assert_image_semantics!(postgres);
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn postgres_upgrade_gains_link_preview_images() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_upgrade_gains_link_preview_images").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "lp_images_upgrade").await;
    let pool = postgres_pool(&scratch.url).await;
    everything_but_images("migrations-postgres")
        .await
        .run(&pool)
        .await
        .unwrap();
    pool.close().await;

    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    let postgres = state.postgres_pool().unwrap();
    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(postgres)
        .await
        .unwrap();
    assert_eq!(
        applied,
        migration_count(&migrations_path("migrations-postgres")).await
    );
    assert_image_semantics!(postgres);
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn the_link_preview_images_migration_exists_in_both_adapters_under_the_reserved_version() {
    for directory in ["migrations", "migrations-postgres"] {
        let migrator = Migrator::new(migrations_path(directory).as_path())
            .await
            .unwrap();
        assert!(
            migrator
                .iter()
                .any(|migration| migration.version == IMAGES_VERSION
                    && migration.description == "add link preview images"),
            "{directory} is missing 20271201000001_add_link_preview_images"
        );
    }
}
