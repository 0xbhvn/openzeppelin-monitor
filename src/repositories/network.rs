//! Network configuration repository implementation.
//!
//! This module provides storage and retrieval of network configurations, which define
//! blockchain connection details and parameters. The repository loads network
//! configurations from JSON files.

#![allow(clippy::result_large_err)]

use std::{collections::HashMap, path::Path};

use async_trait::async_trait;

use crate::{
	models::{ConfigLoader, Network},
	repositories::error::RepositoryError,
};

/// Repository for storing and retrieving network configurations
#[derive(Clone)]
pub struct NetworkRepository {
	/// Map of network slugs to their configurations
	pub networks: HashMap<String, Network>,
}

impl NetworkRepository {
	/// Create a new network repository from the given path
	///
	/// Loads all network configurations from JSON files in the specified directory
	/// (or default config directory if None is provided).
	pub async fn new(path: Option<&Path>) -> Result<Self, RepositoryError> {
		let networks = Self::load_all(path).await?;
		Ok(NetworkRepository { networks })
	}
}

/// Interface for network repository implementations
///
/// This trait defines the standard operations that any network repository must support,
/// allowing for different storage backends while maintaining a consistent interface.
#[async_trait]
pub trait NetworkRepositoryTrait: Clone {
	/// Create a new repository instance
	async fn new(path: Option<&Path>) -> Result<Self, RepositoryError>
	where
		Self: Sized;

	/// Load all network configurations from the given path
	///
	/// If no path is provided, uses the default config directory.
	/// This is a static method that doesn't require an instance.
	async fn load_all(path: Option<&Path>) -> Result<HashMap<String, Network>, RepositoryError>;

	/// Get a specific network by ID
	///
	/// Returns None if the network doesn't exist.
	fn get(&self, network_id: &str) -> Option<Network>;

	/// Get all networks
	///
	/// Returns a copy of the network map to prevent external mutation.
	fn get_all(&self) -> HashMap<String, Network>;
}

#[async_trait]
impl NetworkRepositoryTrait for NetworkRepository {
	async fn new(path: Option<&Path>) -> Result<Self, RepositoryError> {
		NetworkRepository::new(path).await
	}

	async fn load_all(path: Option<&Path>) -> Result<HashMap<String, Network>, RepositoryError> {
		Network::load_all(path).await.map_err(|e| {
			RepositoryError::load_error(
				"Failed to load networks",
				Some(Box::new(e)),
				Some(HashMap::from([(
					"path".to_string(),
					path.map_or_else(|| "default".to_string(), |p| p.display().to_string()),
				)])),
			)
		})
	}

	fn get(&self, network_id: &str) -> Option<Network> {
		self.networks.get(network_id).cloned()
	}

	fn get_all(&self) -> HashMap<String, Network> {
		self.networks.clone()
	}
}

/// Service layer for network repository operations
///
/// This type provides a higher-level interface for working with network configurations,
/// handling repository initialization and access through a trait-based interface.

#[derive(Clone)]
pub struct NetworkService<T: NetworkRepositoryTrait> {
	repository: T,
}

impl<T: NetworkRepositoryTrait> NetworkService<T> {
	/// Create a new network service with the default repository implementation
	pub async fn new(
		path: Option<&Path>,
	) -> Result<NetworkService<NetworkRepository>, RepositoryError> {
		let repository = NetworkRepository::new(path).await?;
		Ok(NetworkService { repository })
	}

	/// Create a new network service with a custom repository implementation
	pub fn new_with_repository(repository: T) -> Result<Self, RepositoryError> {
		Ok(NetworkService { repository })
	}

	/// Create a new network service with a specific configuration path
	pub async fn new_with_path(
		path: Option<&Path>,
	) -> Result<NetworkService<NetworkRepository>, RepositoryError> {
		let repository = NetworkRepository::new(path).await?;
		Ok(NetworkService { repository })
	}

	/// Get a specific network by ID
	pub fn get(&self, network_id: &str) -> Option<Network> {
		self.repository.get(network_id)
	}

	/// Get all networks
    pub fn get_all(&self) -> HashMap<String, Network> {
        self.repository.get_all()
    }

    /// Reload network configurations from disk
    pub async fn reload(&mut self, path: Option<&Path>) -> Result<(), RepositoryError> {
        self.repository = T::new(path).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_load_error_messages() {
		// Test with invalid path to trigger load error
		let invalid_path = Path::new("/non/existent/path");
		let result = NetworkRepository::load_all(Some(invalid_path)).await;

		assert!(result.is_err());
		let err = result.unwrap_err();
		match err {
			RepositoryError::LoadError(message) => {
				assert!(message.to_string().contains("Failed to load networks"));
			}
			_ => panic!("Expected RepositoryError::LoadError"),
		}
	}
#[tokio::test]
	async fn test_load_all_happy_path() {
		// Prepare a temporary directory for JSON configs
		let dir = std::env::temp_dir().join("network_repo_test");
		let _ = std::fs::remove_dir_all(&dir);
		std::fs::create_dir_all(&dir).unwrap();

		// Write two valid JSON files matching Network's fields
		let alpha = r#"{"id":"alpha","name":"Alpha Network","rpc_url":"https://alpha","chain_id":1}"#;
		std::fs::write(dir.join("alpha.json"), alpha).unwrap();
		let beta = r#"{"id":"beta","name":"Beta Network","rpc_url":"https://beta","chain_id":2}"#;
		std::fs::write(dir.join("beta.json"), beta).unwrap();

		// Invoke the loader
		let networks = NetworkRepository::load_all(Some(&dir)).await.unwrap();

		// Verify both entries are present
		assert_eq!(networks.len(), 2);
		assert!(networks.contains_key("alpha"));
		assert!(networks.contains_key("beta"));

		// Verify one entry’s fields
		let a = networks.get("alpha").unwrap();
		assert_eq!(a.id, "alpha");
		assert_eq!(a.chain_id, 1);

		// Clean up
		std::fs::remove_dir_all(&dir).unwrap();
	}

	#[tokio::test]
	async fn test_get_and_get_all() {
		// Construct a sample Network instance
		let net = Network {
			id: "x".into(),
			name: "X Network".into(),
			rpc_url: "https://x".into(),
			chain_id: 99,
		};
		let mut map = std::collections::HashMap::new();
		map.insert("x".to_string(), net.clone());

		// Build a repository from the map
		let repo = NetworkRepository { networks: map.clone() };

		// Test get existing
		assert_eq!(repo.get("x").unwrap().id, "x");
		// Test get missing
		assert!(repo.get("missing").is_none());

		// Test get_all returns a clone
		let mut all = repo.get_all();
		assert_eq!(all.len(), 1);
		// Mutate returned map and ensure original stays intact
		all.remove("x");
		assert!(repo.get_all().contains_key("x"));
	}

	#[tokio::test]
	async fn test_service_reload() {
		// Setup temp dir with initial JSON
		let dir = std::env::temp_dir().join("network_service_reload");
		let _ = std::fs::remove_dir_all(&dir);
		std::fs::create_dir_all(&dir).unwrap();
		let alpha = r#"{"id":"alpha","name":"Alpha","rpc_url":"https://alpha","chain_id":1}"#;
		std::fs::write(dir.join("alpha.json"), alpha).unwrap();

		// Initialize service
		let mut svc = NetworkService::new(Some(&dir)).await.unwrap();
		assert!(svc.get("alpha").is_some());
		assert!(svc.get("beta").is_none());

		// Add another JSON
		let beta = r#"{"id":"beta","name":"Beta","rpc_url":"https://beta","chain_id":2}"#;
		std::fs::write(dir.join("beta.json"), beta).unwrap();

		// Reload and verify
		svc.reload(Some(&dir)).await.unwrap();
		let all = svc.get_all();
		assert_eq!(all.len(), 2);
		assert!(all.contains_key("beta"));

		// Clean up
		std::fs::remove_dir_all(&dir).unwrap();
	}
}
