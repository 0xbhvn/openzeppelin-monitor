use std::{collections::HashMap, env, path::Path};

use async_trait::async_trait;
use sqlx::{sqlite::SqlitePool, Row};

use crate::{
    models::Network,
    repositories::{error::RepositoryError, NetworkRepositoryTrait},
};

/// Database-backed repository for network configurations
#[derive(Clone)]
pub struct DbNetworkRepository {
    networks: HashMap<String, Network>,
}

impl DbNetworkRepository {
    /// Returns the SQLite connection URL based on the provided path, the `DATABASE_URL` environment variable, or a default value.
    ///
    /// If a path is given, constructs a URL using that path. If not, checks for the `DATABASE_URL` environment variable. If neither is available, defaults to `"sqlite://monitor.db"`.
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
    /// ```    fn db_url(path: Option<&Path>) -> String {
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
impl NetworkRepositoryTrait for DbNetworkRepository {
    /// Asynchronously creates a new `DbNetworkRepository` by loading all networks from the database.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    /// # use your_crate::{DbNetworkRepository, RepositoryError};
    /// # tokio_test::block_on(async {
    /// let repo = DbNetworkRepository::new(Some(Path::new("monitor.db"))).await?;
    /// assert!(repo.get_all().len() >= 0);
    /// # Ok::<(), RepositoryError>(())
    /// # });
    /// ```
    async fn new(path: Option<&Path>) -> Result<Self, RepositoryError> {
    async fn new(path: Option<&Path>) -> Result<Self, RepositoryError> {
        let networks = Self::load_all(path).await?;
        Ok(Self { networks })
    }

    /// Loads all network configurations from the SQLite database.
    ///
    /// Connects to the database, retrieves all entries from the `networks` table, deserializes each network from JSON, and returns a map of network slugs to `Network` objects. Returns a `RepositoryError` if the database connection, query, or deserialization fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    /// # use your_crate::{DbNetworkRepository, RepositoryError};
    /// # tokio_test::block_on(async {
    /// let networks = DbNetworkRepository::load_all(Some(Path::new("monitor.db"))).await?;
    /// assert!(networks.contains_key("mainnet"));
    /// # Ok::<(), RepositoryError>(())
    /// # });
    /// ```
    async fn load_all(path: Option<&Path>) -> Result<HashMap<String, Network>, RepositoryError>
    async fn load_all(path: Option<&Path>) -> Result<HashMap<String, Network>, RepositoryError> {
        let url = Self::db_url(path);
        let pool = SqlitePool::connect(&url)
            .await
            .map_err(|e| RepositoryError::load_error("Failed to connect to database", Some(Box::new(e)), None))?;
        let rows = sqlx::query("SELECT slug, data FROM networks")
            .fetch_all(&pool)
            .await
            .map_err(|e| RepositoryError::load_error("Failed to load networks", Some(Box::new(e)), None))?;
        let mut networks = HashMap::new();
        for row in rows {
            let slug: String = row.try_get("slug").map_err(|e| RepositoryError::load_error("Failed to load networks", Some(Box::new(e)), None))?;
            let data: String = row.try_get("data").map_err(|e| RepositoryError::load_error("Failed to load networks", Some(Box::new(e)), None))?;
            let network: Network = serde_json::from_str(&data).map_err(|e| RepositoryError::load_error("Failed to parse network", Some(Box::new(e)), None))?;
            networks.insert(slug, network);
        }
        Ok(networks)
    }

    /// Retrieves a network configuration by its ID, if it exists.
    ///
    /// Returns a cloned `Network` corresponding to the given `network_id`, or `None` if not found.
    ///
    /// # Examples
    ///
    /// ```
    /// let repo = DbNetworkRepository { networks: HashMap::new() };
    /// assert_eq!(repo.get("mainnet"), None);
    /// ```
    fn get(&self, network_id: &str) -> Option<Network> {
        self.networks.get(network_id).cloned()
    }

    /// Returns a clone of all stored network configurations.
    ///
    /// # Examples
    ///
    /// ```
    /// let repo = DbNetworkRepository { networks: HashMap::new() };
    /// let all_networks = repo.get_all();
    /// assert!(all_networks.is_empty());
    /// ```
    fn get_all(&self) -> HashMap<String, Network> {
        self.networks.clone()
    }
}
