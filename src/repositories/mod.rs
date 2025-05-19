//! Repository implementations for configuration management.
//!
//! This module provides traits and implementations for loading and managing
//! configuration data from the filesystem. Each repository type handles a specific
//! configuration type and provides:
//!
//! - Loading configurations from JSON files
//! - Validating configuration references between different types
//! - Accessing configurations through a service layer
//!
//! Currently supported repositories:
//! - Monitor: Loads and validates monitor configurations, ensuring referenced networks and triggers
//!   exist
//! - Network: Loads network configurations defining blockchain connection details
//! - Trigger: Loads trigger configurations defining actions to take when conditions match

mod error;
mod monitor;
mod network;
mod trigger;
mod db_monitor_repository;
mod db_network_repository;
mod db_trigger_repository;

pub use error::RepositoryError;
pub use monitor::{MonitorRepository, MonitorRepositoryTrait, MonitorService};
pub use network::{NetworkRepository, NetworkRepositoryTrait, NetworkService};
pub use trigger::{TriggerRepository, TriggerRepositoryTrait, TriggerService};
pub use db_monitor_repository::DbMonitorRepository;
pub use db_network_repository::DbNetworkRepository;
pub use db_trigger_repository::DbTriggerRepository;

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that all repository types and services are correctly re-exported.
    #[test]
    fn test_re_exports_exist() {
        // Confirm each type exists by inspecting its type name at compile time.
        assert!(std::any::type_name::<RepositoryError>().ends_with("RepositoryError"));
        assert!(std::any::type_name::<MonitorRepository>().ends_with("MonitorRepository"));
        assert!(std::any::type_name::<MonitorRepositoryTrait>().ends_with("MonitorRepositoryTrait"));
        assert!(std::any::type_name::<MonitorService>().ends_with("MonitorService"));
        assert!(std::any::type_name::<NetworkRepository>().ends_with("NetworkRepository"));
        assert!(std::any::type_name::<NetworkRepositoryTrait>().ends_with("NetworkRepositoryTrait"));
        assert!(std::any::type_name::<NetworkService>().ends_with("NetworkService"));
        assert!(std::any::type_name::<TriggerRepository>().ends_with("TriggerRepository"));
        assert!(std::any::type_name::<TriggerRepositoryTrait>().ends_with("TriggerRepositoryTrait"));
        assert!(std::any::type_name::<TriggerService>().ends_with("TriggerService"));
        assert!(std::any::type_name::<DbMonitorRepository>().ends_with("DbMonitorRepository"));
        assert!(std::any::type_name::<DbNetworkRepository>().ends_with("DbNetworkRepository"));
        assert!(std::any::type_name::<DbTriggerRepository>().ends_with("DbTriggerRepository"));
    }
}