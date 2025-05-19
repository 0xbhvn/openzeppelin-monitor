use std::{collections::HashMap, env, marker::PhantomData, path::Path};

use async_trait::async_trait;
use sqlx::{sqlite::SqlitePool, Row};

use crate::{
    models::{Monitor, Network, Trigger},
    repositories::{
        error::RepositoryError,
        network::NetworkRepository,
        trigger::TriggerRepository,
        MonitorRepository, MonitorRepositoryTrait, NetworkRepositoryTrait, NetworkService,
        TriggerRepositoryTrait, TriggerService,
    },
};

/// Database-backed repository for monitor configurations
#[derive(Clone)]
pub struct DbMonitorRepository<N: NetworkRepositoryTrait + Send + 'static, T: TriggerRepositoryTrait + Send + 'static> {
    monitors: HashMap<String, Monitor>,
    _network_repository: PhantomData<N>,
    _trigger_repository: PhantomData<T>,
}

impl<N, T> DbMonitorRepository<N, T>
where
    N: NetworkRepositoryTrait + Send + Sync + 'static,
    T: TriggerRepositoryTrait + Send + Sync + 'static,
{
    fn db_url(path: Option<&Path>) -> String {
        if let Some(p) = path {
            format!("sqlite://{}", p.display())
        } else if let Ok(url) = env::var("DATABASE_URL") {
            url
        } else {
            "sqlite://monitor.db".to_string()
        }
    }
}

#[async_trait]
impl<N, T> MonitorRepositoryTrait<N, T> for DbMonitorRepository<N, T>
where
    N: NetworkRepositoryTrait + Send + Sync + 'static,
    T: TriggerRepositoryTrait + Send + Sync + 'static,
{
    async fn new(
        path: Option<&Path>,
        network_service: Option<NetworkService<N>>,
        trigger_service: Option<TriggerService<T>>,
    ) -> Result<Self, RepositoryError> {
        let monitors = Self::load_all(path, network_service, trigger_service).await?;
        Ok(Self {
            monitors,
            _network_repository: PhantomData,
            _trigger_repository: PhantomData,
        })
    }

    async fn load_all(
        path: Option<&Path>,
        network_service: Option<NetworkService<N>>,
        trigger_service: Option<TriggerService<T>>,
    ) -> Result<HashMap<String, Monitor>, RepositoryError> {
        let url = Self::db_url(path);
        let pool = SqlitePool::connect(&url)
            .await
            .map_err(|e| RepositoryError::load_error("Failed to connect to database", Some(Box::new(e)), None))?;
        let rows = sqlx::query("SELECT name, data FROM monitors")
            .fetch_all(&pool)
            .await
            .map_err(|e| RepositoryError::load_error("Failed to load monitors", Some(Box::new(e)), None))?;
        let mut monitors = HashMap::new();
        for row in rows {
            let name: String = row.try_get("name").map_err(|e| RepositoryError::load_error("Failed to load monitors", Some(Box::new(e)), None))?;
            let data: String = row.try_get("data").map_err(|e| RepositoryError::load_error("Failed to load monitors", Some(Box::new(e)), None))?;
            let monitor: Monitor = serde_json::from_str(&data).map_err(|e| RepositoryError::load_error("Failed to parse monitor", Some(Box::new(e)), None))?;
            monitors.insert(name, monitor);
        }

        let networks = match network_service {
            Some(service) => service.get_all(),
            None => N::new(None).await?.get_all(),
        };

        let triggers = match trigger_service {
            Some(service) => service.get_all(),
            None => T::new(None).await?.get_all(),
        };

        MonitorRepository::<NetworkRepository, TriggerRepository>::validate_monitor_references(&monitors, &triggers, &networks)?;
        Ok(monitors)
    }

    async fn load_from_path(
        &self,
        path: Option<&Path>,
        network_service: Option<NetworkService<N>>,
        trigger_service: Option<TriggerService<T>>,
    ) -> Result<Monitor, RepositoryError> {
        match path {
            Some(p) => {
                let monitor = Monitor::load_from_path(p).await.map_err(|e| RepositoryError::load_error("Failed to load monitors", Some(Box::new(e)), Some(HashMap::from([("path".to_string(), p.display().to_string())]))))?;
                let networks = match network_service {
                    Some(service) => service.get_all(),
                    None => N::new(None).await?.get_all(),
                };
                let triggers = match trigger_service {
                    Some(service) => service.get_all(),
                    None => T::new(None).await?.get_all(),
                };
                let monitors = HashMap::from([(monitor.name.clone(), monitor.clone())]);
                MonitorRepository::<NetworkRepository, TriggerRepository>::validate_monitor_references(&monitors, &triggers, &networks)?;
                Ok(monitor)
            }
            None => Err(RepositoryError::load_error("Failed to load monitors", None, None)),
        }
    }

    fn get(&self, monitor_id: &str) -> Option<Monitor> {
        self.monitors.get(monitor_id).cloned()
    }

    fn get_all(&self) -> HashMap<String, Monitor> {
        self.monitors.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, path::Path, marker::PhantomData};
    use std::collections::HashMap;
    use sqlx::{sqlite::SqlitePool, Executor};
    use tempfile::TempDir;
    use async_trait::async_trait;
    use crate::repositories::{NetworkService, TriggerService};
    use crate::repositories::error::RepositoryError;
    use crate::models::{Network, Trigger, Monitor};

    /// Dummy implementations to satisfy the trait bounds without touching a real database
    struct DummyNetworkService;
    #[async_trait]
    impl crate::repositories::NetworkRepositoryTrait for DummyNetworkService {
        async fn new(_: Option<&Path>) -> Result<Self, RepositoryError> { Ok(Self) }
        fn get_all(&self) -> Vec<Network> { Vec::new() }
    }

    struct DummyTriggerService;
    #[async_trait]
    impl crate::repositories::TriggerRepositoryTrait for DummyTriggerService {
        async fn new(_: Option<&Path>) -> Result<Self, RepositoryError> { Ok(Self) }
        fn get_all(&self) -> Vec<Trigger> { Vec::new() }
    }

    /// Verify db_url() behavior with explicit path, environment variable override, and fallback
    #[tokio::test]
    async fn test_db_url_with_path_env_and_default() {
        let p = Path::new("foo.db");
        assert_eq!(
            DbMonitorRepository::<DummyNetworkService, DummyTriggerService>::db_url(Some(p)),
            "sqlite://foo.db"
        );

        env::set_var("DATABASE_URL", "sqlite://bar.db");
        assert_eq!(
            DbMonitorRepository::<DummyNetworkService, DummyTriggerService>::db_url(None),
            "sqlite://bar.db"
        );
        env::remove_var("DATABASE_URL");

        assert_eq!(
            DbMonitorRepository::<DummyNetworkService, DummyTriggerService>::db_url(None),
            "sqlite://monitor.db"
        );
    }

    /// Expect load_from_path(None, …) to error out immediately
    #[tokio::test]
    async fn test_load_from_path_none() {
        let repo = DbMonitorRepository::<DummyNetworkService, DummyTriggerService> {
            monitors: Default::default(),
            _network_repository: PhantomData,
            _trigger_repository: PhantomData,
        };
        let res = repo.load_from_path(None, None, None).await;
        assert!(res.is_err(), "Expected error when path is None");
    }

    /// When the DB exists but the table is missing, load_all (via new) should return an error
    #[tokio::test]
    async fn test_load_all_missing_table() {
        let url = "sqlite::memory:";
        let pool = SqlitePool::connect(&url).await.unwrap();
        let result = DbMonitorRepository::<DummyNetworkService, DummyTriggerService>::new(
            Some(Path::new(":memory:")),
            Some(NetworkService::new(pool.clone())),
            Some(TriggerService::new(pool))
        ).await;
        assert!(result.is_err(), "Expected load_all to error when tables are missing");
    }

    /// Happy path: empty monitors table yields an empty HashMap
    #[tokio::test]
    async fn test_load_all_empty_table() {
        // Create a temp file and set up an empty monitors table
        let temp = TempDir::new().unwrap();
        let db_path = temp.path().join("empty.db");
        let url = format!("sqlite://{}", db_path.display());
        let pool = SqlitePool::connect(&url).await.unwrap();
        pool.execute("CREATE TABLE monitors (name TEXT PRIMARY KEY, data TEXT)")
            .await
            .unwrap();

        let repo = DbMonitorRepository::<DummyNetworkService, DummyTriggerService>::new(
            Some(&db_path),
            None,
            None
        )
        .await
        .unwrap();
        assert!(repo.get_all().is_empty(), "Expected empty monitors map when table has no rows");
    }

    /// Test get() and get_all() on a manually constructed repository instance
    #[tokio::test]
    async fn test_get_and_get_all() {
        let mut m = Monitor::default();
        m.name = "m1".into();
        let mut map = HashMap::new();
        map.insert("m1".to_string(), m.clone());

        let repo = DbMonitorRepository::<DummyNetworkService, DummyTriggerService> {
            monitors: map.clone(),
            _network_repository: PhantomData,
            _trigger_repository: PhantomData,
        };

        let all = repo.get_all();
        assert_eq!(all.len(), 1);
        assert!(all.contains_key("m1"));

        let single = repo.get("m1");
        assert_eq!(single.unwrap().name, "m1");
        assert!(repo.get("does_not_exist").is_none());
    }
}