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
    /// Compute the SQLite connection URL
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
impl NetworkRepositoryTrait for DbNetworkRepository {
    async fn new(path: Option<&Path>) -> Result<Self, RepositoryError> {
        let networks = Self::load_all(path).await?;
        Ok(Self { networks })
    }

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

    fn get(&self, network_id: &str) -> Option<Network> {
        self.networks.get(network_id).cloned()
    }

    fn get_all(&self) -> HashMap<String, Network> {
        self.networks.clone()
    }
}
