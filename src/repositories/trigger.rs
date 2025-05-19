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
	use std::{fs, env};
	use std::fs::File;
	use std::io::Write;

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

	#[tokio::test]
	async fn test_load_all_success() {
		// Arrange: create a temporary directory
		let mut dir = env::temp_dir();
		dir.push("trigger_repo_tests_load_success");
		let _ = fs::remove_dir_all(&dir);
		fs::create_dir_all(&dir).unwrap();

		// Write alpha.json
		let mut f1 = File::create(dir.join("alpha.json")).unwrap();
		write!(f1, r#"{"id":"alpha","name":"Alpha Trigger"}"#).unwrap();
		// Write beta.json
		let mut f2 = File::create(dir.join("beta.json")).unwrap();
		write!(f2, r#"{"id":"beta","name":"Beta Trigger"}"#).unwrap();

		// Act
		let result = TriggerRepository::load_all(Some(&dir)).await;

		// Assert
		assert!(result.is_ok());
		let map = result.unwrap();
		assert_eq!(map.len(), 2);
		let a = map.get("alpha").unwrap();
		assert_eq!(a.id, "alpha");
		let b = map.get("beta").unwrap();
		assert_eq!(b.id, "beta");
	}

	#[tokio::test]
	async fn test_get_and_get_all_methods() {
		// Arrange
		let mut dir = env::temp_dir();
		dir.push("trigger_repo_tests_get");
		let _ = fs::remove_dir_all(&dir);
		fs::create_dir_all(&dir).unwrap();
		let mut f = File::create(dir.join("gamma.json")).unwrap();
		write!(f, r#"{"id":"gamma","name":"Gamma"}"#).unwrap();

		// Act
		let repo = TriggerRepository::new(Some(&dir)).await.unwrap();

		// Assert get()
		let single = repo.get("gamma");
		assert!(single.is_some());
		assert_eq!(single.unwrap().id, "gamma");
		assert!(repo.get("does_not_exist").is_none());

		// Assert get_all()
		let all = repo.get_all();
		assert_eq!(all.len(), 1);
		assert!(all.contains_key("gamma"));
	}

	#[tokio::test]
	async fn test_trigger_service_constructors_and_get() {
		// Arrange
		let mut dir = env::temp_dir();
		dir.push("trigger_service_tests");
		let _ = fs::remove_dir_all(&dir);
		fs::create_dir_all(&dir).unwrap();
		let mut f = File::create(dir.join("delta.json")).unwrap();
		write!(f, r#"{"id":"delta","name":"Delta"}"#).unwrap();

		// Act & Assert: default new
		let service_default = TriggerService::new(Some(&dir)).await.unwrap();
		assert_eq!(service_default.get("delta").unwrap().id, "delta");

		// Act & Assert: new_with_repository
		let repo = TriggerRepository::new(Some(&dir)).await.unwrap();
		let service_custom = TriggerService::new_with_repository(repo.clone()).unwrap();
		assert_eq!(service_custom.get("delta").unwrap().id, "delta");
	}

	#[tokio::test]
	async fn test_trigger_service_reload() {
		// Arrange
		let mut dir = env::temp_dir();
		dir.push("trigger_service_reload_tests");
		let _ = fs::remove_dir_all(&dir);
		fs::create_dir_all(&dir).unwrap();
		// initial files
		let mut f1 = File::create(dir.join("one.json")).unwrap();
		write!(f1, r#"{"id":"one","name":"One"}"#).unwrap();
		let mut f2 = File::create(dir.join("two.json")).unwrap();
		write!(f2, r#"{"id":"two","name":"Two"}"#).unwrap();

		// Act: initial load
		let mut service = TriggerService::new(Some(&dir)).await.unwrap();
		assert_eq!(service.get_all().len(), 2);

		// Mutate directory
		let _ = fs::remove_file(dir.join("one.json"));
		let mut f3 = File::create(dir.join("three.json")).unwrap();
		write!(f3, r#"{"id":"three","name":"Three"}"#).unwrap();

		// Act: reload
		service.reload(Some(&dir)).await.unwrap();

		// Assert
		let all = service.get_all();
		assert_eq!(all.len(), 2);
		assert!(!all.contains_key("one"));
		assert!(all.contains_key("two"));
		assert!(all.contains_key("three"));
	}
}