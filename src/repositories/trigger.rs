//! Trigger configuration repository implementation.
//!
//! This module provides storage and retrieval of trigger configurations, which define
//! actions to take when monitor conditions are met. The repository loads trigger
//! configurations from JSON files.

#![allow(clippy::result_large_err)]

use std::{collections::HashMap, path::Path};

use async_trait::async_trait;

use crate::{
	models::{ConfigLoader, Trigger},
	repositories::error::RepositoryError,
};

/// Repository for storing and retrieving trigger configurations
#[derive(Clone)]
pub struct TriggerRepository {
	/// Map of trigger names to their configurations
	pub triggers: HashMap<String, Trigger>,
}

impl TriggerRepository {
	/// Create a new trigger repository from the given path
	///
	/// Loads all trigger configurations from JSON files in the specified directory
	/// (or default config directory if None is provided).
	pub async fn new(path: Option<&Path>) -> Result<Self, RepositoryError> {
		let triggers = Self::load_all(path).await?;
		Ok(TriggerRepository { triggers })
	}
}

/// Interface for trigger repository implementations
///
/// This trait defines the standard operations that any trigger repository must support,
/// allowing for different storage backends while maintaining a consistent interface.
#[async_trait]
pub trait TriggerRepositoryTrait: Clone {
	/// Create a new trigger repository from the given path
	async fn new(path: Option<&Path>) -> Result<Self, RepositoryError>
	where
		Self: Sized;

	/// Load all trigger configurations from the given path
	///
	/// If no path is provided, uses the default config directory.
	/// This is a static method that doesn't require an instance.
	async fn load_all(path: Option<&Path>) -> Result<HashMap<String, Trigger>, RepositoryError>;

	/// Get a specific trigger by ID
	///
	/// Returns None if the trigger doesn't exist.
	fn get(&self, trigger_id: &str) -> Option<Trigger>;

	/// Get all triggers
	///
	/// Returns a copy of the trigger map to prevent external mutation.
	fn get_all(&self) -> HashMap<String, Trigger>;
}

#[async_trait]
impl TriggerRepositoryTrait for TriggerRepository {
	async fn new(path: Option<&Path>) -> Result<Self, RepositoryError> {
		TriggerRepository::new(path).await
	}

	async fn load_all(path: Option<&Path>) -> Result<HashMap<String, Trigger>, RepositoryError> {
		Trigger::load_all(path).await.map_err(|e| {
			RepositoryError::load_error(
				"Failed to load triggers",
				Some(Box::new(e)),
				Some(HashMap::from([(
					"path".to_string(),
					path.map_or_else(|| "default".to_string(), |p| p.display().to_string()),
				)])),
			)
		})
	}

	fn get(&self, trigger_id: &str) -> Option<Trigger> {
		self.triggers.get(trigger_id).cloned()
	}

	fn get_all(&self) -> HashMap<String, Trigger> {
		self.triggers.clone()
	}
}

/// Service layer for trigger repository operations
///
/// This type provides a higher-level interface for working with trigger configurations,
/// handling repository initialization and access through a trait-based interface.
#[derive(Clone)]
pub struct TriggerService<T: TriggerRepositoryTrait> {
	repository: T,
}

impl<T: TriggerRepositoryTrait> TriggerService<T> {
	/// Create a new trigger service with the default repository implementation
	pub async fn new(
		path: Option<&Path>,
	) -> Result<TriggerService<TriggerRepository>, RepositoryError> {
		let repository = TriggerRepository::new(path).await?;
		Ok(TriggerService { repository })
	}

	/// Create a new trigger service with a custom repository implementation
	pub fn new_with_repository(repository: T) -> Result<Self, RepositoryError> {
		Ok(TriggerService { repository })
	}

	/// Create a new trigger service with a specific configuration path
	pub async fn new_with_path(
		path: Option<&Path>,
	) -> Result<TriggerService<TriggerRepository>, RepositoryError> {
		let repository = TriggerRepository::new(path).await?;
		Ok(TriggerService { repository })
	}

	/// Get a specific trigger by ID
	pub fn get(&self, trigger_id: &str) -> Option<Trigger> {
		self.repository.get(trigger_id)
	}

	/// Get all triggers
    pub fn get_all(&self) -> HashMap<String, Trigger> {
        self.repository.get_all()
    }

    /// Reload trigger configurations from disk
    pub async fn reload(&mut self, path: Option<&Path>) -> Result<(), RepositoryError> {
        self.repository = T::new(path).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::repositories::error::RepositoryError;
	use std::path::PathBuf;

	#[tokio::test]
	async fn test_load_error_messages() {
		// Test with invalid path to trigger load error
		let invalid_path = PathBuf::from("/non/existent/path");
		let result = TriggerRepository::load_all(Some(&invalid_path)).await;
		assert!(result.is_err());
		let err = result.unwrap_err();
		match err {
			RepositoryError::LoadError(message) => {
				assert!(message.to_string().contains("Failed to load triggers"));
			}
			_ => panic!("Expected RepositoryError::LoadError"),
		}
	}
}

// Note: tests use Rust’s built-in test framework and Tokio for async.
    use tempfile::TempDir;                       // for safe temp directories
    use std::{fs::{File, create_dir_all}, io::Write};
    use crate::models::Trigger;                  // our data model
    use crate::repositories::trigger::{          // repository types under test
        TriggerRepository, TriggerRepositoryTrait, TriggerService
    };

    #[tokio::test]
    async fn test_load_all_success() {
        // Create a temporary directory and write a valid JSON trigger file
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("good_trigger.json");
        let mut file = File::create(&file_path).unwrap();
        let json = r#"{
            "id": "good_id",
            "name": "Good Trigger",
            "condition": "x > 1",
            "action": "do_something"
        }"#;
        file.write_all(json.as_bytes()).unwrap();

        // Load all triggers from the temp directory
        let map = TriggerRepository::load_all(Some(temp_dir.path())).await.unwrap();
        assert_eq!(map.len(), 1);
        // Filename without extension becomes the map key
        assert_eq!(map.keys().next().unwrap(), "good_trigger");

        // Verify each field matches the JSON
        let trigger = map.get("good_trigger").unwrap();
        assert_eq!(trigger.id, "good_id");
        assert_eq!(trigger.name, "Good Trigger");
        assert_eq!(trigger.condition, "x > 1");
        assert_eq!(trigger.action, "do_something");
    }

    #[tokio::test]
    async fn test_load_all_malformed_json() {
        // Create a temporary directory and write malformed JSON
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("bad.json");
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"{ this is not valid json }").unwrap();

        // Attempt to load should return a LoadError
        let result = TriggerRepository::load_all(Some(temp_dir.path())).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            RepositoryError::LoadError(message) => {
                assert!(message.to_string().contains("Failed to load triggers"));
            }
            other => panic!("Expected RepositoryError::LoadError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_get_and_get_all() {
        // Prepare two valid trigger JSON files
        let temp_dir = TempDir::new().unwrap();
        for (name, json) in &[("a.json", r#"{"id":"a_id","name":"A","condition":"true","action":"act"}"#),
                              ("b.json", r#"{"id":"b_id","name":"B","condition":"false","action":"act2"}"#)] {
            let path = temp_dir.path().join(name);
            let mut f = File::create(&path).unwrap();
            f.write_all(json.as_bytes()).unwrap();
        }

        // Initialize repository
        let repo = TriggerRepository::new(Some(temp_dir.path())).await.unwrap();

        // Test existing trigger
        let a = repo.get("a").unwrap();
        assert_eq!(a.id, "a_id");
        // Test non-existent trigger
        assert!(repo.get("nonexistent").is_none());

        // Test get_all returns both entries
        let all = repo.get_all();
        assert_eq!(all.len(), 2);
        let keys: std::collections::HashSet<_> = all.keys().cloned().collect();
        assert_eq!(keys, ["a".to_string(), "b".to_string()].iter().cloned().collect());
    }

    #[tokio::test]
    async fn test_service_new_with_path() {
        // One fixture for service init
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        let mut f = File::create(path.join("svc.json")).unwrap();
        let json = r#"{"id":"svc_id","name":"Svc","condition":"ok","action":"run"}"#;
        f.write_all(json.as_bytes()).unwrap();

        // Initialize service and verify
        let service = TriggerService::new(Some(path)).await.unwrap();
        let t = service.get("svc").unwrap();
        assert_eq!(t.id, "svc_id");
        assert_eq!(service.get_all().len(), 1);
    }

    #[test]
    fn test_service_new_with_repository() {
        use std::path::Path;

        // Dummy repository always returns a single trigger "dummy"
        #[derive(Clone)]
        struct DummyRepo;
        #[async_trait]
        impl TriggerRepositoryTrait for DummyRepo {
            async fn new(_path: Option<&Path>) -> Result<Self, RepositoryError> {
                Ok(DummyRepo)
            }
            async fn load_all(_path: Option<&Path>) -> Result<std::collections::HashMap<String, Trigger>, RepositoryError> {
                let mut m = std::collections::HashMap::new();
                m.insert("dummy".into(), Trigger { id: "dummy".into(), name: "Dummy".into(), condition: "true".into(), action: "none".into() });
                Ok(m)
            }
            fn get(&self, id: &str) -> Option<Trigger> {
                if id == "dummy" {
                    Some(Trigger { id: "dummy".into(), name: "Dummy".into(), condition: "true".into(), action: "none".into() })
                } else {
                    None
                }
            }
            fn get_all(&self) -> std::collections::HashMap<String, Trigger> {
                let mut m = std::collections::HashMap::new();
                m.insert("dummy".into(), Trigger { id: "dummy".into(), name: "Dummy".into(), condition: "true".into(), action: "none".into() });
                m
            }
        }

        // Initialize service with DummyRepo
        let service = TriggerService::new_with_repository(DummyRepo).unwrap();
        // Verify service delegates to DummyRepo
        let d = service.get("dummy").unwrap();
        assert_eq!(d.id, "dummy");
        assert_eq!(service.get_all().len(), 1);
    }

    #[tokio::test]
    async fn test_service_reload() {
        // Set up initial fixture
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        let mut f1 = File::create(path.join("first.json")).unwrap();
        f1.write_all(r#"{"id":"one","name":"One","condition":"1","action":"act1"}"#.as_bytes()).unwrap();

        let mut service = TriggerService::new(Some(path)).await.unwrap();
        assert_eq!(service.get_all().len(), 1);

        // Add a second fixture on disk
        let mut f2 = File::create(path.join("second.json")).unwrap();
        f2.write_all(r#"{"id":"two","name":"Two","condition":"2","action":"act2"}"#.as_bytes()).unwrap();

        // Reload and verify both entries
        service.reload(Some(path)).await.unwrap();
        let all = service.get_all();
        assert_eq!(all.len(), 2);
        assert!(all.contains_key("first") && all.contains_key("second"));
    }