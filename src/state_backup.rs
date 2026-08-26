//! Runtime coordination required while replacing the authoritative database.

use std::{collections::HashMap, sync::atomic::Ordering, time::Duration};

use anyhow::{bail, Context, Result};
use sqlx::{pool::PoolConnection, Postgres};
use tokio::sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::{
    models::Room,
    state::{with_pool, AppState, RoomChannel, SELECT_ROOMS},
};

#[derive(Default)]
pub(crate) struct BackupRuntime {
    operation: Mutex<()>,
    maintenance: std::sync::atomic::AtomicBool,
    requests: RwLock<()>,
}

impl AppState {
    pub(crate) async fn lock_backup_operation(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.backup_runtime.operation.lock().await
    }

    pub(crate) fn begin_maintenance(&self) -> MaintenanceGuard<'_> {
        self.backup_runtime
            .maintenance
            .store(true, Ordering::Release);
        MaintenanceGuard { state: self }
    }

    pub(crate) fn maintenance_active(&self) -> bool {
        self.backup_runtime.maintenance.load(Ordering::Acquire)
    }

    pub(crate) async fn lock_request(&self) -> RwLockReadGuard<'_, ()> {
        self.backup_runtime.requests.read().await
    }

    pub(crate) async fn lock_requests_for_maintenance(&self) -> RwLockWriteGuard<'_, ()> {
        self.backup_runtime.requests.write().await
    }

    pub(crate) async fn lock_postgres_connections(&self) -> Result<PostgresConnections> {
        let pool = self
            .postgres_pool()
            .context("PostgreSQL connection pool is unavailable")?;
        let maximum = pool.options().get_max_connections();
        let timeout = Duration::from_secs(self.config.work_queue.wait_timeout_secs);
        let connections = tokio::time::timeout(timeout, async {
            let mut connections = Vec::with_capacity(maximum as usize);
            for _ in 0..maximum {
                connections.push(pool.acquire().await?);
            }
            Ok::<_, sqlx::Error>(connections)
        })
        .await
        .context("timed out waiting for active database work")??;
        Ok(PostgresConnections { connections })
    }

    pub(crate) async fn reload_room_cache(&self) -> Result<()> {
        let loaded: Vec<Room> = with_pool!(self, |pool| {
            sqlx::query_as(SELECT_ROOMS).fetch_all(pool).await
        })
        .context("reload rooms after database restore")?;
        let rooms: HashMap<_, _> = loaded.iter().cloned().map(|room| (room.id, room)).collect();
        let channels: HashMap<_, _> = loaded
            .into_iter()
            .map(|room| (room.id, RoomChannel::new()))
            .collect();
        *self.rooms.write().await = rooms;
        *self.channels.write().await = channels;
        self.members.write().await.clear();
        Ok(())
    }
}

pub(crate) struct PostgresConnections {
    connections: Vec<PoolConnection<Postgres>>,
}

impl PostgresConnections {
    pub(crate) async fn close(self) -> Result<()> {
        let mut errors = Vec::new();
        for connection in self.connections {
            if let Err(error) = connection.close().await {
                errors.push(error.to_string());
            }
        }
        if !errors.is_empty() {
            bail!("close restored database connections: {}", errors.join("; "));
        }
        Ok(())
    }
}

pub(crate) struct MaintenanceGuard<'a> {
    state: &'a AppState,
}

impl Drop for MaintenanceGuard<'_> {
    fn drop(&mut self) {
        self.state
            .backup_runtime
            .maintenance
            .store(false, Ordering::Release);
    }
}
