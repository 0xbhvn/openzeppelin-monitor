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
