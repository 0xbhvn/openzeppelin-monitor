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
    /// Constructs the SQLite database URL from a provided path, environment variable, or default value.
    ///
    /// If a path is given, returns a URL using that path. Otherwise, attempts to read the `DATABASE_URL` environment variable. If neither is available, defaults to `"sqlite://monitor.db"`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    /// let url = db_url(Some(Path::new("custom.db")));
    /// assert_eq!(url, "sqlite://custom.db");
    ///
    /// std::env::set_var("DATABASE_URL", "sqlite://env.db");
    /// let url_env = db_url(None);
    /// assert_eq!(url_env, "sqlite://env.db");
    ///
    /// std::env::remove_var("DATABASE_URL");
    /// let url_default = db_url(None);
    /// assert_eq!(url_default, "sqlite://monitor.db");
    /// ```
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
    /// Asynchronously creates a new database-backed monitor repository, loading all monitors and validating their references.
    ///
    /// Loads all monitor configurations from the database at the specified path (or default location), retrieves related network and trigger data, and validates monitor references before initializing the repository.
    ///
    /// # Examples
    ///
    /// ```
    /// use my_crate::DbMonitorRepository;
    /// # use my_crate::{NetworkRepository, TriggerRepository, NetworkService, TriggerService};
    /// # use std::path::Path;
    /// # async fn example() {
    /// let repo = DbMonitorRepository::<NetworkRepository, TriggerRepository>::new(
    ///     Some(Path::new("monitor.db")),
    ///     None,
    ///     None,
    /// ).await.unwrap();
    /// assert!(!repo.get_all().is_empty());
    /// # }
    /// ```
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

    /// Asynchronously loads all monitors from the database, deserializes them, and validates their references against available networks and triggers.
    ///
    /// Returns a map of monitor names to `Monitor` objects if successful. Validation ensures that each monitor's referenced networks and triggers exist.
    ///
    /// # Errors
    ///
    /// Returns a `RepositoryError` if the database connection fails, monitor data cannot be loaded or parsed, or if validation fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use my_crate::repositories::DbMonitorRepository;
    /// # use my_crate::repositories::{NetworkRepository, TriggerRepository};
    /// # use std::collections::HashMap;
    /// # tokio_test::block_on(async {
    /// let monitors = DbMonitorRepository::<NetworkRepository, TriggerRepository>::load_all(None, None, None).await?;
    /// assert!(monitors.len() >= 0);
    /// # Ok::<(), my_crate::repositories::RepositoryError>(())
    /// # });
    /// ```
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

    /// Loads a monitor configuration from the specified file path, validating its references against available networks and triggers.
    ///
    /// Returns an error if the path is not provided, the file cannot be loaded, or validation fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    /// # async fn example<N, T>(repo: &DbMonitorRepository<N, T>)
    /// # where
    /// #     N: NetworkRepositoryTrait + Send + Sync,
    /// #     T: TriggerRepositoryTrait + Send + Sync,
    /// # {
    /// let monitor = repo.load_from_path(Some(Path::new("monitor.json")), None, None).await?;
    /// assert_eq!(monitor.name, "example-monitor");
    /// # Ok::<(), RepositoryError>(())
    /// # }
    /// ```
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

    /// Retrieves a monitor by its ID if it exists in the repository.
    ///
    /// # Examples
    ///
    /// ```
    /// let repo = DbMonitorRepository::<_, _> { monitors: monitors_map, _phantom: PhantomData };
    /// if let Some(monitor) = repo.get("monitor1") {
    ///     assert_eq!(monitor.name, "monitor1");
    /// }
    /// ```
    fn get(&self, monitor_id: &str) -> Option<Monitor> {
        self.monitors.get(monitor_id).cloned()
    }

    /// Returns a clone of all monitors stored in the repository.
    ///
    /// # Examples
    ///
    /// ```
    /// let all_monitors = repo.get_all();
    /// assert!(all_monitors.contains_key("example_monitor"));
    /// ```
    fn get_all(&self) -> HashMap<String, Monitor> {
        self.monitors.clone()
    }
}
