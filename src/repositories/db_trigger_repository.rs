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

#[cfg(test)]
mod tests {
    use super::DbTriggerRepository;
    use crate::models::Trigger;
    use sqlx::SqlitePool;
    use std::{env, fs, path::PathBuf};
    use serde_json;

    /// Creates a temp SQLite DB file and returns its PathBuf and a connected pool
    async fn setup_db(db_name: &str) -> (PathBuf, SqlitePool) {
        let mut path = env::temp_dir();
        path.push(db_name);
        let _ = fs::remove_file(&path);
        let url = format!("sqlite://{}", path.display());
        let pool = SqlitePool::connect(&url).await.unwrap();
        sqlx::query("CREATE TABLE triggers (name TEXT PRIMARY KEY, data TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        (path, pool)
    }

    #[tokio::test]
    async fn load_all_returns_inserted_trigger() {
        let (path, pool) = setup_db("db_trigger_one.db").await;
        let trigger = Trigger { id: "t1".to_string() };
        let data = serde_json::to_string(&trigger).unwrap();
        sqlx::query("INSERT INTO triggers (name, data) VALUES (?, ?)")
            .bind(&trigger.id)
            .bind(&data)
            .execute(&pool)
            .await
            .unwrap();

        let map = DbTriggerRepository::load_all(Some(&path)).await.unwrap();
        assert_eq!(map.get(&trigger.id), Some(&trigger));
    }

    #[tokio::test]
    async fn new_and_repository_getters_work() {
        let (path, pool) = setup_db("db_trigger_two.db").await;
        let trigger1 = Trigger { id: "a".to_string() };
        let trigger2 = Trigger { id: "b".to_string() };
        for trg in [&trigger1, &trigger2] {
            let data = serde_json::to_string(trg).unwrap();
            sqlx::query("INSERT INTO triggers (name, data) VALUES (?, ?)")
                .bind(&trg.id)
                .bind(&data)
                .execute(&pool)
                .await
                .unwrap();
        }

        let repo = DbTriggerRepository::new(Some(&path)).await.unwrap();
        assert_eq!(repo.get("a"), Some(trigger1.clone()));
        assert_eq!(repo.get("b"), Some(trigger2.clone()));
        let all = repo.get_all();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn load_all_missing_table_errors() {
        env::set_var("DATABASE_URL", "sqlite::memory:");
        let result = DbTriggerRepository::load_all(None).await;
        assert!(result.is_err(), "Expected load_all to error when table is missing");
    }
} // end of tests module