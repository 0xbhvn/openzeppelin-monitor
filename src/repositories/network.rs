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
}
use tempfile::tempdir;
    use serde_json;
    use crate::models::{BlockChainType, RpcUrl};

    // Test loading from an empty directory
    #[tokio::test]
    async fn test_load_all_empty_directory() {
        let dir = tempdir().unwrap();
        let networks = NetworkRepository::load_all(Some(dir.path())).await.unwrap();
        assert!(networks.is_empty(), "Expected no networks in an empty directory");
    }

    // Test loading valid JSON network configurations
    #[tokio::test]
    async fn test_load_all_with_valid_json() {
        let dir = tempdir().unwrap();
        let network = Network {
            slug: "mainnet".to_string(),
            name: "Mainnet".to_string(),
            network_type: BlockChainType::EVM,
            rpc_url: RpcUrl::parse("http://example.com").unwrap(),
            ..Default::default()
        };
        let json = serde_json::to_string(&network).unwrap();
        std::fs::write(dir.path().join("mainnet.json"), json).unwrap();

        let networks = NetworkRepository::load_all(Some(dir.path())).await.unwrap();
        assert_eq!(networks.len(), 1, "Should load exactly one network");
        assert_eq!(networks.get("mainnet"), Some(&network));
    }

    // Test get method for existing and non-existent network
    #[test]
    fn test_get_and_get_nonexistent() {
        let mut map = HashMap::new();
        let fake = Network {
            slug: "fake".to_string(),
            name: "FakeNet".to_string(),
            network_type: BlockChainType::EVM,
            rpc_url: RpcUrl::parse("http://localhost").unwrap(),
            ..Default::default()
        };
        map.insert("fake".to_string(), fake.clone());
        let repo = NetworkRepository { networks: map.clone() };

        assert_eq!(repo.get("fake"), Some(fake.clone()));
        assert_eq!(repo.get("nope"), None);
    }

    // Test get_all returns a clone of the networks map
    #[test]
    fn test_get_all_returns_clone() {
        let mut map = HashMap::new();
        let net = Network {
            slug: "one".to_string(),
            name: "OneNet".to_string(),
            network_type: BlockChainType::EVM,
            rpc_url: RpcUrl::parse("http://localhost").unwrap(),
            ..Default::default()
        };
        map.insert("one".to_string(), net.clone());
        let repo = NetworkRepository { networks: map.clone() };

        let all = repo.get_all();
        assert_eq!(all, map);

        // Mutate the returned map to verify original is untouched
        let mut modified = all.clone();
        modified.clear();
        assert!(!repo.get_all().is_empty(), "Original networks map should be unchanged");
    }

    // Test NetworkService::new and get_all
    #[tokio::test]
    async fn test_service_new_and_get_all() {
        let dir = tempdir().unwrap();
        let network = Network {
            slug: "net".to_string(),
            name: "Net".to_string(),
            network_type: BlockChainType::EVM,
            rpc_url: RpcUrl::parse("http://example.com").unwrap(),
            ..Default::default()
        };
        let json = serde_json::to_string(&network).unwrap();
        std::fs::write(dir.path().join("net.json"), json).unwrap();

        let service = NetworkService::new(Some(dir.path())).await.unwrap();
        let all = service.get_all();
        assert!(all.contains_key("net"), "Service should return the loaded network");
    }

    // Test NetworkService::new_with_repository and get
    #[test]
    fn test_service_new_with_repository_and_get() {
        let dummy = Network {
            slug: "d".to_string(),
            name: "Dummy".to_string(),
            network_type: BlockChainType::EVM,
            rpc_url: RpcUrl::parse("http://localhost").unwrap(),
            ..Default::default()
        };
        let mut map = HashMap::new();
        map.insert("d".to_string(), dummy.clone());
        let repo = NetworkRepository { networks: map.clone() };
        let service = NetworkService::new_with_repository(repo.clone()).unwrap();

        assert_eq!(service.get("d"), Some(dummy.clone()));
        assert_eq!(service.get_all(), map);
    }

    // Test NetworkService::reload
    #[tokio::test]
    async fn test_service_reload() {
        let dir1 = tempdir().unwrap();
        let a = Network {
            slug: "a".to_string(),
            name: "A".to_string(),
            network_type: BlockChainType::EVM,
            rpc_url: RpcUrl::parse("http://example.com").unwrap(),
            ..Default::default()
        };
        let json_a = serde_json::to_string(&a).unwrap();
        std::fs::write(dir1.path().join("a.json"), json_a).unwrap();
        let mut service = NetworkService::new(Some(dir1.path())).await.unwrap();
        assert!(service.get("a").is_some());

        let dir2 = tempdir().unwrap();
        let b = Network {
            slug: "b".to_string(),
            name: "B".to_string(),
            network_type: BlockChainType::EVM,
            rpc_url: RpcUrl::parse("http://example.com").unwrap(),
            ..Default::default()
        };
        let json_b = serde_json::to_string(&b).unwrap();
        std::fs::write(dir2.path().join("b.json"), json_b).unwrap();
        service.reload(Some(dir2.path())).await.unwrap();
        assert!(service.get("b").is_some(), "After reload, should have network 'b'");
        assert!(service.get("a").is_none(), "After reload, old networks should be gone");
    }
}