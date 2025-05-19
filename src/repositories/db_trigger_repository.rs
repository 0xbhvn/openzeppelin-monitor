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
    /// Constructs the SQLite database URL from a file path, environment variable, or default value.
    ///
    /// If a path is provided, returns a URL using that path. Otherwise, uses the `DATABASE_URL` environment variable if set, or defaults to `"sqlite://monitor.db"`.
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
impl TriggerRepositoryTrait for DbTriggerRepository {
    /// Creates a new `DbTriggerRepository` by loading all triggers from the specified SQLite database.
    ///
    /// If a path is provided, it is used to determine the database location; otherwise, the environment variable `DATABASE_URL` or the default `"sqlite://monitor.db"` is used.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    /// let repo = DbTriggerRepository::new(Some(Path::new("mydb.sqlite"))).await?;
    /// assert!(repo.get_all().len() >= 0);
    /// ```
    async fn new(path: Option<&Path>) -> Result<Self, RepositoryError>
    async fn new(path: Option<&Path>) -> Result<Self, RepositoryError> {
        let triggers = Self::load_all(path).await?;
        Ok(Self { triggers })
    }

    /// Loads all triggers from the SQLite database into a map keyed by trigger name.
    ///
    /// Connects asynchronously to the database, retrieves all rows from the `triggers` table,
    /// and deserializes each trigger's JSON data into a `Trigger` object. Returns a map of trigger names to their corresponding `Trigger` instances, or a `RepositoryError` if any step fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    /// # use your_crate::{DbTriggerRepository, TriggerRepositoryTrait};
    /// # tokio_test::block_on(async {
    /// let triggers = DbTriggerRepository::load_all(Some(Path::new("monitor.db"))).await.unwrap();
    /// assert!(triggers.contains_key("example_trigger"));
    /// # });
    /// ```
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

    /// Retrieves a trigger by its ID, returning a cloned copy if found.
    ///
    /// # Examples
    ///
    /// ```
    /// let repo = DbTriggerRepository { triggers: HashMap::new() };
    /// assert!(repo.get("nonexistent").is_none());
    /// ```
    fn get(&self, trigger_id: &str) -> Option<Trigger> {
        self.triggers.get(trigger_id).cloned()
    }

    /// Returns a clone of all triggers stored in the repository.
    ///
    /// # Examples
    ///
    /// ```
    /// let repo = DbTriggerRepository { triggers: HashMap::new() };
    /// let all_triggers = repo.get_all();
    /// assert!(all_triggers.is_empty());
    /// ```
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