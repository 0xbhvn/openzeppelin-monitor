use std::{collections::HashMap, env, path::Path};

use async_trait::async_trait;
use sqlx::{sqlite::SqlitePool, Row};

use crate::{
    models::Trigger,
    repositories::{error::RepositoryError, TriggerRepositoryTrait},
};

/// Database-backed repository for trigger configurations
#[derive(Clone)]
pub struct DbTriggerRepository {
    triggers: HashMap<String, Trigger>,
}

impl DbTriggerRepository {
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
impl TriggerRepositoryTrait for DbTriggerRepository {
    async fn new(path: Option<&Path>) -> Result<Self, RepositoryError> {
        let triggers = Self::load_all(path).await?;
        Ok(Self { triggers })
    }

    async fn load_all(path: Option<&Path>) -> Result<HashMap<String, Trigger>, RepositoryError> {
        let url = Self::db_url(path);
        let pool = SqlitePool::connect(&url)
            .await
            .map_err(|e| RepositoryError::load_error("Failed to connect to database", Some(Box::new(e)), None))?;
        let rows = sqlx::query("SELECT name, data FROM triggers")
            .fetch_all(&pool)
            .await
            .map_err(|e| RepositoryError::load_error("Failed to load triggers", Some(Box::new(e)), None))?;
        let mut triggers = HashMap::new();
        for row in rows {
            let name: String = row.try_get("name").map_err(|e| RepositoryError::load_error("Failed to load triggers", Some(Box::new(e)), None))?;
            let data: String = row.try_get("data").map_err(|e| RepositoryError::load_error("Failed to load triggers", Some(Box::new(e)), None))?;
            let trigger: Trigger = serde_json::from_str(&data).map_err(|e| RepositoryError::load_error("Failed to parse trigger", Some(Box::new(e)), None))?;
            triggers.insert(name, trigger);
        }
        Ok(triggers)
    }

    fn get(&self, trigger_id: &str) -> Option<Trigger> {
        self.triggers.get(trigger_id).cloned()
    }

    fn get_all(&self) -> HashMap<String, Trigger> {
        self.triggers.clone()
    }
}
